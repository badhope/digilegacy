# Governance

English | [简体中文](zh-CN/GOVERNANCE.md)

The project is small enough to move quickly and sensitive enough that speed cannot come from skipping boundaries. These rules keep changes predictable.

## Principles

1. Define the user outcome, interface and acceptance criteria before implementation.
2. Keep one source of truth: commands, errors and schemas live in `contracts/`.
3. Entry points do not own business logic.
4. Irreversible changes leave a decision record.
5. Do not fake completion. An unsupported path fails explicitly.

## Roles

| Role | Responsibility |
|---|---|
| Product owner | User outcomes, priority and acceptance criteria |
| Architecture owner | Contracts, module boundaries, ADRs and cross-domain decisions |
| Module owner | Domain implementation, tests, migrations and rollback |
| Frontend owner | Entry points, loading behavior, accessibility and design system |
| Security and data owner | Vault, keys, backups, migrations and audit |
| Reviewer | Independent review of contracts, permissions, data impact and tests |

One person may hold several roles. The same Agent may not approve its own change.

## Definition of Ready

A change is ready when:

- The user outcome is concrete.
- The owning module and owner are known.
- Commands, tools and schemas are listed.
- The permission class is stated.
- Data impact is understood: tables, fields, migrations and rollback.
- Acceptance covers commands, UI states, errors and performance.
- External dependencies and risks are recorded.
- Irreversible changes have an ADR or RFC draft.

## Definition of Done

A change is done when:

- Business logic sits in domains or application code, not in an entry point.
- Errors use contract error codes.
- Write operations have permission, approval and audit boundaries.
- New behavior has unit or integration coverage.
- The UI covers loading, empty and error states.
- Generated artifacts are current.
- Architecture gates pass.
- Documentation and changelog are updated.
- No secret or sensitive value enters the work tree.
- `./tools/verify.sh` passes.

## Change classes

| Class | Examples | Required process |
|---|---|---|
| Local | Copy, styling, a single screen | Pull request and self-check |
| Contract | New command, tool or schema | Design note, generator update, consumer check |
| Data | Migration or field semantics | Change record, backup and rollback plan, second review |
| Security | Encryption, keys, recovery, permissions | ADR, security review and test vectors |
| Release | Versioning, signing, compliance | Release checklist and rollback rehearsal |

## ADRs and RFCs

An ADR records a decision that is hard to reverse. An RFC explores a proposal that is still open. Both live under `docs/adr/` and `docs/rfc/`. An ADR states context, decision, alternatives, consequences and the condition for revisiting it.

## Review focus

Reviewers look for a second source of truth, permission bypass, Agent self-approval, data loss, fake success states, mobile overflow, blocking UI and performance regressions. A passing test suite does not excuse a broken boundary.

## Release rhythm

Run the fast gates on every change, the full gate before merge, and a manual smoke on the paths a user will touch. Review budgets and open risks regularly. A milestone is not complete because the code exists; it is complete when the acceptance path works and the remaining gaps are written down.
