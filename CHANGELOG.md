# Changelog

## [0.0.25] - November 2026

### Fixed

- `POST /api/v1/diagnostics/export` now matches its contract: the server registers the route and the application dispatch table handles `diagnostics.export`. Before this, the HTTP endpoint and the documented `invoke diagnostics.export` CLI command returned "not supported" even though the dedicated `diagnostics export` command worked.

### Testing

- Added a router regression test that calls the real HTTP route and checks `audit.valid` plus schema version.
- The application unit test now exercises the dispatch path instead of calling the method directly.

English | [简体中文](docs/zh-CN/CHANGELOG.md)

Notable user-facing changes. The detailed per-milestone engineering log is kept in Chinese under [docs/zh-CN/CHANGELOG.md](docs/zh-CN/CHANGELOG.md).

## [0.0.24] - November 2026

### Fixed

- A corrupt `data/models/active.json` is now deleted during startup instead of being silently ignored. Runtime state already reported "not running", but the invalid file could linger and confuse operators.
- A verified model now reports download progress `1.0` instead of leaving the last polling value slightly below 100%.

### Testing

- Added a unit test for corrupt `active.json` cleanup.
- Live browser smoke now includes the settings root on desktop and 390px mobile, so the local-model panel itself is covered by the route, overflow and axe checks (16 routes).

## [0.0.23] - November 2026

### Changed

- Local models are now presented strictly as capability-test candidates. The "recommended" badge, the "default" wording and the default MiniCPM5-1B copy were removed; users choose which model to download, enable and switch to. No model is bundled in the release package or enabled automatically.
- The test environment now preinstalls and enables **MiniCPM5-2B**, matching the requested 2B model. The local MiniCPM5-1B file was removed; 1B remains in the catalog for users who want the lighter test candidate.

### Testing

- Settings copy and catalog metadata no longer contain a default or recommended model.
- API reports 1B as `missing` and 2B as `ready`/active in the test environment; no model file is shipped in the archive.
- Rust workspace: 189 passed / 0 failed. Web: 79 passed / 0 failed.

## [0.0.22] - November 2026

### Removed

- Removed **Spark-X2.5-1.7B** from the local-model catalog and settings panel. It loaded and returned valid plans, but its only distinct capability was a native 1M context that the 8K/16K runtime on the 2-core target cannot use; it was slower than MiniCPM5-1B and less clearly positioned than MiniCPM5-2B.
- The local copy at `data/models/spark-x2.5-1.7b` was deleted, freeing about 1.1 GB. The model was never part of the source commit or release archive.

### Testing

- Catalog assertion added: only `minicpm5-1b` and `minicpm5-2b` remain, and no Spark id can be downloaded or activated.
- Rust workspace: 189 passed / 0 failed. Web: 79 passed / 0 failed.

## [0.0.21] - November 2026

### Fixed

- Concurrent activation of the same model is serialized by an activation mutex; two simultaneous requests now both return 200, share one PID and leave one sidecar.
- The local planner timeout is no longer silently capped at 120 seconds. Local models default to 180 seconds and may be raised to 300 through `DIGILEGACY_LLM_TIMEOUT_SECS`.
- On startup, a stale `active.json` whose PID and port are both dead is removed, so the planner cannot keep pointing at a stopped sidecar. A sidecar that is still listening is retained.
- The English settings page now translates the recommendation badge, context labels, parameter strings and all three model descriptions instead of mixing Chinese copy into English.
- When the platform API or local-model route is unreachable, the panel shows a readable unavailable message instead of `Unexpected end of JSON input`.

### Added

- `DIGILEGACY_LOCAL_MODEL_CTX` configures the sidecar context window (2,048–131,072, default 8,192). A 9,028-token prompt succeeds at 16K after failing at the 8K default.

### Testing

- Rust workspace: 187 passed / 0 failed. Web: 79 passed / 0 failed. `./tools/verify.sh` OK.
- Concurrent activation, damaged `active.json`, ghost PID cleanup, port exhaustion, read-only model directory, file-size quota, path traversal and full zh/en UI were exercised; results are recorded in `docs/operations/measurements/local-models-functional-2026-11-20.md`.

## [0.0.20] - November 2026

### Added

- Settings → Connections now includes a curated local-model catalog. **MiniCPM5-1B** is the default: 1.08B parameters, 131,072-token context, Apache-2.0, standard Llama architecture and a 688 MB Q4_K_M file.
- Each model has a real download action with progress, `.part` staging, streamed SHA256 verification and a persisted verification marker. The server downloads from ModelScope with `curl` because the CDN rejects some TLS fingerprints.
- "Enable" starts `tools/local_model_server.py`, a minimal OpenAI-compatible sidecar, then writes `data/models/active.json`. The planner reads that file when LLM environment variables are absent, so Web, CLI and HTTP planning all use the enabled local model without Ollama or manual env edits.
- MiniCPM5-2B is listed as a quality option with an explicit **actually >2B** label. Spark-X2.5-1.7B is listed as an experimental strict-<2B long-context option. On this build it loaded through `llama-cpp-python 0.3.37` and returned a valid plan, but it remains labelled experimental because long-context behaviour is not stress-tested.

### Fixed

- Local-model planning requests carry a JSON grammar constraint, so a 1B model cannot silently truncate the closing braces of a plan.
- A restarted server recognises an already-running sidecar through `active.json`; enabling the same model is idempotent, and enabling another model terminates the previous sidecar by PID.
- Activation now returns semantic errors (`NOT_FOUND` for unknown models, `CONFLICT` for unfinished files, `CORRUPT_DATA` for SHA256 failures) instead of mapping every failure to 502.
- Verification now checks file size and recomputes SHA256 before loading a model, so a forged marker cannot make a corrupted file look usable.
- A sidecar that exits immediately is reported within one poll instead of waiting the full 45-second readiness timeout.
- The Web panel clears the stale "downloading" notice when a download finishes and reacts immediately when the write token is saved.

### Testing

- `digilegacy-cli runs plan` against the real local sidecar returned a valid `assets.list` plan (`tokensIn=3596`, `tokensOut=19`) and a valid `assets.create` plan through the current program, with no mock provider.
- The server download manager fetched and SHA256-verified the 1.56 GB MiniCPM5-2B Q4_K_M file; MiniCPM5-1B was independently downloaded and verified at 688,065,920 B.
- Functional matrix: 33 checks across catalog, auth, download success/duplicate/failure, SHA256 corruption, activation, switching, stop, restart recovery, PID safety and planner integration; all passed. See `docs/operations/measurements/local-models-functional-2026-11-20.md`.
- Planner timings on the 2-core test host: MiniCPM5-1B 55 s, MiniCPM5-2B 116 s, Spark-X2.5-1.7B 83 s.
- Rust workspace: 185 passed / 0 failed. Web: 79 passed / 0 failed.
- Real Chromium live smoke: 14/14 routes passed, zero axe violations, zero page exceptions, including the new local-model settings block.

## [0.0.19] - November 2026

### Added

- Recovery-code unlock is wired through all four surfaces: the `security.recover` contract, `POST /api/v1/security/recover`, CLI `security recover`, and a "Use a recovery code" mode on the Web lock screen. The code is meant to be stored offline from the one-time setup output; a successful unlock only installs the current process session and is never persisted.
- The lock screen keeps the master-password form and adds a recovery mode switch. Empty input, a wrong code and rate limiting each get a localized message; focus still moves to the main region after success.

### Security

- `security.unlock` and `security.recover` share an in-process failure limiter: five failures inside a 60-second window block further attempts for 30 seconds and return `429 RATE_LIMITED`. A correct password is also refused while blocked. A successful unlock resets the counter.
- The global vault gate now allows `security.recover`, so the recovery path is not rejected with `VAULT_LOCKED`; every other command still returns `423` while locked.
- CLI `security recover` reads the code from stdin and, like `security unlock`, only affects that process. Scripts can still use `DIGILEGACY_VAULT_PASSWORD` for ordinary commands.

### Testing

- `tools/verify.sh` exit 0: **184 Rust + 79 Web** tests; contracts **44 commands / 23 tools / 60 schemas / 21 errors**; schema v16 / 31 tables. Web entry 80.75/82 KB, JS gzip 104.95/105.2 KB, CSS gzip 20.22/21.1 KB, total 125.17/125.5 KB.
- Real API drill on an isolated vault database: locked overview `423` → wrong password `400` → correct unlock `200` → overview `200` → relock, recovery code `200` → assets `200` (one item) → relock. Five wrong passwords produced `400/400/400/400/429`, and a correct password during the block still returned `429`. Evidence: `security-recovery-0.0.19-2026-11-20.json`.
- CLI: status readable while locked; `assets list` without a password returns `VAULT_LOCKED`; with `DIGILEGACY_VAULT_PASSWORD` it read one item; `security recover` succeeded, but the next process was still locked. Real Chromium covered password, recovery, relock and 390 px, with 0 axe violations and 0 page errors; 14/14 live routes and 9/9 visual baselines passed.

## [0.0.18] - November 2026

### Added

- Asset lifecycle is now explicit: assets accept `active` / `archived`, the detail page can archive and restore in place, and the CLI exposes `assets update --status`. Archiving hides an asset from the default list without deleting it.
- Planning is editable: `plan.profile.set` updates the holder name and `plan.heirs.set` replaces the heir list with add, edit, reorder, executor selection and deletion. Both return a fresh `plan.get` snapshot and are exposed through CLI, HTTP and the Web planning page.
- Empty states now carry the next action: a truly empty asset list opens the editor, a filtered empty list clears filters, and an empty automation run list starts a run.
- Approval rejection requires a reason in both the Web form and the domain command; the console also gets inline save confirmation.

### Fixed

- Offline mock mode now echoes the requested asset name and status, so archive/restore feedback works without a live API.
- Write-token guidance now points to **Settings → Security & data** everywhere, matching where `DIGILEGACY_API_TOKEN` is actually entered.
- The server now mounts the asset write routes declared by the contract: `POST /api/v1/assets` and `PATCH /api/v1/assets/{id}`.

### Security

- The console lock is now a real vault session, not a local boolean. `security.init` bootstraps `security_config` from the local CLI, `security.unlock` verifies the master password and installs the process DEK, and `security.lock` clears it. The Web lock screen only opens after the local API accepts the password.
- While a configured vault is locked, the application refuses every command except `security.status`, `security.unlock` and `security.lock`; locked reads return `423 VAULT_LOCKED`. The Web shell is not mounted behind the lock screen, so stale data is not left on screen.
- Vault bootstrap re-encrypts existing connection secrets and encrypted policy candidates from the old file/config key to a vault-derived domain key. Once `security_config` exists, field encryption derives from the unlocked session DEK, not the database key file.
- Web startup reads `security.status`, hides the console when locked, and focuses the main region after unlock without scrolling past the topbar. The skip link is fully hidden when unfocused, and the password field regains focus after a wrong attempt.
- CLI invocations are separate processes, so a standalone `security unlock` cannot carry the key forward. A locked CLI prompts for the password on a terminal, or accepts `DIGILEGACY_VAULT_PASSWORD` for scripts; the key dies with the process.

### Testing

- `tools/verify.sh` exit 0: **182 Rust + 77 Web** tests; contracts **43 commands / 23 tools / 59 schemas / 21 errors**; schema v16 / 31 tables. Web entry 79.72/82 KB, JS gzip 104.46/104.6 KB, CSS gzip 20.15/21.1 KB, total 124.61/124.8 KB.
- Real API drill on an isolated vault database: locked status, `423 VAULT_LOCKED`, wrong password `400 INVALID_ARGS`, correct unlock `200`, overview `200`, lock again, assets `423`. A real Chromium run covered wrong password, successful unlock, re-lock, 390 px layout, axe 0 violations and no page errors; 14/14 live routes and 9/9 visual baselines passed.

## [0.0.17] - November 2026

### Security

- The optional `DIGILEGACY_OUTBOUND_ALLOWLIST` is now enforced when a connection URL is validated, not only when it is probed. A disallowed host is refused before DNS resolution.
- Outbound HTTP(S), policy-feed and SMTP/IMAP probes now resolve the host once, validate every returned address, and pin the HTTP/TCP connection to that validated set. A hostname cannot pass the check as public and then connect to a private address on a second DNS answer.
- Cloud metadata addresses are rejected even when `DIGILEGACY_ALLOW_PRIVATE_NETWORK` is on; carrier-grade NAT, benchmarking, reserved and documentation ranges are rejected too.
- `DIGILEGACY_OUTBOUND_ALLOWLIST` adds an optional comma-separated host allowlist (`example.com`, `*.example.com`). When set, any host outside it is refused before DNS resolution.
- A versioned Linux release archive with `install.sh`, `start.sh`, `stop.sh` and `uninstall.sh`. The installer preserves the environment file on upgrade, never prints the write token, and keeps database keys when data is preserved. A clean-host drill installs, exports diagnostics, verifies a checkpoint, uninstalls and reinstalls against the preserved database.
- Outbound clients bypass environment HTTP proxies. A proxy would resolve the target again outside the pinned address set, so proxy routing is deliberately not used for these connection probes.

### Operations

- A same-origin nginx server block is included at `share/nginx/digilegacy.conf.example`. It covers SPA fallback, `/api` proxying, immutable asset caching, `no-store` for `index.html` and security headers; it was verified with nginx 1.18.0.
- The installer now makes static Web assets, docs, the nginx sample and operations tools readable after its private `umask 077`; without this, a non-root nginx worker could not read the SPA. The clean-host drill asserts exact modes.
- Added a policy-source precheck (`tools/policies/source_audit.py`) and a 152-row human worklist. It reports URL presence, HTTPS, root vs deep link, duplicate groups and reachability; it never edits policy data or provenance. The 2026-11-20 run found 150 root URLs, 2 deep links, 17 duplicate groups and 152/152 entries still needing human review.

### Testing

- A live Chromium smoke renders 15 route/theme/viewport combinations against the real API and Vue dev server, runs axe WCAG A/AA/2.2 AA with colour contrast, and requires no page exceptions, API 5xx or horizontal overflow. It caught and fixed two topbar `label-content-name-mismatch` issues; the final run has zero axe violations. The nine visual baseline shots also re-passed.

## [0.0.16] - November 2026

### Added

- A handoff-oriented account setup guide in **Settings → Connections**. SMTP, Webhook, Feishu and WeCom buttons prefill the existing encrypted connection form, and a copy action produces a note that can be sent to the account owner.
- `docs/operations/CONNECTION-SETUP.md` and its Chinese mirror, with the fields each provider needs, the separation between the platform write token and the account credential, and a checklist for handing the tested connection back.

### Accessibility

- The connections settings route is now part of the all-route axe gate, raising it from 15 to 16 routes.

### Changed

- Bundle budget moves to entry 82 KB, JS total 103.1 KB gzip, CSS 21.1 KB gzip and total 124.2 KB gzip for the setup guide. The 24 KB route budget and 130 KB vendor budget are unchanged; the settings route is 20.79 KB.

## [0.0.15] - November 2026

### Added

- A read-only `GET /api/v1/metrics` JSON snapshot: request totals, status classes, in-flight requests, latency p50/p95/p99/max and latency buckets. It records no paths, names, labels or credentials, and unknown routes are counted too.
- `digilegacy-cli diagnostics export --allow-write --json` now produces the sanitized structural snapshot, including `audit.valid` and per-table row counts. The contract command was listed but not wired before.
- A reproducible SLO reference dataset (`seed_reference`) and a strict capacity harness (`tools/bench/capacity.py`) that measures real HTTP reads and writes on a throwaway database.
- Capacity and restore drill evidence under `docs/operations/measurements/`: reads p95 9.54 ms / p99 11.09 ms, writes p95 3.70 ms, restore to healthy 158 ms with 10,120 valid audit rows.

### Changed

- SLO and runbook measurement now uses `/api/v1/metrics`, `diagnostics export` and `security status` instead of referring to a metrics endpoint that did not exist.

## [0.0.14] - November 2026

### Added

- A local `security checkpoint`, `security checkpoints` and `security verify` CLI surface. Checkpoints use a WAL checkpoint, an authenticated manifest and a `backup.create` audit record; `security status` now reports the real last backup time.
- An operations section with provisional SLOs, a monthly availability budget, and a runbook for startup, daily checks, backup, offline restore, upgrades and common failures.
- The runbook documents the deliberate absence of an online `security restore` command: restoring a live database stays a manual, offline operation.

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
