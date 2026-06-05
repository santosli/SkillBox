# SkillBox - Manage agent skills from one local source of truth

> A local macOS app and CLI for organizing, importing, deploying, syncing, and updating agent skills across multiple agent runtimes.

![Status](https://img.shields.io/badge/status-local--first%20MVP-blue)
![Platform](https://img.shields.io/badge/platform-macOS-111827)
![Tauri](https://img.shields.io/badge/Tauri-2-24C8DB)
![Rust](https://img.shields.io/badge/Rust-core-B7410E)
![Node.js](https://img.shields.io/badge/Node.js-legacy%20CLI-43853D)

![SkillBox dashboard](docs/screenshots/skillbox-dashboard.png)

SkillBox is a local tool for managing `SKILL.md`-based skills, rules, prompts, and capability packs without treating any one agent runtime as the source of truth. User-created skills live in `~/.skillbox/user-skills`, remote skills live in versioned snapshots under `~/.skillbox/remote-skills`, and agent runtime folders such as `~/.codex/skills`, `~/.agents/skills`, and `~/.claude/skills` are deployment targets.

The project currently ships a Tauri + React desktop shell, Rust crates for the core management workflows, a Rust CLI, and a legacy Node CLI that still carries the transition-era GitHub install entrypoint.

## Why

- **One managed store for every runtime.** SkillBox keeps durable state in `~/.skillbox` and deploys into agent folders instead of letting runtime directories become the only copy.
- **Review before import.** Local scans produce import candidates first, including suggested user/remote classification, system-skill handling, conflicts, and usage counts.
- **Symlink deployment by default.** Runtime targets point back to the managed store, so user skill edits and remote rollbacks can take effect without copy churn.
- **Rust-backed workflows.** Desktop commands call Rust crates for scan, import, deploy, GitHub URL parsing, user-skill Git sync, remote source binding, update checks, update/rollback previews, workspace registry, usage hooks, and operation history.
- **Safety over silent mutation.** Existing non-symlink runtime content is not overwritten silently; imports use managed copies and backup-aware replacement paths.

## Screenshots

![SkillBox skill card detail](docs/screenshots/skillbox-dashboard-card.png)

Skill cards make usage and maintenance state visible at a glance, including call counts, update status, tags, favorites, and deployed runtime targets.

![SkillBox workspaces](docs/screenshots/skillbox-workspaces.png)

The Workspaces view tracks global and project-local skill roots across Codex CLI, Claude Code, Codex App, and project-specific runtimes.

![SkillBox history](docs/screenshots/skillbox-history.png)

History combines real skill calls and management operations, with prompt excerpts redacted down to small, reviewable snippets.

![SkillBox import review](docs/screenshots/skillbox-import-review.jpg)

Import review keeps local scans explicit: candidates are classified before SkillBox copies them into the managed store.

## What SkillBox Manages

```text
~/.skillbox/
  user-skills/
    <skill-name>/
      SKILL.md
  remote-skills/
    <skill-name>/
      source.json
      current -> versions/<version>
      versions/
        <version>/
          SKILL.md
  backups/
  skillbox.sqlite
```

Runtime directories are deployment targets:

- `~/.codex/skills`
- `~/.agents/skills`
- `~/.claude/skills`
- project-local `.codex/skills`
- project-local `.agents/skills`
- project-local `.claude/skills`

Longer-term support for Claude, OpenClaw, Cursor, Claude Code, Copilot, and other native formats should go through explicit agent adapters rather than hard-coded UI behavior.

## Features

- Scan local `SKILL.md` roots and return sorted skills with frontmatter metadata, content hashes, symlink status, and scan errors.
- Import existing local skills into `~/.skillbox/user-skills` or `~/.skillbox/remote-skills`.
- Deploy and undeploy managed skills into runtime folders through symlinks.
- Parse GitHub tree, blob, raw, and contents API URLs that point to skill directories or `SKILL.md`.
- Track remote GitHub sources, check for updates, preview all-file diffs, apply updates, and roll back to immutable versions.
- Manage workspace roots for global and project-local runtimes.
- Sync user skills through a shared Git repository with desktop diff review and Conventional Commit message generation.
- Record usage events from Codex App, Codex CLI, and Claude Code CLI hooks without storing full chat transcripts.
- Browse desktop operation and usage history from SQLite-backed records.

## Install

SkillBox is currently run from source. There is no packaged release in this repository yet.

### Requirements

- macOS
- Node.js and npm
- Rust stable and cargo
- Git
- Tauri CLI through the `apps/desktop` workspace dependency

If `cargo` is not available in a fresh shell, load Rust's environment first:

```sh
source ~/.cargo/env
```

### From A Local Checkout

```sh
npm install
npm run hooks:install
cargo test --offline
npm test
```

`npm install` runs the hook installer, which points Git at the tracked `.githooks/` directory. The pre-commit hook checks whether staged implementation or workflow changes require matching docs updates.

## Desktop App

Run the browser preview:

```sh
npm --workspace apps/desktop run dev
```

Run the Tauri desktop app:

```sh
npm --workspace apps/desktop run tauri dev
```

`tauri dev` loads `http://127.0.0.1:1420`. The Vite dev server is configured with `--strictPort`, so port `1420` must be free before starting the desktop app.

Build the frontend:

```sh
npm --workspace apps/desktop run build
```

## CLI

The Rust CLI is the target CLI surface:

```sh
cargo run -p skillbox-cli -- paths
cargo run -p skillbox-cli -- scan ~/.codex/skills ~/.agents/skills ~/.claude/skills
cargo run -p skillbox-cli -- import ./path/to/skill --type user
cargo run -p skillbox-cli -- deploy my-skill --target ~/.codex/skills
cargo run -p skillbox-cli -- workspace-scan
cargo run -p skillbox-cli -- check-remote-updates
```

The legacy Node CLI remains available for compatibility and the current GitHub install workflow:

```sh
node packages/skillbox-cli/bin/skillbox.js scan --json
node packages/skillbox-cli/bin/skillbox.js paths --json
node packages/skillbox-cli/bin/skillbox.js parse-github-url <github-skill-url> --json
node packages/skillbox-cli/bin/skillbox.js install <github-skill-url> --json
```

## Architecture

```text
React desktop UI
  -> Tauri commands
  -> skillbox-core / skillbox-github / skillbox-git
  -> local filesystem, SQLite, Git, and structured GitHub source metadata
```

Workspace layout:

```text
apps/desktop/              Tauri + React desktop app
apps/desktop/src-tauri/    Tauri command bridge
crates/skillbox-core/      scan, import, deploy, SQLite, workspaces, updates, hooks
crates/skillbox-github/    GitHub skill URL parsing and normalization
crates/skillbox-git/       structured Git service boundary
crates/skillbox-cli/       Rust CLI
packages/skillbox-core/    legacy Node core
packages/skillbox-cli/     legacy Node CLI
docs/                      architecture, data model, workflows, ADRs
```

New core business logic should go into Rust crates. React should call structured Tauri commands instead of owning filesystem, Git, GitHub download, migration, or rollback behavior.

## Safety Model

- `~/.skillbox` is the managed source of truth.
- Runtime folders are treated as deployment targets, not durable state.
- Existing non-symlink runtime skills are not overwritten silently.
- Import replacement paths must preserve backups or refuse the operation.
- GitHub URLs, downloaded archives, external paths, and runtime skills are treated as untrusted input.
- Product code uses structured parameters for Git and external commands instead of executing user-provided shell strings.
- Usage hooks record small usage events and prompt excerpts, not full conversation bodies.

## Docs

- [Architecture](docs/architecture.md)
- [Data model](docs/data-model.md)
- [Workflows](docs/workflows.md)
- [Implementation status](docs/implementation-status.md)
- [Contributing](CONTRIBUTING.md)
- [Managed store ADR](docs/decisions/0001-managed-store-is-source-of-truth.md)
- [Symlink deployment ADR](docs/decisions/0002-symlink-deployment-by-default.md)
- [Rust core migration ADR](docs/decisions/0003-migrate-node-cli-behavior-to-rust-core.md)
- [Agent adapter ADR](docs/decisions/0004-support-multiple-agent-runtimes-through-adapters.md)

## Development Checks

```sh
npm test
cargo test --offline
npm --workspace apps/desktop run build
npm run docs:check-staged
```

For UI changes, also run the Vite or Tauri app and verify the affected screen manually.

## Current Boundaries

- The first implementation phase focuses on `SKILL.md` directories and Codex-style runtime roots.
- GitHub install is still a legacy Node CLI workflow; Rust covers URL parsing, remote source binding, update checks, version previews, update apply, rollback apply, and operation logging.
- Rust and Node SQLite schemas are not fully unified yet.
- Copy-snapshot deployment and native adapters for non-directory skill formats are future work.

## License

This repository does not currently include a license file.
