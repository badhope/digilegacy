# Verification

English | [简体中文](zh-CN/VERIFICATION.md)

Every change runs through the same gates. The goal is not a green badge; it is confidence that the contract, the data and the user path still agree.

## One command

```bash
./tools/verify.sh
```

This runs the contract check, architecture rules, Rust formatting and linting, the Rust workspace tests, Web typecheck and tests, the Web production build and bundle budgets.

Useful individual commands:

```bash
node tools/generate.mjs --check
node tools/arch-check.mjs
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cd web && npm run typecheck && npm test && npm run build
node tools/bench/web-budget.mjs
```

## What each gate protects

| Gate | Question it answers |
|---|---|
| Contract generation | Did a command, tool, schema or error drift between clients? |
| Architecture check | Did an entry point grow business logic, or a removed path return? |
| Rust format and lint | Is the backend mechanically clean? |
| Rust tests | Do domain rules, storage upgrades and entry points still behave? |
| Fuzz smoke | Do seeded malformed inputs panic a parser or leak markup? |
| Web typecheck | Do generated types and components agree? |
| Web tests | Do cache, routing and page states still work? |
| Web accessibility | Do every route and modal layer pass the axe WCAG A/AA rules, and do light/dark text tokens hold at least 4.5:1? |
| Visual regression (manual) | Do nine fixed-viewport snapshots still match the committed baseline, including dark and mobile overview? |
| Production build | Does the app compile without hidden runtime-only imports? |
| Bundle budget | Did a change quietly make the main experience heavier? |
| Schema diagnostics | Does a fresh database open at the expected version? |

## Test layers

- **Domain tests** cover state transitions, validation and provenance rules.
- **Store tests** open a database, apply migrations and verify the resulting schema.
- **Application tests** exercise use cases without HTTP.
- **Network guard tests** cover metadata/private/link-local URL rejection, loopback allowance and signed policy-feed transport against a local HTTP server.
- **SMTP TLS tests** start local STARTTLS and implicit TLS servers with a test certificate, then verify the full mail dialog and UTF-8 body.
- **Fuzz smoke tests** use fixed seeds and 45,000 inputs across five public boundaries: URL validation, recovery seals, paper shares, KDF parameters and HTML escaping. They are deterministic regression tests, not coverage-guided fuzzing.
- **Server tests** cover transport behavior and error mapping.
- **Web tests** cover cache behavior, routing, page states, modal focus containment, the mobile navigation drawer, and axe WCAG A/AA scans of the app shell, the three dialogs and all fifteen feature routes.
- **Route accessibility tests** mount each view against the real router, settle its data, then run axe on the rendered body. A failing route reports the violation ID and help text, so a new page cannot regress silently.
- **Contrast tests** parse the light and dark design tokens and require at least 4.5:1 for 15 foreground/background pairs in each theme. This supplements axe, which cannot compute colour contrast reliably in jsdom.
- **Manual smoke** checks the path a person actually clicks.
- **Visual regression** captures nine fixed-viewport WebP shots against a mock build with a frozen clock, locale and theme. It runs outside `tools/verify.sh`; see [tools/visual/README.md](../tools/visual/README.md).

## Manual smoke before release

1. Start the API and the Web console with a fresh data directory.
2. Create an asset, edit it, and confirm the detail page and delivered report show the same value.
3. Configure an heir and complete a check-in.
4. Ask the Agent to draft a plan; confirm a write step waits for approval.
5. Approve one write and reject another with a reason; confirm both outcomes are audited.
6. Sync a policy feed and confirm the library is unchanged until a candidate is approved.
7. Open the mobile layout and the dark theme; check for overflow and unreadable text.
8. On a narrow viewport, open the navigation drawer with the keyboard: focus should enter the drawer, `Escape` should close it and return focus to the menu button.
9. Open the tools page and move through the search field and permission filter with the keyboard; both should announce a name, and the filter should expose its current value.
10. Build the mock console, start the preview server and run `python3 tools/visual/regress.py`; all nine shots should stay within the 0.4% pixel budget.

## Release checklist

- [ ] `./tools/verify.sh` passes.
- [ ] The changelog describes user-visible changes.
- [ ] The English and Chinese docs describe the same behavior.
- [ ] No secret, token or local path is committed.
- [ ] The generated artifacts are current.
- [ ] A backup and restore path has been tested when storage changed.

The detailed Chinese verification record keeps the historical evidence for each delivery. The English page describes the gates and the expected behavior, not a running tally.
