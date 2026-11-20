# Service level objectives

These objectives describe a single-node, self-hosted DigiLegacy deployment. They
exclude failures outside the operator's boundary: host loss, network partitions,
DNS, upstream mail or webhook providers, and the user's browser.

The numbers are **provisional**. The release gate is implemented, but the
capacity and failure drills that would validate the latency targets are still
tracked as M6 work. Until those drills run, treat p95 values as design targets,
not measured promises.

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

There is no `/metrics` endpoint in the current build. Use these signals until
one lands:

```bash
# Availability probe
curl -fsS http://127.0.0.1:8787/api/v1/health

# Structure, schema and audit-chain state
cargo run -q -p digilegacy-cli -- invoke diagnostics.export --json

# Backup freshness and recovery/key state
cargo run -q -p digilegacy-cli -- security status --json
```

A probe should alert on:

- three consecutive health failures, or more than 5 failures in 15 minutes
- `audit.valid = false`
- `lastBackupAt` older than 24 hours
- p95 latency above target for 30 minutes
- 5xx rate above 1% for 5 minutes

Do not alert on a single failed request. Retry once before opening an incident.

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
| No Prometheus/metrics endpoint | Objectives need external probes and log parsing | Add a read-only metrics surface with no personal data |
| Capacity drill not run | Latency targets are unverified | Seed the reference dataset, run read/write/approval load |
| No automated restore drill | RTO is a target, not evidence | Run the restore procedure quarterly and record duration |
| No provider-level delivery SLO | SMTP/webhook latency depends on the provider | Measure acceptance and final delivery separately |

Owner: the operator on call. Review the objectives after every incident and at
least once per quarter.
