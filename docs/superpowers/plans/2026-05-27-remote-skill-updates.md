# Remote Skill Updates Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the GitHub-only remote skill source binding, update, rollback, diff review, and operation-log workflow approved in the design spec.

**Architecture:** Keep all filesystem, Git, GitHub, SQLite, and version-switching behavior in Rust core crates. Reuse `skillbox-github` for URL/ref parsing, extend `skillbox-git` for structured snapshot/diff primitives, expose typed Tauri and CLI commands, and keep React as a presentation layer that normalizes backend objects and renders review dialogs.

**Tech Stack:** Rust crates (`skillbox-git`, `skillbox-github`, `skillbox-core`, `skillbox-cli`, Tauri bridge), SQLite through `rusqlite`, Git CLI through structured `Command`, React/Vite desktop UI, Node test runner for UI helpers, Cargo tests for Rust behavior.

---

## Scope Check

The approved spec includes remote source binding, version changes, diff review, operation history, CLI/Tauri exposure, and UI. These are tightly coupled because update and rollback must be logged, previewed, and applied through the same version model. This plan keeps them in one sequence but requires frequent commits so each subsystem is reviewable.

## File Structure

- Modify `crates/skillbox-core/Cargo.toml`: add direct dependencies needed by core orchestration, including `skillbox-github`.
- Modify `crates/skillbox-core/src/lib.rs`: operation log, source metadata, binding preview/bind, version list, version-change preview/apply, deployment classification, tests.
- Modify `crates/skillbox-git/src/lib.rs`: structured Git fetch, checkout, no-index diff, file listing, and helper tests.
- Modify `crates/skillbox-github/src/lib.rs`: ref-kind classification helpers and GitHub source URL normalization additions.
- Modify `crates/skillbox-cli/src/main.rs`: CLI commands for binding, versions, preview/apply, and operation listing.
- Modify `apps/desktop/src-tauri/src/lib.rs`: Tauri commands mirroring the Rust core API.
- Create `apps/desktop/src/GitDiffView.jsx`: shared unified diff renderer extracted from `App.jsx`.
- Create `apps/desktop/src/remoteSkills.js`: remote source/version normalization, display labels, action gating, and helper tests.
- Modify `apps/desktop/src/skillStatusRefresh.js`: support pinned remote status and richer source/version metadata.
- Modify `apps/desktop/src/App.jsx`: source binding dialog, version review dialog, version list, operation history panel, command wiring.
- Modify `apps/desktop/src/styles.css`: dialog, version list, operation log, and diff review styles.
- Modify `apps/desktop/src/App.import-candidates.test.js`: add UI helper coverage or import helper tests from `remoteSkills.js`.
- Modify `docs/data-model.md`, `docs/workflows.md`, `docs/architecture.md`, `docs/implementation-status.md`: keep documented workflow and current status aligned with implementation.

## Task 1: Generic Operation Log Foundation

**Files:**
- Modify: `crates/skillbox-core/src/lib.rs`
- Modify: `docs/data-model.md`

- [ ] **Step 1: Write failing Rust tests for operation records**

Add tests inside `#[cfg(test)] mod tests` in `crates/skillbox-core/src/lib.rs`:

```rust
#[test]
fn operation_log_records_success_failure_and_cancellation() {
    let managed_root = temp_dir("operation-log-statuses").join("SkillBox");
    ensure_managed_layout(&managed_root).unwrap();

    let started = start_operation(
        OperationStart {
            operation_type: "bind_remote_source".to_string(),
            actor: "cli".to_string(),
            entity_type: "skill".to_string(),
            entity_name: "find-skills".to_string(),
            summary: "Bind find-skills to GitHub source".to_string(),
            payload: serde_json::json!({"sourceUrl":"https://github.com/acme/skills/tree/main/find-skills"}),
        },
        &managed_root,
    )
    .unwrap();
    assert_eq!(started.status, OperationStatus::Started);

    let succeeded = finish_operation(
        OperationFinish {
            id: started.id.clone(),
            status: OperationStatus::Succeeded,
            summary: "Bound find-skills to GitHub source".to_string(),
            error: None,
            payload: serde_json::json!({"validation":"same_skill_changed"}),
        },
        &managed_root,
    )
    .unwrap();
    assert_eq!(succeeded.status, OperationStatus::Succeeded);

    let failed = start_operation(
        OperationStart {
            operation_type: "update_remote_skill".to_string(),
            actor: "desktop".to_string(),
            entity_type: "skill".to_string(),
            entity_name: "find-skills".to_string(),
            summary: "Update find-skills".to_string(),
            payload: serde_json::json!({"fromVersion":"manual-abc","toVersion":"123"}),
        },
        &managed_root,
    )
    .unwrap();
    let failed = finish_operation(
        OperationFinish {
            id: failed.id,
            status: OperationStatus::Failed,
            summary: "Update find-skills failed".to_string(),
            error: Some("Missing SKILL.md".to_string()),
            payload: serde_json::json!({"restoredCurrent":true}),
        },
        &managed_root,
    )
    .unwrap();
    assert_eq!(failed.status, OperationStatus::Failed);
    assert_eq!(failed.error.as_deref(), Some("Missing SKILL.md"));

    let cancelled = start_operation(
        OperationStart {
            operation_type: "preview_version_change".to_string(),
            actor: "desktop".to_string(),
            entity_type: "skill".to_string(),
            entity_name: "find-skills".to_string(),
            summary: "Preview rollback for find-skills".to_string(),
            payload: serde_json::json!({"action":"rollback"}),
        },
        &managed_root,
    )
    .unwrap();
    let cancelled = finish_operation(
        OperationFinish {
            id: cancelled.id,
            status: OperationStatus::Cancelled,
            summary: "Rollback preview cancelled".to_string(),
            error: None,
            payload: serde_json::json!({"cancelledBy":"user"}),
        },
        &managed_root,
    )
    .unwrap();
    assert_eq!(cancelled.status, OperationStatus::Cancelled);

    let list = list_operations(OperationFilter::default(), &managed_root).unwrap();
    assert_eq!(list.operations.len(), 3);
    assert_eq!(list.operations[0].status, OperationStatus::Cancelled);
    assert_eq!(list.operations[1].status, OperationStatus::Failed);
    assert_eq!(list.operations[2].status, OperationStatus::Succeeded);
}

#[test]
fn operation_log_filters_by_entity_and_status() {
    let managed_root = temp_dir("operation-log-filters").join("SkillBox");
    ensure_managed_layout(&managed_root).unwrap();

    let alpha = start_operation(
        OperationStart {
            operation_type: "deploy_skill".to_string(),
            actor: "cli".to_string(),
            entity_type: "skill".to_string(),
            entity_name: "alpha".to_string(),
            summary: "Deploy alpha".to_string(),
            payload: serde_json::json!({}),
        },
        &managed_root,
    )
    .unwrap();
    finish_operation(
        OperationFinish {
            id: alpha.id,
            status: OperationStatus::Succeeded,
            summary: "Deployed alpha".to_string(),
            error: None,
            payload: serde_json::json!({}),
        },
        &managed_root,
    )
    .unwrap();

    let beta = start_operation(
        OperationStart {
            operation_type: "deploy_skill".to_string(),
            actor: "cli".to_string(),
            entity_type: "skill".to_string(),
            entity_name: "beta".to_string(),
            summary: "Deploy beta".to_string(),
            payload: serde_json::json!({}),
        },
        &managed_root,
    )
    .unwrap();
    finish_operation(
        OperationFinish {
            id: beta.id,
            status: OperationStatus::Failed,
            summary: "Deploy beta failed".to_string(),
            error: Some("target exists".to_string()),
            payload: serde_json::json!({}),
        },
        &managed_root,
    )
    .unwrap();

    let filtered = list_operations(
        OperationFilter {
            entity_type: Some("skill".to_string()),
            entity_name: Some("beta".to_string()),
            status: Some(OperationStatus::Failed),
            limit: Some(20),
        },
        &managed_root,
    )
    .unwrap();

    assert_eq!(filtered.operations.len(), 1);
    assert_eq!(filtered.operations[0].entity_name, "beta");
    assert_eq!(filtered.operations[0].status, OperationStatus::Failed);
}
```

- [ ] **Step 2: Run the focused test to verify red**

Run: `cargo test -p skillbox-core --offline operation_log`

Expected: FAIL because `OperationStatus`, `OperationStart`, `OperationFinish`, `OperationFilter`, `start_operation`, `finish_operation`, and `list_operations` do not exist.

- [ ] **Step 3: Add operation types and SQLite migration**

Add these public types near the other serializable API structs in `crates/skillbox-core/src/lib.rs`:

```rust
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationStatus {
    Started,
    Succeeded,
    Failed,
    Cancelled,
}

impl OperationStatus {
    fn as_str(self) -> &'static str {
        match self {
            OperationStatus::Started => "started",
            OperationStatus::Succeeded => "succeeded",
            OperationStatus::Failed => "failed",
            OperationStatus::Cancelled => "cancelled",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OperationStart {
    pub operation_type: String,
    pub actor: String,
    pub entity_type: String,
    pub entity_name: String,
    pub summary: String,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OperationFinish {
    pub id: String,
    pub status: OperationStatus,
    pub summary: String,
    pub error: Option<String>,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct OperationFilter {
    pub entity_type: Option<String>,
    pub entity_name: Option<String>,
    pub status: Option<OperationStatus>,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OperationRecord {
    pub id: String,
    #[serde(rename = "type")]
    pub operation_type: String,
    pub status: OperationStatus,
    pub actor: String,
    pub entity_type: String,
    pub entity_name: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub summary: String,
    pub error: Option<String>,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OperationList {
    pub operations: Vec<OperationRecord>,
}
```

Extend `init_database` by adding the `operations` table to the `execute_batch` block:

```sql
CREATE TABLE IF NOT EXISTS operations (
  id TEXT PRIMARY KEY,
  type TEXT NOT NULL,
  status TEXT NOT NULL,
  actor TEXT NOT NULL,
  entity_type TEXT NOT NULL,
  entity_name TEXT NOT NULL,
  started_at TEXT NOT NULL,
  finished_at TEXT,
  summary TEXT NOT NULL,
  error TEXT,
  payload_json TEXT NOT NULL
);
```

- [ ] **Step 4: Implement operation functions**

Add these functions below `init_database` helpers:

```rust
pub fn start_operation(
    request: OperationStart,
    managed_root: impl AsRef<Path>,
) -> Result<OperationRecord> {
    let paths = ensure_managed_layout(managed_root.as_ref().to_path_buf())?;
    let id = operation_id();
    let now = iso_timestamp_now();
    let payload_json = serde_json::to_string(&request.payload).map_err(|error| error.to_string())?;
    let connection = Connection::open(&paths.database_path).map_err(|error| error.to_string())?;
    connection
        .execute(
            "
            INSERT INTO operations (
              id, type, status, actor, entity_type, entity_name,
              started_at, finished_at, summary, error, payload_json
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, ?8, NULL, ?9)
            ",
            params![
                id,
                request.operation_type,
                OperationStatus::Started.as_str(),
                request.actor,
                request.entity_type,
                request.entity_name,
                now,
                request.summary,
                payload_json
            ],
        )
        .map_err(|error| error.to_string())?;

    load_operation(&connection, &id)
}

pub fn finish_operation(
    request: OperationFinish,
    managed_root: impl AsRef<Path>,
) -> Result<OperationRecord> {
    let paths = ensure_managed_layout(managed_root.as_ref().to_path_buf())?;
    let payload_json = serde_json::to_string(&request.payload).map_err(|error| error.to_string())?;
    let now = iso_timestamp_now();
    let connection = Connection::open(&paths.database_path).map_err(|error| error.to_string())?;
    connection
        .execute(
            "
            UPDATE operations
            SET status = ?2,
                finished_at = ?3,
                summary = ?4,
                error = ?5,
                payload_json = ?6
            WHERE id = ?1
            ",
            params![
                request.id,
                request.status.as_str(),
                now,
                request.summary,
                request.error,
                payload_json
            ],
        )
        .map_err(|error| error.to_string())?;

    load_operation(&connection, &request.id)
}

pub fn list_operations(
    filter: OperationFilter,
    managed_root: impl AsRef<Path>,
) -> Result<OperationList> {
    let paths = ensure_managed_layout(managed_root.as_ref().to_path_buf())?;
    let connection = Connection::open(&paths.database_path).map_err(|error| error.to_string())?;
    let limit = filter.limit.unwrap_or(50).clamp(1, 500);
    let mut statement = connection
        .prepare(
            "
            SELECT id, type, status, actor, entity_type, entity_name,
                   started_at, finished_at, summary, error, payload_json
            FROM operations
            WHERE (?1 IS NULL OR entity_type = ?1)
              AND (?2 IS NULL OR entity_name = ?2)
              AND (?3 IS NULL OR status = ?3)
            ORDER BY started_at DESC, id DESC
            LIMIT ?4
            ",
        )
        .map_err(|error| error.to_string())?;
    let status = filter.status.map(OperationStatus::as_str);
    let rows = statement
        .query_map(
            params![filter.entity_type, filter.entity_name, status, limit],
            operation_from_row,
        )
        .map_err(|error| error.to_string())?;
    let mut operations = Vec::new();
    for row in rows {
        operations.push(row.map_err(|error| error.to_string())?);
    }
    Ok(OperationList { operations })
}
```

Add private helpers:

```rust
fn operation_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    format!("op-{nanos}")
}

fn iso_timestamp_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default();
    format!("{seconds}")
}

fn load_operation(connection: &Connection, id: &str) -> Result<OperationRecord> {
    connection
        .query_row(
            "
            SELECT id, type, status, actor, entity_type, entity_name,
                   started_at, finished_at, summary, error, payload_json
            FROM operations
            WHERE id = ?1
            ",
            params![id],
            operation_from_row,
        )
        .map_err(|error| error.to_string())
}

fn operation_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<OperationRecord> {
    let status: String = row.get(2)?;
    let payload_json: String = row.get(10)?;
    Ok(OperationRecord {
        id: row.get(0)?,
        operation_type: row.get(1)?,
        status: parse_operation_status(&status).unwrap_or(OperationStatus::Failed),
        actor: row.get(3)?,
        entity_type: row.get(4)?,
        entity_name: row.get(5)?,
        started_at: row.get(6)?,
        finished_at: row.get(7)?,
        summary: row.get(8)?,
        error: row.get(9)?,
        payload: serde_json::from_str(&payload_json).unwrap_or_else(|_| serde_json::json!({})),
    })
}

fn parse_operation_status(value: &str) -> Option<OperationStatus> {
    match value {
        "started" => Some(OperationStatus::Started),
        "succeeded" => Some(OperationStatus::Succeeded),
        "failed" => Some(OperationStatus::Failed),
        "cancelled" => Some(OperationStatus::Cancelled),
        _ => None,
    }
}
```

- [ ] **Step 5: Verify operation tests pass**

Run: `cargo test -p skillbox-core --offline operation_log`

Expected: PASS.

- [ ] **Step 6: Document the operation table**

In `docs/data-model.md`, add the `operations` table under SQLite with the same columns from Step 3 and state that operation records are append-only from the UI perspective and retained permanently.

- [ ] **Step 7: Commit**

```bash
git add crates/skillbox-core/src/lib.rs docs/data-model.md
git commit -m "feat(core): add operation log"
```

## Task 2: GitHub Source Metadata And Ref Semantics

**Files:**
- Modify: `crates/skillbox-core/Cargo.toml`
- Modify: `crates/skillbox-core/src/lib.rs`
- Modify: `crates/skillbox-github/src/lib.rs`
- Modify: `docs/data-model.md`

- [ ] **Step 1: Write failing tests for ref kinds and pinned updates**

Add tests in `crates/skillbox-github/src/lib.rs`:

```rust
#[test]
fn classifies_commit_ref_without_network() {
    assert_eq!(
        classify_ref_text("0123456789abcdef0123456789abcdef01234567"),
        GitHubRefKind::Commit
    );
}

#[test]
fn non_commit_ref_stays_unknown_until_resolved() {
    assert_eq!(classify_ref_text("main"), GitHubRefKind::Unknown);
    assert_eq!(classify_ref_text("v1.0.0"), GitHubRefKind::Unknown);
}
```

Add tests in `crates/skillbox-core/src/lib.rs`:

```rust
#[test]
fn check_remote_skill_updates_marks_pinned_sources() {
    let root = temp_dir("remote-pinned-sources");
    let managed_root = root.join("SkillBox");
    let paths = ensure_managed_layout(&managed_root).unwrap();

    fs::create_dir_all(paths.remote_skills_root.join("tagged")).unwrap();
    fs::write(
        paths.remote_skills_root.join("tagged").join("source.json"),
        r#"{
          "type":"github",
          "repoUrl":"https://github.com/acme/skills.git",
          "ref":"v1.0.0",
          "refKind":"tag",
          "tracking":false,
          "currentVersion":"0123456789abcdef0123456789abcdef01234567",
          "installedSha":"0123456789abcdef0123456789abcdef01234567"
        }"#,
    )
    .unwrap();

    let result = check_remote_skill_updates(&managed_root).unwrap();
    let tagged = remote_status(&result.statuses, "tagged");
    assert_eq!(tagged.state, RemoteSkillUpdateState::Pinned);
    assert!(!tagged.update_available);
    assert_eq!(tagged.message.as_deref(), Some("Pinned GitHub source."));
}

#[test]
fn check_remote_skill_updates_compares_latest_sha_to_current_version_for_manual_binding() {
    let root = temp_dir("remote-manual-bound-update");
    let managed_root = root.join("SkillBox");
    let paths = ensure_managed_layout(&managed_root).unwrap();
    let remote = bare_remote_with_main("remote-manual-bound-update-origin");
    let latest_sha = remote_head(&remote);

    write_remote_source_with_json(
        &paths.remote_skills_root.join("bound"),
        &format!(
            r#"{{
              "type":"github",
              "repoUrl":"{}",
              "ref":"main",
              "refKind":"branch",
              "tracking":true,
              "currentVersion":"manual-abc123def456",
              "installedSha":null,
              "latestSha":"{}"
            }}"#,
            remote.to_string_lossy(),
            latest_sha
        ),
    );

    let result = check_remote_skill_updates(&managed_root).unwrap();
    let bound = remote_status(&result.statuses, "bound");
    assert_eq!(bound.state, RemoteSkillUpdateState::UpdateAvailable);
    assert_eq!(bound.latest_sha.as_deref(), Some(latest_sha.as_str()));
    assert_eq!(bound.current_version.as_deref(), Some("manual-abc123def456"));
    assert_eq!(bound.installed_sha, None);
}
```

- [ ] **Step 2: Run tests to verify red**

Run:

```bash
cargo test -p skillbox-github --offline ref
cargo test -p skillbox-core --offline remote
```

Expected: FAIL because ref-kind types, pinned state, `current_version`, and the test helper `write_remote_source_with_json` do not exist.

- [ ] **Step 3: Implement GitHub ref-kind helpers**

In `crates/skillbox-github/src/lib.rs`, add:

```rust
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum GitHubRefKind {
    Branch,
    Tag,
    Commit,
    Unknown,
}

pub fn classify_ref_text(reference: &str) -> GitHubRefKind {
    let value = reference.trim();
    if value.len() == 40 && value.chars().all(|ch| ch.is_ascii_hexdigit()) {
        GitHubRefKind::Commit
    } else {
        GitHubRefKind::Unknown
    }
}
```

- [ ] **Step 4: Extend core remote source and update status**

In `crates/skillbox-core/Cargo.toml`, add:

```toml
skillbox-github = { path = "../skillbox-github" }
```

In `crates/skillbox-core/src/lib.rs`, extend `RemoteSkillUpdateState`:

```rust
pub enum RemoteSkillUpdateState {
    NotCheckable,
    UpToDate,
    UpdateAvailable,
    CheckFailed,
    Pinned,
}
```

Extend `RemoteSkillUpdateStatus`:

```rust
pub struct RemoteSkillUpdateStatus {
    pub skill_name: String,
    pub source_type: Option<String>,
    pub current_version: Option<String>,
    pub installed_sha: Option<String>,
    pub latest_sha: Option<String>,
    pub ref_kind: Option<String>,
    pub tracking: bool,
    pub update_available: bool,
    pub state: RemoteSkillUpdateState,
    pub message: Option<String>,
}
```

Extend `RemoteSkillSource`:

```rust
struct RemoteSkillSource {
    #[serde(rename = "type")]
    source_type: String,
    #[serde(rename = "repoUrl", alias = "repo_url")]
    repo_url: Option<String>,
    #[serde(rename = "ref", alias = "reference")]
    reference: Option<String>,
    #[serde(rename = "refKind", alias = "ref_kind")]
    ref_kind: Option<String>,
    tracking: Option<bool>,
    #[serde(rename = "currentVersion", alias = "current_version")]
    current_version: Option<String>,
    #[serde(rename = "installedSha", alias = "installed_sha")]
    installed_sha: Option<String>,
    #[serde(rename = "latestSha", alias = "latest_sha")]
    latest_sha: Option<String>,
}
```

Update every `RemoteSkillUpdateStatus` construction in `check_one_remote_skill_update` to fill the new fields. For GitHub sources, return `Pinned` when `tracking == Some(false)` or `refKind` is `tag` or `commit`:

```rust
let ref_kind = source.ref_kind.clone();
let tracking = source.tracking.unwrap_or_else(|| {
    !matches!(ref_kind.as_deref(), Some("tag") | Some("commit"))
});
if !tracking {
    return RemoteSkillUpdateStatus {
        skill_name: skill_name.to_string(),
        source_type: Some(source.source_type),
        current_version: source.current_version,
        installed_sha: source.installed_sha,
        latest_sha: source.latest_sha,
        ref_kind,
        tracking: false,
        update_available: false,
        state: RemoteSkillUpdateState::Pinned,
        message: Some("Pinned GitHub source.".to_string()),
    };
}
```

For tracking branches, compute update availability against `currentVersion` first, then `installedSha`:

```rust
let active = source
    .current_version
    .as_deref()
    .or(source.installed_sha.as_deref());
let update_available = active != Some(latest_sha.as_str());
```

- [ ] **Step 5: Add test helper for raw source JSON**

Inside the Rust test module, add:

```rust
fn write_remote_source_with_json(remote_root: &std::path::Path, json: &str) {
    fs::create_dir_all(remote_root).unwrap();
    fs::write(remote_root.join("source.json"), json).unwrap();
}
```

- [ ] **Step 6: Verify tests pass**

Run:

```bash
cargo test -p skillbox-github --offline ref
cargo test -p skillbox-core --offline remote
```

Expected: PASS.

- [ ] **Step 7: Update data-model docs**

In `docs/data-model.md`, update the GitHub `source.json` example to include `refKind`, `tracking`, `currentVersion`, and nullable `installedSha`. Add a short rule that branch sources are tracking and tag/commit sources are pinned.

- [ ] **Step 8: Commit**

```bash
git add crates/skillbox-core/Cargo.toml crates/skillbox-core/src/lib.rs crates/skillbox-github/src/lib.rs docs/data-model.md Cargo.lock
git commit -m "feat(github): model remote source refs"
```

## Task 3: Git Snapshot And Diff Primitives

**Files:**
- Modify: `crates/skillbox-git/src/lib.rs`

- [ ] **Step 1: Write failing tests for snapshot fetch and no-index diff**

Add tests in `crates/skillbox-git/src/lib.rs`:

```rust
#[test]
fn fetch_ref_path_checks_out_only_requested_path() {
    let remote = bare_remote_with_skill("git-snapshot-origin");
    let temp = temp_dir("git-snapshot-work");
    let checkout = temp.join("checkout");

    let sha = fetch_ref_path(
        remote.to_str().unwrap(),
        "main",
        "skills/demo",
        &checkout,
    )
    .unwrap();

    assert!(!sha.is_empty());
    assert!(checkout.join("skills/demo/SKILL.md").exists());
    assert!(!checkout.join("README.md").exists());
}

#[test]
fn diff_no_index_tree_reports_changed_files() {
    let temp = temp_dir("git-diff-no-index");
    let old_root = temp.join("old");
    let new_root = temp.join("new");
    fs::create_dir_all(&old_root).unwrap();
    fs::create_dir_all(&new_root).unwrap();
    fs::write(old_root.join("SKILL.md"), "name: demo\n").unwrap();
    fs::write(new_root.join("SKILL.md"), "name: demo\nversion: 2\n").unwrap();
    fs::write(new_root.join("extra.txt"), "extra\n").unwrap();

    let files = diff_no_index_tree(&old_root, &new_root).unwrap();

    assert!(files.iter().any(|file| file.path == "SKILL.md" && file.status == "M"));
    assert!(files.iter().any(|file| file.path == "extra.txt" && file.status == "A"));
    assert!(files.iter().any(|file| file.diff.contains("+version: 2")));
}
```

- [ ] **Step 2: Run test to verify red**

Run: `cargo test -p skillbox-git --offline snapshot`

Expected: FAIL because `fetch_ref_path`, `diff_no_index_tree`, and `bare_remote_with_skill` do not exist.

- [ ] **Step 3: Add public diff and snapshot structs**

Add to `crates/skillbox-git/src/lib.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitDiffFile {
    pub path: String,
    pub old_path: Option<String>,
    pub status: String,
    pub diff: String,
}
```

- [ ] **Step 4: Implement structured snapshot fetch**

Add:

```rust
pub fn fetch_ref_path(
    repo_url: &str,
    reference: &str,
    path: &str,
    checkout_root: impl AsRef<Path>,
) -> Result<String, String> {
    let checkout_root = checkout_root.as_ref();
    fs::create_dir_all(checkout_root).map_err(|error| error.to_string())?;
    git(checkout_root, &["init", "-b", "main"])?;
    git(checkout_root, &["remote", "add", "origin", repo_url])?;
    git(checkout_root, &["fetch", "--depth", "1", "origin", reference])?;
    let sha = git(checkout_root, &["rev-parse", "FETCH_HEAD"])?.trim().to_string();
    git_owned(
        checkout_root,
        &[
            "checkout".to_string(),
            "FETCH_HEAD".to_string(),
            "--".to_string(),
            path.to_string(),
        ],
    )?;
    Ok(sha)
}
```

This command uses a temporary working tree owned by SkillBox and checks out only the requested skill path.

- [ ] **Step 5: Implement no-index diff parsing**

Add:

```rust
pub fn diff_no_index_tree(
    old_root: impl AsRef<Path>,
    new_root: impl AsRef<Path>,
) -> Result<Vec<GitDiffFile>, String> {
    let old_root = old_root.as_ref();
    let new_root = new_root.as_ref();
    let name_status = git_diff_no_index(&[
        "--no-index",
        "--name-status",
        "-M",
        old_root.to_str().unwrap_or(""),
        new_root.to_str().unwrap_or(""),
    ])?;
    let unified = git_diff_no_index(&[
        "--no-index",
        "-M",
        "--",
        old_root.to_str().unwrap_or(""),
        new_root.to_str().unwrap_or(""),
    ])?;
    Ok(parse_no_index_files(&name_status, &unified, old_root, new_root))
}

fn git_diff_no_index(args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .arg("diff")
        .args(args)
        .output()
        .map_err(|error| error.to_string())?;
    if output.status.success() || output.status.code() == Some(1) {
        return Ok(String::from_utf8_lossy(&output.stdout).to_string());
    }
    Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
}
```

Implement `parse_no_index_files` by splitting `--name-status` lines and extracting each file's diff section from the unified diff by `diff --git` boundaries. Normalize paths by stripping the `old_root` and `new_root` prefixes from diff headers so UI paths are relative to the skill root.

- [ ] **Step 6: Add test helper for local bare remote**

Inside the test module, add:

```rust
fn bare_remote_with_skill(label: &str) -> PathBuf {
    let remote = temp_dir(label).join("remote.git");
    Command::new("git").args(["init", "--bare"]).arg(&remote).output().unwrap();
    let work = temp_dir(&format!("{label}-work"));
    Command::new("git").args(["init", "-b", "main"]).arg(&work).output().unwrap();
    fs::create_dir_all(work.join("skills/demo")).unwrap();
    fs::write(work.join("README.md"), "root\n").unwrap();
    fs::write(
        work.join("skills/demo/SKILL.md"),
        "---\nname: demo\ndescription: Demo\n---\n",
    )
    .unwrap();
    run_git(&work, &["add", "."]);
    run_git(&work, &["commit", "-m", "Initial skill"]);
    run_git(&work, &["remote", "add", "origin", remote.to_str().unwrap()]);
    run_git(&work, &["push", "-u", "origin", "main"]);
    remote
}
```

- [ ] **Step 7: Verify tests pass**

Run: `cargo test -p skillbox-git --offline snapshot`

Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add crates/skillbox-git/src/lib.rs
git commit -m "feat(github): add snapshot diff primitives"
```

## Task 4: Manual GitHub Source Binding

**Files:**
- Modify: `crates/skillbox-core/src/lib.rs`
- Modify: `docs/workflows.md`

- [ ] **Step 1: Write failing tests for binding preview and bind execution**

Add tests in `crates/skillbox-core/src/lib.rs`:

```rust
#[test]
fn preview_remote_source_binding_detects_exact_match() {
    let root = temp_dir("source-binding-exact");
    let managed_root = root.join("SkillBox");
    let paths = ensure_managed_layout(&managed_root).unwrap();
    let source = root.join("local").join("demo");
    make_skill(&source, "demo", "Demo skill");
    import_skill(&source, SkillKind::Remote, &managed_root).unwrap();
    let remote = bare_remote_with_skill_content("source-binding-exact-origin", "demo", "Demo skill", "");

    let preview = preview_remote_source_binding(
        RemoteSourceBindingRequest {
            skill_name: "demo".to_string(),
            source_url: format!("{}/tree/main/skills/demo", github_file_url(&remote)),
            actor: "cli".to_string(),
        },
        &managed_root,
    )
    .unwrap();

    assert_eq!(preview.validation, SourceBindingValidation::ExactMatch);
    assert_eq!(preview.skill_name, "demo");
    assert_eq!(preview.ref_kind.as_deref(), Some("branch"));
    assert!(preview.tracking);
}

#[test]
fn bind_changed_source_does_not_switch_current() {
    let root = temp_dir("source-binding-changed");
    let managed_root = root.join("SkillBox");
    let paths = ensure_managed_layout(&managed_root).unwrap();
    let source = root.join("local").join("find-skills");
    make_skill(&source, "find-skills", "Find skills");
    let imported = import_skill(&source, SkillKind::Remote, &managed_root).unwrap();
    let before_current = fs::read_link(paths.remote_skills_root.join("find-skills").join("current")).unwrap();
    let remote = bare_remote_with_skill_content(
        "source-binding-changed-origin",
        "find-skills",
        "Find skills",
        "Updated body\n",
    );
    let url = format!("{}/tree/main/skills/find-skills", github_file_url(&remote));
    let preview = preview_remote_source_binding(
        RemoteSourceBindingRequest {
            skill_name: "find-skills".to_string(),
            source_url: url.clone(),
            actor: "desktop".to_string(),
        },
        &managed_root,
    )
    .unwrap();

    assert_eq!(preview.validation, SourceBindingValidation::SameSkillChanged);
    let result = bind_remote_source(
        BindRemoteSourceRequest {
            skill_name: "find-skills".to_string(),
            source_url: url,
            actor: "desktop".to_string(),
        },
        &managed_root,
    )
    .unwrap();

    let after_current = fs::read_link(paths.remote_skills_root.join("find-skills").join("current")).unwrap();
    assert_eq!(after_current, before_current);
    assert_eq!(result.validation, SourceBindingValidation::SameSkillChanged);
    assert!(result.latest_sha.is_some());
    assert!(!paths.remote_skills_root.join("find-skills").join("versions").join(result.latest_sha.unwrap()).exists());
    assert!(imported.managed_path.exists());
    let operations = list_operations(OperationFilter::default(), &managed_root).unwrap();
    assert!(operations.operations.iter().any(|operation| operation.operation_type == "bind_remote_source"));
}

#[test]
fn preview_remote_source_binding_rejects_name_mismatch() {
    let root = temp_dir("source-binding-mismatch");
    let managed_root = root.join("SkillBox");
    let source = root.join("local").join("alpha");
    make_skill(&source, "alpha", "Alpha skill");
    import_skill(&source, SkillKind::Remote, &managed_root).unwrap();
    let remote = bare_remote_with_skill_content("source-binding-mismatch-origin", "beta", "Beta skill", "");

    let preview = preview_remote_source_binding(
        RemoteSourceBindingRequest {
            skill_name: "alpha".to_string(),
            source_url: format!("{}/tree/main/skills/beta", github_file_url(&remote)),
            actor: "cli".to_string(),
        },
        &managed_root,
    )
    .unwrap();

    assert_eq!(preview.validation, SourceBindingValidation::Mismatch);
    assert!(preview.message.contains("Remote skill name beta does not match alpha"));
}
```

- [ ] **Step 2: Run tests to verify red**

Run: `cargo test -p skillbox-core --offline source_binding`

Expected: FAIL because binding types and functions do not exist.

- [ ] **Step 3: Add source binding API types**

Add public types:

```rust
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceBindingValidation {
    ExactMatch,
    SameSkillChanged,
    Mismatch,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteSourceBindingRequest {
    pub skill_name: String,
    pub source_url: String,
    pub actor: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RemoteSourceBindingPreview {
    pub skill_name: String,
    pub source_url: String,
    pub repo_url: String,
    pub owner: String,
    pub repo: String,
    pub path: String,
    pub reference: String,
    pub ref_kind: Option<String>,
    pub tracking: bool,
    pub current_version: String,
    pub installed_sha: Option<String>,
    pub latest_sha: Option<String>,
    pub validation: SourceBindingValidation,
    pub local_hash: String,
    pub remote_hash: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BindRemoteSourceRequest {
    pub skill_name: String,
    pub source_url: String,
    pub actor: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BindRemoteSourceResult {
    pub skill_name: String,
    pub validation: SourceBindingValidation,
    pub current_version: String,
    pub installed_sha: Option<String>,
    pub latest_sha: Option<String>,
    pub source_path: PathBuf,
    pub operation_id: String,
}
```

- [ ] **Step 4: Implement binding preview**

Add `preview_remote_source_binding`:

```rust
pub fn preview_remote_source_binding(
    request: RemoteSourceBindingRequest,
    managed_root: impl AsRef<Path>,
) -> Result<RemoteSourceBindingPreview> {
    validate_skill_name(&request.skill_name)?;
    let paths = ensure_managed_layout(managed_root.as_ref().to_path_buf())?;
    let local_current = paths.remote_skills_root.join(&request.skill_name).join("current");
    let local_skill = read_skill(&local_current)?;
    let current_version = current_remote_version(&paths, &request.skill_name)?;
    let source = skillbox_github::parse_github_skill_url(&request.source_url)?;
    let temp = temporary_work_dir("source-binding");
    let checkout = temp.join("checkout");
    let latest_sha = skillbox_git::fetch_ref_path(&source.repo_url, &source.reference, &source.path, &checkout)?;
    let remote_skill_path = checkout.join(&source.path);
    let remote_skill = read_skill(&remote_skill_path)?;
    let ref_kind = resolve_ref_kind(&source.repo_url, &source.reference)?;
    let tracking = ref_kind == "branch";
    let validation = if remote_skill.name != request.skill_name {
        SourceBindingValidation::Mismatch
    } else if remote_skill.content_hash == local_skill.content_hash {
        SourceBindingValidation::ExactMatch
    } else {
        SourceBindingValidation::SameSkillChanged
    };
    let message = source_binding_message(&request.skill_name, &remote_skill.name, validation);

    Ok(RemoteSourceBindingPreview {
        skill_name: request.skill_name,
        source_url: source.url,
        repo_url: source.repo_url,
        owner: source.owner,
        repo: source.repo,
        path: source.path,
        reference: source.reference,
        ref_kind: Some(ref_kind),
        tracking,
        current_version,
        installed_sha: None,
        latest_sha: Some(latest_sha),
        validation,
        local_hash: local_skill.content_hash,
        remote_hash: Some(remote_skill.content_hash),
        message,
    })
}
```

Add these private helpers used by source binding:

```rust
fn current_remote_version(paths: &ManagedPaths, skill_name: &str) -> Result<String> {
    let current = paths.remote_skills_root.join(skill_name).join("current");
    let target = fs::read_link(&current).map_err(|error| error.to_string())?;
    target
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string)
        .ok_or_else(|| format!("Current version target is invalid: {}", current.display()))
}

fn temporary_work_dir(label: &str) -> PathBuf {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    std::env::temp_dir().join(format!("skillbox-{label}-{nanos}"))
}

fn resolve_ref_kind(repo_url: &str, reference: &str) -> Result<String> {
    if skillbox_github::classify_ref_text(reference) == skillbox_github::GitHubRefKind::Commit {
        return Ok("commit".to_string());
    }
    if skillbox_git::ls_remote(repo_url, &format!("refs/heads/{reference}"))?.is_some() {
        return Ok("branch".to_string());
    }
    if skillbox_git::ls_remote(repo_url, &format!("refs/tags/{reference}"))?.is_some() {
        return Ok("tag".to_string());
    }
    Ok("branch".to_string())
}

fn source_binding_message(
    requested_name: &str,
    remote_name: &str,
    validation: SourceBindingValidation,
) -> String {
    match validation {
        SourceBindingValidation::ExactMatch => {
            "Remote source matches the current skill content.".to_string()
        }
        SourceBindingValidation::SameSkillChanged => {
            "Skill names match but content differs. Binding will not replace current.".to_string()
        }
        SourceBindingValidation::Mismatch => {
            format!("Remote skill name {remote_name} does not match {requested_name}.")
        }
    }
}
```

- [ ] **Step 5: Implement bind execution with operation logging**

Add `bind_remote_source`:

```rust
pub fn bind_remote_source(
    request: BindRemoteSourceRequest,
    managed_root: impl AsRef<Path>,
) -> Result<BindRemoteSourceResult> {
    let managed_root = managed_root.as_ref().to_path_buf();
    let operation = start_operation(
        OperationStart {
            operation_type: "bind_remote_source".to_string(),
            actor: request.actor.clone(),
            entity_type: "skill".to_string(),
            entity_name: request.skill_name.clone(),
            summary: format!("Bind {} to GitHub source", request.skill_name),
            payload: serde_json::json!({"sourceUrl": request.source_url}),
        },
        &managed_root,
    )?;
    let preview = match preview_remote_source_binding(
        RemoteSourceBindingRequest {
            skill_name: request.skill_name.clone(),
            source_url: request.source_url.clone(),
            actor: request.actor,
        },
        &managed_root,
    ) {
        Ok(preview) => preview,
        Err(error) => {
            let _ = finish_operation(
                OperationFinish {
                    id: operation.id,
                    status: OperationStatus::Failed,
                    summary: format!("Bind {} failed", request.skill_name),
                    error: Some(error.clone()),
                    payload: serde_json::json!({}),
                },
                &managed_root,
            );
            return Err(error);
        }
    };

    if preview.validation == SourceBindingValidation::Mismatch {
        finish_operation(
            OperationFinish {
                id: operation.id,
                status: OperationStatus::Failed,
                summary: format!("Bind {} rejected", request.skill_name),
                error: Some(preview.message.clone()),
                payload: serde_json::json!({"validation":"mismatch"}),
            },
            &managed_root,
        )?;
        return Err(preview.message);
    }

    let paths = ensure_managed_layout(managed_root.clone())?;
    let source_path = paths.remote_skills_root.join(&preview.skill_name).join("source.json");
    write_github_source_metadata(&source_path, &preview)?;
    finish_operation(
        OperationFinish {
            id: operation.id.clone(),
            status: OperationStatus::Succeeded,
            summary: format!("Bound {} to GitHub source", preview.skill_name),
            error: None,
            payload: serde_json::json!({
                "validation": format!("{:?}", preview.validation),
                "currentVersion": preview.current_version,
                "latestSha": preview.latest_sha,
                "tracking": preview.tracking
            }),
        },
        &managed_root,
    )?;

    Ok(BindRemoteSourceResult {
        skill_name: preview.skill_name,
        validation: preview.validation,
        current_version: preview.current_version,
        installed_sha: preview.installed_sha,
        latest_sha: preview.latest_sha,
        source_path,
        operation_id: operation.id,
    })
}
```

Add metadata helpers:

```rust
fn write_github_source_metadata(path: &Path, preview: &RemoteSourceBindingPreview) -> Result<()> {
    let parent = path.parent().ok_or_else(|| "Source metadata path has no parent.".to_string())?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let json = serde_json::json!({
        "type": "github",
        "owner": preview.owner,
        "repo": preview.repo,
        "path": preview.path,
        "ref": preview.reference,
        "refKind": preview.ref_kind,
        "tracking": preview.tracking,
        "repoUrl": preview.repo_url,
        "url": preview.source_url,
        "currentVersion": preview.current_version,
        "installedSha": preview.installed_sha,
        "latestSha": preview.latest_sha,
        "sourceLinkedAt": iso_timestamp_now()
    });
    fs::write(path, serde_json::to_string_pretty(&json).map_err(|error| error.to_string())?)
        .map_err(|error| error.to_string())
}

fn read_remote_source(remote_root: &Path) -> Result<RemoteSkillSource> {
    let source_path = remote_root.join("source.json");
    let content = fs::read_to_string(&source_path).map_err(|error| error.to_string())?;
    serde_json::from_str(&content).map_err(|error| error.to_string())
}
```

- [ ] **Step 6: Add binding test helpers**

Inside the Rust test module, add:

```rust
fn bare_remote_with_skill_content(
    label: &str,
    skill_name: &str,
    description: &str,
    body: &str,
) -> PathBuf {
    let remote = bare_remote(label);
    let work = temp_dir(&format!("{label}-work"));
    run_git(&work, &["init", "-b", "main"]);
    let skill_dir = work.join("skills").join(skill_name);
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(
        skill_dir.join("SKILL.md"),
        format!("---\nname: {skill_name}\ndescription: {description}\n---\n{body}"),
    )
    .unwrap();
    run_git(&work, &["add", "."]);
    run_git(&work, &["commit", "-m", "Add skill"]);
    run_git(&work, &["remote", "add", "origin", remote.to_str().unwrap()]);
    run_git(&work, &["push", "-u", "origin", "main"]);
    remote
}

fn github_file_url(remote: &Path) -> String {
    remote.to_string_lossy().trim_end_matches(".git").to_string()
}
```

- [ ] **Step 7: Verify binding tests pass**

Run: `cargo test -p skillbox-core --offline source_binding`

Expected: PASS.

- [ ] **Step 8: Document source binding workflow**

In `docs/workflows.md`, add a section `Bind Remote Source` covering `exact_match`, `same_skill_changed`, and `mismatch`, and state that changed-source binding does not switch `current`.

- [ ] **Step 9: Commit**

```bash
git add crates/skillbox-core/src/lib.rs docs/workflows.md
git commit -m "feat(github): bind remote skill sources"
```

## Task 5: Remote Version List And Diff Preview

**Files:**
- Modify: `crates/skillbox-core/src/lib.rs`

- [ ] **Step 1: Write failing tests for version list and preview**

Add tests:

```rust
#[test]
fn list_remote_skill_versions_marks_current() {
    let root = temp_dir("remote-version-list");
    let managed_root = root.join("SkillBox");
    let paths = ensure_managed_layout(&managed_root).unwrap();
    let source = root.join("local").join("demo");
    make_skill(&source, "demo", "Demo skill");
    import_skill(&source, SkillKind::Remote, &managed_root).unwrap();
    let versions = list_remote_skill_versions("demo", &managed_root).unwrap();

    assert_eq!(versions.skill_name, "demo");
    assert_eq!(versions.versions.len(), 1);
    assert!(versions.versions[0].is_current);
    assert!(versions.versions[0].version.starts_with("manual-"));
}

#[test]
fn preview_rollback_lists_every_changed_file() {
    let root = temp_dir("remote-preview-rollback");
    let managed_root = root.join("SkillBox");
    let paths = ensure_managed_layout(&managed_root).unwrap();
    let source_v1 = root.join("local-v1").join("demo");
    make_skill(&source_v1, "demo", "Demo skill");
    import_skill(&source_v1, SkillKind::Remote, &managed_root).unwrap();
    let v1 = current_remote_version(&paths, "demo").unwrap();

    let remote_root = paths.remote_skills_root.join("demo");
    let v2_path = remote_root.join("versions").join("0123456789abcdef0123456789abcdef01234567");
    copy_skill_dir(&source_v1, &v2_path).unwrap();
    fs::write(v2_path.join("SKILL.md"), "---\nname: demo\ndescription: Demo skill\n---\nupdated\n").unwrap();
    fs::write(v2_path.join("extra.txt"), "extra\n").unwrap();
    update_current_symlink(&remote_root, &v2_path).unwrap();

    let preview = preview_remote_version_change(
        RemoteVersionChangeRequest {
            skill_name: "demo".to_string(),
            action: RemoteVersionChangeAction::Rollback,
            target_version: Some(v1.clone()),
            actor: "cli".to_string(),
        },
        &managed_root,
    )
    .unwrap();

    assert_eq!(preview.from_version, "0123456789abcdef0123456789abcdef01234567");
    assert_eq!(preview.to_version, v1);
    assert!(preview.files.iter().any(|file| file.path == "SKILL.md"));
    assert!(preview.files.iter().any(|file| file.path == "extra.txt"));
    assert!(preview.files.iter().any(|file| file.diff.contains("-extra")));
}
```

- [ ] **Step 2: Run tests to verify red**

Run: `cargo test -p skillbox-core --offline remote_version`

Expected: FAIL because version-list and preview types/functions do not exist.

- [ ] **Step 3: Add version and preview types**

Add:

```rust
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteVersionChangeAction {
    Update,
    Rollback,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteVersionChangeRequest {
    pub skill_name: String,
    pub action: RemoteVersionChangeAction,
    pub target_version: Option<String>,
    pub actor: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RemoteSkillVersion {
    pub version: String,
    pub is_current: bool,
    pub kind: String,
    pub short_label: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RemoteSkillVersionList {
    pub skill_name: String,
    pub current_version: String,
    pub versions: Vec<RemoteSkillVersion>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RemoteDiffFile {
    pub path: String,
    pub old_path: Option<String>,
    pub status: String,
    pub label: String,
    pub diff: String,
    pub old_hash: Option<String>,
    pub new_hash: Option<String>,
    pub old_size: Option<u64>,
    pub new_size: Option<u64>,
    pub binary: bool,
    pub too_large: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AffectedDeployment {
    pub target_root: PathBuf,
    pub target_path: PathBuf,
    pub mode: String,
    pub state: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RemoteVersionChangePreview {
    pub preview_id: String,
    pub skill_name: String,
    pub action: RemoteVersionChangeAction,
    pub from_version: String,
    pub to_version: String,
    pub files: Vec<RemoteDiffFile>,
    pub affected_deployments: Vec<AffectedDeployment>,
}
```

- [ ] **Step 4: Implement version listing**

Add `list_remote_skill_versions`:

```rust
pub fn list_remote_skill_versions(
    skill_name: &str,
    managed_root: impl AsRef<Path>,
) -> Result<RemoteSkillVersionList> {
    validate_skill_name(skill_name)?;
    let paths = ensure_managed_layout(managed_root.as_ref().to_path_buf())?;
    let current_version = current_remote_version(&paths, skill_name)?;
    let versions_root = paths.remote_skills_root.join(skill_name).join("versions");
    let mut versions = Vec::new();
    for entry in fs::read_dir(&versions_root).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        if !entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false) {
            continue;
        }
        let version = entry.file_name().to_string_lossy().to_string();
        versions.push(RemoteSkillVersion {
            short_label: short_version_label(&version),
            kind: if version.starts_with("manual-") { "manual" } else { "github" }.to_string(),
            is_current: version == current_version,
            path: entry.path(),
            version,
        });
    }
    versions.sort_by(|left, right| right.is_current.cmp(&left.is_current).then(left.version.cmp(&right.version)));
    Ok(RemoteSkillVersionList {
        skill_name: skill_name.to_string(),
        current_version,
        versions,
    })
}
```

- [ ] **Step 5: Implement preview target resolution**

Add helpers:

```rust
fn resolve_remote_version_change_target(
    paths: &ManagedPaths,
    request: &RemoteVersionChangeRequest,
) -> Result<String> {
    match request.action {
        RemoteVersionChangeAction::Rollback => {
            let target = request
                .target_version
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| "Rollback target version is required.".to_string())?;
            resolve_remote_version_prefix(paths, &request.skill_name, target)
        }
        RemoteVersionChangeAction::Update => {
            let source = read_remote_source(&paths.remote_skills_root.join(&request.skill_name))?;
            source.latest_sha.ok_or_else(|| "No latest GitHub SHA is available.".to_string())
        }
    }
}
```

Implement `resolve_remote_version_prefix` to scan `versions/*`, accept exact matches, accept one unambiguous prefix match, reject no matches with `Version not found: <input>`, and reject multiple matches with `Version prefix is ambiguous: <input>`.

- [ ] **Step 6: Implement diff preview**

Add `preview_remote_version_change`:

```rust
pub fn preview_remote_version_change(
    request: RemoteVersionChangeRequest,
    managed_root: impl AsRef<Path>,
) -> Result<RemoteVersionChangePreview> {
    validate_skill_name(&request.skill_name)?;
    let paths = ensure_managed_layout(managed_root.as_ref().to_path_buf())?;
    let from_version = current_remote_version(&paths, &request.skill_name)?;
    let to_version = resolve_remote_version_change_target(&paths, &request)?;
    let remote_root = paths.remote_skills_root.join(&request.skill_name);
    let from_path = remote_root.join("versions").join(&from_version);
    let to_path = remote_root.join("versions").join(&to_version);
    let from_skill = read_skill(&from_path)?;
    let to_skill = read_skill(&to_path)?;
    if from_skill.name != to_skill.name || to_skill.name != request.skill_name {
        return Err(format!("Version skill name does not match {}", request.skill_name));
    }

    let git_files = skillbox_git::diff_no_index_tree(&from_path, &to_path)?;
    let files = git_files
        .into_iter()
        .map(|file| remote_diff_file(&from_path, &to_path, file))
        .collect::<Result<Vec<_>>>()?;
    let affected_deployments = classify_affected_deployments(&paths, &request.skill_name)?;
    let preview_id = content_hash_text(&format!("{}:{}:{}", request.skill_name, from_version, to_version));

    Ok(RemoteVersionChangePreview {
        preview_id,
        skill_name: request.skill_name,
        action: request.action,
        from_version,
        to_version,
        files,
        affected_deployments,
    })
}
```

`remote_diff_file` must fill `old_hash`, `new_hash`, `old_size`, `new_size`, `binary`, and `too_large`. Treat files over `120_000` bytes as `too_large`; treat files that cannot be decoded as UTF-8 as `binary`; keep them in the file list with empty `diff` and hash/size metadata.

Add the supporting helpers used above:

```rust
fn content_hash_text(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn classify_affected_deployments(
    paths: &ManagedPaths,
    skill_name: &str,
) -> Result<Vec<AffectedDeployment>> {
    let deployments = load_deployments(&paths.database_path)?;
    let current = paths.remote_skills_root.join(skill_name).join("current");
    let expected_current = fs::read_link(&current).unwrap_or(current.clone());
    let mut affected = Vec::new();
    for deployment in deployments.get(skill_name).cloned().unwrap_or_default() {
        let link_target = fs::read_link(&deployment.target_path).ok();
        let state = if link_target.as_ref() == Some(&current) {
            "follows_current"
        } else if link_target
            .as_ref()
            .map(|target| target.starts_with(paths.remote_skills_root.join(skill_name).join("versions")))
            .unwrap_or(false)
        {
            "pinned_version"
        } else {
            "unmanaged"
        };
        let message = match state {
            "follows_current" => "Deployment follows current and will update automatically.",
            "pinned_version" => "Deployment is pinned to an old version.",
            _ => "Deployment target is not a SkillBox-managed current symlink.",
        };
        affected.push(AffectedDeployment {
            target_root: deployment.target_root,
            target_path: deployment.target_path,
            mode: deployment.mode,
            state: state.to_string(),
            message: message.to_string(),
        });
    }
    let _ = expected_current;
    Ok(affected)
}
```

- [ ] **Step 7: Verify preview tests pass**

Run: `cargo test -p skillbox-core --offline remote_version`

Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add crates/skillbox-core/src/lib.rs
git commit -m "feat(core): preview remote version changes"
```

## Task 6: Apply Update And Rollback Safely

**Files:**
- Modify: `crates/skillbox-core/src/lib.rs`
- Modify: `docs/workflows.md`

- [ ] **Step 1: Write failing tests for apply and restore**

Add tests:

```rust
#[test]
fn apply_rollback_switches_current_and_records_operation() {
    let root = temp_dir("apply-rollback");
    let managed_root = root.join("SkillBox");
    let paths = ensure_managed_layout(&managed_root).unwrap();
    let source_v1 = root.join("local-v1").join("demo");
    make_skill(&source_v1, "demo", "Demo skill");
    import_skill(&source_v1, SkillKind::Remote, &managed_root).unwrap();
    let v1 = current_remote_version(&paths, "demo").unwrap();
    let remote_root = paths.remote_skills_root.join("demo");
    let v2 = "0123456789abcdef0123456789abcdef01234567";
    let v2_path = remote_root.join("versions").join(v2);
    copy_skill_dir(&source_v1, &v2_path).unwrap();
    fs::write(v2_path.join("SKILL.md"), "---\nname: demo\ndescription: Demo skill\n---\nupdated\n").unwrap();
    update_current_symlink(&remote_root, &v2_path).unwrap();

    let result = apply_remote_version_change(
        RemoteVersionChangeApplyRequest {
            skill_name: "demo".to_string(),
            action: RemoteVersionChangeAction::Rollback,
            target_version: v1.clone(),
            preview_id: None,
            actor: "cli".to_string(),
        },
        &managed_root,
    )
    .unwrap();

    assert_eq!(result.from_version, v2);
    assert_eq!(result.to_version, v1);
    assert_eq!(current_remote_version(&paths, "demo").unwrap(), result.to_version);
    let operations = list_operations(OperationFilter::default(), &managed_root).unwrap();
    assert!(operations.operations.iter().any(|operation| operation.operation_type == "rollback_remote_skill"));
}

#[test]
fn apply_update_writes_latest_version_and_preserves_old_version() {
    let root = temp_dir("apply-update");
    let managed_root = root.join("SkillBox");
    let paths = ensure_managed_layout(&managed_root).unwrap();
    let source = root.join("local").join("find-skills");
    make_skill(&source, "find-skills", "Find skills");
    import_skill(&source, SkillKind::Remote, &managed_root).unwrap();
    let old_version = current_remote_version(&paths, "find-skills").unwrap();
    let remote = bare_remote_with_skill_content(
        "apply-update-origin",
        "find-skills",
        "Find skills",
        "Updated remote body\n",
    );
    let url = format!("{}/tree/main/skills/find-skills", github_file_url(&remote));
    bind_remote_source(
        BindRemoteSourceRequest {
            skill_name: "find-skills".to_string(),
            source_url: url,
            actor: "cli".to_string(),
        },
        &managed_root,
    )
    .unwrap();
    let latest_sha = read_remote_source(&paths.remote_skills_root.join("find-skills")).unwrap().latest_sha.unwrap();

    let result = apply_remote_version_change(
        RemoteVersionChangeApplyRequest {
            skill_name: "find-skills".to_string(),
            action: RemoteVersionChangeAction::Update,
            target_version: latest_sha.clone(),
            preview_id: None,
            actor: "cli".to_string(),
        },
        &managed_root,
    )
    .unwrap();

    assert_eq!(result.to_version, latest_sha);
    assert!(paths.remote_skills_root.join("find-skills").join("versions").join(&old_version).exists());
    assert!(paths.remote_skills_root.join("find-skills").join("versions").join(&result.to_version).exists());
    assert_eq!(current_remote_version(&paths, "find-skills").unwrap(), result.to_version);
}
```

- [ ] **Step 2: Run tests to verify red**

Run: `cargo test -p skillbox-core --offline apply_`

Expected: FAIL because `RemoteVersionChangeApplyRequest`, `apply_remote_version_change`, and result types do not exist.

- [ ] **Step 3: Add apply request and result types**

Add:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteVersionChangeApplyRequest {
    pub skill_name: String,
    pub action: RemoteVersionChangeAction,
    pub target_version: String,
    pub preview_id: Option<String>,
    pub actor: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RemoteVersionChangeApplyResult {
    pub skill_name: String,
    pub action: RemoteVersionChangeAction,
    pub from_version: String,
    pub to_version: String,
    pub current_path: PathBuf,
    pub affected_deployments: Vec<AffectedDeployment>,
    pub operation_id: String,
}
```

- [ ] **Step 4: Implement apply workflow with operation logging**

Add:

```rust
pub fn apply_remote_version_change(
    request: RemoteVersionChangeApplyRequest,
    managed_root: impl AsRef<Path>,
) -> Result<RemoteVersionChangeApplyResult> {
    validate_skill_name(&request.skill_name)?;
    let managed_root = managed_root.as_ref().to_path_buf();
    let operation_type = match request.action {
        RemoteVersionChangeAction::Update => "update_remote_skill",
        RemoteVersionChangeAction::Rollback => "rollback_remote_skill",
    };
    let operation = start_operation(
        OperationStart {
            operation_type: operation_type.to_string(),
            actor: request.actor.clone(),
            entity_type: "skill".to_string(),
            entity_name: request.skill_name.clone(),
            summary: format!("Apply {:?} for {}", request.action, request.skill_name),
            payload: serde_json::json!({"targetVersion": request.target_version, "previewId": request.preview_id}),
        },
        &managed_root,
    )?;
    let result = apply_remote_version_change_inner(&request, &managed_root);
    match result {
        Ok(result) => {
            finish_operation(
                OperationFinish {
                    id: operation.id.clone(),
                    status: OperationStatus::Succeeded,
                    summary: format!("Changed {} from {} to {}", result.skill_name, result.from_version, result.to_version),
                    error: None,
                    payload: serde_json::json!({
                        "fromVersion": result.from_version,
                        "toVersion": result.to_version,
                        "affectedDeployments": result.affected_deployments
                    }),
                },
                &managed_root,
            )?;
            Ok(RemoteVersionChangeApplyResult { operation_id: operation.id, ..result })
        }
        Err(error) => {
            let _ = finish_operation(
                OperationFinish {
                    id: operation.id,
                    status: OperationStatus::Failed,
                    summary: format!("Remote version change failed for {}", request.skill_name),
                    error: Some(error.clone()),
                    payload: serde_json::json!({"targetVersion": request.target_version}),
                },
                &managed_root,
            );
            Err(error)
        }
    }
}
```

Implement `apply_remote_version_change_inner` to:

1. Resolve `from_version` from `current`.
2. Resolve `to_version` exactly or through unambiguous prefix for rollback.
3. For update, download the GitHub snapshot if `versions/<to_version>` does not exist.
4. Validate target `SKILL.md` and matching skill name.
5. Save the old `current` symlink target.
6. Switch `current` to `versions/<to_version>`.
7. Update `source.json.currentVersion`; set `installedSha` only when `to_version` is a GitHub SHA.
8. Re-index the target skill with `index_skill`.
9. If any step after symlink switching fails, restore the previous symlink target and return an error containing `restored current`.

- [ ] **Step 5: Implement update snapshot install**

Add a helper:

```rust
fn ensure_github_version_snapshot(
    paths: &ManagedPaths,
    skill_name: &str,
    target_sha: &str,
) -> Result<PathBuf> {
    let remote_root = paths.remote_skills_root.join(skill_name);
    let version_path = remote_root.join("versions").join(target_sha);
    if version_path.exists() {
        read_skill(&version_path)?;
        return Ok(version_path);
    }
    let source = read_remote_source(&remote_root)?;
    let repo_url = source.repo_url.ok_or_else(|| "GitHub source is missing repoUrl.".to_string())?;
    let source_path = source.path.ok_or_else(|| "GitHub source is missing path.".to_string())?;
    let temp = temporary_work_dir("remote-update");
    let checkout = temp.join("checkout");
    skillbox_git::fetch_ref_path(&repo_url, target_sha, &source_path, &checkout)?;
    copy_skill_dir(&checkout.join(source_path), &version_path)?;
    Ok(version_path)
}
```

- [ ] **Step 6: Verify apply tests pass**

Run: `cargo test -p skillbox-core --offline apply_`

Expected: PASS.

- [ ] **Step 7: Update workflow docs**

In `docs/workflows.md`, replace the existing target `Update Remote Skill` and `Rollback Remote Skill` sections with the preview-then-apply flow, including current restoration on failure and no version pruning.

- [ ] **Step 8: Commit**

```bash
git add crates/skillbox-core/src/lib.rs docs/workflows.md
git commit -m "feat(core): apply remote version changes"
```

## Task 7: GitHub Candidate Search

**Files:**
- Modify: `crates/skillbox-core/Cargo.toml`
- Modify: `crates/skillbox-core/src/lib.rs`

- [ ] **Step 1: Write failing tests for ranking and search result parsing**

Add tests:

```rust
#[test]
fn ranks_source_candidates_by_name_path_trust_and_recency() {
    let candidates = rank_remote_source_candidates(
        "find-skills",
        vec![
            RemoteSourceCandidate {
                owner: "small".to_string(),
                repo: "misc".to_string(),
                path: "tools/other".to_string(),
                reference: "main".to_string(),
                source_url: "https://github.com/small/misc/tree/main/tools/other".to_string(),
                repo_url: "https://github.com/small/misc.git".to_string(),
                name: Some("other".to_string()),
                description: Some("Other".to_string()),
                stars: 1000,
                archived: false,
                fork: false,
                updated_at: "2026-01-01T00:00:00Z".to_string(),
                match_reasons: vec![],
                score: 0,
            },
            RemoteSourceCandidate {
                owner: "acme".to_string(),
                repo: "skills".to_string(),
                path: "skills/find-skills".to_string(),
                reference: "main".to_string(),
                source_url: "https://github.com/acme/skills/tree/main/skills/find-skills".to_string(),
                repo_url: "https://github.com/acme/skills.git".to_string(),
                name: Some("find-skills".to_string()),
                description: Some("Find skills".to_string()),
                stars: 10,
                archived: false,
                fork: false,
                updated_at: "2025-01-01T00:00:00Z".to_string(),
                match_reasons: vec![],
                score: 0,
            },
        ],
    );

    assert_eq!(candidates[0].path, "skills/find-skills");
    assert!(candidates[0].match_reasons.contains(&"Exact skill name match".to_string()));
}
```

- [ ] **Step 2: Run test to verify red**

Run: `cargo test -p skillbox-core --offline source_candidates`

Expected: FAIL because candidate types and ranking function do not exist.

- [ ] **Step 3: Add candidate types**

Add:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RemoteSourceCandidate {
    pub owner: String,
    pub repo: String,
    pub path: String,
    pub reference: String,
    pub source_url: String,
    pub repo_url: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub stars: u32,
    pub archived: bool,
    pub fork: bool,
    pub updated_at: String,
    pub match_reasons: Vec<String>,
    pub score: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RemoteSourceCandidateSearch {
    pub skill_name: String,
    pub candidates: Vec<RemoteSourceCandidate>,
}
```

- [ ] **Step 4: Implement deterministic ranking**

Add:

```rust
pub fn rank_remote_source_candidates(
    skill_name: &str,
    candidates: Vec<RemoteSourceCandidate>,
) -> Vec<RemoteSourceCandidate> {
    let normalized_skill = skill_name.to_ascii_lowercase();
    let mut ranked = candidates
        .into_iter()
        .map(|mut candidate| {
            let mut score = 0;
            if candidate
                .name
                .as_deref()
                .map(|name| name.eq_ignore_ascii_case(skill_name))
                .unwrap_or(false)
            {
                score += 500;
                candidate.match_reasons.push("Exact skill name match".to_string());
            }
            if candidate.path.to_ascii_lowercase().contains(&normalized_skill) {
                score += 300;
                candidate.match_reasons.push("Path contains skill name".to_string());
            }
            if candidate
                .description
                .as_deref()
                .map(|description| description.to_ascii_lowercase().contains(&normalized_skill))
                .unwrap_or(false)
            {
                score += 100;
                candidate.match_reasons.push("Description mentions skill name".to_string());
            }
            if !candidate.archived {
                score += 40;
            }
            if !candidate.fork {
                score += 30;
            }
            score += i32::try_from(candidate.stars.min(1000) / 25).unwrap_or(0);
            candidate.score = score;
            candidate
        })
        .collect::<Vec<_>>();
    ranked.sort_by(|left, right| right.score.cmp(&left.score).then(left.path.cmp(&right.path)));
    ranked
}
```

- [ ] **Step 5: Add network-backed search command with clear failure**

Add `find_remote_source_candidates(skill_name, managed_root)` that:

1. Reads the current local remote skill.
2. Builds a GitHub search URL for `SKILL.md` and the skill name.
3. Calls GitHub Search with a clear user agent.
4. Maps the JSON result to `RemoteSourceCandidate`.
5. Ranks candidates with `rank_remote_source_candidates`.

Use a dependency-free structured `curl` invocation so the first implementation does not introduce a new Rust HTTP dependency:

```rust
fn github_api_get(url: &str) -> Result<String> {
    let output = std::process::Command::new("curl")
        .arg("-fsSL")
        .arg("-H")
        .arg("Accept: application/vnd.github+json")
        .arg("-H")
        .arg("User-Agent: SkillBox")
        .arg(url)
        .output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}
```

Build the search URL with structured URL encoding from the existing `url` crate in `skillbox-github` or by adding `url = "2"` to `skillbox-core` if direct encoding is simpler. The command must pass the whole URL as one argument and must not use shell strings.

- [ ] **Step 6: Verify ranking tests pass**

Run: `cargo test -p skillbox-core --offline source_candidates`

Expected: PASS. Network-backed search is manually verified later because automated tests use deterministic ranking inputs.

- [ ] **Step 7: Commit**

```bash
git add crates/skillbox-core/Cargo.toml crates/skillbox-core/src/lib.rs Cargo.lock
git commit -m "feat(github): search remote skill sources"
```

## Task 8: CLI And Tauri Commands

**Files:**
- Modify: `crates/skillbox-cli/src/main.rs`
- Modify: `apps/desktop/src-tauri/src/lib.rs`

- [ ] **Step 1: Add CLI smoke tests manually through command design**

Add CLI match arms in `crates/skillbox-cli/src/main.rs` for:

```text
remote-source-preview <skill-name> <github-url> [--managed-root <path>]
bind-remote-source <skill-name> <github-url> [--managed-root <path>]
remote-versions <skill-name> [--managed-root <path>]
remote-preview-change <skill-name> --action update|rollback [--to <version>] [--managed-root <path>]
remote-apply-change <skill-name> --action update|rollback --to <version> [--managed-root <path>]
operations [--entity-type <type>] [--entity-name <name>] [--status started|succeeded|failed|cancelled] [--managed-root <path>]
```

- [ ] **Step 2: Implement CLI request mapping**

Use the existing `positional`, `option`, `managed_root`, and `print_json` helpers. For action parsing, add:

```rust
fn remote_change_action(value: Option<String>) -> Result<skillbox_core::RemoteVersionChangeAction, String> {
    match value.as_deref() {
        Some("update") => Ok(skillbox_core::RemoteVersionChangeAction::Update),
        Some("rollback") => Ok(skillbox_core::RemoteVersionChangeAction::Rollback),
        _ => Err("Use --action update|rollback".to_string()),
    }
}
```

Each command should pass `actor: "cli".to_string()`.

- [ ] **Step 3: Add Tauri command wrappers**

In `apps/desktop/src-tauri/src/lib.rs`, add commands:

```rust
#[tauri::command]
fn find_remote_source_candidates(skill_name: String) -> Result<Value, String> {
    let result = skillbox_core::find_remote_source_candidates(
        &skill_name,
        skillbox_core::default_managed_root(),
    )?;
    serde_json::to_value(result).map_err(|error| error.to_string())
}

#[tauri::command]
fn preview_remote_source_binding(
    request: skillbox_core::RemoteSourceBindingRequest,
) -> Result<Value, String> {
    let result = skillbox_core::preview_remote_source_binding(
        request,
        skillbox_core::default_managed_root(),
    )?;
    serde_json::to_value(result).map_err(|error| error.to_string())
}

#[tauri::command]
fn bind_remote_source(request: skillbox_core::BindRemoteSourceRequest) -> Result<Value, String> {
    let result = skillbox_core::bind_remote_source(request, skillbox_core::default_managed_root())?;
    serde_json::to_value(result).map_err(|error| error.to_string())
}
```

Add these wrappers too:

```rust
#[tauri::command]
fn list_remote_skill_versions(skill_name: String) -> Result<Value, String> {
    let result = skillbox_core::list_remote_skill_versions(
        &skill_name,
        skillbox_core::default_managed_root(),
    )?;
    serde_json::to_value(result).map_err(|error| error.to_string())
}

#[tauri::command]
fn preview_remote_version_change(
    request: skillbox_core::RemoteVersionChangeRequest,
) -> Result<Value, String> {
    let result = skillbox_core::preview_remote_version_change(
        request,
        skillbox_core::default_managed_root(),
    )?;
    serde_json::to_value(result).map_err(|error| error.to_string())
}

#[tauri::command]
fn apply_remote_version_change(
    request: skillbox_core::RemoteVersionChangeApplyRequest,
) -> Result<Value, String> {
    let result = skillbox_core::apply_remote_version_change(
        request,
        skillbox_core::default_managed_root(),
    )?;
    serde_json::to_value(result).map_err(|error| error.to_string())
}

#[tauri::command]
fn list_operations(request: skillbox_core::OperationFilter) -> Result<Value, String> {
    let result = skillbox_core::list_operations(request, skillbox_core::default_managed_root())?;
    serde_json::to_value(result).map_err(|error| error.to_string())
}
```

Register all new commands in `tauri::generate_handler!`.

- [ ] **Step 4: Verify CLI and Tauri compile**

Run:

```bash
cargo test -p skillbox-cli --offline
cargo check -p skillbox-desktop --offline
```

Expected: PASS.

- [ ] **Step 5: Run one CLI manual command against a temp root**

Run:

```bash
cargo run -p skillbox-cli --offline -- operations --managed-root /tmp/skillbox-ops-plan
```

Expected: JSON with an `operations` array.

- [ ] **Step 6: Commit**

```bash
git add crates/skillbox-cli/src/main.rs apps/desktop/src-tauri/src/lib.rs
git commit -m "feat(cli): expose remote skill operations"
```

## Task 9: Frontend Normalization And Shared Diff Component

**Files:**
- Create: `apps/desktop/src/GitDiffView.jsx`
- Create: `apps/desktop/src/remoteSkills.js`
- Modify: `apps/desktop/src/App.jsx`
- Modify: `apps/desktop/src/App.import-candidates.test.js`
- Modify: `apps/desktop/src/skillStatusRefresh.js`

- [ ] **Step 1: Write failing UI helper tests**

Add to `apps/desktop/src/App.import-candidates.test.js`:

```js
import {
  canApplyRemoteVersionChange,
  formatRemoteRefBehavior,
  normalizeRemoteSourceBindingPreview,
  normalizeRemoteVersionPreview,
  remoteVersionActionLabel
} from './remoteSkills.js';

test('formats remote ref behavior for tracking and pinned sources', () => {
  assert.equal(formatRemoteRefBehavior({ refKind: 'branch', reference: 'main', tracking: true }), 'Tracking branch: main');
  assert.equal(formatRemoteRefBehavior({ refKind: 'tag', reference: 'v1.0.0', tracking: false }), 'Pinned tag: v1.0.0');
  assert.equal(formatRemoteRefBehavior({ refKind: 'commit', reference: 'abc123', tracking: false }), 'Pinned commit: abc123');
});

test('normalizes changed source binding without replacing current version', () => {
  const preview = normalizeRemoteSourceBindingPreview({
    skill_name: 'find-skills',
    validation: 'same_skill_changed',
    current_version: 'manual-abc',
    latest_sha: '1234567890abcdef',
    ref_kind: 'branch',
    tracking: true,
    message: 'Skill names match but content differs.'
  });

  assert.equal(preview.validation, 'same_skill_changed');
  assert.equal(preview.replacesCurrent, false);
  assert.equal(preview.statusLabel, 'Source can be linked; current version will stay active.');
});

test('remote version preview requires files before apply', () => {
  assert.equal(canApplyRemoteVersionChange({ files: [], loading: false }), false);
  assert.equal(canApplyRemoteVersionChange({ files: [{ path: 'SKILL.md' }], loading: true }), false);
  assert.equal(canApplyRemoteVersionChange({ files: [{ path: 'SKILL.md' }], loading: false }), true);
});

test('normalizes remote version preview files', () => {
  const preview = normalizeRemoteVersionPreview({
    skill_name: 'demo',
    action: 'rollback',
    from_version: 'abcdef',
    to_version: 'manual-123',
    files: [{ path: 'SKILL.md', status: 'M', diff: '@@\n-old\n+new\n' }]
  });

  assert.equal(preview.skillName, 'demo');
  assert.equal(preview.files[0].label, 'Modified');
  assert.equal(remoteVersionActionLabel(preview), 'Rollback');
});
```

- [ ] **Step 2: Run tests to verify red**

Run: `npm test -- apps/desktop/src/App.import-candidates.test.js`

Expected: FAIL because `remoteSkills.js` does not exist.

- [ ] **Step 3: Create `remoteSkills.js`**

Create `apps/desktop/src/remoteSkills.js`:

```js
export function formatRemoteRefBehavior(source = {}) {
  const ref = source.reference || source.ref || '';
  if (source.tracking || source.refKind === 'branch' || source.ref_kind === 'branch') {
    return `Tracking branch: ${ref || 'main'}`;
  }
  if (source.refKind === 'tag' || source.ref_kind === 'tag') {
    return `Pinned tag: ${ref}`;
  }
  return `Pinned commit: ${ref}`;
}

export function normalizeRemoteSourceBindingPreview(preview = {}) {
  const validation = preview.validation || 'mismatch';
  return {
    skillName: preview.skillName || preview.skill_name || '',
    validation,
    currentVersion: preview.currentVersion || preview.current_version || '',
    latestSha: preview.latestSha || preview.latest_sha || '',
    refKind: preview.refKind || preview.ref_kind || '',
    tracking: Boolean(preview.tracking),
    message: preview.message || '',
    replacesCurrent: false,
    statusLabel:
      validation === 'exact_match'
        ? 'Source can be linked; current version already matches.'
        : validation === 'same_skill_changed'
          ? 'Source can be linked; current version will stay active.'
          : 'This source does not match the selected skill.'
  };
}

export function normalizeRemoteVersionPreview(preview = {}) {
  const files = (preview.files || []).map((file) => ({
    path: file.path || '',
    oldPath: file.oldPath || file.old_path || '',
    status: file.status || '',
    label: remoteFileStatusLabel(file.status || ''),
    diff: file.diff || '',
    oldHash: file.oldHash || file.old_hash || '',
    newHash: file.newHash || file.new_hash || '',
    oldSize: file.oldSize ?? file.old_size ?? null,
    newSize: file.newSize ?? file.new_size ?? null,
    binary: Boolean(file.binary),
    tooLarge: Boolean(file.tooLarge ?? file.too_large)
  })).filter((file) => file.path);

  return {
    previewId: preview.previewId || preview.preview_id || '',
    skillName: preview.skillName || preview.skill_name || '',
    action: preview.action || 'update',
    fromVersion: preview.fromVersion || preview.from_version || '',
    toVersion: preview.toVersion || preview.to_version || '',
    files,
    activePath: files[0]?.path || '',
    affectedDeployments: preview.affectedDeployments || preview.affected_deployments || []
  };
}

export function canApplyRemoteVersionChange({ files = [], loading = false } = {}) {
  return !loading && files.length > 0;
}

export function remoteVersionActionLabel(preview = {}) {
  return preview.action === 'rollback' ? 'Rollback' : 'Update';
}

export function remoteFileStatusLabel(status) {
  if (status.startsWith('A')) return 'Added';
  if (status.startsWith('D')) return 'Deleted';
  if (status.startsWith('R')) return 'Renamed';
  if (status.startsWith('M')) return 'Modified';
  return status || 'Changed';
}
```

- [ ] **Step 4: Extract `GitDiffView`**

Create `apps/desktop/src/GitDiffView.jsx` by moving the existing `GitDiffView` component out of `App.jsx`:

```jsx
import { parseUnifiedDiff } from './gitDiffView.js';

export function GitDiffView({ diff }) {
  const rows = parseUnifiedDiff(diff);

  if (rows.length === 0) {
    return <div className="gitDiffEmpty">No diff to show.</div>;
  }

  return (
    <div className="githubDiffScroller">
      <table className="githubDiffTable" aria-label="Unified diff">
        <tbody>
          {rows.map((row, index) => (
            <tr className={`githubDiffRow ${row.kind}`} key={`${index}-${row.kind}`}>
              <td className="githubDiffLineNumber">{row.oldLine ?? ''}</td>
              <td className="githubDiffLineNumber">{row.newLine ?? ''}</td>
              <td className="githubDiffMarker">{row.marker}</td>
              <td className="githubDiffCode">{row.content || ' '}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
```

Import it in `App.jsx` with:

```js
import { GitDiffView } from './GitDiffView.jsx';
```

Remove the local `GitDiffView` function from `App.jsx`.

- [ ] **Step 5: Update remote update status normalization**

In `apps/desktop/src/skillStatusRefresh.js`, include `currentVersion`, `refKind`, and `tracking` in `normalizeRemoteSkillUpdates`, and return a `Pinned` row status:

```js
if (status.state === 'pinned') {
  return { label: 'Pinned', tone: 'blue' };
}
```

Update `dashboardStatusNotice` to include pinned count:

```js
const pinned = statuses.filter((status) => status.state === 'pinned').length;
if (pinned) parts.push(`${pinned} pinned`);
```

- [ ] **Step 6: Verify UI helper tests pass**

Run: `npm test -- apps/desktop/src/App.import-candidates.test.js`

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add apps/desktop/src/GitDiffView.jsx apps/desktop/src/remoteSkills.js apps/desktop/src/App.jsx apps/desktop/src/App.import-candidates.test.js apps/desktop/src/skillStatusRefresh.js
git commit -m "feat(desktop): share remote diff helpers"
```

## Task 10: Desktop Remote Workflows And Final Docs

**Files:**
- Modify: `apps/desktop/src/App.jsx`
- Modify: `apps/desktop/src/styles.css`
- Modify: `docs/architecture.md`
- Modify: `docs/workflows.md`
- Modify: `docs/implementation-status.md`
- Modify: `CONTRIBUTING.md`

- [ ] **Step 1: Add App state for remote workflows**

In `App.jsx`, add state near existing modal state:

```js
const [remoteSourceDialog, setRemoteSourceDialog] = useState({
  open: false,
  skillName: '',
  sourceUrl: '',
  preview: null,
  error: '',
  loading: false
});
const [remoteVersionDialog, setRemoteVersionDialog] = useState({
  open: false,
  loading: false,
  applying: false,
  preview: null,
  activePath: '',
  error: ''
});
const [remoteVersions, setRemoteVersions] = useState({});
const [operationHistory, setOperationHistory] = useState({});
```

- [ ] **Step 2: Add command handlers**

Add functions:

```js
async function openRemoteSourceDialog(skill) {
  setRemoteSourceDialog({
    open: true,
    skillName: skill.name,
    sourceUrl: '',
    preview: null,
    error: '',
    loading: false
  });
}

async function previewRemoteSourceBinding(event) {
  event.preventDefault();
  setRemoteSourceDialog((current) => ({ ...current, loading: true, error: '' }));
  try {
    const result = await invoke('preview_remote_source_binding', {
      request: {
        skill_name: remoteSourceDialog.skillName,
        source_url: remoteSourceDialog.sourceUrl,
        actor: 'desktop'
      }
    });
    setRemoteSourceDialog((current) => ({
      ...current,
      preview: normalizeRemoteSourceBindingPreview(result),
      loading: false
    }));
  } catch (previewError) {
    setRemoteSourceDialog((current) => ({
      ...current,
      loading: false,
      error: previewError.message || String(previewError)
    }));
  }
}

async function bindPreviewedRemoteSource() {
  setRemoteSourceDialog((current) => ({ ...current, loading: true, error: '' }));
  try {
    await invoke('bind_remote_source', {
      request: {
        skill_name: remoteSourceDialog.skillName,
        source_url: remoteSourceDialog.sourceUrl,
        actor: 'desktop'
      }
    });
    setRemoteSourceDialog((current) => ({ ...current, open: false, loading: false }));
    await refreshSkillStatuses();
  } catch (bindError) {
    setRemoteSourceDialog((current) => ({
      ...current,
      loading: false,
      error: bindError.message || String(bindError)
    }));
  }
}
```

Add `openRemoteVersionReview` and `applyRemoteVersionChange`:

```js
async function openRemoteVersionReview(skill, action, targetVersion = '') {
  setRemoteVersionDialog({ open: true, loading: true, applying: false, preview: null, activePath: '', error: '' });
  try {
    const result = await invoke('preview_remote_version_change', {
      request: {
        skill_name: skill.name,
        action,
        target_version: targetVersion || null,
        actor: 'desktop'
      }
    });
    const preview = normalizeRemoteVersionPreview(result);
    setRemoteVersionDialog({
      open: true,
      loading: false,
      applying: false,
      preview,
      activePath: preview.activePath,
      error: ''
    });
  } catch (previewError) {
    setRemoteVersionDialog({
      open: true,
      loading: false,
      applying: false,
      preview: null,
      activePath: '',
      error: previewError.message || String(previewError)
    });
  }
}

async function applyRemoteVersionChange() {
  const preview = remoteVersionDialog.preview;
  if (!preview) return;
  setRemoteVersionDialog((current) => ({ ...current, applying: true, error: '' }));
  try {
    await invoke('apply_remote_version_change', {
      request: {
        skill_name: preview.skillName,
        action: preview.action,
        target_version: preview.toVersion,
        preview_id: preview.previewId || null,
        actor: 'desktop'
      }
    });
    setRemoteVersionDialog((current) => ({ ...current, open: false, applying: false }));
    await refreshSkillStatuses();
  } catch (applyError) {
    setRemoteVersionDialog((current) => ({
      ...current,
      applying: false,
      error: applyError.message || String(applyError)
    }));
  }
}
```

- [ ] **Step 3: Extend `SkillDetailDialog` props and actions**

Pass these props:

```jsx
onBindRemoteSource={() => openRemoteSourceDialog(selectedSkill)}
onReviewUpdate={() => openRemoteVersionReview(selectedSkill, 'update')}
onReviewRollback={(version) => openRemoteVersionReview(selectedSkill, 'rollback', version.version)}
remoteUpdate={remoteSkillUpdates.statuses.find((item) => item.skillName === selectedSkill.name)}
versions={remoteVersions[selectedSkill.name] || null}
operations={operationHistory[selectedSkill.name] || []}
```

In `SkillDetailDialog`, add remote-only actions:

```jsx
{skill.type === 'remote' ? (
  <div className="remoteSkillActionStack">
    <button className="button secondary" type="button" onClick={onBindRemoteSource}>
      Bind GitHub source
    </button>
    <button
      className="button primary"
      disabled={!remoteUpdate?.updateAvailable}
      type="button"
      onClick={onReviewUpdate}
    >
      Review update
    </button>
  </div>
) : null}
```

Render versions below actions:

```jsx
{skill.type === 'remote' && versions ? (
  <RemoteVersionsPanel versions={versions} onReviewRollback={onReviewRollback} />
) : null}
```

- [ ] **Step 4: Add dialogs and panels**

Add components in `App.jsx`:

```jsx
function RemoteSourceBindingDialog({ dialog, status, onBind, onClose, onPreview, onUpdate }) {
  const canBind = dialog.preview && dialog.preview.validation !== 'mismatch' && !dialog.loading;
  return (
    <div className="modalBackdrop" role="presentation" onMouseDown={(event) => closeOnBackdropClick(event, onClose)}>
      <section className="remoteImportDialog" role="dialog" aria-modal="true" aria-labelledby="remote-source-title">
        <div className="importSheetHeader">
          <div>
            <h2 id="remote-source-title">Bind GitHub source</h2>
            <p>Link this remote skill to a GitHub source without replacing the current version.</p>
          </div>
          <button className="iconButton" type="button" aria-label="Close source binding" onClick={onClose}>
            <X aria-hidden="true" />
          </button>
        </div>
        <form className="remoteImportForm" onSubmit={onPreview}>
          <label className="remoteImportField">
            <span>GitHub source URL</span>
            <input
              autoFocus
              disabled={dialog.loading}
              value={dialog.sourceUrl}
              onChange={(event) => onUpdate({ sourceUrl: event.target.value })}
            />
          </label>
          {dialog.preview ? (
            <div className="sourceBindingPreview">
              <strong>{dialog.preview.statusLabel}</strong>
              <span>{dialog.preview.message}</span>
            </div>
          ) : null}
          {dialog.error ? <div className="formError">{dialog.error}</div> : null}
          <div className="remoteImportFooter">
            <button className="button secondary" type="button" onClick={onClose}>Cancel</button>
            <button className="button secondary" disabled={dialog.loading} type="submit">
              {dialog.loading ? 'Checking...' : 'Preview'}
            </button>
            <button className="button primary" disabled={!canBind} type="button" onClick={onBind}>
              Bind source
            </button>
          </div>
        </form>
      </section>
    </div>
  );
}
```

Add `RemoteVersionReviewDialog`:

```jsx
function RemoteVersionReviewDialog({ dialog, onActivatePath, onApply, onClose }) {
  const preview = dialog.preview;
  const activeFile =
    preview?.files.find((file) => file.path === dialog.activePath) ||
    preview?.files[0] ||
    null;
  const canApply = canApplyRemoteVersionChange({
    files: preview?.files || [],
    loading: dialog.loading || dialog.applying
  });
  return (
    <div className="modalBackdrop" role="presentation" onMouseDown={(event) => closeOnBackdropClick(event, onClose)}>
      <section className="syncDialog gitCommitDialog" role="dialog" aria-modal="true" aria-labelledby="remote-version-title">
        <div className="importSheetHeader">
          <div>
            <h2 id="remote-version-title">{preview ? remoteVersionActionLabel(preview) : 'Review version change'}</h2>
            <p>{preview ? `${preview.fromVersion} -> ${preview.toVersion}` : 'Loading remote version diff.'}</p>
          </div>
          <button className="iconButton" disabled={dialog.applying} type="button" aria-label="Close version review" onClick={onClose}>
            <X aria-hidden="true" />
          </button>
        </div>
        {dialog.loading ? <div className="gitEmptyState">Loading diff...</div> : null}
        {preview ? (
          <div className="gitCommitReview">
            <aside className="gitFilePane">
              <div className="gitFilePaneHeader">
                <strong>{preview.files.length} files</strong>
              </div>
              <div className="gitFileList">
                {preview.files.map((file) => (
                  <button
                    className={activeFile?.path === file.path ? 'gitFileRow active' : 'gitFileRow'}
                    key={file.path}
                    type="button"
                    onClick={() => onActivatePath(file.path)}
                  >
                    <span>
                      <strong>{file.path}</strong>
                      <small>{file.label}</small>
                    </span>
                  </button>
                ))}
              </div>
            </aside>
            <section className="gitDiffPane" aria-label="Remote version diff">
              <div className="gitDiffHeader">
                <strong>{activeFile?.path || 'Diff'}</strong>
                {activeFile ? <span>{activeFile.label}</span> : null}
              </div>
              {activeFile?.binary || activeFile?.tooLarge ? (
                <div className="gitDiffEmpty">
                  {activeFile.oldHash || 'new'} -> {activeFile.newHash || 'deleted'}
                </div>
              ) : (
                <GitDiffView diff={activeFile?.diff || ''} />
              )}
            </section>
          </div>
        ) : null}
        {dialog.error ? <div className="formError">{dialog.error}</div> : null}
        <div className="remoteImportFooter">
          <button className="button secondary" disabled={dialog.applying} type="button" onClick={onClose}>Cancel</button>
          <button className="button primary" disabled={!canApply} type="button" onClick={onApply}>
            {dialog.applying ? 'Applying...' : 'Apply change'}
          </button>
        </div>
      </section>
    </div>
  );
}
```

- [ ] **Step 5: Add styles**

In `styles.css`, add classes:

```css
.remoteSkillActionStack {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
}

.sourceBindingPreview,
.remoteVersionSummary,
.operationHistoryPanel {
  border: 1px solid #e5e7eb;
  border-radius: 8px;
  padding: 12px;
  background: #f8fafc;
}

.remoteVersionList {
  display: grid;
  gap: 8px;
}

.remoteVersionRow {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: 8px;
  align-items: center;
}
```

- [ ] **Step 6: Update docs after UI wiring**

Update:

- `docs/architecture.md`: Rust core now owns source binding, version preview/apply, and operation log; React only bridges typed commands.
- `docs/workflows.md`: add CLI/Tauri entries and manual verification commands.
- `docs/implementation-status.md`: mark remote source binding, diff preview, update/rollback, and operation log as implemented after Step 7 verification passes.
- `CONTRIBUTING.md`: add remote update/rollback verification commands to the workflow examples.

- [ ] **Step 7: Run full verification**

Run:

```bash
cargo test --offline
npm test
cargo check -p skillbox-desktop --offline
```

Expected: PASS.

- [ ] **Step 8: Run focused manual verification**

Run a temp-root binding/update/rollback smoke flow with local bare Git repositories:

```bash
cargo run -p skillbox-cli --offline -- check-remote-updates --managed-root /tmp/skillbox-remote-mvp
cargo run -p skillbox-cli --offline -- operations --managed-root /tmp/skillbox-remote-mvp
```

Expected: JSON responses, no panic, and operation list available even when empty.

- [ ] **Step 9: Start desktop app for visual verification**

Run:

```bash
npm --workspace apps/desktop run dev
```

Expected: Vite serves the desktop frontend on `http://127.0.0.1:1420`. Use the in-app browser to verify:

- Remote skill detail shows `Bind GitHub source`.
- Changed-source binding preview says current version will stay active.
- Review update opens a full diff dialog.
- Rollback from the versions list opens the same diff dialog.
- Pinned sources show `Pinned`.
- Operation history displays failed records.

- [ ] **Step 10: Commit**

```bash
git add apps/desktop/src/App.jsx apps/desktop/src/styles.css docs/architecture.md docs/workflows.md docs/implementation-status.md CONTRIBUTING.md
git commit -m "feat(desktop): add remote skill update review"
```

## Final Verification Before Merge

- [ ] Run `cargo test --offline`.
- [ ] Run `npm test`.
- [ ] Run `cargo check -p skillbox-desktop --offline`.
- [ ] Run `git status --short` and verify only intended files are present.
- [ ] Review `docs/superpowers/specs/2026-05-27-remote-skill-updates-design.md` against this plan and confirm each requirement has a task:
  - GitHub-only source provider: Tasks 2, 4, 7.
  - Manual binding with exact/same/mismatch: Task 4.
  - Branch/tag/commit display and pinned behavior: Tasks 2, 9, 10.
  - Immutable versions retained permanently: Tasks 5, 6.
  - Update and rollback diff review: Tasks 5, 6, 9, 10.
  - All changed files in diff review: Tasks 3, 5, 9.
  - Runtime symlink follow behavior and pinned deployment detection: Tasks 5, 6.
  - Generic operation log with failures and cancellation: Tasks 1, 4, 6, 8, 10.
  - Docs and verification updates: Task 10.
