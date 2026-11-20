# Benchmarks

Two different checks live here; do not conflate them.

## Capacity drill

`capacity.py` measures a running server over real HTTP. It expects a throwaway
database seeded by:

```bash
DIGILEGACY_DB=/tmp/digilegacy-capacity/live.db \
  cargo run -q -p digilegacy-server --example seed_reference
```

The seeder tops up to 1,000 assets, 250 heirs and 10,000 audit rows. Then start
the release server and run:

```bash
DIGILEGACY_DB=/tmp/digilegacy-capacity/live.db \
DIGILEGACY_BIND=127.0.0.1:8799 \
DIGILEGACY_API_TOKEN=capacity-token \
  ./target/release/digilegacy-server &

python3 tools/bench/capacity.py \
  --base-url http://127.0.0.1:8799 --token capacity-token \
  --reads 1200 --writes 120 --concurrency 8 \
  --out docs/operations/measurements/capacity-2026-11-20.json --strict
```

Reads mix health, overview, assets, plan, audit, security and metrics. Writes
create scratch assets through `/api/v1/invoke`. The JSON records the dataset
size, client-side percentiles, errors and the server's own counters. `--strict`
exits non-zero when a latency target or an error budget is missed.

Use a temporary database only. The write phase is additive and never cleans up.

## Bundle budget

`web-budget.mjs` checks `web/dist/assets` against `budgets.json`. It runs inside
`./tools/verify.sh` after the production build.

`core-bus.sh` runs the foundation bus benchmark example; it is a microbenchmark,
not an HTTP capacity result.
