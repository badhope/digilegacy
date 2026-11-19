# ADR-0001: Contracts are the source of truth

- Status: Accepted
- Date: 2026-11-19

## Context

The same capability is exposed through the Web console, CLI, HTTP API and MCP. Hand-written interfaces drift quickly: a permission changes in one entry point, a field is renamed in another, and the UI keeps a stale mock.

## Decision

Commands, tools, errors and schemas are declared once in `contracts/`. A generator produces the Rust command tables, HTTP routes, OpenAPI document, CLI metadata, MCP catalog and TypeScript client types. Generated files are never edited by hand. CI runs the generator in check mode, so a missing propagation fails before merge.

## Alternatives considered

- Hand-maintain each client. Rejected: drift is guaranteed and review cannot see all consumers.
- Generate only the Web client. Rejected: the backend and Agent would still have separate truths.
- Use an external schema registry. Rejected for now: it adds operational weight without solving the in-repo consistency problem.

## Consequences

Interface changes have a visible, reviewable path. A new command costs a contract entry plus implementation, not four unrelated edits. The generator becomes part of the critical path and must stay fast and deterministic.

## Review trigger

Revisit if the contract generator cannot express a needed transport, or if a second product needs a versioned public API with independent release cadence.
