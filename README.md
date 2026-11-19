# DigiLegacy

English | [简体中文](README.zh-CN.md)

**A self-hosted, API-first platform for planning and passing on digital assets.**

Digital legacy is usually scattered across notes, password managers and platform-specific rules. DigiLegacy brings it into one auditable workspace: what you own, who receives it, when it should be released, and what evidence was produced along the way.

[Documentation](docs/README.md) · [Architecture](docs/ARCHITECTURE.md) · [Contributing](CONTRIBUTING.md) · [Security](SECURITY.md)

## Why DigiLegacy

- **One source of truth.** Assets, heirs, trigger state, platform policies and deliverables live in the same data model.
- **Agentic, but not autonomous.** An Agent can read, plan and draft. Any action that changes data waits for a human decision.
- **Private by design.** Sensitive fields are encrypted before they reach the database. The platform can run on your own machine or server.
- **Auditable end to end.** Imports, approvals, policy changes and deliveries leave a trace you can inspect later.

## What it does

| Area | What you get |
|---|---|
| **Assets** | A structured inventory of accounts, subscriptions, devices and other digital property, with platform-specific inheritance notes. |
| **Planning** | Heirs, executor, emergency contacts and a release chain that can pause, resume and record check-ins. |
| **Delivery** | Human-readable reports, checklists, legal templates and a toolkit for the people who need to act. |
| **Automation** | An Agent workspace backed by a tool registry. Read-only tools run immediately; write tools pause for approval. |
| **Policy library** | Platform policies with provenance, plus a review queue for remote updates. Nothing is overwritten until a person approves it. |
| **Operations** | Encrypted connections, delivery channels, background jobs, approvals and an audit hash chain. |

## Architecture at a glance

```text
Web console        CLI        MCP        HTTP clients
      \             |          |            /
       \            |          |           /
        └────── Platform API / CommandBus ──────┘
                           |
                 domains and application
                           |
              store · security · integrations
                           |
                 audit · jobs · approvals
```

The Web app, CLI, MCP server and HTTP API all call the same command layer. A command is described once in `contracts/`, then generated into Rust, TypeScript, OpenAPI and tool manifests. That keeps the interfaces from drifting.

The backend is a modular Rust monolith. The frontend is Vue 3 with route-level code splitting. The database is SQLCipher, with field-level encryption for secrets and personal data.

## Quick start

Prerequisites: Rust, Node.js and a C toolchain for SQLCipher.

```bash
# 1. Generate the contract artifacts
node tools/generate.mjs

# 2. Start the API
DIGILEGACY_BIND=127.0.0.1:8787 \
DIGILEGACY_API_TOKEN=change-me \
cargo run -p digilegacy-server

# 3. Start the Web console
cd web
npm install
npm run dev
```

Open `http://127.0.0.1:5173`. Write actions use the token configured in **Settings → Connections**.

CLI and MCP are first-class local entry points:

```bash
cargo run -p digilegacy-cli -- health
cargo run -p digilegacy-cli -- commands --json

printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/list"}' \
  | cargo run -p digilegacy-mcp
```

## Repository layout

```text
core/          domain logic, contracts, storage, security, integrations
apps/          HTTP server, CLI and MCP entry points
web/           Vue console
contracts/     commands, tools, errors, schemas and generated OpenAPI
content/       policy data and translations
docs/          product, architecture, operations and delivery docs
tools/         generators, architecture checks and performance gates
```

## Documentation

- [Documentation index](docs/README.md)
- [Product positioning](docs/POSITIONING.md)
- [Architecture](docs/ARCHITECTURE.md)
- [Verification and release gates](docs/VERIFICATION.md)
- [Governance](docs/GOVERNANCE.md)
- [Architecture decisions](docs/adr/)
- [简体中文文档](docs/zh-CN/README.md)

## Project status

The core product flows are implemented and the repository is under active development. The public interfaces are contract-driven, the test and release gates are in place, and the remaining work is tracked in the changelog and issue tracker.

## Contributing

Read [CONTRIBUTING.md](CONTRIBUTING.md) before changing a command, schema or policy. Security reports should follow [SECURITY.md](SECURITY.md).

## License

MIT. See [LICENSE](LICENSE).
