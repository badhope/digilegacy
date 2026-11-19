#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

if [ -f "$HOME/.cargo/env" ]; then
  # shellcheck disable=SC1090
  source "$HOME/.cargo/env"
fi

echo "== contracts =="
node tools/generate.mjs --check
node tools/arch-check.mjs

echo "== rust =="
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

echo "== web =="
(
  cd web
  npm run typecheck
  npm test
  npm run build
)
node tools/bench/web-budget.mjs

echo "verify: OK"
