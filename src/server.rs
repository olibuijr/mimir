//! `mimir serve`: the HTTP API.
use crate::db;
use crate::models::{clip_query, doc_text, dot, query_text, Models};
use crate::spec;
use crate::types::{Hit, MatchReq, MatchResp, Skill, Summary};
use anyhow::Result;
use axum::extract::{DefaultBodyLimit, Path, Request, State};
use axum::http::{header, HeaderMap, HeaderValue, Method, StatusCode};
use axum::middleware::Next;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use rusqlite::Connection;
use serde_json::{json, Value};
use std::convert::Infallible;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;
use std::time::Instant;

/// Skills sent to the reranker per prompt (picked by cosine).
pub const CANDIDATES: usize = 12;
/// Default reranker score a skill needs to be suggested. Tuned with `mimir eval`.
pub const DEFAULT_MIN: f32 = 0.1;

pub struct App {
    pub db: Mutex<Connection>,
    pub models: Models,
    pub token: String,
    /// Live events (JSON) for the web UI: match, load, skill, log_cleared.
    pub events: broadcast::Sender<String>,
}

impl App {
    /// Tell every connected UI that something changed. No listeners is fine.
    pub fn emit(&self, v: Value) {
        let _ = self.events.send(v.to_string());
    }

    /// DB handle; survives a poisoned mutex (a panicked request must not brick the server).
    pub fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.db.lock().unwrap_or_else(|e| e.into_inner())
    }
}

pub struct E(pub StatusCode, pub String);
impl IntoResponse for E {
    fn into_response(self) -> Response {
        if self.0.is_server_error() {
            eprintln!("{} {}", self.0.as_u16(), crate::models::clip(&self.1, 300));
        }
        (self.0, Json(json!({ "error": self.1 }))).into_response()
    }
}
impl From<anyhow::Error> for E {
    fn from(e: anyhow::Error) -> Self {
        E(StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}"))
    }
}
impl From<rusqlite::Error> for E {
    fn from(e: rusqlite::Error) -> Self {
        E(StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    }
}

type S = State<Arc<App>>;

/// Constant-time equality (length is not secret).
pub fn ct_eq(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn unauthorized() -> E {
    E(StatusCode::UNAUTHORIZED, "unauthorized".into())
}

/// Bearer token only. Mutating skills never accepts the browser cookie.
pub fn auth_bearer(app: &App, h: &HeaderMap) -> Result<(), E> {
    let got = h.get("authorization").and_then(|v| v.to_str().ok()).and_then(|v| v.strip_prefix("Bearer "));
    if got.is_some_and(|t| ct_eq(t, &app.token)) {
        Ok(())
    } else {
        Err(unauthorized())
    }
}

/// Bearer token or the `mimir` cookie set by the web UI login.
pub fn auth(app: &App, h: &HeaderMap) -> Result<(), E> {
    if auth_bearer(app, h).is_ok() {
        return Ok(());
    }
    let cookie = h.get("cookie").and_then(|v| v.to_str().ok()).unwrap_or("");
    let got = cookie.split(';').filter_map(|p| p.trim().strip_prefix("mimir=")).next();
    if got.is_some_and(|t| ct_eq(t, &app.token)) {
        Ok(())
    } else {
        Err(unauthorized())
    }
}

/// DB and model calls are blocking; keep them off the async workers.
pub async fn blocking<T: Send + 'static>(app: Arc<App>, f: impl FnOnce(&App) -> Result<T, E> + Send + 'static) -> Result<T, E> {
    tokio::task::spawn_blocking(move || f(&app))
        .await
        .map_err(|e| E(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
}

/// Paths anyone may fetch: the health probe, the embedded UI and the login form post.
fn is_public(m: &Method, path: &str) -> bool {
    matches!((m, path), (&Method::GET, "/health" | "/" | "/app.js" | "/app.css") | (&Method::POST, "/api/login" | "/api/logout"))
}

/// Skill bodies carry secrets and mutations change state: these never accept the browser cookie.
fn bearer_only(path: &str) -> bool {
    path.starts_with("/skills/") || path == "/reembed" || path == "/api/dist/mimir"
}

/// The one gate: authenticates BEFORE any body is read, and stamps every response with the version.
async fn gate(State(app): S, req: Request, next: Next) -> Response {
    let (m, path) = (req.method().clone(), req.uri().path().to_string());
    let denied = if is_public(&m, &path) {
        None
    } else if bearer_only(&path) {
        auth_bearer(&app, req.headers()).err()
    } else {
        auth(&app, req.headers()).err()
    };
    let mut r = match denied {
        Some(e) => e.into_response(),
        None => next.run(req).await,
    };
    r.headers_mut().insert("x-mimir-version", HeaderValue::from_static(env!("CARGO_PKG_VERSION")));
    r
}

pub fn router(app: Arc<App>) -> Router {
    Router::new()
        .route("/health", get(|| async { Json(json!({ "ok": true, "version": env!("CARGO_PKG_VERSION") })) }))
        .route("/skills", get(list))
        .route("/skills/{name}", get(get_skill).delete(delete_skill))
        // Some skills ship megabytes of reference data (base64 adds a third): only PUT gets the big limit.
        .route("/skills/{name}", put(put_skill).route_layer(DefaultBodyLimit::max(64 << 20)))
        .route("/match", post(match_skills))
        .route("/reembed", post(reembed))
        .route("/api/events", get(events))
        .route("/api/dist/mimir", get(dist))
        .merge(crate::api::routes())
        .merge(crate::web::routes())
        .layer(axum::middleware::from_fn_with_state(app.clone(), gate))
        .with_state(app)
}

/// The server's own executable, for `mimir update`.
async fn dist() -> Result<Response, E> {
    let bytes = tokio::task::spawn_blocking(|| -> Result<Vec<u8>> { Ok(std::fs::read(std::env::current_exe()?)?) })
        .await
        .map_err(|e| E(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))??;
    Ok(([(header::CONTENT_TYPE, "application/octet-stream")], bytes).into_response())
}

async fn list(State(app): S) -> Result<Json<Vec<Summary>>, E> {
    blocking(app, |a| Ok(db::list(&a.conn())?)).await.map(Json)
}

async fn get_skill(State(app): S, Path(name): Path<String>, h: HeaderMap) -> Result<Json<Skill>, E> {
    let load = h.get("x-mimir-load").and_then(|v| v.to_str().ok()) == Some("1");
    blocking(app, move |a| {
        let conn = a.conn();
        let s = db::get(&conn, &name)?.ok_or(E(StatusCode::NOT_FOUND, format!("no skill named {name}")))?;
        // Only a real use counts as a load (MCP skill_get, `mimir get`), not scripts reading skills.
        if load {
            db::log_load(&conn, &name)?;
            a.emit(json!({ "type": "load", "skill": name, "ts": now() }));
        }
        Ok(s)
    })
    .await
    .map(Json)
}

async fn put_skill(State(app): S, Path(name): Path<String>, Json(mut s): Json<Skill>) -> Result<Json<Summary>, E> {
    blocking(app, move |a| {
        check_put(&name, &mut s)?;
        // The frontmatter is the single source of the description.
        s.description = spec::parse(&s.body).0.description.unwrap_or_default().trim().to_string();
        s.name = name;
        let e = a.models.embed_batch(&[doc_text(&s.name, &s.description)])?.remove(0);
        let r = db::put(&a.conn(), &s, &e, &a.models.embed_model)?;
        a.emit(json!({ "type": "skill", "action": "put", "name": r.name, "version": r.version }));
        Ok(r)
    })
    .await
    .map(Json)
}

/// Check a PUT before anything is stored. Files sent: they must decode and have safe paths.
/// Files omitted: the stored ones stay, and are not read here. Spec errors never depend on the
/// file list beyond the path checks (stored paths passed them when written); it only adds warnings,
/// which a PUT does not return. Reading the stored list would also race with a concurrent PUT.
fn check_put(name: &str, s: &mut Skill) -> Result<(), E> {
    let paths = checked_files(s).map_err(|e| E(StatusCode::BAD_REQUEST, format!("{name}: {e}")))?.unwrap_or_default();
    let paths: Vec<&str> = paths.iter().map(String::as_str).collect();
    let report = spec::validate(name, &s.body, &paths);
    if !report.ok() {
        return Err(E(
            StatusCode::UNPROCESSABLE_ENTITY,
            format!("{name}: SKILL.md does not follow the Agent Skills spec (agentskills.io/specification): {}", report.errors.join("; ")),
        ));
    }
    Ok(())
}

/// Validate the files of a PUT and store the checked copies (modes masked to 0o777) back into the skill.
/// Returns their paths, or None when the PUT sent no files.
fn checked_files(s: &mut Skill) -> Result<Option<Vec<String>>, String> {
    let Some(files) = &s.files else { return Ok(None) };
    let clean = crate::types::check_files(files)?;
    let paths = clean.iter().map(|f| f.path.clone()).collect();
    s.files = Some(clean);
    Ok(Some(paths))
}

async fn delete_skill(State(app): S, Path(name): Path<String>) -> Result<Json<Value>, E> {
    blocking(app, move |a| {
        if db::delete(&a.conn(), &name)? {
            a.emit(json!({ "type": "skill", "action": "delete", "name": name }));
            Ok(Json(json!({ "deleted": name })))
        } else {
            Err(E(StatusCode::NOT_FOUND, format!("no skill named {name}")))
        }
    })
    .await
}

async fn match_skills(State(app): S, h: HeaderMap, Json(req): Json<MatchReq>) -> Result<Json<MatchResp>, E> {
    let client = h
        .get("x-mimir-client")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.chars().filter(|c| !c.is_control()).take(64).collect::<String>())
        .filter(|v| !v.is_empty());
    let client = match req.source.as_deref() {
        Some("mcp") => Some(format!("{}{}", db::MCP_PREFIX, client.as_deref().unwrap_or("unknown"))),
        _ => client,
    };
    blocking(app, move |a| Ok(do_match(a, &req, client.as_deref())?)).await.map(Json)
}

async fn reembed(State(app): S) -> Result<Json<Value>, E> {
    blocking(app, |a| {
        let rows: Vec<(String, String)> = {
            let c = a.conn();
            let mut st = c.prepare("SELECT name, description FROM skills")?;
            let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<_>>()?;
            rows
        };
        let texts: Vec<String> = rows.iter().map(|(n, d)| doc_text(n, d)).collect();
        let mut done = 0;
        for (chunk, names) in texts.chunks(16).zip(rows.chunks(16)) {
            let embs = a.models.embed_batch(chunk)?;
            let c = a.conn();
            for ((n, _), e) in names.iter().zip(embs) {
                db::set_embedding(&c, n, &e, &a.models.embed_model)?;
                done += 1;
            }
        }
        a.emit(json!({ "type": "skill", "action": "reembed" }));
        Ok(Json(json!({ "reembedded": done })))
    })
    .await
}

/// Server-sent events for the web UI. A client that falls behind gets `resync` and refetches.
async fn events(State(app): S) -> Result<Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>>, E> {
    let rx = app.events.subscribe();
    let stream = futures_util::stream::unfold(rx, |mut rx| async move {
        let data = match rx.recv().await {
            Ok(m) => m,
            Err(broadcast::error::RecvError::Lagged(_)) => r#"{"type":"resync"}"#.to_string(),
            Err(broadcast::error::RecvError::Closed) => return None,
        };
        Some((Ok(Event::default().data(data)), rx))
    });
    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

/// Common words that say nothing about which skill is needed.
const STOPWORDS: &[&str] = &[
    "about", "after", "also", "been", "before", "being", "could", "does", "doing", "done", "each", "from", "have", "here", "into", "just", "like",
    "make", "more", "most", "much", "need", "only", "other", "over", "please", "should", "some", "such", "than", "that", "their", "them", "then",
    "there", "these", "they", "this", "those", "through", "using", "want", "were", "what", "when", "where", "which", "while", "will", "with",
    "would", "your", "skill", "skills", "file", "files", "use", "used", "uses",
];

/// Lowercase alphanumeric tokens of at least 4 chars, minus stopwords, deduplicated.
pub fn tokens(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for t in text.to_lowercase().split(|c: char| !c.is_alphanumeric()) {
        if t.chars().count() >= 4 && !STOPWORDS.contains(&t) && !out.iter().any(|o| o == t) {
            out.push(t.to_string());
        }
    }
    out
}

/// Skills whose name+description holds a prompt token that no other skill has, most shared tokens first.
pub fn lexical_rescue(prompt: &str, skills: &[(String, String)]) -> Vec<String> {
    let mut index: std::collections::HashMap<String, Vec<usize>> = std::collections::HashMap::new();
    for (i, (n, d)) in skills.iter().enumerate() {
        for t in tokens(&format!("{n} {d}")) {
            index.entry(t).or_default().push(i);
        }
    }
    let mut votes: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
    for t in tokens(prompt) {
        if let Some([i]) = index.get(&t).map(Vec::as_slice) {
            *votes.entry(*i).or_default() += 1;
        }
    }
    let mut v: Vec<(usize, usize)> = votes.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    v.into_iter().map(|(i, _)| skills[i].0.clone()).collect()
}

/// Extra reranker candidates the lexical rescue may add.
const RESCUE_MAX: usize = 4;

fn lexical_default() -> bool {
    std::env::var("MIMIR_LEXICAL").is_ok_and(|v| v == "1")
}

/// Embed the prompt, take the closest skills by cosine, let the reranker judge them.
pub fn do_match(a: &App, req: &MatchReq, client: Option<&str>) -> Result<MatchResp> {
    let t = Instant::now();
    let k = req.k.unwrap_or(3);
    let min = req.min.unwrap_or(DEFAULT_MIN);
    let q = req.prompt.trim();
    let (clipped, prompt_len) = clip_query(q);
    let qv = a
        .models
        .embed(&[query_text(&clipped)])
        .map_err(|e| {
            eprintln!("match: embed failed after {} ms: {e:#}", t.elapsed().as_millis());
            e
        })?
        .remove(0);
    let mut all: Vec<(String, String, f32)> = {
        let c = a.conn();
        db::vectors(&c)?.into_iter().map(|(n, d, v)| { let s = dot(&qv, &v); (n, d, s) }).collect()
    };
    all.sort_by(|x, y| y.2.total_cmp(&x.2));
    let mut cands: Vec<(String, String, f32)> = all.iter().take(CANDIDATES).cloned().collect();
    if req.lexical.unwrap_or_else(lexical_default) {
        let skills: Vec<(String, String)> = all.iter().map(|(n, d, _)| (n.clone(), d.clone())).collect();
        let extra: Vec<String> = lexical_rescue(q, &skills).into_iter().filter(|n| !cands.iter().any(|c| &c.0 == n)).take(RESCUE_MAX).collect();
        for name in extra {
            if let Some(c) = all.iter().find(|c| c.0 == name) {
                cands.push(c.clone());
            }
        }
    }

    let scores = if req.rerank.unwrap_or(true) {
        let docs: Vec<String> = cands.iter().map(|(n, d, _)| format!("{n}: {d}")).collect();
        a.models.rerank(&clipped, &docs).map_err(|e| {
            eprintln!("match: rerank failed after {} ms: {e:#}", t.elapsed().as_millis());
            e
        })?
    } else {
        cands.iter().map(|c| c.2).collect()
    };
    let mut hits: Vec<Hit> = cands
        .into_iter()
        .zip(scores)
        .map(|((name, description, cosine), score)| Hit { name, description, score, cosine })
        .filter(|h| h.score >= min)
        .collect();
    hits.sort_by(|x, y| y.score.total_cmp(&x.score));
    hits.truncate(k);
    let ms = t.elapsed().as_millis() as u64;
    if req.log.unwrap_or(true) {
        let names: Vec<_> = hits.iter().map(|h| json!([h.name, h.score])).collect();
        let id = db::log_match(&a.conn(), &clipped, &Value::Array(names).to_string(), ms, client, prompt_len)?;
        let row_hits: Vec<_> = hits.iter().map(|h| json!({ "name": h.name, "score": h.score })).collect();
        a.emit(json!({ "type": "match", "row": {
            "id": id, "ts": now(), "prompt": clipped, "hits": row_hits, "ms": ms, "loaded": [],
            "client": client, "prompt_len": prompt_len,
        } }));
    }
    Ok(MatchResp { hits, ms })
}

/// Log rows older than this are pruned (30 days).
const RETENTION: i64 = 30 * 86400;

fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

fn backup_now(app: &App, dir: &std::path::Path) {
    match db::backup(&app.conn(), dir, 7) {
        Ok(p) => eprintln!("backup written: {}", p.display()),
        Err(e) => eprintln!("backup failed: {e:#}"),
    }
}

pub fn serve(bind: &str, db_path: &std::path::Path) -> Result<()> {
    let token = std::env::var("MIMIR_TOKEN").map_err(|_| anyhow::anyhow!("MIMIR_TOKEN must be set for serve"))?;
    anyhow::ensure!(token.len() >= 16, "MIMIR_TOKEN must be at least 16 characters");
    let (events, _) = broadcast::channel(256);
    let app = Arc::new(App { db: Mutex::new(db::open(db_path)?), models: Models::from_env(), token, events });
    {
        let c = app.conn();
        db::prune(&c, now() - RETENTION)?;
        if let Ok(Some(stored)) = db::meta_get(&c, "embed_model") {
            if stored != app.models.embed_model {
                let dim = db::meta_get(&c, "embed_dim").ok().flatten().unwrap_or_default();
                eprintln!(
                    "WARNING: stored embeddings come from model {stored:?} (dim {dim}) but the configured model is {:?}; run `mimir reembed` or matching will be wrong",
                    app.models.embed_model
                );
            }
        }
    }
    let backups = db_path.parent().map(|p| p.to_path_buf()).filter(|p| !p.as_os_str().is_empty()).unwrap_or_else(|| ".".into()).join("backups");
    backup_now(&app, &backups);
    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build()?;
    rt.block_on(async {
        let l = tokio::net::TcpListener::bind(bind).await?;
        eprintln!("mimir {} serving on {bind}, db {}", env!("CARGO_PKG_VERSION"), db_path.display());
        // Daily retention prune and backup on a plain thread (tokio's time feature is not enabled).
        let a = app.clone();
        std::thread::spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_secs(86400));
            if let Err(e) = db::prune(&a.conn(), now() - RETENTION) {
                eprintln!("prune failed: {e:#}");
            }
            backup_now(&a, &backups);
        });
        axum::serve(l, router(app)).await?;
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn put_stores_masked_file_modes() {
        let file = |mode| crate::types::File { path: "scripts/a.sh".into(), mode, b64: "aGk=".into() };
        let mut s = Skill { files: Some(vec![file(0o104755)]), ..Default::default() };
        assert_eq!(checked_files(&mut s).unwrap(), Some(vec!["scripts/a.sh".to_string()]));
        assert_eq!(s.files.unwrap()[0].mode, 0o755);
        assert_eq!(checked_files(&mut Skill::default()).unwrap(), None);
        assert!(checked_files(&mut Skill { files: Some(vec![crate::types::File { path: "../x".into(), mode: 0o644, b64: String::new() }]), ..Default::default() }).is_err());
    }

    #[test]
    fn put_check_needs_no_stored_files() {
        let body = "---\nname: demo\ndescription: Demo skill. See references/guide.md and run scripts/go.sh.\n---\n# demo\n";
        let mut s = Skill { body: body.into(), ..Default::default() };
        assert!(check_put("demo", &mut s).is_ok());
        // Clean file lists only change warnings, never the verdict.
        assert_eq!(spec::validate("demo", body, &[]).errors, spec::validate("demo", body, &["references/guide.md", "scripts/go.sh"]).errors);
        assert_eq!(check_put("Demo", &mut s.clone()).unwrap_err().0, StatusCode::UNPROCESSABLE_ENTITY);
        let bad = crate::types::File { path: "../x".into(), mode: 0o644, b64: String::new() };
        assert_eq!(check_put("demo", &mut Skill { files: Some(vec![bad]), ..s }).unwrap_err().0, StatusCode::BAD_REQUEST);
    }

    #[test]
    fn tokenizer_filters() {
        assert_eq!(tokens("Fix the PDF-merge bug, please! use it"), vec!["merge".to_string()]);
        assert_eq!(tokens("rust rust Rust"), vec!["rust".to_string()]);
    }

    #[test]
    fn rescue_needs_a_unique_token() {
        let skills = vec![
            ("pdf-tools".to_string(), "Merge and split PDF documents".to_string()),
            ("xlsx-tools".to_string(), "Edit spreadsheet documents".to_string()),
        ];
        // "documents" is in both skills, "spreadsheet" only in one.
        assert_eq!(lexical_rescue("open the documents", &skills), Vec::<String>::new());
        assert_eq!(lexical_rescue("clean this spreadsheet and documents", &skills), vec!["xlsx-tools".to_string()]);
    }
}
