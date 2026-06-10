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
npm --workspace apps/desktop run tauri dev
```

The Tauri dev shell loads the Vite dev server at
`http://127.0.0.1:1420`. Keep that port free because the dev config uses
`--strictPort`.

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

Update docs when a change affects user-visible behavior, workflows, storage,
schema, release behavior, or long-term architecture. Useful starting points:

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
you have verified that no docs update is needed, commit with:

```sh
SKILLBOX_SKIP_DOCS_CHECK=1 git commit -m "type(scope): summary"
```

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
