# Local User Skills Git Sync Design

Date: 2026-05-25

## Scope

Implement a one-click Git sync workflow for local user-created skills stored in
`~/SkillBox/user-skills`.

This feature covers only the managed user skills repository. It does not sync
remote skills, runtime deployment directories, or agent-specific adapter output.
`~/SkillBox` remains the source of truth, and `~/.codex/skills`,
`~/.agents/skills`, and future agent runtime directories remain deployment
targets.

## User Goal

The user should be able to back up and push local user skills with one primary
action after a short first-time setup. The first setup records the Git remote.
Subsequent syncs should use that remote automatically.

## Entry Points

The primary entry point is the User Skill detail page.

- For a user skill with no configured sync remote, the detail page shows a
  primary `Set up sync` action in the sync panel and header action area.
- For a user skill with a configured `origin`, the detail page shows a primary
  `Sync now` action.
- The Settings page can show read-only sync metadata, such as repository path,
  branch, and remote URL. It is informational and not required for the MVP
  workflow.
- Remote skills keep their existing update-oriented language and do not show
  user-skills sync actions.

The action operates on the whole `~/SkillBox/user-skills` Git repository, not
only the currently selected skill. The UI copy should make that scope clear.

## First-Time Setup Flow

When `~/SkillBox/user-skills` is not a Git repository, or it is a Git repository
without an `origin` remote:

1. The user clicks `Set up sync`.
2. A modal asks for:
   - Remote URL: required.
   - Commit message: default `Sync user skills`, editable.
   - Push after commit: enabled by default.
3. On confirmation, the backend runs the structured workflow:
   - Ensure `~/SkillBox/user-skills` exists.
   - Run `git init -b main` if needed.
   - Add or update `origin` with the provided remote URL.
   - Run `git add .`.
   - Create a commit only if there are staged changes.
   - Push with upstream tracking when push is enabled:
     `git push -u origin main`.
4. The UI refreshes sync state and reports whether it initialized, committed,
   pushed, or found no local changes.

If there are no local changes during first-time setup, the workflow should not
create an empty commit. It should still push if requested, so an existing local
history can be connected to the remote.

## Subsequent Sync Flow

When `~/SkillBox/user-skills` is a Git repository with `origin`:

1. The user clicks `Sync now`.
2. The UI uses the default commit message `Sync user skills`.
3. The sync panel includes a collapsed `Sync options` disclosure. Expanding it
   reveals a commit message input prefilled with `Sync user skills`; the next
   click on `Sync now` uses the edited value.
4. The backend runs:
   - `git add .`
   - commit if the repository has staged changes
   - push to the configured upstream or `origin main`
5. The UI reports:
   - `Synced` when commit and push complete.
   - `Already synced` when there are no local changes and push succeeds or has
     nothing to send.
   - `Needs sync` when local changes remain.
   - `Push failed` when a local commit exists but the remote push failed.

## UI State Model

The sync panel should derive its state from a backend status command rather than
from local React-only assumptions.

States:

- `not_configured`: no Git repository or no `origin` remote.
- `clean`: repository exists, remote exists, no local changes.
- `dirty`: repository exists and local changes are present.
- `syncing`: a sync operation is running.
- `push_failed`: transient result after a sync attempt created or found local
  commits but push failed.
- `error`: status or sync command failed before a more specific state was known.

Recommended display:

| State | Primary action | Tone | Detail |
| --- | --- | --- | --- |
| `not_configured` | `Set up sync` | amber | Git remote required |
| `clean` | `Sync now` | green | Up to date locally |
| `dirty` | `Sync now` | amber | Local changes pending |
| `syncing` | disabled | slate | Sync in progress |
| `push_failed` | `Retry push` | red | Local commit kept |
| `error` | `Retry` | red | Show stderr summary |

The dashboard status badge for user skills can reuse this state, but the detail
page owns the full setup and retry interaction. `push_failed` does not need to
survive app restart in the MVP; after refresh, an unpushed local commit can be
shown as `dirty` or `ahead` depending on the Git status data available.

## Backend Contract

Add Rust-backed commands instead of extending React or the legacy Node CLI.

Suggested command names:

- `user_skills_git_status() -> UserSkillsGitStatus`
- `sync_user_skills_git(request: UserSkillsSyncRequest) -> UserSkillsSyncResult`

Suggested request:

```text
remote_url: optional string
commit_message: optional string, defaulting to "Sync user skills"
push: boolean, default true
```

Suggested status fields:

```text
repo_path
initialized
branch
remote_url
dirty
raw_status
state
last_error
```

Suggested result fields:

```text
repo_path
initialized
remote_updated
branch
dirty
raw_status
committed
commit_sha
pushed
push_attempted
state
message
```

The Rust implementation should live in core business crates, with Tauri only
bridging typed inputs and outputs. Git commands must use structured arguments,
never shell strings.

`crates/skillbox-git` may be expanded for reusable Git primitives such as
status, init, remote read/write, add, commit, and push. The workflow orchestration
can live in `skillbox-core` because it knows the managed paths.

## Error Handling

- Invalid or empty remote URL: reject before running Git and keep the setup modal
  open.
- Git init failure: return structured error and leave existing files untouched.
- Remote add or set-url failure: return stderr and do not proceed to commit.
- Commit failure: do not push; return stderr.
- Push failure: preserve local repository and local commit. The UI marks
  `Push failed` and lets the user retry.
- Missing Git binary: return a clear setup error.
- No commit message: use `Sync user skills`.

No workflow step should delete user skill content. No runtime directory should be
modified by this feature.

## Testing And Verification

Automated Rust tests should cover:

- Initializing an empty `user-skills` directory.
- Adding `origin` during first-time setup.
- Updating an existing `origin` when the user provides a new remote.
- No-op sync when there are no changes.
- Commit creation when files changed.
- Push failure preserving the local commit.
- Status mapping for uninitialized, clean, dirty, and push-failed cases.

UI tests or component-level tests should cover:

- User skill detail shows `Set up sync` when no remote exists.
- User skill detail shows `Sync now` when remote exists.
- Default commit message is `Sync user skills`.
- The commit message can be edited before sync.
- Remote skill detail does not show user-skills sync actions.

Manual verification should include:

- Run the Rust tests with `cargo test --offline`.
- Run the desktop tests with `npm test`.
- Start the desktop app and verify the setup modal, subsequent sync action, and
  push-failure state against a temporary local bare repository.

## Out Of Scope

- GitHub repository creation.
- Authentication setup for GitHub or other Git hosts.
- Syncing remote skills.
- Per-skill partial commits.
- Pull, merge, conflict resolution, or bidirectional sync.
- Syncing deployed runtime folders.
