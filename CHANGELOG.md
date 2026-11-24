# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Fixed
- `PUT /skills/{name}` stored file modes unmasked (e.g. setuid bits); the masked copies from the file check were thrown away. Modes are now stored masked to 0o777, as 0.4.0 intended.
- `mimir eval` panicked (index out of bounds) on a cases file with no cases; it now says so and exits 1.
- The web UI dev server (`npm run dev`) listened on every interface and added the real token to proxied requests, so anyone on the LAN had full access. It now listens on localhost; `MIMIR_DEV_HOST` opts into other hosts.
- `deploy/install.sh` appended a second `MIMIR_TOKEN` when `~/.config/mimir/env` held one shorter than 16 characters, so systemd (last line) and the CLI (first line) used different tokens. It now replaces the short one.
- The database right after creation and each new backup briefly had the default file mode, and the backups directory was 0755. Both files are now created 0600 and the directory 0700 (an existing one is tightened).
- `mimir get` and the MCP `skill_get` put the skill name into the URL unencoded and, with the server down, joined it onto the cache path unchecked (`../x` escaped the cache). Names are now checked against the spec's name rule first, with a clear error.
- Model server failures came back as HTTP 500, like a Mímir bug. They are now 502 when the model server answered with an error or an unusable response, 503 when it cannot be reached and 504 when it is too slow; messages name the call and the server, without credentials from its URL.
- The server's model timeouts (embedding 2 s, then reranking 3 s) could outlast the hook's 2.5 s, so a slow reranker meant no suggestions at all. A `/match` now has one deadline (`budget_ms` from the client, else `MIMIR_MATCH_BUDGET_MS`, default 2000 ms) shared by both calls; when the reranker does not fit, hits come back in embedding order with `"reranked": false` and the hook does not auto-load from them.

## [0.5.0] - 2026-11-24

First public release, under the MIT license.

### Added
- For scripts and agents: a global `--json` flag (`list`, `search`, `validate`, `doctor`, `new`), and with it errors print `{"error": "..."}` and exit 1. `doctor --json` returns `{ok, version, checks:[{level, message}], archive_commands}`.
- `mimir new NAME [-d DESCRIPTION]` starts a skill directory whose SKILL.md already follows the Agent Skills spec.
- `mimir skill [--install DIR]` prints or installs the agent skill for operating Mímir (`skills/mimir/SKILL.md`), compiled into the binary so it matches the installed version.
- `MIMIR_RERANK_URL`: reranking can come from a different server than embeddings (one llama-server per model); defaults to `MIMIR_MODELS_URL`.
- Releases: a static x86_64 Linux binary with SQLite and the web UI built in (`release.sh`), packaged with `install.sh` (server as a systemd user service, creates the token) and `client.sh`.

### Changed
- `deploy/client.sh` asks for the server URL instead of assuming one and installs the binary shipped next to it; `deploy/install.sh` installs a release binary or builds from source and creates `~/.config/mimir/env` with a random token.
- Piping output into `head` no longer panics.

## [0.4.2] - 2026-10-31

### Fixed
- Agents' `skill_search` MCP calls never reached the Activity page (they were sent with `log: false`). They are now logged and broadcast live, with the client prefixed `mcp:` (e.g. `mcp:claude@mac`) and an `mcp` tag on the log row. No schema change.

### Changed
- MCP searches (min 0, k 5) nearly always "hit", so they are kept out of the hook's stats: `matches`, `with_hit`, `hit_rate`, latency, `per_day`, `top`, `score_bands`, never-matched/stale and the Library's per-skill counts are hook-only. `/api/stats` adds `mcp_searches`, `per_day[].mcp` and `top[].mcp`; Activity shows an "MCP searches" badge and the Library's match badge a `+N mcp` note.

## [0.4.1] - 2026-10-30

### Fixed
- Open UI tabs kept running the old code after a redeploy (live updates mean the page is never reloaded). `/api/config` now reports a fingerprint of the embedded UI (`ui`); when the page reconnects after a restart and the fingerprint changed, it reloads itself.

## [0.4.0] - 2026-10-30

Built from a review by four Fable agents (server/security, matching/agents, UI, operations) and a
wave of six Sonnet coders; integrated, tested and measured by hand.

### Fixed
- **The hook stalled every prompt for ~30 s when the server was unreachable** (only the overall timeout was set, not the TCP connect). Now: 300 ms connect timeout, 2.5 s total, and a 30 s "server down" marker so later prompts skip the call (measured: 301 ms, then 1 ms).
- `GET /skills/{name}` returned secret values to the browser cookie; it is bearer-only now.
- Request bodies (up to 64 MB) were parsed before auth on every route; auth now runs first and only `PUT /skills/{name}` allows 64 MB (2 MB elsewhere). `MemoryMax` raised to 512M.
- `skill_save` with some `files` deleted all other files; files now merge (`remove_files` to delete), and a PUT without `files` keeps them.
- Invalid base64 in a PUT was a 500 (now 400 with the reason); file modes are masked.
- UI assets were cached across redeploys (ETag was the crate version); the ETag is now a content hash.
- "Never matched in 30 days" flooded 75 of 84 skills with warnings; it is a separate `stale` flag that needs 7 days of log.
- Embedding/rerank timeouts (60 s / 30 s) outlived the hook; now 2 s / 3 s for matching (batch embedding keeps 60 s).

### Added
- **Auto-load**: when the top match scores ≥ 0.9 (MIMIR_AUTOLOAD_MIN), the hook loads that skill and injects its SKILL.md directly, once per session; other matches stay suggestions. Measured on the eval set: fires on 89/140 prompts, right skill 84 (94%), never on prompts that need no skill; the 5 misses load a closely related skill while the right one is still suggested.
- Version history (20 per skill) with a Versions tab and line diff; daily + startup backups (7 kept); embedding-model tracking with a re-embed warning.
- `mimir export DIR`, `mimir doctor`, `mimir update` (self-update from the server), `deploy/client.sh` for new machines; the hook warns once a day when the client is older than the server.
- Offline fallback: `skill_get`/`mimir get` serve the cached copy when the server is unreachable; the cache is updated in place (no dangling symlinks) and skipped when unchanged.
- Log rows record the client (`claude@macbook`, `codex@server`) and the original prompt length; long prompts are clipped (first 700 + last 300 chars) for matching, storage and the UI.
- UI: Replay a logged prompt in the Playground, per-skill 30-day trend, relative SKILL.md links open the file, client chips, reconnect banner, backup age in the sidebar, phone layout and tooltip fixes, AA contrast, keyboard tree navigation; Library patches counts from live events instead of refetching.
- Eval: 140 cases (every skill covered, 35 that need no skill incl. follow-ups and look-alikes, long pasted logs); `a|-` labels scored correctly; misses show the expected skill's cosine rank.
- Experimental lexical rescue (`MIMIR_LEXICAL=1` / `lexical: true`), off by default.

### Changed
- **Default threshold 0.05 → 0.1.**

### Measured (eval/cases.tsv, 140 cases, A/B on the same server)
| | recall@1 | recall@3 | decisions right | false alarms | missed | p50 |
|---|---|---|---|---|---|---|
| min 0.05 (v0.3 default) | 0.92 | 0.99 | 0.89 | 5 / 35 | 3 | 126 ms |
| **min 0.1 (new default)** | 0.92 | 0.99 | **0.93** | **0 / 35** | 4 | 126 ms |
| min 0.1 + lexical rescue | 0.92 | 1.00 | 0.92 | 0 / 35 | 5 | 134 ms |

Lexical rescue is not worth turning on. Remaining misses are mostly near-duplicate `asc-*` skills that the reranker scores alike (skill-description overlap, not the matcher).

## [0.3.0] - 2026-10-30

### Added
- Agent Skills specification (agentskills.io/specification) enforced everywhere: `src/spec.rs` parses SKILL.md with a real YAML parser and checks the name rules (1-64 a-z/0-9/single hyphens, must equal the skill/directory name), required `description` (1-1024), the only allowed fields (`name`, `description`, `license`, `compatibility` ≤500, `metadata` string map, `allowed-tools`) and relative file paths. `PUT /skills/{name}`, `mimir import` and MCP `skill_save` refuse violations with every reason. Recommendations (body under 500 lines / ~5000 tokens, referenced `scripts/`/`references/`/`assets/` files present, relative paths) are warnings.
- `mimir validate [DIR...]`: checks local skill directories or every skill on the server.
- Web UI: Spec tab (verdict, errors, recommendations, frontmatter, spec directory layout), "not spec compliant" chips.
- Realtime UI: `GET /api/events` (server-sent events: match, load, skill put/delete/reembed, log_cleared, resync). Library, Skill detail, Activity and the sidebar update as things happen; the 10 s poll is gone. A "Live" indicator shows the connection.

### Changed
- The description is always taken from the SKILL.md frontmatter (single source of truth).
- A load is recorded only for real use (`X-Mimir-Load: 1`, sent by MCP `skill_get` and `mimir get`); scripts reading skills no longer inflate load stats.
- `mimir import` reads the skill name from the directory name, as the spec requires; `--name` is gone.
- Five imported skills rewritten to be compliant without losing data (extra fields moved under `metadata`, nested metadata flattened, a list `compatibility` joined, an unquoted description quoted): avoid-ai-writing, hermes-agent, iphone-control, ponytail, zig-0-16.

### Measured
- Validator agreement with the reference `skills-ref validate` on all 84 skills: identical verdicts (same 5 failures before the fix, 0 after). Like skills-ref, plain YAML scalars (`170`, `true`) count as strings; Mímir is stricter on nested metadata maps, which the spec defines as string to string.
- Live latency in headless Chromium: new match row on Activity 111 ms after the hook ran, Skill detail 421 ms, Library 613 ms (debounced refetch); no reloads, no console errors.

## [0.2.0] - 2026-10-30

### Added
- Web UI (Svelte 5, Liquid Glass, dark) embedded in the binary: library, skill detail, playground, activity.
- `/api` routes for config, skills, skill detail, files, stats and the match log; `DELETE /api/log`.
- Cookie login (`POST /api/login`, `/api/logout`): HttpOnly `mimir` cookie; reads accept bearer or cookie, `PUT`/`DELETE /skills/{name}` and `/reembed` stay bearer-only.
- `load_log` table: `GET /skills/{name}` records that a skill was loaded; log indexes on `ts`.
- Retention: `match_log` and `load_log` rows older than 30 days are pruned at startup and every 24 h.
- `build.rs` copies `web/dist` into the binary (stubs if the UI is not built); `deploy/install.sh` builds the UI first.

### Fixed
- A panic while holding the database mutex no longer poisons it for every later request.
- Token comparison is constant-time.

### Fixed (integration)
- Hook ignores background-task notifications (they were matched like user prompts).
- Score bands and threshold no longer serialize as long floats; plural chips.
- web.rs borrow error; Svelte a11y warnings (list roles, Menu state).

### Measured
- `mimir eval` A/B, 0.1.0 vs 0.2.0 interleaved x2: identical recall@1 0.96, decide-acc 0.91 at min 0.05, p50 141-142 ms / p95 160-161 ms. Bundle 45 kB gzip (3 files).

## [0.1.0] - 2026-10-30

### Added
- `mimir serve`: HTTP API (:7440, bearer token) storing skills, their files and secrets in SQLite.
- Matching: EmbeddingGemma cosine shortlist (12) judged by the Qwen3 reranker with a task instruction, both served by llama.cpp.
- CLI: `import`, `list`, `search`, `get`, `rm`, `reembed`, `eval`.
- `mimir hook`: Claude Code `UserPromptSubmit` hook that injects the matching skills.
- `mimir mcp`: stdio MCP server with `skill_search`, `skill_get`, `skill_list`, `skill_save`; `skill_get` installs files and `.env` into `~/.cache/mimir/skills/<name>`.
- Match log table for later tuning.
- `eval/cases.tsv`: 64 labelled prompts (55 with a skill, 9 without) over 38 skills.

### Measured (private eval set, llama.cpp models on an RTX 3080)
| Matcher | recall@1 | recall@3 | decisions right at chosen min | false alarms | p50 latency |
|---|---|---|---|---|---|
| EmbeddingGemma cosine only (best min 0.6) | 0.85 | 0.98 | 0.77 | 7 / 9 | 6 ms |
| + Qwen3 reranker, no instruction (min 0.05) | 0.96 | 0.98 | 0.89 | 0 / 9 | 133 ms |
| + Qwen3 reranker with task instruction (min 0.05, default) | 0.96 | 0.98 | 0.91 | 0 / 9 | 141 ms |
