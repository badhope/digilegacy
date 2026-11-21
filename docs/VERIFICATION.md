# Verification

English | [简体中文](zh-CN/VERIFICATION.md)

Every change runs through the same gates. The goal is not a green badge; it is confidence that the contract, the data and the user path still agree.

## One command

```bash
./tools/verify.sh
```

This runs the contract check, architecture rules, Rust formatting and linting, the Rust workspace tests, Web typecheck and tests, the Web production build and bundle budgets.

Useful individual commands:

```bash
node tools/generate.mjs --check
node tools/arch-check.mjs
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cd web && npm run typecheck && npm test && npm run build
node tools/bench/web-budget.mjs
```

## What each gate protects

| Gate | Question it answers |
|---|---|
| Contract generation | Did a command, tool, schema or error drift between clients? |
| Architecture check | Did an entry point grow business logic, or a removed path return? |
| Rust format and lint | Is the backend mechanically clean? |
| Rust tests | Do domain rules, storage upgrades and entry points still behave? |
| Fuzz smoke | Do seeded malformed inputs panic a parser or leak markup? |
| Backup and restore | Can an authenticated checkpoint be created, listed and verified, and does an offline restore reach health with a valid audit chain? |
| HTTP metrics | Does a read-only endpoint expose request counts, status classes and latency buckets without personal data? |
| Web typecheck | Do generated types and components agree? |
| Web tests | Do cache, routing and page states still work? |
| Web accessibility | Do every route and modal layer pass the axe WCAG A/AA rules, and do light/dark text tokens hold at least 4.5:1? |
| Visual regression (manual) | Do nine fixed-viewport snapshots still match the committed baseline, including dark and mobile overview? |
| Production build | Does the app compile without hidden runtime-only imports? |
| Bundle budget | Did a change quietly make the main experience heavier? |
| Schema diagnostics | Does a fresh database open at the expected version, and can the sanitized diagnostic export read the audit chain? |
| Capacity and restore drills (manual) | Does the reference dataset meet the latency targets, and does a checkpoint restore reach health within RTO? |
| DNS rebinding drill (manual) | Does a hostname resolved once still connect to the validated public address after the DNS answer changes to a private address, with no second query? |

## Test layers

- **Domain tests** cover state transitions, validation and provenance rules.
- **Store tests** open a database, apply migrations and verify the resulting schema.
- **Application tests** exercise use cases without HTTP.
- **Network guard tests** cover metadata/private/link-local URL rejection, loopback allowance and signed policy-feed transport against a local HTTP server.
- **SMTP TLS tests** start local STARTTLS and implicit TLS servers with a test certificate, then verify the full mail dialog and UTF-8 body.
- **Live Chromium smoke** renders 15 route/theme/viewport combinations against the real API and Vue dev server. It requires a `main` element, no horizontal overflow, no page exceptions, no API 5xx and no console errors, and runs axe WCAG A/AA/2.2 AA with colour contrast enabled in the real browser. The first run found two `label-content-name-mismatch` violations in the topbar (search and language buttons); both were fixed. The final 2026-11-20 run passed all 15 with zero axe violations; evidence is `docs/operations/measurements/live-browser-2026-11-20.json`. This supplements, but does not replace, human screen-reader review.
- **Release package drill** builds a versioned tar archive, unpacks it under a fresh HOME, prefix and data directory, installs, starts, checks health and metrics, runs `diagnostics export`, creates and verifies a checkpoint, uninstalls while preserving data, reinstalls against the preserved database and finally purges. Evidence: `docs/operations/measurements/install-2026-11-20.json`.
- **Outbound resolution tests** resolve a loopback URL, assert every returned address passes the block rules, and reject metadata, CGNAT, benchmarking, reserved and documentation ranges. The HTTP, policy-sync and SMTP paths then connect to the resolved address list instead of resolving the hostname again.
- **Fuzz smoke tests** use fixed seeds and 45,000 inputs across five public boundaries: URL validation, recovery seals, paper shares, KDF parameters and HTML escaping. They are deterministic regression tests, not coverage-guided fuzzing.
- **Checkpoint CLI test** creates a checkpoint in a temporary encrypted store, lists it, verifies its manifest, and confirms `security status` sees the audit record.
- **Connections route test** mounts `Settings/Settings/connections` with the handoff guide and connection form, then runs the same axe gate used by every other route.
- **Server tests** cover transport behavior, error mapping, the metrics snapshot, middleware counting of both handled routes and fallback 404s, and the absence of secrets or asset names in the output.
- **Diagnostics test** exports the structural snapshot and requires `audit.valid`, schema v16, 31 tables and no row values.
- **Capacity drill** seeds 1,000 assets, 250 heirs and 10,000 audit rows, then runs 1,200 reads and 120 writes over real HTTP. The 2026-11-20 result was read p95 9.54 ms / p99 11.09 ms and write p95 3.70 ms with zero errors.
- **Restore drill** copies a verified 3.5 MB checkpoint to a new path, starts a temporary instance, probes health, reads counts and verifies 10,120 audit rows. The 2026-11-20 result was healthy in 158 ms.
- **Web tests** cover cache behavior, routing, page states, modal focus containment, the mobile navigation drawer, and axe WCAG A/AA scans of the app shell, the three dialogs and all sixteen feature routes.
- **Route accessibility tests** mount each view against the real router, settle its data, then run axe on the rendered body. A failing route reports the violation ID and help text, so a new page cannot regress silently.
- **Contrast tests** parse the light and dark design tokens and require at least 4.5:1 for 15 foreground/background pairs in each theme. This supplements axe, which cannot compute colour contrast reliably in jsdom.
- **Manual smoke** checks the path a person actually clicks.
- **Visual regression** captures nine fixed-viewport WebP shots against a mock build with a frozen clock, locale and theme. It runs outside `tools/verify.sh`; see [tools/visual/README.md](../tools/visual/README.md).

## Policy source precheck

`content/policies/platforms.json` holds 152 policies and every provenance row
starts as `unverified`. `python3 tools/policies/source_audit.py` produces a
review worklist, not a verification result.

- 2026-11-20: 152/152 URLs present and HTTPS; 150 platform root URLs, only 2
  specific deep links; 17 duplicate groups covering 38 entries.
- Reachability: 134 reachable, 2 blocked, 2 broken, 11 timeout, 3 transport
  errors. Timeout/blocked is not proof of a dead source.
- 152/152 still need human review because every row is a root URL, a duplicate,
  or did not return a clean 2xx/3xx.
- Evidence: `docs/operations/measurements/policy-sources-2026-11-20.json`;
  worklist: `docs/operations/policy-source-worklist.csv`; procedure:
  `docs/operations/POLICY-SOURCES.md`.
- Boundary: the script does not edit policy data or provenance. A reachable
  home page does not prove the page contains the cited rule.
- The verify gate now byte-compiles the policy audit, clean-host drill and
  capacity scripts; the last full run is **184 Rust + 79 Web** tests, with
  **44 commands / 23 tools / 60 schemas / 21 errors** and schema v16 / 31 tables.

## Manual smoke before release

1. Start the API and the Web console with a fresh data directory.
2. Run `printf '%s\n' "<master-password>" | digilegacy security init`, save the recovery code, restart the API, and confirm `/api/v1/overview` returns `423 VAULT_LOCKED` until the Web lock screen accepts the password.
3. Run `DIGILEGACY_VAULT_PASSWORD="<master-password>" digilegacy assets list` against the same database and confirm the CLI returns the asset list; without the variable it must exit with `VAULT_LOCKED`.
4. Create an asset, edit it, and confirm the detail page and delivered report show the same value.
5. Configure an heir and complete a check-in.
6. Ask the Agent to draft a plan; confirm a write step waits for approval.
7. Approve one write and reject another with a reason; confirm both outcomes are audited.
6. Sync a policy feed and confirm the library is unchanged until a candidate is approved.
7. Open the mobile layout and the dark theme; check for overflow and unreadable text.
8. On a narrow viewport, open the navigation drawer with the keyboard: focus should enter the drawer, `Escape` should close it and return focus to the menu button.
9. Open the tools page and move through the search field and permission filter with the keyboard; both should announce a name, and the filter should expose its current value.
10. Build the mock console, start the preview server and run `python3 tools/visual/regress.py`; all nine shots should stay within the 0.4% pixel budget. Then start the API and the dev server and run `python3 tools/visual/live_smoke.py`; all 15 route/theme/viewport combinations should pass.
11. Create a checkpoint with `security checkpoint --label smoke`, list it, run `security verify <id>` and confirm `security status` reports the new `lastBackupAt`.
12. Read `/api/v1/metrics`; confirm counters increase after a few requests and contain no path, asset name or token.
13. Run `diagnostics export --allow-write --json`; confirm `audit.valid: true` and schema v16 / 31 tables.
14. Run `unshare -Urnm python3 tools/security/dns_rebind_drill.py`; confirm PASS, one A query, a pinned connection and a rejected rebound resolution.

## Release checklist

- [ ] `./tools/verify.sh` passes.
- [ ] The changelog describes user-visible changes.
- [ ] The English and Chinese docs describe the same behavior.
- [ ] No secret, token or local path is committed.
- [ ] The generated artifacts are current.
- [ ] A backup and restore path has been tested when storage changed.
- [ ] The clean-host drill passes: `python3 tools/package/clean_drill.py`; the evidence JSON records archive SHA256, install, export, checkpoint, uninstall and reinstall.
- [ ] A verified checkpoint exists before any migration or destructive repair; restore follows the offline runbook.

The detailed Chinese verification record keeps the historical evidence for each delivery. The English page describes the gates and the expected behavior, not a running tally.

## 0.0.19 recovery and failure limit

- Added `security.recover`: draft permission, `requiresUnlock=false`, `POST /api/v1/security/recover`, CLI `security recover` reading the code from stdin. The application vault gate explicitly allows the recovery command while locked.
- `security.unlock` and `security.recover` share an in-process limiter: five failures inside 60 seconds block attempts for 30 seconds and return `429 RATE_LIMITED`; a correct password is also refused while blocked, and success resets the counter. The existing `RATE_LIMITED` error keeps the total at 21.
- The Web lock screen switches between master password and recovery code. Recovery uses `autocomplete=one-time-code`; empty input, wrong code and rate limiting have localized messages. Real Chromium covered password, recovery, relock and 390 px: 0 axe violations on the lock screen and unlocked page, the wrong-password message is "主密码不正确", recovery opens the main region, 0 page errors and no mobile horizontal overflow.
- Isolated vault real API: locked overview `423` → wrong password `400` → correct unlock `200` → overview `200` → relock, recovery `200` → assets `200` (one item) → relock. Five wrong passwords were `400/400/400/400/429`; a correct password during the block still returned `429`.
- CLI: status readable while locked; `assets list` without `DIGILEGACY_VAULT_PASSWORD` exited 5 with `VAULT_LOCKED`; with the variable it read one item; `security recover` succeeded, but the next independent process was still locked. Evidence: `security-recovery-0.0.19-2026-11-20.json`.
- Gate: `./tools/verify.sh` exit 0, **184 Rust + 79 Web**, **44 commands / 23 tools / 60 schemas / 21 errors**, schema v16 / 31 tables. Entry 80.75/82 KB, JS gzip 104.95/105.2 KB, CSS gzip 20.22/21.1 KB, total 125.17/125.5 KB. Log: `/tmp/digilegacy-verify-0.0.19.log`.
- Live Chromium: 14/14 routes, 0 axe violations, no page errors, no horizontal overflow, `live-browser-0.0.19-2026-11-20.json`; 9/9 visual baselines, max 0.173%.
- Boundary: the limiter is process-local and resets on restart; the recovery code remains a one-time offline material printed only at setup; the SQLCipher file key is unchanged, and the vault DEK still covers field keys and the session only.

## 0.0.19 release archive

- `tools/package/build.sh` produced `packages/digilegacy-0.0.19-linux-x86_64.tar.gz` (18,293,344 bytes, SHA256 `53f86e570d59571dccfd27c0df789af665978551f4d413d61c14296cb805e8ee`).
- Clean-host drill: 12/12 PASS, fresh HOME/prefix/data/minimal environment; install, health v16/31, diagnostics, checkpoint verification, uninstall preserving data and keys, reinstall reading the preserved database, and full purge. Evidence: `install-0.0.19-2026-11-20.json`; the archive SHA matches the package.
- Live Chromium 0.0.19: 14/14 routes, 0 axe violations, no page errors, no 390 px horizontal overflow. Evidence: `live-browser-0.0.19-2026-11-20.json`.
- Recovery and rate limiting passed through API, CLI, desktop and mobile lock screens; evidence: `security-recovery-0.0.19-2026-11-20.json`. Visual regression: 9/9.

## 0.0.18 release archive

- `tools/package/build.sh` produced `packages/digilegacy-0.0.18-linux-x86_64.tar.gz` (18,274,248 bytes, SHA256 `0b947a3dd13f5618f0e324bf9e4182029052f3e21f6efc7b9ba941bd6f252ee6`).
- Clean-host drill: 12/12 PASS, archive unpacked, health v16/31, diagnostics valid, checkpoint verified, uninstall preserved data and keys, reinstall read the preserved database. Evidence: `docs/operations/measurements/install-0.0.18-2026-11-20.json`.
- Live Chromium 0.0.18: 14/14 routes, 0 axe violations, no page errors, no horizontal overflow. Evidence: `docs/operations/measurements/live-browser-0.0.18-2026-11-20.json`.
- Vault lock drill: locked overview 423, wrong password 400, correct unlock 200, overview 200, relock assets 423; CLI blocked without `DIGILEGACY_VAULT_PASSWORD`. Evidence: `docs/operations/measurements/security-lock-0.0.18-2026-11-20.json`.

## 0.0.31 built-in starter pack

- Schema v18 / 32 tables. `agent_skills.builtin` is added and the v18 pack is seeded once through the migration ledger. A real existing database upgraded to 8 built-in skills (3 enabled); `agent.harness.status` returned `skills.builtin: 8` and `builtinMcp: 6`.
- The bundled `digilegacy-mcp` answered `initialize` + `tools/list` with 20 read/draft tools and `overview.get` returned `isError: false`. The three write tools are intentionally absent from the MCP manifest and remain behind platform approval.
- Harness shows 23 tools, 6 MCP catalog entries and 8 skills. The MCP catalog uses native expandable rows; the bundled entry is open by default and shows source, transport, real command and official docs. Built-in skills carry a badge; local-model cards say weights are downloaded on demand and never bundled.
- Planner regression: a fresh DB system prompt contains the user skill plus 数字资产盘点, 执行人交付手册 and 平台遗产政策核对.
- Gate: `./tools/verify.sh` exit 0; 202 Rust + 83 Web; **48 commands / 23 tools / 65 schemas / 21 errors**; schema v18 / 32 tables; entry 82.70/83 KB, JS gzip 116.00/116 KB, CSS gzip 21.66/22 KB, total 137.65/138 KB.
- Live Chromium: 18/18 routes, 0 axe violations, no page errors, no 390 px overflow. Evidence: `docs/operations/measurements/live-browser-0.0.31-2026-11-22.json`; screenshots in `screenshots-0.0.31/`.
- Archive: `packages/digilegacy-0.0.31-linux-x86_64.tar.gz`; the sidecar `.sha256` is authoritative. Clean-host drill 12/12 PASS; evidence `docs/operations/measurements/install-0.0.31-2026-11-22.json`, including an archive-SHA consistency check.
