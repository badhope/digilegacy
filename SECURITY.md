# Security policy

English | [简体中文](docs/zh-CN/SECURITY.md)

## Reporting a vulnerability

**Do not open a public issue for a security problem.**

Use GitHub's [private security advisory](https://github.com/badhope/digilegacy/security/advisories/new) form. If that is unavailable, open a minimal public issue asking for a private channel without technical details.

Include the affected version, platform, reproduction steps and the impact you believe it has. We aim to acknowledge reports within a week. There is no bug bounty.

## Supported versions

Only the latest release and the `main` branch receive security fixes. Older releases may be kept for reference but are not supported.

## Threat model

DigiLegacy is an API-first digital legacy platform, not a local-only desktop app. The platform owns data, permissions, approvals, audit and exports; Web, CLI, MCP and Agent are entry points.

### What is in scope

| Threat | Mitigation |
|---|---|
| Database or backup is stolen | Sensitive fields are encrypted before write; key material stays in process memory and never enters logs or browser storage. |
| The SQLite file is opened outside the app | Authenticated field-level encryption; decryption is refused while the vault is locked. |
| A credential leaks after lock | Decryption is refused in the Rust layer, not hidden by a UI overlay. |
| Ciphertext is modified | Authenticated encryption rejects modified ciphertext. |
| A report or toolkit leaks secrets | Reports and toolkits never contain credential fields; exports are owner-only. |
| Malformed HTTP, IPC or MCP input | Schema validation, size caps and enum/bounds checks before writes. |
| An Agent approves its own request | Write tools require human approval; key, network and desktop capabilities never reach the Agent or MCP. |
| Header injection or malformed mail | CR/LF stripped from headers; SMTP body is dot-stuffed. |
| A webhook or connection probe is abused | Bounded bodies, explicit timeouts, credential redaction and fail-closed configuration. |

### What is out of scope

- Malware running as the same OS user. It can read process memory or wait until unlock.
- Someone who already knows the master password or recovery material.
- Plaintext exports. A delivered report is an ordinary file once written.
- A compromised build or release pipeline. Verify release checksums.

## Security practices

- Secrets are encrypted before storage and are never returned by read models.
- Approval and audit records are append-only by design; a rejected action does not silently disappear.
- Generated clients and contracts are reviewed together so a permission change cannot be missed in one entry point.
- Dependency and release checks run in CI; a known critical issue blocks release.

## Migration note

Security-critical behavior lives in `core/security` and `core/store`. Do not reintroduce the removed desktop application or link its old code into a new entry point.
