# Operations

Guides for running a self-hosted DigiLegacy instance.

- [SLO](SLO.md) defines the service objectives, error budget, measurement commands and measured baselines.
- [Measurements](measurements/) holds the reproducible capacity and restore drill evidence.
- [Install, upgrade and uninstall](INSTALL.md) covers the release archive and clean-host drill.
- [Runbook](RUNBOOK.md) covers startup, daily checks, checkpoints, restore, upgrades and common failures.
- [Connection setup handoff](CONNECTION-SETUP.md) is the page to send when the mailbox or webhook account belongs to someone else.
- [Policy source precheck](POLICY-SOURCES.md) explains the 152-row source worklist and keeps machine reachability separate from human verification.

The product is a single-node deployment by default. `GET /api/v1/metrics` exposes process-local counters and latency buckets as JSON; it is not Prometheus text format. The SLO guide explains the probe, metrics, diagnostics and checkpoint signals.

Related documents:

- [Verification and release gates](../VERIFICATION.md)
- [Architecture](../ARCHITECTURE.md)
- [Security policy](../../SECURITY.md)
