# Changelog

All notable changes to SkillBox will be documented in this file.

The format is based on Keep a Changelog, and this project uses semantic
version tags such as `v0.3.0`.

## Unreleased

- Add one-fetch GitHub repository collection preview/apply with explicit child
  selection, stale ref/tree validation, and per-skill managed provenance. This
  Phase C change is unreleased; collection-level update/rollback remains planned.
- Update collection roadmap/status wording to distinguish the shipped v0.8.0
  local A+B work from the unreleased Phase C implementation.

## 0.8.0

- Group scanned copied skills with validated v3 GitHub installer-lockfile provenance into display-only Installed source collections, while keeping live Git collections authoritative and selected children on the existing per-skill import path.
- Make collection child type controls truthful: only actionable importable children expose User/Remote actions; imported, system, and conflict children show read-only status/classification instead of disabled controls.
- Add read-only local Git worktree discovery to Import Review, grouping repository
  children into one collection card while preserving independent child selection.
- Persist reviewed collection source identity and imported child relationships in
  schema v8 after a successful, stale-checked apply.
- Keep GitHub multi-skill fetch and collection-level update/rollback planned for a
  later phase; this change does not claim those workflows.
- Open Import Review before the local scan finishes, expose accessible staged progress
  and retry state, and reuse repository identity and snapshot work across duplicate
  locations. Closing the review dismisses the UI and ignores stale results; it does
  not claim to abort the underlying read-only Rust scan. Native Tauri scan requests
  now use the bridge's camelCase argument contract.

## 0.7.1

- Keep All, Calls, References, and Operations together in the History filter control on desktop and narrow windows.
- Query the selected History event type directly so older References and Operations remain visible outside the bounded mixed-history window.
- Prevent rapid History tab and refresh requests from overwriting newer results.
- Add mandatory DMG-level notarization, stapling, and Gatekeeper verification before macOS release assets are published.

## 0.7.0

- Add explicit inbound user-skills Git updates through Check remote, Review incoming changes, and preview-confirmed fast-forward Apply, with clear clean, dirty, ahead, behind, diverged, remote-only, and missing-branch states.
- Keep inbound Git resolution deliberate: SkillBox never auto-merges, rebases, resets, stashes, force-pushes, or resolves conflicts, and it validates incoming trees, affected deployments, stale previews, backup refs, Git/SQLite recovery, and concurrent local changes before reporting success.
- Group same-name Import Review candidates into one skill card, collapse equivalent repository/runtime locations into one Rust-qualified variant, keep materially different variants explicit, and submit at most one primary source per skill.
- Keep mixed User/Remote advice as one content variant and require an explicit, accessible Skill type choice before import rather than manufacturing duplicate variants.
- Align resolved, required, and disabled Skill type controls to one stable width with evenly split User/Remote actions and responsive narrow-layout behavior.
- Refresh supporting documentation, GitHub Actions, desktop dependencies, and release maintenance without changing the local-first review-before-write safety model.

## 0.6.1

- Classify local skill usage evidence as confirmed, inferred, or reference while keeping Calls focused on confirmed and defensible inferred invocations.
- Improve Codex history reconciliation and aggregate-only usage auditing without treating low-signal catalogs, prose, shell payloads, or tool output as Calls.
- Reconstruct Cursor Calls from structured transcript Read events, including safe historical skills that were later moved or removed, with one invocation per transcript user turn and skill.
- Separate History references from Calls across Dashboard, Workspaces, History, Rankings, and skill details, with clearer local coverage diagnostics.

## 0.6.0

- Add a versioned Rust runtime-profile registry for Agents, Codex, Claude Code, Cursor, and exact custom `SKILL.md` roots, with deterministic discovery precedence and schema-v6 workspace backfill that does not require a rescan.
- Preserve structured `SKILL.md` frontmatter during read-only compatibility checks, reporting unknown optional fields as warnings and malformed or hard incompatibilities as blockers without rewriting source files.
- Require a fresh compatibility preview before deployment across core, CLI, Tauri, and desktop; stale skill snapshots, targets, or profile metadata are rejected before runtime writes, and warning-level GitHub target installs require explicit confirmation.
- Show runtime profile identity and Compatible/Warning/Blocked results in Workspaces and deployment review, replacing React path-marker inference.
- Deploy GitHub target installs through freshly revalidated canonical workspace paths so symlink aliases cannot change the runtime write target after preview.
- Prioritize Top skills and Full ranking on the Rankings page while keeping Local data coverage available through an accessible expandable disclosure.

## 0.5.1

- Expand History to the same full content width as other standard pages for clearer timelines and better use of desktop space.
- Share a reusable full-width `PageFrame` across Dashboard, Workspaces, Rankings, and History so standard pages stay consistently aligned.
- Simplify compact usage labels to `Calls` and `<n> calls`, while detailed help continues to explain that these are locally observed calls rather than account-level analytics.

## 0.5.0

- Check macOS updater metadata at most once per day, keep the last successful result across launches, and show a one-click Update action beside the SkillBox brand while retaining artifact signature verification during install.
- Add a standalone Skill Usage Rankings page with 7-day, 30-day, all-time, skill-type (User/Remote/System), Agent, and Workspace filters, labeling its metric as `Locally observed calls` and keeping it separate from source trust.
- Import historical Codex skill calls from explicit user-input carriers in local session rollout logs via `usage-backfill-codex` / `Sync histories`. Backfill accepts complete `<skill>` blocks and explicit `[$skill](.../SKILL.md)` links, ignores assistant/tool output, and deduplicates by turn plus normalized name/path and against existing hook events.
- Expand Rankings history sync to Claude Code structured Skill attribution and explicitly attached Cursor `SKILL.md` context. Cursor history is read from a validated read-only SQLite schema and fails closed when the private format is unsupported.
- Ignore relative paths and code-template placeholders in Codex history so stale examples do not surface as sync errors or usage calls; provider summaries now identify which history source reported errors.
- Report ranking coverage with earliest/latest observed events, canonical stored-origin counts for hooks, Codex, Claude Code, Cursor, and other observations, plus provider-specific latest scan counts.
- Reserve Codex reported runs as an independent metric with provider, subject kind, time window, scope, and provenance; reported runs never enter `skill_usage_events` or local ranking, total, and delta values.
- Let Confirm local import choose User or Remote before moving skills into the managed store.
- Distinguish Codex system skills in Rankings as `System` (not importable) instead of `Not imported`; keep same-name regular and System observations as source-aware ranking rows, including when the regular skill is already managed or both sources share one runtime root.
- Mark ambiguous legacy events as `Unknown source` instead of guessing a regular skill, and validate Rankings imports against the complete source identity recorded in SQLite.
- Mark unmanaged ranking skills whose local source is gone as `Deleted`.
- Canonicalize legacy usage agent ids (`agents`/`claude` → `codex`/`claude-code`) via schema v5 and rebuild affected stats from canonical events so Rankings, Dashboard, and Workspace totals stay consistent.
- Update PostCSS and Nano ID to patched releases so the v0.5 dependency audit completes without high-severity findings.

## 0.4.5

- Install standalone GitHub skill repositories and root `SKILL.md` URLs through a preview-confirmed flow. SkillBox excludes `.git` metadata from managed snapshots and keeps later source updates root-aware.
- Add normal project directories as workspaces through a read-only setup preview, then explicitly create and register exactly one selected `.agents/skills`, `.codex/skills`, or `.claude/skills` root with stale-preview, traversal, symlink, and cleanup protections.
- Choose a local project or skills folder with the packaged macOS app's native single-directory picker while retaining manual path entry. Folder selection immediately opens the preview, and cancelling the picker leaves the current setup unchanged.

## 0.4.4

- Add consent-gated VibeLoft page-view telemetry with DNT/GPC support, withdrawal controls, and a website privacy notice that remains separate from SkillBox app and CLI data.
- Group import-equivalent candidates across runtime roots into one review row, show untouched copies, and keep imports limited to the deterministic primary source.
- Validate complete User and Remote skill contents, permissions, and symlink targets before grouping or reuse, and show exact source paths when imports fail.

## 0.4.3

- Keep Dashboard and Skill Detail favorite toggles responsive after deleting a managed skill.

## 0.4.2

- Refresh the Dashboard with attention-based status stripes, neutral type and tag treatments, readable skill names, consolidated filters, adaptive cards, clearer agent tooltips, keyboard search, and actionable empty states.
- Add workspace search and align the Workspaces and History filter tabs with the refreshed Dashboard controls.
- Restore reliable favorite toggles from skill cards without opening the skill detail view.

## 0.4.1

- Add reviewed skill deletion across Rust core, CLI, Tauri, and desktop, including removal from every managed workspace and a retained recovery backup.
- Clarify single-workspace removal in the deployment picker while preserving symlink ownership checks and active-import recovery paths.
- Harden deletion previews and rollback behavior for complete remote roots, equivalent workspace paths, corrupted caches, broken remote layouts, and concurrent target replacement.
- Make the typed deletion confirmation value directly copyable from the desktop dialog.
- Disable automatic capitalization, completion, correction, and spellchecking by default for every desktop input and textarea, including dynamically rendered dialogs.

## 0.4.0

- Make upgrades safer with ordered SQLite migrations, automatic pre-migration backups, and post-migration integrity checks.
- Persist Dashboard favorites and tags in SQLite, including automatic migration from existing desktop local storage.
- Add Managed store health checks in Settings and CLI, with accurate symlink diagnostics and explicit cleanup of stale deployment records without deleting runtime files.
- Expand operation history across imports, deployments, skill type and workspace changes, Git synchronization, usage hook setup, and Doctor cleanup attempts.

## 0.3.10

- Require preview confirmation before installing remote GitHub skills, so users review the incoming `SKILL.md` diff before SkillBox writes managed state or deploys anything.
- Clarify user-skills Git sync conflict handling: when the remote diverges, SkillBox keeps the local commit and asks the user to resolve with normal Git tooling before retrying.
- Improve public-facing project copy and search setup, including README/homepage wording and Google Search Console verification support.
- Refresh safe desktop/runtime dependencies and GitHub Actions release infrastructure, while deferring React 19 to a dedicated migration track.

## 0.3.9

- Add a safety-focused first-run onboarding flow that explains read-only scans, review-before-import, and intentional deploys.
- Add a public SkillBox homepage with the promo video, install links, and searchable product documentation.
- Improve homepage metadata and FAQ content so users can discover SkillBox through search.

## 0.3.8

- Add Import Revert for deploy-back imports so a runtime skill can be restored to its pre-import folder.
- Preserve remote managed versions during revert and allow the same skill to be imported and reverted again.
- Block unsafe import reverts when a skill has multiple workspace deployments or the source/backup no longer matches the recorded state.
- Add CLI, desktop bridge, and Skill Detail controls for reviewing and confirming import reverts.

## 0.3.7

- Add workspace skill review tabs that separate unimported, imported, and system candidates while preserving symlink-only workspace skills.
- Reuse the searchable skill review list in Import Review, hiding duplicate symlink candidates when their source skill is already present.
- Allow managed skills to be changed between User and Remote storage with a confirmation flow that retargets existing workspace deployments.

## 0.3.6

- Fix a desktop startup blank screen caused by duplicate React runtimes after dependency updates.
- Keep the desktop React dependency resolved to a single runtime so icon rendering does not crash the app.

## 0.3.5

- Move active workspace agent icons next to the Active workspaces label in the skill detail deployment panel.
- Keep the active workspace icon stack vertically centered with the label text.

## 0.3.4

- Rename the remote skill update confirmation button to Apply Update.
- Refresh only the updated remote skill status after applying a version change to avoid unnecessary dashboard stalls.
- Preserve the rest of the remote update status table during targeted refreshes.

## 0.3.3

- Share the Dashboard page title template across Dashboard, Settings, Workspaces, and History for consistent page headers.
- Compact the Settings page into a clearer tabbed workbench with stacked setting groups and no duplicate status summary.
- Improve remote diff review readability by explaining omitted oversized previews and separating footer actions from the diff pane.
- Add the SkillBox promo video and source package to the public documentation.

## 0.3.2

- Install GitHub-backed remote skills from the desktop Install dialog without deploying them automatically.
- Stop counting managed remote skill `current` symlinks as active runtime workspaces.
- Keep newly imported skill tags empty until users add their own labels.
- Align dashboard page title spacing with the sidebar brand.

## 0.3.1

- Fix remote update previews when a version diff includes directory entries.
- Fix applying remote updates for skills that symlink to shared directories inside the same GitHub repository.
- Preserve symlink escape protections for local imports and external paths while snapshotting safe same-repo shared files.

## 0.3.0

- Add signed in-app update checks for the macOS desktop app, with
  user-confirmed install and restart.
- Publish Tauri updater artifacts and `latest.json` alongside the signed DMG in
  the release workflow.
- Build both macOS app and DMG bundles in the release workflow so updater
  archives are generated, verified, and published.
- Upload updater artifacts with versioned asset filenames so `latest.json`
  update URLs match the GitHub Release downloads.
- Extend release automation and documentation so app updater assets are verified
  before Homebrew publication.
- Upgrade the desktop build tooling to Vite 8 to clear high-severity npm audit
  findings before release.

## 0.2.0

- Retire the legacy Node CLI/core packages and move GitHub install, rollback,
  update checking, and compatibility command entry points onto the Rust
  CLI/core path.
- Strengthen CI and dependency governance with Rust clippy warnings-as-errors,
  Rust and npm audit jobs, Dependabot configuration, and a PR template.
- Add public project roadmap and good-first-issue guidance for contributors.
- Align public release, security, contribution, architecture, and workflow docs
  with the Rust-only CLI/core direction.
- Improve desktop maintainability by splitting large UI and core modules, and
  link the sidebar Help action to GitHub Issues.

## 0.1.1

- Promote the macOS app from public alpha to the first regular release.
- Improve workspace scans, SKILL.md description parsing, user skill sync
  defaults, remote update detection, dashboard tagging, and desktop layout.
- Update release automation to publish regular releases while keeping alpha tag
  support.

## 0.1.0-alpha.3

- Prepare public alpha documentation, CI, release workflow, and Homebrew cask
  template.
- Add mounted DMG signature, Gatekeeper, version, and bundle identifier checks
  before publishing release assets.

## 0.1.0-alpha.1

Planned first public alpha.

- Local macOS desktop app for `SKILL.md`-based skill management.
- Scan global and project-local runtime skill directories.
- Import local and remote skills into `~/.skillbox`.
- Deploy managed skills to runtime directories with symlinks.
- Track remote skill sources and versions.
- Optional usage hook injection for local call counting.
