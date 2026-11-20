#!/usr/bin/env bash
set -euo pipefail
umask 077

usage() {
  cat <<'EOF'
Install DigiLegacy from an unpacked release tarball.

Usage: ./install.sh [options]

  --prefix <dir>    Install directory (default: /opt/digilegacy)
  --data-dir <dir>  Database, keys and checkpoints (default: <prefix>/data)
  --bind <addr>     API bind address (default: 127.0.0.1:8787)
  --token <token>   Platform write token; generated when omitted
  --help            Show this help

The installer keeps an existing etc/digilegacy.env on upgrade. It never prints
the write token, and it creates key files with mode 0600 on first health check.
EOF
}

PREFIX=/opt/digilegacy
DATA_DIR=
BIND=127.0.0.1:8787
TOKEN=

while [[ $# -gt 0 ]]; do
  case "$1" in
    --prefix)
      PREFIX=${2:?--prefix requires a value}
      shift 2
      ;;
    --data-dir)
      DATA_DIR=${2:?--data-dir requires a value}
      shift 2
      ;;
    --bind)
      BIND=${2:?--bind requires a value}
      shift 2
      ;;
    --token)
      TOKEN=${2:?--token requires a value}
      shift 2
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      echo "unknown option: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

PREFIX=${PREFIX%/}
[[ "$PREFIX" == /* && "$PREFIX" != "/" ]] || {
  echo "prefix must be an absolute path other than /" >&2
  exit 2
}
DATA_DIR=${DATA_DIR:-$PREFIX/data}
DATA_DIR=${DATA_DIR%/}
[[ "$DATA_DIR" == /* && "$DATA_DIR" != "/" ]] || {
  echo "data directory must be an absolute path other than /" >&2
  exit 2
}
[[ "$BIND" == *:* ]] || {
  echo "bind must be host:port" >&2
  exit 2
}

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
for required in \
  "$ROOT/bin/digilegacy-server" \
  "$ROOT/bin/digilegacy-cli" \
  "$ROOT/bin/digilegacy-mcp" \
  "$ROOT/web/index.html" \
  "$ROOT/start.sh" \
  "$ROOT/stop.sh" \
  "$ROOT/uninstall.sh"; do
  [[ -e "$required" ]] || {
    echo "release archive is incomplete: missing $required" >&2
    exit 1
  }
done

install -d -m 0755 "$PREFIX" "$PREFIX/bin" "$PREFIX/web" "$PREFIX/content" "$PREFIX/docs" "$PREFIX/tools" "$PREFIX/etc" "$PREFIX/run" "$PREFIX/log"
install -d -m 0700 "$DATA_DIR" "$DATA_DIR/keys"

install -m 0755 "$ROOT/bin/digilegacy-server" "$PREFIX/bin/digilegacy-server"
install -m 0755 "$ROOT/bin/digilegacy-cli" "$PREFIX/bin/digilegacy-cli"
install -m 0755 "$ROOT/bin/digilegacy-mcp" "$PREFIX/bin/digilegacy-mcp"
install -m 0755 "$ROOT/start.sh" "$PREFIX/bin/digilegacy-start"
install -m 0755 "$ROOT/stop.sh" "$PREFIX/bin/digilegacy-stop"
install -m 0755 "$ROOT/uninstall.sh" "$PREFIX/bin/digilegacy-uninstall"
cp -R "$ROOT/web/." "$PREFIX/web/"
cp -R "$ROOT/content/." "$PREFIX/content/"
cp -R "$ROOT/docs/." "$PREFIX/docs/"
if [[ -d "$ROOT/tools" ]]; then
  cp -R "$ROOT/tools/." "$PREFIX/tools/"
fi
install -m 0644 "$ROOT/README.md" "$PREFIX/README.md"
install -m 0644 "$ROOT/VERSION" "$PREFIX/VERSION"
if [[ -f "$ROOT/SECURITY.md" ]]; then
  install -m 0644 "$ROOT/SECURITY.md" "$PREFIX/SECURITY.md"
fi
if [[ -f "$ROOT/LICENSE" ]]; then
  install -m 0644 "$ROOT/LICENSE" "$PREFIX/LICENSE"
fi

# Uninstall --keep-data copies keys next to the database. Restore them before
# the first health check so a reinstall can read the preserved database.
for key in db.key config.key; do
  if [[ -f "$DATA_DIR/keys/$key" && ! -f "$PREFIX/etc/$key" ]]; then
    install -m 0600 "$DATA_DIR/keys/$key" "$PREFIX/etc/$key"
  fi
done

ENV_FILE="$PREFIX/etc/digilegacy.env"
if [[ -f "$ENV_FILE" ]]; then
  echo "keeping existing $ENV_FILE"
else
  if [[ -z "$TOKEN" ]]; then
    TOKEN=$(od -An -N32 -tx1 /dev/urandom | tr -d ' \n')
  fi
  cat > "$ENV_FILE" <<EOF
DIGILEGACY_DB=$DATA_DIR/digilegacy.db
DIGILEGACY_DB_KEY_FILE=$PREFIX/etc/db.key
DIGILEGACY_CONFIG_KEY_FILE=$PREFIX/etc/config.key
DIGILEGACY_BIND=$BIND
DIGILEGACY_API_TOKEN=$TOKEN
RUST_LOG=info
EOF
  chmod 0600 "$ENV_FILE"
  echo "wrote $ENV_FILE (mode 0600); the write token is not printed"
fi

set -a
# shellcheck disable=SC1090
. "$ENV_FILE"
set +a

"$PREFIX/bin/digilegacy-cli" health --json >/dev/null
echo "installed DigiLegacy $(cat "$PREFIX/VERSION")"
echo "prefix:   $PREFIX"
echo "data:     $DIGILEGACY_DB"
echo "start:    $PREFIX/bin/digilegacy-start"
echo "health:   $PREFIX/bin/digilegacy-cli health --json"
echo "uninstall:$PREFIX/bin/digilegacy-uninstall"
echo "web:      $PREFIX/web (serve this directory and proxy /api to $DIGILEGACY_BIND)"
