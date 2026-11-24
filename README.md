<img src="site/favicon.svg" alt="" width="72" height="72">

# Mímir

[![CI](https://github.com/olibuijr/mimir/actions/workflows/ci.yml/badge.svg)](https://github.com/olibuijr/mimir/actions/workflows/ci.yml)
[![Version](https://img.shields.io/badge/version-0.5.0-6b5bd2)](CHANGELOG.md)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

**A self-hosted skill server for AI coding agents.**

[Website](https://mimir.olibuijr.com/) · [Install](#install) · [Contributing](CONTRIBUTING.md)

Mímir keeps every [Agent Skill](https://agentskills.io/specification) (SKILL.md,
scripts, reference files and secrets) in one SQLite database on a server you run,
instead of a `~/.claude/skills` folder copied to every machine. Each prompt is
matched to skills by meaning with an embedding model and a reranker, and the
right skill is handed to Claude Code or Codex before it starts work.

```
prompt ──mimir hook──▶ server :7440 /match
                         EmbeddingGemma → cosine top 12
                         Qwen3-reranker (task instruction) → score ≥ min
                 ◀── "Mímir matched skills: unifi (0.97) …"   (injected as context)
agent ──mimir mcp──▶ skill_get("unifi") → SKILL.md + files + .env in ~/.cache/mimir/skills/unifi/
```

One Rust binary does everything: the server with its web UI, and the CLI, prompt
hook and MCP server on each machine.

- **One library:** skills are versioned (20 kept), backed up daily, and checked
  against the Agent Skills spec on every save.
- **Found by meaning:** a prompt like "the gateway drops port 443" finds the
  router skill without naming it. On a 140-prompt eval set the right skill is the
  top hit 89% of the time (97% in the top three), and prompts that need no skill get none.
- **Never in the way:** the hook gives up after 2.5 s and stays silent when the
  server is down; loaded skills are cached for offline use.
- **Web UI:** library, skill detail with files, versions and diffs, a playground
  to see why a prompt did or did not match, and live activity.

## Install

Download the static x86_64 Linux binary (SQLite and the web UI included).

On the server:

```sh
curl -LO https://github.com/olibuijr/mimir/releases/latest/download/mimir-x86_64-linux.tar.gz
tar xzf mimir-x86_64-linux.tar.gz && cd mimir-x86_64-linux
./install.sh        # ~/.local/bin/mimir, a random token in ~/.config/mimir/env, systemd user service on :7440
```

On every machine with Claude Code or Codex (needs `jq`; asks for the token):

```sh
MIMIR_URL=http://SERVER:7440 ./client.sh
```

It installs the binary and writes `~/.config/mimir/env` (`MIMIR_URL`, `MIMIR_TOKEN`,
mode 600). It adds the `UserPromptSubmit` hook to `~/.claude/settings.json` and
`~/.codex/hooks.json`, registers the MCP server with Claude and Codex, and runs
`mimir doctor`. It is safe to run again. Then move existing skill folders in with
`mimir import ~/.claude/skills/*`.

### Models

Matching needs an OpenAI-style API with `/v1/embeddings` and `/v1/rerank`, such
as [llama.cpp](https://github.com/ggml-org/llama.cpp)'s `llama-server`. Both
models are small and run on a CPU or a modest GPU:

```sh
llama-server -hf ggml-org/embeddinggemma-300M-GGUF --embeddings --port 8084
llama-server -hf ggml-org/Qwen3-Reranker-0.6B-Q8_0-GGUF --reranking --port 8085
```

Then set `MIMIR_MODELS_URL=http://127.0.0.1:8084` and `MIMIR_RERANK_URL=http://127.0.0.1:8085`
in `~/.config/mimir/env` and run `systemctl --user restart mimir`. Set
`MIMIR_EMBED_MODEL` and `MIMIR_RERANK_MODEL` to the names your server reports
at `/v1/models`. `MIMIR_DB` moves the database (default `~/.local/share/mimir/mimir.db`).
After changing the embedding model run `mimir reembed`.

### Build from source

Rust 1.88+ and Node 20+:

```sh
git clone https://github.com/olibuijr/mimir.git && cd mimir
deploy/install.sh    # builds web/ and the binary, then installs as above
```

By hand, the hook is

```json
"hooks": { "UserPromptSubmit": [ { "hooks": [ { "type": "command", "command": "mimir hook", "timeout": 10 } ] } ] }
```

and the MCP server is `claude mcp add -s user mimir -- ~/.local/bin/mimir mcp` (Codex: `codex mcp add mimir -- ...`).

## Use with AI agents

Agents use Mímir through the MCP tools `skill_search`, `skill_get`, `skill_list`
and `skill_save`, or through the CLI. Every command answers in JSON with `--json`.
The binary also carries a skill ([skills/mimir/SKILL.md](skills/mimir/SKILL.md))
that teaches an agent to find and load skills, write new ones that pass the spec,
and fix a broken setup.

```sh
mimir import skills/mimir                    # put the Mímir skill in your library
mimir --json search "deploy the app to the VM" -k 3
mimir new release-flow -d "How we cut a release: ..."   # a spec-valid skill to fill in
mimir --json validate release-flow && mimir import release-flow
mimir --json doctor                          # every check with ok / warn / fail
```

## Hook behaviour

- Never stalls a prompt: 300 ms connect timeout, 2.5 s overall (`MIMIR_HOOK_TIMEOUT_MS`). When the server is
  unreachable the hook writes `~/.cache/mimir/down` and skips the call for 30 s. It is silent on any failure.
- Fits the server's work into that: each `/match` carries a `budget_ms` (the client's timeout minus 500 ms,
  so 2000 ms for the hook; `MIMIR_MATCH_BUDGET_MS` on the server for callers that send none, default 2000).
  The embedding call (at most `MIMIR_EMBED_TIMEOUT_MS`, 2000) and the rerank call (at most
  `MIMIR_RERANK_TIMEOUT_MS`, 3000) share that deadline. When less than 250 ms is left for the reranker, or it
  runs out of time, the hits come back in embedding order with cosine scores and `"reranked": false`, and
  the hook auto-loads nothing.
- Sends `X-Mimir-Client: <claude|codex|cli>@<host>` so the activity log shows which agent on which machine asked.
- Auto-loads: suggestions alone were never acted on, so when the top hit scores at least `MIMIR_AUTOLOAD_MIN`
  (default 0.9) and was not already loaded in this session (`session_id`, remembered in `~/.cache/mimir/sessions/`),
  the hook loads it, installs its files and injects its SKILL.md (frontmatter stripped, capped at 24,000 chars).
  The other hits are listed as suggestions.
- Tells you once a day when the server is newer than the client: run `mimir update`.

## Agent Skills spec

Every skill is an [Agent Skills](https://agentskills.io/specification) directory: `<name>/SKILL.md` with YAML frontmatter (`name` = directory name, `description` ≤1024 chars, optional `license`, `compatibility`, `metadata`, `allowed-tools`) plus optional `scripts/`, `references/`, `assets/`. Mímir refuses anything that breaks the spec (server, `import`, MCP `skill_save`) and reports recommendations as warnings. `mimir get` writes the same layout to `~/.cache/mimir/skills/<name>/`.

## CLI

| Command | Does |
|---|---|
| `mimir import DIR...` | import skill dirs (`SKILL.md` + files; `.env` becomes secrets). Re-import bumps the version |
| `mimir list` | all skills |
| `mimir new NAME [-d TEXT]` | start `NAME/SKILL.md` with spec-valid frontmatter and a body outline |
| `mimir skill [--install DIR]` | print the agent skill for operating Mímir, or write it to `DIR/mimir/SKILL.md` |
| `mimir validate [DIR...]` | check skill directories, or every skill on the server, against the Agent Skills spec |
| `mimir search WORDS` | ranked matches with reranker and cosine scores (`--no-rerank`, `--min`, `-k`) |
| `mimir get NAME` | install into `~/.cache/mimir/skills/NAME` and print SKILL.md; uses the cached copy when the server is unreachable. Files are updated in place, never wiped, so symlinks into the cache keep working |
| `mimir export DIR [NAME...]` | write skills as plain Agent Skills directories `DIR/NAME/` (exact SKILL.md, files with modes). `.env` only with `--with-secrets` (mode 600) |
| `mimir doctor` | check server, version, token, models, embeddings, backup age, hooks and MCP registration for Claude and Codex, and stray file-level skills; prints the `mv` command to archive them; exit 1 on any failure |
| `mimir update [--force]` | replace this binary with the server's when that is newer |
| `mimir rm NAME` | delete |
| `mimir reembed` | recompute all embeddings |
| `mimir eval CASES.tsv` | recall@1/@3 and latency (embed-only and reranked), the threshold table (reranked), and for every miss the expected skill's cosine rank. A label `a\|-` accepts skill `a` or no suggestion. `MIMIR_EVAL_LEXICAL=1` sends `lexical:true` |

`--json` works with `list`, `search`, `validate`, `doctor` and `new`; with it, errors print `{"error": "..."}` and exit 1.

Paths written as `~/.claude/skills/NAME` or `${CLAUDE_SKILL_DIR}` inside a skill are rewritten to the
cache dir when it is installed.

## API

Every route except `/health`, the UI assets, `POST /api/login` and `POST /api/logout` is authenticated **before the request
body is read** (401 otherwise). `/skills/*`, `/reembed` and `/api/dist/mimir` accept only
`Authorization: Bearer $MIMIR_TOKEN` (they return or change secrets); `/api/*` and `/match` also accept
the UI cookie. Bodies are limited to 2 MB, except `PUT /skills/{name}` (64 MB). Every response carries
`X-Mimir-Version`.

| Route | |
|---|---|
| `GET /health` | `{ok, version}` |
| `GET /skills` | `[{name, description, version}]` |
| `GET/PUT/DELETE /skills/{name}` | full skill: `{description, body, files:[{path, mode, b64}], secrets?}`. On PUT, omitted `files`/`secrets` are kept; the body must follow the Agent Skills spec. `X-Mimir-Load: 1` on GET records a real use |
| `POST /match` | `{prompt, k?, min?, rerank?, lexical?, log?}` → `{hits:[{name, description, score, cosine}], ms}`; optional `X-Mimir-Client` header. Prompts over 1,000 chars are clipped (first 700 + last 300) |
| `POST /reembed` | recompute embeddings |
| `GET /api/skills/{name}/history[/{version}]` | previous versions (20 kept) |
| `GET /api/dist/mimir` | this server's binary, for `mimir update` |
| `GET /api/events` | server-sent events for the UI |

Backups: the server writes `backups/mimir-YYYYMMDD-HHMMSS.db` next to the database at startup and daily
(`VACUUM INTO`, 7 kept). Restore = stop the service, copy a backup over `mimir.db`, start it.

## Web UI

Open `http://SERVER:7440/`. The first visit shows a login sheet; paste `MIMIR_TOKEN` and it is
stored as an HttpOnly, SameSite=Strict cookie (30 days). The cookie authorises reads and the UI's own
routes only; `PUT`/`DELETE /skills/{name}` and `/reembed` still need the bearer token. Secret values
are never sent to the browser. The UI (Svelte 5, `web/`) is built to `web/dist` and embedded in the
binary by `build.rs`; without it the server serves a "UI not built" stub.

Dev: `cd web && npm run dev` serves on :7441 and proxies `/api` and `/match` to `MIMIR_URL` (default `http://127.0.0.1:7440`) with the token
from `~/.config/mimir/env`. Because it adds the token, it listens on localhost only; `MIMIR_DEV_HOST=0.0.0.0 npm run dev`
opts into serving the LAN (anyone who can reach :7441 then has full access).

Extra routes (cookie or bearer): `GET /api/config`,
`GET /api/skills`, `GET /api/skills/{name}`, `GET /api/skills/{name}/files/{path}`,
`GET /api/stats?since=`, `GET /api/log?skill=&nohit=1&q=&before=&limit=`,
`DELETE /api/log?before=` (needs header `X-Mimir: 1`). Log rows are pruned after 30 days.

## Development

`cargo test` for the unit tests. Change matching only with an A/B from `mimir eval`
on a labelled cases file of your own (`prompt<TAB>expected`; `eval/` is ignored by
git), and record the numbers in the changelog. Maintainers publish a release with
`./release.sh`, which builds the static binary and uploads it with `gh`.

Mímir is free and open source: use it, fork it, change it and ship it. See
[CONTRIBUTING.md](CONTRIBUTING.md).

[Changelog](CHANGELOG.md) · [Agent skill](skills/mimir/SKILL.md) · [MIT license](LICENSE)
