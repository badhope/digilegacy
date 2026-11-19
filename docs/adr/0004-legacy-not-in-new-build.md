# ADR-0004: The old desktop application is not part of the new build

- Status: Accepted
- Date: 2026-11-19

## Context

The previous desktop application carried useful security and domain behavior, but its UI, routing and packaging no longer matched the API-first platform. Keeping it alongside the new build would double the maintenance surface and confuse the product boundary.

## Decision

Remove the old application from the build. Port the security kernel and domain rules into modules with tests. Do not keep legacy routes or redirect them. Backups from the old application remain importable through a migration path.

## Alternatives considered

- Keep the desktop shell and add an API. Rejected: two products, two release paths.
- Redirect old routes for compatibility. Rejected: it keeps dead UI concepts alive.
- Rewrite security from scratch. Rejected: unnecessary risk; the algorithms and test vectors were already sound.

## Consequences

The repository has one product shape and a smaller dependency surface. Users of the old desktop build need a migration path for data, not a UI compatibility layer. Historical code stays in git history, not in the working tree.

## Review trigger

Revisit only if a native client can be generated from the same contract and adds a capability the Web console cannot provide.
