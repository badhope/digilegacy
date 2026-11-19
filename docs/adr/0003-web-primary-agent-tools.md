# ADR-0003: Web is the primary client; the Agent uses tools

- Status: Accepted
- Date: 2026-11-19

## Context

It is easy to build an Agent product as a chat window with a private backend. That creates two truths: what the person sees and what the Agent can do. It also makes approval and audit cosmetic.

## Decision

The Web console is the primary place where a person inspects and changes data. The Agent is another client of the same platform, using the same commands and permission table. Read and draft tools may run directly. Write tools create an approval and pause. Key, network and desktop tools are never exposed to the Agent or MCP.

## Alternatives considered

- Agent-first chat UI. Rejected: poor fit for reviewing assets, diffs and documents.
- Agent with a separate API. Rejected: guaranteed divergence and a larger security surface.
- Fully autonomous write access. Rejected: unacceptable for irreversible legacy actions.

## Consequences

Every Agent action is visible in the same interface as a human action. Approval becomes a real state, not a warning. The Agent is less autonomous by design, and that is the point.

## Review trigger

Revisit only if a future client can consume the same command contract and approval model without creating a parallel data path.
