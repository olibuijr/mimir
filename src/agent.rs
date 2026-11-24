//! Commands for agents: `mimir new` (start a spec-compliant skill) and `mimir skill` (the skill for operating Mímir itself).
use crate::spec;
use anyhow::{bail, ensure, Result};
use std::path::Path;

/// The skill that teaches an agent to use Mímir. Compiled in, so it always matches this binary.
const SKILL: &str = include_str!("../skills/mimir/SKILL.md");

pub fn skill(install: Option<&Path>) -> Result<()> {
    let Some(dir) = install else {
        print!("{SKILL}");
        return Ok(());
    };
    let dir = dir.join("mimir");
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join("SKILL.md"), SKILL)?;
    println!("installed {}", dir.join("SKILL.md").display());
    Ok(())
}

/// YAML double-quoted scalar, so any description survives the frontmatter parser.
fn yaml_str(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', " "))
}

pub fn new_skill(parent: &Path, name: &str, description: Option<&str>, json: bool) -> Result<()> {
    ensure!(spec::valid_name(name), "invalid skill name {name:?}: use 1-64 lowercase letters, digits and single hyphens, not starting or ending with a hyphen");
    let dir = parent.join(name);
    let md = dir.join("SKILL.md");
    if md.exists() {
        bail!("{} already exists; edit it instead", md.display());
    }
    let desc = description.map(str::trim).filter(|d| !d.is_empty()).unwrap_or("TODO: what this skill does and when to use it. List the tasks, tools and trigger words a prompt would contain");
    let body = format!(
        "---\nname: {name}\ndescription: {}\n---\n\n# {name}\n\n## When to use\n\nTODO: the requests this skill answers, and what it is not for.\n\n## How\n\nTODO: the steps, commands and checks. Put scripts, long reference material and templates in\nthe scripts, references and assets folders, and link them by paths relative to this file.\n\n## Gotchas\n\nTODO: what goes wrong and how to tell.\n",
        yaml_str(desc)
    );
    let report = spec::validate(name, &body, &[]);
    ensure!(report.ok(), "generated SKILL.md does not follow the spec: {}", report.errors.join("; "));
    std::fs::create_dir_all(&dir)?;
    std::fs::write(&md, &body)?;
    let next = format!("mimir validate {0} && mimir import {0}", dir.display());
    if json {
        println!("{}", serde_json::json!({ "path": md, "next": next }));
    } else {
        println!("created {}\nfill in the TODOs, then: {next}", md.display());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_skill_is_valid_and_quotes_description() {
        let t = std::env::temp_dir().join(format!("mimir-new-{}", std::process::id()));
        new_skill(&t, "demo-skill", Some("Does \"x\": y\nand z"), true).unwrap();
        let md = std::fs::read_to_string(t.join("demo-skill/SKILL.md")).unwrap();
        assert!(spec::validate("demo-skill", &md, &[]).ok());
        let (fm, _) = spec::parse(&md);
        assert_eq!(fm.description.as_deref(), Some("Does \"x\": y and z"));
        assert!(new_skill(&t, "demo-skill", None, true).is_err());
        assert!(new_skill(&t, "Bad_Name", None, true).is_err());
        std::fs::remove_dir_all(&t).unwrap();
    }

    #[test]
    fn bundled_skill_follows_the_spec() {
        assert!(spec::validate("mimir", SKILL, &[]).ok(), "{:?}", spec::validate("mimir", SKILL, &[]).errors);
    }
}
