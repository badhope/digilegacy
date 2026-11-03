# Security Policy

## Reporting a vulnerability

**Do not open a public issue for a security problem.**

Report privately through GitHub's [Security Advisory](https://github.com/badhope/digilegacy/security/advisories/new) form. If that is unavailable, open a minimal public issue asking for a private channel — without details — and a maintainer will follow up.

Please include: affected version, operating system, reproduction steps, and the impact you believe it has. We aim to acknowledge within 7 days. There is no bug bounty.

## Supported versions

The project is at `0.2.x`. Only the latest release receives security fixes.

## Threat model

DigiLegacy is a **local-first desktop application**. Being explicit about what it does and does not defend against is more useful than a vague promise.

### In scope

| Threat | Mitigation |
|---|---|
| Theft of the database file (backup, disk image, stolen laptop) | Sensitive fields are AES-256-GCM encrypted before they are written. The key is derived from the master password with PBKDF2-SHA256 (600,000 iterations) and exists only in process memory. A plaintext (pre-encryption) backup imported into an encrypted vault is refused while locked and field-encrypted while unlocked. |
| Someone reading the SQLite file without the app | Same as above: `account_identifier`, `account_description`, `heir_name`, `heir_contact`, `notes`, `credential_enc`, and all `user_profile` fields are stored as `enc:v1:` ciphertext. |
| Credential disclosure after the app is locked | Decryption is refused at the Rust layer while locked, not merely hidden by the UI overlay. |
| Tampering with ciphertext | AES-GCM authenticates; a modified ciphertext fails to decrypt. |
| Report/toolkit leaking credentials | Reports and toolkits never contain credential fields. Only the encrypted backup carries `credential_enc`, and only as ciphertext. Exported backups, reports and the temporary SMTP netrc are written owner-only (`0600` on Unix). |
| Malicious or malformed IPC input | Per-command size caps (200 MB for ZIP, 20 MB for report HTML, 8 MB for backups and policy patches, 64 KB for a single credential, 8 KB per asset text field, 500 chars per profile field), finite/non-negative value bounds, 1–365 day check-in interval, and category/disposal enum validation before every write — including `set_disposal_wish`, which bypasses the asset create/update path. |
| SMTP header injection | `\r`/`\n` are stripped from every header field; the body is dot-stuffed. |
| Remote code execution via "open platform website" | Only `http`/`https` URLs with a host are accepted; the URL is handed to the OS shell handler, never evaluated in the WebView. |
| Heartbeat server exposed to the internet | `/check` requires a bearer admin token (503 when unset — fail closed); unregistered tokens are rejected; request bodies are capped and state writes are atomic. |

### Out of scope

- **Malware running as the same user.** A process with your privileges can read process memory, hook the UI, or wait for you to unlock. No local application can prevent this.
- **Someone who knows your master password.** Encryption protects data at rest, not against a holder of the key.
- **Plaintext exports.** Reports and toolkits are ordinary files; once written they are as private as a printed page.
- **The recovery key.** It is a second key to the same vault. Store it with the same care as the master password.
- **A compromised build pipeline.** Verify release artifacts against the checksums published with each release.

## Cryptography details

| Property | Value |
|---|---|
| Cipher | AES-256-GCM |
| Nonce | 12 random bytes per encryption, prepended to the ciphertext |
| Field format | `enc:v1:` + base64(nonce ‖ ciphertext) |
| KDF | PBKDF2-HMAC-SHA256 |
| Iterations | 600,000 (v2, current); 100,000 (v1, read-only compatibility) |
| Salt | 16 random bytes, self-describing prefix `v2.` |
| Recovery seal | Master key encrypted under a key derived from a 24-character recovery code; `RS2.` prefix selects the v2 iteration count. Backup format v3 exports the seal so a recovery code keeps working after moving machines; importing a v1/v2 backup clears the stale local seal. |
| Key lifetime | In-memory only, wrapped in `Zeroizing`; cleared on lock and on idle timeout (10 minutes) |

New vaults always use v2 parameters. Older vaults keep unlocking because the salt and seal formats are self-describing.

## What to do if you find a leaked secret

If you believe a token, key, or credential has been committed, treat it as compromised: rotate it at the provider first, then remove it from history. Rewriting Git history does not un-leak a secret that has already been pushed.
