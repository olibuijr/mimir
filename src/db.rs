use crate::models::{from_blob, to_blob};
use crate::types::{File, FileInfo, LogHit, LogRow, Skill, Summary};
use anyhow::Result;
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::{BTreeMap, HashMap};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

pub(crate) const SCHEMA: &str = "
PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;
CREATE TABLE IF NOT EXISTS skills (
    name        TEXT PRIMARY KEY,
    description TEXT NOT NULL,
    body        TEXT NOT NULL,
    version     INTEGER NOT NULL DEFAULT 1,
    updated_at  INTEGER NOT NULL,
    embedding   BLOB
);
CREATE TABLE IF NOT EXISTS files (
    skill   TEXT NOT NULL REFERENCES skills(name) ON DELETE CASCADE,
    path    TEXT NOT NULL,
    content BLOB NOT NULL,
    mode    INTEGER NOT NULL,
    PRIMARY KEY (skill, path)
);
CREATE TABLE IF NOT EXISTS secrets (
    skill TEXT NOT NULL REFERENCES skills(name) ON DELETE CASCADE,
    key   TEXT NOT NULL,
    value TEXT NOT NULL,
    PRIMARY KEY (skill, key)
);
CREATE TABLE IF NOT EXISTS match_log (
    ts     INTEGER NOT NULL,
    prompt TEXT NOT NULL,
    hits   TEXT NOT NULL,
    ms     INTEGER NOT NULL,
    client TEXT,
    prompt_len INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS skill_history (
    skill       TEXT NOT NULL,
    version     INTEGER NOT NULL,
    description TEXT NOT NULL,
    body        TEXT NOT NULL,
    updated_at  INTEGER NOT NULL,
    PRIMARY KEY (skill, version)
);
CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS load_log(ts INTEGER NOT NULL, skill TEXT NOT NULL);
CREATE INDEX IF NOT EXISTS match_log_ts ON match_log(ts);
CREATE INDEX IF NOT EXISTS load_log_ts ON load_log(ts);
";

pub fn open(path: &Path) -> Result<Connection> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    // The DB holds secrets: create it 0600 before SQLite does, so it never exists with the default
    // mode (SQLite gives the -wal and -shm files the main file's mode). The chmod covers older files.
    create_private(path, false)?;
    let c = Connection::open(path)?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    init(&c)?;
    Ok(c)
}

/// Create `path` with mode 0600 (`new`: it must not exist yet). An existing file is left as it is.
fn create_private(path: &Path, new: bool) -> Result<()> {
    use std::os::unix::fs::OpenOptionsExt;
    let mut o = std::fs::OpenOptions::new();
    o.write(true).mode(0o600);
    if new {
        o.create_new(true)
    } else {
        o.create(true)
    };
    o.open(path)?;
    Ok(())
}

/// Create missing tables and add columns older databases lack.
pub(crate) fn init(c: &Connection) -> Result<()> {
    c.execute_batch(SCHEMA)?;
    let cols: Vec<String> =
        c.prepare("SELECT name FROM pragma_table_info('match_log')")?.query_map([], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
    if !cols.iter().any(|n| n == "client") {
        c.execute_batch("ALTER TABLE match_log ADD COLUMN client TEXT")?;
    }
    if !cols.iter().any(|n| n == "prompt_len") {
        c.execute_batch("ALTER TABLE match_log ADD COLUMN prompt_len INTEGER NOT NULL DEFAULT 0")?;
    }
    Ok(())
}

pub fn meta_get(c: &Connection, key: &str) -> Result<Option<String>> {
    Ok(c.query_row("SELECT value FROM meta WHERE key = ?1", [key], |r| r.get(0)).optional()?)
}

pub fn meta_set(c: &Connection, key: &str, value: &str) -> Result<()> {
    c.execute("INSERT INTO meta (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value", params![key, value])?;
    Ok(())
}

fn note_embedding(c: &Connection, model: &str, dim: usize) -> Result<()> {
    meta_set(c, "embed_model", model)?;
    meta_set(c, "embed_dim", &dim.to_string())
}

/// UTC "YYYY-MM-DD" of an epoch second.
pub fn day_string(ts: i64) -> String {
    let (y, m, d) = civil(ts);
    format!("{y:04}-{m:02}-{d:02}")
}

fn civil(ts: i64) -> (i64, i64, i64) {
    let z = ts.div_euclid(86400) + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + (m <= 2) as i64, m, d)
}

/// `VACUUM INTO dir/mimir-YYYYMMDD-HHMMSS.db` (dir 0700, file 0600 from the start), keep only the newest `keep` backups.
pub fn backup(c: &Connection, dir: &Path, keep: usize) -> Result<PathBuf> {
    use std::os::unix::fs::DirBuilderExt;
    std::fs::DirBuilder::new().recursive(true).mode(0o700).create(dir)?;
    // Older versions made the directory 0755.
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
    let t = now();
    let (y, mo, d) = civil(t);
    let sod = t.rem_euclid(86400);
    let path = dir.join(format!("mimir-{y:04}{mo:02}{d:02}-{:02}{:02}{:02}.db", sod / 3600, sod % 3600 / 60, sod % 60));
    let _ = std::fs::remove_file(&path);
    // VACUUM INTO accepts an empty file, which then keeps the private mode it was created with.
    create_private(&path, true)?;
    c.execute("VACUUM INTO ?1", [path.to_string_lossy().as_ref()])?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    let mut all = backups(dir);
    all.sort();
    let n = all.len().saturating_sub(keep);
    for old in all.into_iter().take(n) {
        let _ = std::fs::remove_file(old);
    }
    Ok(path)
}

fn backups(dir: &Path) -> Vec<PathBuf> {
    let Ok(rd) = std::fs::read_dir(dir) else { return vec![] };
    rd.filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("mimir-") && n.ends_with(".db")))
        .collect()
}

/// Newest backup and its mtime (epoch seconds).
pub fn last_backup(dir: &Path) -> Option<(PathBuf, i64)> {
    let p = backups(dir).into_iter().max()?;
    let m = std::fs::metadata(&p).ok()?.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs() as i64;
    Some((p, m))
}

/// Directory of the main database file (from the connection).
pub fn db_dir(c: &Connection) -> Option<PathBuf> {
    let f: String = c.query_row("SELECT file FROM pragma_database_list WHERE name = 'main'", [], |r| r.get(0)).ok()?;
    Path::new(&f).parent().map(Path::to_path_buf)
}

/// Client prefix of match rows made by an agent's MCP skill_search (e.g. `mcp:claude@mac`).
pub const MCP_PREFIX: &str = "mcp:";
/// SQL condition (on match_log) that keeps hook rows and drops MCP searches.
const HOOK_ONLY: &str = "(m.client IS NULL OR substr(m.client, 1, 4) <> 'mcp:')";

/// True for a match row made by an MCP skill_search rather than the prompt hook.
pub fn is_mcp(client: Option<&str>) -> bool {
    client.is_some_and(|c| c.starts_with(MCP_PREFIX))
}

/// Timestamp of the oldest match row.
pub fn log_first_ts(c: &Connection) -> Result<Option<i64>> {
    Ok(c.query_row("SELECT MIN(ts) FROM match_log", [], |r| r.get(0))?)
}

/// name -> (last match ts, matches in 7 days, matches in 30 days) from hook rows, computed in SQL.
pub fn match_counts(c: &Connection, t: i64) -> Result<HashMap<String, (i64, i64, i64)>> {
    let mut st = c.prepare(&format!(
        "SELECT json_extract(j.value, '$[0]'), MAX(m.ts), SUM(m.ts >= ?1), COUNT(*)
         FROM match_log m, json_each(m.hits) j WHERE m.ts >= ?2 AND {HOOK_ONLY} GROUP BY 1"
    ))?;
    let rows = st.query_map([t - 7 * 86400, t - 30 * 86400], |r| Ok((r.get::<_, Option<String>>(0)?, (r.get(1)?, r.get(2)?, r.get(3)?))))?;
    let mut m = HashMap::new();
    for r in rows {
        if let (Some(n), v) = r? {
            m.insert(n, v);
        }
    }
    Ok(m)
}

/// Skill loads at or after `since`, per skill.
pub fn load_counts(c: &Connection, since: i64) -> Result<HashMap<String, i64>> {
    let mut st = c.prepare("SELECT skill, COUNT(*) FROM load_log WHERE ts >= ?1 GROUP BY skill")?;
    let rows = st.query_map([since], |r| Ok((r.get(0)?, r.get(1)?)))?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Timestamps of hook matches that hit `skill` and of its loads, both at or after `since`.
pub fn skill_activity(c: &Connection, skill: &str, since: i64) -> Result<(Vec<i64>, Vec<i64>)> {
    let mut st = c.prepare(&format!(
        "SELECT ts FROM match_log m WHERE ts >= ?2 AND {HOOK_ONLY} AND EXISTS (SELECT 1 FROM json_each(hits) WHERE json_extract(value,'$[0]') = ?1)"
    ))?;
    let m = st.query_map(params![skill, since], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
    let mut st = c.prepare("SELECT ts FROM load_log WHERE skill = ?1 AND ts >= ?2")?;
    let l = st.query_map(params![skill, since], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
    Ok((m, l))
}

/// True when stored embeddings have differing dimensions or differ from the recorded dimension.
pub fn dim_mismatch(c: &Connection) -> Result<bool> {
    let mut st = c.prepare("SELECT DISTINCT LENGTH(embedding) / 4 FROM skills WHERE embedding IS NOT NULL")?;
    let dims: Vec<i64> = st.query_map([], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
    if dims.len() > 1 {
        return Ok(true);
    }
    let stored = meta_get(c, "embed_dim")?.and_then(|s| s.parse::<i64>().ok());
    Ok(matches!((dims.first(), stored), (Some(d), Some(s)) if *d != s))
}

/// (version, updated_at, description, body) from skill_history, newest first.
pub fn history(c: &Connection, skill: &str) -> Result<Vec<(i64, i64, String, String)>> {
    let mut st = c.prepare("SELECT version, updated_at, description, body FROM skill_history WHERE skill = ?1 ORDER BY version DESC")?;
    let rows = st.query_map([skill], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn history_version(c: &Connection, skill: &str, version: i64) -> Result<Option<(i64, i64, String, String)>> {
    Ok(c.query_row(
        "SELECT version, updated_at, description, body FROM skill_history WHERE skill = ?1 AND version = ?2",
        params![skill, version],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
    )
    .optional()?)
}

/// SKILL.md body and file paths of one skill, for the spec check.
pub fn spec_input(c: &Connection, name: &str) -> Result<Option<(String, Vec<String>)>> {
    let Some(body) = c.query_row("SELECT body FROM skills WHERE name = ?1", [name], |r| r.get::<_, String>(0)).optional()? else {
        return Ok(None);
    };
    let mut st = c.prepare("SELECT path FROM files WHERE skill = ?1 ORDER BY path")?;
    let paths = st.query_map([name], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
    Ok(Some((body, paths)))
}

pub fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

pub fn list(c: &Connection) -> Result<Vec<Summary>> {
    let mut st = c.prepare("SELECT name, description, version FROM skills ORDER BY name")?;
    let rows = st.query_map([], |r| Ok(Summary { name: r.get(0)?, description: r.get(1)?, version: r.get(2)? }))?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn get(c: &Connection, name: &str) -> Result<Option<Skill>> {
    let Some((description, body, version)) = c
        .query_row("SELECT description, body, version FROM skills WHERE name = ?1", [name], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?))
        })
        .optional()?
    else {
        return Ok(None);
    };
    let mut st = c.prepare("SELECT path, mode, content FROM files WHERE skill = ?1 ORDER BY path")?;
    let files = st
        .query_map([name], |r| Ok(File { path: r.get(0)?, mode: r.get(1)?, b64: B64.encode(r.get::<_, Vec<u8>>(2)?) }))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut st = c.prepare("SELECT key, value FROM secrets WHERE skill = ?1 ORDER BY key")?;
    let secrets = st
        .query_map([name], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<BTreeMap<String, String>>>()?;
    Ok(Some(Skill { name: name.into(), description, body, version, files: Some(files), secrets: Some(secrets) }))
}

/// Insert or replace a skill. The previous version is snapshotted into skill_history (newest 20 kept).
/// Files and secrets are replaced only when `s.files` / `s.secrets` is Some.
pub fn put(c: &Connection, s: &Skill, embedding: &[f32], model: &str) -> Result<Summary> {
    let tx = c.unchecked_transaction()?;
    tx.execute(
        "INSERT OR REPLACE INTO skill_history (skill, version, description, body, updated_at)
         SELECT name, version, description, body, updated_at FROM skills WHERE name = ?1",
        [&s.name],
    )?;
    tx.execute(
        "DELETE FROM skill_history WHERE skill = ?1 AND version NOT IN
           (SELECT version FROM skill_history WHERE skill = ?1 ORDER BY version DESC LIMIT 20)",
        [&s.name],
    )?;
    tx.execute(
        "INSERT INTO skills (name, description, body, version, updated_at, embedding)
         VALUES (?1, ?2, ?3, 1, ?4, ?5)
         ON CONFLICT(name) DO UPDATE SET description = excluded.description, body = excluded.body,
           version = version + 1, updated_at = excluded.updated_at, embedding = excluded.embedding",
        params![s.name, s.description, s.body, now(), to_blob(embedding)],
    )?;
    if let Some(files) = &s.files {
        tx.execute("DELETE FROM files WHERE skill = ?1", [&s.name])?;
        for f in files {
            tx.execute(
                "INSERT INTO files (skill, path, content, mode) VALUES (?1, ?2, ?3, ?4)",
                params![s.name, f.path, B64.decode(&f.b64)?, f.mode],
            )?;
        }
    }
    note_embedding(&tx, model, embedding.len())?;
    if let Some(sec) = &s.secrets {
        tx.execute("DELETE FROM secrets WHERE skill = ?1", [&s.name])?;
        for (k, v) in sec {
            tx.execute("INSERT INTO secrets (skill, key, value) VALUES (?1, ?2, ?3)", params![s.name, k, v])?;
        }
    }
    let version = tx.query_row("SELECT version FROM skills WHERE name = ?1", [&s.name], |r| r.get(0))?;
    tx.commit()?;
    Ok(Summary { name: s.name.clone(), description: s.description.clone(), version })
}

pub fn delete(c: &Connection, name: &str) -> Result<bool> {
    Ok(c.execute("DELETE FROM skills WHERE name = ?1", [name])? > 0)
}

/// (name, description, embedding) for every embedded skill.
pub fn vectors(c: &Connection) -> Result<Vec<(String, String, Vec<f32>)>> {
    let mut st = c.prepare("SELECT name, description, embedding FROM skills WHERE embedding IS NOT NULL")?;
    let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, from_blob(&r.get::<_, Vec<u8>>(2)?))))?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn set_embedding(c: &Connection, name: &str, e: &[f32], model: &str) -> Result<()> {
    c.execute("UPDATE skills SET embedding = ?2 WHERE name = ?1", params![name, to_blob(e)])?;
    note_embedding(c, model, e.len())?;
    Ok(())
}

/// Returns the new row id.
/// `prompt` is already clipped by the caller; `prompt_len` is the original length in chars.
pub fn log_match(c: &Connection, prompt: &str, hits: &str, ms: u64, client: Option<&str>, prompt_len: usize) -> Result<i64> {
    c.execute(
        "INSERT INTO match_log (ts, prompt, hits, ms, client, prompt_len) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![now(), prompt, hits, ms as i64, client, prompt_len as i64],
    )?;
    Ok(c.last_insert_rowid())
}

/// A skill counts as loaded for a match when it was fetched within this many seconds after it.
pub const LOAD_WINDOW: i64 = 120;

/// Record that a skill was fetched (GET /skills/{name}, i.e. MCP skill_get).
pub fn log_load(c: &Connection, skill: &str) -> Result<()> {
    c.execute("INSERT INTO load_log (ts, skill) VALUES (?1, ?2)", params![now(), skill])?;
    Ok(())
}

/// Delete log rows older than `before_ts`; returns how many went.
pub fn prune(c: &Connection, before_ts: i64) -> Result<usize> {
    let a = c.execute("DELETE FROM match_log WHERE ts < ?1", [before_ts])?;
    let b = c.execute("DELETE FROM load_log WHERE ts < ?1", [before_ts])?;
    Ok(a + b)
}

/// DELETE /api/log: clears match rows only.
pub fn clear_log(c: &Connection, before: i64) -> Result<usize> {
    Ok(c.execute("DELETE FROM match_log WHERE ts < ?1", [before])?)
}

fn parse_hits(s: &str) -> Vec<LogHit> {
    let v: Vec<(String, f32)> = serde_json::from_str(s).unwrap_or_default();
    v.into_iter().map(|(name, score)| LogHit { name, score }).collect()
}

fn log_row(r: &rusqlite::Row) -> rusqlite::Result<LogRow> {
    Ok(LogRow { id: r.get(0)?, ts: r.get(1)?, prompt: r.get(2)?, hits: parse_hits(&r.get::<_, String>(3)?), ms: r.get(4)?, loaded: vec![], client: r.get(5)?, prompt_len: r.get(6)? })
}

/// All (ts, skill) loads at or after `since`, oldest first.
pub fn loads_since(c: &Connection, since: i64) -> Result<Vec<(i64, String)>> {
    let mut st = c.prepare("SELECT ts, skill FROM load_log WHERE ts >= ?1 ORDER BY ts")?;
    let rows = st.query_map([since], |r| Ok((r.get(0)?, r.get(1)?)))?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Match rows at or after `since`, newest first, `loaded` not yet filled.
pub fn log_since(c: &Connection, since: i64) -> Result<Vec<LogRow>> {
    let mut st = c.prepare("SELECT rowid AS id, ts, prompt, hits, ms, client, prompt_len FROM match_log WHERE ts >= ?1 ORDER BY rowid DESC")?;
    let rows = st.query_map([since], log_row)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Fill `loaded`: skills of each row's hits that have a load within LOAD_WINDOW s after the match.
/// `loads` must be sorted by ts.
pub fn fill_loaded(rows: &mut [LogRow], loads: &[(i64, String)]) {
    let mut by: HashMap<&str, Vec<i64>> = HashMap::new();
    for (ts, s) in loads {
        by.entry(s.as_str()).or_default().push(*ts);
    }
    for r in rows {
        r.loaded = r
            .hits
            .iter()
            .filter(|h| {
                by.get(h.name.as_str()).is_some_and(|v| {
                    let i = v.partition_point(|t| *t < r.ts);
                    v.get(i).is_some_and(|t| *t <= r.ts + LOAD_WINDOW)
                })
            })
            .map(|h| h.name.clone())
            .collect();
    }
}

/// One page of the log, newest first (keyset on id). Fetches `limit + 1` rows so the caller can tell
/// whether more exist; `loaded` is filled.
pub fn log_page(c: &Connection, skill: Option<&str>, nohit: bool, q: Option<&str>, before: Option<i64>, limit: usize) -> Result<Vec<LogRow>> {
    use rusqlite::types::Value;
    let mut sql = String::from("SELECT rowid AS id, ts, prompt, hits, ms, client, prompt_len FROM match_log WHERE 1=1");
    let mut args: Vec<Value> = vec![];
    if let Some(s) = skill {
        sql += " AND EXISTS (SELECT 1 FROM json_each(hits) WHERE json_extract(value,'$[0]') = ?)";
        args.push(s.to_string().into());
    }
    if nohit {
        sql += " AND json_array_length(hits) = 0";
    }
    if let Some(q) = q {
        sql += " AND instr(lower(prompt), lower(?)) > 0";
        args.push(q.to_string().into());
    }
    if let Some(b) = before {
        sql += " AND rowid < ?";
        args.push(b.into());
    }
    sql += " ORDER BY rowid DESC LIMIT ?";
    args.push((limit as i64 + 1).into());
    let mut st = c.prepare(&sql)?;
    let mut rows = st.query_map(rusqlite::params_from_iter(args), log_row)?.collect::<rusqlite::Result<Vec<_>>>()?;
    attach_loaded(c, &mut rows)?;
    Ok(rows)
}

pub fn attach_loaded(c: &Connection, rows: &mut [LogRow]) -> Result<()> {
    let (Some(lo), Some(hi)) = (rows.iter().map(|r| r.ts).min(), rows.iter().map(|r| r.ts).max()) else { return Ok(()) };
    let loads: Vec<_> = loads_since(c, lo)?.into_iter().filter(|(t, _)| *t <= hi + LOAD_WINDOW).collect();
    fill_loaded(rows, &loads);
    Ok(())
}

/// Hits JSON of every hook match row that hit `skill` (for co-hit counts).
pub fn hits_with(c: &Connection, skill: &str) -> Result<Vec<Vec<LogHit>>> {
    let mut st = c.prepare(&format!(
        "SELECT hits FROM match_log m WHERE {HOOK_ONLY} AND EXISTS (SELECT 1 FROM json_each(hits) WHERE json_extract(value,'$[0]') = ?1)"
    ))?;
    let rows = st.query_map([skill], |r| Ok(parse_hits(&r.get::<_, String>(0)?)))?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Base columns of every skill: (name, description, version, updated_at, files, bytes, secrets).
pub struct Base {
    pub name: String,
    pub description: String,
    pub version: i64,
    pub updated_at: i64,
    pub files: i64,
    pub bytes: i64,
    pub secrets: i64,
}

pub fn bases(c: &Connection) -> Result<Vec<Base>> {
    let mut st = c.prepare(
        "SELECT s.name, s.description, s.version, s.updated_at,
           (SELECT COUNT(*) FROM files f WHERE f.skill = s.name),
           (SELECT COALESCE(SUM(LENGTH(f.content)), 0) FROM files f WHERE f.skill = s.name) + LENGTH(CAST(s.body AS BLOB)),
           (SELECT COUNT(*) FROM secrets x WHERE x.skill = s.name)
         FROM skills s ORDER BY s.name",
    )?;
    let rows = st.query_map([], |r| {
        Ok(Base { name: r.get(0)?, description: r.get(1)?, version: r.get(2)?, updated_at: r.get(3)?, files: r.get(4)?, bytes: r.get(5)?, secrets: r.get(6)? })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// (description, body, version, updated_at, embedding) of one skill.
#[allow(clippy::type_complexity)]
pub fn detail_row(c: &Connection, name: &str) -> Result<Option<(String, String, i64, i64, Option<Vec<f32>>)>> {
    Ok(c.query_row("SELECT description, body, version, updated_at, embedding FROM skills WHERE name = ?1", [name], |r| {
        Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get::<_, Option<Vec<u8>>>(4)?.map(|b| from_blob(&b))))
    })
    .optional()?)
}

pub fn file_infos(c: &Connection, name: &str) -> Result<Vec<FileInfo>> {
    let mut st = c.prepare("SELECT path, mode, LENGTH(content) FROM files WHERE skill = ?1 ORDER BY path")?;
    let rows = st.query_map([name], |r| Ok(FileInfo { path: r.get(0)?, mode: r.get(1)?, size: r.get(2)? }))?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn secret_keys(c: &Connection, name: &str) -> Result<Vec<String>> {
    let mut st = c.prepare("SELECT key FROM secrets WHERE skill = ?1 ORDER BY key")?;
    let rows = st.query_map([name], |r| r.get(0))?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn file_content(c: &Connection, name: &str, path: &str) -> Result<Option<Vec<u8>>> {
    Ok(c.query_row("SELECT content FROM files WHERE skill = ?1 AND path = ?2", params![name, path], |r| r.get(0)).optional()?)
}

/// (skills, match_log rows, db size in bytes)
pub fn counts(c: &Connection) -> Result<(i64, i64, i64)> {
    let n = |sql: &str| c.query_row(sql, [], |r| r.get::<_, i64>(0));
    Ok((n("SELECT COUNT(*) FROM skills")?, n("SELECT COUNT(*) FROM match_log")?, n("SELECT page_count * page_size FROM pragma_page_count, pragma_page_size")?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mem() -> Connection {
        let c = Connection::open_in_memory().unwrap();
        init(&c).unwrap();
        c
    }

    fn add_match(c: &Connection, ts: i64, prompt: &str, hits: &str) {
        c.execute("INSERT INTO match_log (ts, prompt, hits, ms) VALUES (?1, ?2, ?3, 10)", params![ts, prompt, hits]).unwrap();
    }

    #[test]
    fn log_load_and_prune() {
        let c = mem();
        log_load(&c, "a").unwrap();
        c.execute("INSERT INTO load_log (ts, skill) VALUES (100, 'old')", []).unwrap();
        add_match(&c, 100, "old", "[]");
        add_match(&c, now(), "new", "[]");
        assert_eq!(prune(&c, now() - 1000).unwrap(), 2);
        assert_eq!(loads_since(&c, 0).unwrap().len(), 1);
        assert_eq!(log_since(&c, 0).unwrap().len(), 1);
    }

    #[test]
    fn loaded_within_window() {
        let c = mem();
        add_match(&c, 1000, "p1", r#"[["a",0.9],["b",0.5]]"#);
        add_match(&c, 2000, "p2", r#"[["a",0.9]]"#);
        add_match(&c, 3000, "none", "[]");
        for (ts, s) in [(1100, "a"), (1121, "b"), (1990, "a"), (2500, "a")] {
            c.execute("INSERT INTO load_log (ts, skill) VALUES (?1, ?2)", params![ts, s]).unwrap();
        }
        let rows = log_page(&c, None, false, None, None, 10).unwrap();
        let by = |p: &str| rows.iter().find(|r| r.prompt == p).unwrap();
        assert_eq!(by("p1").loaded, vec!["a"]); // b loaded 121 s later: too late
        assert!(by("p2").loaded.is_empty()); // a loaded before the match or 500 s after
        assert_eq!(log_page(&c, None, true, None, None, 10).unwrap().len(), 1);
        assert_eq!(log_page(&c, Some("b"), false, None, None, 10).unwrap().len(), 1);
        assert_eq!(log_page(&c, Some("a"), false, Some("P2"), None, 10).unwrap()[0].prompt, "p2");
        assert_eq!(log_page(&c, None, false, None, Some(rows[1].id), 10).unwrap().len(), 1);
    }

    fn skill(name: &str, body: &str, files: Option<Vec<File>>) -> Skill {
        Skill { name: name.into(), description: "d".into(), body: body.into(), version: 0, files, secrets: None }
    }

    #[test]
    fn history_keeps_20_and_tracks_meta() {
        let c = mem();
        for i in 0..25 {
            put(&c, &skill("a", &format!("body {i}"), None), &[0.5, 0.5], "m1").unwrap();
        }
        let h = history(&c, "a").unwrap();
        assert_eq!(h.len(), 20);
        assert_eq!(h[0].0, 24); // current is v25, newest snapshot v24
        assert_eq!(h[0].3, "body 23");
        assert_eq!(h[19].0, 5);
        assert_eq!(meta_get(&c, "embed_model").unwrap().as_deref(), Some("m1"));
        assert_eq!(meta_get(&c, "embed_dim").unwrap().as_deref(), Some("2"));
        set_embedding(&c, "a", &[1.0, 0.0, 0.0], "m2").unwrap();
        assert_eq!(meta_get(&c, "embed_dim").unwrap().as_deref(), Some("3"));
        assert!(history(&c, "nope").unwrap().is_empty());
    }

    #[test]
    fn files_none_keeps_some_replaces() {
        let c = mem();
        let f = |p: &str| File { path: p.into(), mode: 0o644, b64: B64.encode("x") };
        put(&c, &skill("a", "b", Some(vec![f("one"), f("two")])), &[1.0], "m").unwrap();
        put(&c, &skill("a", "b2", None), &[1.0], "m").unwrap();
        assert_eq!(get(&c, "a").unwrap().unwrap().files.unwrap().len(), 2);
        put(&c, &skill("a", "b3", Some(vec![f("three")])), &[1.0], "m").unwrap();
        let files = get(&c, "a").unwrap().unwrap().files.unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "three");
        put(&c, &skill("a", "b4", Some(vec![])), &[1.0], "m").unwrap();
        assert!(get(&c, "a").unwrap().unwrap().files.unwrap().is_empty());
    }

    #[test]
    fn migrates_v03_schema() {
        let dir = std::env::temp_dir().join(format!("mimir-mig-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("old.db");
        {
            let c = Connection::open(&path).unwrap();
            c.execute_batch(
                "CREATE TABLE skills (name TEXT PRIMARY KEY, description TEXT NOT NULL, body TEXT NOT NULL,
                   version INTEGER NOT NULL DEFAULT 1, updated_at INTEGER NOT NULL, embedding BLOB);
                 CREATE TABLE match_log (ts INTEGER NOT NULL, prompt TEXT NOT NULL, hits TEXT NOT NULL, ms INTEGER NOT NULL);
                 CREATE TABLE load_log(ts INTEGER NOT NULL, skill TEXT NOT NULL);
                 INSERT INTO match_log VALUES (5, 'old prompt', '[]', 3);",
            )
            .unwrap();
        }
        let c = open(&path).unwrap();
        let rows = log_page(&c, None, false, None, None, 10).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].client, None);
        assert_eq!(rows[0].prompt_len, 0);
        log_match(&c, "p", "[]", 1, Some("cli@x"), 42).unwrap();
        assert_eq!(log_page(&c, None, false, None, None, 10).unwrap()[0].prompt_len, 42);
        assert!(history(&c, "x").unwrap().is_empty());
        drop(c);
        open(&path).unwrap(); // second open is a no-op
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn db_and_backup_dir_are_private_from_the_start() {
        let dir = std::env::temp_dir().join(format!("mimir-perm-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
        // The file exists 0600 before SQLite ever opens it.
        let p = dir.join("new.db");
        std::fs::create_dir_all(&dir).unwrap();
        create_private(&p, false).unwrap();
        assert_eq!(mode(&p), 0o600);
        assert!(create_private(&p, true).is_err());
        let c = open(&p).unwrap();
        put(&c, &skill("a", "b", None), &[1.0], "m").unwrap();
        assert_eq!(mode(&p), 0o600);
        assert_eq!(mode(&dir.join("new.db-wal")), 0o600);
        // An older, world-readable DB is tightened.
        let old = dir.join("old.db");
        std::fs::write(&old, b"").unwrap();
        std::fs::set_permissions(&old, std::fs::Permissions::from_mode(0o644)).unwrap();
        open(&old).unwrap();
        assert_eq!(mode(&old), 0o600);
        // A backups dir that does not exist yet is created 0700.
        let b = backup(&c, &dir.join("backups"), 7).unwrap();
        assert_eq!(mode(&dir.join("backups")), 0o700);
        assert_eq!(mode(&b), 0o600);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn backup_keeps_newest() {
        let dir = std::env::temp_dir().join(format!("mimir-bak-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        for n in ["20200101-000000", "20200102-000000", "20200103-000000"] {
            std::fs::write(dir.join(format!("mimir-{n}.db")), b"x").unwrap();
        }
        let c = mem();
        put(&c, &skill("a", "b", None), &[1.0], "m").unwrap();
        let p = backup(&c, &dir, 2).unwrap();
        assert!(p.exists());
        assert_eq!(std::fs::metadata(&p).unwrap().permissions().mode() & 0o777, 0o600);
        assert_eq!(std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777, 0o700);
        assert_eq!(get(&open(&p).unwrap(), "a").unwrap().unwrap().body, "b");
        let mut left: Vec<_> = backups(&dir).into_iter().map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect();
        left.sort();
        assert_eq!(left.len(), 2);
        assert_eq!(left[0], "mimir-20200103-000000.db");
        assert_eq!(last_backup(&dir).unwrap().0, p);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn mcp_rows_stay_out_of_hook_counts() {
        let c = mem();
        let t = now();
        log_match(&c, "hook", r#"[["a",0.9]]"#, 1, Some("claude@x"), 4).unwrap();
        log_match(&c, "agent", r#"[["a",0.1],["b",0.05]]"#, 1, Some("mcp:claude@x"), 5).unwrap();
        let m = match_counts(&c, t).unwrap();
        assert_eq!(m["a"].2, 1);
        assert!(!m.contains_key("b"));
        assert_eq!(skill_activity(&c, "a", t - 60).unwrap().0.len(), 1);
        assert_eq!(hits_with(&c, "a").unwrap().len(), 1);
        assert_eq!(log_page(&c, None, false, None, None, 10).unwrap().len(), 2);
        assert!(is_mcp(Some("mcp:claude@x")) && !is_mcp(Some("claude@x")) && !is_mcp(None));
    }

    #[test]
    fn counts_and_log_span() {
        let c = mem();
        let t = now();
        add_match(&c, t - 10 * 86400, "old", r#"[["a",0.9]]"#);
        add_match(&c, t - 86400, "new", r#"[["a",0.9],["b",0.4]]"#);
        let m = match_counts(&c, t).unwrap();
        assert_eq!(m["a"].2, 2);
        assert_eq!(m["a"].1, 1);
        assert_eq!(m["b"].2, 1);
        assert!(!m.contains_key("c"));
        assert_eq!(log_first_ts(&c).unwrap(), Some(t - 10 * 86400));
        assert_eq!(day_string(1_709_164_800), "2024-02-29");
    }
}
