# Local User Skills Git Sync Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the approved one-remote Git sync workflow for `~/SkillBox/user-skills`.

**Architecture:** Add structured Git primitives to `skillbox-git`, orchestrate the managed `user-skills` workflow in `skillbox-core`, expose it through Rust CLI and Tauri commands, then wire the User Skill detail UI to status, setup, sync, and retry states. React remains presentation-only and never shells out.

**Tech Stack:** Rust crates (`skillbox-git`, `skillbox-core`, `skillbox-cli`, Tauri bridge), React/Vite desktop UI, Node test runner for component logic, Cargo tests for Rust behavior.

---

## File Structure

- Modify `crates/skillbox-git/src/lib.rs`: reusable structured Git commands for status, init, remote URL, add, commit, push.
- Modify `crates/skillbox-core/Cargo.toml`: depend on `skillbox-git`.
- Modify `crates/skillbox-core/src/lib.rs`: `UserSkillsGitStatus`, `UserSkillsSyncRequest`, `UserSkillsSyncResult`, status and sync workflow.
- Modify `crates/skillbox-cli/src/main.rs`: add `user-skills-status` and `sync-user-skills`.
- Modify `apps/desktop/src-tauri/Cargo.toml`: depend on `skillbox-git` indirectly through core only; no direct dependency needed unless compilation requires it.
- Modify `apps/desktop/src-tauri/src/lib.rs`: add `user_skills_git_status` and `sync_user_skills_git`.
- Modify `apps/desktop/src/App.jsx`: add state, setup modal, sync options, status mapping, and user-only header actions.
- Modify `apps/desktop/src/styles.css`: add sync panel/modal styles.
- Add or modify `apps/desktop/src/App.import-candidates.test.js`: pure UI helper tests for sync status/action derivation.
- Modify `docs/workflows.md`, `docs/architecture.md`, `docs/implementation-status.md`, and `CONTRIBUTING.md` after implementation changes, keeping docs aligned with the pre-commit rule.

## Task 1: Rust Git Primitives

**Files:**
- Modify: `crates/skillbox-git/src/lib.rs`

- [ ] **Step 1: Write failing tests for Git primitives**

Add tests under `#[cfg(test)]` in `crates/skillbox-git/src/lib.rs`:

```rust
#[test]
fn init_add_commit_and_status_report_clean_repo() {
    let temp = temp_dir("skillbox-git-clean");
    write_file(&temp.join("demo.txt"), "demo");

    init_main(&temp).unwrap();
    add_all(&temp).unwrap();
    let sha = commit(&temp, "Initial sync").unwrap();
    let status = status(&temp).unwrap();

    assert!(!sha.is_empty());
    assert!(status.initialized);
    assert_eq!(status.branch, "main");
    assert!(!status.dirty);
}

#[test]
fn remote_url_can_be_added_and_updated() {
    let temp = temp_dir("skillbox-git-remote");
    init_main(&temp).unwrap();

    set_origin_url(&temp, "https://example.com/one.git").unwrap();
    assert_eq!(origin_url(&temp).unwrap(), Some("https://example.com/one.git".to_string()));

    set_origin_url(&temp, "https://example.com/two.git").unwrap();
    assert_eq!(origin_url(&temp).unwrap(), Some("https://example.com/two.git".to_string()));
}
```

- [ ] **Step 2: Verify red**

Run: `cargo test -p skillbox-git --offline`

Expected: fails because `init_main`, `add_all`, `commit`, `set_origin_url`, and `origin_url` do not exist.

- [ ] **Step 3: Implement primitives**

Add public functions:

```rust
pub fn init_main(repo: impl AsRef<Path>) -> Result<(), String>;
pub fn origin_url(repo: impl AsRef<Path>) -> Result<Option<String>, String>;
pub fn set_origin_url(repo: impl AsRef<Path>, remote_url: &str) -> Result<(), String>;
pub fn add_all(repo: impl AsRef<Path>) -> Result<(), String>;
pub fn staged_changes(repo: impl AsRef<Path>) -> Result<bool, String>;
pub fn commit(repo: impl AsRef<Path>, message: &str) -> Result<String, String>;
pub fn push_origin_main(repo: impl AsRef<Path>, set_upstream: bool) -> Result<(), String>;
```

Keep all Git calls as `Command::new("git").arg("-C").arg(repo).args(args)`. Do not use shell strings.

- [ ] **Step 4: Verify green**

Run: `cargo test -p skillbox-git --offline`

Expected: all `skillbox-git` tests pass.

## Task 2: Core User-Skills Sync Workflow

**Files:**
- Modify: `crates/skillbox-core/Cargo.toml`
- Modify: `crates/skillbox-core/src/lib.rs`

- [ ] **Step 1: Write failing core tests**

Add tests to `crates/skillbox-core/src/lib.rs`:

```rust
#[test]
fn user_skills_git_status_is_not_configured_without_origin() {
    let managed_root = temp_path("skillbox-user-status");
    let status = user_skills_git_status(&managed_root).unwrap();

    assert_eq!(status.state, UserSkillsGitState::NotConfigured);
    assert!(!status.initialized);
    assert!(status.remote_url.is_none());
}

#[test]
fn sync_user_skills_initializes_shared_repo_and_commits_all_skills() {
    let managed_root = temp_path("skillbox-user-sync");
    let paths = ensure_managed_layout(&managed_root).unwrap();
    skill_fixture(&paths.user_skills_root.join("alpha"), "alpha", "Alpha skill");
    skill_fixture(&paths.user_skills_root.join("beta"), "beta", "Beta skill");
    let remote = bare_remote("skillbox-user-sync-remote");

    let result = sync_user_skills_git(
        UserSkillsSyncRequest {
            remote_url: Some(remote.to_string_lossy().to_string()),
            commit_message: Some("Sync user skills".to_string()),
            push: true,
        },
        &managed_root,
    )
    .unwrap();

    assert!(result.initialized);
    assert!(result.remote_updated);
    assert!(result.committed);
    assert!(result.pushed);
    assert_eq!(result.state, UserSkillsGitState::Clean);
}

#[test]
fn sync_user_skills_reports_push_failed_without_losing_commit() {
    let managed_root = temp_path("skillbox-user-push-fail");
    let paths = ensure_managed_layout(&managed_root).unwrap();
    skill_fixture(&paths.user_skills_root.join("alpha"), "alpha", "Alpha skill");

    let result = sync_user_skills_git(
        UserSkillsSyncRequest {
            remote_url: Some("/no/such/remote.git".to_string()),
            commit_message: Some("Sync user skills".to_string()),
            push: true,
        },
        &managed_root,
    )
    .unwrap();

    assert!(result.committed);
    assert!(!result.pushed);
    assert_eq!(result.state, UserSkillsGitState::PushFailed);
    assert!(result.message.contains("push"));
}
```

- [ ] **Step 2: Verify red**

Run: `cargo test -p skillbox-core --offline user_skills`

Expected: fails because user-skills sync types and functions do not exist.

- [ ] **Step 3: Implement core types and workflow**

Add serializable API:

```rust
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UserSkillsGitState {
    NotConfigured,
    Clean,
    Dirty,
    PushFailed,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UserSkillsGitStatus {
    pub repo_path: PathBuf,
    pub initialized: bool,
    pub branch: String,
    pub remote_url: Option<String>,
    pub dirty: bool,
    pub raw_status: String,
    pub state: UserSkillsGitState,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserSkillsSyncRequest {
    pub remote_url: Option<String>,
    pub commit_message: Option<String>,
    pub push: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UserSkillsSyncResult {
    pub repo_path: PathBuf,
    pub initialized: bool,
    pub remote_updated: bool,
    pub branch: String,
    pub dirty: bool,
    pub raw_status: String,
    pub committed: bool,
    pub commit_sha: Option<String>,
    pub pushed: bool,
    pub push_attempted: bool,
    pub state: UserSkillsGitState,
    pub message: String,
}
```

Implement:

```rust
pub fn user_skills_git_status(managed_root: impl AsRef<Path>) -> Result<UserSkillsGitStatus>;
pub fn sync_user_skills_git(
    request: UserSkillsSyncRequest,
    managed_root: impl AsRef<Path>,
) -> Result<UserSkillsSyncResult>;
```

Use `ensure_managed_layout`, operate only on `paths.user_skills_root`, default empty or whitespace commit message to `Sync user skills`, and reject empty remote URL when a remote is required.

- [ ] **Step 4: Verify green**

Run: `cargo test -p skillbox-core --offline user_skills`

Expected: user-skills tests pass.

## Task 3: CLI And Tauri Bridge

**Files:**
- Modify: `crates/skillbox-cli/src/main.rs`
- Modify: `apps/desktop/src-tauri/src/lib.rs`

- [ ] **Step 1: Write failing CLI test by command execution**

Run before implementation:

```sh
cargo run -p skillbox-cli --offline -- user-skills-status --managed-root /tmp/skillbox-cli-sync-test
```

Expected: exits non-zero with `Unknown command: user-skills-status`.

- [ ] **Step 2: Implement CLI commands**

Add command handling:

```rust
"user-skills-status" => print_json(&skillbox_core::user_skills_git_status(managed_root(command_args))?),
"sync-user-skills" => {
    let request = skillbox_core::UserSkillsSyncRequest {
        remote_url: option(command_args, "--remote"),
        commit_message: option(command_args, "--message"),
        push: !has_flag(command_args, "--no-push"),
    };
    print_json(&skillbox_core::sync_user_skills_git(request, managed_root(command_args))?)
}
```

Add a `has_flag(args, name)` helper.

- [ ] **Step 3: Implement Tauri commands**

Add:

```rust
#[tauri::command]
fn user_skills_git_status() -> Result<Value, String> {
    let status = skillbox_core::user_skills_git_status(skillbox_core::default_managed_root())?;
    serde_json::to_value(status).map_err(|error| error.to_string())
}

#[tauri::command]
fn sync_user_skills_git(request: skillbox_core::UserSkillsSyncRequest) -> Result<Value, String> {
    let result = skillbox_core::sync_user_skills_git(
        request,
        skillbox_core::default_managed_root(),
    )?;
    serde_json::to_value(result).map_err(|error| error.to_string())
}
```

Register both in `tauri::generate_handler!`.

- [ ] **Step 4: Verify CLI green**

Run:

```sh
cargo run -p skillbox-cli --offline -- user-skills-status --managed-root /tmp/skillbox-cli-sync-test
```

Expected: JSON with `"state": "not_configured"`.

## Task 4: React Sync UI Helpers And Tests

**Files:**
- Modify: `apps/desktop/src/App.jsx`
- Modify: `apps/desktop/src/App.import-candidates.test.js`

- [ ] **Step 1: Write failing UI helper tests**

Export pure helpers from `App.jsx`:

```js
export function userSyncAction(syncStatus, skillType) {
  if (skillType !== 'user') return null;
  if (!syncStatus || syncStatus.state === 'not_configured') return 'Set up sync';
  if (syncStatus.state === 'push_failed') return 'Retry push';
  return 'Sync now';
}

export function normalizeUserSkillsGitStatus(status) {
  return {
    repoPath: status?.repoPath || status?.repo_path || '',
    remoteUrl: status?.remoteUrl || status?.remote_url || '',
    state: status?.state || 'not_configured',
    dirty: Boolean(status?.dirty),
    message: status?.message || status?.lastError || status?.last_error || ''
  };
}
```

Add tests:

```js
test('user sync action is setup before remote and hidden for remote skills', () => {
  assert.equal(userSyncAction({ state: 'not_configured' }, 'user'), 'Set up sync');
  assert.equal(userSyncAction({ state: 'clean' }, 'remote'), null);
});

test('user sync action retries failed push and syncs configured remotes', () => {
  assert.equal(userSyncAction({ state: 'push_failed' }, 'user'), 'Retry push');
  assert.equal(userSyncAction({ state: 'dirty' }, 'user'), 'Sync now');
});
```

- [ ] **Step 2: Verify red**

Run: `npm test -- apps/desktop/src/App.import-candidates.test.js`

Expected: fails because helpers are not exported or implemented.

- [ ] **Step 3: Implement helpers**

Add exports and normalize backend snake_case fields to camelCase for UI use.

- [ ] **Step 4: Verify green**

Run: `npm test -- apps/desktop/src/App.import-candidates.test.js`

Expected: helper tests pass.

## Task 5: React Sync Workflow UI

**Files:**
- Modify: `apps/desktop/src/App.jsx`
- Modify: `apps/desktop/src/styles.css`

- [ ] **Step 1: Add state and status loading**

Add state:

```js
const [userSkillsGit, setUserSkillsGit] = useState(normalizeUserSkillsGitStatus(null));
const [syncDialog, setSyncDialog] = useState({ open: false, remoteUrl: '', commitMessage: defaultSyncCommitMessage, push: true, error: '' });
const [syncOptionsOpen, setSyncOptionsOpen] = useState(false);
const [syncCommitMessage, setSyncCommitMessage] = useState(defaultSyncCommitMessage);
```

In `refresh`, call `invoke('user_skills_git_status')` with `managed_state` and preferences. In browser preview, use `not_configured`.

- [ ] **Step 2: Add setup and sync handlers**

Implement:

```js
function openSyncDialog() {
  setSyncDialog({
    open: true,
    remoteUrl: userSkillsGit.remoteUrl || '',
    commitMessage: syncCommitMessage || defaultSyncCommitMessage,
    push: true,
    error: ''
  });
}

function closeSyncDialog() {
  if (status === 'syncing') return;
  setSyncDialog((current) => ({ ...current, open: false, error: '' }));
}

async function submitSyncSetup(event) {
  event.preventDefault();
  if (!syncDialog.remoteUrl.trim()) {
    setSyncDialog((current) => ({ ...current, error: 'Enter a Git remote URL.' }));
    return;
  }
  await runUserSkillsSync({
    remoteUrl: syncDialog.remoteUrl,
    commitMessage: syncDialog.commitMessage,
    push: syncDialog.push,
    closeDialog: true
  });
}

async function runUserSkillsSync({ remoteUrl = '', commitMessage = syncCommitMessage, push = true, closeDialog = false } = {}) {
  setStatus('syncing');
  setError('');
  setNotice('');
  try {
    const result = await invoke('sync_user_skills_git', {
      request: {
        remote_url: remoteUrl.trim() || null,
        commit_message: commitMessage.trim() || defaultSyncCommitMessage,
        push
      }
    });
    const normalized = normalizeUserSkillsGitStatus(result);
    setUserSkillsGit(normalized);
    setSyncCommitMessage(commitMessage.trim() || defaultSyncCommitMessage);
    if (closeDialog) setSyncDialog((current) => ({ ...current, open: false, error: '' }));
    setNotice(result.message || syncNotice(normalized));
    setStatus('ready');
  } catch (syncError) {
    const message = syncError.message || String(syncError) || 'Unable to sync user skills.';
    if (closeDialog) {
      setSyncDialog((current) => ({ ...current, error: message }));
    } else {
      setError(message);
    }
    setStatus('ready');
  }
}
```

Use Tauri command `sync_user_skills_git` with:

```js
{
  request: {
    remote_url: remoteUrl || null,
    commit_message: commitMessage || defaultSyncCommitMessage,
    push
  }
}
```

- [ ] **Step 3: Render user-only actions**

In `SkillDetail`, replace the current generic action buttons with:

- user skill: sync button from `userSyncAction(userSkillsGit, skill.type)`, `Deploy`
- remote skill: `Check update`, `Rollback`, `Deploy`

Render sync panel metadata and collapsed sync options for user skills.

- [ ] **Step 4: Add setup modal**

Add `UserSkillsSyncDialog` modeled after `RemoteImportDialog`, with remote URL, commit message, push checkbox, error, cancel, and confirm.

- [ ] **Step 5: Add CSS**

Add compact styles for `.syncOptions`, `.syncMeta`, `.syncDialog`, and checkbox rows, reusing existing modal/button visual language.

- [ ] **Step 6: Verify UI tests**

Run: `npm test`

Expected: all Node/UI tests pass.

## Task 6: Documentation And Full Verification

**Files:**
- Modify: `docs/workflows.md`
- Modify: `docs/architecture.md`
- Modify: `docs/implementation-status.md`
- Modify: `CONTRIBUTING.md`

- [ ] **Step 1: Update docs**

Mark Rust core/Tauri support for user-skills Git sync as implemented. Keep legacy Node wording only where it remains true for other workflows.

- [ ] **Step 2: Run Rust formatting**

Run: `cargo fmt`

Expected: exits 0.

- [ ] **Step 3: Run full Rust tests**

Run: `cargo test --offline`

Expected: all Rust tests pass.

- [ ] **Step 4: Run full Node tests**

Run: `npm test`

Expected: all Node tests pass.

- [ ] **Step 5: Manual CLI smoke**

Run:

```sh
TEMP_ROOT=$(mktemp -d)
cargo run -p skillbox-cli --offline -- user-skills-status --managed-root "$TEMP_ROOT"
```

Expected: JSON state `not_configured`.
