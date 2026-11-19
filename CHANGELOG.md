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

### Security

- Outbound connection and policy endpoints now reject cloud metadata hosts, link-local addresses and private literal IPs before any request. Loopback stays available for local development; a LAN deployment can opt in with `DIGILEGACY_ALLOW_PRIVATE_NETWORK=1`.
- Policy sync applies the same URL guard before fetching a remote feed, so a candidate review cannot be used to reach an internal service.
- Signed policy feeds are covered by a real local HTTP test: HMAC body verification, tamper rejection and `If-None-Match` / 304 behavior.

## Earlier releases

The project previously shipped the security kernel, encrypted storage, recovery and backup flows, the first real domain services, policy and deliverable exports, triggers and integrations. See the detailed Chinese log for the full sequence.

## Unreleased

- Real STARTTLS and implicit TLS mail server testing.
- DNS-resolved private address checks and a configurable outbound allowlist for deployments that need more than literal-IP blocking.
