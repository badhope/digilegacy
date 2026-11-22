# Release Checklist

Release: **0.0.34** (Linux x86_64, 2026-11-22)

Status: **engineering evidence complete; human sign-off not yet given.**

The archive checksum is in `packages/digilegacy-0.0.34-linux-x86_64.tar.gz.sha256`
and inside the archive as `SHA256SUMS`. Do not copy a checksum from a chat
message; verify the file next to the archive.

## Automated evidence

| Gate | Result | Evidence |
| --- | --- | --- |
| Contract / architecture / format / clippy | pass | `./tools/verify.sh` exit 0 |
| Rust + Web tests | **211 Rust + 99 Web** | `./tools/verify.sh` |
| Contracts | **49 commands / 23 tools / 67 schemas / 21 errors** | `./tools/generate.mjs --check` |
| Schema | **v19 / 32 tables** | health + diagnostics |
| Bundle budget | entry 87.36/88 KB; vendor 101.41/130 KB; route max 22.48/24 KB; JS 120.97/121 KB; CSS 22.12/23 KB; total 143.09/144 KB gzip | `web-budget` |
| Live Chromium + axe | 18/18 routes PASS, 0 axe violations, 0 failures | `measurements/live-browser-0.0.34-2026-11-22.json` |
| Asset depth UX | six records, kind filter, deep form, save-and-continue, draft restore, archived delete; list 6→5, GET 404, audit row | `measurements/ux-asset-depth-0.0.34-2026-11-22.json` |
| Clean-host install | 12/12 PASS; reinstall reads preserved DB | `measurements/install-0.0.34-2026-11-22.json` |
| Archive | `packages/digilegacy-0.0.34-linux-x86_64.tar.gz`; SHA256 in the sidecar `.sha256`, archive also carries `SHA256SUMS` | `...tar.gz.sha256` |

## Operational baselines carried forward

These drills were measured on the 2026-11-20 code line and were not rerun for
0.0.34. The asset schema and UI changes do not touch the outbound proxy,
capacity, restore or DNS-pinning paths; treat them as baseline evidence.

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
