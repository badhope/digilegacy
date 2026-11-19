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
| Web typecheck | Do generated types and components agree? |
| Web tests | Do cache, routing and page states still work? |
| Production build | Does the app compile without hidden runtime-only imports? |
| Bundle budget | Did a change quietly make the main experience heavier? |
| Schema diagnostics | Does a fresh database open at the expected version? |

## Test layers

- **Domain tests** cover state transitions, validation and provenance rules.
- **Store tests** open a database, apply migrations and verify the resulting schema.
- **Application tests** exercise use cases without HTTP.
- **Network guard tests** cover metadata/private/link-local URL rejection, loopback allowance and signed policy-feed transport against a local HTTP server.
- **Server tests** cover transport behavior and error mapping.
- **Web tests** cover cache behavior, routing and important page states.
- **Manual smoke** checks the path a person actually clicks.

## Manual smoke before release

1. Start the API and the Web console with a fresh data directory.
2. Create an asset, edit it, and confirm the detail page and delivered report show the same value.
3. Configure an heir and complete a check-in.
4. Ask the Agent to draft a plan; confirm a write step waits for approval.
5. Approve one write and reject another with a reason; confirm both outcomes are audited.
6. Sync a policy feed and confirm the library is unchanged until a candidate is approved.
7. Open the mobile layout and the dark theme; check for overflow and unreadable text.

## Release checklist

- [ ] `./tools/verify.sh` passes.
- [ ] The changelog describes user-visible changes.
- [ ] The English and Chinese docs describe the same behavior.
- [ ] No secret, token or local path is committed.
- [ ] The generated artifacts are current.
- [ ] A backup and restore path has been tested when storage changed.

The detailed Chinese verification record keeps the historical evidence for each delivery. The English page describes the gates and the expected behavior, not a running tally.
