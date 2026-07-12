# Roadmap

SkillBox is early-stage software. This roadmap describes the public direction,
not a date-based commitment. Implementation details can change as the app gets
more real-world use.

## Current Focus: 0.4.x

The 0.4 development line establishes the reliability foundation required for
the later search, runtime, synchronization, and stable-release milestones:

- ordered, transactional SQLite migrations with schema version history;
- one-time pre-migration backups and post-migration integrity checks;
- SQLite-backed dashboard favorites and tags with legacy local-storage migration;
- a read-only Doctor workflow across Rust core, CLI, Tauri, and desktop Settings;
- broader operation auditing for managed-store, runtime, Git, workspace, and
  hook configuration mutations.
- reviewed removal from one workspace and full managed-skill deletion with
  all-workspace cleanup, ownership preflight, and retained recovery backups.

The implementation is complete. The currently shipped version and distribution
assets are tracked in `docs/release.md`; a version is not considered shipped
until the release workflow and its distribution checks succeed.

## Near-Term Priorities

These are the next areas where focused contributions are most useful:

- **Rust CLI and desktop parity.** Keep CLI behavior and desktop Tauri commands
  aligned on the shared Rust core.
- **Search and navigation.** Add FTS-backed search for skills, operations, and
  usage history on top of the versioned SQLite schema.
- **Runtime profiles.** Model additional `SKILL.md` roots, precedence, and
  frontmatter capabilities without hard-coding agent behavior in React.
- **Remote trust.** Preserve source provenance and show explicit trust state
  before install or update without using popularity as proof of safety.
- **User-skills inbound sync.** Report ahead/behind/diverged Git state and allow
  reviewed fast-forward updates without automatically merging conflicts.
- **Dependency hygiene.** Keep Tauri, Vite, Rust crates, and GitHub Actions
  current without weakening the local safety model.
- **Documentation polish.** Keep screenshots, install instructions, and safety
  expectations aligned with the latest release.

## Versioned Evolution Path

The planned sequence is reliability, discovery and trust, runtime portability,
safe synchronization and recovery, product hardening, release qualification,
then a stable contract. A milestone advances only after its promotion gates are
verified; completing a feature list alone does not qualify a release.

| Version | Product outcome | Promotion gates |
| --- | --- | --- |
| **0.4 — Reliability foundation** | Versioned database migrations, persisted user metadata, Doctor diagnostics, and durable mutation auditing. | Upgrade and backup tests pass; Doctor is available through core, CLI, Tauri, and desktop; audited workflows record both success and failure; release automation passes. |
| **0.5 — Discovery and source trust** | FTS-backed search across skills, operations, and usage history; clearer source provenance and trust classification before remote install or update. | CLI and desktop return consistent results; schema upgrades and representative large-library queries are tested; trust state never treats popularity as proof of safety. |
| **0.6 — Runtime profiles and portability** | Rust-owned runtime profiles model roots, precedence, frontmatter capabilities, and compatibility without hard-coding agent behavior in React. | Each supported profile has fixtures and compatibility tests; unsupported fields are reported before deployment; runtime-specific behavior remains behind an adapter boundary. |
| **0.7 — Safe sync, deployment, and recovery** | Reviewed inbound user-skills Git updates, copy-snapshot deployment, and stronger restore/audit workflows complement the existing symlink path. | Ahead/behind/diverged states are explicit; conflicts are never auto-merged; overwrite protection, rollback, and backup restoration have automated coverage. |
| **0.8 — Product hardening** | Large-library performance, actionable diagnostics, accessibility, onboarding, and recovery behavior are ready for sustained daily use. | Performance budgets and critical UI workflows are verified; no known data-loss path remains; supported upgrade and recovery procedures are documented and exercised. |
| **0.9 — Release candidate** | Feature scope is frozen while security, migration compatibility, packaging, updater, Homebrew, and real-world beta feedback are closed out. | Threat-model review is complete; upgrades from every supported prior release are tested; blocker defects are closed; signed and notarized distribution rehearsals pass. |
| **1.0 — Stable local skill management** | SkillBox offers a documented, supportable contract for discovering, importing, managing, deploying, updating, synchronizing, diagnosing, and recovering supported skills. | Core workflows meet their definitions of done; supported runtimes and limitations are explicit; migrations and recovery are proven; release artifacts and docs match; no open blocker or known data-loss issue remains. |

Minor-version scope may change with evidence from real usage. When the scope,
ordering, status, or promotion gate of a milestone changes, the same change set
must update this roadmap and `docs/implementation-status.md`.

## Good First Contribution Areas

Good first issues should be small, testable, and low-risk. The best starter
work usually lives in:

- documentation fixes and screenshots;
- issue templates and contributor guidance;
- focused UI copy or empty-state polish;
- tests for existing helpers;
- small CLI or normalization improvements that do not touch destructive file
  operations.

See [Good first issues](good-first-issues.md) for contributor and maintainer
guidance.

## Later Directions

These are important, but need more design or production feedback before they
should become starter work:

- broader CLI packaging and distribution beyond the current macOS desktop
  release;
- Windows and Linux support evaluation;
- optional ecosystem adapters beyond the runtime profiles proven before 1.0;
- collaboration features that preserve the local-first source-of-truth model.

## Non-Goals

SkillBox should not become:

- a hosted cloud account or remote synchronization service;
- an automatic executor of arbitrary user-provided shell strings;
- a tool that silently overwrites existing runtime content;
- an agent-specific format that treats one runtime as the global source of
  truth.
