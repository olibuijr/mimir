use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A file that ships with a skill (script, reference doc, asset). Content is base64.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct File {
    pub path: String,
    pub mode: u32,
    pub b64: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Skill {
    #[serde(default)]
    pub name: String,
    pub description: String,
    /// Full SKILL.md text, frontmatter included.
    pub body: String,
    #[serde(default)]
    pub version: i64,
    /// None on a PUT keeps the stored files; Some replaces them. GET always returns Some.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub files: Option<Vec<File>>,
    /// None on a PUT keeps the stored secrets; Some replaces them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secrets: Option<BTreeMap<String, String>>,
}

/// Validate files of a PUT: every b64 decodes, paths are relative and clean, no duplicates, no SKILL.md.
/// Returns the files with the mode masked to 0o777.
pub fn check_files(files: &[File]) -> Result<Vec<File>, String> {
    use base64::{engine::general_purpose::STANDARD as B64, Engine};
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::with_capacity(files.len());
    for f in files {
        let p = f.path.as_str();
        if p.starts_with('/') || p.split('/').any(|s| s == ".." || s.is_empty()) {
            return Err(format!("file path `{p}` must be relative to the skill directory"));
        }
        if p == "SKILL.md" {
            return Err("SKILL.md is stored as the body, not as a file".into());
        }
        if !seen.insert(p) {
            return Err(format!("duplicate file path `{p}`"));
        }
        if B64.decode(&f.b64).is_err() {
            return Err(format!("file `{p}`: content is not valid base64"));
        }
        out.push(File { path: f.path.clone(), mode: f.mode & 0o777, b64: f.b64.clone() });
    }
    Ok(out)
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Summary {
    pub name: String,
    pub description: String,
    pub version: i64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Hit {
    pub name: String,
    pub description: String,
    /// Reranker score (or cosine when reranking is off).
    pub score: f32,
    pub cosine: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct MatchReq {
    pub prompt: String,
    #[serde(default)]
    pub k: Option<usize>,
    #[serde(default)]
    pub min: Option<f32>,
    #[serde(default)]
    pub rerank: Option<bool>,
    #[serde(default)]
    pub log: Option<bool>,
    /// Experimental lexical rescue (unique trigger words); off unless set or MIMIR_LEXICAL=1.
    #[serde(default)]
    pub lexical: Option<bool>,
    /// Who asked: `"mcp"` for an agent's skill_search tool call. The server logs those rows with the
    /// client prefixed `mcp:` and keeps them out of the hook's hit-rate stats.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// How long the caller will wait for the answer, in ms. The server fits its model calls into it
    /// (skipping the reranker when too little is left). Default `MIMIR_MATCH_BUDGET_MS` (2000).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget_ms: Option<u64>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MatchResp {
    pub hits: Vec<Hit>,
    pub ms: u64,
    /// False when the scores are cosine similarities: reranking was off, or it did not fit the budget
    /// and the hits are in embedding order. Older servers do not send it.
    #[serde(default = "yes")]
    pub reranked: bool,
}

fn yes() -> bool {
    true
}

// Responses of the read API (/api/...).

#[derive(Serialize, Clone, Debug)]
pub struct SkillRow {
    pub name: String,
    pub description: String,
    pub version: i64,
    pub updated_at: i64,
    pub files: i64,
    pub bytes: i64,
    pub secrets: i64,
    pub last_matched: Option<i64>,
    pub matches_7d: i64,
    pub matches_30d: i64,
    pub loads_7d: i64,
    /// Passes the Agent Skills spec (no spec errors).
    pub compliant: bool,
    pub warnings: Vec<String>,
    /// No match in 30 days while the log spans at least 7 days.
    pub stale: bool,
}

#[derive(Serialize, Clone, Debug)]
pub struct FileInfo {
    pub path: String,
    pub mode: u32,
    pub size: i64,
}

#[derive(Serialize, Clone, Debug)]
pub struct Similar {
    pub name: String,
    pub cosine: f32,
    pub co_hits: i64,
}

#[derive(Serialize, Clone, Debug)]
pub struct LogHit {
    pub name: String,
    pub score: f32,
}

#[derive(Serialize, Clone, Debug)]
pub struct LogRow {
    pub id: i64,
    pub ts: i64,
    pub prompt: String,
    pub hits: Vec<LogHit>,
    pub ms: i64,
    pub loaded: Vec<String>,
    pub client: Option<String>,
    pub prompt_len: i64,
}

#[derive(Serialize, Clone, Debug)]
pub struct DayCount {
    pub day: String,
    pub matches: i64,
    pub loads: i64,
}

#[derive(Serialize, Clone, Debug)]
pub struct SkillDetail {
    pub name: String,
    pub description: String,
    pub version: i64,
    pub updated_at: i64,
    pub body: String,
    pub html: String,
    pub frontmatter: crate::spec::Frontmatter,
    pub spec: crate::spec::Report,
    pub files: Vec<FileInfo>,
    pub secret_keys: Vec<String>,
    pub recent: Vec<LogRow>,
    pub similar: Vec<Similar>,
    pub per_day: Vec<DayCount>,
}



#[cfg(test)]
mod tests {
    use super::*;

    fn f(path: &str, b64: &str, mode: u32) -> File {
        File { path: path.into(), mode, b64: b64.into() }
    }

    #[test]
    fn check_files_rules() {
        let ok = check_files(&[f("scripts/a.sh", "aGk=", 0o100755)]).unwrap();
        assert_eq!(ok[0].mode, 0o755);
        assert!(check_files(&[f("/abs", "aGk=", 0o644)]).is_err());
        assert!(check_files(&[f("a/../b", "aGk=", 0o644)]).is_err());
        assert!(check_files(&[f("SKILL.md", "aGk=", 0o644)]).is_err());
        assert!(check_files(&[f("x", "!!", 0o644)]).is_err());
        assert!(check_files(&[f("x", "aGk=", 0o644), f("x", "aGk=", 0o644)]).is_err());
    }
}
