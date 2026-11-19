# Contributing to DigiLegacy 归零

Thanks for considering a contribution. This project handles data people care about after they die — correctness and honesty matter more than speed.

## Ways to contribute

| Kind | Where | Notes |
|---|---|---|
| Policy corrections | `digilegacy/src-tauri/src/platform_data.rs` | Every entry carries `data_source`. A correction without a citable source will not be merged. |
| Platform additions | same file + `digilegacy/src/api/mock-platforms.json` | The mock seed is used by the browser preview and E2E; keep both in sync. |
| Bug reports | GitHub Issues | Include version, OS, reproduction steps, and the exact error text. |
| Translations | currently none — see Roadmap in README | The UI is Simplified Chinese only; an i18n framework has not been chosen yet. |
| Documentation | `README.md`, `README.zh-CN.md`, `docs/` | English and Chinese docs must be updated together; screenshots and the demo video are generated, not hand-drawn (see below). |

## Development setup

Prerequisites: Node 22.22.2 (see `.nvmrc`), Rust stable, and the Tauri system dependencies for your OS.

```bash
cd digilegacy
npm ci
npm run tauri:dev        # desktop app with hot reload
npm run dev              # browser preview only (uses src/api/mock.ts, no real encryption)
```

Run the full local gate before opening a pull request:

```bash
bash scripts/verify.sh --with-e2e
```

It runs typecheck, unit tests with coverage thresholds, production build, the static demo build, the heartbeat-server behaviour tests, Playwright E2E, `cargo fmt --check`, `cargo clippy -D warnings`, and `cargo test --locked`. The script records each command's exit code explicitly — do not replace it with `cmd | tail`, which reports the exit code of `tail`.

## Rules that are not negotiable

1. **No fabrication.** Do not claim a test passed if it did not run. If a check is skipped, say so in the pull request description.
2. **Fail closed on security paths.** When the vault is locked, credential reads must error rather than return empty strings — an empty string is indistinguishable from "the user never saved one".
3. **Never weaken the encryption boundary.** No plaintext passwords in logs, exports, error messages, or test fixtures. New fields that hold personal data must go through `vault::encrypt_if_needed` / `decrypt_if_needed`.
4. **Every bug fix ships with a test that fails before the fix.** Mutation-style reasoning: if you revert your fix, the new test must go red.
5. **Do not add a dependency to save a few lines.** Prefer the standard library or something already installed. If a new dependency is genuinely required, explain the bundle-size and build-time cost in the pull request.
6. **Keep user data readable without this app.** Reports and toolkits are open HTML/JSON/ZIP on purpose.

## Commit messages

Conventional commits, imperative mood, Chinese or English:

```
fix(vault): 档案重加密漏字段导致启用主密码时 panic
feat(report): 导出失败时显示错误而不是静默
docs(readme): 修正命令数量与表数量
```

Scope is the module (`vault`, `db`, `report`, `profile`, `ci`, `docs`). Keep the subject under 72 characters; put the reasoning in the body.

## Pull requests

- One logical change per pull request. Refactors that are not required by the change belong in a separate pull request.
- Update `CHANGELOG.md` under `## [Unreleased]` for user-visible changes.
- If you change a documented number (test count, table count, command count, platform count), update every place that states it — `README.md`, `README.zh-CN.md`, `docs/index.html`, `docs/zh/index.html`, and `docs/VERIFICATION.md` are cross-checked by hand.
- The website is bilingual: `docs/index.html` (English) and `docs/zh/index.html` (Chinese) share `docs/site.css`; the nav language switch must keep working in both directions.

## Regenerating screenshots and the demo video

```bash
cd digilegacy
npm run dev                     # keep running in another terminal
node scripts/demo-media.mjs     # docs/screenshots/*.png @2x + docs/media/demo-walkthrough.webm
```

The script drives Playwright (already a dev dependency): it seeds a demo state that looks like a real, unlocked vault, asserts each page's header title before capturing, then records a scripted walkthrough. Convert the recording to MP4 with the bundled ffmpeg:

```bash
ffmpeg -i docs/media/demo-walkthrough.webm -c:v libx264 -crf 24 -pix_fmt yuv420p        -movflags +faststart docs/media/demo-walkthrough.mp4
```

Do not commit screenshots taken at a different viewport.

## Rebuilding the web demo

```bash
cd digilegacy
npm run build:demo      # VITE_DEMO=1 → base /digilegacy/demo/, hash routing
rm -rf ../docs/demo && cp -r dist ../docs/demo
```

The demo uses `src/api/mock.ts` (localStorage, unencrypted) and shows a banner saying so. Never point real credentials at it.

## License

By contributing you agree that your contribution is licensed under the MIT License (see `LICENSE`).
