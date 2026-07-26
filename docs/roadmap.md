# Roadmap

SkillBox is early-stage software. This roadmap describes the public direction,
not a date-based commitment. Implementation details can change as the app gets
more real-world use.

## Current Focus: 0.6.x

The 0.6 development line makes `SKILL.md` deployment targets explicit and
portable without moving runtime knowledge into React:

- a versioned Rust runtime-profile registry for Agents, Codex, Claude Code,
  Cursor, and exact custom `SKILL.md` roots;
- deterministic discovery precedence and schema-backed workspace identity;
- read-only frontmatter and deployment compatibility previews;
- stale-preview protection and explicit confirmation before runtime writes.

The implementation scope is complete. Release qualification and the currently
shipped version and distribution assets are tracked in `docs/release.md`; a
version is not considered shipped until the release workflow and its
distribution checks succeed.

## Near-Term Priorities

These are the next areas where focused contributions are most useful:

- **Rust CLI and desktop parity.** Keep CLI behavior and desktop Tauri commands
  aligned on the shared Rust core.
- **Search and navigation.** After 0.5, add FTS-backed search for skills,
  operations, and usage history on top of the versioned SQLite schema.
- **Runtime profile qualification.** Exercise profile migration and
  compatibility results against real-world `SKILL.md` libraries while keeping
  native non-`SKILL.md` adapters out of the 0.6 boundary.
- **Remote trust.** After 0.5, preserve source provenance and show explicit
  trust state before install or update without using popularity as proof of
  safety.
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
| **0.5 — Local usage discovery and release awareness** | Local skill usage rankings with time-range, skill-type, agent, and workspace filters; explicit local-only coverage; auditable multi-provider history sync from Codex, Claude Code, and Cursor; daily signed app-update awareness without automatic downloads. | CLI and desktop return consistent ranking results; ranking coverage is explicit and never presented as global popularity; history providers resolve real local skills, deduplicate stable event identities, and preserve successful imports when another provider fails; schema upgrades and representative ranking queries are tested; update checks are rate-limited and every install is revalidated after an explicit click. |
| **0.6 — Runtime profiles and portability** | Rust-owned runtime profiles model roots, precedence, frontmatter capabilities, and compatibility without hard-coding agent behavior in React. | Each supported profile has fixtures and compatibility tests; unsupported fields are reported before deployment; runtime-specific behavior remains behind an adapter boundary. |
| **0.7 — Safe sync, deployment, and recovery** | Reviewed inbound user-skills Git updates, copy-snapshot deployment, and stronger restore/audit workflows complement the existing symlink path. | Ahead/behind/diverged states are explicit; conflicts are never auto-merged; overwrite protection, rollback, and backup restoration have automated coverage. |
| **0.8 — Product hardening** | Large-library performance, actionable diagnostics, accessibility, onboarding, and recovery behavior are ready for sustained daily use. | Performance budgets and critical UI workflows are verified; no known data-loss path remains; supported upgrade and recovery procedures are documented and exercised. |
| **0.9 — Release candidate** | Feature scope is frozen while security, migration compatibility, packaging, updater, Homebrew, and real-world beta feedback are closed out. | Threat-model review is complete; upgrades from every supported prior release are tested; blocker defects are closed; signed and notarized distribution rehearsals pass. |
| **1.0 — Stable local skill management** | SkillBox offers a documented, supportable contract for discovering, importing, managing, deploying, updating, synchronizing, diagnosing, and recovering supported skills. | Core workflows meet their definitions of done; supported runtimes and limitations are explicit; migrations and recovery are proven; release artifacts and docs match; no open blocker or known data-loss issue remains. |

### 0.5 Usage Ranking Boundaries

Skill Usage Rankings are explicitly labeled `Locally observed calls`: a local
discovery signal, not a community leaderboard or a trust score. The default ranking includes currently managed skills (with
zero rows when no events were observed) plus unmanaged skills that appear in
the selected local time window. Coverage is limited to events stored in the
local SkillBox database. It does not upload or merge usage across devices. A
zero count means that SkillBox has not observed a call; disabled, untrusted, or
unsupported hooks can make recorded usage incomplete. Users can optionally run
one `Sync histories` action to backfill auditable local history from Codex,
Claude Code, and Cursor without changing those agents' configuration. Codex
only accepts explicit user-input carriers containing a complete `<skill>` block
or an explicit `[$skill](.../SKILL.md)` link. Claude Code only accepts
structured Skill tool/command attribution that resolves to a real `SKILL.md`.
Cursor only accepts explicitly attached `context.cursorRules` entries that
resolve to a real `SKILL.md`; its private SQLite schema is validated, opened
read-only, and rejected when incompatible. Assistant/tool prose and chat bodies
are never copied. Every provider deduplicates stable event identities, so
repeated scans remain idempotent.

Every ranking response includes coverage for the selected filters: earliest and
latest observed event times, mutually exclusive canonical stored-origin counts
for `agent_hook`, `codex_session_backfill`,
`claude_code_session_backfill`, `cursor_session_backfill`, and other events,
plus provider-specific session/file counts from the latest history scans. Event
origin counts sum to the locally observed total; scan counts are operational
coverage and do not change with ranking filters.

Ranking order must be deterministic: observed call count descending, most
recent observed use descending, then skill name ascending. Unmanaged rows are
labeled Not imported; Codex `*/.system/` skills are labeled System and are not
importable from Rankings; sources missing from the skill's observed runtime
roots are labeled Deleted. Prompt excerpts and event metadata never affect
ranking, and local usage frequency never changes source trust, safety, or
quality classification.

Future Codex reported runs remain a separate metric and storage boundary. They
must retain provider, subject kind, time window, scope, and provenance, must not
be written to `skill_usage_events`, and must never be included in local ranking,
total, or delta values.

### 0.6 Runtime Profile Boundaries

The built-in `agents`, `codex`, `claude-code`, `cursor`, and
`custom-skill-md` profiles all manage the current `SKILL.md` directory format.
Profiles own root discovery, precedence, accepted frontmatter capabilities, and
symlink deployment compatibility. They do not claim native support for
non-`SKILL.md` Claude, OpenClaw, Cursor, Claude Code, or Copilot formats.

Compatibility checks preserve unknown optional frontmatter and report it as a
warning. Malformed metadata, required incompatibilities, unsafe paths, foreign
targets, and unsupported deployment modes block deployment. SkillBox does not
rewrite frontmatter, translate formats, select a target automatically, or write
to a runtime before a fresh preview is explicitly confirmed.

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

- FTS-backed search across skills, operations, and usage history;
- remote source provenance and trust classification without treating popularity
  as proof of safety;
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
