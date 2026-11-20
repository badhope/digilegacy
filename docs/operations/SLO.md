# Service level objectives

These objectives describe a single-node, self-hosted DigiLegacy deployment. They
exclude failures outside the operator's boundary: host loss, network partitions,
DNS, upstream mail or webhook providers, and the user's browser.

The numbers are **provisional**. The first capacity and restore drills ran on
2026-11-20 on a local reference host (see "Measured baselines"). They validate
the procedure and the current build, not every operator's hardware. Re-run the
capacity drill on the actual host before quoting a p95 as a promise.

## Objectives

| Area | Objective | Window | Measurement |
|---|---|---|---|
| API availability | **99.5%** of health probes succeed | Rolling 30 days | `GET /api/v1/health` every 60 s outside the host |
| Read latency | p95 ≤ **250 ms**, p99 ≤ **800 ms** | Rolling 7 days | `overview`, `assets.list`, `policies.list` on the reference dataset |
| Write latency | p95 ≤ **500 ms** excluding external delivery | Rolling 7 days | Asset create/update, approval decision, checkpoint create |
| Server error rate | **< 1%** 5xx responses | 5-minute windows | Reverse proxy or access log |
| Audit chain | **100%** valid on every daily check | Daily | `invoke diagnostics.export --json`, field `audit.valid` |
| Backup freshness | RPO **24 h**, RTO **2 h** | Continuous | `security status --json`, field `lastBackupAt`; restore drill |
| Final notification | **95%** accepted within **5 min** | Rolling 7 days | `notifications.list` plus provider logs |
| Release quality | **100%** of `./tools/verify.sh` gates pass | Every release | CI log and local rerun |

### Availability budget

30 days = 720 hours. At 99.5% availability the monthly error budget is **3 h 36
min**. A health probe every 60 seconds produces 43,200 probes per 30 days; 216
failed probes exhaust the budget.

Availability is measured by an external probe, not by `ps` or a process
supervisor. A process that is running but returning 500 is unavailable.

### Latency reference dataset

The provisional p95 targets assume:

- 1,000 assets, 250 heirs/contacts and 10,000 audit records
- one operator and at most two concurrent Agent runs
- SQLCipher on local SSD storage
- reads served by the platform process, not by a remote database

Record the actual dataset size with every benchmark. A p95 measured on an empty
database is not evidence for this objective.

## Measurement

`GET /api/v1/metrics` is a read-only, process-local JSON snapshot. It exposes
request totals, status classes, in-flight requests, latency p50/p95/p99/max and
latency buckets. It deliberately stores no paths, asset names, heir names,
connection labels or credentials. The counters reset when the process restarts,
so alerting must combine the snapshot with an external health probe.

```bash
# Availability probe
curl -fsS http://127.0.0.1:8787/api/v1/health

# HTTP counters and latency buckets since process start
curl -fsS -H "Authorization: Bearer $DIGILEGACY_API_TOKEN" \
  http://127.0.0.1:8787/api/v1/metrics

# Audit-chain state and row counts (writes one audit entry)
cargo run -q -p digilegacy-cli -- diagnostics export --allow-write --json

# Backup freshness and recovery/key state
cargo run -q -p digilegacy-cli -- security status --json
```

The metrics p95 is bucket-estimated (upper bound of the matching bucket), so
treat it as an alerting signal, not a percentile calculation. The drill harness
below records exact client-side percentiles.

A probe should alert on:

- three consecutive health failures, or more than 5 failures in 15 minutes
- `requests.status5xx / requests.total > 0.01` over a 5-minute increase
- `latencyMs.p95` above target for 30 minutes
- `audit.valid = false` from the diagnostics export
- `lastBackupAt` older than 24 hours
- metrics `uptimeSeconds` resetting unexpectedly (process restarted)

Do not alert on a single failed request. Retry once before opening an incident.

## Capacity drill

The reference dataset is reproducible on a throwaway database:

```bash
export DIGILEGACY_DB=/tmp/digilegacy-capacity/live.db
cargo run -q -p digilegacy-server --example seed_reference
# The seeder tops up to 1,000 assets, 250 heirs and 10,000 audit rows.

DIGILEGACY_BIND=127.0.0.1:8799 DIGILEGACY_API_TOKEN=capacity-token \
  ./target/release/digilegacy-server &
python3 tools/bench/capacity.py \
  --base-url http://127.0.0.1:8799 --token capacity-token \
  --reads 1200 --writes 120 --concurrency 8 \
  --out docs/operations/measurements/capacity-2026-11-20.json --strict
```

The script mixes health, overview, assets, plan, audit, security and metrics
reads, then creates scratch assets through the normal invoke endpoint. It
records the dataset size, client-side p50/p95/p99, error counts and the server's
own counters. `--strict` fails when a target or an error budget is missed.

## Measured baselines

2026-11-20, release build `0.0.15`, Linux x86-64 local host, loopback HTTP,
1,000 assets / 250 heirs / 10,000 audit rows before the write phase:

| Signal | Result | Target |
|---|---|---|
| Reads, 1,200 requests, concurrency 8 | p50 6.38 ms, p95 **9.54 ms**, p99 **11.09 ms**, 0 errors | p95 ≤ 250 ms, p99 ≤ 800 ms |
| Writes, 120 asset creates, concurrency 4 | p50 1.98 ms, p95 **3.70 ms**, p99 9.38 ms, 0 errors | p95 ≤ 500 ms |
| Restore to healthy | **158 ms** after copying a 3.5 MB checkpoint; health, 1,120 assets and 10,120 audit rows readable | RTO ≤ 2 h |
| Audit chain after restore | `valid: true`, `checked: 10,120` | 100% valid |

Raw evidence: `docs/operations/measurements/capacity-2026-11-20.json` and
`restore-2026-11-20.json`. These are local synthetic numbers. They do not cover
provider delivery latency, disk failure, or a host under real user load.

## Error budget policy

- At **50%** burn, stop non-essential feature work and review the failing signal.
- At **80%**, ship only fixes and operational work until the budget recovers.
- At **100%**, freeze releases; the next release must include the corrective
  action and a verification note.

For a single-node self-hosted product, a broken release is usually an
availability event even when the root cause is a bad migration or a missing key.
Count it against the budget.

## Current gaps

| Gap | Impact | Next action |
|---|---|---|
| Metrics are JSON, not Prometheus text | Prometheus needs a small adapter | Add a text exporter or scrape the JSON snapshot |
| Capacity drill is one synthetic host | A different disk/CPU can miss the target | Re-run `tools/bench/capacity.py` on the production host before launch |
| Restore drill is manual | RTO is measured for a 3.5 MB database, not a large one | Repeat quarterly with the production database size |
| No provider-level delivery SLO | SMTP/webhook latency depends on the provider | Measure acceptance and final delivery separately with a real account |

Owner: the operator on call. Review the objectives after every incident and at
least once per quarter.
