# Release Checklist

Release: **0.0.19** (Linux x86_64, 2026-11-20)

Status: **engineering evidence complete; human sign-off not yet given.**

The archive checksum is in `packages/digilegacy-0.0.19-linux-x86_64.tar.gz.sha256`
and inside the archive as `SHA256SUMS`. Do not copy a checksum from a chat
message; verify the file next to the archive.

## Automated evidence

| Gate | Result | Evidence |
| --- | --- | --- |
| Contract / architecture / format / clippy | pass | `./tools/verify.sh` exit 0 |
| Rust + Web tests | **184 Rust + 79 Web** | `./tools/verify.sh` |
| Contracts | **44 commands / 23 tools / 60 schemas / 21 errors** | `./tools/generate.mjs --check` |
| Schema | **v16 / 31 tables** | health + diagnostics |
| Bundle budget | entry 80.75/82 KB; JS 104.95/105.2 KB; CSS 20.22/21.1 KB; total 125.17/125.5 KB gzip | `web-budget` |
| Live Chromium + axe | 14/14 routes pass, 0 axe violations | `measurements/live-browser-0.0.19-2026-11-20.json` |
| Visual baselines | 9/9 within 0.4%; max overview-mobile 0.173% | `tools/visual/regress.py` |
| Vault recovery + rate limit | locked 423; wrong 400; unlock 200; relock; recovery 200; assets 200; five failures end in 429; correct password still 429 | `measurements/security-recovery-0.0.19-2026-11-20.json` |
| Vault lock API + CLI | 423 → 400 → 200 → 200 → 423; CLI blocked without password env | `measurements/security-lock-0.0.18-2026-11-20.json` (0.0.18 baseline) |
| Clean-host install | 12 steps PASS; reinstall reads preserved DB | `measurements/install-0.0.19-2026-11-20.json` |
| Archive | `packages/digilegacy-0.0.19-linux-x86_64.tar.gz`, 18,293,344 bytes, SHA256 `53f86e570d59571dccfd27c0df789af665978551f4d413d61c14296cb805e8ee` | `...tar.gz.sha256` |

## Operational baselines carried from 0.0.18

These drills were measured on the 0.0.18 code line and were not rerun for this
patch. The recovery and rate-limit changes do not touch the outbound proxy,
capacity, restore, DNS-pinning or policy-source paths; treat them as baseline
evidence, not fresh 0.0.19 measurements.

| Gate | Baseline | Evidence |
| --- | --- | --- |
| Capacity | reads p95 9.54 ms, writes p95 3.70 ms | `measurements/capacity-2026-11-20.json` |
| Restore | healthy in 158 ms, 10,120 valid audit rows | `measurements/restore-2026-11-20.json` |
| DNS rebinding | 3/3 PASS, one A query, rebound answer rejected | `measurements/dns-rebind-2026-11-20.json` |
| nginx same-origin | `/`, SPA fallback, `/api`, asset 200; headers correct | `measurements/proxy-2026-11-20.json` |
| Policy source precheck | 152/152 need human review; 150 roots, 2 deep links | `measurements/policy-sources-2026-11-20.json` |

## Human gates still open

- [ ] Real SMTP/Webhook delivery latency: the account owner fills
  `/settings/connections` and sends a real test; local protocol tests do not
  measure provider delivery.
- [ ] Human screen-reader pass and light/dark visual review: automated axe and
  screenshots do not replace a person.
- [ ] Policy source verification: 152 rows remain `unverified`; the worklist is
  a starting point, not proof.
- [ ] Release owner sign-off below.

## Sign-off

| Role | Name | Decision | Date |
| --- | --- | --- | --- |
| Release owner |  |  |  |
| Security/data reviewer |  |  |  |
| Content owner (policy) |  |  |  |

Sign only after the open human gates above are either completed or explicitly
accepted as release limitations. Until then this is a candidate, not a signed
release.
