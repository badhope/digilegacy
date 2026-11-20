#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
if ! command -v cargo >/dev/null 2>&1 && [[ -f "$HOME/.cargo/env" ]]; then
  # Non-login shells used by CI do not always load the Rust environment.
  # shellcheck disable=SC1091
  . "$HOME/.cargo/env"
fi
if ! command -v cargo >/dev/null 2>&1; then
  echo "cargo is required; install Rust or source \$HOME/.cargo/env" >&2
  exit 2
fi
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT/Cargo.toml" | head -1)
ARCH=$(uname -m)
case "$ARCH" in
  x86_64|aarch64) ;;
  *) echo "unsupported architecture: $ARCH" >&2; exit 2 ;;
esac

STAGE="$ROOT/build/package/digilegacy-$VERSION"
OUT="$ROOT/packages"
ARCHIVE="$OUT/digilegacy-$VERSION-linux-$ARCH.tar.gz"

rm -rf "$STAGE"
install -d -m 0755 "$STAGE/bin" "$STAGE/web" "$STAGE/content" "$STAGE/docs" "$STAGE/share/nginx" "$STAGE/tools/policies"
install -d -m 0755 "$OUT"

echo "building release binaries for $VERSION ($ARCH)"
cargo build --release -p digilegacy-server -p digilegacy-cli -p digilegacy-mcp

if [[ ! -d "$ROOT/web/node_modules" ]]; then
  echo "installing Web dependencies"
  (cd "$ROOT/web" && npm ci)
fi
echo "building Web console"
(cd "$ROOT/web" && npm run build)

install -m 0755 "$ROOT/target/release/digilegacy-server" "$STAGE/bin/digilegacy-server"
install -m 0755 "$ROOT/target/release/digilegacy-cli" "$STAGE/bin/digilegacy-cli"
install -m 0755 "$ROOT/target/release/digilegacy-mcp" "$STAGE/bin/digilegacy-mcp"
cp -R "$ROOT/web/dist/." "$STAGE/web/"
cp -R "$ROOT/content/." "$STAGE/content/"
cp -R "$ROOT/packaging/nginx/." "$STAGE/share/nginx/"
install -m 0755 "$ROOT/tools/policies/source_audit.py" "$STAGE/tools/policies/source_audit.py"
install -m 0755 "$ROOT/tools/local_model_server.py" "$STAGE/tools/local_model_server.py"
find "$STAGE/tools" -type d -name __pycache__ -prune -exec rm -rf {} +
cp -R "$ROOT/docs/operations" "$STAGE/docs/operations"
# The install drill evidence describes the archive produced from this tree;
# embedding a pre-build copy would make the archive cite a different SHA.
rm -f "$STAGE/docs/operations/measurements/install-2026-11-20.json"
install -d -m 0755 "$STAGE/docs/zh-CN"
cp -R "$ROOT/docs/zh-CN/operations" "$STAGE/docs/zh-CN/operations"
install -m 0644 "$ROOT/packaging/README.md" "$STAGE/README.md"
install -m 0644 "$ROOT/CHANGELOG.md" "$STAGE/CHANGELOG.md"
install -m 0644 "$ROOT/docs/ARCHITECTURE.md" "$STAGE/docs/ARCHITECTURE.md"
install -m 0644 "$ROOT/docs/VERIFICATION.md" "$STAGE/docs/VERIFICATION.md"
install -m 0644 "$ROOT/SECURITY.md" "$STAGE/SECURITY.md"
if [[ -f "$ROOT/LICENSE" ]]; then
  install -m 0644 "$ROOT/LICENSE" "$STAGE/LICENSE"
fi
install -m 0755 "$ROOT/packaging/install.sh" "$STAGE/install.sh"
install -m 0755 "$ROOT/packaging/uninstall.sh" "$STAGE/uninstall.sh"
install -m 0755 "$ROOT/packaging/start.sh" "$STAGE/start.sh"
install -m 0755 "$ROOT/packaging/stop.sh" "$STAGE/stop.sh"
printf '%s\n' "$VERSION" > "$STAGE/VERSION"

(
  cd "$STAGE"
  find . -type f ! -name SHA256SUMS -print0 | sort -z | xargs -0 sha256sum > SHA256SUMS
)
tar -C "$(dirname "$STAGE")" -czf "$ARCHIVE" "$(basename "$STAGE")"
(
  cd "$OUT"
  sha256sum "$(basename "$ARCHIVE")" > "$(basename "$ARCHIVE").sha256"
)

echo "archive: $ARCHIVE"
echo "checksum: $ARCHIVE.sha256"
du -h "$ARCHIVE" | awk '{print "size: "$1}'
