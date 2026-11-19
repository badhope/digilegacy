#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../.."
if [ -f "$HOME/.cargo/env" ]; then
  # shellcheck disable=SC1090
  source "$HOME/.cargo/env"
fi
cargo run -q -p digilegacy-foundation --release --example bus_bench
