//! Read API for the web UI: /api/config, /skills, /stats, /log. Login/logout live in web.rs.
use crate::db::{self, now};
use crate::models::{dot, Models};
use crate::server::{auth, blocking, App, E, CANDIDATES, DEFAULT_MIN};
use crate::spec;
use crate::types::{DayCount, SkillDetail, SkillRow, Similar};
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use pulldown_cmark::{html::push_html, CowStr, Event, Options, Parser, Tag};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

type S = State<Arc<App>>;
const DAY: i64 = 86400;

pub fn routes() -> Router<Arc<App>> {
    Router::new()
        .route("/api/config", get(config))
        .route("/api/skills", get(skills))
        .route("/api/skills/{name}", get(skill))
        .route("/api/skills/{name}/history", get(history))
        .route("/api/skills/{name}/history/{version}", get(history_version))
        .route("/api/skills/{name}/files/{*path}", get(file))
        .route("/api/stats", get(stats))
        .route("/api/log", get(log).delete(clear_log))
}

fn models_ok(m: &Models) -> bool {
    [&m.url, &m.rerank_url].iter().all(|u| ureq::get(&format!("{u}/v1/models")).timeout(Duration::from_secs(1)).call().is_ok())
}

async fn config(State(app): S, h: HeaderMap) -> Result<Json<Value>, E> {
    auth(&app, &h)?;
    blocking(app, |a| {
        let t = now();
        let (skills, log_rows, db_bytes, stored, mismatch, first, backup_at) = {
            let c = a.conn();
            let (s, l, b) = db::counts(&c)?;
            let backup_at = db::db_dir(&c).and_then(|d| db::last_backup(&d.join("backups"))).map(|(_, m)| m);
            (s, l, b, db::meta_get(&c, "embed_model")?, db::dim_mismatch(&c)?, db::log_first_ts(&c)?, backup_at)
        };
        let reembed_needed = mismatch || stored.as_deref().is_some_and(|m| m != a.models.embed_model);
        Ok(Json(json!({
            "version": env!("CARGO_PKG_VERSION"),
            // Fingerprint of the embedded UI; open pages reload themselves when it changes.
            "ui": env!("MIMIR_ASSET_HASH"),
            "min": (DEFAULT_MIN as f64 * 1000.0).round() / 1000.0,
            "candidates": CANDIDATES,
            "embed_model": a.models.embed_model,
            "rerank_model": a.models.rerank_model,
            "models_ok": models_ok(&a.models),
            "skills": skills,
            "log_rows": log_rows,
            "db_bytes": db_bytes,
            "embed_model_stored": stored,
            "reembed_needed": reembed_needed,
            "backup_at": backup_at,
            "log_days": first.map_or(0, |f| (t - f).max(0) / DAY),
        })))
    })
    .await
}

/// Stale: no match in 30 days, but only judged once the log spans at least 7 days.
fn is_stale(log_spans_week: bool, matches_30d: i64) -> bool {
    log_spans_week && matches_30d == 0
}

type ReportKey = (String, i64, i64);

/// Spec reports memoized per (skill, version, updated_at), process-wide.
fn cached_report(a: &App, name: &str, version: i64, updated_at: i64) -> spec::Report {
    static CACHE: OnceLock<Mutex<HashMap<ReportKey, spec::Report>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    let key = (name.to_string(), version, updated_at);
    if let Some(r) = cache.lock().unwrap_or_else(|e| e.into_inner()).get(&key) {
        return r.clone();
    }
    let report = match db::spec_input(&a.conn(), name) {
        Ok(Some((body, paths))) => spec::validate(name, &body, &paths.iter().map(String::as_str).collect::<Vec<_>>()),
        _ => return spec::Report::default(),
    };
    let mut g = cache.lock().unwrap_or_else(|e| e.into_inner());
    g.retain(|k, _| k.0 != name);
    g.insert(key, report.clone());
    report
}

async fn skills(State(app): S, h: HeaderMap) -> Result<Json<Vec<SkillRow>>, E> {
    auth(&app, &h)?;
    blocking(app, |a| {
        let t = now();
        let (bases, m, l7, first) = {
            let c = a.conn();
            (db::bases(&c)?, db::match_counts(&c, t)?, db::load_counts(&c, t - 7 * DAY)?, db::log_first_ts(&c)?)
        };
        let spans_week = first.is_some_and(|f| t - f >= 7 * DAY);
        let out = bases
            .into_iter()
            .map(|b| {
                let (last, m7, m30) = m.get(b.name.as_str()).copied().unwrap_or_default();
                let report = cached_report(a, &b.name, b.version, b.updated_at);
                let compliant = report.ok();
                let mut warnings: Vec<String> = report.errors.into_iter().map(|e| format!("spec: {e}")).collect();
                warnings.extend(report.warnings);
                SkillRow {
                    loads_7d: l7.get(&b.name).copied().unwrap_or(0),
                    stale: is_stale(spans_week, m30),
                    compliant,
                    name: b.name,
                    description: b.description,
                    version: b.version,
                    updated_at: b.updated_at,
                    files: b.files,
                    bytes: b.bytes,
                    secrets: b.secrets,
                    last_matched: (m30 > 0).then_some(last),
                    matches_7d: m7,
                    matches_30d: m30,
                    warnings,
                }
            })
            .collect();
        Ok(out)
    })
    .await
    .map(Json)
}

fn unsafe_url(u: &str) -> bool {
    let u = u.trim_start().to_ascii_lowercase();
    ["javascript:", "data:", "vbscript:"].iter().any(|p| u.starts_with(p))
}

/// SKILL.md body (frontmatter stripped) as HTML. Raw HTML in the source is dropped and
/// script-ish link targets are neutralised, so the result is safe to inject.
pub fn render_md(md: &str) -> String {
    let events = Parser::new_ext(spec::body(md), Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS)
        .filter(|e| !matches!(e, Event::Html(_) | Event::InlineHtml(_)))
        .map(|e| match e {
            Event::Start(Tag::Link { link_type, dest_url, title, id }) if unsafe_url(&dest_url) => {
                Event::Start(Tag::Link { link_type, dest_url: CowStr::from("#"), title, id })
            }
            Event::Start(Tag::Image { link_type, dest_url, title, id }) if unsafe_url(&dest_url) => {
                Event::Start(Tag::Image { link_type, dest_url: CowStr::from("#"), title, id })
            }
            e => e,
        });
    let mut out = String::new();
    push_html(&mut out, events);
    out
}

async fn skill(State(app): S, Path(name): Path<String>, h: HeaderMap) -> Result<Json<SkillDetail>, E> {
    auth(&app, &h)?;
    blocking(app, move |a| {
        let (row, files, secret_keys, mut recent, others, co) = {
            let c = a.conn();
            let Some(row) = db::detail_row(&c, &name)? else {
                return Err(E(StatusCode::NOT_FOUND, format!("no skill named {name}")));
            };
            let mut recent = db::log_page(&c, Some(&name), false, None, None, 20)?;
            recent.truncate(20);
            (row, db::file_infos(&c, &name)?, db::secret_keys(&c, &name)?, recent, db::vectors(&c)?, db::hits_with(&c, &name)?)
        };
        let t = now();
        let (mt, lt) = db::skill_activity(&a.conn(), &name, t - 30 * DAY)?;
        let today = t.div_euclid(DAY);
        let mut per_day: Vec<DayCount> = (0..30).rev().map(|i| DayCount { day: day_string((today - i) * DAY), matches: 0, loads: 0 }).collect();
        let idx = |ts: i64| usize::try_from(29 - (today - ts.div_euclid(DAY))).ok().filter(|i| *i < 30);
        for ts in mt {
            if let Some(i) = idx(ts) {
                per_day[i].matches += 1;
            }
        }
        for ts in lt {
            if let Some(i) = idx(ts) {
                per_day[i].loads += 1;
            }
        }
        let (description, body, version, updated_at, emb) = row;
        let mut co_hits: HashMap<String, i64> = HashMap::new();
        for hits in &co {
            for h in hits.iter().filter(|h| h.name != name) {
                *co_hits.entry(h.name.clone()).or_default() += 1;
            }
        }
        let mut similar: Vec<Similar> = match emb {
            Some(e) => others
                .into_iter()
                .filter(|(n, _, _)| *n != name)
                .map(|(n, _, v)| Similar { cosine: dot(&e, &v), co_hits: co_hits.get(&n).copied().unwrap_or(0), name: n })
                .collect(),
            None => vec![],
        };
        similar.sort_by(|x, y| y.cosine.total_cmp(&x.cosine));
        similar.truncate(6);
        recent.truncate(20);
        let paths: Vec<&str> = files.iter().map(|f| f.path.as_str()).collect();
        let report = spec::validate(&name, &body, &paths);
        Ok(SkillDetail {
            html: render_md(&body),
            frontmatter: spec::parse(&body).0,
            spec: report,
            name,
            description,
            version,
            updated_at,
            body,
            files,
            secret_keys,
            recent,
            similar,
            per_day,
        })
    })
    .await
    .map(Json)
}

async fn history(State(app): S, Path(name): Path<String>, h: HeaderMap) -> Result<Json<Value>, E> {
    auth(&app, &h)?;
    blocking(app, move |a| {
        let c = a.conn();
        let Some((description, body, version, updated_at, _)) = db::detail_row(&c, &name)? else {
            return Err(E(StatusCode::NOT_FOUND, format!("no skill named {name}")));
        };
        let mut out = vec![json!({ "version": version, "updated_at": updated_at, "description": description, "bytes": body.len(), "current": true })];
        for (v, ts, d, b) in db::history(&c, &name)? {
            out.push(json!({ "version": v, "updated_at": ts, "description": d, "bytes": b.len() }));
        }
        Ok(Json(Value::Array(out)))
    })
    .await
}

async fn history_version(State(app): S, Path((name, version)): Path<(String, i64)>, h: HeaderMap) -> Result<Json<Value>, E> {
    auth(&app, &h)?;
    blocking(app, move |a| {
        let c = a.conn();
        let row = match db::detail_row(&c, &name)? {
            Some((d, b, v, ts, _)) if v == version => Some((v, ts, d, b)),
            _ => db::history_version(&c, &name, version)?,
        };
        let (version, updated_at, description, body) = row.ok_or(E(StatusCode::NOT_FOUND, format!("no version {version} of {name}")))?;
        Ok(Json(json!({ "version": version, "updated_at": updated_at, "description": description, "html": render_md(&body), "body": body })))
    })
    .await
}

async fn file(State(app): S, Path((name, path)): Path<(String, String)>, h: HeaderMap) -> Result<Response, E> {
    auth(&app, &h)?;
    let bytes = blocking(app, move |a| {
        db::file_content(&a.conn(), &name, &path)?.ok_or(E(StatusCode::NOT_FOUND, "no such file".into()))
    })
    .await?;
    let ct = if std::str::from_utf8(&bytes).is_ok() { "text/plain; charset=utf-8" } else { "application/octet-stream" };
    Ok(([(header::CONTENT_TYPE, ct), (header::X_CONTENT_TYPE_OPTIONS, "nosniff")], bytes).into_response())
}

#[derive(Deserialize)]
struct StatsQ {
    since: Option<i64>,
}

/// Nearest-rank percentile of a sorted slice.
fn pct(sorted: &[i64], p: f64) -> i64 {
    if sorted.is_empty() {
        return 0;
    }
    sorted[((sorted.len() - 1) as f64 * p).round() as usize]
}

use db::day_string;

async fn stats(State(app): S, Query(q): Query<StatsQ>, h: HeaderMap) -> Result<Json<Value>, E> {
    auth(&app, &h)?;
    blocking(app, move |a| {
        let t = now();
        let since = q.since.unwrap_or(t - DAY);
        let from = since.min(t - 30 * DAY);
        let (mut rows, loads, names) = {
            let c = a.conn();
            (db::log_since(&c, from)?, db::loads_since(&c, from)?, db::bases(&c)?)
        };
        db::fill_loaded(&mut rows, &loads);

        // MCP searches (min 0, k 5) nearly always "hit"; they are counted apart and kept out of the
        // hook's rates, top matches, never-matched and score bands.
        let mut mcp_top: HashMap<String, i64> = HashMap::new();
        let mut mcp_searches = 0;
        for r in rows.iter().filter(|r| r.ts >= since && db::is_mcp(r.client.as_deref())) {
            mcp_searches += 1;
            for h in &r.hits {
                *mcp_top.entry(h.name.clone()).or_default() += 1;
            }
        }
        let mut mcp_per_day: HashMap<String, i64> = HashMap::new();
        rows.retain(|r| {
            let m = db::is_mcp(r.client.as_deref());
            if m {
                *mcp_per_day.entry(day_string(r.ts)).or_default() += 1;
            }
            !m
        });

        let win: Vec<_> = rows.iter().filter(|r| r.ts >= since).collect();
        let with_hit = win.iter().filter(|r| !r.hits.is_empty()).count();
        let mut ms: Vec<i64> = win.iter().map(|r| r.ms).collect();
        ms.sort_unstable();

        // last 14 days, oldest first, zero-filled
        let today = t.div_euclid(DAY);
        let mut per_day: Vec<(String, i64, i64)> = (0..14).rev().map(|i| (day_string((today - i) * DAY), 0, 0)).collect();
        for r in rows.iter().filter(|r| r.ts.div_euclid(DAY) > today - 14) {
            let d = day_string(r.ts);
            if let Some(e) = per_day.iter_mut().find(|e| e.0 == d) {
                e.1 += 1;
                e.2 += !r.hits.is_empty() as i64;
            }
        }

        let mut top: HashMap<&str, (i64, i64)> = HashMap::new();
        for r in &win {
            for h in &r.hits {
                top.entry(h.name.as_str()).or_default().0 += 1;
            }
        }
        for (_, s) in loads.iter().filter(|(ts, _)| *ts >= since) {
            top.entry(s.as_str()).or_default().1 += 1;
        }
        let mut top: Vec<_> = top.into_iter().filter(|(_, v)| v.0 > 0).collect();
        top.sort_by(|x, y| y.1 .0.cmp(&x.1 .0).then(y.1 .1.cmp(&x.1 .1)).then(x.0.cmp(y.0)));
        top.truncate(10);

        let cutoff = t - 30 * DAY;
        let recent30: Vec<_> = rows.iter().filter(|r| r.ts >= cutoff).collect();
        let matched: std::collections::HashSet<&str> = recent30.iter().flat_map(|r| r.hits.iter().map(|h| h.name.as_str())).collect();
        let never: Vec<&str> = names.iter().map(|b| b.name.as_str()).filter(|n| !matched.contains(n)).collect();

        let bands = [(0.05f32, 0.2f32), (0.2, 0.5), (0.5, 0.8), (0.8, 1.0)];
        let mut sb = [(0i64, 0i64); 4];
        for r in &recent30 {
            for h in &r.hits {
                let Some(i) = bands.iter().position(|(lo, hi)| h.score >= *lo && (h.score < *hi || *hi >= 1.0)) else { continue };
                sb[i].0 += 1;
                sb[i].1 += r.loaded.contains(&h.name) as i64;
            }
        }

        Ok(Json(json!({
            "since": since,
            "matches": win.len(),
            "mcp_searches": mcp_searches,
            "with_hit": with_hit,
            "hit_rate": if win.is_empty() { 0.0 } else { with_hit as f64 / win.len() as f64 },
            "p50_ms": pct(&ms, 0.5),
            "p95_ms": pct(&ms, 0.95),
            "per_day": per_day.iter().map(|(d, m, w)| json!({ "day": d, "matches": m, "with_hit": w, "mcp": mcp_per_day.get(d).copied().unwrap_or(0) })).collect::<Vec<_>>(),
            "top": top.iter().map(|(n, v)| json!({ "name": n, "matches": v.0, "loads": v.1, "mcp": mcp_top.get(*n).copied().unwrap_or(0) })).collect::<Vec<_>>(),
            "never_matched": never,
            "score_bands": bands.iter().zip(sb).map(|((lo, hi), (s, l))| json!({ "lo": (*lo as f64 * 100.0).round() / 100.0, "hi": (*hi as f64 * 100.0).round() / 100.0, "suggested": s, "loaded": l })).collect::<Vec<_>>(),
        })))
    })
    .await
}

#[derive(Deserialize)]
struct LogQ {
    skill: Option<String>,
    nohit: Option<String>,
    q: Option<String>,
    before: Option<i64>,
    limit: Option<usize>,
}

async fn log(State(app): S, Query(q): Query<LogQ>, h: HeaderMap) -> Result<Json<Value>, E> {
    auth(&app, &h)?;
    blocking(app, move |a| {
        let limit = q.limit.unwrap_or(50).clamp(1, 200);
        let nz = |s: &Option<String>| s.as_deref().filter(|s| !s.is_empty()).map(str::to_string);
        let nohit = matches!(q.nohit.as_deref(), Some("1" | "true"));
        let mut rows = db::log_page(&a.conn(), nz(&q.skill).as_deref(), nohit, nz(&q.q).as_deref(), q.before, limit)?;
        let next = if rows.len() > limit {
            rows.truncate(limit);
            rows.last().map(|r| r.id)
        } else {
            None
        };
        Ok(Json(json!({ "rows": rows, "next": next })))
    })
    .await
}

#[derive(Deserialize)]
struct ClearQ {
    before: i64,
}

async fn clear_log(State(app): S, Query(q): Query<ClearQ>, h: HeaderMap) -> Result<Json<Value>, E> {
    auth(&app, &h)?;
    // Guards cookie-authenticated callers against cross-site requests.
    if h.get("x-mimir").and_then(|v| v.to_str().ok()) != Some("1") {
        return Err(E(StatusCode::BAD_REQUEST, "missing X-Mimir: 1 header".into()));
    }
    blocking(app, move |a| {
        let deleted = db::clear_log(&a.conn(), q.before)?;
        a.emit(json!({ "type": "log_cleared" }));
        Ok(Json(json!({ "deleted": deleted })))
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn days_and_markdown() {
        assert_eq!(day_string(0), "1970-01-01");
        assert_eq!(day_string(1_709_164_800), "2024-02-29");
        let h = render_md("---\nname: x\n---\n# Hi\n<script>alert(1)</script>\n[a](javascript:alert(1))");
        assert!(h.contains("<h1>Hi</h1>") && !h.contains("<script") && !h.contains("javascript:") && !h.contains("name: x"));
        assert_eq!(pct(&[1, 2, 3, 4, 5], 0.5), 3);
    }

    #[test]
    fn stale_needs_a_week_of_log() {
        assert!(is_stale(true, 0));
        assert!(!is_stale(true, 1));
        assert!(!is_stale(false, 0));
    }
}
