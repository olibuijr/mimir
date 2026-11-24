#!/usr/bin/env bash
# Install or upgrade the Mímir server for this user: binary in ~/.local/bin, a token in
# ~/.config/mimir/env (created once), and the systemd user service, then start it.
# Works from a release tarball (uses ./mimir) or from the repository (builds web/ and the binary).
set -euo pipefail
here=$(cd -- "$(dirname -- "$0")" && pwd)
if [[ -x "$here/mimir" ]]; then
  bin="$here/mimir"
else
  cd "$here/.."
  (cd web && npm ci --no-audit --no-fund && npm run build)
  cargo build --release --locked
  bin=target/release/mimir
fi
unit="$here/mimir.service"
install -Dm755 "$bin" ~/.local/bin/mimir
mkdir -p ~/.config/mimir
if ! grep -q '^MIMIR_TOKEN=.\{16,\}' ~/.config/mimir/env 2>/dev/null; then
  token=$(openssl rand -hex 24)
  # Replace a too-short token rather than add a second line: systemd uses the last, the CLI the first.
  if grep -q '^MIMIR_TOKEN=' ~/.config/mimir/env 2>/dev/null; then
    sed -i "s/^MIMIR_TOKEN=.*/MIMIR_TOKEN=$token/" ~/.config/mimir/env
  else
    (umask 077; printf 'MIMIR_TOKEN=%s\n' "$token" >> ~/.config/mimir/env)
  fi
  echo "wrote a new MIMIR_TOKEN to ~/.config/mimir/env (clients and the web UI need it)"
fi
chmod 600 ~/.config/mimir/env
install -Dm644 "$unit" ~/.config/systemd/user/mimir.service
systemctl --user daemon-reload
systemctl --user enable --now mimir.service
systemctl --user restart mimir.service
sleep 1
curl -fsS localhost:7440/health && echo
cat <<'EOF'
Mímir is running on :7440. It needs an embeddings and rerank API: set MIMIR_MODELS_URL
(and MIMIR_RERANK_URL if reranking is served elsewhere) in ~/.config/mimir/env, then
systemctl --user restart mimir. Check everything with: mimir doctor
To keep it running after logout: loginctl enable-linger "$USER"
EOF
