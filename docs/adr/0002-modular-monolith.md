# ADR-0002: Start as a modular monolith

- Status: Accepted
- Date: 2026-11-19

## Context

The product has clear domains, but the load pattern and deployment shape are still young. Splitting services now would add network boundaries, deployment overhead and distributed failure modes before there is evidence they are needed.

## Decision

Keep one Rust workspace and one runtime process. Enforce boundaries inside the codebase: entry points depend on application services, application services depend on domains and infrastructure, and domains never depend on transport. A command bus is the only path from an entry point into business logic.

## Alternatives considered

- Microservices per domain. Rejected: premature operational cost and cross-service transactions.
- A single crate without internal boundaries. Rejected: it would make later extraction expensive and hide ownership.
- Serverless functions. Rejected: poor fit for a local-first database and long-running jobs.

## Consequences

Deployment stays simple, tests are fast and transactions remain local. The team must actively prevent boundary erosion. If a domain later needs independent scaling, it can be extracted behind the existing command contract.

## Review trigger

Revisit when a domain has a genuinely different scaling, availability or compliance requirement, or when one process can no longer meet the measured load.
