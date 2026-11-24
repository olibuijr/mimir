mod agent;
mod api;
mod client;
mod db;
mod models;
mod server;
mod spec;
mod types;
mod web;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::time::Duration;

/// Mímir: a skill server. Skills live in one SQLite database; prompts find them by meaning.
#[derive(Parser)]
#[command(version, about)]
struct Cli {
    /// Machine-readable output for scripts and agents (list, search, validate, doctor, new; errors as {"error": ...})
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Run the HTTP server (needs MIMIR_TOKEN; models from MIMIR_MODELS_URL)
    Serve {
        #[arg(long, default_value = "0.0.0.0:7440")]
        bind: String,
        /// SQLite file (default $MIMIR_DB or ~/.local/share/mimir/mimir.db)
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Import Agent Skills directories (named after the skill, with SKILL.md; .env becomes secrets)
    Import {
        dirs: Vec<PathBuf>,
        /// Only show what would be imported
        #[arg(long)]
        dry_run: bool,
    },
    /// List all skills
    List,
    /// Check skills against the Agent Skills spec: given directories, or every skill on the server
    Validate { dirs: Vec<PathBuf> },
    /// Find skills for a request
    Search {
        query: Vec<String>,
        #[arg(short, default_value_t = 5)]
        k: usize,
        /// Minimum score (default 0: show everything ranked)
        #[arg(long, default_value_t = 0.0)]
        min: f32,
        /// Cosine only, skip the reranker
        #[arg(long)]
        no_rerank: bool,
    },
    /// Install a skill into ~/.cache/mimir/skills/<name> and print its SKILL.md
    Get { name: String },
    /// Delete a skill
    Rm { name: String },
    /// Recompute every skill embedding (after changing the embedding model)
    Reembed,
    /// Score the matcher on a labelled cases file (prompt<TAB>expected)
    Eval { cases: PathBuf },
    /// Write skills as plain Agent Skills directories DIR/<name>/ (all skills when no names are given)
    Export {
        dir: PathBuf,
        names: Vec<String>,
        /// Also write each skill's secrets as DIR/<name>/.env (mode 600)
        #[arg(long)]
        with_secrets: bool,
    },
    /// Check the server, token, hooks, MCP registration and stray file-level skills on this machine
    Doctor,
    /// Replace this binary with the server's when the server is newer
    Update {
        #[arg(long)]
        force: bool,
    },
    /// Claude Code / Codex UserPromptSubmit hook (reads the hook JSON on stdin)
    Hook,
    /// MCP server over stdio
    Mcp,
    /// Start a new skill: write DIR/NAME/SKILL.md with valid frontmatter and a body outline
    New {
        name: String,
        /// What the skill does and when to use it, with trigger words (prompts are matched against this)
        #[arg(long, short)]
        description: Option<String>,
        /// Parent directory (default: the current directory)
        #[arg(long, default_value = ".")]
        dir: PathBuf,
    },
    /// Print the agent skill for operating Mímir, or install it as DIR/mimir/SKILL.md
    Skill {
        #[arg(long, value_name = "DIR")]
        install: Option<PathBuf>,
    },
}

fn main() {
    // Exit quietly when the reader goes away (`mimir --json list | head`) instead of panicking.
    // SAFETY: restoring the default SIGPIPE action before any thread starts.
    unsafe { libc::signal(libc::SIGPIPE, libc::SIG_DFL) };
    if let Err(e) = run() {
        if std::env::args().any(|a| a == "--json") {
            println!("{}", serde_json::json!({ "error": format!("{e:#}") }));
        } else {
            eprintln!("Error: {e:#}");
        }
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    client::load_env();
    let cli = Cli::parse();
    let json = cli.json;
    let c = || client::Client::from_env();
    match cli.cmd {
        Cmd::Serve { bind, db } => {
            let db = db.or_else(|| std::env::var_os("MIMIR_DB").map(PathBuf::from)).unwrap_or_else(|| PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".local/share/mimir/mimir.db"));
            server::serve(&bind, &db)?;
        }
        Cmd::Import { dirs, dry_run } => {
            let c = if dry_run { None } else { Some(c()?) };
            let mut failed = 0;
            for d in &dirs {
                let s = match client::read_dir(d) {
                    Ok(s) => s,
                    Err(e) => { failed += 1; eprintln!("{}: {e:#}", d.display()); continue }
                };
                // Refuse locally with every reason, before anything is uploaded.
                let report = client::check(&s);
                if !report.ok() {
                    failed += 1;
                    eprintln!("{:<28} NOT AGENT SKILLS SPEC COMPLIANT:", s.name);
                    report.errors.iter().for_each(|e| eprintln!("    - {e}"));
                    continue;
                }
                let size: usize = client::files_of(&s).iter().map(|f| f.b64.len() * 3 / 4).sum();
                let sec = s.secrets.as_ref().map_or(0, |m| m.len());
                let warn = if report.warnings.is_empty() { String::new() } else { format!(", {} spec warning(s)", report.warnings.len()) };
                match &c {
                    None => println!("{:<28} ok  {} files, {} KB, {sec} secrets{warn}", s.name, client::files_of(&s).len(), size >> 10),
                    Some(c) => match c.put(&s) {
                        Ok(r) => println!("{:<28} v{}  {} files, {} KB, {sec} secrets{warn}", r.name, r.version, client::files_of(&s).len(), size >> 10),
                        Err(e) => { failed += 1; eprintln!("{:<28} FAILED: {e:#}", s.name) }
                    },
                }
            }
            anyhow::ensure!(failed == 0, "{failed} skill(s) failed");
        }
        Cmd::Validate { dirs } => {
            let failed = client::validate(&dirs, json)?;
            anyhow::ensure!(failed == 0, "{failed} skill(s) do not follow the Agent Skills spec");
        }
        Cmd::List if json => println!("{}", serde_json::to_string(&c()?.list()?)?),
        Cmd::List => {
            for s in c()?.list()? {
                println!("{:<28} v{:<3} {}", s.name, s.version, models::clip(&s.description, 110));
            }
        }
        Cmd::Search { query, k, min, no_rerank } => {
            let r = c()?.matches(
                &types::MatchReq { prompt: query.join(" "), k: Some(k), min: Some(min), rerank: Some(!no_rerank), log: Some(false), ..Default::default() },
                Duration::from_secs(30),
            )?;
            if json { println!("{}", serde_json::to_string(&r)?) } else { println!("{}\n({} ms)", client::fmt_hits(&r.hits), r.ms) }
        }
        Cmd::Get { name } => println!("{}", client::get_text(&c()?, &name)?),
        Cmd::Rm { name } => println!("{}", c()?.delete(&name)?),
        Cmd::Reembed => println!("{}", c()?.reembed()?),
        Cmd::Eval { cases } => client::eval(&c()?, &cases)?,
        Cmd::Export { dir, names, with_secrets } => {
            client::export(&c()?, &dir, &names, with_secrets)?;
        }
        Cmd::Doctor => {
            if !client::doctor(json) {
                std::process::exit(1);
            }
        }
        Cmd::Update { force } => client::update(&c()?, force)?,
        Cmd::Hook => client::hook()?,
        Cmd::Mcp => client::mcp()?,
        Cmd::New { name, description, dir } => agent::new_skill(&dir, &name, description.as_deref(), json)?,
        Cmd::Skill { install } => agent::skill(install.as_deref())?,
    }
    Ok(())
}
