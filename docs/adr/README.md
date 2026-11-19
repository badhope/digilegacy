# Architecture decisions

English | [简体中文](../zh-CN/adr/README.md)

| ID | Decision | Status |
|---|---|---|
| [0001](0001-contract-first.md) | Contracts are the source of truth; generated clients are never edited by hand. | Accepted |
| [0002](0002-modular-monolith.md) | Start as a modular monolith, not a set of microservices. | Accepted |
| [0003](0003-web-primary-agent-tools.md) | The Web console is the primary client; the Agent works through tools and approvals. | Accepted |
| [0004](0004-legacy-not-in-new-build.md) | The old desktop application is not part of the new build. | Accepted |

An ADR is written when a choice is expensive to reverse. Open proposals use the [RFC template](../rfc/0001-template.md).
