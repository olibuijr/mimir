---
name: mimir
description: Use Mímir, a self-hosted skill server for AI coding agents, to find, load, create, update and validate Agent Skills, and to set up, upgrade or troubleshoot the server and its clients. Use when a task may have a skill, when a prompt hook lists "Mímir matched skills", when asked to save, write, import, export or fix a skill, to make a skill follow the Agent Skills spec, or when skill matching, the hook, the MCP server or the models behave wrongly. Triggers - mimir, skill server, skill_search, skill_get, skill_save, SKILL.md, agent skill, import skill, matched skills, mimir doctor, mimir update, reembed, skill eval.
license: MIT
compatibility: The mimir binary on PATH with MIMIR_URL and MIMIR_TOKEN in ~/.config/mimir/env (set up by deploy/client.sh), or the mimir MCP server registered with the agent.
metadata:
  project: https://github.com/olibuijr/mimir
  website: https://mimir.olibuijr.com
---

# Mímir

Mímir stores every skill (SKILL.md, scripts, reference files and secrets) in one
server and matches prompts to skills by meaning. Skills live there, not in
`~/.claude/skills` or other skill folders. Every command takes `--json`; errors
then print `{"error": "..."}` and exit 1. `mimir skill` prints this file.

## 1. Find and load a skill

A prompt hook may already have injected "Mímir matched skills" (and sometimes a
whole loaded skill). Load each listed skill that fits the task **before acting**.
Otherwise search with what you are trying to do:

| With the MCP server | From a shell |
| --- | --- |
| `skill_search(query)` | `mimir --json search WORDS -k 5` |
| `skill_get(name)` | `mimir get NAME` |
| `skill_list()` | `mimir --json list` |

Loading installs the skill's files and `.env` into `~/.cache/mimir/skills/NAME/`.
Run its scripts from there with the paths it reports. When the server is down,
`get` serves the cached copy and says so. Scores are reranker judgements from 0 to
1; above about 0.1 the skill is usually relevant, above 0.9 almost always.

## 2. Create or update a skill

Save what you learned by hand (probe commands, deploy steps, gotchas) into a skill
in the same session, so the next one does not rediscover it.

```sh
mimir new my-skill -d "What it does and when to use it, with trigger words"
# edit my-skill/SKILL.md, add scripts/, references/, assets/
mimir validate my-skill
mimir import my-skill          # creates it, or bumps the version
```

From an agent, `skill_save(name, body, files?, remove_files?, executable?, secrets?)`
does the same. `body` is the full SKILL.md. `files` is merged into the existing files.
Omit `secrets` to keep the current ones. To change an existing skill, `skill_get` it
first and edit that text; never rewrite from memory. Version history keeps 20
versions per skill.

Rules the server enforces (the Agent Skills spec, agentskills.io/specification):

- Frontmatter has only `name`, `description`, `license`, `compatibility`,
  `metadata` (string to string) and `allowed-tools`. Move anything else under `metadata`.
- `name` equals the directory name: 1-64 lowercase letters, digits and single hyphens.
- `description` is 1-1024 characters and is what prompts are matched against.
  Name the tasks, tools, services and words a user would type. Quote it if it
  contains `: `.
- Body under 500 lines. Refer to the skill's own files by paths relative to its root.

Secrets go in `.env` (on import) or `secrets` (on save). Never print secret values
or paste them into SKILL.md.

## 3. Check and fix a machine

```sh
mimir --json doctor     # server, version, token, models, backups, hooks, MCP, stray skill folders
mimir update            # replace this binary with the server's when that is newer
```

`doctor` exits 1 when a check fails. Read each `fail` message; it names the fix.
Stray file-level skills should be imported (`mimir import DIR...`) and then archived
with the `archive_commands` it prints; do not delete them. A new machine is set up
with `deploy/client.sh` from the repository. It installs the binary, writes the
env file, and adds the hook and MCP server for Claude Code and Codex.

## 4. Run the server

```sh
mimir serve --bind 0.0.0.0:7440      # needs MIMIR_TOKEN (16+ chars)
```

It needs an OpenAI-style embeddings and rerank API. `MIMIR_MODELS_URL` serves
`/v1/embeddings`; `MIMIR_RERANK_URL` serves `/v1/rerank` and defaults to the same
URL. The defaults expect EmbeddingGemma and Qwen3-Reranker-0.6B (`MIMIR_EMBED_MODEL`,
`MIMIR_RERANK_MODEL`). After changing the embedding model run `mimir reembed`.
Change matching settings only with an A/B from `mimir eval CASES.tsv`
(`prompt<TAB>expected-skill`, `-` for none).

The database is `~/.local/share/mimir/mimir.db`, with daily backups in
`backups/` beside it (7 kept). To restore, stop the service, copy a backup over the
database, and start it again. The web UI is at the server's URL; sign in with the token.

## 5. Working on Mímir itself

Source: https://github.com/olibuijr/mimir. Run `cargo test` and `(cd web && npm run build)`
before a pull request. Keep `Cargo.toml`'s version and `CHANGELOG.md` in step, and
record `mimir eval` numbers in the changelog for any matching change.
