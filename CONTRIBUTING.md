# Contributing

English | [简体中文](docs/zh-CN/CONTRIBUTING.md)

DigiLegacy is contract-first. When you change a product capability, change the contract first, then the implementation, then the UI.

## Before you start

- Read [positioning](docs/POSITIONING.md), [architecture](docs/ARCHITECTURE.md) and [verification](docs/VERIFICATION.md).
- Identify the domain, module and command affected by the change.
- A new product capability starts as an RFC, then lands in `contracts/`.
- Implement new behavior in `core/`, `apps/` or `web/`; do not revive the removed desktop build.

## Contract rules

1. `contracts/commands.json`, `tools.json`, `errors.json`, `schemas/` and `mocks.json` are the source of truth.
2. Never edit generated files by hand. They are recreated from the contract.
3. After a contract change, run:

```bash
node tools/generate.mjs
node tools/generate.mjs --check
```

4. A new command needs an input/output schema, a permission class, transport declarations for HTTP/CLI/MCP, error references, mock data when the Web client needs it, and at least one architecture or behavior test.
5. A new tool references an existing command. Write tools require `approval=required`. MCP only exposes read and draft tools.

## Implementation rules

- Business logic belongs in `core/domains` or the relevant application crate, never in an entry point.
- All entry points dispatch through the same `CommandBus`; do not create a second backend.
- The Web client talks to the platform through typed commands. Mock mode is for local demos, not a parallel implementation.
- Errors use codes from `contracts/errors.json`; do not invent strings in transport code.
- Any write path considers permission, approval and audit together.
- Migrating security code must preserve algorithms and test vectors; do not rewrite crypto while moving it.

## Branches and commits

Use `feat/<module>-<short>`, `fix/<issue>-<short>` or `docs/<topic>`.

```text
feat(assets): add asset list cursor pagination
fix(agent): reject self-approved write runs
docs(adr): record contract-first decision
```

One pull request should answer one reviewable question. Keep refactors, formatting and behavior changes separate.

## Before merging

```bash
./tools/verify.sh
```

All gates must pass. In the pull request, state the module, command or page affected, the user-visible change, permission and data impact, the verification commands and results, and anything intentionally left out.

## Done means

- Code, contract, tests and docs ship together.
- No fake success state remains.
- UI changes cover loading, empty and error states.
- Database changes follow the safe order: inspect constraints, write a small sample, then re-check list, detail and download paths.
- The change matches the definition of done in [governance](docs/GOVERNANCE.md).
