# ADR 0006: Review Inbound User-Skills Git Before Fast-Forward

## Status

Accepted for the v0.7 implementation; release qualification is pending.

## Context

SkillBox already treats `~/.skillbox/user-skills` as one shared Git repository
and supports reviewed local commits with an optional push to `origin/main`.
Automatically pulling or resolving a second device's changes would weaken the
managed-store trust boundary: incoming files are untrusted, deployed runtime
symlinks point into this repository, and a Git merge can combine content that
the user never reviewed as one snapshot.

Worktree cleanliness and branch history are separate facts. A repository can be
dirty while also being behind or diverged, so one overloaded "sync state" is
not sufficient for a safe apply decision.

## Decision

Inbound updates use three explicit stages:

1. **Check remote** fetches `origin/main` and reports worktree state separately
   from the branch relation.
2. **Review incoming changes** validates the remote tree and presents the
   repository-wide skill/file diff, deployment impact, and conflict diagnosis.
3. **Apply fast-forward** requires the reviewed `preview_id`, re-fetches and
   revalidates every bound input, creates a backup ref, materializes only the
   reviewed Git blobs, and advances `main` with a compare-and-swap ref update.

The relation model is `unknown`, `synced`, `ahead`, `behind`, `diverged`,
`remote_only`, or `no_remote_branch`, while worktree state is independently
`clean` or `dirty`.

SkillBox does not automatically merge, rebase, reset, force-push, stash, choose
a side, insert conflict markers, or provide an in-app merge editor. Diverged
history must be resolved with normal Git tooling outside SkillBox. Fetching is
never performed on startup or in the background.

The preview identity binds the local and remote commit identities, merge base,
sanitized remote and branch, worktree state, validated incoming snapshot, file
changes, and deployment impact. Apply rejects stale or mismatched state before
working-tree writes. Incoming trees reject invalid skills, unsafe paths and file
types, Git metadata, traversal, and escaping symlinks. Incoming added, renamed,
or type-changing paths also reject collisions with ignored or untracked local
content, including ancestor and descendant collisions.

An update to a deployed skill is allowed only after the preview discloses its
targets. Deleting or renaming a deployed skill is blocked in v0.7; users must
undeploy it first. A local unborn repository may initialize from remote
`origin/main` only when it has no user content.

Before apply, when a local HEAD exists, SkillBox creates
`refs/skillbox/backups/inbound/<operation-id>` at that commit. After Git
advances, the user-skill SQLite index is reconciled transactionally. Apply,
outbound Git, managed user-skill mutations, and deploy/undeploy share one
mutation lock. The inbound materializer does not execute repository hooks,
filters, textconv, external diff, or merge drivers. If a mutation or reindex
fails, SkillBox compensates only while HEAD still equals the expected commit
and operation-owned paths remain unchanged; it refuses to erase unrelated
concurrent work. Apply also holds the Git index lock and preserves replaced or
deleted tracked files in an operation-scoped `.git/skillbox/` recovery snapshot,
so concurrent staging or worktree edits fail closed instead of being overwritten.
Operation history stores aggregate commit/ref/count,
mutation-phase, and compensation information, not credentials or diff content.
Displayed remote identities remove userinfo, query, and fragment secrets.

## Consequences

- Inbound and outbound workflows remain separate. Existing outbound
  `sync-user-skills` push defaults and `push_failed` semantics do not change.
- Incoming updates are repository-wide; selective per-skill apply is not
  offered.
- Fetch updates remote-tracking refs but remains read-only toward the worktree.
- Dirty repositories can still be inspected, but cannot be applied.
- `ahead`, `synced`, `diverged`, and `no_remote_branch` never modify the
  worktree through inbound apply.
- v0.7 is intentionally limited to `origin/main`.
- Backup refs are an internal recovery aid, not an automatic history rewrite or
  a substitute for normal Git conflict resolution.
- Conflict diagnosis is derived from commit/tree changes and never executes a
  repository-configured merge driver.
