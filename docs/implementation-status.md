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
- Implemented GitHub URL normalization for repository, tree, blob, raw, and contents API URLs, including sanitized standalone repository-root skill snapshots.
- Implemented a first CLI surface for the planned commands.
- Added a Tauri + React desktop shell with scan and path bridge commands.
- Implemented Rust/Tauri user-skills Git sync for the shared `~/.skillbox/user-skills` repository, including Settings-managed remote configuration, per-skill dirty status, desktop commit review with diff preview, generated Conventional Commit messages, and selected-file commits.
- Implemented Rust/Tauri/CLI remote skill update status checks, Dashboard status refresh, last-checked timestamps, and configurable 5-minute auto refresh.
- Implemented GitHub-only remote source search/binding, immutable remote version listing, all-file diff preview, update/rollback apply, and permanent operation logging in Rust core, Rust CLI, Tauri commands, and desktop review dialogs.
- Implemented network-backed GitHub install preview/apply in Rust core, Rust CLI, and desktop UI, including first-install diff review, preview identity checks, version snapshots, `current` symlink updates, source metadata, optional CLI deploy, and legacy CLI aliases.
- Added compatibility coverage for Node MVP SQLite stores, including legacy `operations` migration and explicit timestamp writes for old `skills`/`deployments` tables.
- Added shared desktop diff rendering and remote skill workflow normalization helpers for source binding and version change previews.
- Implemented SQLite-backed workspace registry for global and project-local skills roots, including `.codex/skills`, `.agents/skills`, `.claude/skills`, scan-time auto registration, imported skill counts, preview-confirmed single-root project initialization, manual add/forget, Rust CLI compatibility commands, Tauri commands, and a searchable desktop Workspaces page with type filters and per-workspace skill review/import.
- Implemented import records and import revert in Rust core, Rust CLI, Tauri commands, and Skill Detail UI, including backup restoration, conservative legacy reconciliation, multi-workspace blocking, and warning/danger confirmation states.
- Implemented whole-directory duplicate grouping for Import Review, including deterministic primary source selection, retained additional source paths, grouped desktop review/search, explicit untouched-copy messaging, and primary-only backup/symlink deployment that preserves existing revert guarantees.
- Added signed macOS app update checks and user-confirmed install/restart through the Tauri updater plugin, plus release workflow assets for updater archives, signatures, and `latest.json`.
- Added ordered, transactional Rust SQLite migrations, consistent pre-migration backups for existing databases, schema version tracking, and integrity validation.
- Moved dashboard favorites and user-edited tags from browser-only storage into SQLite, with a one-time desktop migration for existing local metadata.
- Added a read-only Doctor workflow in Rust core, Rust CLI, Tauri, and desktop Settings for checking schema/integrity, managed skill layouts, remote `current` links, deployments, workspaces, import backups, and stale metadata.
- Extended operation auditing across direct/reviewed imports, deploy/undeploy, skill type changes, workspace add/forget, user-skills Git remote/sync, and usage hook injection, including failed attempts.
- Implemented reviewed skill deletion across Rust core, CLI, Tauri, and Skill Detail UI, including all-workspace symlink removal, active-import and foreign-target blockers, preview identity checks, transactional active-state cleanup, and retained deletion backups. Single-workspace removal continues through the deployment picker and shared `undeploy_skill` core.
- Added Rust crate scaffolding for the planned Tauri/Rust architecture.
- Verified the desktop shell in browser preview at `http://127.0.0.1:1420/`.

## Next Implementation Targets

The `0.4` implementation scope is complete; release status remains separate
from implementation status and is tracked in `docs/release.md`. The next milestones follow
[the versioned evolution path](roadmap.md#versioned-evolution-path):

### 0.5 — Discovery And Source Trust

- Add FTS-backed search across skills, operations, and usage history.
- Add remote source provenance and trust classification without treating
  popularity as verification.
- Keep search and trust results aligned across Rust core, CLI, Tauri, and desktop.

### 0.6 — Runtime Profiles And Portability

- Add Rust-owned runtime profiles for additional `SKILL.md` roots, precedence,
  and frontmatter dialects.
- Add portability checks and pre-deployment compatibility reporting.

### 0.7 And Later

- Add safe inbound Git status and reviewed fast-forward updates for the shared
  user-skills repository.
- Add copy-snapshot deployment and stronger restore/audit workflows.
- Complete product hardening, release-candidate qualification, and the explicit
  1.0 promotion gates defined in the roadmap.

When implementation changes milestone status, scope, or ordering, update this
file and `docs/roadmap.md` in the same change set.
