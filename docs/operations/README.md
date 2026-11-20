# Operations

Guides for running a self-hosted DigiLegacy instance.

- [SLO](SLO.md) defines the service objectives, error budget and current measurement gaps.
- [Runbook](RUNBOOK.md) covers startup, daily checks, checkpoints, restore, upgrades and common failures.

The product is a single-node deployment by default. There is no built-in Prometheus endpoint yet; the SLO guide explains how to measure the objectives with health probes, the CLI and diagnostics.

Related documents:

- [Verification and release gates](../VERIFICATION.md)
- [Architecture](../ARCHITECTURE.md)
- [Security policy](../../SECURITY.md)
