## What changed

<!-- One paragraph. Link the issue if there is one. -->

## Why

<!-- The behaviour that was wrong, or the gap being filled. -->

## Verification

Paste the commands you actually ran and their results. If a check was skipped, say so and why.

| Check | Command | Result |
|---|---|---|
| Typecheck | `npx vue-tsc --noEmit` | |
| Unit tests | `npm test` | |
| Coverage gate | `npm run test:coverage` | |
| E2E | `npx playwright test` | |
| Rust tests | `cargo test --locked` | |
| Clippy | `cargo clippy --all-targets -- -D warnings` | |

## Checklist

- [ ] A test fails without this change and passes with it (for bug fixes)
- [ ] No secret, token, phone number, or real account identifier appears in code, tests, logs, or screenshots
- [ ] No new dependency, or the bundle-size / build-time cost is explained above
- [ ] Documented numbers (test counts, table count, command count, platform count) updated everywhere they appear
- [ ] `CHANGELOG.md` updated under `[Unreleased]` if the change is user-visible
- [ ] New UI text is reachable by keyboard and has an accessible name
