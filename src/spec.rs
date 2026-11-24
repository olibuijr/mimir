//! The Agent Skills specification (https://agentskills.io/specification).
//!
//! A skill is a directory named after the skill holding `SKILL.md` (YAML frontmatter + Markdown)
//! and optionally `scripts/`, `references/`, `assets/` and any other files. Mímir stores exactly
//! that and refuses anything that breaks the spec's hard rules; soft recommendations become warnings.
//! Checked against the reference validator `skills-ref validate`.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use yaml_rust2::{Yaml, YamlLoader};

/// The only frontmatter fields the spec defines. Anything else belongs under `metadata`.
pub const FIELDS: &[&str] = &["name", "description", "license", "compatibility", "metadata", "allowed-tools"];
pub const MAX_NAME: usize = 64;
pub const MAX_DESCRIPTION: usize = 1024;
pub const MAX_COMPATIBILITY: usize = 500;
/// Recommended size of the SKILL.md body.
pub const MAX_LINES: usize = 500;
pub const MAX_TOKENS: usize = 5000;

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
pub struct Frontmatter {
    pub name: Option<String>,
    pub description: Option<String>,
    pub license: Option<String>,
    pub compatibility: Option<String>,
    pub metadata: BTreeMap<String, String>,
    #[serde(rename = "allowed-tools")]
    pub allowed_tools: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Report {
    /// Spec violations: the skill is refused.
    pub errors: Vec<String>,
    /// Spec recommendations not met.
    pub warnings: Vec<String>,
}

impl Report {
    pub fn ok(&self) -> bool {
        self.errors.is_empty()
    }
}

/// Split SKILL.md into (frontmatter YAML, Markdown body). None when it does not start with `---`.
pub fn split(md: &str) -> Option<(&str, &str)> {
    let md = md.strip_prefix('\u{feff}').unwrap_or(md);
    let first = md.split_inclusive('\n').next()?;
    if first.trim_end() != "---" {
        return None;
    }
    let rest = &md[first.len()..];
    let mut off = 0;
    for line in rest.split_inclusive('\n') {
        if line.trim_end() == "---" {
            return Some((&rest[..off], &rest[off + line.len()..]));
        }
        off += line.len();
    }
    None
}

/// The Markdown body, or the whole text when there is no frontmatter.
pub fn body(md: &str) -> &str {
    split(md).map_or(md, |(_, b)| b)
}

/// The spec's name rule: 1-64 of a-z, 0-9 and single inner hyphens.
pub fn valid_name(n: &str) -> bool {
    !n.is_empty()
        && n.len() <= MAX_NAME
        && n.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && !n.starts_with('-')
        && !n.ends_with('-')
        && !n.contains("--")
}

/// A YAML scalar as text. Like the reference validator (strictyaml), plain scalars such as
/// `170` or `true` count as strings; sequences and maps do not.
fn scalar(v: &Yaml) -> Option<String> {
    match v {
        Yaml::String(s) | Yaml::Real(s) => Some(s.clone()),
        Yaml::Integer(i) => Some(i.to_string()),
        Yaml::Boolean(b) => Some(b.to_string()),
        _ => None,
    }
}

/// Parse the frontmatter. Errors are spec violations in the YAML itself (syntax, unknown fields, types).
pub fn parse(md: &str) -> (Frontmatter, Vec<String>) {
    let mut fm = Frontmatter::default();
    let mut errors = vec![];
    let Some((yaml, _)) = split(md) else {
        errors.push("SKILL.md must start with YAML frontmatter between `---` lines".into());
        return (fm, errors);
    };
    let docs = match YamlLoader::load_from_str(yaml) {
        Ok(d) => d,
        Err(e) => {
            errors.push(format!("invalid YAML in frontmatter: {e} (quote values that contain `: `)"));
            return (fm, errors);
        }
    };
    let Some(Yaml::Hash(map)) = docs.into_iter().next() else {
        errors.push("frontmatter must be a YAML mapping".into());
        return (fm, errors);
    };
    for (k, v) in map {
        let Yaml::String(key) = k else {
            errors.push("frontmatter keys must be strings".into());
            continue;
        };
        let text = |errors: &mut Vec<String>| match scalar(&v) {
            Some(s) => Some(s),
            None => {
                errors.push(format!("`{key}` must be a string"));
                None
            }
        };
        match key.as_str() {
            "name" => fm.name = text(&mut errors),
            "description" => fm.description = text(&mut errors),
            "license" => fm.license = text(&mut errors),
            "compatibility" => fm.compatibility = text(&mut errors),
            "allowed-tools" => fm.allowed_tools = text(&mut errors),
            "metadata" => match &v {
                Yaml::Hash(m) => {
                    for (mk, mv) in m {
                        match (scalar(mk), scalar(mv)) {
                            (Some(mk), Some(mv)) => {
                                fm.metadata.insert(mk, mv);
                            }
                            (Some(mk), None) => errors.push(format!("metadata `{mk}` must be a string value, not a list or map")),
                            (None, _) => errors.push("metadata keys must be strings".into()),
                        }
                    }
                }
                Yaml::Null => {}
                _ => errors.push("`metadata` must be a map of string keys to string values".into()),
            },
            other => errors.push(format!(
                "unexpected field `{other}`: the spec allows only {}; put extra data under `metadata`",
                FIELDS.join(", ")
            )),
        }
    }
    (fm, errors)
}

/// Validate a skill: its name (the directory name), SKILL.md and the paths of its other files.
pub fn validate(name: &str, md: &str, files: &[&str]) -> Report {
    let (fm, mut errors) = parse(md);
    // After a YAML error the fields are unknown; do not also report them as missing.
    let parsed = errors.is_empty() && split(md).is_some();
    let mut warnings = vec![];

    if !valid_name(name) {
        errors.push(format!("skill name `{name}` must be 1-{MAX_NAME} lowercase letters, digits and single hyphens, not starting or ending with a hyphen"));
    }
    match &fm.name {
        None if parsed => errors.push("missing required field `name`".into()),
        Some(n) if n != name => errors.push(format!("`name: {n}` must match the skill's directory name `{name}`")),
        _ => {}
    }
    match fm.description.as_deref().map(str::trim) {
        None if parsed => errors.push("missing required field `description`".into()),
        Some("") => errors.push("`description` must not be empty".into()),
        Some(d) if d.chars().count() > MAX_DESCRIPTION => {
            errors.push(format!("`description` is {} characters; the maximum is {MAX_DESCRIPTION}", d.chars().count()))
        }
        _ => {}
    }
    if let Some(c) = &fm.compatibility {
        let n = c.trim().chars().count();
        if n == 0 || n > MAX_COMPATIBILITY {
            errors.push(format!("`compatibility` must be 1-{MAX_COMPATIBILITY} characters (it is {n})"));
        }
    }
    for f in files {
        if f.starts_with('/') || f.split('/').any(|p| p == ".." || p.is_empty()) {
            errors.push(format!("file path `{f}` must be relative to the skill directory"));
        }
        if *f == "SKILL.md" {
            errors.push("SKILL.md is stored as the body, not as a file".into());
        }
    }

    let b = body(md);
    let lines = b.lines().count();
    if lines > MAX_LINES {
        warnings.push(format!("SKILL.md body is {lines} lines; the spec recommends under {MAX_LINES} (move detail into references/)"));
    }
    let tokens = b.len() / 4;
    if tokens > MAX_TOKENS {
        warnings.push(format!("SKILL.md body is about {tokens} tokens; the spec recommends under {MAX_TOKENS}"));
    }
    if let Some(d) = &fm.description {
        if d.trim().chars().count() < 60 {
            warnings.push("description is short; say what the skill does and when to use it, with trigger words".into());
        }
    }
    for r in references(b) {
        let present = files.iter().any(|f| *f == r || f.starts_with(&format!("{}/", r.trim_end_matches('/'))));
        if !present {
            warnings.push(format!("SKILL.md refers to `{r}`, which is not in the skill"));
        }
    }
    if b.contains(&format!(".claude/skills/{name}")) || b.contains("${CLAUDE_SKILL_DIR}") {
        warnings.push("uses an absolute or tool-specific skill path; the spec uses paths relative to the skill root (e.g. scripts/run.sh)".into());
    }
    errors.dedup();
    Report { errors, warnings }
}

/// Relative file references in the body: Markdown link targets and `scripts/`, `references/`,
/// `assets/` paths. External links, anchors and absolute paths are skipped.
pub fn references(body: &str) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    let stop = |c: char| c.is_whitespace() || "()[]<>`'\"|,;*".contains(c);
    let clean = |s: &str| s.trim_end_matches(['.', ':', '!', '?']).to_string();
    // Markdown links: ](target)
    for part in body.split("](").skip(1) {
        let t = part.split([')', ' ', '#']).next().unwrap_or("");
        let local = !t.is_empty() && !t.contains("://") && !t.starts_with(['/', '~', '$', '<']) && !t.starts_with("mailto:");
        if local && !t.starts_with('#') {
            out.push(clean(t));
        }
    }
    // Conventional directories mentioned anywhere.
    for dir in ["scripts/", "references/", "assets/"] {
        let mut rest = body;
        while let Some(i) = rest.find(dir) {
            let before = rest[..i].chars().last();
            let tail = &rest[i..];
            let end = tail.find(stop).unwrap_or(tail.len());
            let path = clean(&tail[..end]);
            // Only standalone mentions (not `foo/scripts/x` or `~/x/scripts/`) and not bare dirs.
            if before.is_none_or(&stop) && path.len() > dir.len() && !path.contains("..") && !path.contains('*') && !path.contains('<') {
                out.push(path);
            }
            rest = &tail[end.max(1)..];
        }
    }
    out.sort();
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn md(fm: &str) -> String {
        format!("---\n{fm}\n---\n# Body\n")
    }

    #[test]
    fn names_follow_the_spec() {
        for ok in ["pdf-processing", "a", "1984-dns", "zig-0-16"] {
            assert!(valid_name(ok), "{ok}");
        }
        for bad in ["", "PDF-Processing", "-pdf", "pdf-", "pdf--processing", "a/b", "../etc", "x:y", "a_b", &"a".repeat(65)] {
            assert!(!valid_name(bad), "{bad}");
        }
    }

    #[test]
    fn minimal_skill_is_valid() {
        let r = validate("pdf-processing", &md("name: pdf-processing\ndescription: Extract PDF text, fill forms and merge files. Use when handling PDFs."), &[]);
        assert!(r.ok(), "{:?}", r.errors);
        assert!(r.warnings.is_empty(), "{:?}", r.warnings);
    }

    #[test]
    fn optional_fields_parse() {
        let (fm, e) = parse(&md("name: x\ndescription: d\nlicense: Apache-2.0\ncompatibility: Requires git\nallowed-tools: Bash(git:*) Read\nmetadata:\n  author: example-org\n  version: \"1.0\""));
        assert!(e.is_empty(), "{e:?}");
        assert_eq!(fm.license.as_deref(), Some("Apache-2.0"));
        assert_eq!(fm.allowed_tools.as_deref(), Some("Bash(git:*) Read"));
        assert_eq!(fm.metadata.get("version").map(String::as_str), Some("1.0"));
        let (fm, e) = parse(&md("name: x\ndescription: d\nmetadata:\n  tool_count: 170\n  beta: true"));
        assert!(e.is_empty(), "{e:?}");
        assert_eq!(fm.metadata.get("tool_count").map(String::as_str), Some("170"));
    }

    #[test]
    fn folded_and_quoted_descriptions() {
        let (fm, e) = parse(&md("name: x\ndescription: >\n  line one\n  line two"));
        assert!(e.is_empty());
        assert_eq!(fm.description.as_deref().map(str::trim), Some("line one line two"));
        let (fm, _) = parse(&md("name: x\ndescription: \"Manage the \\\"router\\\": DHCP\""));
        assert_eq!(fm.description.as_deref(), Some("Manage the \"router\": DHCP"));
    }

    #[test]
    fn violations_are_errors() {
        let e = |fm: &str, name: &str| validate(name, &md(fm), &[]).errors;
        assert!(e("name: x\ndescription: d\nversion: 1.0", "x").iter().any(|m| m.contains("unexpected field `version`")));
        assert!(e("name: x\ndescription: d\nargument-hint: \"[a]\"", "x").iter().any(|m| m.contains("argument-hint")));
        assert!(e("name: y\ndescription: d", "x").iter().any(|m| m.contains("must match")));
        assert!(e("description: d", "x").iter().any(|m| m.contains("missing required field `name`")));
        assert!(e("name: x", "x").iter().any(|m| m.contains("`description`")));
        assert!(e(&format!("name: x\ndescription: {}", "a".repeat(1025)), "x").iter().any(|m| m.contains("1025")));
        assert!(e("name: x\ndescription: d\ncompatibility:\n  - opencode", "x").iter().any(|m| m.contains("`compatibility` must be a string")));
        assert!(e("name: x\ndescription: d\nmetadata:\n  tags: [a, b]", "x").iter().any(|m| m.contains("metadata `tags`")));
        let bad_yaml = e("name: x\ndescription: Use it: now: really", "x");
        assert!(bad_yaml.iter().any(|m| m.contains("invalid YAML")));
        assert!(!bad_yaml.iter().any(|m| m.contains("missing")), "{bad_yaml:?}");
        assert!(!validate("x", "# no frontmatter", &[]).ok());
        assert!(!validate("x", &md("name: x\ndescription: d"), &["../escape"]).ok());
    }

    #[test]
    fn file_references() {
        let b = "Run `scripts/extract.py` and see [guide](references/REFERENCE.md) or [web](https://x.y/a.md) and ~/.x/scripts/no.sh.";
        assert_eq!(references(b), vec!["references/REFERENCE.md".to_string(), "scripts/extract.py".to_string()]);
        let fm = "name: x\ndescription: Extract PDF text, fill forms and merge files. Use when handling PDFs.";
        let r = validate("x", &format!("---\n{fm}\n---\n{b}\n"), &["scripts/extract.py"]);
        assert_eq!(r.warnings, vec!["SKILL.md refers to `references/REFERENCE.md`, which is not in the skill".to_string()]);
    }

    #[test]
    fn body_split() {
        assert_eq!(body("---\nname: x\n---\nhello\n"), "hello\n");
        assert_eq!(body("no fm"), "no fm");
    }
}
