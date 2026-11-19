## Summary

<!-- One sentence describing the user-visible change. -->

## Scope

- Module / command:
- Page / entry point:
- ADR / RFC:
- Issue:

## Type

- [ ] feat
- [ ] fix
- [ ] refactor
- [ ] docs
- [ ] test
- [ ] chore

## Contract and permissions

- [ ] `contracts/*` changed and `node tools/generate.mjs` was run
- [ ] New commands have input/output schemas
- [ ] Write tools use `approval=required`
- [ ] MCP does not expose key/network/desktop tools
- [ ] Every frontend action maps to a command in the manifest

## Verification

- [ ] `./tools/verify.sh` passes
- [ ] Rust format, clippy and tests pass
- [ ] Web typecheck, tests and build pass
- [ ] Loading, empty and error states were checked
- [ ] Bundle budget did not regress beyond the allowed threshold

## Data and security

- [ ] No new secret or sensitive value is committed
- [ ] Database changes include migration and rollback
- [ ] Write actions have an audit path
- [ ] Old application code was not copied into the new build

## Screenshots / recordings

<!-- For UI changes, attach the key screen. Do not just say "UI updated". -->

## Not covered

<!-- State what is intentionally left out. -->
