#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Uninstall DigiLegacy.

Usage: digilegacy-uninstall [--prefix <dir>] [--purge-data]

The data directory is preserved by default. --purge-data removes it after the
installation directory has been removed; use it only after a backup exists.
EOF
}

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
if [[ -x "$SCRIPT_DIR/digilegacy-server" ]]; then
  DEFAULT_PREFIX=$(cd "$SCRIPT_DIR/.." && pwd)
elif [[ -x "$SCRIPT_DIR/bin/digilegacy-server" ]]; then
  DEFAULT_PREFIX=$SCRIPT_DIR
else
  DEFAULT_PREFIX=/opt/digilegacy
fi
PREFIX=${DIGILEGACY_PREFIX:-$DEFAULT_PREFIX}
PURGE_DATA=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --prefix)
      PREFIX=${2:?--prefix requires a value}
      shift 2
      ;;
    --purge-data)
      PURGE_DATA=1
      shift
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

if [[ -f "$PREFIX/run/digilegacy.pid" ]]; then
  "$PREFIX/bin/digilegacy-stop" || {
    echo "could not stop the server; refusing to remove a running installation" >&2
    exit 1
  }
fi

DATA_DIR=
ENV_FILE="$PREFIX/etc/digilegacy.env"
if [[ -f "$ENV_FILE" ]]; then
  set -a
  # shellcheck disable=SC1090
  . "$ENV_FILE"
  set +a
  if [[ -n "${DIGILEGACY_DB:-}" ]]; then
    DATA_DIR=$(dirname "$DIGILEGACY_DB")
  fi
fi

if [[ "$PURGE_DATA" -eq 0 && -n "$DATA_DIR" && "$DATA_DIR" == /* && "$DATA_DIR" != "/" ]]; then
  install -d -m 0700 "$DATA_DIR/keys"
  for key in db.key config.key; do
    if [[ -f "$PREFIX/etc/$key" ]]; then
      install -m 0600 "$PREFIX/etc/$key" "$DATA_DIR/keys/$key"
    fi
  done
  cat > "$DATA_DIR/keys/RESTORE.txt" <<EOF
Database keys preserved from $PREFIX.

To reuse this database, reinstall with --data-dir "$DATA_DIR". The installer
restores db.key and config.key from this directory before the health check.
Back up the database and these key files separately.
EOF
  chmod 0600 "$DATA_DIR/keys/RESTORE.txt"
fi

rm -rf "$PREFIX"
echo "removed installation directory: $PREFIX"

if [[ "$PURGE_DATA" -eq 1 ]]; then
  if [[ -n "$DATA_DIR" && "$DATA_DIR" == /* && "$DATA_DIR" != "/" && "$DATA_DIR" != "$PREFIX" ]]; then
    rm -rf "$DATA_DIR"
    echo "removed data directory: $DATA_DIR"
  elif [[ -n "$DATA_DIR" ]]; then
    echo "data directory was inside the installation directory and is already removed: $DATA_DIR"
  else
    echo "no data directory recorded; nothing else to purge"
  fi
else
  echo "data and keys preserved: ${DATA_DIR:-unknown}; remove them manually after a backup"
fi
