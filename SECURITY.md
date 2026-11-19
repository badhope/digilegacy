# Security Policy

## Reporting a vulnerability

**Do not open a public issue for a security problem.**

Report privately through GitHub's [Security Advisory](https://github.com/badhope/digilegacy/security/advisories/new) form. If that is unavailable, open a minimal public issue asking for a private channel — without details — and a maintainer will follow up.

Please include: affected version, operating system, reproduction steps, and the impact you believe it has. We aim to acknowledge within 7 days. There is no bug bounty.

## Supported versions

The project is at `0.7.x` (M0). Only the latest release receives security fixes.

## Threat model

DigiLegacy is an **API-first digital legacy platform**, not a local-only desktop app. The platform owns data, permissions, approvals, audit and exports; Web, CLI, MCP and Agent are entry points.

### M0 boundary

M0 is an engineering shell and does **not** claim production-grade encryption:

- Web uses generated mock data by default.
- `core/security` and `core/store` are boundaries only.
- Domain commands return `UNSUPPORTED` rather than fake data.
- HTTP write commands return `401 AUTH_REQUIRED` until a scoped token exists.
- `key` / `network` / `desktop` commands are blocked on HTTP.

### In scope (target design)

| Threat | Mitigation |
|---|---|
| Theft of the database/backup | Sensitive fields are encrypted before write; key material stays in process memory, never in logs or the WebView. M1 migrates the legacy vault, without rewriting the algorithm. |
| Reading the SQLite file without the app | Field-level ciphertext and authenticated encryption; decryption refused while locked. |
| Credential disclosure after lock | Decryption refused at the Rust layer, not just hidden by a UI overlay. |
| Tampering with ciphertext | Authenticated encryption; modified ciphertext fails to decrypt. |
| Report/toolkit leaking credentials | Reports and toolkits never contain credential fields; exports are owner-only. |
| Malicious or malformed IPC/HTTP input | Per-command size caps, enum/bounds validation, and schema validation before writes. |
| Agent self-approval | `write` tools require human approval; `key`/`network`/`desktop` never reach Agent/MCP. |
| SMTP header injection | Strip CR/LF from headers; body dot-stuffing. |
| Heartbeat endpoint exposed | Bearer admin token, fail closed when unset; bounded bodies; atomic state writes. |

### Out of scope

- Malware running as the same OS user. It can read process memory or wait for unlock.
- Someone who knows the master password/recovery key.
- Plaintext exports. Reports and toolkits are ordinary files once written.
- A compromised build/release pipeline. Verify release checksums.

## Current migration note

The old application source has been removed. Security-critical behavior lives only in `core/security` and `core/store`; do not reintroduce old application code or link it into new entry points.
