# Landscape review: what 12 comparable projects taught us

> Snapshot date: 2026-11-06. Stars, licences, languages and last-push dates below come from the
> GitHub REST API on that date and can be re-checked per row. Where a claim is about behaviour
> rather than metadata, it is marked as taken from that project's README or source tree — not inferred.

This is not a feature-count exercise. The only question that matters here is the same one the
product roadmap uses: **when the family opens the envelope, can they finish the job without help?**

## 1. The projects we studied

| # | Project | Stars | Licence | Stack | What it is |
|---|---|---|---|---|---|
| 1 | [bitwarden/clients](https://github.com/bitwarden/clients) | 14,161 | mixed (GPL-3.0 + Bitwarden licence) | TypeScript | Reference implementation of Emergency Access |
| 2 | [dani-garcia/vaultwarden](https://github.com/dani-garcia/vaultwarden) | 70,322 | AGPL-3.0 | Rust | Bitwarden-compatible server, single binary |
| 3 | [keepassxreboot/keepassxc](https://github.com/keepassxreboot/keepassxc) | 29,811 | mixed | C++ | Offline-first vault, gold standard for local files |
| 4 | [giovantenne/lastsignal](https://github.com/giovantenne/lastsignal) | 758 | MIT | Ruby/Rails | Email-first dead man's switch, E2EE payloads |
| 5 | [adamdecaf/deadcheck](https://github.com/adamdecaf/deadcheck) | 334 | Apache-2.0 | Go | Switch that survives its own host going down |
| 6 | [alpyxn/aeterna](https://github.com/alpyxn/aeterna) | 308 | GPL-3.0 | Go + React | Self-hosted legacy messages with attachments |
| 7 | [bkupidura/dead-man-hand](https://github.com/bkupidura/dead-man-hand) | 384 | GPL-3.0 | Go | Executes actions when you stop responding |
| 8 | [aliasvault/aliasvault](https://github.com/aliasvault/aliasvault) | 3,466 | AGPL-3.0 | TypeScript/.NET | Privacy-first manager with built-in alias mail |
| 9 | [padloc/padloc](https://github.com/padloc/padloc) | 2,941 | AGPL-3.0 | JavaScript | Multi-package manager incl. Tauri desktop |
| 10 | [passbolt/passbolt_api](https://github.com/passbolt/passbolt_api) | 6,212 | AGPL-3.0 | PHP | Team credential server with strict API design |
| 11 | [Infisical/infisical](https://github.com/Infisical/infisical) | 30,307 | mixed | TypeScript | Secrets platform: rotation and audit trails |
| 12 | [micr0-dev/wellness-ping](https://github.com/micr0-dev/wellness-ping) | 163 | none declared | Go | Minimal check-in service |

Four commercial products were reviewed for behaviour only (no code reuse): Google Inactive Account
Manager, SafeBeyond, Cipherwill, Everplans.

## 2. What we actually adopted, and where it lives in this codebase

Every row below is implemented, not aspirational. The file column is the proof.

| Idea | Source | How it landed here |
|---|---|---|
| Three-stage confirmation before anything is delivered | LastSignal, SafeBeyond | `notify.rs` — you → emergency contact → executor, each with its own wait window |
| Shamir secret sharing so the family can open the vault without the owner | Inheriti, LastSignal | `heir_recovery.rs` — 3-of-5 GF(256), Base32 shares, SHA-256 checksum per share |
| A printable emergency kit readable without the app | KeePassXC | `report.rs` + `action_guide.rs` — self-contained HTML, open formats |
| Bring-your-own-hostile-code guardrails for attachments | Aeterna | attachments never leave the machine; exports are plain files |
| Check-in that works over ordinary email | LastSignal, wellness-ping | `imap_checkin.rs` — replying to a mail counts as a check-in |
| Multi-party authorisation with an audit trail | digital-legacy-guardian (Python, 4★) | review pipeline: AI may pre-review, only a human verdict approves |
| A switch that does not depend on this machine staying up | deadcheck | `heartbeat.rs` + `server/heartbeat-server.mjs` — zero-content token ping |
| Vacations must not trigger a false alarm | LastSignal, GoodTrust guides | `set_dms_pause` — date-based pause, auto-expires |
| Annual re-review of a plan that silently goes stale | Everplans | `record_planning_review` + dashboard issue row |
| Record *where* the backup and recovery code physically are | Everplans | `db_custody.rs` — missing custody becomes a blocker |
| Per-asset beneficiary rather than one global heir | Cipherwill | `db_heirs.rs` — multiple heirs + one executor + per-asset disposal |
| Honest single-node boundary instead of pretending to be HA | LastSignal, Posthumous | README and in-app copy state the machine must be running |

## 3. What we deliberately did **not** copy

| Idea | Source | Why not |
|---|---|---|
| Hosted key escrow | Cipherwill, GoodTrust | Breaks the core promise: the data never leaves your machine |
| Beneficiary-initiated request with a timer veto | AbsentKey | Needs an always-online relay; offline-first has nowhere to receive the request |
| Blockchain attestation | Inheriti | Adds a trust assumption nobody can verify, solves nothing for the family |
| Daily check-in cadence | Snug Safety | Wrong frequency for estate planning; 30-day default with 1–365 range covers it |
| Federated multi-node | Posthumous | Requires running several servers — outside the target user's ability |
| Model-driven autonomous actions | Just In Case | AI here may only pre-review; never approve or execute |

## 4. What the ecosystem review changed in v0.5.0

Three gaps were obvious only after reading the others' code, not their homepages:

1. **Every serious project ships a non-GUI entry point.** Bitwarden has `bw`, Vaultwarden is
   API-first, KeePassXC ships `keepassxc-cli`. A planner that can only be driven by clicking is a
   plan nobody can script, back up or verify from a server. → `digilegacy-cli`.
2. **Every serious project separates transport from core.** Passbolt's API server and Vaultwarden's
   Rocket routes are thin adapters over one service layer. Our command layer was welded to Tauri's
   `AppHandle`, which is exactly the coupling that forces a second implementation later. →
   `paths.rs` + `dispatch.rs` + `bootstrap.rs`; the same command set now serves desktop, CLI and HTTP.
3. **Read/write must be a deployment decision, not a UI toggle.** Passbolt's permission model and
   Infisical's scoped tokens both treat write capability as something granted at the boundary.
   → the HTTP server is read-only unless started with `--allow-write`, and key/outbound commands
   are refused over HTTP regardless.

## 5. Where this project is genuinely different

- Competitors ask you to trust a company's continued existence. Here the company is irrelevant:
  the data is a file on your disk.
- Competitors deliver messages. This delivers a **runnable checklist** — per-platform documents,
  procedure, expected duration, difficulty, official links, plus a rehearsal that tells you where
  your family would get stuck.
- Competitors hide the single point of failure. This one prints it: if the machine is off, nothing
  is evaluated — and every exported document says so.

## 6. Honest limitations

- The policy library ships 152 platforms; 147 of them are explicitly marked *unverified* and 5 have
  a source but no review date. `check-policy-provenance.mjs` keeps that ratio from getting worse; it
  does not make the data verified.
- The dead man's switch evaluates only while the app (or the heartbeat server you host) is running.
- Share-splitting protects the *vault key*; it does not protect a family from losing all five paper
  shares. The recovery drill exists to make that risk concrete before it matters.
