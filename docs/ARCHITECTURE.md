# Architecture

English | [简体中文](zh-CN/ARCHITECTURE.md)

DigiLegacy is a modular monolith with several entry points and one command layer. The design favors explicit boundaries over premature service splitting.

## System shape

```text
┌──────────────┬──────────────┬──────────────┬──────────────┐
│ Web console  │ CLI          │ MCP server   │ HTTP clients │
└──────┬───────┴──────┬───────┴──────┬───────┴──────┬───────┘
       │              │              │              │
       └──────────────┴──────┬───────┴──────────────┘
                             │
                    Platform API / CommandBus
                             │
                 ┌───────────┴───────────┐
                 │ application services  │
                 └───────────┬───────────┘
                             │
        ┌──────────┬─────────┼─────────┬──────────┐
        │ domains  │ security│ store   │integrations
        └──────────┴─────────┴─────────┴──────────┘
                             │
                    audit · jobs · approvals
```

The entry points do transport, authentication and presentation. They do not own business rules. A single dispatcher routes commands to application services, which use domain modules and infrastructure crates.

## Contracts first

Every command, tool, error code and input/output schema is declared in `contracts/`. The generator produces:

- Rust command and error tables
- the HTTP route manifest and OpenAPI document
- CLI command metadata
- the MCP tool catalog
- TypeScript types, IPC bindings and mock data for the Web client

Generated files are never edited by hand. `node tools/generate.mjs --check` is part of the release gate, so an interface change that is not propagated fails before merge.

A command declares its permission class: `read`, `draft`, `write`, `key`, `network` or `desktop`. HTTP opens read/draft, protects write with a bearer token, and always rejects the last three. MCP only exposes read/draft tools.

## Application and domains

`core/application` coordinates use cases: load state, validate inputs, call domain functions, persist changes and append audit records. It is the only layer that knows how a user-facing operation is composed.

`core/domains` contains the rules that must stay true regardless of entry point: asset ownership, heir relationships, trigger state transitions, policy provenance, sync candidates and approval semantics. Domain code does not know about HTTP, Vue or CLI arguments.

`core/integrations` is the boundary for external systems: HTTP feeds, SMTP, LLM providers, webhooks and connection probes. Network access is isolated here and returns structured results rather than leaking transport errors into domain code.

## Data and security

SQLCipher protects the database at rest. Sensitive values are encrypted again at the field level before storage. Key material is derived and kept in process memory; it is never written to logs, generated reports or browser storage.

The vault supports recovery codes, Shamir shares, authenticated backups and checkpoints. Destructive operations require explicit confirmation and produce an audit event. Audit records form a hash chain so later changes can be detected.

## Agent model

An Agent run is a durable state machine:

1. A planner proposes steps using registered tools.
2. Read-only steps execute immediately.
3. A write step creates an approval and pauses the run.
4. A human approves or rejects with a reason.
5. The run resumes, completes or stops; every transition is recorded.

Approvals are first-class records, not a UI flag. A background worker resumes approved runs and reclaims stale jobs. The Agent cannot approve its own request, and it cannot reach key, network or desktop commands.

## Storage and migrations

Schema changes are versioned in `core/store/src/schema.sql` and applied through a migration ledger. Every migration has a name, a version and tests that open an older database and verify the upgrade. Runtime diagnostics expose the current schema version and table count without exposing data.

## Frontend

The Web console is a Vue 3 application with five product areas and a global settings surface. Routes are lazy-loaded; shared data access goes through a typed platform client. Reads use a short-lived cache with request coalescing; writes invalidate the affected domains so the next screen is fresh. The UI is designed for desktop and mobile, with light and dark themes.

## Observability and operations

The server exposes a health endpoint, structured command envelopes, trace IDs and an audit query surface. Background jobs have explicit states and error codes. Connection probes report health without returning credentials. Diagnostics can produce a support bundle that excludes secrets.

## Deployment

A deployment needs the Rust server, a writable data directory and an API token. The Web console can be served by any static host and points at the Platform API. CLI and MCP run locally against the same database or API. SQLite/SQLCipher keeps the default deployment small; the command layer can later be hosted behind a shared service without changing clients.

## Non-goals

- Microservices for their own sake.
- A second backend for the Agent.
- Editing generated clients by hand.
- Hiding failures behind optimistic UI.
- Treating local deployment as a separate product.

See [architecture decisions](adr/) for the reasoning behind these choices.
