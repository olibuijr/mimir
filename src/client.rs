//! Client side: talks to `mimir serve`, imports skill dirs, materializes skills, runs the hook and MCP.
use crate::spec;
use crate::types::{File, Hit, MatchReq, MatchResp, Skill, Summary};
use anyhow::{anyhow, bail, Context, Result};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::io::{BufRead, Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub struct Client {
    pub url: String,
    token: String,
    agent: ureq::Agent,
}

/// The server could not be reached (connect failure, timeout, broken connection).
#[derive(Debug)]
pub struct Unreachable(pub String);
impl std::fmt::Display for Unreachable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for Unreachable {}

pub fn is_unreachable(e: &anyhow::Error) -> bool {
    e.downcast_ref::<Unreachable>().is_some()
}

fn home() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/tmp".into()))
}

/// ~/.cache/mimir: hook state (down marker, sessions, stamps). Skills live in `cache_dir()`.
fn cache_root() -> PathBuf {
    home().join(".cache/mimir")
}

fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// Reads MIMIR_* settings from ~/.config/mimir/env (KEY=VALUE lines) unless already set in the environment.
pub fn load_env() {
    let Ok(text) = std::fs::read_to_string(home().join(".config/mimir/env")) else { return };
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            let (k, v) = (k.trim(), v.trim().trim_matches('"').trim_matches('\''));
            if std::env::var_os(k).is_none() {
                std::env::set_var(k, v);
            }
        }
    }
}

/// "claude" when running under Claude Code, "codex" under Codex, else "cli".
fn kind_from(keys: &[String]) -> &'static str {
    if keys.iter().any(|k| k == "CLAUDE_PROJECT_DIR" || k == "CLAUDECODE") {
        "claude"
    } else if keys.iter().any(|k| k.starts_with("CODEX_")) {
        "codex"
    } else {
        "cli"
    }
}

fn short_host() -> String {
    let raw = std::fs::read_to_string("/proc/sys/kernel/hostname")
        .or_else(|_| std::fs::read_to_string("/etc/hostname"))
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_default();
    let h = raw.trim().split('.').next().unwrap_or("").to_lowercase();
    if h.is_empty() { "unknown".into() } else { h }
}

/// Value of the X-Mimir-Client header, e.g. "claude@macbook".
pub fn client_id() -> String {
    let keys: Vec<String> = std::env::vars_os().map(|(k, _)| k.to_string_lossy().into_owned()).collect();
    let id = format!("{}@{}", kind_from(&keys), short_host());
    id.chars().take(64).collect()
}

/// Numeric (major, minor, patch) of a version string; suffixes like -rc1 are ignored.
fn parse_ver(v: &str) -> (u64, u64, u64) {
    let mut it = v.trim().trim_start_matches('v').split('.').map(|p| p.chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse::<u64>().unwrap_or(0));
    (it.next().unwrap_or(0), it.next().unwrap_or(0), it.next().unwrap_or(0))
}

/// `a` is a newer version than `b`.
fn newer(a: &str, b: &str) -> bool {
    parse_ver(a) > parse_ver(b)
}

impl Client {
    pub fn from_env() -> Result<Self> {
        Self::with_connect(Duration::from_secs(3))
    }

    pub fn with_connect(connect: Duration) -> Result<Self> {
        Ok(Client {
            url: std::env::var("MIMIR_URL").unwrap_or_else(|_| "http://192.168.1.12:7440".into()).trim_end_matches('/').into(),
            token: std::env::var("MIMIR_TOKEN").context("MIMIR_TOKEN not set (put it in ~/.config/mimir/env)")?,
            agent: ureq::AgentBuilder::new().timeout_connect(connect).build(),
        })
    }

    fn req(&self, method: &str, path: &str, timeout: Duration) -> ureq::Request {
        self.agent
            .request(method, &format!("{}{path}", self.url))
            .set("Authorization", &format!("Bearer {}", self.token))
            .set("X-Mimir-Client", &client_id())
            .timeout(timeout)
    }

    fn finish(what: &str, res: std::result::Result<ureq::Response, ureq::Error>) -> Result<ureq::Response> {
        match res {
            Ok(r) => Ok(r),
            Err(ureq::Error::Status(code, r)) => {
                let v: Value = r.into_json().unwrap_or(Value::Null);
                bail!("{what}: {code} {}", v["error"].as_str().unwrap_or(""))
            }
            Err(e) => Err(anyhow::Error::new(Unreachable(format!("{what}: {e}")))),
        }
    }

    /// Like `send`, also returning the server's X-Mimir-Version header.
    pub fn send_ver<T: DeserializeOwned>(&self, method: &str, path: &str, body: Option<Value>, timeout: Duration) -> Result<(T, Option<String>)> {
        let r = self.req(method, path, timeout);
        let res = match body {
            Some(b) => r.send_json(b),
            None => r.call(),
        };
        let r = Self::finish(&format!("{method} {path}"), res)?;
        let ver = r.header("X-Mimir-Version").map(String::from);
        Ok((r.into_json()?, ver))
    }

    pub fn send<T: DeserializeOwned>(&self, method: &str, path: &str, body: Option<Value>, timeout: Duration) -> Result<T> {
        Ok(self.send_ver(method, path, body, timeout)?.0)
    }

    pub fn list(&self) -> Result<Vec<Summary>> {
        self.send("GET", "/skills", None, Duration::from_secs(10))
    }
    /// Fetch a skill without counting it as used.
    pub fn get(&self, name: &str) -> Result<Skill> {
        self.send("GET", &skill_path(name)?, None, Duration::from_secs(30))
    }

    /// Fetch a skill to use it: the server records a load (suggested vs actually loaded).
    pub fn load(&self, name: &str) -> Result<Skill> {
        self.load_within(name, Duration::from_secs(30))
    }

    pub fn load_within(&self, name: &str, timeout: Duration) -> Result<Skill> {
        let path = skill_path(name)?;
        let r = Self::finish(&format!("GET {path}"), self.req("GET", &path, timeout).set("X-Mimir-Load", "1").call())?;
        Ok(r.into_json()?)
    }
    pub fn put(&self, s: &Skill) -> Result<Summary> {
        self.send("PUT", &skill_path(&s.name)?, Some(serde_json::to_value(s)?), Duration::from_secs(60))
    }
    pub fn delete(&self, name: &str) -> Result<Value> {
        self.send("DELETE", &skill_path(name)?, None, Duration::from_secs(10))
    }
    pub fn reembed(&self) -> Result<Value> {
        self.send("POST", "/reembed", Some(json!({})), Duration::from_secs(300))
    }
    pub fn matches(&self, req: &MatchReq, timeout: Duration) -> Result<MatchResp> {
        Ok(self.matches_ver(req, timeout)?.0)
    }
    pub fn matches_ver(&self, req: &MatchReq, timeout: Duration) -> Result<(MatchResp, Option<String>)> {
        self.send_ver("POST", "/match", Some(serde_json::to_value(req)?), timeout)
    }
}

/// Refuse names that are not spec skill names: they go into URLs and cache paths unescaped.
fn check_name(name: &str) -> Result<()> {
    anyhow::ensure!(
        spec::valid_name(name),
        "invalid skill name {name:?}: skill names are 1-64 lowercase letters, digits and single hyphens, not starting or ending with a hyphen"
    );
    Ok(())
}

/// `/skills/<name>` for a checked name.
fn skill_path(name: &str) -> Result<String> {
    check_name(name)?;
    Ok(format!("/skills/{name}"))
}

/// Files of a skill as returned by the server (always present on GET).
pub fn files_of(s: &Skill) -> &[File] {
    s.files.as_deref().unwrap_or(&[])
}

// ---------- import ----------

const SKIP_DIRS: &[&str] = &[".git", "node_modules", "__pycache__", "target", ".venv", "venv"];
const MAX_FILE: u64 = 8 << 20;

/// Read a skill directory as the Agent Skills spec defines it: the directory is named after the
/// skill and holds SKILL.md plus any files. `.env` becomes the skill's secrets.
pub fn read_dir(dir: &Path) -> Result<Skill> {
    let dir = &dir.canonicalize().with_context(|| dir.display().to_string())?;
    let body = std::fs::read_to_string(dir.join("SKILL.md")).with_context(|| format!("{}/SKILL.md", dir.display()))?;
    let name = dir.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_string();
    let description = spec::parse(&body).0.description.unwrap_or_default().trim().to_string();
    let mut files = vec![];
    let mut secrets = None;
    walk(dir, dir, &mut files, &mut secrets)?;
    Ok(Skill { name, description, body, version: 0, files: Some(files), secrets })
}

/// Spec report for a skill read from disk or the server.
pub fn check(s: &Skill) -> spec::Report {
    spec::validate(&s.name, &s.body, &files_of(s).iter().map(|f| f.path.as_str()).collect::<Vec<_>>())
}

/// `mimir validate`: local directories, or every skill on the server. Returns the number of failing skills.
pub fn validate(dirs: &[std::path::PathBuf], as_json: bool) -> Result<usize> {
    let mut reports: Vec<(String, spec::Report)> = vec![];
    if dirs.is_empty() {
        let c = Client::from_env()?;
        for s in c.list()? {
            let v: Value = c.send("GET", &format!("/api/skills/{}", s.name), None, Duration::from_secs(20))?;
            reports.push((s.name, serde_json::from_value(v["spec"].clone()).unwrap_or_default()));
        }
    } else {
        for d in dirs {
            match read_dir(d) {
                Ok(s) => reports.push((s.name.clone(), check(&s))),
                Err(e) => reports.push((d.display().to_string(), spec::Report { errors: vec![format!("{e:#}")], warnings: vec![] })),
            }
        }
    }
    if as_json {
        let failed = reports.iter().filter(|(_, r)| !r.ok()).count();
        let skills: Vec<Value> = reports.iter().map(|(n, r)| json!({ "name": n, "ok": r.ok(), "errors": r.errors, "warnings": r.warnings })).collect();
        println!("{}", json!({ "skills": skills, "valid": reports.len() - failed, "invalid": failed }));
        return Ok(failed);
    }
    let mut failed = 0;
    for (n, r) in &reports {
        if r.errors.is_empty() && r.warnings.is_empty() {
            continue;
        }
        println!("{n}");
        for e in &r.errors {
            println!("  error: {e}");
        }
        for w in &r.warnings {
            println!("  warn:  {w}");
        }
        failed += !r.ok() as usize;
    }
    let warned = reports.iter().filter(|(_, r)| r.ok() && !r.warnings.is_empty()).count();
    println!("{} skills: {} valid ({} with warnings), {failed} invalid", reports.len(), reports.len() - failed, warned);
    Ok(failed)
}

fn walk(root: &Path, dir: &Path, files: &mut Vec<File>, secrets: &mut Option<BTreeMap<String, String>>) -> Result<()> {
    let mut entries: Vec<_> = std::fs::read_dir(dir)?.collect::<std::io::Result<_>>()?;
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let p = e.path();
        let rel = p.strip_prefix(root)?.to_string_lossy().to_string();
        let meta = std::fs::metadata(&p)?; // follows symlinks
        if meta.is_dir() {
            if !SKIP_DIRS.contains(&e.file_name().to_string_lossy().as_ref()) {
                walk(root, &p, files, secrets)?;
            }
        } else if rel == "SKILL.md" {
        } else if rel == ".env" {
            *secrets = Some(parse_env(&std::fs::read_to_string(&p)?));
        } else if meta.len() > MAX_FILE {
            eprintln!("  skip {rel}: {} MB is too big", meta.len() >> 20);
        } else {
            files.push(File { path: rel, mode: meta.permissions().mode() & 0o777, b64: B64.encode(std::fs::read(&p)?) });
        }
    }
    Ok(())
}

fn parse_env(text: &str) -> BTreeMap<String, String> {
    text.lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .filter_map(|l| l.split_once('='))
        .map(|(k, v)| {
            let v = v.trim();
            let v = if v.len() >= 2 && ((v.starts_with('\'') && v.ends_with('\'')) || (v.starts_with('"') && v.ends_with('"'))) {
                &v[1..v.len() - 1]
            } else {
                v
            };
            (k.trim().trim_start_matches("export ").to_string(), v.to_string())
        })
        .collect()
}

fn shell_quote(v: &str) -> String {
    format!("'{}'", v.replace('\'', "'\\''"))
}

// ---------- materialize ----------

pub fn cache_dir() -> PathBuf {
    std::env::var("MIMIR_CACHE").map(PathBuf::from).unwrap_or_else(|_| cache_root().join("skills"))
}

/// A relative path that stays inside the skill directory.
fn safe_rel(p: &str) -> bool {
    !p.is_empty() && !p.starts_with('/') && !p.split('/').any(|c| c.is_empty() || c == "..")
}

fn read_manifest(dir: &Path) -> Vec<String> {
    std::fs::read_to_string(dir.join(".mimir-files")).map(|t| t.lines().map(String::from).filter(|l| safe_rel(l)).collect()).unwrap_or_default()
}

/// Write a skill's files and .env to the local cache and return (dir, SKILL.md with paths pointing there).
/// Files are overwritten in place and only files that left the skill are deleted (tracked in
/// `.mimir-files`), so symlinks into the cache never dangle. An unchanged version is not rewritten.
pub fn materialize(s: &Skill) -> Result<(PathBuf, String)> {
    let dir = cache_dir().join(&s.name);
    let fix = |text: &str| rewrite_paths(text, &s.name, &dir);
    let body = fix(&s.body);
    let ver = s.version.to_string();
    let same = std::fs::read_to_string(dir.join(".mimir-version")).is_ok_and(|v| v.trim() == ver);
    if same && dir.join(".mimir-files").exists() && dir.join("SKILL.md").exists() {
        return Ok((dir, body));
    }
    std::fs::create_dir_all(&dir)?;
    let old = read_manifest(&dir);
    let mut written: Vec<String> = vec![];
    let put = |p: &Path, bytes: &[u8], mode: u32| -> Result<()> {
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent)?;
        }
        if p.exists() {
            let _ = std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o600)); // allow overwriting a read-only file
        }
        std::fs::write(p, bytes)?;
        std::fs::set_permissions(p, std::fs::Permissions::from_mode(mode))?;
        Ok(())
    };
    for f in files_of(s) {
        if !safe_rel(&f.path) {
            continue;
        }
        let bytes = B64.decode(&f.b64)?;
        let bytes = match String::from_utf8(bytes) {
            Ok(t) => fix(&t).into_bytes(),
            Err(e) => e.into_bytes(),
        };
        put(&dir.join(&f.path), &bytes, f.mode & 0o777)?;
        written.push(f.path.clone());
    }
    if let Some(sec) = s.secrets.as_ref().filter(|s| !s.is_empty()) {
        let text: String = sec.iter().map(|(k, v)| format!("{k}={}\n", shell_quote(v))).collect();
        put(&dir.join(".env"), text.as_bytes(), 0o600)?;
        written.push(".env".into());
    }
    put(&dir.join("SKILL.md"), body.as_bytes(), 0o644)?;
    written.push("SKILL.md".into());
    for gone in old.iter().filter(|o| !written.contains(o)) {
        let p = dir.join(gone);
        if std::fs::remove_file(&p).is_ok() {
            let mut parent = p.parent().map(Path::to_path_buf);
            while let Some(d) = parent.filter(|d| *d != dir) {
                if std::fs::remove_dir(&d).is_err() {
                    break;
                }
                parent = d.parent().map(Path::to_path_buf);
            }
        }
    }
    std::fs::write(dir.join(".mimir-files"), written.join("\n") + "\n")?;
    std::fs::write(dir.join(".mimir-version"), &ver)?;
    Ok((dir, body))
}

/// Skills were written for ~/.claude/skills/<name>; point those paths at the cache dir.
fn rewrite_paths(text: &str, name: &str, dir: &Path) -> String {
    let d = dir.to_string_lossy();
    let h = home();
    let mut t = text.replace("${CLAUDE_SKILL_DIR}", &d);
    for prefix in [format!("{}/.claude/skills/{name}", h.display()), format!("~/.claude/skills/{name}"), format!("$HOME/.claude/skills/{name}")] {
        t = t.replace(&prefix, &d);
    }
    t
}

pub fn get_text(c: &Client, name: &str) -> Result<String> {
    check_name(name)?;
    let s = match c.load(name) {
        Ok(s) => s,
        Err(e) if is_unreachable(&e) => return cached_text(name).ok_or(e),
        Err(e) => return Err(e),
    };
    let (dir, body) = materialize(&s)?;
    let n = files_of(&s).len();
    let env = if s.secrets.as_ref().is_some_and(|m| !m.is_empty()) { ", plus .env with its secrets (mode 600)" } else { "" };
    Ok(format!(
        "Skill `{}` v{} — its {n} file(s) are in {}{env}. Follow it as you would a loaded skill.\n\n{body}",
        s.name,
        s.version,
        dir.display()
    ))
}

/// The last materialized copy of a skill, for when the server cannot be reached.
fn cached_text(name: &str) -> Option<String> {
    check_name(name).ok()?;
    let dir = cache_dir().join(name);
    let body = std::fs::read_to_string(dir.join("SKILL.md")).ok()?;
    let ver = std::fs::read_to_string(dir.join(".mimir-version")).map(|v| v.trim().to_string()).unwrap_or_else(|_| "?".into());
    let n = read_manifest(&dir).iter().filter(|f| *f != "SKILL.md" && *f != ".env").count();
    Some(format!(
        "Mímir server unreachable: using the CACHED copy of skill `{name}` v{ver} (may be out of date) — its {n} file(s) are in {}. Follow it as you would a loaded skill.\n\n{body}",
        dir.display()
    ))
}

// ---------- export ----------

/// `mimir export`: write skills as verbatim Agent Skills directories under `out`. Returns how many.
pub fn export(c: &Client, out: &Path, names: &[String], with_secrets: bool) -> Result<usize> {
    let names: Vec<String> = if names.is_empty() { c.list()?.into_iter().map(|s| s.name).collect() } else { names.to_vec() };
    for name in &names {
        anyhow::ensure!(spec::valid_name(name), "invalid skill name {name:?}");
        let s = c.get(name)?;
        let dir = out.join(name);
        std::fs::create_dir_all(&dir)?;
        let write = |p: &Path, bytes: &[u8], mode: u32| -> Result<()> {
            if let Some(parent) = p.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(p, bytes)?;
            std::fs::set_permissions(p, std::fs::Permissions::from_mode(mode))?;
            Ok(())
        };
        write(&dir.join("SKILL.md"), s.body.as_bytes(), 0o644)?;
        for f in files_of(&s).iter().filter(|f| safe_rel(&f.path)) {
            write(&dir.join(&f.path), &B64.decode(&f.b64)?, f.mode & 0o777)?;
        }
        if with_secrets {
            if let Some(sec) = s.secrets.as_ref().filter(|m| !m.is_empty()) {
                let text: String = sec.iter().map(|(k, v)| format!("{k}={}\n", shell_quote(v))).collect();
                write(&dir.join(".env"), text.as_bytes(), 0o600)?;
            }
        }
    }
    println!("exported {} skill(s) to {}", names.len(), out.display());
    Ok(names.len())
}

// ---------- update ----------

/// `mimir update`: replace this binary with the one the server runs, when that is newer.
pub fn update(c: &Client, force: bool) -> Result<()> {
    let me = env!("CARGO_PKG_VERSION");
    let resp = Client::finish("GET /api/dist/mimir", c.req("GET", "/api/dist/mimir", Duration::from_secs(300)).call())?;
    let ver = resp.header("X-Mimir-Version").unwrap_or("?").to_string();
    if !force && !newer(&ver, me) {
        println!("mimir {me} is up to date (server runs {ver})");
        return Ok(());
    }
    let exe = std::env::current_exe()?;
    let tmp = exe.with_file_name(".mimir-update.tmp");
    let res = (|| -> Result<()> {
        let mut f = std::fs::File::create(&tmp)?;
        let n = std::io::copy(&mut resp.into_reader(), &mut f)?;
        anyhow::ensure!(n > 1 << 20, "downloaded only {n} bytes, not a mimir binary");
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755))?;
        std::fs::rename(&tmp, &exe)?;
        Ok(())
    })();
    if res.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    res?;
    println!("updated {}: {me} → {ver}", exe.display());
    Ok(())
}

// ---------- doctor ----------

struct Out {
    ok: bool,
    /// With --json the checks are collected and printed once as {ok, checks:[{level, message}]}.
    json: Option<Vec<Value>>,
}
impl Out {
    fn say(&mut self, level: &str, mark: &str, m: &str) {
        match &mut self.json {
            Some(v) => v.push(json!({ "level": level, "message": m })),
            None => println!("{mark} {m}"),
        }
    }
    fn good(&mut self, m: impl AsRef<str>) {
        self.say("ok", "✓", m.as_ref());
    }
    fn bad(&mut self, m: impl AsRef<str>) {
        self.ok = false;
        self.say("fail", "✗", m.as_ref());
    }
    fn warn(&mut self, m: impl AsRef<str>) {
        self.say("warn", "!", m.as_ref());
    }
    fn check(&mut self, good: bool, yes: &str, no: &str) {
        if good { self.good(yes) } else { self.bad(no) }
    }
}

fn read_json(p: &Path) -> Option<Value> {
    serde_json::from_str(&std::fs::read_to_string(p).ok()?).ok()
}

fn sh_word(s: &str) -> String {
    if !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || "._-".contains(c)) { s.into() } else { shell_quote(s) }
}

const ARCHIVE: &str = "~/.local/share/mimir/skills-archive";

/// File-level skills left outside Mímir: (label, dir as shown, names).
fn strays() -> Vec<(&'static str, &'static str, Vec<String>)> {
    let h = home();
    [("claude", "~/.claude/skills", ".claude/skills", Some("synced")), ("codex", "~/.codex/skills", ".codex/skills", Some(".system")), ("agents", "~/.agents/skills", ".agents/skills", None)]
        .into_iter()
        .map(|(label, shown, rel, keep)| {
            let mut names: Vec<String> = std::fs::read_dir(h.join(rel))
                .map(|d| d.filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| Some(n.as_str()) != keep).collect())
                .unwrap_or_default();
            names.sort();
            (label, shown, names)
        })
        .collect()
}

/// `mimir doctor`; returns false when any check failed.
pub fn doctor(as_json: bool) -> bool {
    let mut o = Out { ok: true, json: as_json.then(Vec::new) };
    let me = env!("CARGO_PKG_VERSION");
    match Client::from_env() {
        Err(e) => o.bad(format!("{e:#}")),
        Ok(c) => match c.send_ver::<Value>("GET", "/health", None, Duration::from_secs(5)) {
            Err(e) => o.bad(format!("server unreachable at {}: {e:#}", c.url)),
            Ok((h, ver)) => {
                let sv = ver.or_else(|| h["version"].as_str().map(String::from)).unwrap_or_else(|| "?".into());
                if newer(&sv, me) {
                    o.bad(format!("client {me} is older than server {sv} at {}: run `mimir update`", c.url));
                } else if newer(me, &sv) {
                    o.warn(format!("client {me} is newer than server {sv} at {}", c.url));
                } else {
                    o.good(format!("server {} reachable, version {sv} matches the client", c.url));
                }
                match c.list() {
                    Ok(l) => o.good(format!("token accepted ({} skills)", l.len())),
                    Err(e) => o.bad(format!("token not accepted: {e:#}")),
                }
                match c.send::<Value>("GET", "/api/config", None, Duration::from_secs(10)) {
                    Err(e) => o.bad(format!("/api/config: {e:#}")),
                    Ok(cfg) => {
                        match cfg["models_ok"].as_bool() {
                            Some(b) => o.check(b, "embedding and reranking models reachable", "embedding/reranking models are not reachable from the server"),
                            None => o.warn("server did not report models_ok"),
                        }
                        if let Some(b) = cfg["reembed_needed"].as_bool() {
                            o.check(!b, "embeddings match the configured model", "embeddings are stale for the configured model: run `mimir reembed`");
                        }
                        match cfg.get("backup_at") {
                            Some(Value::Number(n)) => {
                                let age = (now() - n.as_i64().unwrap_or(0)).max(0);
                                let msg = format!("last backup {}", if age < 3600 { format!("{} min ago", age / 60) } else if age < 86400 { format!("{} h ago", age / 3600) } else { format!("{} days ago", age / 86400) });
                                if age > 2 * 86400 { o.warn(msg) } else { o.good(msg) }
                            }
                            Some(_) => o.warn("no database backup yet"),
                            None => {}
                        }
                    }
                }
            }
        },
    }
    let h = home();
    let mentions = |v: Option<Value>| v.is_some_and(|v| v.to_string().contains("mimir"));
    let claude = read_json(&h.join(".claude/settings.json"));
    o.check(mentions(claude.as_ref().map(|v| v["hooks"]["UserPromptSubmit"].clone())), "Claude hook in ~/.claude/settings.json", "Claude hook missing in ~/.claude/settings.json (UserPromptSubmit: mimir hook)");
    o.check(read_json(&h.join(".claude.json")).is_some_and(|v| v["mcpServers"]["mimir"].is_object()), "Claude MCP server registered", "Claude MCP server missing: claude mcp add -s user mimir -- ~/.local/bin/mimir mcp");
    o.check(mentions(read_json(&h.join(".codex/hooks.json"))), "Codex hook in ~/.codex/hooks.json", "Codex hook missing in ~/.codex/hooks.json");
    o.check(std::fs::read_to_string(h.join(".codex/config.toml")).is_ok_and(|t| t.contains("[mcp_servers.mimir]")), "Codex MCP server in ~/.codex/config.toml", "Codex MCP server missing: codex mcp add mimir -- ~/.local/bin/mimir mcp");
    let mut moves = vec![];
    for (label, shown, names) in strays() {
        if names.is_empty() {
            o.good(format!("no stray skills in {shown}"));
        } else {
            o.bad(format!("stray skills in {shown}: {}", names.join(", ")));
            moves.extend(names.iter().map(|n| format!("mv {shown}/{} {ARCHIVE}/{label}-{}", sh_word(n), sh_word(n))));
        }
    }
    if let Some(checks) = o.json.take() {
        let fix = if moves.is_empty() { vec![] } else { std::iter::once(format!("mkdir -p {ARCHIVE}")).chain(moves).collect() };
        println!("{}", json!({ "ok": o.ok, "version": me, "checks": checks, "archive_commands": fix }));
    } else if !moves.is_empty() {
        println!("  archive them with:\n    mkdir -p {ARCHIVE}");
        moves.iter().for_each(|m| println!("    {m}"));
    }
    o.ok
}

// ---------- hook ----------

fn read_ts(p: &Path) -> Option<i64> {
    std::fs::read_to_string(p).ok()?.trim().parse().ok()
}

fn session_file(sid: &str) -> PathBuf {
    let clean: String = sid.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_').take(100).collect();
    let clean = if clean.is_empty() { format!("ppid-{}", std::os::unix::process::parent_id()) } else { clean };
    cache_root().join("sessions").join(clean)
}

fn session_has(f: &Path, name: &str) -> bool {
    std::fs::read_to_string(f).is_ok_and(|t| t.lines().any(|l| l == name))
}

fn session_add(f: &Path, name: &str) {
    let Some(dir) = f.parent() else { return };
    let _ = std::fs::create_dir_all(dir);
    // forget sessions untouched for 2 days
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let old = e.metadata().and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).is_some_and(|d| d > Duration::from_secs(2 * 86400));
            if old {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
    if let Ok(mut h) = std::fs::OpenOptions::new().create(true).append(true).open(f) {
        let _ = writeln!(h, "{name}");
    }
}

const MAX_INJECT: usize = 24_000;

/// SKILL.md without frontmatter, cut to MAX_INJECT chars.
fn cap_body(md: &str, dir: &Path) -> String {
    let body = spec::body(md).trim();
    match body.char_indices().nth(MAX_INJECT) {
        None => body.to_string(),
        Some((i, _)) => format!("{}\n\n[truncated: read the rest from {}/SKILL.md]", &body[..i], dir.display()),
    }
}

/// The hook's additionalContext: an auto-loaded skill, the remaining suggestions, an optional notice.
fn build_context(loaded: Option<(&str, &Path, &str)>, rest: &[Hit], notice: Option<&str>) -> String {
    let mut parts: Vec<String> = vec![];
    if let Some((name, dir, md)) = loaded {
        parts.push(format!(
            "Mímir loaded skill `{name}` for this prompt (best match). It is already loaded: follow it, no need to call skill_get. Its files are in {}.\n\n{}",
            dir.display(),
            cap_body(md, dir)
        ));
    }
    if !rest.is_empty() {
        let lines: Vec<String> = rest.iter().map(|h| format!("- {} ({:.2}): {}", h.name, h.score, crate::models::clip(&h.description, 300))).collect();
        parts.push(format!(
            "Mímir matched {} skills from the skill server for this prompt. Before acting, load each that applies with the mimir MCP tool `skill_get` (it also installs the skill's scripts locally); ignore any that do not fit.\n{}",
            if loaded.is_some() { "more" } else { "these" },
            lines.join("\n")
        ));
    }
    if let Some(n) = notice {
        parts.push(n.to_string());
    }
    parts.join("\n\n")
}

/// UserPromptSubmit hook: auto-load a confident match, suggest the rest. Never fails or stalls the prompt.
pub fn hook() -> Result<()> {
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;
    let v: Value = serde_json::from_str(&input).unwrap_or(Value::Null);
    let prompt = v["prompt"].as_str().unwrap_or("").trim();
    // Slash commands already pick their skill; tiny prompts ("yes", "go on") carry no topic.
    // Background-task notifications arrive as prompts too; they are not the user asking for anything.
    let system = prompt.starts_with("<task-notification") || prompt.starts_with("<system-reminder") || prompt.contains("[SYSTEM NOTIFICATION");
    if system || prompt.starts_with('/') || prompt.chars().count() < 12 {
        return Ok(());
    }
    let down = cache_root().join("down");
    if read_ts(&down).is_some_and(|t| (0..30).contains(&(now() - t))) {
        return Ok(());
    }
    let Ok(c) = Client::with_connect(Duration::from_millis(300)) else { return Ok(()) };
    let timeout = Duration::from_millis(std::env::var("MIMIR_HOOK_TIMEOUT_MS").ok().and_then(|s| s.parse().ok()).unwrap_or(2500));
    let (r, server_ver) = match c.matches_ver(&MatchReq { prompt: prompt.into(), ..Default::default() }, timeout) {
        Ok(x) => x,
        Err(e) => {
            if is_unreachable(&e) {
                let _ = std::fs::create_dir_all(cache_root());
                let _ = std::fs::write(&down, now().to_string());
            }
            return Ok(());
        }
    };
    let _ = std::fs::remove_file(&down);
    if r.hits.is_empty() {
        return Ok(());
    }
    // Suggestions alone are never acted on, so a confident top hit is loaded right here (once per session).
    let min: f32 = std::env::var("MIMIR_AUTOLOAD_MIN").ok().and_then(|s| s.parse().ok()).unwrap_or(0.9);
    let sf = session_file(v["session_id"].as_str().unwrap_or(""));
    let mut loaded: Option<(String, PathBuf, String)> = None;
    if let Some(top) = r.hits.first().filter(|h| h.score >= min && !session_has(&sf, &h.name)) {
        if let Ok((dir, body)) = c.load_within(&top.name, timeout).and_then(|s| materialize(&s)) {
            session_add(&sf, &top.name);
            loaded = Some((top.name.clone(), dir, body));
        }
    }
    let rest = if loaded.is_some() { &r.hits[1..] } else { &r.hits[..] };
    let mut notice = None;
    if let Some(sv) = server_ver.filter(|sv| newer(sv, env!("CARGO_PKG_VERSION"))) {
        let stamp = cache_root().join("skew-notice");
        if read_ts(&stamp).is_none_or(|t| now() - t > 86400) {
            let _ = std::fs::write(&stamp, now().to_string());
            notice = Some(format!("Mímir client {} is older than the server {sv}; run `mimir update`", env!("CARGO_PKG_VERSION")));
        }
    }
    let ctx = build_context(loaded.as_ref().map(|(n, d, b)| (n.as_str(), d.as_path(), b.as_str())), rest, notice.as_deref());
    println!("{}", json!({ "hookSpecificOutput": { "hookEventName": "UserPromptSubmit", "additionalContext": ctx } }));
    Ok(())
}

// ---------- MCP (stdio) ----------

fn tools() -> Value {
    json!([
        {
            "name": "skill_search",
            "description": "Semantic search over all skills on the Mímir skill server (embeddings + reranker). Use when a task might have a skill with instructions, CLIs or credentials for it.",
            "inputSchema": { "type": "object", "properties": {
                "query": { "type": "string", "description": "What you are trying to do" },
                "k": { "type": "integer", "description": "Max results (default 5)" }
            }, "required": ["query"] }
        },
        {
            "name": "skill_get",
            "description": "Load a skill: returns its SKILL.md instructions and installs its scripts, files and .env secrets into ~/.cache/mimir/skills/<name>/ on this machine (falls back to the cached copy when the server is unreachable). Call before following a skill.",
            "inputSchema": { "type": "object", "properties": { "name": { "type": "string" } }, "required": ["name"] }
        },
        {
            "name": "skill_list",
            "description": "List every skill on the Mímir skill server with its description.",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "skill_save",
            "description": "Create or update a skill on the Mímir server (replaces the old version, bumps its version). body is the full SKILL.md and must follow the Agent Skills spec (agentskills.io/specification): YAML frontmatter with only name (must equal the skill name: 1-64 lowercase letters, digits, single hyphens), description (1-1024 chars; what it does and when to use it, with trigger words, since prompts are matched against it), and optionally license, compatibility (string, max 500), metadata (string to string map) and allowed-tools; keep the body under 500 lines. Put scripts in scripts/, docs in references/, templates and data in assets/, and refer to them by paths relative to the skill root. Violations are refused with the reasons. files maps relative paths to text content and is MERGED into the skill's current files: the given paths are added or overwritten, every other file is kept (omit to keep all files). remove_files lists paths to delete. executable lists file paths to mark 0755. secrets (KEY: value) become the skill's .env; omit to keep the current secrets. Never echo secret values back to the user.",
            "inputSchema": { "type": "object", "properties": {
                "name": { "type": "string" },
                "body": { "type": "string" },
                "files": { "type": "object", "additionalProperties": { "type": "string" }, "description": "Paths to add or overwrite; other files are kept" },
                "remove_files": { "type": "array", "items": { "type": "string" }, "description": "Paths to delete from the skill" },
                "executable": { "type": "array", "items": { "type": "string" } },
                "secrets": { "type": "object", "additionalProperties": { "type": "string" } }
            }, "required": ["name", "body"] }
        }
    ])
}

/// Overlay `given` (path to text) on `current`, drop `remove`, mark `exec` paths 0755.
/// Existing files keep their mode unless listed in `exec`; new ones get 0644.
fn merge_files(current: Vec<File>, given: Option<&serde_json::Map<String, Value>>, exec: &[&str], remove: &[&str]) -> Vec<File> {
    let mut map: BTreeMap<String, File> = current.into_iter().map(|f| (f.path.clone(), f)).collect();
    for r in remove {
        map.remove(*r);
    }
    for (p, v) in given.into_iter().flatten() {
        let mode = map.get(p).map_or(0o644, |f| f.mode);
        map.insert(p.clone(), File { path: p.clone(), mode, b64: B64.encode(v.as_str().unwrap_or("")) });
    }
    for f in map.values_mut().filter(|f| exec.contains(&f.path.as_str())) {
        f.mode = 0o755;
    }
    map.into_values().collect()
}

fn str_list(v: &Value) -> Vec<&str> {
    v.as_array().map(|a| a.iter().filter_map(|x| x.as_str()).collect()).unwrap_or_default()
}

fn call_tool(c: &Client, name: &str, a: &Value) -> Result<String> {
    match name {
        "skill_search" => {
            let q = a["query"].as_str().ok_or_else(|| anyhow!("query is required"))?;
            let k = a["k"].as_u64().unwrap_or(5) as usize;
            let r = c.matches(&MatchReq { prompt: q.into(), k: Some(k), min: Some(0.0), log: Some(true), source: Some("mcp".into()), ..Default::default() }, Duration::from_secs(20))?;
            Ok(fmt_hits(&r.hits))
        }
        "skill_get" => get_text(c, a["name"].as_str().ok_or_else(|| anyhow!("name is required"))?),
        "skill_list" => Ok(c.list()?.iter().map(|s| format!("- {} (v{}): {}", s.name, s.version, s.description)).collect::<Vec<_>>().join("\n")),
        "skill_save" => {
            let name = a["name"].as_str().ok_or_else(|| anyhow!("name is required"))?;
            let body = a["body"].as_str().ok_or_else(|| anyhow!("body is required"))?.to_string();
            let description = spec::parse(&body).0.description.unwrap_or_default();
            // Merge into the stored files; a skill that does not exist yet starts empty.
            let current = match c.get(name) {
                Ok(s) => s.files.unwrap_or_default(),
                Err(e) if e.to_string().contains(": 404") => vec![],
                Err(e) => return Err(e),
            };
            let files = merge_files(current, a["files"].as_object(), &str_list(&a["executable"]), &str_list(&a["remove_files"]));
            let secrets = a["secrets"].as_object().map(|m| m.iter().map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string())).collect());
            let s = c.put(&Skill { name: name.into(), description, body, version: 0, files: Some(files), secrets })?;
            Ok(format!("saved {} v{}", s.name, s.version))
        }
        _ => bail!("unknown tool {name}"),
    }
}

pub fn fmt_hits(hits: &[Hit]) -> String {
    if hits.is_empty() {
        return "no matching skills".into();
    }
    hits.iter().map(|h| format!("- {} ({:.2}, cos {:.2}): {}", h.name, h.score, h.cosine, h.description)).collect::<Vec<_>>().join("\n")
}

pub fn mcp() -> Result<()> {
    let c = Client::from_env()?;
    let stdin = std::io::stdin();
    let mut out = std::io::stdout();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let Ok(msg) = serde_json::from_str::<Value>(&line) else { continue };
        let Some(id) = msg.get("id").cloned() else { continue }; // notification
        let p = &msg["params"];
        let result = match msg["method"].as_str().unwrap_or("") {
            "initialize" => Ok(json!({
                "protocolVersion": p["protocolVersion"].as_str().unwrap_or("2025-06-18"),
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "mimir", "version": env!("CARGO_PKG_VERSION") },
                "instructions": "Mímir is the user's skill server: all their skills (instructions, CLIs, credentials) live there instead of in ~/.claude/skills. Use skill_search when a task may have a skill, skill_get to load one, skill_save to create or update one."
            })),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({ "tools": tools() })),
            "tools/call" => {
                let (text, err) = match call_tool(&c, p["name"].as_str().unwrap_or(""), &p["arguments"]) {
                    Ok(t) => (t, false),
                    Err(e) => (format!("{e:#}"), true),
                };
                Ok(json!({ "content": [{ "type": "text", "text": text }], "isError": err }))
            }
            m => Err(json!({ "code": -32601, "message": format!("method not found: {m}") })),
        };
        let resp = match result {
            Ok(r) => json!({ "jsonrpc": "2.0", "id": id, "result": r }),
            Err(e) => json!({ "jsonrpc": "2.0", "id": id, "error": e }),
        };
        writeln!(out, "{resp}")?;
        out.flush()?;
    }
    Ok(())
}

// ---------- eval ----------

struct Case {
    prompt: String,
    exp: Vec<String>,
    /// "no suggestion" is also a right answer (`-` alone, or `a|-`).
    none_ok: bool,
}

/// cases file: `prompt<TAB>expected` per line, expected = skill name, `a|b` for either, `-` for none, `a|-` for a or none.
fn parse_cases(text: &str) -> Vec<Case> {
    text.lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .filter_map(|l| l.split_once('\t'))
        .map(|(p, e)| {
            let parts: Vec<&str> = e.trim().split('|').map(str::trim).collect();
            Case { prompt: p.to_string(), none_ok: parts.contains(&"-"), exp: parts.into_iter().filter(|x| *x != "-" && !x.is_empty()).map(String::from).collect() }
        })
        .collect()
}

#[derive(Debug, PartialEq, Clone, Copy)]
enum Outcome {
    Right,
    CorrectNone,
    Wrong,
    Missed,
    FalseAlarm,
}

/// Judge the top suggestion (None = nothing suggested) against a case.
fn classify(c: &Case, top: Option<&str>) -> Outcome {
    match top {
        None if c.none_ok => Outcome::CorrectNone,
        None => Outcome::Missed,
        Some(n) if c.exp.iter().any(|e| e == n) => Outcome::Right,
        Some(_) if c.exp.is_empty() => Outcome::FalseAlarm,
        Some(_) => Outcome::Wrong,
    }
}

pub fn eval(c: &Client, path: &Path) -> Result<()> {
    let cases = parse_cases(&std::fs::read_to_string(path)?);
    anyhow::ensure!(!cases.is_empty(), "{}: no cases (one `prompt<TAB>expected` per line)", path.display());
    let lexical = std::env::var("MIMIR_EVAL_LEXICAL").is_ok_and(|v| v == "1").then_some(true);
    for rerank in [false, true] {
        let mut rows = vec![];
        let mut ms = vec![];
        for case in &cases {
            let r = c.matches(&MatchReq { prompt: case.prompt.clone(), k: Some(5), min: Some(-10.0), rerank: Some(rerank), log: Some(false), lexical, ..Default::default() }, Duration::from_secs(30))?;
            ms.push(r.ms);
            rows.push(r.hits);
        }
        ms.sort();
        // Recall only counts cases that need a skill; `a|-` and `-` cases have no wrong ranking to measure.
        let pos: Vec<usize> = (0..cases.len()).filter(|&i| !cases[i].exp.is_empty() && !cases[i].none_ok).collect();
        let at = |n: usize| pos.iter().filter(|&&i| rows[i].iter().take(n).any(|x| cases[i].exp.contains(&x.name))).count() as f32 / pos.len().max(1) as f32;
        println!(
            "\n== {}{} ({} cases, {} need a skill, {} accept none) | recall@1 {:.2}  recall@3 {:.2} | latency p50 {} ms  p95 {} ms",
            if rerank { "embed + rerank" } else { "embed only" },
            if lexical.is_some() { " + lexical" } else { "" },
            cases.len(),
            pos.len(),
            cases.iter().filter(|c| c.none_ok).count(),
            at(1),
            at(3),
            ms[ms.len() / 2],
            ms[(ms.len() * 95 / 100).min(ms.len() - 1)]
        );
        if !rerank {
            continue; // cosine scores have no meaningful threshold
        }
        println!("   min    decide-acc  right-skill  wrong-skill  missed  false-alarm  correct-none");
        for t in [0.01f32, 0.05, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9] {
            let mut n = [0usize; 5];
            for (case, h) in cases.iter().zip(&rows) {
                let top = h.first().filter(|x| x.score >= t).map(|x| x.name.as_str());
                n[classify(case, top) as usize] += 1;
            }
            let [right, none, wrong, missed, fa] = n;
            println!("   {t:<6} {:<11.2} {right:<12} {wrong:<12} {missed:<7} {fa:<12} {none}", (right + none) as f32 / cases.len().max(1) as f32);
        }
        for &i in &pos {
            let case = &cases[i];
            let top = rows[i].first();
            if top.is_some_and(|x| case.exp.contains(&x.name)) {
                continue;
            }
            // where the plain cosine ranking puts the expected skill
            let rank = c
                .matches(&MatchReq { prompt: case.prompt.clone(), k: Some(200), min: Some(-10.0), rerank: Some(false), log: Some(false), ..Default::default() }, Duration::from_secs(30))
                .ok()
                .and_then(|r| r.hits.iter().position(|h| case.exp.contains(&h.name)))
                .map_or(">200".to_string(), |p| format!("#{}", p + 1));
            println!(
                "MISS {:<60} exp={:<22} got={} cosine-rank={rank}",
                crate::models::clip(&case.prompt, 60),
                case.exp.join("|"),
                top.map_or("-".into(), |x| format!("{}={:.2}", x.name, x.score))
            );
        }
        if std::env::var_os("MIMIR_EVAL_VERBOSE").is_some() {
            for (case, h) in cases.iter().zip(&rows) {
                let top: Vec<String> = h.iter().take(3).map(|x| format!("{}={:.2}", x.name, x.score)).collect();
                let mark = match classify(case, h.first().map(|x| x.name.as_str())) {
                    Outcome::Right => "ok ",
                    _ if case.none_ok => " - ",
                    _ => "XX ",
                };
                println!("{mark} {:<60} exp={:<22} {}", crate::models::clip(&case.prompt, 60), case.exp.join("|"), top.join(" "));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(path: &str, mode: u32, text: &str) -> File {
        File { path: path.into(), mode, b64: B64.encode(text) }
    }

    #[test]
    fn versions_compare_numerically() {
        assert!(newer("0.10.0", "0.9.9"));
        assert!(newer("1.0.0", "0.99.99"));
        assert!(!newer("0.4.0", "0.4.0"));
        assert!(!newer("0.3.0", "0.4.0-rc1"));
        assert_eq!(parse_ver("v0.4.1-rc2"), (0, 4, 1));
        assert_eq!(parse_ver("garbage"), (0, 0, 0));
    }

    #[test]
    fn client_kind_from_env_names() {
        let k = |v: &[&str]| kind_from(&v.iter().map(|s| s.to_string()).collect::<Vec<_>>());
        assert_eq!(k(&["PATH", "CLAUDECODE"]), "claude");
        assert_eq!(k(&["CLAUDE_PROJECT_DIR"]), "claude");
        assert_eq!(k(&["CODEX_HOME"]), "codex");
        assert_eq!(k(&["PATH", "HOME"]), "cli");
        assert_eq!(k(&["CODEX_X", "CLAUDECODE"]), "claude");
    }

    #[test]
    fn merge_overlays_removes_and_keeps_modes() {
        let cur = vec![file("scripts/a.sh", 0o755, "a"), file("references/b.md", 0o644, "b"), file("old.txt", 0o644, "o")];
        let given: serde_json::Map<String, Value> = serde_json::from_value(json!({ "references/b.md": "B2", "new.txt": "n", "scripts/c.sh": "c" })).unwrap();
        let out = merge_files(cur, Some(&given), &["scripts/c.sh"], &["old.txt"]);
        let get = |p: &str| out.iter().find(|f| f.path == p).unwrap();
        assert_eq!(out.len(), 4);
        assert!(out.iter().all(|f| f.path != "old.txt"));
        assert_eq!(get("scripts/a.sh").mode, 0o755); // untouched file keeps its mode
        assert_eq!(get("references/b.md").b64, B64.encode("B2"));
        assert_eq!(get("new.txt").mode, 0o644);
        assert_eq!(get("scripts/c.sh").mode, 0o755);
    }

    #[test]
    fn merge_without_input_keeps_everything() {
        let out = merge_files(vec![file("a", 0o600, "x")], None, &[], &[]);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].mode, 0o600);
    }

    #[test]
    fn rel_paths_are_confined() {
        assert!(safe_rel("scripts/a.sh"));
        assert!(!safe_rel("../x"));
        assert!(!safe_rel("/etc/passwd"));
        assert!(!safe_rel("a//b"));
        assert!(!safe_rel(""));
    }

    #[test]
    fn body_is_capped_and_frontmatter_stripped() {
        let md = format!("---\nname: x\ndescription: d\n---\n{}", "é".repeat(MAX_INJECT + 10));
        let out = cap_body(&md, Path::new("/c/x"));
        assert!(!out.starts_with("---"));
        assert!(out.contains("[truncated: read the rest from /c/x/SKILL.md]"));
        assert_eq!(out.chars().filter(|c| *c == 'é').count(), MAX_INJECT);
        let small = cap_body("---\nname: x\ndescription: d\n---\nhello\n", Path::new("/c/x"));
        assert_eq!(small, "hello");
    }

    #[test]
    fn context_lists_loaded_skill_and_suggestions() {
        let hit = |n: &str| Hit { name: n.into(), description: "does things".into(), score: 0.5, cosine: 0.5 };
        let md = "---\nname: unifi\ndescription: d\n---\nUse the router.";
        let ctx = build_context(Some(("unifi", Path::new("/c/unifi"), md)), &[hit("other")], Some("update me"));
        assert!(ctx.contains("loaded skill `unifi`") && ctx.contains("/c/unifi") && ctx.contains("Use the router."));
        assert!(!ctx.contains("name: unifi"));
        assert!(ctx.contains("- other (0.50)") && ctx.ends_with("update me"));
        let only = build_context(None, &[hit("a")], None);
        assert!(only.contains("skill_get") && !only.contains("loaded skill"));
    }

    #[test]
    fn eval_labels() {
        let cs = parse_cases("# c\np1\ta\np2\ta|b\np3\t-\np4\ta|-\n");
        assert_eq!(cs.len(), 4);
        assert!(!cs[0].none_ok && cs[0].exp == ["a"]);
        assert!(cs[1].exp == ["a", "b"]);
        assert!(cs[2].none_ok && cs[2].exp.is_empty());
        assert!(cs[3].none_ok && cs[3].exp == ["a"]);
    }

    #[test]
    fn eval_without_cases_is_an_error() {
        let p = std::env::temp_dir().join(format!("mimir-eval-empty-{}.tsv", std::process::id()));
        std::fs::write(&p, "# only a comment\n").unwrap();
        let c = Client { url: "http://127.0.0.1:1".into(), token: "t".into(), agent: ureq::agent() };
        assert!(eval(&c, &p).unwrap_err().to_string().contains("no cases"));
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn eval_classification() {
        let cs = parse_cases("p\ta\np\t-\np\ta|-\n");
        assert_eq!(classify(&cs[0], Some("a")), Outcome::Right);
        assert_eq!(classify(&cs[0], Some("b")), Outcome::Wrong);
        assert_eq!(classify(&cs[0], None), Outcome::Missed);
        assert_eq!(classify(&cs[1], None), Outcome::CorrectNone);
        assert_eq!(classify(&cs[1], Some("a")), Outcome::FalseAlarm);
        assert_eq!(classify(&cs[2], None), Outcome::CorrectNone);
        assert_eq!(classify(&cs[2], Some("a")), Outcome::Right);
        assert_eq!(classify(&cs[2], Some("z")), Outcome::Wrong);
    }

    #[test]
    fn skill_names_are_checked_before_urls_and_cache_paths() {
        // Port 9 (discard) is never served: any request would fail as unreachable, not as invalid.
        let c = Client { url: "http://127.0.0.1:9".into(), token: "t".into(), agent: ureq::AgentBuilder::new().build() };
        for bad in ["../etc", "a/b", "x?y=1", "Bad", "", "a b"] {
            let e = get_text(&c, bad).unwrap_err();
            assert!(!is_unreachable(&e) && e.to_string().contains("invalid skill name"), "{bad}: {e:#}");
            assert!(c.get(bad).is_err() && c.delete(bad).is_err() && c.load_within(bad, Duration::from_secs(1)).is_err());
            assert!(cached_text(bad).is_none());
        }
        assert_eq!(skill_path("pdf-tools").unwrap(), "/skills/pdf-tools");
        // A valid name does reach the network (and fails as unreachable here).
        assert!(c.get("pdf-tools").is_err_and(|e| is_unreachable(&e)));
    }
}
