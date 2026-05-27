# Remote Skill Updates Design

Date: 2026-05-27

## Scope

Implement a safe update workflow for remote skills stored in
`~/SkillBox/remote-skills`.

The MVP supports GitHub as the only network source provider. It covers source
binding, update checks, update previews, update execution, rollback previews,
rollback execution, and a generic operation log for auditable side-effecting
workflows. `~/SkillBox` remains the source of truth. Runtime directories such as
`~/.codex/skills`, `~/.agents/skills`, and project-local skills roots remain
deployment targets.

This design does not add GitLab, Gitee, Hugging Face, registry-backed sources,
or multi-agent adapter formats. The backend should keep provider boundaries
explicit so those sources can be added later without changing the remote skill
version model.

## User Goal

The user should be able to link a remote skill to its GitHub source, see whether
new versions are available, review the exact file changes before changing the
active version, update safely, and rollback safely.

No remote source match, update, or rollback should silently replace the current
skill version. Every operation that changes managed state should be recorded,
including failures and user cancellations.

## Source Binding Model

Remote skills have a source binding stored in
`remote-skills/<skill-name>/source.json`.

The source binding answers:

- Where did this skill come from?
- Which ref is being tracked or pinned?
- Which managed version is currently active?
- If the current version came from GitHub, which commit SHA is active?
- What is the latest known GitHub SHA for the bound source?

Suggested GitHub source metadata:

```json
{
  "type": "github",
  "owner": "openai",
  "repo": "skills",
  "path": "skills/example",
  "ref": "main",
  "refKind": "branch",
  "tracking": true,
  "repoUrl": "https://github.com/openai/skills.git",
  "url": "https://github.com/openai/skills/tree/main/skills/example",
  "currentVersion": "manual-abc123def456",
  "installedSha": null,
  "latestSha": "full-github-commit-sha",
  "sourceLinkedAt": "2026-05-27T00:00:00.000Z",
  "installedAt": "2026-05-27T00:00:00.000Z"
}
```

`currentVersion` is the active managed version directory and may be a manual
version or a GitHub commit SHA. `installedSha` is only set when the active
version came from GitHub. A manually imported remote skill can be bound to
GitHub without pretending that the current version is already a GitHub commit.

Manual sources keep the existing non-checkable shape:

```json
{
  "type": "manual",
  "currentVersion": "manual-abc123def456",
  "installedSha": "manual-abc123def456",
  "installedAt": "2026-05-27T00:00:00.000Z"
}
```

## GitHub Ref Semantics

The UI must explain ref behavior anywhere the user binds, installs, or reviews a
GitHub source.

| Ref kind | Display | Update behavior |
| --- | --- | --- |
| Branch | `Tracking branch: main` | Check the branch head for updates |
| Tag | `Pinned tag: v1.2.0` | Do not actively check for updates |
| Commit | `Pinned commit: abc123...` | Do not actively check for updates |

Branch refs are tracking sources. Tag and commit refs are pinned sources. A
pinned source can still be installed and rollback can still be used, but it does
not appear as update-checkable unless the user switches the source to a branch.

When the user enters a tag or commit source, the confirmation UI should say that
the source is pinned and SkillBox will not show update availability until the
source is changed to a branch.

## Automatic Source Matching

Automatic matching is only for finding candidates. It never writes
`source.json`, installs a version, updates `current`, or deploys anything.

Flow:

1. The user opens a remote skill with missing manual source metadata or a
   `manual` source.
2. The user clicks `Find source`.
3. SkillBox searches Claude Marketplace skills using the local skill name and
   path hints, then maps each accepted listing back to a GitHub source URL.
4. The candidate list shows repository, path, ref, source URL, owner,
   marketplace install signal, star count, updated time, and match reasons.
5. The user chooses one candidate to preview.
6. SkillBox fetches the candidate `SKILL.md` and validates it against the local
   skill.
7. The user can bind only after validation succeeds.

Candidate ranking:

1. Exact frontmatter `name` match.
2. GitHub path or directory name match.
3. Claude Marketplace install signal.
4. Repository trust signals such as owner and stars.
5. Recent listing activity.

Ranking is only a hint. The user must confirm the candidate explicitly.

## Manual Source Binding

Manual source binding accepts only GitHub URLs in the MVP:

- GitHub tree URL pointing to a skill directory.
- GitHub blob URL pointing to `SKILL.md`.
- GitHub raw URL pointing to `SKILL.md`.
- GitHub contents API URL pointing to a skill directory or `SKILL.md`.

The validation result has three states:

| Result | Meaning | Allowed MVP action |
| --- | --- | --- |
| `exact_match` | Local skill name and content hash match the GitHub candidate | Bind source |
| `same_skill_changed` | Skill name matches but content differs | Bind source only, do not replace current version |
| `mismatch` | Name differs or the remote content is not a valid skill | Reject by default |

For `same_skill_changed`, binding writes GitHub source metadata and
`latestSha`, but does not write `versions/<latestSha>` and does not switch
`current`. The row can immediately show `Update available` because the source is
linked and the remote version differs from the active version.

## Version Model

The existing immutable snapshot model remains the canonical model.

```text
remote-skills/
  <skill-name>/
    source.json
    current -> versions/<version>
    versions/
      <version>/
        SKILL.md
```

Rules:

- GitHub versions use the full commit SHA.
- Manual versions use `manual-<contentHash12>`.
- Version directories are immutable after creation.
- All historical versions are retained permanently.
- `current` is the only active version pointer.
- Rollback changes only `current` and metadata; it does not delete versions.
- Short SHA prefixes can be accepted for user input, but the resolved target
  must be unambiguous and `current` must point to the full version directory.

The `version` field inside `SKILL.md` frontmatter is display metadata only. It is
not the version directory key and must not drive update or rollback resolution.

## Update Check

Update checks are supported only for GitHub branch sources in the MVP.

Flow:

1. Read every remote skill `source.json`.
2. Mark missing source metadata as `no_source`.
3. Mark manual sources as `not_checkable`.
4. Mark pinned GitHub tag or commit sources as `pinned`.
5. For tracking GitHub branch sources, run a structured `git ls-remote` against
   `repoUrl` and `ref`.
6. Compare the returned full SHA with `currentVersion` and `installedSha`.
7. Return per-skill status without changing managed state.

Network and Git failures are returned per skill. A failed check does not modify
`source.json`, `current`, or any version directory.

## Update Preview And Execution

Update is a two-step workflow: preview first, execute second.

Preview flow:

1. Resolve the latest target SHA for the tracking branch.
2. Download the target version into a temporary directory.
3. Validate the target has a readable `SKILL.md`.
4. Validate the target skill name matches the local skill name.
5. Generate a file-level diff between `currentVersion` and the target version.
6. Return a `RemoteVersionChangePreview` to the UI.

Execution flow:

1. Require a successful preview token or equivalent validated preview identity.
2. Revalidate that the target SHA still matches the requested update target.
3. Write `versions/<latestSha>` if it does not already exist.
4. Atomically switch `current` to `versions/<latestSha>`.
5. Update `source.json.currentVersion`, `installedSha`, and `latestSha`.
6. Update SQLite skill metadata.
7. Return affected deployments and final status.

The old active version remains in `versions`. If any step after reading the old
`current` fails, SkillBox must restore `current` to its previous target before
returning failure whenever restoration is possible.

## Rollback Preview And Execution

Rollback uses the same review requirement as update.

Flow:

1. The skill detail page lists all historical versions.
2. The current version is labeled `Current`.
3. GitHub versions display a short SHA with full SHA copy or tooltip support.
4. Manual versions display `manual-<hash>` and available installed time.
5. The user selects a target version and clicks `Review rollback`.
6. SkillBox validates the target version exists and contains `SKILL.md`.
7. SkillBox generates a diff from `currentVersion` to the target version.
8. The user confirms rollback from the diff review dialog.
9. SkillBox atomically switches `current` to the target version and updates
   `source.json.currentVersion`.

Rollback does not delete versions and does not rewrite the selected target. If
the target input is a short SHA prefix, multiple matches must be rejected.

## Diff Review

Update and rollback must both go through a diff review page before execution.
If diff preview fails, the destructive action is disabled.

Suggested preview shape:

```text
skill_name
action: update | rollback
from_version
to_version
source
files:
  path
  old_path
  status
  label
  diff
  old_hash
  new_hash
  old_size
  new_size
  binary
  too_large
affected_deployments
preview_id
```

Diff rules:

- Every changed file must appear in the file list.
- Text files show unified diff.
- Added and deleted text files show full add/delete diff.
- Renamed files show rename information and content diff when content changed.
- Binary files show old hash, new hash, size change, and file type.
- Very large files show old hash, new hash, size change, and a `too_large`
  marker instead of rendering a full diff.

The desktop app should reuse the existing unified diff parser and renderer. The
current user-skills sync dialog is too Git-commit-specific to reuse wholesale,
so the shared piece should be a reusable diff review panel or component used by
both local sync and remote version change dialogs.

## Deployment Behavior

Remote update and rollback operate on the managed store. They do not redeploy
each target manually.

Expected deployment behavior:

- Runtime deployments that symlink to
  `remote-skills/<skill-name>/current` automatically follow the new active
  version.
- The completion view lists affected deployments.
- If a deployment points to a specific historical version directory, show
  `Deployment is pinned to an old version` and do not silently change it.
- If a runtime target is not a SkillBox-managed symlink, do not modify it.

This keeps deployment side effects explicit and preserves the managed store as
the only source of truth.

## Generic Operation Log

Operation logging is a generic SkillBox capability, not a remote-skill-specific
feature.

Every side-effecting operation should create an append-only operation record.
Failures and user cancellations must also be recorded. Read-only status checks
do not need success records in the MVP, but user-triggered check failures may be
recorded when useful for troubleshooting.

Suggested SQLite table:

```text
operations
  id TEXT PRIMARY KEY
  type TEXT NOT NULL
  status TEXT NOT NULL
  actor TEXT NOT NULL
  entity_type TEXT NOT NULL
  entity_name TEXT NOT NULL
  started_at TEXT NOT NULL
  finished_at TEXT
  summary TEXT NOT NULL
  error TEXT
  payload_json TEXT NOT NULL
```

Rules:

- Rust core writes operation records. React and CLI callers pass structured
  requests and actor metadata but do not write logs directly.
- Records are append-only from the UI perspective.
- Records are retained permanently in the MVP.
- Operations start as `started`, then become `succeeded`, `failed`, or
  `cancelled`.
- A single operation can contain `payload.steps` for internal progress instead
  of creating noisy top-level operation rows for each step.

Initial operation types:

```text
import_skill
deploy_skill
bind_remote_source
install_remote_skill
preview_version_change
update_remote_skill
rollback_remote_skill
sync_user_skills_git
set_user_skills_remote
add_workspace
forget_workspace
set_preference
```

Operation payloads should include relevant from/to state, changed paths,
affected deployments, backup paths, resolved source metadata, commit SHA, and
rollback restoration status where applicable.

## Entry Points

Dashboard:

- Remote skills with GitHub branch sources show `Up to date`,
  `Update available`, `Check failed`, or `Not checkable`.
- Pinned GitHub tag or commit sources show `Pinned`.
- Manual sources show `Not checkable` and can show `Find source` or
  `Bind GitHub source`.

Skill detail:

- `Find source` for manual or missing sources.
- `Bind GitHub source` for manual URL entry.
- `Check update` for tracking GitHub branch sources.
- `Review update` when an update is available.
- `Versions` list for rollback.
- `Operations` panel or link showing recent operation log entries for the
  selected skill.

Settings or a future Operations page can expose the global operation log with
filters by operation type, entity type, status, actor, and time.

## Backend Contract

The Rust core should own all filesystem, Git, GitHub download, validation,
version switching, SQLite, and operation-log behavior. Tauri should only bridge
typed commands. React should not run Git, read arbitrary filesystem paths, or
write managed store metadata directly.

Suggested commands:

```text
find_remote_source_candidates(skill_name) -> SourceCandidateSearchResult
preview_remote_source_binding(skill_name, source_url) -> SourceBindingPreview
bind_remote_source(request) -> OperationResult
check_remote_skill_updates() -> RemoteUpdateCheckResult
preview_remote_version_change(request) -> RemoteVersionChangePreview
apply_remote_version_change(request) -> OperationResult
list_remote_skill_versions(skill_name) -> RemoteSkillVersionList
list_operations(filter) -> OperationList
```

Suggested preview actions:

```text
remote_version_change.action = update | rollback
source_binding.validation = exact_match | same_skill_changed | mismatch
```

GitHub URL parsing should remain in `skillbox-github`. Structured Git commands
such as `ls-remote` should remain in `skillbox-git`. Managed-store orchestration
belongs in `skillbox-core`.

## Error Handling

- Invalid GitHub URL: reject before network access.
- Unsupported provider: reject with an MVP-specific message.
- Missing `SKILL.md`: reject preview or install.
- Skill name mismatch: reject source binding, update, and rollback by default.
- Search failure: show retryable error and leave managed state unchanged.
- Download failure: leave managed state unchanged.
- Diff generation failure: disable update or rollback execution.
- Existing version directory: validate it and reuse it if it matches the target
  version.
- `current` switch failure: attempt to restore the previous `current` target and
  record whether restoration succeeded.
- SQLite update failure after `current` switch: attempt restoration before
  returning failure.
- Operation log write failure before a side effect: reject the operation so the
  side effect is not unaudited.
- Operation log update failure after a side effect: return a warning in the
  result and keep the managed state consistent.

No failure path should delete user-created skill content or unmanaged runtime
content.

## Testing And Verification

Automated Rust tests should cover:

- GitHub URL parsing for tree, blob, raw, and contents API URLs.
- Source binding preview for `exact_match`, `same_skill_changed`, and
  `mismatch`.
- Binding a changed GitHub source without switching `current`.
- Writing `latestSha` after source binding.
- Branch sources being checkable.
- Tag and commit sources being pinned.
- No-op update when current GitHub SHA equals latest SHA.
- Update preview across text, add, delete, rename, binary, and large-file cases.
- Update execution writing a new immutable version and preserving the old one.
- Update failure restoring the previous `current`.
- Rollback preview and execution for full SHA and unambiguous short SHA.
- Rollback rejecting missing and ambiguous targets.
- Deployed symlink classification for `current`, pinned version, and unmanaged
  target.
- Operation records for success, failure, and cancellation.
- Permanent retention behavior by avoiding automatic pruning in all workflows.

UI tests should cover:

- Candidate ranking and match reason display.
- Ref behavior copy for tracking branch, pinned tag, and pinned commit.
- Binding a changed source displays `Update available` without replacing the
  current version.
- Remote update and rollback require diff review before confirmation.
- Diff review lists every changed file.
- Binary or too-large file rows display hash and size metadata.
- Versions list marks the current version and opens rollback review.
- Operation history shows failed operations.

Manual verification should include:

- Bind a manually imported skill to a GitHub branch source with changed content
  and confirm `current` does not move.
- Run update preview and verify all changed files are visible.
- Apply update and verify `current` points to the new SHA.
- Roll back to the manual version and verify `current` points back.
- Confirm an already deployed runtime symlink follows `current`.
- Confirm a deployment pinned to an old version is reported but not modified.

## Out Of Scope

- Providers other than GitHub.
- Private GitHub repository authentication flow.
- GitHub repository creation.
- Automatic source binding without user confirmation.
- Background or scheduled update installation.
- Automatic pruning of old remote skill versions.
- Editing operation log records from the UI.
- Full adapter support for non-`SKILL.md` agent-native formats.
- Runtime deployment migration for unmanaged directories.
