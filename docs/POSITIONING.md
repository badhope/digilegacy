# Product positioning

English | [简体中文](zh-CN/POSITIONING.md)

## In one sentence

**DigiLegacy is an API-first digital legacy platform. The Web console is the primary client; Agents use the same platform through tools.**

It is not a desktop utility with an Agent bolted on, and it is not a chat shell with a second backend hidden behind it.

## The problem

Digital legacy work is split across asset lists, people, time-based release rules, platform policies and the documents a family actually needs when something happens. Most tools solve one slice and leave the rest in notes or someone's head.

DigiLegacy puts those slices into one source of truth, one permission model and one command layer. The Web console, CLI, MCP server and Agent all see the same data.

## Who it is for

| Audience | What they do | Main surface |
|---|---|---|
| Owners and families | Record assets, name heirs, configure release rules and review deliverables. | Web console |
| Agents | Read the platform, draft plans and propose actions that need approval. | Tool calls / MCP |
| Operators and developers | Inspect health, discover commands, diagnose issues, migrate and restore. | CLI / Platform API |

## Product promises

- **A single record.** An asset is entered once and referenced consistently from the list, the detail view and the delivered report.
- **Human control over change.** The Agent can propose. A person approves. The platform executes and records what happened.
- **Useful output, not raw data.** Families receive reports, checklists, legal templates and a practical toolkit.
- **Auditability.** Every sensitive action has an actor, a reason and a trace.
- **Self-hosting without a second product.** Local deployment is one way to run the same platform, not a different feature set.

## Boundaries

- The platform owns data, permissions, approvals, audit and execution.
- Agents are clients of the platform, not a parallel backend.
- Write tools require approval; key, network and desktop capabilities are never exposed to Agents.
- The interface contract is the source of truth. Hand-editing generated clients is not allowed.
- The project does not pretend a feature works when it does not. Unsupported commands fail explicitly.

## What success looks like

A new user can record an asset, name an heir and understand the next step without reading a manual. A family member can open a delivered report and know what to do. An operator can trace any change back to the person and command that produced it.
