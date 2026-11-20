#!/usr/bin/env bash
set -euo pipefail

PREFIX=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
PID_FILE="$PREFIX/run/digilegacy.pid"

if [[ ! -f "$PID_FILE" ]]; then
  echo "DigiLegacy is not running (no pid file)"
  exit 0
fi

pid=$(cat "$PID_FILE")
if ! [[ "$pid" =~ ^[0-9]+$ ]] || ! kill -0 "$pid" 2>/dev/null; then
  echo "process $pid is not running; removing stale pid file"
  rm -f "$PID_FILE"
  exit 0
fi

if [[ "$(tr '\0' ' ' </proc/"$pid"/cmdline 2>/dev/null || true)" != *"$PREFIX/bin/digilegacy-server"* ]]; then
  echo "pid $pid does not look like this installation; refusing to stop it" >&2
  exit 1
fi

kill "$pid"
for _ in $(seq 1 40); do
  if ! kill -0 "$pid" 2>/dev/null; then
    rm -f "$PID_FILE"
    echo "stopped DigiLegacy (pid $pid)"
    exit 0
  fi
  sleep 0.25
done

echo "process $pid did not stop after SIGTERM; inspect it manually" >&2
exit 1
