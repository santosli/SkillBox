# Implementation Status

## Completed

- Created the monorepo layout for CLI/core, desktop app, and Rust crates.
- Retired the legacy Node CLI/core packages after migrating their product behavior into Rust.
- Installed Rust stable with rustup.
- Migrated the core scan, import, symlink deploy, SQLite indexing, and GitHub URL parsing paths into Rust crates.
- Replaced the temporary system `sqlite3` shell-out with `rusqlite` parameterized writes.
- Installed `rustfmt` and verified Rust formatting.
- Switched the Tauri bridge from spawning the Node CLI to calling Rust crates directly.
- Implemented `SKILL.md` frontmatter parsing.
- Implemented recursive skill scanning.
- Implemented user and remote import storage.
- Implemented symlink deployment with overwrite protection.
- Implemented the versioned Rust runtime-profile registry for `.agents/skills`,
  `.codex/skills`, `.claude/skills`, `.cursor/skills`, and exact custom
  `SKILL.md` roots, including schema-v6 workspace backfill, structured
  frontmatter preservation, compatibility previews, stale-preview deployment
  confirmation, CLI/Tauri parity, and profile-aware desktop workspaces.
- Implemented GitHub URL normalization for repository, tree, blob, raw, and contents API URLs, including sanitized standalone repository-root skill snapshots.
- Implemented a first CLI surface for the planned commands.
- Added a Tauri + React desktop shell with scan and path bridge commands.
- Implemented Rust/Tauri user-skills Git sync for the shared `~/.skillbox/user-skills` repository, including Settings-managed remote configuration, per-skill dirty status, desktop commit review with diff preview, generated Conventional Commit messages, and selected-file commits.
- Implemented explicit inbound user-skills Git synchronization on `main` through
  Check remote -> Review incoming changes -> Apply fast-forward, with separate
  worktree/relation state, stale-preview rejection, untrusted-tree validation,
  deployed deletion/rename blockers, backup refs, and independent Git/SQLite
  recovery auditing. This shipped in `v0.7.0` and is included in the current
  `v0.7.1` release.
- Shipped the v0.7.1 History filter/layout patch: All, Calls, References, and
  Operations stay contained in the segmented control, and selected kinds query
  Rust directly so bounded mixed-history results do not hide older references.
- Implemented Rust/Tauri/CLI remote skill update status checks, Dashboard status refresh, last-checked timestamps, and configurable 5-minute auto refresh.
- Implemented GitHub-only remote source search/binding, immutable remote version listing, all-file diff preview, update/rollback apply, and permanent operation logging in Rust core, Rust CLI, Tauri commands, and desktop review dialogs.
- Implemented network-backed GitHub install preview/apply in Rust core, Rust CLI, and desktop UI, including first-install diff review, preview identity checks, version snapshots, `current` symlink updates, source metadata, optional CLI deploy, and legacy CLI aliases.
- Added compatibility coverage for Node MVP SQLite stores, including legacy `operations` migration and explicit timestamp writes for old `skills`/`deployments` tables.
- Added shared desktop diff rendering and remote skill workflow normalization helpers for source binding and version change previews.
- Implemented SQLite-backed workspace registry for global and project-local skills roots, including `.codex/skills`, `.agents/skills`, `.claude/skills`, scan-time auto registration, imported skill counts, preview-confirmed single-root project initialization, manual add/forget, Rust CLI compatibility commands, Tauri commands, and a searchable desktop Workspaces page with type filters and per-workspace skill review/import.
- Implemented import records and import revert in Rust core, Rust CLI, Tauri commands, and Skill Detail UI, including backup restoration, conservative legacy reconciliation, multi-workspace blocking, and warning/danger confirmation states.
- Implemented Rust-owned Import Review groups with stable content/status variant identities, full-snapshot-equivalent locations, location-level User/Remote advice with explicit mixed-suggestion confirmation, explicit selection for materially different same-name variants, group-level Calls/search/tab counts, and one-primary-only import enforcement that preserves existing revert guarantees.
- Implemented v0.8.0 Skill Collections Phase A+B: Rust discovers the nearest safe Git worktree for local Import Review, groups repository children with canonical worktree/HEAD identity, keeps external copies unlinked, and persists reviewed collection/member provenance after a stale-checked child import. GitHub multi-skill fetch and collection-level update/rollback are not part of the published v0.8.0 release.
- Added a bounded installer-lockfile fallback for copied skills: valid v3 GitHub provenance can form a display-only `installed_source` collection after filesystem scanning, while live Git identity wins and selected children retain the ordinary per-skill import/apply contract.
- Added signed macOS app update checks and user-confirmed install/restart through the Tauri updater plugin, plus release workflow assets for updater archives, signatures, and `latest.json`.
- Added daily macOS updater metadata checks with a SQLite-backed successful-result cache, a sidebar Update reminder, one-click metadata recheck plus signed install/restart, and retry-safe pending updates without automatic downloads.
- Added ordered, transactional Rust SQLite migrations, consistent pre-migration backups for existing databases, schema version tracking, and integrity validation.
- Moved dashboard favorites and user-edited tags from browser-only storage into SQLite, with a one-time desktop migration for existing local metadata.
- Added a read-only Doctor workflow in Rust core, Rust CLI, Tauri, and desktop Settings for checking schema/integrity, managed skill layouts, remote `current` links, deployments, workspaces, import backups, and stale metadata.
- Extended operation auditing across direct/reviewed imports, deploy/undeploy, skill type changes, workspace add/forget, user-skills Git remote/sync, and usage hook injection, including failed attempts.
- Implemented reviewed skill deletion across Rust core, CLI, Tauri, and Skill Detail UI, including all-workspace symlink removal, active-import and foreign-target blockers, preview identity checks, transactional active-state cleanup, and retained deletion backups. Single-workspace removal continues through the deployment picker and shared `undeploy_skill` core.
- Implemented evidence-aware local Skill Usage Rankings across Rust core, CLI, Tauri, and desktop. schema v7 persists `confirmed/inferred/reference` plus bounded provenance, upgrades duplicate events only toward stronger evidence, and idempotently rebuilds `skill_usage_stats` from `confirmed + inferred` Calls without requiring a rescan. Codex structured per-turn skill carriers and Cursor transcript assistant `Read` events are inferred Calls; Cursor Reads deduplicate once per transcript user turn and skill and can retain safe historical-missing lexical evidence without granting filesystem authority. Stop hooks and Claude Code native Skill attribution are confirmed; Cursor state attachments and public `usage-record` events are references; Cursor `ReadFile` candidates remain diagnostic-only. Dashboard, Workspaces, History, Skill Detail, and Rankings keep Calls separate from History references. Coverage returns evidence totals, time ranges, overlapping provenance source counts, scan/backfill aggregates, and an aggregate-only `usage-audit`; it does not expose prompt/chat/tool bodies. Codex provider-native run totals remain unavailable, so Calls are explicitly a known local undercount. `Sync histories` is user-triggered, idempotent, and can recover or upgrade evidence while preserving successful provider results when another provider fails.
- Added Rust crate scaffolding for the planned Tauri/Rust architecture.
- Verified the desktop shell in browser preview at `http://127.0.0.1:1420/`.

## Next Implementation Targets

The `0.6` implementation and release qualification are complete. SkillBox
`v0.8.0` is the current shipped release, including reviewed inbound sync,
History query/layout fixes, and Skill Collections Phase A+B. The current
`v0.9.0` work follows [issue #46](https://github.com/santosli/SkillBox/issues/46):
Phase C is implemented for `v0.9.0` and remains unreleased pending `v0.9.0`
release qualification; Phase D remains planned for a later `v0.9.x` release.
The next milestones follow [the versioned evolution path](roadmap.md#versioned-evolution-path):

### 0.7 — Safe Sync, Deployment, And Recovery

- Reviewed inbound user-skills Git shipped in `v0.7.0`. It separates
  clean/dirty worktree state from
  unknown/synced/ahead/behind/diverged/remote-only/no-remote-branch relation,
  then uses Check remote -> Review incoming changes -> Apply fast-forward.
- The inbound implementation validates the complete remote skill tree, binds a
  `preview_id`, discloses deployed updates, blocks deployed deletes/renames,
  creates an internal backup ref, and transactionally reconciles the user-skill
  index with Git compensation on failure.
- Inbound remains explicit and `origin/main`-only. It does not run on startup,
  select individual skills, or automatically merge, rebase, reset, stash,
  force-push, or resolve conflicts.
- The v0.7.1 History patch keeps All, Calls, References, and Operations aligned
  with bounded server-side queries and request-generation protection.
- Add copy-snapshot deployment as an explicit, compatibility-checked
  alternative without weakening the current symlink protections.
- Strengthen backup inspection, restore previews, and recovery auditing; these
  broader hardening items are not claimed as shipped.
- Keep runtime profiles, schema-v6 workspace migration, schema-v7 evidence
  classification, and Calls/reference semantics backward compatible.
- Keep native non-`SKILL.md` formats behind the future adapter boundary.

### 0.9.x — Git-backed Skill Collections

- [Git-backed Skill Collections](https://github.com/santosli/SkillBox/issues/46)
  are the active v0.9.0 milestone. Phase A+B shipped in v0.8.0: local repository
  grouping and schema-backed collection/member relationships are available in
  Import Review. A collection records canonical worktree/repository identity,
  reviewed HEAD, and child-relative provenance; child skills remain independent
  for selection, import, deployment, Calls, and history.
- Phase C, one-fetch GitHub multi-skill preview/apply, is implemented for
  v0.9.0 and remains unreleased pending v0.9.0 release qualification. It is
  not shipped yet.
  Phase D, commit-consistent collection update/rollback, remains planned for a
  later v0.9.x release.
- Collection scans must remain read-only. Apply must revalidate repository
  identity, HEAD/ref, and tree snapshot; execute no hooks, submodules, filters,
  scripts, or arbitrary shell; and preserve existing path, overwrite,
  duplicate-name, backup, and recovery protections. The current local apply is
  preflighted and compensatable rather than a claim of a cross-filesystem
  transaction; failures report rollback limitations instead of hiding partial
  work.
- Add FTS-backed search across skills, operations, and usage history.
- Add remote source provenance and trust classification without treating
  popularity as verification.
- Add copy-snapshot deployment and stronger restore/audit workflows.
- Complete product hardening, release-candidate qualification, and the explicit
  1.0 promotion gates defined in the roadmap.

When implementation changes milestone status, scope, or ordering, update this
file and `docs/roadmap.md` in the same change set.
