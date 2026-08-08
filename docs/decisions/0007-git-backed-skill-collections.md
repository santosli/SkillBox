# ADR 0007: Git-Backed Skill Collections

## Status

Accepted for v0.8.0 Phase A+B. GitHub multi-skill fetch and collection-level
update/rollback remain planned Phase C/D work.

## Context

Import Review can discover many independent `SKILL.md` directories from one
local repository. Showing only a flat list loses the repository boundary,
while grouping by name or content alone can incorrectly merge unrelated copies.
Runtime symlinks and Git worktrees also need a stable identity that does not
depend on the displayed path.

The source-of-truth and safety boundary remains unchanged: scans are read-only,
managed skills remain independent deployable entities, and untrusted Git trees
must not execute hooks, filters, submodules, repository scripts, or arbitrary
shell commands.

## Decision

Phase A uses the hardened Rust `GitService` to discover, for each candidate, the
nearest safe Git worktree and its Git common directory. A collection is keyed
by that canonical worktree/repository identity and includes the reviewed branch
or detached state, HEAD, sanitized origin, and ordered repository-relative
children. Nested repositories are separate collections. Runtime symlinks that
resolve to a child in the same worktree map to the same child; copies outside
Git metadata remain standalone or are displayed as unlinked locations.

Existing Rust-owned skill group/variant/type-review semantics remain inside each
child. A collection card is a UI disclosure, not a new deployable unit: users
select children individually or with Select all applicable, and each selected
child still has one primary source, type, managed identity, deployment, Calls,
and history record.

Phase B adds schema v8 tables `skill_collections` and
`skill_collection_members`. Rows are written only after a selected-child import
passes preview identity, canonical-root, HEAD, full snapshot, path, variant,
type, duplicate-name, and managed-target checks. The apply path uses the
existing managed mutation lock and a preflight plus operation-scoped
compensatable rollback. It does not claim an indivisible cross-filesystem
transaction; if rollback cannot safely remove changed content, the content is
preserved and the failure explains the recovery boundary.

## Consequences

- Import Review can show one repository card with many children without
  weakening same-name variant or type-review rules.
- Existing skill rows, deployments, usage/history, import records, and remote
  versions remain independent and do not require a rescan after migration.
- Collection availability is derived from the canonical worktree at read time;
  moving or deleting the worktree does not delete managed skills or provenance.
- Phase C will add one-fetch GitHub repository preview/apply; Phase D will add
  reviewed collection update/rollback. Neither is part of the current contract.
