# SkillBox Agent Guide

## Project Goal

SkillBox is a local macOS app for managing Codex-compatible skills.

It manages two categories of skills:

- User-created skills, stored under `~/SkillBox/user-skills`.
- Remote/downloaded skills, stored under `~/SkillBox/remote-skills`.

The managed directories are the source of truth. Runtime directories such as
`~/.codex/skills` or `~/.agents/skills` should be treated as deployment targets,
not as canonical storage.

## Default Architecture

- Desktop app: Tauri + React.
- Core logic: Rust crates for scanning, importing, installing, deploying,
  updating, rollback, and Git integration.
- Local index: SQLite.
- CLI: a small `skillbox` command that reuses the same core logic as the app.

Prefer keeping business logic in shared core crates. The desktop UI should call
core commands instead of owning file-system, Git, or GitHub behavior directly.

## Storage Model

- `~/SkillBox/user-skills` contains user-created skills.
- `~/SkillBox/remote-skills` contains remote skill snapshots and version history.
- A valid skill directory must contain `SKILL.md`.
- Remote skills should record their GitHub owner, repo, path, ref, installed
  commit SHA, and latest known commit SHA.
- User-created skills should be synchronized through one Git repository rooted at
  `~/SkillBox/user-skills`.

## Deployment Model

- Default deployment uses symlinks from runtime skill directories to managed
  skill directories.
- Do not silently overwrite an existing runtime skill directory.
- Any migration must validate the destination, create a backup or rollback path,
  and then rescan the resulting state.

## Development Rules

- Read the existing project structure before adding new abstractions.
- Keep file-system operations explicit, validated, and reversible where possible.
- Never execute user-provided shell strings directly. Use structured command
  arguments and canonicalized paths.
- Treat remote archives and GitHub paths as untrusted input.
- Preserve user-created skill content unless the user explicitly confirms a
  destructive operation.
- Prefer small, testable units for scanning, parsing, import, deployment, update,
  and rollback behavior.

## Verification Expectations

Every meaningful change should include either automated tests or a clear manual
verification note for the affected workflow.

Required workflows to keep verifiable:

- Scan local skill roots.
- Import existing skills into managed directories.
- Install a remote skill from a GitHub URL.
- Deploy a managed skill to a runtime directory.
- Check for remote updates.
- Update a remote skill while preserving version history.
- Roll back a remote skill to a previous version.
- Sync the user-created skills Git repository.

Before claiming a workflow is complete, run the relevant tests or commands and
report what was verified.
