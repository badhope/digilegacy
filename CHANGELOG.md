# Changelog

All notable changes to this project are documented here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.0] - 2026-11-03

### Added

- **Command palette (⌘K / Ctrl+K).** Full keyboard path — type to filter, ↑↓ to move, Enter to run, Esc to close — covering all nine pages plus theme, language and lock actions. A visible header trigger exposes the shortcut so it is discoverable without reading docs. Accessible as a `dialog` + `combobox`, with `aria-activedescendant` and `aria-selected` wired.
- **Checkpoints with one-click restore.** A named snapshot taken the moment you press the button, distinct from the daily automatic backup. Restore goes through the same authenticated-import path (source password, wrapped key, AEAD tag), and takes a `pre-restore` checkpoint first so the rollback itself is reversible. Capped at 12, auto checkpoints evicted before manual ones.
- **Tool registry with permission tiers (`agent_tools.rs`).** Tool definition, execution and permission now live on one `ToolSpec`, so `tool_definitions()` and `execute()` cannot drift apart. Two tiers (`Read`, `Draft`) are enforced centrally instead of per-tool. Five tests assert every tool is annotated and that no tier can write to the database.
- **Design system layer (`design-upgrade.css`).** Modular type scale, 4px spacing ladder, five-step elevation ladder (separate values for dark mode), semantic surface tokens, and duration tokens. Cards and stat values use the new ladder; `prefers-reduced-motion` is honoured throughout.
- **New visual primitives:** conic-gradient metric ring, segmented dimension bar, skeleton loaders, and a proper empty-state block.

### Changed

- **Dashboard completion section** now pairs one headline ring with a four-dimension segmented bar — the percentage says how much is left, the breakdown says which dimension is behind.
- **Header** gained a search/command trigger that collapses to an icon on narrow viewports.
- **Theme toggle got a stable `data-testid`.** An E2E test selected `header button` by position and broke the moment a new button was added; the selector now names the control.

### Fixed

- **Recovery shares were split with a biased polynomial generator.** `sharks` 0.5.0 carries RUSTSEC-2024-0398: polynomial coefficients are drawn from `[1,255]` instead of `[0,255]`, which lets an attacker who observes the *same* secret shared ~500–1500 times (2-of-N) narrow the candidate space. No fixed upgrade exists upstream; the crate is now replaced by `blahaj` 0.6, the API-compatible fork published with the fix. Existing printed recovery shares still recombine — the share wire format is unchanged and reconstitution was never affected.
- **CI failed because `mca.gov.cn` cannot be reached from GitHub runners.** The authorities verifier treated a connection-level failure as a dead link. Dead-link evidence is HTTP-level (404/500/error page); a connection reset against a site known to block overseas datacenter IPs is "unverifiable from this network", printed as `NET?` with an explicit warning, and — when at least one other source is reachable — no longer fails the job. Network-unreachable entries are also never written back over a previous (networked) verification result.

- **CI was red on `main` after the 0.3.0 design work.** Three independent breakages: `commands_win_shell.rs` (Windows-only FFI) compiled without a `cfg` gate and failed on the ubuntu Rust job; `open_external` was gated `#[cfg(target_os = "windows")]` at the function level, so the non-Windows build lost the symbol that `generate_handler!` references unconditionally; and the win_shell imports tripped `unused_imports` under `-D warnings`. Windows behavior is unchanged — the command bodies already carried per-OS branches.
- **Deleted files never disappeared from the API-mirrored repos.** The GitHub API sync script built the remote tree on top of the existing one (`base_tree`), so anything removed locally (e.g. the removed `naiveTheme.ts`, which still imported the undeclared `naive-ui` package and turned the remote typecheck and release build red) stayed forever. The tree is now built from scratch — remote content mirrors `git ls-files` exactly, and the script retries secondary rate limits with backoff and moves the release tag instead of skipping it.

- **Metric ring label overlapped the percentage.** `place-items: center` stacked the two lines on top of each other; the ring now uses a column flow.
- **Command palette footer was pushed outside the panel** when the list overflowed; the panel is now a bounded flex column and only the list scrolls.
- **Checkpoints were invisible in the browser preview.** `loadCheckpoints()` sat inside the `if (isTauri)` guard, so the demo always showed the empty state even though the mock supports checkpoints.

## [0.2.3] - 2026-11-03

### Fixed

- **Legal templates page was covered by a single icon.** `@vicons` SVGs carry only a `viewBox`, so an icon placed directly in a text container rendered at the SVG default 300×150 and filled the whole page. Inline icons now have a global fallback size, and the legal notice uses a flex layout.
- **The report title read `DIGITALL EGACYPLAN`.** The English title was written with manual spaces (`D I G I T A L   L E G A C Y   P L A N`); HTML collapses runs of spaces, so the word boundaries vanished. Real text plus CSS `letter-spacing` now.
- **The reminder page said "All good" while the dashboard called the missing mail channel a blocker.** The state card now shows a delivery warning when no SMTP channel is configured, using the same rule as the backend (`dms_enabled && trigger_channel == "smtp"`).
- **Mixed-language UI in English mode.** Review payload field names were hardcoded Chinese in the Rust layer and printed verbatim; they are now stable `payload.field.*` identifiers translated by the frontend. Platform category names (4 of 16) had no English translations; they are now complete and covered by a gate in `check-doc-facts`.
- **`Heir: 演示继承人 (null)`** — a null heir contact was interpolated literally. Now distinguishes "no contact recorded".
- **The demo completion metric could never reach 100%.** The mock hardcoded `triggerReady` to `false` instead of deriving it from SMTP state like the backend.
- **A test asserted the buggy behaviour.** `HeirGuideView.test.ts` expected the category filter to show the raw Chinese data value instead of the translated label.

### Changed

- **`db.rs` split: backup logic moved to `db_backup.rs`** (~1,500 lines). Backup has its own validation layer, transaction boundaries and authenticated-import rules; the split gives security review a clear scope. Pure move, zero behaviour change.
- **`commands.rs` split into seven domain modules** — platform, assets, report, review, ai, profile, security — plus a shared `commands_win_shell.rs` (used by both `open_external` and `reveal_exported_file`). `lib.rs` still references `commands::*`; the IPC registration path is unchanged. Pure move, zero behaviour change.
- **Screenshots are regenerated in both languages** (Chinese in `docs/screenshots/`, English in `docs/screenshots/en/`), and the capture script asserts the page title matches the target language so a wrong page fails the run.
- **README rewritten as an outward-facing product page** (English primary, Chinese mirror), moving the test-count ledger to `docs/VERIFICATION.md` where it belongs.
- **`.github/CODEOWNERS` added**: crypto, backup, recovery, outbound-connection and gate scripts require maintainer review.
- **Repository hygiene**: 5 stale local branches and 1 remote branch removed (single `main` everywhere), `git filter-branch` leftovers purged, `.git` shrunk 358 MB → 29 MB.

## [0.2.2] - 2026-10-28

### Added

- **Family action guide** (`action_guide.rs`, `toolkit.rs`): a self-contained, printable HTML checklist grouped by platform — documents required, procedure, difficulty, expected duration, official link, and who handles each asset. It answers "what do I do first" rather than repeating the report. Includes an offline FAQ of 12 questions (no LLM, no network).
- **Custody locations** (`db_custody.rs`): where the encrypted backup lives and where the recovery code was written down. Without these two facts the family cannot read credentials even with the toolkit; both are field-encrypted and printed into the action guide, with an explicit warning when blank.
- **Family drill** (`toolkit::evaluate_drill`): checks, using metadata only, where the family would get stuck — missing heir, missing policy, unverified policy, unreachable trigger chain. Reads no credentials.
- **SMTP configuration in the UI** (`smtp_config` table): host / port / sender / username / app password plus a "send test email" button. Previously SMTP was environment-variables-only, so the packaged app could never actually reach the family; delivery still degrades to the local outbox when unconfigured.
- **Import snapshot + merge mode**: importing a backup now writes a pre-import snapshot first and defaults to *merge* (append) instead of wiping the database. Replace is still available behind a separate destructive confirmation.
- **Policy provenance gate** (`scripts/check-policy-provenance.mjs`): every built-in policy must carry a real source or an explicit "unverified" marker; the count of unverified entries is ratcheted so it can only decrease. Never auto-fills sources or review dates.
- **Four-dimension completion metric**: the dashboard now reports disposal decided / heir named / identifier filled / trigger reachable separately instead of one percentage that counted "picked a disposal method" as done.
- **Report export parity**: the exported report now carries the platform guidance, heir contact and bilingual copy that the on-screen preview always had, and the ZIP is written with owner-only permissions.

- **Multiple heirs and an executor** (`db_heirs.rs`): the profile can now record several heirs, each with a relation, contact and allocation note, plus exactly one executor. The third-stage dead-man's-switch notification goes to the executor; the report and action guide list everyone and mark who coordinates. Whole-list replace runs in one transaction so a rejected save never leaves half a list.
- **Optional bring-your-own-key assistant** (`ai_assist.rs`): off by default with no built-in endpoint — if you have an OpenAI-compatible service, enter its address, model and key. The outbound payload is built from a whitelist (platform name, category, disposal method, policy fields, your question) so accounts, names, contacts, amounts and credentials can never leave the machine; the UI shows a preview before the first request. Keys are field-encrypted, passed to curl through stdin (never argv), and answers are labelled AI-generated and unreviewed.
- **Backup format v4**: carries heirs, SMTP settings, custody locations and assistant config, so a machine move no longer requires re-entering them.

- **Generic review pipeline with AI pre-review and human sign-off** (`review.rs`, `review_profiles.rs`, `db_review.rs`, `ReviewView`): scenes are data — a scene declares its id, criteria and whether AI participates, and the pipeline contains no business nouns. Only `platform-policy` is registered today; adding legal templates or family notes is a profile plus one registry line. AI can only advance an item to *awaiting human*: `Approved` is reachable only through a human verdict, enforced by the state machine's type signature. Every run and decision appends an immutable audit row; content changes invalidate the prior decision by fingerprint. With no endpoint configured the pipeline degrades to human-only review instead of failing.

- **Strict review mode** (`ReviewStrictness`): the prompt enumerates every mandatory check and demands one answer line per item; `validate_strict_answer` voids the whole verdict if any item is unanswered (a skipped item means it was not looked at), promotes a `PASS` headline to `Flag` when per-item findings disagree, and `submit_human_review` refuses to approve a flagged item without a written reason. Legal documents run in this mode with six checks — signer, blanks, date format, amount consistency, witness block, contradictory dispositions — and the prompt forbids judging legality.
- **Four review scenes** on the one pipeline: platform policies, legal documents (strict), family notes, asset classification. Legal templates moved to `src/data/legal-templates.json` so the UI and the reviewer read the same bytes instead of two drifting copies.
- **Reviewer sees what is being reviewed**: each item carries a truncated content preview, and rows collapse to a summary line by default (152 policies fully expanded ran to several thousand pixels).

- **Authoritative source registry** (`src/data/authorities.json`, `scripts/verify-authorities.mjs`): eight references (Civil Code articles, PIPL, Supreme Court, PBC, CAC, MCA) embedded at compile time and shown in the inheritance report. The verifier probes every URL and records two separate facts — *reachable* and *content matched* — because a live page does not prove it contains the cited article. It also detects government error pages that still return HTTP 200.
- **Demo data schema versioning**: the browser preview now discards stale `localStorage` when the seed structure changes, so fixes to demo content no longer stay invisible behind an old cache.

### Fixed

- **Dashboard showed 100% while telling the user their family would receive nothing.** "Trigger channel ready" counted a heartbeat-only setup as complete, even though a heartbeat notifies nobody. It now requires an actual SMTP channel, and the checklist shows a warning state when DMS is on but unreachable.
- **Profile's heir list, custody fields, SMTP and assistant settings only loaded in the desktop app**, leaving the browser demo permanently empty even though the mock implements all four. They now load in both environments.
- **The DMS page still said SMTP must be configured through environment variables**, months after in-app configuration shipped.
- **Review UI leaked raw i18n keys**: an item with no AI verdict rendered `review.ai.null` and unknown statuses rendered `review.status.<value>`. Both now fall back to an em dash, with the allowed value sets declared explicitly.
- **Review UI fabricated an audit record**: the browser mock returned a synthetic "human approved" row for items nobody had reviewed, and stamped a review time on AI-skipped items. History now returns empty until something actually happened, and only a human verdict produces a `reviewed_at`.
- **Human-only scenes still offered "Run AI pre-review"**, which implies the button does something. It is now hidden and the section is retitled "review scope" with an explicit "nothing leaves this machine".
- **Per-scene payload fields were all identical**, so the legal scene claimed it would send platform-policy fields. Each scene now declares its own list.

- **Exporting a report dropped the guidance** the preview showed: platform policy, required documents, difficulty, duration and the official link were in the preview but absent from the file. The heir received a shortened document.
- **The report was always Chinese** (`<html lang="zh-CN">` hardcoded), so English users exported a Chinese document.
- **The ZIP toolkit skipped `write_private`**, so `02-资产清单.json` (all account identifiers) landed with default umask permissions while reports and backups were owner-only.
- **The discovery questionnaire was decorative**: answers only changed a counter. They now pre-select the matching asset categories (still fully editable).
- **`is_confirmed` was always 1** and `unconfirmed_count` therefore always 0 while being invisible in the UI. The metric is removed rather than left lying.
- **"Inherit" could be saved with no heir**, producing reports that say "heir: not specified". Both the frontend and the DB now reject it; batch registration validates the whole set before writing anything.
- **`db.rs` deadlocked the dashboard** when it read SMTP settings through a helper that re-acquired the global connection mutex. The check now queries the same connection.

- **备份格式 v3**: exports now carry the `recovery_seal`, so a recovery key still works after moving to a new machine. v1/v2 backups remain importable; importing a backup without a seal clears the stale local one.
- **Read-only DMS status + notification history**: the dashboard and the Legacy Care page now call `get_dms_status` (no SMTP, no outbox write) and `get_dms_history` (latest 50 entries from the local outbox), so past notifications stay visible after a refresh.
- **Auto-backup status** in Profile: snapshot count, latest file name and size, with an explicit "no snapshot yet" state and a visible error path.
- **Check-in history** on the Legacy Care page: `CheckInStatus.recent_check_ins` returns the latest 5 confirmations (newest first) so users can verify whether they actually checked in, instead of only seeing the single last timestamp.
- **English money formatting**: amounts now render as `123.5k` / `2.5M` in the English UI instead of reusing the Chinese 万 divisor.
- **Per-user file permissions** for backups, reports and the temporary SMTP netrc (`0600` on Unix) via a shared `fsutil::write_private`.
- **Bilingual UI is now real**: the header language switch drives the whole desktop UI, the dictionary key sets are asserted equal in tests, and `<html lang>` follows the selected locale.
- **Frontend visual pass**: Naive UI theme bridge and every emoji icon replaced by Ionicon / FontAwesome vectors through the new `PlatformIcon.vue` (24+ platforms, 16 categories); `main.css` gained the warm-ambient / glass-header / pill-nav / layered-card / dark-mode layers.
- **Opening the dashboard could send real email.** The frontend called `evaluate_dms`, which dispatches SMTP mail and writes the outbox; merely viewing a page performed a notification round. UI now reads the read-only status.
- **Plaintext backups could bypass encryption.** Importing a backup without `security_config` into an encrypted vault wrote plaintext into columns that must hold `enc:v1:` ciphertext; the import is now rejected while locked and encrypts field-by-field while unlocked.
- **Remaining days were truncated.** `num_days()` made "20 hours left" render as 0 days and the UI treated it as overdue; the value now rounds up.
- **Policy patch and backup import left stale platform caches.** `platformsExtended` was never invalidated, so the heir guide kept showing pre-patch policy text.
- **The ZIP toolkit shipped a broken report.** `01-继承报告.html` was an in-page fragment without `<!DOCTYPE html>` or print styles; it now uses the same standalone document as the single-file export.
- **`set_disposal_wish` bypassed every input check** (arbitrary length text, invalid enum) even though it writes straight to the asset table.
- **`import_backup` and `export_report_to_file` had no size caps**, unlike every other IPC entry point.
- **Heartbeat endpoint validation could be bypassed** through environment variables; non-http(s) schemes are now rejected inside `post_heartbeat`.
- **Report templates opened a blank WebView window** (`target="_blank"`) instead of the system browser; the form link now goes through `open_external`.
- **Chinese dictionary no longer carries emoji in UI strings**, and the English dictionary no longer has keys the Chinese one lacks.
- **DiscoverView step indicator** rendered raw dictionary keys (missing `t()`); route titles are resolved through the testable `resolveTitleKey`.

### Scenario simulation (2026-10-26)

- **Backend scenario suite** (`src-tauri/src/scenarios.rs`): **2824 checkpoints** over real SQLite + real AES — asset lifecycle (3 categories × 7 disposals × 3 value shapes × encrypted/plaintext), backup round-trips (30 rounds incl. v1 compatibility and 4 malformed inputs), credential vault (20 assets × encrypted/plaintext), DMS timeline (7 intervals × 10 offsets) and policy patches (30 rounds).
- **Frontend scenario suite** (`src/scenarios.test.ts`): **1714 checkpoints** across ten matrices — asset store, filter/sort/search, money formatting × locale, locked-state write paths, placeholder detection, mock lifecycle and platform cache invalidation.
- **Browser scenario suite** (`e2e/scenarios.spec.ts`): **39 scenarios** — 8 routes × 2 locales × 2 themes, 20 platform-search keywords, 4 filters × 4 sorts, 6 legal categories, heir-guide tabs/difficulty/collapse, DMS idempotence, report actions and per-page language/theme round-trips.

### Hardening follow-up (2026-10-26, third pass)

Acting on the review findings:

- **CI no longer allows a false green.** `cargo install cargo-audit --locked --quiet || true` swallowed install failures and then audited with whatever old binary was on the runner. The step now pins `0.22.1`, drops `|| true`, and prints `cargo audit --version` as evidence.
- **Documented numbers are now machine-checked, with an honest counting convention.** `scripts/doc-facts-lib.mjs` re-derives Rust/frontend/E2E test counts, IPC command count, table count and platform count from source; `check-doc-facts.mjs` fails CI when README, the bilingual website, `ARCHITECTURE.md` or `ROADMAP.md` disagree, and `sync-doc-facts.mjs` fixes them locally. The derivation lives in one place because the previous duplication had E2E at 76 in one script and 38 in the other. The Rust row reports **118 passing / 1 ignored** and states that 120 tests are declared in source — one is `#[cfg(unix)]` and does not compile on Windows, so quoting the declared number as "passing" would overstate the evidence.
- **Reports are rendered by Rust, not scraped from the DOM.** `report.rs` is the single renderer used by both the file export and the ZIP toolkit: every user-controlled value is HTML-escaped, print styles live in one place, and a report containing undecrypted placeholders is refused instead of exported. The refusal policy is a pure function (`commands::render_report_response`) so the placeholder gate and the size cap are covered by unit tests without a Tauri runtime.
- **"Unlocked" is now a verified state.** `security_status` checks the in-memory key against the stored verifier; a key that does not match reports as locked, and write paths refuse it. Previously a half-finished recovery could leave a mismatched key while the UI still claimed to be unlocked — the entry point for the placeholder write-back bug.
- **DMS reliability limits are stated in the product.** The Legacy Care page carries a dedicated block: detection only runs while the app runs, SMTP is user-supplied, the outbox is plaintext, and the optional heartbeat is the only channel that survives app exit. It also shows which channel is actually available, saying plainly that family receives no email when none is configured.
- **`db.rs` split by domain.** 3875 → 1882 production lines plus `db_platforms.rs` (policy patches) and `db_tests.rs` (1726 lines of tests). Pure move; the 117-test suite is the regression net.
- **Import robustness.** A deterministic 513-input fuzz-style test (random byte strings, type confusion, extreme ids, negative/absurd values, truncation) asserts no panic, no partial writes, non-empty errors. It immediately found that a non-array `user_profile`/`check_in_log` was silently treated as an empty array — the import "succeeded" and wiped the profile. Non-arrays are now rejected.
- **Dead config documented.** `stage3_wait_days` does not participate in stage computation; the field now says so explicitly instead of being an unexplained `allow(dead_code)`.
- **Removed 14 unreferenced screenshots** (2.9 MB) left over from v0.1.

### Adversarial debug (2026-10-26, second pass)

Reproduced and fixed four more defects, each with a regression test:

- **Decrypted placeholders could be written back, destroying the real data.** In a locked vault (or with a mismatched key) reads degrade to `【加密数据 · 需解锁】`; the asset-detail and profile forms submitted that placeholder, and `update_asset_db` / `save_user_profile_db` re-encrypted it as if it were the real value. Saving once permanently overwrote the ciphertext. Writes now reject placeholders before touching the DB, and the UI shows a blocking alert.
- **Assets could be deleted while locked.** `delete_asset_db` skipped the vault guard, so "locked" protected reads but not destruction.
- **Policy-patch platforms could not hold assets.** `digital_assets.platform_id` references `platforms`, but patches only wrote `platform_overrides` — selecting a patched platform and saving failed with `FOREIGN KEY constraint failed`. Patches are now mirrored into `platforms`; clearing a patch keeps in-use rows (marked `（补丁已清除）`) instead of failing or cascading deletes; asset lists show the patched name via `COALESCE`.
- **DMS never fired if the user never checked in.** With `last_check_in IS NULL` the stage stayed 0 forever. Enabling the switch now records an anchor; changing the interval does not reset an existing countdown.

Also fixed: external-link validation accepted `https://trusted.example.com@evil.example.com/` (userinfo spoofing) and control characters; the DMS outbox was written with default permissions; dashboard and report totals summed negative/absurd values while the UI filtered them.

### Scenario-found defects

- **Backup import accepted invalid asset fields.** A hand-edited backup could write an unknown category, an unknown disposal method, or a negative/absurd value straight into the asset table, poisoning statistics and reports. The import now validates every row inside the transaction and leaves existing data untouched on rejection.
- **Total estimated value could go negative.** `assets.totalValue` used `|| 0`, so a negative `estimated_value` was added as-is; it now sums only finite, non-negative amounts, matching the backend.
- **The web-demo mock accepted invalid assets**, so the demo behaved differently from the desktop build; it now enforces the same category/disposal/value bounds.

### Changed

- Frontend unit tests grew from 296 to **424** across 32 files; Rust tests 86 → **204 declared / 202 passing on Windows**; E2E 37 → **80**. Coverage 85.2% statements / 86.7% lines. README badges and website pills synced by `scripts/check-doc-facts.mjs`.
- Release pipeline publishes the MSI and NSIS installer with a SHA-256 digest file and re-downloads them after publishing to verify the checksum. Builds are not code-signed (no certificate is configured).

## [0.2.1] - 2026-10-26

### Changed

- `base64` 0.22 → 0.23, syncing dependabot #39. This was the last tagged build before the family-toolkit work in 0.2.2; it shipped no user-visible change of its own.

## [0.2.0] - 2026-10-24

### Added

- **Asset search, sort and composition chart.** Full-text search over platform / account / notes with AND semantics, four sort orders, and an ECharts donut breaking assets into property / personality / mixed. `AssetsView`, `AssetCompositionChart`.
- **Platform search in the discovery wizard**, so a 152-entry policy database is navigable by typing.
- **Interactive web demo** (`VITE_DEMO=1` build) deployed alongside the website: the same UI on mock data, hash routing, with a persistent banner warning that demo data lives in `localStorage` and must not hold real credentials.
- **Backup format v2**: exports now include `heartbeat_config` and `platform_overrides`. v1 backups remain importable and no longer wipe those settings on import.
- **`PRAGMA user_version`** schema versioning (`SCHEMA_VERSION = 2`) so future migrations have a baseline instead of guessing from `PRAGMA table_info`.
- **Command-layer input validation**: 8 KB text caps, finite/non-negative value bounds, positive platform id, 1–365 day DMS interval, 500-char profile fields.
- **Heartbeat server hardening**: `/check` requires `Authorization: Bearer $HEARTBEAT_ADMIN_TOKEN` (503 when unset — fail closed), unregistered tokens are rejected with 403, alert state persists across restarts and resets on the next heartbeat, `/health` probe, 413 on oversized bodies, atomic state writes. Covered by `server/heartbeat-server.test.mjs`.
- **404 route** with a recovery path instead of a blank page; `document.title` now tracks the active route.
- **Bilingual website.** `docs/zh/index.html` is a full Chinese counterpart of the English site (problem, 8 features, 10 screenshots, security, comparison, getting started, roadmap, FAQ, footer), sharing `docs/site.css`. The language switch sits in the nav as an `EN | 中文` pill, stays visible when the section links collapse on narrow screens, marks the current language with `aria-current`, and preserves the section anchor when switching. Bidirectional `hreflang`, per-page `canonical`/`og:locale`, and `zh/` entries in the sitemap.
- **App screenshot above the fold.** The hero now shows the real 2400×1600 dashboard capture; the gallery grew from 7 to 10 images (adding asset detail, profile and dark mode) and every shot links to the full-size PNG.
- **`README.zh-CN.md`** — the Chinese documentation is now a standalone file instead of a section at the bottom of an English README. Both READMEs cross-link at the top and their tables of contents were re-checked (0 broken anchors).
- **Bilingual demo banner** in the static demo build, so the "do not enter real credentials" warning is readable to English visitors too.
- **Demo media pipeline** (`scripts/demo-media.mjs`): ten screenshots (nine screens + dark mode) and a 32-second walkthrough video. Each capture asserts the page header matches the expected title, so a wrong-route screenshot fails the run instead of shipping.

### Fixed

- **CI was red on the Rust job for two consecutive runs.** `cargo audit --deny vulnerabilities` is rejected by cargo-audit 0.22 (`--deny` only accepts warnings/unmaintained/unsound/yanked). The step now runs plain `cargo audit`, whose default is to fail on vulnerabilities.
- **Asset load failures were rendered as "no assets yet".** The store swallowed the error and the view treated an empty array as an empty result; failures now surface a `role="alert"` card with retry.
- **Asset detail edited the store object directly**, so "Cancel" kept the unsaved values and a failed save produced an unhandled rejection. Detail now works on a deep-copied draft, stays on the page on failure, and disables the buttons while saving.
- **Danger confirmations focused the destructive button.** `FeedbackLayer` now lands on Cancel for `danger: true`, traps Tab inside the dialog, and restores focus to the trigger on close.
- **Importing a backup left every store stale**, making a successful restore look like a failure. Import now refreshes security, dashboard, assets and platforms.
- **DMS settings had two sources of truth.** Saving the profile silently rewrote the interval set on the DMS page. Profile is now read-only for DMS and points at the single editor; interval options live in `constants/dms.ts`.
- **Dark-mode contrast**: `--gray-0` was never defined (lock card, help drawer and tour fell back to near-white), sidebar text reused theme-flipping tokens (1.72:1), solid buttons used "text" tokens as backgrounds (2.26–3.49:1), and the heir-guide prerequisite card was hard-coded light yellow. All now use dedicated `--surface` / `--solid-*` / `--on-solid` / `--sidebar-*` tokens (verified 5.88–9.22:1).
- **Report page rendered a blank shell with clickable-but-dead export buttons** while loading. Added a loading state, an error state with retry, and hid the action bar until the report exists.

### Changed

- **README rewritten as a showcase, not a manual.** Both `README.md` and `README.zh-CN.md` now open with the real dashboard screenshot, a one-line pitch, the live-demo/download links, a four-stat band (152 platforms · 45 commands · AES-256-GCM · 0 servers), an eight-shot guided tour, a capability grid, a verification table and a tech stack. Module layering and the encryption data flow stay in `docs/ARCHITECTURE.md`, verification evidence in `docs/VERIFICATION.md`, and the long-form comparison, security notes and FAQ live on the website (EN + 中文) instead of bloating the front page.
- README and `docs/VERIFICATION.md` re-derived from real runs: 86 Rust tests, 265 frontend unit tests, 37 E2E, coverage 79.4/79.0/73.2/80.8.
- `src/router.ts` and `src/api/invoke.ts` — previously 0% / partially covered despite being the single entry points for navigation and IPC — now have dedicated unit tests (`router.test.ts`, extended `invoke.test.ts`), covering the 404 fallback, per-route `document.title`, scroll restoration, mock routing, and the "no backend" error path.

## [0.1.0] - 2026-10-22

Initial public release.

### Added

- Digital asset discovery over a built-in policy database of 152 platforms across 16 categories, with a guided self-check questionnaire and de-duplication against already-registered assets.
- Asset classification (property / personality / mixed) and disposal planning (inherit, liquidate, memorialize, authorize viewing, delete, donate, no action).
- Inheritance report (print-ready HTML) and a ZIP toolkit containing the report, asset list, summary, and instructions.
- Dead man's switch with a three-stage confirmation chain, system-tray residency, and an optional zero-content cloud heartbeat with a reference self-hosted server.
- Heir guidance wizard with per-platform procedures, required documents, and a monetization/transfer path card.
- Legal template library across six categories.
- AES-256-GCM field-level encryption with PBKDF2-SHA256 (600k) key derivation, master-password lock with idle auto-lock, one-time recovery key, and encrypted backup export/import.
- Optional encrypted credential vault, excluded from report and toolkit exports.
- Light/dark theme following the system preference, reduced-motion support, print styles.
- Daily automatic backup (7-day retention, atomic writes).
- GitHub Actions CI: frontend typecheck/tests/coverage/build, Playwright E2E, Rust fmt/clippy/test, heartbeat server behaviour, Windows installer build, `npm audit` and `cargo audit`.
