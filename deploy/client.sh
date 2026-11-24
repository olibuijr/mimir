#!/usr/bin/env bash
# Set up a Mímir client on this machine: binary, ~/.config/mimir/env, Claude + Codex hook and MCP, then `mimir doctor`.
# Safe to run again. Needs jq and curl. Set MIMIR_URL to the server, e.g.
#   MIMIR_URL=http://server.lan:7440 ./client.sh
# (asked for when not set; MIMIR_TOKEN is asked for too unless already in the environment).
set -euo pipefail
URL="${MIMIR_URL:-}"
if [ -z "$URL" ] && ! grep -q '^MIMIR_URL=' "$HOME/.config/mimir/env" 2>/dev/null; then
  read -rp "Mímir server URL (e.g. http://server.lan:7440): " URL </dev/tty
fi
URL="${URL:-$(sed -n 's/^MIMIR_URL=//p' "$HOME/.config/mimir/env" 2>/dev/null | head -1)}"
BIN="$HOME/.local/bin/mimir"
ENVF="$HOME/.config/mimir/env"
command -v jq >/dev/null || { echo "jq is required" >&2; exit 1; }

# 1. config (token is asked for once, file is private)
mkdir -p "$(dirname "$ENVF")"
if ! grep -q '^MIMIR_TOKEN=.\+' "$ENVF" 2>/dev/null; then
  if [ -n "${MIMIR_TOKEN:-}" ]; then token="$MIMIR_TOKEN"; else read -rsp "MIMIR_TOKEN: " token </dev/tty; echo; fi
  (umask 077; printf 'MIMIR_URL=%s\nMIMIR_TOKEN=%s\n' "$URL" "$token" > "$ENVF")
fi
grep -q '^MIMIR_URL=' "$ENVF" || printf 'MIMIR_URL=%s\n' "$URL" >> "$ENVF"
chmod 600 "$ENVF"

# 2. binary: self-update when present, else the copy next to this script, else download from the server
mkdir -p "$(dirname "$BIN")"
here=$(cd -- "$(dirname -- "$0")" && pwd)
if [ -x "$BIN" ] && "$BIN" update; then
  :
elif [ -x "$here/mimir" ]; then
  install -m755 "$here/mimir" "$BIN"
else
  set -a; . "$ENVF"; set +a
  curl -fsS -H "Authorization: Bearer $MIMIR_TOKEN" "$MIMIR_URL/api/dist/mimir" -o "$BIN.new"
  chmod 755 "$BIN.new"; mv "$BIN.new" "$BIN"
fi
echo "mimir $("$BIN" --version | awk '{print $2}') at $BIN"

# 3. UserPromptSubmit hook, same JSON shape for Claude Code and Codex
add_hook() {
  local f="$1"
  mkdir -p "$(dirname "$f")"
  [ -s "$f" ] || echo '{}' > "$f"
  if jq -e '[.hooks.UserPromptSubmit[]?.hooks[]?.command // empty | select(test("mimir hook"))] | length > 0' "$f" >/dev/null; then
    echo "hook already in $f"
  else
    jq --arg c "$BIN hook" '.hooks //= {} | .hooks.UserPromptSubmit //= [] | .hooks.UserPromptSubmit += [{"hooks":[{"type":"command","command":$c,"timeout":10}]}]' "$f" > "$f.tmp"
    mv "$f.tmp" "$f"
    echo "hook added to $f"
  fi
}
add_hook "$HOME/.claude/settings.json"
add_hook "$HOME/.codex/hooks.json"

# 4. MCP servers
if command -v claude >/dev/null; then
  claude mcp get mimir >/dev/null 2>&1 || claude mcp add -s user mimir -- "$BIN" mcp
else
  echo "claude not installed: skipping its MCP registration"
fi
if command -v codex >/dev/null; then
  codex mcp get mimir >/dev/null 2>&1 || codex mcp add mimir -- "$BIN" mcp
else
  echo "codex not installed: skipping its MCP registration"
fi

echo
"$BIN" doctor
