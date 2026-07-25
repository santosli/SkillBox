# Contributing To SkillBox

SkillBox is a local macOS app and CLI for managing agent skills. Contributions
are welcome, especially bug reports, install feedback, tests, and small focused
patches.

## Development Environment

Install:

- Node.js and npm
- Rust stable with `cargo`, `rustfmt`, and the macOS targets needed for Tauri
- Git
- Xcode Command Line Tools on macOS

Install dependencies:

```sh
npm install
```

Run the desktop app in development:

```sh
npm --workspace apps/desktop run tauri:dev
```

The Tauri dev shell loads the Vite dev server at
`http://127.0.0.1:1420`. Keep that port free because the dev config uses
`--strictPort`. On macOS, the development app starts without activation, then
returns to the regular Dock policy, restores the previously active
application, and creates an unfocused window. Automatic Rust rebuilds therefore
remain available in the Dock without interrupting work in another app. The
development app is named `SkillBox Dev` and uses its own bundle identifier.
Production builds retain the `SkillBox` name and launch-focus behavior from
`tauri.conf.json`.

## Branch And Agent Workflow

Create complex changes on a `codex/<short-slug>` branch before editing files.
Do not develop or accumulate a complex change directly on `main`. An existing
non-`main` branch may be reused when its scope matches the task.

A change is complex when any of the following applies:

- it is a major change under Documentation Expectations;
- it crosses two or more product layers or module boundaries;
- it affects persistence or migrations, recovery, trust or security boundaries,
  destructive filesystem behavior, runtime adapters, signing, release, or
  distribution;
- it contains two or more independently verifiable implementation tasks or is
  expected to require multiple focused commits.

Check `git status --short --branch` first. Create the branch from the latest
available `main` when the worktree is clean. Never discard or automatically
stash existing work to satisfy this rule. If current-task changes already exist
on `main`, create the task branch while preserving them. Stop and report the
conflict when unrelated changes cannot be isolated safely.

Read-only work and small, single-file changes that do not meet the criteria
above do not require a task branch. The canonical release automation, which
explicitly starts from a clean `main`, is the normal exception; feature
implementation must already have been completed and integrated through a task
branch. Other exceptions require explicit user authorization.

For agent-assisted complex work, identify independently executable subtasks
before implementation. Delegate at least one bounded subtask when agent capacity
is available and file or state ownership does not overlap. The coordinating
agent owns branch setup, task boundaries, integration, Git operations, full
verification, and the final report. The main conversation should summarize key
decisions, progress, and integrated results instead of reproducing every
subtask's internal process.

Agents share one worktree. Do not assign overlapping files concurrently, and do
not run concurrent branch switches, staging, commits, merges, releases,
repository-wide formatting, or bulk generators. Every delegated task must state
its file scope and acceptance criteria, and the coordinating agent must inspect
the resulting diff and verification evidence.

Delegation may be skipped when the task is small, strictly sequential, cannot
be divided without overlapping files or shared mutable state, involves a
destructive or unique external operation, has no available agent capacity, or
would cost more to coordinate than to perform directly. Record the reason in
the task progress update.

## Test Commands

Run the JavaScript tests:

```sh
npm test
```

Run Rust tests:

```sh
cargo test --offline
```

Run formatting and whitespace checks:

```sh
cargo fmt --check
git diff --check
```

For UI changes, also run the app and verify the affected workflow manually.

## Architecture Rules

- Put business logic in Rust crates, primarily `crates/skillbox-core`,
  `crates/skillbox-git`, or `crates/skillbox-github`.
- React components should render state, manage interaction, and call Tauri
  commands. They should not directly own filesystem, Git, GitHub, download,
  migration, or rollback behavior.
- Tauri commands should convert parameters and call Rust core logic.
- Do not execute user-provided shell strings. Use structured arguments and
  validated paths.
- Treat GitHub URLs, downloaded content, existing runtime folders, and external
  paths as untrusted input.
- Do not silently overwrite existing non-symlink runtime targets.
- Preserve user-created skill content unless a destructive operation was
  explicitly confirmed.
- New agent ecosystems should go through an adapter or compatibility layer
  instead of hard-coding one agent format globally.

## Documentation Expectations

Documentation is part of the definition of done for every major change. Update
the relevant docs in the same change set as the implementation; do not defer
them until release preparation.

A change is major when it adds, removes, or materially changes any of the
following:

- a user-visible workflow, CLI/Tauri contract, or supported runtime;
- storage layout, SQLite schema, migration, backup, recovery, or compatibility;
- module boundaries, adapters, source-of-truth rules, trust boundaries, or
  destructive-operation safeguards;
- milestone scope, ordering, completion status, or promotion gates;
- installation, upgrading, signing, release, rollback, or distribution.

Use this mapping to decide which documents must change:

| Change | Required documentation |
| --- | --- |
| Milestone scope or status | `docs/roadmap.md` and `docs/implementation-status.md` |
| Architecture or trust boundary | `docs/architecture.md` and, when the decision is durable, `docs/decisions/*` |
| Storage, schema, migration, or recovery | `docs/data-model.md` |
| User workflow or definition of done | `docs/workflows.md` |
| User-visible feature or release content | `README.md`, `README.zh-CN.md`, and `CHANGELOG.md` as applicable |
| Development, testing, or release policy | `CONTRIBUTING.md` and the relevant release documentation |

Useful starting points:

- [README.md](README.md)
- [docs/roadmap.md](docs/roadmap.md)
- [docs/good-first-issues.md](docs/good-first-issues.md)
- [docs/architecture.md](docs/architecture.md)
- [docs/data-model.md](docs/data-model.md)
- [docs/workflows.md](docs/workflows.md)
- [docs/implementation-status.md](docs/implementation-status.md)
- [docs/decisions](docs/decisions)

The repository installs Git hooks through `npm install`. The pre-commit hook
checks whether staged implementation or workflow changes need matching docs. If
you have verified that a small internal change does not affect any category
above, commit with:

```sh
SKILLBOX_SKIP_DOCS_CHECK=1 git commit -m "type(scope): summary"
```

Do not use this escape hatch for a major change. A major change is not complete,
must not advance a roadmap milestone, and must not be released until its docs
and verification evidence are current.

## Commit Messages

Use Conventional Commits:

```text
<type>(<scope>): <summary>
```

Allowed types: `feat`, `fix`, `docs`, `test`, `refactor`, `chore`, `build`,
`ci`, `perf`, `style`.

Preferred scopes: `desktop`, `core`, `cli`, `scan`, `import`, `docs`, `hooks`,
`github`.

Examples:

```text
fix(import): skip system skills during import review
ci(release): add signed macos alpha build
docs(readme): document alpha install paths
```

## Pull Request Checks

CI runs the repository's baseline quality gates on pull requests and pushes to
`main`:

- `npm test`
- `cargo fmt --check`
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`
- `cargo test --offline`
- `git diff --check`

CI also runs dependency security checks:

- `cargo audit`
- `npm audit --audit-level=high`

Dependabot checks npm, Cargo, and GitHub Actions dependencies weekly.

## Starter Issues

Issues labeled `good first issue` should be small, testable, and avoid
destructive filesystem behavior. Maintainers should use the `Starter task`
issue template for this work and include likely files, acceptance criteria,
verification commands, and guardrails.

See [docs/good-first-issues.md](docs/good-first-issues.md) for the contributor
and maintainer checklist.

## Release Invariants

Public releases must be:

- tagged as `v*`;
- built as universal macOS DMGs;
- signed and notarized before direct install instructions are published;
- accompanied by `SHA256SUMS` and a DMG-specific `.sha256` asset;
- compatible with the Homebrew tap cask;
- clear that `~/.skillbox` is user data and is not removed by normal uninstall.

Pre-release alpha tags may use `v*-alpha.*`, but stable releases use normal
semantic version tags such as `v0.1.1`.

The release workflow expects these GitHub Actions secrets:

- `APPLE_CERTIFICATE`
- `APPLE_CERTIFICATE_PASSWORD`
- `APPLE_SIGNING_IDENTITY`
- `APPLE_ID`
- `APPLE_PASSWORD`
- `APPLE_TEAM_ID`
- `KEYCHAIN_PASSWORD`

See [docs/release.md](docs/release.md) for the release checklist.
