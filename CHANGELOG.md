# Changelog

English | [简体中文](docs/zh-CN/CHANGELOG.md)

Notable user-facing changes. The detailed per-milestone engineering log is kept in Chinese under [docs/zh-CN/CHANGELOG.md](docs/zh-CN/CHANGELOG.md).

## [0.0.13] - November 2026

### Added

- Five seeded fuzz smoke targets run on every Rust test pass: external URL validation, recovery-code normalization and seal parsing, paper-share decoding, KDF parameter parsing, and HTML escaping.
- The targets run 45,000 deterministic inputs with fixed seeds, so a parser panic is reproducible instead of depending on a fresh random corpus. `rand` was already a runtime dependency of the security crate and becomes dev-only for the render crate; the production dependency graph does not grow.

## [0.0.12] - November 2026

### Added

- A lightweight visual regression baseline: nine fixed-viewport WebP snapshots covering desktop light, desktop dark and a 390 px mobile overview. The whole set is about 292 KB and runs as a manual release check.
- The visual script freezes the clock and locale before load, writes per-shot diffs only on failure, and allows a 0.4% pixel budget so lossy WebP re-encoding does not produce noise.

### Fixed

- Narrow screens no longer squeeze page titles into two or three characters per line. Headers stack their actions below the copy and use a smaller display size below 700 px.
- The Web gzip budgets move from 101.1 / 122 to 101.15 / 122.1 KB for that mobile header fix; entry, vendor and CSS budgets are unchanged.

## [0.0.11] - November 2026

### Accessibility

- The axe WCAG A/AA gate now scans every Web route, not only the app shell and modal layers. Fifteen route renders are checked after their data settles: overview, assets, policies, discovery, asset detail, planning, deliverables, automation, run start, run detail, tools, approvals, both settings sections and not-found.
- The tool permission filter and its search field now have explicit accessible names.

## [0.0.10] - November 2026

### Added

- A contract-driven platform API, CLI and MCP entry point that share one command layer.
- Encrypted asset, planning, delivery and connection workflows with approvals and audit.
- A policy library with provenance, remote feed sync and a human review queue.
- Final notification delivery through Webhook, Feishu, WeCom and SMTP.
- Agent runs with planning, approval pause/resume, cancellation, budgets and usage reporting.
- Web console flows for asset discovery, planning check-ins, deliverables, policy review and connection health.

### Changed

- Version labels now follow the `0.0.x` patch scheme; older milestone tags remain historical in the detailed Chinese log.
- The desktop shell is no longer part of the product; Web, CLI, MCP and HTTP are the supported entry points.
- The Web app uses route-level loading and shared query caching instead of one large entry bundle.
- Connection credentials use a separate config key and `enc:v2` storage; read models never return secrets.

### Fixed

- Approval writes are queued durably instead of blocking the request.
- Remote policy updates no longer overwrite the library without review.
- Mobile tables no longer depend on a fixed-width horizontal scroll.
- Dynamic labels no longer leak raw translation keys.
- SMTP delivery is covered against local STARTTLS and implicit TLS servers, including certificate verification and the post-TLS `EHLO`.
- SMTP response/body lines are decoded as UTF-8 bytes instead of per-byte Latin-1 characters, so non-ASCII message bodies survive the wire.
- Modal layers keep `Tab` / `Shift+Tab` inside the active dialog and restore focus to the opener when closed; the lock screen focuses its unlock action.
- The console has a localized skip-to-main link, a labeled primary navigation, and a mobile drawer that behaves as a modal: focus moves to its close action, `Escape` closes it, the background becomes inert, and focus returns to the menu button.
- Secondary and semantic text tokens were adjusted so light and dark themes meet WCAG AA contrast for normal text; a token test now checks 15 foreground/background pairs per theme.
- An axe-core gate covers the app shell and the Agent drawer, command palette and lock screen under WCAG A/AA, and the mobile drawer has its own keyboard regression test.

### Security

- Outbound connection and policy endpoints now reject cloud metadata hosts, link-local addresses and private literal IPs before any request. Loopback stays available for local development; a LAN deployment can opt in with `DIGILEGACY_ALLOW_PRIVATE_NETWORK=1`.
- Policy sync applies the same URL guard before fetching a remote feed, so a candidate review cannot be used to reach an internal service.
- Signed policy feeds are covered by a real local HTTP test: HMAC body verification, tamper rejection and `If-None-Match` / 304 behavior.

## Earlier releases

The project previously shipped the security kernel, encrypted storage, recovery and backup flows, the first real domain services, policy and deliverable exports, triggers and integrations. See the detailed Chinese log for the full sequence.

## Unreleased

- DNS-resolved private address checks and a configurable outbound allowlist for deployments that need more than literal-IP blocking.
- Browser-level light/dark and screen-reader checks are still manual; the visual baseline is a fixed-environment reference, not a substitute for them.
