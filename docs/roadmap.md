# Roadmap

SkillBox is early-stage software. This roadmap describes the public direction,
not a date-based commitment. Implementation details can change as the app gets
more real-world use.

## Current Focus: 0.9.x

SkillBox `v0.8.0` is shipped. The 0.6 line made `SKILL.md` deployment targets
explicit and portable without moving runtime knowledge into React:

- a versioned Rust runtime-profile registry for Agents, Codex, Claude Code,
  Cursor, and exact custom `SKILL.md` roots;
- deterministic discovery precedence and schema-backed workspace identity;
- read-only frontmatter and deployment compatibility previews;
- stale-preview protection and explicit confirmation before runtime writes.

The runtime-profile and evidence-aware usage work passed release qualification
through the signed and notarized `v0.6.1` distribution, and the v0.7 line added
reviewed inbound Git sync plus the History query/layout patch in the signed
and notarized `v0.7.1` distribution. The current release identity and
distribution invariants remain documented in `docs/release.md`.

Copy-snapshot deployment and broader recovery hardening remain planned product
work. Git-backed Skill Collections are the active `v0.9.0` milestone; Phase A+B
shipped in `v0.8.0`, and the Phase C implementation is complete for `v0.9.0`
and remains unreleased pending `v0.9.0` release qualification. Collection-level update/rollback
remains planned Phase D work for a later `v0.9.x` release.

## Near-Term Priorities

These are the next areas where focused contributions are most useful:

- **Git-backed Skill Collections (v0.9.0).** Rust now treats one
  canonical Git repository/worktree and reviewed SHA as a local collection
  source while keeping child `SKILL.md` directories independently selectable
  and deployable. Phase C's one-fetch GitHub preview/apply is implemented for
  `v0.9.0` and remains unreleased pending `v0.9.0` release qualification; collection
  update/rollback remains open. Track
  the remaining scope in [GitHub issue #46](https://github.com/santosli/SkillBox/issues/46).
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
| **0.7 — Safe sync, deployment, and recovery** | Reviewed inbound user-skills Git updates are shipped; copy-snapshot deployment and broader restore/audit hardening remain planned follow-ups to the existing symlink path. | Worktree and branch-relation states are explicit; conflicts are never auto-merged; incoming trees, stale previews, deployed deletion/rename blockers, backup refs, index reconciliation, overwrite protection, and recovery paths have automated coverage. |
| **0.8 — Skill Collections foundation and product hardening** | Git-backed Skill Collections add repository-level discovery, persisted reviewed provenance, installed-source display grouping, and large-library import-review performance. | Collections use one canonical local repository/worktree identity, preserve per-skill selection/deploy/Calls independence, and pass local grouping, migration, recovery, and untrusted-input tests with CLI/Tauri parity. |
| **0.9 — Reviewed GitHub Skill Collection install** | Phase C adds one-fetch GitHub multi-skill preview/apply with explicit child selection and one reviewed commit SHA. Collection-level update/rollback remains later Phase D work. | Remote trees are bounded and fail closed before checkout; source/ref/tree/selection/managed state are stale-checked; selected children apply transactionally/compensatably; CLI/Tauri parity, recovery, and desktop review gates pass. |
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

### 0.9 Git-backed Skill Collections

[Git-backed Skill Collections](https://github.com/santosli/SkillBox/issues/46)
are targeted for `v0.9.0`. Phase A+B shipped in `v0.8.0`: a collection is a
repository/source entity identified by its canonical Git repository or
worktree plus ref/HEAD. A GitHub remote is optional. Child `SKILL.md`
directories remain independent skills for selection, deployment, Calls, and
history, while the collection follows one reviewed commit SHA so children do
not silently drift across unrelated revisions.

Local Import Review will resolve candidate real paths and their nearest safe
Git root, group normal repositories and Git worktrees by canonical repository
identity, treat nested repositories as separate collections, and map runtime
symlinks back to the same collection. Similar copies outside Git metadata stay
standalone or unlinked because content similarity alone cannot establish
collection membership. The collection card will show repository path,
optional remote, branch/HEAD, skill count, and a searchable expandable child
list with individual and select-all controls; it will never deploy children
automatically.

When a copied runtime installation has no live Git metadata, a supported v3
installer lockfile may provide a display-only `installed_source` grouping by
validated canonical GitHub source URL. Live Git identity remains authoritative;
lockfile groups never fabricate branch/HEAD, fetch/update, or collection apply
permissions, and child imports continue through the ordinary per-skill safety
contract.

A remote repository URL will use one bounded fetch/check to preview all valid
children. Preview reports eligible/invalid children, duplicate-name and managed
conflict diagnostics, plus bounded path/name conflicts; it does not claim a
removed-child diff because collection updates are Phase D. Apply installs only
explicitly selected child snapshots while retaining collection provenance.
Runtime deployment, Calls, and History remain per-skill; collection-level
update/rollback remains Phase D.

Delivery is phased across the `v0.8.0` and `v0.9.x` releases:

1. **Implemented:** Repository detection and local Import Review grouping.
2. **Implemented:** Collection/source persistence and child relationships.
3. **Implemented for `v0.9.0` / awaiting qualification:** GitHub multi-skill
   install preview/apply with one repository fetch. It is not part of the
   published `v0.8.0` release; it is implemented on main and remains
   unreleased pending `v0.9.0` release qualification.
4. Collection-level update/rollback and UI detail.

Phase 4 is not implemented and must not be described as shipped.

All scans are read-only. Apply must recheck canonical Git root, HEAD/ref, and
tree snapshot and reject stale previews. Collection operations must never run
hooks, submodules, filters, repository scripts, or arbitrary shell strings.
Existing traversal, symlink escape, size/count, non-symlink overwrite,
backup/revert, and duplicate-name protections remain mandatory. Duplicate
child names and managed-skill conflicts require explicit resolution; invalid
children are blocked rather than silently imported.

The `v0.8.0` acceptance gates include a many-skill local repository appearing
as one collection with N children, runtime symlinks avoiding duplicates,
standalone copies remaining standalone, one fetch/check per remote repository,
one-SHA consistency, explicit child selection, independent per-skill deploy and
Calls behavior, rollback/recovery and untrusted-tree coverage, and CLI/Tauri
parity for core collection operations.

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
