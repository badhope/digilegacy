# Architecture

DigiLegacy is a single-process Tauri 2 desktop application. There is no backend service, no database server, and no network call other than the optional heartbeat the user configures themselves.

```
┌──────────────────────────────────────────────────────────────┐
│ WebView (Vue 3 + TypeScript + Pinia)                         │
│   views/ ── stores/ ── api/invoke.ts ──┐                     │
└───────────────────────────────────────┼──────────────────────┘
                                        │ Tauri IPC (92 commands)
┌───────────────────────────────────────▼──────────────────────┐
│ Rust core (src-tauri/src)                                    │
│   commands.rs   thin command layer, split by domain below    │
│   agent_tools   tool registry + permission tiers              │
│   checkpoint    named snapshots + restore                     │
│   vault.rs      master key in memory, field encrypt/decrypt   │
│   crypto.rs     AES-256-GCM + Argon2id/PBKDF2 primitives      │
│   recovery.rs   one-time recovery key seal                    │
│   db.rs         schema, migrations, security, DMS, reports    │
│   fsutil.rs     0600 file writes (backups / reports / netrc)   │
│   notify.rs     DMS evaluation + outbox + SMTP (lettre)       │
│   heartbeat.rs  zero-content heartbeat over curl              │
│   backup.rs     daily snapshot, 7-day retention, atomic write │
│   platform_data.rs  built-in 152-platform policy database     │
└───────────────────────────────────────┬──────────────────────┘
                                        │ rusqlite (bundled SQLite, WAL)
                                        ▼
                             %APPDATA%/com.digilegacy.app/digilegacy.db
```

## Layering rules

1. **`commands.rs` is a thin adapter.** It validates IPC input (size caps, enum values, existence checks), wraps results in `ApiResponse<T>`, and delegates to `db::*`. The commands are split by domain into `commands_{platform,assets,report,review,ai,profile,security,custody}.rs` (plus a shared `commands_win_shell.rs` used by both `open_external` and `reveal_exported_file`); `lib.rs` still registers them through `commands::*`, so the IPC surface is unchanged. Business logic and SQL belong in `db.rs` and its `db_*.rs` store modules; cryptographic primitives belong in `crypto.rs` / `vault.rs` / `recovery.rs`.
2. **Frontend never talks to the filesystem or SQLite directly.** Every side effect goes through `invoke()`. The only exception is downloading a file in browser-preview mode, which the mock handles.
3. **`api/invoke.ts` is the single IPC seam.** In a Tauri webview it calls `@tauri-apps/api/core`; in `npm run dev` — and in the `VITE_DEMO=1` static build used for the website demo — it lazily loads `api/mock.ts`. The mock must implement the same command names and response shape; it is the only thing E2E and the public demo talk to. In a normal production build that branch is dead code, so Rollup drops `mock.ts` from the installer bundle entirely (verify: `ls dist/assets | grep mock` returns nothing after `npm run build`).
4. **`db.rs` owns the connection.** A process-global `Mutex<Option<Connection>>` holds one WAL connection with `busy_timeout=5000`. Tests serialize on `vault::test_lock()` because both the connection and the master key are process-global. The schema version lives in `PRAGMA user_version` (`SCHEMA_VERSION`), so a future migration can branch on a number instead of re-deriving intent from `PRAGMA table_info`.

## Data flow: saving an asset

```
DiscoverView form
  → assets store addAsset()
    → invoke('add_asset', { req })
      → commands::add_asset
        → db::add_asset_db
          → validate_category / validate_disposal            (reject unknown enums)
          → ensure_vault_writable                            (fail closed when locked)
          → vault::encrypt_if_needed per sensitive field
          → INSERT INTO digital_assets
```

Reading reverses it: `row_to_asset` calls `vault::decrypt_or_placeholder`, which returns a placeholder string rather than failing the whole list when one row cannot be decrypted (for example, while locked). Credential reads are stricter: `get_asset_credential_db` returns `Err` while locked instead of an empty string, because an empty string is indistinguishable from "never saved".

## Encryption model

| Concern | Where | Notes |
|---|---|---|
| Key derivation | `crypto.rs` | Argon2id (m=19 MiB, t=2, p=1) for new vaults, encoded in a `v3.<m>.<t>.<p>.` salt prefix. PBKDF2 legacy paths still resolve: `v2.` = 600k, bare base64 = 100k. |
| Two-layer keys | `vault.rs` | A random data key (DEK) encrypts fields; the password-derived KEK only wraps the DEK (`security_config.wrapped_dek`). Password change = rewrap, O(1). Fields carry `enc:v2:` (HKDF `field` domain); legacy `enc:v1:` still decrypts. |
| Field encryption | `vault.rs` | `enc:v1:` + base64(nonce ‖ ciphertext), one random 12-byte nonce per value. |
| Key lifetime | `vault.rs` | `Zeroizing<[u8; 32]>` in a `Mutex<Option<_>>`; cleared on lock and on idle timeout. |
| Recovery | `recovery.rs` | The master key itself is encrypted under a key derived from a 24-character code. Changing the password rotates the code and re-encrypts every field in one transaction. |
| Re-encryption | `db.rs` | Three paths share `PROFILE_SENSITIVE_COLUMNS` and the asset column list: enabling encryption, changing the password, and recovery reset. They must stay in lockstep — a column added to one path only is a data-loss bug. |

## Failure isolation

- The background loop (`lib.rs::spawn_background_loop`) wraps each round in `catch_unwind`: a panic in DMS evaluation or heartbeat is logged and the next round still runs. Without this, one panic silently kills the reminder chain and the user believes they are still protected.
- Daily backup failure never blocks startup.
- Heartbeat and SMTP failures degrade to the local outbox rather than surfacing as errors to the user.

## Testing strategy

| Layer | Tool | What it proves |
|---|---|---|
| Rust units | `cargo test` | SQL, encryption round-trips, migration, re-encryption, DMS staging, SMTP header sanitisation, backup retention. |
| Frontend units | Vitest + jsdom | Store behaviour, view rendering, error paths, accessibility attributes. Coverage thresholds are enforced over all of `src/**`. |
| Scenario matrices | `scenarios.rs` (Rust) + `scenarios.test.ts` (Vitest) | Combination sweeps that assert invariants after every step rather than per-function: 2824 backend checkpoints over real SQLite + AES, and 1714 frontend checkpoints across ten matrices. Counts are quoted as checkpoints only — the "combination" definition varies per family, so a single combo total was a drift source. These caught the missing backup-import validation and the negative `totalValue` sum. |
| E2E | Playwright (browser preview) | Real component tree, router, and store interactions against the mock backend. Catches view-layer regressions unit tests miss. `e2e/scenarios.spec.ts` adds 39 matrix scenarios (8 routes × 2 locales × 2 themes, 20 search keywords, 4×4 filter/sort, etc.). |
| Release | CI `release-build` + `release.yml` | The installers actually build, and version numbers agree across three manifests. |

E2E does not feed the v8 coverage numbers. The two are complementary: unit coverage measures branch breadth, E2E measures whether the screens actually work.

## DMS: read vs. dispatch

`notify::status_snapshot` (`get_dms_status`) only reads `dms_config` and computes the stage — no SMTP, no outbox write. `notify::evaluate_dms` is the dispatching path and is only called by the background loop. The UI must never call the dispatcher: opening a page used to send real mail. `get_dms_history` reads the newest 50 entries from `dms_outbox/*.jsonl`, and `get_check_in_status` returns `recent_check_ins` (latest 5 confirmations) so the check-in log is actually read by the UI, not just written.

## Report rendering

`report.rs::render_report_html` is the only place report HTML is produced. Both the single-file export and the ZIP toolkit call it through the `render_report` command, so the preview, the exported file and the toolkit cannot drift. Every user-controlled value passes through `escape_html`; print styles live in the module. `contains_placeholder` refuses to render when any sensitive field is still undecrypted — exporting a report that says "locked" would mislead the heir.

Why not scrape the DOM (the old approach): the page shows placeholders while locked, carries Naive UI's injected styles, and required the ZIP path to duplicate the wrapper markup — which is how the toolkit once shipped a bare fragment.

## Module layout

| File | Responsibility |
|---|---|
| `db.rs` | connection lifecycle, schema, migrations, security, DMS, report queries |
| `db_backup.rs` | backup export / authenticated import / stock encryption (split out of db.rs) |
| `agent_tools.rs` | tool registry: definition + execution + permission bound on one spec |
| `checkpoint.rs` | named snapshots, retention, restore plumbing |
| `commands_checkpoint.rs` | checkpoint IPC commands |
| `db_assets.rs` | asset CRUD (split out of db.rs; write-gate + placeholder rejection preserved) |
| `db_profile.rs` | user profile read/write; owns `PROFILE_SENSITIVE_COLUMNS` used by all three re-encryption paths |
| `heir_recovery.rs` | Shamir threshold recovery kit: share encode/decode, printable HTML, metadata (shares never persisted) |
| `kdbx_export.rs` | KDBX 4 long-term archive (KeePassXC-readable); refuses placeholders, archive password independent of master |
| `imap_checkin.rs` | Inbound mail check-in (reply = I'm fine): sender whitelist, uid dedup, lookback window; credentials vault-encrypted |
| `review_heir.rs` | "Heir comprehension" review scene: audits generated deliverables (action guide, heir plan) in strict mode — 6 checks from the family's reading perspective |
| `db_platforms.rs` | policy-patch import/export/clear, `platforms` mirroring for the asset foreign key |
| `db_tests.rs` | the db regression suite (split out so production code stays readable) |
| `report.rs` | report HTML rendering (escaping, print styles, placeholder refusal) |
| `fsutil.rs` | owner-only file writes/appends |
| `db_custody.rs` | custody locations (where the backup and recovery code live), field-encrypted |
| `action_guide.rs` | the family's printable, platform-grouped action checklist |
| `toolkit.rs` | action-guide assembly + family drill (metadata only, no credentials) |
| `family_faq.rs` | 12 deterministic bilingual FAQ entries embedded in the guide (no LLM, no network) |
| `db_heirs.rs` | multiple heirs + exactly one executor; whole-list replace in a transaction |
| `ai_assist.rs` | optional bring-your-own-key assistant: whitelist-built outbound payload, Rust-side ureq, no built-in service |
| `agent.rs` | bounded local tool loop: six read/draft tools, max 8 rounds, no credential/write tool, Tauri step events |
| `review.rs` | generic review state machine: AI can only advance to `AwaitingHuman`, `Approved` requires a human verdict |
| `review_profiles.rs` | the scene registry — adding a scene is data, not pipeline code |
| `db_review.rs` | append-only review audit trail; stale detection by content fingerprint |
| `notify.rs` | DMS staging, outbox, SMTP (UI config first, env vars as fallback); read-only status snapshot |
| `report.rs` | report HTML rendering (escaping, print styles, placeholder refusal, bilingual) |

## Heir recovery kit (Shamir threshold)

A single-machine app has exactly one decryption path: the owner's master password.
If the owner is incapacitated, the family cannot open the database even with the file.
The recovery kit closes that gap without a server:

- **Threshold split.** `blahaj` (GF(256) Shamir; the maintained fork of `sharks`
  after RUSTSEC-2024-0398) splits either the master key
  (default) or its ciphertext (passphrase mode) into `n` shares; any `threshold`
  of them rebuild the key. Defaults are 3-of-5, bounded to 2..=10.
- **Shares are never stored.** Only metadata (threshold, total, key fingerprint,
  creation time, optional KDF salt) goes into `heir_recovery`. Shares exist only
  in the printable HTML the user distributes. A copied database therefore yields
  nothing.
- **Human-transcribable encoding.** Shares are `DGL1-` + Base32 (A-Z2-7, 8-char
  groups) + a 2-byte SHA-256 checksum. Base32 avoids the case/`+/=` ambiguity of
  Base64 and does not collide with the `-` group separator; a single mistyped
  character fails the checksum instead of silently rebuilding a wrong key.
- **Batch fingerprinting.** Each share carries the key fingerprint, and the
  metadata stores it too. Mixing shares from different batches (or using shares
  after a password rotation) is rejected with an explicit message instead of
  producing a key that cannot decrypt anything.
- **Invalidation is transactional.** `change_master_password_db` and
  `reset_security_config_db` both delete the metadata inside the same
  `BEGIN IMMEDIATE` transaction that rewrites the security config, so the UI can
  never claim a stale kit is still valid.
- **Passphrase mode.** With a passphrase, the split payload is the AES-GCM
  ciphertext of the master key under an Argon2id-derived wrapping key. Shares
  alone are then insufficient — useful when shares are held by people who should
  not be able to recover unilaterally.

## Placeholder is read-only

`vault::decrypt_or_placeholder` degrades undecryptable values to `【加密数据 · 需解锁】` so one bad row cannot fail a whole list. That placeholder is a **display artifact, never data**: `db::reject_placeholder` blocks it in every write path (`update_asset_db`, `save_user_profile_db`), and the asset-detail / profile pages block saving before the IPC call. Without this, a locked read followed by a save re-encrypted the placeholder and destroyed the original ciphertext.

`delete_asset_db` also goes through `ensure_vault_writable`: a locked vault must not permit destruction any more than it permits reading.

## Policy patches and the platform foreign key

`digital_assets.platform_id` references `platforms`. Patches therefore have to be mirrored into `platforms` (`sync_platform_overrides_into_platforms`) or selecting a patched platform fails on insert. Asset queries join `platform_overrides` and select `COALESCE(o.name, p.name)` so the patched name wins. Clearing a patch removes only unused mirror rows; rows still referenced by assets are kept and renamed `（补丁已清除）` so user data is never cascaded away.

## Backup format

`version: 5` exports `security_config`, `recovery_seal`, `wrapped_dek`, heirs, review records and the user-configured integration metadata. Imports accept v1 through v5. A backup **without** `security_config` is treated as plaintext: importing it into an encrypted vault is rejected while locked and field-encrypted while unlocked — plaintext must never land in an `enc:v1:` or `enc:v2:` column. A malformed or incomplete `security_config` is rejected fail-closed; merge mode never replaces a different local key configuration.

## Authoritative sources

`src/data/authorities.json` is the registry of external references (statutes, ministries, courts). It is embedded at compile time with `include_str!`, so the UI, the report and the reviewer all read the same bytes.

The registry records **two independent facts** per entry, and the UI never merges them:

| Field | Proves | Does not prove |
|---|---|---|
| `reachable` | the page exists and answered | that it contains the cited text |
| `content_matched` | the page text contains the declared needles | that the citation is legally correct |

`scripts/verify-authorities.mjs` fills both by probing each URL (with retries — government sites time out intermittently). It also rejects the common failure mode of a **200 response that is actually an error page** ("哎呀,出错了"), because those would otherwise be counted as reachable.

One class of entry is deliberately marked `kind: "portal"` — the National Laws and Regulations Database. It is a search entry point, not an article page, and the report labels it as such so a reader does not assume the link opens the exact statute.

## Content review pipeline

The review pipeline is deliberately **scene-agnostic**. A scene is a `ReviewProfile` (id, title key, criteria, whether AI participates); the pipeline itself contains no business nouns. Four scenes are registered: `platform-policy`, `legal-template`, `family-note` and `asset-classification`, all with AI pre-review. Legal documents run under **strict mode** rather than being excluded — see below. Registering a fifth is a function plus a line in `all_profiles()`; the pipeline itself never changes.

### Strict mode

`ReviewProfile.strictness` turns "be careful" into something the code can check. In `Strict` mode:

1. The prompt enumerates every `StrictCheck` and requires one line per item in the form `CHECK <id>: <FOUND|OK|NA> <reason>`.
2. `validate_strict_answer` parses that answer. **If any item is missing, the whole verdict is downgraded to `Unknown`** — a skipped item usually means the model did not look, not that the field is fine.
3. If items report `FOUND` while the headline says `PASS`, the verdict is promoted to `Flag`: the per-item findings win over the summary line.
4. Clearing a flagged or undetermined item with `Approved` requires a non-empty human note, in both the UI and `submit_human_review`. Refusing does not require a reason — refusing is the conservative action.

Legal documents use six checks: signer present, blanks filled, date format consistent, amounts consistent, witness block present, no contradictory disposal instructions. The prompt explicitly forbids judging legality or citing statutes — that stays with lawyers and notaries.

Three invariants are enforced by types rather than comments:

1. **AI never approves.** `ReviewStatus::from_verdicts` takes `Option<&HumanVerdict>`; `AiVerdict::Pass` can only reach `AwaitingHuman`. There is no code path where an AI verdict yields `Approved`.
2. **Graceful degradation.** With no endpoint configured, `run_ai_review` records `Skipped` and leaves items in the human queue — it does not error, and it does not fabricate a verdict.
3. **Append-only trail.** Every AI run and every human decision appends a `review_records` row carrying the content fingerprint, both verdicts and the actor. Records are never updated, only added.

Each item also carries a `preview` — a truncated, newline-flattened rendering of what is being reviewed. Without it the reviewer sees only a title and would have to guess what the entry actually says.

Content changes invalidate prior decisions: `summarise_states_db` compares each item's current fingerprint against its latest record and reports the item as `stale` (back to pending) when they differ, so a review can never apply to text it did not see.

## AI assistant: streaming & local history

The assistant path (`ai_assist.rs`) is one seam used by both the one-shot
`ask_assistant` and the streaming `ask_assistant_streaming` command:

- **One whitelist.** Both build the outbound payload through the same
  `build_safe_payload` — a regression test asserts every `FORBIDDEN_FIELDS`
  entry is absent by name *and* by value.
- **SSE.** The streaming variant reads `data:` lines until `[DONE]`; comment
  lines and unparseable chunks are skipped (heartbeats must not kill a stream).
  Deltas travel to the frontend over a Tauri `Channel`; the frontend falls back
  to the one-shot command outside Tauri (demo site, unit tests).
- **Local history.** Completed Q&A rows land in `ai_chat_log` (schema v7) —
  text only, no asset snapshots. The history card is visible even when the
  assistant is disabled, and clearing requires an explicit confirmation.
- **Review pipeline untouched.** Streaming serves the Q&A path only; the
  review pipeline's "AI never approves" invariant is unchanged.


## Embedded Agent boundary

`agent.rs` is deliberately not a general-purpose autonomous computer agent. It is a
bounded **local research loop** built on the existing BYOK assistant seam:

1. The user supplies a task; the Rust side checks endpoint, API key, length and control characters.
2. The model receives six OpenAI-compatible tools: `query_assets`,
   `query_platform_policy`, `get_planning_stats`, `run_readiness_drill`,
   `get_trigger_status`, and `draft_heir_note`.
3. The only data tools return platform/category/planning facts. Account identifiers,
   names, contacts, notes, credentials, passwords, API keys and recovery codes are
   not part of the tool schema or `query_assets` result.
4. Tool output is marked as untrusted data before it is placed back into the model
   conversation; it cannot redefine system rules.
5. The loop is capped at eight model calls in total (the last one is synthesis
   only) and six tool calls per round. Responses are capped at 1 MiB, tool
   arguments at 8k characters, tool results at 6k and final text at 20k. It emits
   `started`, `tool_start`, `tool_end`, `final` or `truncated` events to the UI.
6. The only write-like tool is `draft_heir_note`, which returns text with an explicit
   human-review notice. There is no Agent write command for assets, profiles,
   credentials or recovery settings.

The existing review pipeline remains separate: AI can flag or pre-review content,
but only a human verdict can produce `Approved`. The Agent is for task completion and
research, not for silently approving content.

## Known limits

- The desktop UI ships in Simplified Chinese and English; `src/i18n/index.ts` holds both dictionaries, `setLocale` also updates `<html lang>`, and a unit test asserts the two key sets are identical. Legal template bodies are Chinese-only by design (they are jurisdiction-specific documents).
- Migrations are additive `ALTER TABLE` checks and `PRAGMA user_version` is currently `SCHEMA_VERSION = 7`; future destructive migrations must branch on the version rather than on column presence alone.
- `db.rs` still owns connection lifecycle, schema, security, DMS and report queries. Asset CRUD moved to `db_assets.rs`, profile to `db_profile.rs`, platform patches to `db_platforms.rs`, custody to `db_custody.rs`, heirs to `db_heirs.rs`, review trail to `db_review.rs`, **backup export/import to `db_backup.rs`**, and the regression suite to `db_tests.rs`.
- `commands.rs` keeps the cross-domain integration tests; the 92 IPC commands live in `commands_{platform,assets,report,review,ai,profile,security,custody,checkpoint}.rs` with a shared `commands_win_shell.rs`.

## Harness layer

The agent used to keep its tool definitions in one JSON array and its dispatch in a `match`, with the permission boundary expressed only in comments. `agent_tools.rs` makes that a data structure instead:

- **One `ToolSpec` per tool** — name, description, JSON Schema, permission tier and executor. `tool_definitions()` and `execute()` are both derived from `registry()`, so they cannot drift apart.
- **Permission tiers are enums, not comments.** `Read` and `Draft` are both allowed today; the check runs centrally in `execute()`, so a future write tier has one obvious place to be gated. A test asserts every tool is annotated and that no current tier can write.
- **Checkpoints reuse the backup pipeline.** `checkpoint.rs` stores named copies of the exported backup JSON and restores through the same authenticated-import function, which means a restore is subject to the identical source-password / wrapped-key / AEAD checks. A `pre-restore` checkpoint is written first.
- The public demo is a static build of the real UI over the mock backend. It is a showcase, not a security boundary: its data lives unencrypted in `localStorage`; recovery-kit, Agent and external integration paths are desktop-only.
