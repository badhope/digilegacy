# Release Checklist

Release: **0.0.17** (Linux x86_64, 2026-11-20)

Status: **engineering evidence complete; human sign-off not yet given.**

The archive checksum is in `packages/digilegacy-0.0.17-linux-x86_64.tar.gz.sha256`
and inside the archive as `SHA256SUMS`. Do not copy a checksum from a chat
message; verify the file next to the archive.

## Automated evidence

| Gate | Result | Evidence |
| --- | --- | --- |
| Contract / architecture / format / clippy | pass | `./tools/verify.sh` |
| Rust + Web tests | **175 Rust + 68 Web** | `./tools/verify.sh` exit 0 |
| Contracts | **39 commands / 23 tools / 56 schemas / 21 errors** | `./tools/generate.mjs --check` |
| Schema | **v16 / 31 tables** | health + diagnostics |
| Bundle budget | JS 103.00/103.1 KB, CSS 21.05/21.1 KB, total 124.05/124.2 KB gzip | `web-budget` |
| Live Chromium + axe | 15/15 routes pass, 0 axe violations | `measurements/live-browser-2026-11-20.json` |
| Visual baselines | 9/9 within 0.4% | `tools/visual/regress.py` |
| Capacity | reads p95 9.54 ms, writes p95 3.70 ms | `measurements/capacity-2026-11-20.json` |
| Restore | healthy in 158 ms, 10,120 valid audit rows | `measurements/restore-2026-11-20.json` |
| Clean-host install | 12 steps PASS, reinstall reads preserved DB | `measurements/install-2026-11-20.json` |
| DNS rebinding | 3/3 PASS, one A query, rebound answer rejected | `measurements/dns-rebind-2026-11-20.json` |
| nginx same-origin | `/`, SPA fallback, `/api`, asset 200; headers correct | `measurements/proxy-2026-11-20.json` |
| Policy source precheck | 152/152 need human review; 150 roots, 2 deep links | `measurements/policy-sources-2026-11-20.json`, `policy-source-worklist.csv` |

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
