#!/usr/bin/env bash
# Build the static Linux binary and publish it as a GitHub release (no CI involved).
#
#   ./release.sh          build, package and upload release v<Cargo.toml version>
#   ./release.sh build    only build and package into dist/
#
# The binary is linked statically against musl with SQLite and the web UI compiled in, so it
# runs on any x86_64 Linux. The asset name has no version, so this link serves the newest:
#   https://github.com/olibuijr/mimir/releases/latest/download/mimir-x86_64-linux.tar.gz
# Needs: rustup target x86_64-unknown-linux-musl, musl-gcc, npm, gh (logged in).
set -euo pipefail
cd -- "$(dirname -- "$0")"
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
tag=v$version
target=x86_64-unknown-linux-musl
name=mimir-x86_64-linux

die() { echo "release: $*" >&2; exit 1; }

build() {
    grep -q "^## \[$version\]" CHANGELOG.md || die "CHANGELOG.md has no section for $version"
    (cd web && npm ci --no-audit --no-fund && npm run build)
    cargo build --release --locked --target $target
    bin=target/$target/release/mimir
    [[ "$("$bin" --version)" == "mimir $version" ]] || die "binary does not report $version"
    rm -rf "dist/$name" "dist/$name.tar.gz"
    mkdir -p "dist/$name"
    cp "$bin" deploy/install.sh deploy/client.sh deploy/mimir.service LICENSE README.md "dist/$name/"
    tar -C dist --owner=0 --group=0 -czf "dist/$name.tar.gz" "$name"
    (cd dist && sha256sum "$name.tar.gz" > "$name.tar.gz.sha256")
    echo "packaged dist/$name.tar.gz ($(du -h "dist/$name.tar.gz" | cut -f1))"
}

notes() {
    awk -v v="$version" '$0 ~ "^## \\[" v "\\]" {on=1; next} /^## \[/ {on=0} on' CHANGELOG.md
    cat <<EOF

## Install

\`\`\`sh
curl -LO https://github.com/olibuijr/mimir/releases/latest/download/$name.tar.gz
tar xzf $name.tar.gz && cd $name
./install.sh                                   # the server, as a systemd user service
MIMIR_URL=http://SERVER:7440 ./client.sh       # each machine with Claude Code or Codex
\`\`\`

A static x86_64 Linux binary (musl; SQLite and the web UI included). Check the download with $name.tar.gz.sha256.
EOF
}

publish() {
    [[ -z "$(git status --porcelain -- src web/src Cargo.toml Cargo.lock skills)" ]] || die "commit the source first"
    git push -q origin HEAD
    git rev-parse -q --verify "refs/tags/$tag" >/dev/null || git tag -a "$tag" -m "Mímir $version"
    git push -q origin "$tag"
    if gh release view "$tag" >/dev/null 2>&1; then
        gh release upload "$tag" "dist/$name.tar.gz" "dist/$name.tar.gz.sha256" --clobber
    else
        gh release create "$tag" "dist/$name.tar.gz" "dist/$name.tar.gz.sha256" --title "Mímir $version" --notes "$(notes)" --latest
    fi
    echo "released $tag: https://github.com/olibuijr/mimir/releases/tag/$tag"
}

case ${1:-release} in
    build) build ;;
    release) build; publish ;;
    *) die "unknown command $1; see the top of release.sh" ;;
esac
