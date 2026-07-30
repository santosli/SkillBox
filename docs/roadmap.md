# Roadmap

SkillBox is early-stage software. This roadmap describes the public direction,
not a date-based commitment. Implementation details can change as the app gets
more real-world use.

## Current Focus: 0.7

SkillBox `v0.6.1` is shipped. The 0.6 line made `SKILL.md` deployment targets
explicit and portable without moving runtime knowledge into React:

- a versioned Rust runtime-profile registry for Agents, Codex, Claude Code,
  Cursor, and exact custom `SKILL.md` roots;
- deterministic discovery precedence and schema-backed workspace identity;
- read-only frontmatter and deployment compatibility previews;
- stale-preview protection and explicit confirmation before runtime writes.

The runtime-profile and evidence-aware usage work passed release qualification
through the signed and notarized `v0.6.1` distribution. The current release
identity and distribution invariants remain documented in `docs/release.md`.

The next implementation target is 0.7: reviewed inbound sync, safer deployment
choices, and stronger recovery. These capabilities are planned, not shipped.

## Near-Term Priorities

These are the next areas where focused contributions are most useful:

- **Reviewed inbound sync.** Report ahead/behind/diverged Git state and allow
  explicit fast-forward updates without automatically merging conflicts.
- **Deployment portability.** Add copy-snapshot deployment as an explicit
  alternative to the current compatibility-checked symlink path.
- **Recovery workflows.** Strengthen restore previews, backup inspection, and
  audit evidence without deleting or overwriting user content silently.
- **Rust CLI and desktop parity.** Keep shared workflows aligned on the Rust
  core while documenting intentional CLI automation and desktop interaction
  differences.
- **Search and navigation.** Add FTS-backed search for skills, operations, and
  usage history on top of the versioned SQLite schema.
- **Remote trust.** Preserve source provenance and show explicit trust state
  before install or update without using popularity as proof of safety.
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
| **0.5 — Local usage discovery and release awareness** | Evidence-aware local skill rankings with time-range, skill-type, agent, and workspace filters; separate Calls and history references; auditable multi-provider history sync; daily signed app-update awareness without automatic downloads. | CLI and desktop reconcile confirmed/inferred/reference evidence consistently; Calls never include low-signal references or claim provider-native totals; history providers resolve real local skills, deduplicate and upgrade stable identities, and preserve successful imports when another provider fails; schema upgrades and representative ranking queries are tested; update checks are rate-limited and every install is revalidated after an explicit click. |
| **0.6 — Runtime profiles and portability** | Rust-owned runtime profiles model roots, precedence, frontmatter capabilities, and compatibility without hard-coding agent behavior in React. | Each supported profile has fixtures and compatibility tests; unsupported fields are reported before deployment; runtime-specific behavior remains behind an adapter boundary. |
| **0.7 — Safe sync, deployment, and recovery** | Reviewed inbound user-skills Git updates, copy-snapshot deployment, and stronger restore/audit workflows complement the existing symlink path. | Ahead/behind/diverged states are explicit; conflicts are never auto-merged; overwrite protection, rollback, and backup restoration have automated coverage. |
| **0.8 — Product hardening** | Large-library performance, actionable diagnostics, accessibility, onboarding, and recovery behavior are ready for sustained daily use. | Performance budgets and critical UI workflows are verified; no known data-loss path remains; supported upgrade and recovery procedures are documented and exercised. |
| **0.9 — Release candidate** | Feature scope is frozen while security, migration compatibility, packaging, updater, Homebrew, and real-world beta feedback are closed out. | Threat-model review is complete; upgrades from every supported prior release are tested; blocker defects are closed; signed and notarized distribution rehearsals pass. |
| **1.0 — Stable local skill management** | SkillBox offers a documented, supportable contract for discovering, importing, managing, deploying, updating, synchronizing, diagnosing, and recovering supported skills. | Core workflows meet their definitions of done; supported runtimes and limitations are explicit; migrations and recovery are proven; release artifacts and docs match; no open blocker or known data-loss issue remains. |

### 0.5 Usage Evidence And Ranking Boundaries

SkillBox classifies local usage evidence as `confirmed`, `inferred`, or
`reference`. Compact UI uses `Calls`, defined as confirmed execution evidence
plus defensible structured invocation evidence. History references remain a
separate secondary metric and never increase Calls or ranking order. These
local metrics are not a community leaderboard, trust score, or account-level
analytics. A zero count means SkillBox currently has no Calls evidence; disabled
hooks and unsupported provider events can make it incomplete.

`Sync histories` is explicit and read-only toward provider stores. Codex accepts
complete per-turn `<skill>` blocks or `[$skill](.../SKILL.md)` links with an
absolute path as inferred invocation, while catalog/prose, shell/tool payloads,
and outputs are excluded. Claude Code native Skill tool/command attribution is
confirmed. Cursor state `context.cursorRules` is a reference; a bounded agent
transcript assistant `Read` of an absolute, allowed `SKILL.md` is inferred and
deduplicated once per transcript user turn and skill. Safe historical-missing
paths remain evidence-only, and `ReadFile` stays diagnostic-only until qualified.
Repeated scans deduplicate stable event identities;
stronger evidence upgrades the existing event without dropping provenance.

Every ranking response includes mutually exclusive current evidence-class
totals and time coverage, with `confirmed + inferred = Calls` and reference
reported separately. Provenance source counts are intentionally not mutually
exclusive: one event may retain an inferred Codex source and a later confirmed
hook source, so source counts need not sum to Calls or event total. Latest
provider file/session/turn counts and backfill outcomes are operational
coverage, not ranking metrics.

Ranking order remains deterministic: Calls descending, most recent use
descending, then skill name and source identity ascending. Reference count does
not improve rank. Existing Not imported, System, Unknown source, and Deleted
boundaries remain source-aware. Prompt excerpts, event metadata, and usage
frequency never change source trust, safety, or quality classification.

schema v7 migrates existing events conservatively, retains provenance, and
idempotently rebuilds all-time stats from Calls without a rescan. A later,
explicit `Sync histories` may recover or upgrade evidence. `usage-audit` exposes
only aggregate evidence/source/time/scan/backfill totals and never returns
prompt, chat, tool payload/output, or complete metadata.

Codex local stores do not expose a stable provider-native skill-run total.
Codex Calls are therefore a known local undercount rather than an estimate to be
filled from prose or shell/tool traces. Future provider-reported runs remain a
separate storage/display boundary and never enter local ranking, total, or
delta values.

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
