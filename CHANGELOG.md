# Changelog

English | [简体中文](docs/zh-CN/CHANGELOG.md)

Notable user-facing changes. The detailed per-milestone engineering log is kept in Chinese under [docs/zh-CN/CHANGELOG.md](docs/zh-CN/CHANGELOG.md).

## [0.9.0] - November 2026

### Added

- A contract-driven platform API, CLI and MCP entry point that share one command layer.
- Encrypted asset, planning, delivery and connection workflows with approvals and audit.
- A policy library with provenance, remote feed sync and a human review queue.
- Final notification delivery through Webhook, Feishu, WeCom and SMTP.
- Agent runs with planning, approval pause/resume, cancellation, budgets and usage reporting.
- Web console flows for asset discovery, planning check-ins, deliverables, policy review and connection health.

### Changed

- The desktop shell is no longer part of the product; Web, CLI, MCP and HTTP are the supported entry points.
- The Web app uses route-level loading and shared query caching instead of one large entry bundle.
- Connection credentials use a separate config key and `enc:v2` storage; read models never return secrets.

### Fixed

- Approval writes are queued durably instead of blocking the request.
- Remote policy updates no longer overwrite the library without review.
- Mobile tables no longer depend on a fixed-width horizontal scroll.
- Dynamic labels no longer leak raw translation keys.

## Earlier releases

The project previously shipped the security kernel, encrypted storage, recovery and backup flows, the first real domain services, policy and deliverable exports, triggers and integrations. See the detailed Chinese log for the full sequence.

## Unreleased

- Signed policy feed end-to-end coverage.
- Real STARTTLS and implicit TLS mail server testing.
- SSRF allowlist for connection probes.
