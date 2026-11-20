#!/usr/bin/env bash
set -euo pipefail

PREFIX=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
ENV_FILE="$PREFIX/etc/digilegacy.env"
PID_FILE="$PREFIX/run/digilegacy.pid"
LOG_FILE="$PREFIX/log/server.log"

[[ -f "$ENV_FILE" ]] || {
  echo "missing $ENV_FILE; run the installer first" >&2
  exit 1
}

set -a
# shellcheck disable=SC1090
. "$ENV_FILE"
set +a

if [[ -f "$PID_FILE" ]]; then
  pid=$(cat "$PID_FILE")
  if [[ "$pid" =~ ^[0-9]+$ ]] && kill -0 "$pid" 2>/dev/null; then
    echo "DigiLegacy is already running (pid $pid)"
    exit 0
  fi
  rm -f "$PID_FILE"
fi

install -d -m 0755 "$PREFIX/log" "$PREFIX/run"
nohup "$PREFIX/bin/digilegacy-server" >>"$LOG_FILE" 2>&1 &
pid=$!
echo "$pid" >"$PID_FILE"

bind=${DIGILEGACY_BIND:-127.0.0.1:8787}
check_host=${bind%:*}
check_port=${bind##*:}
case "$check_host" in
  0.0.0.0|"") check_host=127.0.0.1 ;;
  "::"|"[::]") check_host=::1 ;;
esac

for _ in $(seq 1 40); do
  if ! kill -0 "$pid" 2>/dev/null; then
    echo "server exited during startup; see $LOG_FILE" >&2
    tail -20 "$LOG_FILE" >&2 || true
    rm -f "$PID_FILE"
    exit 1
  fi
  if command -v curl >/dev/null 2>&1; then
    if curl -fsS "http://$check_host:$check_port/api/v1/health" >/dev/null 2>&1; then
      echo "DigiLegacy is ready at http://$check_host:$check_port (pid $pid)"
      exit 0
    fi
  elif "$PREFIX/bin/digilegacy-cli" health --json >/dev/null 2>&1; then
    echo "DigiLegacy is running (pid $pid); install curl for an HTTP readiness check"
    exit 0
  fi
  sleep 0.25
done

echo "server did not become ready; see $LOG_FILE" >&2
tail -20 "$LOG_FILE" >&2 || true
exit 1
