use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub type Result<T> = std::result::Result<T, String>;

const DEFAULT_STATUS_REFRESH_INTERVAL_MINUTES: u32 = 5;
const MIN_STATUS_REFRESH_INTERVAL_MINUTES: u32 = 1;
const MAX_STATUS_REFRESH_INTERVAL_MINUTES: u32 = 1440;
const DEFAULT_REMOTE_UPDATE_TIMEOUT_SECONDS: u32 = 30;
const MIN_REMOTE_UPDATE_TIMEOUT_SECONDS: u32 = 5;
const MAX_REMOTE_UPDATE_TIMEOUT_SECONDS: u32 = 300;
const REMOTE_UPDATE_CHECK_CONCURRENCY: usize = 3;
const CLAUDE_MARKETPLACE_SKILLS_API: &str = "https://claudemarketplaces.com/api/skills";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ManagedPaths {
    pub root: PathBuf,
    pub user_skills_root: PathBuf,
    pub remote_skills_root: PathBuf,
    pub database_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SkillMetadata {
    pub name: String,
    pub description: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Skill {
    pub name: String,
    pub description: String,
    pub version: String,
    pub path: PathBuf,
    pub skill_md_path: PathBuf,
    pub content_hash: String,
    pub source_root: Option<PathBuf>,
    pub is_symlink: bool,
    pub real_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ScanError {
    pub root: PathBuf,
    pub path: Option<PathBuf>,
    pub error: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ScanResult {
    pub roots: Vec<PathBuf>,
    pub skills: Vec<Skill>,
    pub errors: Vec<ScanError>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SkillKind {
    User,
    Remote,
}

impl SkillKind {
    pub fn as_str(self) -> &'static str {
        match self {
            SkillKind::User => "user",
            SkillKind::Remote => "remote",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ImportedSkill {
    pub name: String,
    pub kind: SkillKind,
    pub managed_path: PathBuf,
    pub content_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Deployment {
    pub skill_name: String,
    pub managed_path: PathBuf,
    pub target_root: PathBuf,
    pub target_path: PathBuf,
    pub mode: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ManagedSkillDeployment {
    pub target_root: PathBuf,
    pub target_path: PathBuf,
    pub mode: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ManagedSkill {
    pub name: String,
    pub description: String,
    pub version: String,
    pub path: PathBuf,
    pub skill_md_path: PathBuf,
    pub content_hash: String,
    pub source_root: Option<PathBuf>,
    pub is_symlink: bool,
    pub real_path: PathBuf,
    #[serde(rename = "type")]
    pub kind: SkillKind,
    pub status: String,
    pub deployments: Vec<ManagedSkillDeployment>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ManagedState {
    pub paths: ManagedPaths,
    pub skills: Vec<ManagedSkill>,
    pub is_first_use: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ManagedPreferences {
    pub skip_local_import_confirmation: bool,
    pub status_refresh_interval_minutes: u32,
    pub remote_update_timeout_seconds: u32,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WorkspaceKind {
    Global,
    User,
}

impl WorkspaceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            WorkspaceKind::Global => "global",
            WorkspaceKind::User => "user",
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WorkspaceSource {
    Auto,
    Manual,
}

impl WorkspaceSource {
    pub fn as_str(self) -> &'static str {
        match self {
            WorkspaceSource::Auto => "auto",
            WorkspaceSource::Manual => "manual",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Workspace {
    pub canonical_path: PathBuf,
    pub path: PathBuf,
    pub kind: WorkspaceKind,
    pub source: WorkspaceSource,
    pub agent_id: Option<String>,
    pub display_name: String,
    pub skill_count: usize,
    pub imported_skill_count: usize,
    pub last_scan_error_count: usize,
    pub last_scan_error: Option<String>,
    pub last_scanned_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceAddRequest {
    pub path: PathBuf,
    pub kind: WorkspaceKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WorkspaceScanResult {
    pub workspaces: Vec<Workspace>,
    pub scanned_count: usize,
    pub error_count: usize,
}

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
    pub changed_paths: Vec<String>,
    pub state: UserSkillsGitState,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UserSkillsGitChangeFile {
    pub path: String,
    pub status: String,
    pub diff: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UserSkillsGitChanges {
    pub repo_path: PathBuf,
    pub initialized: bool,
    pub branch: String,
    pub remote_url: Option<String>,
    pub files: Vec<UserSkillsGitChangeFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UserSkillVersion {
    pub version: String,
    pub is_current: bool,
    pub kind: String,
    pub short_label: String,
    pub updated_at: String,
    pub message: Option<String>,
    pub path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UserSkillVersionList {
    pub skill_name: String,
    pub current_version: String,
    pub versions: Vec<UserSkillVersion>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserSkillsSyncRequest {
    pub remote_url: Option<String>,
    pub commit_message: Option<String>,
    pub push: bool,
    pub selected_paths: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserSkillsGitRemoteRequest {
    pub remote_url: String,
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

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteSkillUpdateState {
    NoSource,
    NotCheckable,
    UpToDate,
    UpdateAvailable,
    CheckFailed,
    Pinned,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteSkillUpdateStatus {
    pub skill_name: String,
    pub source_type: Option<String>,
    pub source_url: Option<String>,
    pub current_version: Option<String>,
    pub installed_sha: Option<String>,
    pub latest_sha: Option<String>,
    pub ref_kind: Option<String>,
    pub tracking: bool,
    pub update_available: bool,
    pub state: RemoteSkillUpdateState,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteSkillUpdateCheck {
    pub checked_at: Option<String>,
    pub statuses: Vec<RemoteSkillUpdateStatus>,
}

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
    pub updated_at: String,
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

#[derive(Debug, Clone, Deserialize)]
struct ClaudeMarketplaceSkill {
    name: Option<String>,
    description: Option<String>,
    repo: Option<String>,
    path: Option<String>,
    stars: Option<u64>,
    installs: Option<u64>,
    #[serde(rename = "lastUpdated", alias = "last_updated")]
    last_updated: Option<String>,
    #[serde(rename = "listingStatus", alias = "listing_status")]
    listing_status: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct RemoteSkillSource {
    #[serde(rename = "type")]
    source_type: String,
    #[serde(rename = "url", alias = "sourceUrl", alias = "source_url")]
    source_url: Option<String>,
    path: Option<String>,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ImportCandidate {
    pub name: String,
    pub description: String,
    pub source_path: PathBuf,
    pub source_root: Option<PathBuf>,
    pub real_path: PathBuf,
    pub content_hash: String,
    pub suggested_type: SkillKind,
    pub suggestion_reason: String,
    pub import_status: ImportCandidateStatus,
    pub is_selected: bool,
    pub conflict: Option<String>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImportCandidateStatus {
    Importable,
    Imported,
    System,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ImportCandidateScan {
    pub roots: Vec<PathBuf>,
    pub candidates: Vec<ImportCandidate>,
    pub errors: Vec<ScanError>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportRequestItem {
    pub source_path: PathBuf,
    pub skill_type: SkillKind,
    pub deploy_back_to_source: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ImportedCandidate {
    pub name: String,
    pub kind: SkillKind,
    pub source_path: PathBuf,
    pub managed_path: PathBuf,
    pub content_hash: String,
    pub backup_path: Option<PathBuf>,
    pub deployed_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ImportCandidateError {
    pub source_path: PathBuf,
    pub error: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ImportBatchResult {
    pub imported: Vec<ImportedCandidate>,
    pub errors: Vec<ImportCandidateError>,
}

pub fn default_managed_root() -> PathBuf {
    std::env::var_os("SKILLBOX_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(default_hidden_managed_root)
}

fn default_hidden_managed_root() -> PathBuf {
    home_dir().join(".skillbox")
}

fn legacy_managed_root() -> PathBuf {
    home_dir().join("SkillBox")
}

pub fn default_runtime_roots() -> Vec<PathBuf> {
    vec![
        home_dir().join(".codex/skills"),
        home_dir().join(".agents/skills"),
        home_dir().join(".claude/skills"),
    ]
}

pub fn global_runtime_roots() -> Vec<PathBuf> {
    runtime_roots_under(&home_dir())
}

fn runtime_roots_under(home: &Path) -> Vec<PathBuf> {
    let mut roots = vec![
        home.join(".codex/skills"),
        home.join(".agents/skills"),
        home.join(".claude/skills"),
    ];
    roots.extend(discover_runtime_roots_under(home));
    dedupe_runtime_roots(roots)
}

fn discover_runtime_roots_under(home: &Path) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    discover_runtime_roots(home, 0, 3, &mut roots);
    for base in runtime_root_search_bases(home) {
        discover_runtime_roots(&base, 0, 8, &mut roots);
    }
    dedupe_runtime_roots(roots)
}

fn runtime_root_search_bases(home: &Path) -> Vec<PathBuf> {
    [
        "Desktop",
        "Documents",
        "Downloads",
        "Developer",
        "Projects",
        "Code",
        "code",
        "zone",
        "work",
        "src",
        "Library/Mobile Documents",
    ]
    .iter()
    .map(|relative| home.join(relative))
    .collect()
}

fn discover_runtime_roots(
    current: &Path,
    depth: usize,
    max_depth: usize,
    roots: &mut Vec<PathBuf>,
) {
    if depth > max_depth || !current.is_dir() {
        return;
    }

    if is_runtime_skill_root(current) {
        roots.push(current.to_path_buf());
        return;
    }

    let mut has_direct_runtime_root = false;
    for runtime_parent in [".agents", ".codex", ".claude"] {
        let runtime_root = current.join(runtime_parent).join("skills");
        if runtime_root.is_dir() {
            roots.push(runtime_root);
            has_direct_runtime_root = true;
        }
    }
    if depth > 0 && has_direct_runtime_root {
        return;
    }

    let entries = match fs::read_dir(current) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(_) => continue,
        };
        if !file_type.is_dir() {
            continue;
        }

        if should_skip_runtime_root_search(&path) {
            continue;
        }

        discover_runtime_roots(&path, depth + 1, max_depth, roots);
    }
}

fn is_runtime_skill_root(path: &Path) -> bool {
    path.file_name().and_then(|name| name.to_str()) == Some("skills")
        && matches!(
            path.parent()
                .and_then(|parent| parent.file_name())
                .and_then(|name| name.to_str()),
            Some(".agents" | ".codex" | ".claude")
        )
}

fn should_skip_runtime_root_search(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    if matches!(name, ".agents" | ".codex" | ".claude") {
        return false;
    }
    if name.starts_with('.') {
        return true;
    }
    if matches!(
        name,
        "node_modules"
            | "target"
            | "dist"
            | "build"
            | ".venv"
            | "venv"
            | "SkillBox"
            | "Applications"
            | "Pictures"
            | "Movies"
            | "Music"
            | "Caches"
    ) {
        return true;
    }

    let parent_name = path
        .parent()
        .and_then(|parent| parent.file_name())
        .and_then(|name| name.to_str());
    parent_name == Some("Library") && name != "Mobile Documents"
}

fn dedupe_runtime_roots(roots: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    let mut deduped = Vec::new();

    for root in roots {
        let key = fs::canonicalize(&root).unwrap_or_else(|_| root.clone());
        if seen.insert(key) {
            deduped.push(root);
        }
    }

    deduped
}

pub fn managed_paths(root: impl Into<PathBuf>) -> ManagedPaths {
    let root = expand_home(root.into());
    ManagedPaths {
        user_skills_root: root.join("user-skills"),
        remote_skills_root: root.join("remote-skills"),
        database_path: root.join("skillbox.sqlite"),
        root,
    }
}

pub fn ensure_managed_layout(root: impl Into<PathBuf>) -> Result<ManagedPaths> {
    let root = expand_home(root.into());
    maybe_link_legacy_default_managed_root(&root)?;
    let paths = managed_paths(root);
    fs::create_dir_all(&paths.user_skills_root).map_err(|error| error.to_string())?;
    fs::create_dir_all(&paths.remote_skills_root).map_err(|error| error.to_string())?;
    init_database(&paths.database_path)?;
    Ok(paths)
}

fn maybe_link_legacy_default_managed_root(root: &Path) -> Result<()> {
    if std::env::var_os("SKILLBOX_HOME").is_some() || root != default_hidden_managed_root() {
        return Ok(());
    }
    link_legacy_managed_root_if_needed(root, &legacy_managed_root()).map(|_| ())
}

fn link_legacy_managed_root_if_needed(hidden_root: &Path, legacy_root: &Path) -> Result<bool> {
    if hidden_root == legacy_root || !managed_root_has_content(legacy_root)? {
        return Ok(false);
    }

    match fs::symlink_metadata(hidden_root) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() {
                return Ok(false);
            }
            if !is_empty_managed_stub(hidden_root)? {
                return Ok(false);
            }
            let backup_path = next_empty_root_backup_path(hidden_root);
            fs::rename(hidden_root, &backup_path).map_err(|error| {
                format!(
                    "Failed to back up empty managed root {} to {}: {error}",
                    hidden_root.display(),
                    backup_path.display()
                )
            })?;
            if let Err(error) = symlink_dir(legacy_root, hidden_root) {
                let _ = fs::rename(&backup_path, hidden_root);
                return Err(format!(
                    "Failed to link {} to legacy SkillBox root {}: {error}",
                    hidden_root.display(),
                    legacy_root.display()
                ));
            }
            Ok(true)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            if let Some(parent) = hidden_root.parent() {
                fs::create_dir_all(parent).map_err(|error| error.to_string())?;
            }
            symlink_dir(legacy_root, hidden_root).map_err(|error| {
                format!(
                    "Failed to link {} to legacy SkillBox root {}: {error}",
                    hidden_root.display(),
                    legacy_root.display()
                )
            })?;
            Ok(true)
        }
        Err(error) => Err(error.to_string()),
    }
}

fn managed_root_has_content(root: &Path) -> Result<bool> {
    if !root.is_dir() {
        return Ok(false);
    }
    Ok(directory_has_entries(&root.join("user-skills"))?
        || directory_has_entries(&root.join("remote-skills"))?
        || directory_has_entries(&root.join("backups"))?)
}

fn is_empty_managed_stub(root: &Path) -> Result<bool> {
    if !root.is_dir() {
        return Ok(false);
    }
    for entry in fs::read_dir(root).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        let file_type = entry.file_type().map_err(|error| error.to_string())?;
        match name.as_str() {
            "user-skills" | "remote-skills" => {
                if !file_type.is_dir() || directory_has_entries(&path)? {
                    return Ok(false);
                }
            }
            "skillbox.sqlite" => {
                if !file_type.is_file() {
                    return Ok(false);
                }
            }
            _ => return Ok(false),
        }
    }
    Ok(true)
}

fn directory_has_entries(path: &Path) -> Result<bool> {
    if !path.exists() {
        return Ok(false);
    }
    if !path.is_dir() {
        return Ok(true);
    }
    Ok(fs::read_dir(path)
        .map_err(|error| error.to_string())?
        .next()
        .is_some())
}

fn next_empty_root_backup_path(root: &Path) -> PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_millis();
    let name = root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("skillbox");
    for attempt in 0..100 {
        let suffix = if attempt == 0 {
            format!("{name}.empty-backup-{timestamp}")
        } else {
            format!("{name}.empty-backup-{timestamp}-{attempt}")
        };
        let candidate = root.with_file_name(suffix);
        if !candidate.exists() {
            return candidate;
        }
    }
    root.with_file_name(format!("{name}.empty-backup-{timestamp}-fallback"))
}

pub fn parse_skill_frontmatter(input: &str) -> SkillMetadata {
    let mut metadata = SkillMetadata {
        name: String::new(),
        description: String::new(),
        version: String::new(),
    };
    let mut lines = input.lines();
    if lines.next() != Some("---") {
        return metadata;
    }

    for line in lines {
        if line == "---" {
            break;
        }
        if line.starts_with(' ') || line.starts_with('\t') {
            continue;
        }
        if let Some((key, value)) = line.split_once(':') {
            let value = unquote(value.trim());
            match key.trim() {
                "name" => metadata.name = value,
                "description" => metadata.description = value,
                "version" => metadata.version = value,
                _ => {}
            }
        }
    }

    metadata
}

pub fn read_skill(path: impl AsRef<Path>) -> Result<Skill> {
    let path = path.as_ref().to_path_buf();
    let skill_md_path = path.join("SKILL.md");
    if !skill_md_path.exists() {
        return Err(format!("SKILL.md not found in {}", path.display()));
    }

    let content = fs::read_to_string(&skill_md_path).map_err(|error| error.to_string())?;
    let metadata = parse_skill_frontmatter(&content);
    let name = if metadata.name.is_empty() {
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_string()
    } else {
        metadata.name
    };

    Ok(Skill {
        name,
        description: metadata.description,
        version: metadata.version,
        content_hash: sha256(&content),
        real_path: fs::canonicalize(&path).unwrap_or_else(|_| path.clone()),
        path,
        skill_md_path,
        source_root: None,
        is_symlink: false,
    })
}

pub fn scan_skill_roots(roots: &[PathBuf]) -> Result<ScanResult> {
    let mut skills = Vec::new();
    let mut errors = Vec::new();
    let roots: Vec<PathBuf> = roots.iter().cloned().map(expand_home).collect();

    for root in &roots {
        if !root.exists() {
            continue;
        }
        let mut skill_dirs = Vec::new();
        if let Err(error) = find_skill_dirs(root, 0, 3, &mut skill_dirs) {
            errors.push(ScanError {
                root: root.clone(),
                path: None,
                error,
            });
            continue;
        }

        for skill_dir in skill_dirs {
            match read_skill(&skill_dir) {
                Ok(mut skill) => {
                    skill.source_root = Some(root.clone());
                    skill.is_symlink = fs::symlink_metadata(&skill_dir)
                        .map(|metadata| metadata.file_type().is_symlink())
                        .unwrap_or(false);
                    skills.push(skill);
                }
                Err(error) => errors.push(ScanError {
                    root: root.clone(),
                    path: Some(skill_dir),
                    error,
                }),
            }
        }
    }

    skills.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(ScanResult {
        roots,
        skills,
        errors,
    })
}

pub fn import_skill(
    source_dir: impl AsRef<Path>,
    kind: SkillKind,
    managed_root: impl AsRef<Path>,
) -> Result<ImportedSkill> {
    let paths = ensure_managed_layout(managed_root.as_ref().to_path_buf())?;
    let skill = read_skill(source_dir.as_ref())?;
    validate_skill_name(&skill.name)?;

    let managed_path = match kind {
        SkillKind::User => paths.user_skills_root.join(&skill.name),
        SkillKind::Remote => paths
            .remote_skills_root
            .join(&skill.name)
            .join("versions")
            .join(format!("manual-{}", &skill.content_hash[..12])),
    };

    copy_skill_dir(&skill.path, &managed_path)?;
    if kind == SkillKind::Remote {
        update_current_symlink(&paths.remote_skills_root.join(&skill.name), &managed_path)?;
    }

    index_skill(&paths.database_path, &skill, kind, &managed_path)?;
    Ok(ImportedSkill {
        name: skill.name,
        kind,
        managed_path,
        content_hash: skill.content_hash,
    })
}

pub fn deploy_skill(
    skill_name: &str,
    managed_root: impl AsRef<Path>,
    target_root: impl AsRef<Path>,
) -> Result<Deployment> {
    validate_skill_name(skill_name)?;
    let paths = ensure_managed_layout(managed_root.as_ref().to_path_buf())?;
    let managed_path = resolve_managed_skill_path(&paths, skill_name)?;
    let target_root = expand_home(target_root.as_ref().to_path_buf());
    let target_path = target_root.join(skill_name);

    fs::create_dir_all(&target_root).map_err(|error| error.to_string())?;
    let mut should_create_symlink = false;
    if let Ok(metadata) = fs::symlink_metadata(&target_path) {
        if !metadata.file_type().is_symlink() {
            return Err(format!(
                "Refusing to overwrite existing non-symlink target: {}",
                target_path.display()
            ));
        }
        let linked = fs::canonicalize(&target_path).map_err(|error| error.to_string())?;
        let expected = fs::canonicalize(&managed_path).map_err(|error| error.to_string())?;
        if linked != expected {
            return Err(format!(
                "Refusing to replace symlink pointing elsewhere: {}",
                target_path.display()
            ));
        }
        if !symlink_points_to_path(&target_path, &managed_path)? {
            fs::remove_file(&target_path).map_err(|error| error.to_string())?;
            should_create_symlink = true;
        }
    } else {
        should_create_symlink = true;
    }

    if should_create_symlink {
        symlink_dir(&managed_path, &target_path)?;
    }

    index_deployment(&paths.database_path, skill_name, &target_root, &target_path)?;
    Ok(Deployment {
        skill_name: skill_name.to_string(),
        managed_path,
        target_root,
        target_path,
        mode: "symlink".to_string(),
    })
}

pub fn undeploy_skill(
    skill_name: &str,
    managed_root: impl AsRef<Path>,
    target_root: impl AsRef<Path>,
) -> Result<Deployment> {
    validate_skill_name(skill_name)?;
    let paths = ensure_managed_layout(managed_root.as_ref().to_path_buf())?;
    let managed_path = resolve_managed_skill_path(&paths, skill_name)?;
    let target_root = expand_home(target_root.as_ref().to_path_buf());
    let target_path = target_root.join(skill_name);
    let alias_target_paths = workspace_symlink_paths_to_managed_skill(&target_root, &managed_path);
    let mut target_paths_to_remove = Vec::new();

    match fs::symlink_metadata(&target_path) {
        Ok(metadata) => {
            if !metadata.file_type().is_symlink() {
                return Err(format!(
                    "Refusing to remove existing non-symlink target: {}",
                    target_path.display()
                ));
            }

            let linked = fs::canonicalize(&target_path).map_err(|error| error.to_string())?;
            let expected = fs::canonicalize(&managed_path).map_err(|error| error.to_string())?;
            if linked != expected {
                return Err(format!(
                    "Refusing to remove symlink pointing elsewhere: {}",
                    target_path.display()
                ));
            }

            target_paths_to_remove.push(target_path.clone());
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.to_string()),
    }

    for alias_target_path in alias_target_paths {
        if !target_paths_to_remove
            .iter()
            .any(|path| path == &alias_target_path)
        {
            target_paths_to_remove.push(alias_target_path);
        }
    }

    let removed_target_path = target_paths_to_remove
        .first()
        .cloned()
        .unwrap_or_else(|| target_path.clone());
    for target_path_to_remove in target_paths_to_remove {
        fs::remove_file(&target_path_to_remove).map_err(|error| error.to_string())?;
    }

    remove_deployment(&paths.database_path, skill_name, &target_root)?;
    Ok(Deployment {
        skill_name: skill_name.to_string(),
        managed_path,
        target_root,
        target_path: removed_target_path,
        mode: "symlink".to_string(),
    })
}

pub fn managed_state(managed_root: impl AsRef<Path>) -> Result<ManagedState> {
    let paths = ensure_managed_layout(managed_root.as_ref().to_path_buf())?;
    let mut deployments = load_deployments(&paths.database_path)?;
    let mut skills = Vec::new();

    for skill in scan_skill_roots(std::slice::from_ref(&paths.user_skills_root))?.skills {
        skills.push(managed_skill(skill, SkillKind::User));
    }
    skills.extend(scan_managed_remote_skills(&paths)?);
    let workspaces = load_workspaces(&paths.database_path)?;
    merge_workspace_symlink_deployments(&workspaces, &skills, &mut deployments);

    for skill in skills.iter_mut() {
        skill.deployments = deployments.get(&skill.name).cloned().unwrap_or_default();
    }

    skills.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(ManagedState {
        is_first_use: skills.is_empty(),
        paths,
        skills,
    })
}

fn scan_managed_remote_skills(paths: &ManagedPaths) -> Result<Vec<ManagedSkill>> {
    let mut skills = Vec::new();
    if !paths.remote_skills_root.exists() {
        return Ok(skills);
    }

    for entry in fs::read_dir(&paths.remote_skills_root).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let remote_root = entry.path();
        let current = remote_root.join("current");
        if !current.join("SKILL.md").exists() {
            continue;
        }

        if let Ok(mut skill) = read_skill(&current) {
            skill.source_root = Some(paths.remote_skills_root.clone());
            skill.is_symlink = fs::symlink_metadata(&current)
                .map(|metadata| metadata.file_type().is_symlink())
                .unwrap_or(false);
            skills.push(managed_skill(skill, SkillKind::Remote));
        }
    }

    Ok(skills)
}

fn merge_workspace_symlink_deployments(
    workspaces: &[Workspace],
    skills: &[ManagedSkill],
    deployments: &mut HashMap<String, Vec<ManagedSkillDeployment>>,
) {
    for skill in skills {
        for workspace in workspaces {
            let exact_target_path = workspace.path.join(&skill.name);
            let target_path =
                if workspace_target_is_current_symlink(&exact_target_path, &skill.path) {
                    Some(exact_target_path)
                } else {
                    workspace_symlink_paths_to_managed_skill(&workspace.path, &skill.path)
                        .into_iter()
                        .next()
                };
            let Some(target_path) = target_path else {
                continue;
            };

            let skill_deployments = deployments.entry(skill.name.clone()).or_default();
            if skill_deployments
                .iter()
                .any(|deployment| deployment.target_root == workspace.path)
            {
                continue;
            }

            skill_deployments.push(ManagedSkillDeployment {
                target_root: workspace.path.clone(),
                target_path,
                mode: "symlink".to_string(),
            });
        }
    }
}

fn workspace_symlink_paths_to_managed_skill(
    workspace_path: &Path,
    managed_path: &Path,
) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(workspace_path) else {
        return Vec::new();
    };

    let mut target_paths = entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| workspace_target_is_current_symlink(path, managed_path))
        .collect::<Vec<_>>();
    target_paths.sort();
    target_paths
}

fn workspace_target_is_current_symlink(target_path: &Path, managed_path: &Path) -> bool {
    let Ok(metadata) = fs::symlink_metadata(target_path) else {
        return false;
    };
    if !metadata.file_type().is_symlink() {
        return false;
    }

    match (
        fs::canonicalize(target_path),
        fs::canonicalize(managed_path),
    ) {
        (Ok(target), Ok(expected)) => target == expected,
        _ => false,
    }
}

pub fn managed_preferences(managed_root: impl AsRef<Path>) -> Result<ManagedPreferences> {
    let paths = ensure_managed_layout(managed_root.as_ref().to_path_buf())?;
    let skip_local_import_confirmation =
        read_bool_preference(&paths.database_path, "skip_local_import_confirmation")?
            .unwrap_or(false);
    let status_refresh_interval_minutes =
        read_u32_preference(&paths.database_path, "status_refresh_interval_minutes")?
            .unwrap_or(DEFAULT_STATUS_REFRESH_INTERVAL_MINUTES);
    let remote_update_timeout_seconds =
        read_u32_preference(&paths.database_path, "remote_update_timeout_seconds")?
            .unwrap_or(DEFAULT_REMOTE_UPDATE_TIMEOUT_SECONDS);

    Ok(ManagedPreferences {
        skip_local_import_confirmation,
        status_refresh_interval_minutes,
        remote_update_timeout_seconds,
    })
}

pub fn set_skip_local_import_confirmation(
    managed_root: impl AsRef<Path>,
    skip: bool,
) -> Result<ManagedPreferences> {
    let paths = ensure_managed_layout(managed_root.as_ref().to_path_buf())?;
    write_bool_preference(&paths.database_path, "skip_local_import_confirmation", skip)?;

    managed_preferences(paths.root)
}

pub fn set_status_refresh_interval_minutes(
    managed_root: impl AsRef<Path>,
    minutes: u32,
) -> Result<ManagedPreferences> {
    if !(MIN_STATUS_REFRESH_INTERVAL_MINUTES..=MAX_STATUS_REFRESH_INTERVAL_MINUTES)
        .contains(&minutes)
    {
        return Err(format!(
            "Status refresh interval must be between {MIN_STATUS_REFRESH_INTERVAL_MINUTES} and {MAX_STATUS_REFRESH_INTERVAL_MINUTES} minutes."
        ));
    }

    let paths = ensure_managed_layout(managed_root.as_ref().to_path_buf())?;
    write_u32_preference(
        &paths.database_path,
        "status_refresh_interval_minutes",
        minutes,
    )?;

    managed_preferences(paths.root)
}

pub fn set_remote_update_timeout_seconds(
    managed_root: impl AsRef<Path>,
    seconds: u32,
) -> Result<ManagedPreferences> {
    validate_remote_update_timeout_seconds(seconds)?;

    let paths = ensure_managed_layout(managed_root.as_ref().to_path_buf())?;
    write_u32_preference(
        &paths.database_path,
        "remote_update_timeout_seconds",
        seconds,
    )?;

    managed_preferences(paths.root)
}

pub fn list_workspaces(managed_root: impl AsRef<Path>) -> Result<Vec<Workspace>> {
    let paths = ensure_managed_layout(managed_root.as_ref().to_path_buf())?;
    load_workspaces(&paths.database_path)
}

pub fn scan_workspaces(managed_root: impl AsRef<Path>) -> Result<WorkspaceScanResult> {
    scan_workspaces_under(&home_dir(), managed_root)
}

fn scan_workspaces_under(
    home: &Path,
    managed_root: impl AsRef<Path>,
) -> Result<WorkspaceScanResult> {
    let paths = ensure_managed_layout(managed_root.as_ref().to_path_buf())?;
    let roots = runtime_roots_under(home)
        .into_iter()
        .filter(|root| workspace_root_is_readable(root))
        .collect::<Vec<_>>();
    let mut scanned_count = 0;
    let mut error_count = 0;

    for root in roots {
        let kind = infer_workspace_kind(&root, home);
        let workspace = upsert_workspace(&paths, &root, kind, WorkspaceSource::Auto)?;
        scanned_count += 1;
        error_count += workspace.last_scan_error_count;
    }

    Ok(WorkspaceScanResult {
        workspaces: load_workspaces(&paths.database_path)?,
        scanned_count,
        error_count,
    })
}

pub fn add_workspace(
    request: WorkspaceAddRequest,
    managed_root: impl AsRef<Path>,
) -> Result<Workspace> {
    let paths = ensure_managed_layout(managed_root.as_ref().to_path_buf())?;
    let workspace_path = expand_home(request.path);

    if !workspace_path.exists() {
        return Err(format!(
            "Workspace path does not exist: {}",
            workspace_path.display()
        ));
    }
    if !workspace_path.is_dir() {
        return Err(format!(
            "Workspace path is not a directory: {}",
            workspace_path.display()
        ));
    }

    upsert_workspace(
        &paths,
        &workspace_path,
        request.kind,
        WorkspaceSource::Manual,
    )
}

pub fn forget_workspace(
    path: impl AsRef<Path>,
    managed_root: impl AsRef<Path>,
) -> Result<Vec<Workspace>> {
    let paths = ensure_managed_layout(managed_root.as_ref().to_path_buf())?;
    let workspace_path = expand_home(path.as_ref().to_path_buf());
    let canonical_path = fs::canonicalize(&workspace_path).map_err(|error| {
        format!(
            "Workspace path cannot be resolved: {} ({error})",
            workspace_path.display()
        )
    })?;
    let existing = load_workspace_by_canonical_path(&paths.database_path, &canonical_path)?
        .ok_or_else(|| format!("Workspace is not registered: {}", workspace_path.display()))?;

    if existing.source != WorkspaceSource::Manual {
        return Err("Only manually added workspaces can be forgotten.".to_string());
    }

    let connection = open_database(&paths.database_path).map_err(|error| error.to_string())?;
    connection
        .execute(
            "DELETE FROM workspaces WHERE canonical_path = ?1 AND source = 'manual'",
            params![canonical_path.to_string_lossy()],
        )
        .map_err(|error| error.to_string())?;

    load_workspaces(&paths.database_path)
}

pub fn start_operation(
    request: OperationStart,
    managed_root: impl AsRef<Path>,
) -> Result<OperationRecord> {
    let paths = ensure_managed_layout(managed_root.as_ref().to_path_buf())?;
    let id = operation_id();
    let started_at = operation_timestamp();
    let payload_json =
        serde_json::to_string(&request.payload).map_err(|error| error.to_string())?;
    let connection = open_database(&paths.database_path).map_err(|error| error.to_string())?;

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
                started_at,
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
    let id = request.id.clone();
    let finished_at = operation_timestamp();
    let payload_json =
        serde_json::to_string(&request.payload).map_err(|error| error.to_string())?;
    let connection = open_database(&paths.database_path).map_err(|error| error.to_string())?;

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
                id,
                request.status.as_str(),
                finished_at,
                request.summary,
                request.error,
                payload_json
            ],
        )
        .map_err(|error| error.to_string())?;

    load_operation(&connection, &id)
}

pub fn list_operations(
    filter: OperationFilter,
    managed_root: impl AsRef<Path>,
) -> Result<OperationList> {
    let paths = ensure_managed_layout(managed_root.as_ref().to_path_buf())?;
    let connection = open_database(&paths.database_path).map_err(|error| error.to_string())?;
    let limit = i64::from(filter.limit.unwrap_or(50).clamp(1, 500));
    let status = filter.status.map(OperationStatus::as_str);
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

pub fn user_skills_git_status(managed_root: impl AsRef<Path>) -> Result<UserSkillsGitStatus> {
    let paths = ensure_managed_layout(managed_root.as_ref().to_path_buf())?;
    user_skills_git_status_for_repo(paths.user_skills_root)
}

pub fn user_skills_git_changes(managed_root: impl AsRef<Path>) -> Result<UserSkillsGitChanges> {
    let paths = ensure_managed_layout(managed_root.as_ref().to_path_buf())?;
    let repo = paths.user_skills_root;
    let status = user_skills_git_status_for_repo(repo.clone())?;
    let files = if status.initialized {
        skillbox_git::changed_files(&repo)?
            .into_iter()
            .map(|file| {
                let diff = if file.status == "??" || !skillbox_git::has_head(&repo) {
                    new_file_diff(&repo, &file.path)
                } else {
                    skillbox_git::diff_head_path(&repo, &file.path)
                }?;

                Ok(UserSkillsGitChangeFile {
                    path: file.path,
                    status: file.status,
                    diff,
                })
            })
            .collect::<Result<Vec<_>>>()?
    } else {
        collect_user_skill_files(&repo)?
            .into_iter()
            .map(|path| {
                let diff = new_file_diff(&repo, &path)?;
                Ok(UserSkillsGitChangeFile {
                    path,
                    status: "??".to_string(),
                    diff,
                })
            })
            .collect::<Result<Vec<_>>>()?
    };

    Ok(UserSkillsGitChanges {
        repo_path: status.repo_path,
        initialized: status.initialized,
        branch: status.branch,
        remote_url: status.remote_url,
        files,
    })
}

pub fn list_user_skill_versions(
    skill_name: &str,
    managed_root: impl AsRef<Path>,
) -> Result<UserSkillVersionList> {
    validate_skill_name(skill_name)?;
    let paths = ensure_managed_layout(managed_root.as_ref().to_path_buf())?;
    let repo = paths.user_skills_root;
    let skill_path = repo.join(skill_name);
    let skill = read_skill(&skill_path)?;
    if skill.name != skill_name {
        return Err(format!(
            "User skill name does not match requested skill: {}",
            skill.name
        ));
    }

    let log_entries = skillbox_git::log_path(&repo, skill_name, 20)?;
    let has_uncommitted_skill_changes = user_skill_has_uncommitted_changes(&repo, skill_name)?;
    let mut versions = Vec::new();
    let current_version = if !has_uncommitted_skill_changes {
        log_entries
            .first()
            .map(|entry| entry.sha.clone())
            .unwrap_or_else(|| skill.content_hash.clone())
    } else {
        skill.content_hash.clone()
    };

    if has_uncommitted_skill_changes || log_entries.is_empty() {
        versions.push(UserSkillVersion {
            version: skill.content_hash.clone(),
            is_current: true,
            kind: "working".to_string(),
            short_label: short_version_label(&skill.content_hash),
            updated_at: file_modified_timestamp(&skill.skill_md_path),
            message: None,
            path: skill.path.clone(),
        });
    }

    for entry in log_entries {
        let is_current = !has_uncommitted_skill_changes && entry.sha == current_version;
        versions.push(UserSkillVersion {
            short_label: short_version_label(&entry.sha),
            kind: "git".to_string(),
            is_current,
            updated_at: entry.timestamp,
            message: Some(entry.subject),
            path: skill.path.clone(),
            version: entry.sha,
        });
    }

    Ok(UserSkillVersionList {
        skill_name: skill_name.to_string(),
        current_version,
        versions,
    })
}

pub fn set_user_skills_git_remote(
    request: UserSkillsGitRemoteRequest,
    managed_root: impl AsRef<Path>,
) -> Result<UserSkillsGitStatus> {
    let remote_url = request.remote_url.trim();
    if remote_url.is_empty() {
        return Err("Git remote URL cannot be empty.".to_string());
    }
    if remote_url.chars().any(char::is_whitespace) {
        return Err("Git remote URL cannot contain whitespace.".to_string());
    }

    let paths = ensure_managed_layout(managed_root.as_ref().to_path_buf())?;
    let repo = paths.user_skills_root;
    if !skillbox_git::status(&repo)?.initialized {
        skillbox_git::init_main(&repo)?;
    }
    skillbox_git::set_origin_url(&repo, remote_url)?;
    user_skills_git_status_for_repo(repo)
}

pub fn sync_user_skills_git(
    request: UserSkillsSyncRequest,
    managed_root: impl AsRef<Path>,
) -> Result<UserSkillsSyncResult> {
    let paths = ensure_managed_layout(managed_root.as_ref().to_path_buf())?;
    let repo = paths.user_skills_root;
    let before = skillbox_git::status(&repo)?;
    let initialized = !before.initialized;

    if initialized {
        skillbox_git::init_main(&repo)?;
    }

    let mut remote_updated = false;
    let current_remote = if repo.join(".git").exists() {
        skillbox_git::origin_url(&repo)?
    } else {
        None
    };
    let requested_remote = request
        .remote_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());

    if let Some(remote_url) = requested_remote {
        if current_remote.as_deref() != Some(remote_url) {
            skillbox_git::set_origin_url(&repo, remote_url)?;
            remote_updated = true;
        }
    } else if request
        .remote_url
        .as_deref()
        .is_some_and(|value| value.trim().is_empty())
    {
        return Err("Git remote URL cannot be empty.".to_string());
    }

    if request.push && skillbox_git::origin_url(&repo)?.is_none() {
        return Err("Git remote URL is required before syncing user skills.".to_string());
    }

    if let Some(paths) = &request.selected_paths {
        let selected_paths = validate_git_relative_paths(paths)?;
        skillbox_git::add_paths(&repo, &selected_paths)?;
    } else {
        skillbox_git::add_all(&repo)?;
    }
    let has_staged_changes = skillbox_git::staged_changes(&repo)?;
    let commit_message = normalized_commit_message(request.commit_message.as_deref());
    let commit_sha = if has_staged_changes {
        Some(skillbox_git::commit(&repo, &commit_message)?)
    } else {
        None
    };
    let committed = commit_sha.is_some();
    let mut pushed = false;
    let mut state_override = None;
    let mut message = if committed {
        "Committed user skills.".to_string()
    } else {
        "Already synced.".to_string()
    };

    if request.push {
        match skillbox_git::push_origin_main(&repo, true) {
            Ok(()) => {
                pushed = true;
                message = if committed {
                    "Synced user skills.".to_string()
                } else {
                    "Already synced.".to_string()
                };
            }
            Err(error) => {
                state_override = Some(UserSkillsGitState::PushFailed);
                message = format!("Git push failed: {error}");
            }
        }
    }

    let status = user_skills_git_status_for_repo(repo)?;
    Ok(UserSkillsSyncResult {
        repo_path: status.repo_path,
        initialized,
        remote_updated,
        branch: status.branch,
        dirty: status.dirty,
        raw_status: status.raw_status,
        committed,
        commit_sha,
        pushed,
        push_attempted: request.push,
        state: state_override.unwrap_or(status.state),
        message,
    })
}

pub fn check_remote_skill_updates(
    managed_root: impl AsRef<Path>,
) -> Result<RemoteSkillUpdateCheck> {
    let preferences = managed_preferences(managed_root.as_ref())?;
    check_remote_skill_updates_with_timeout(managed_root, preferences.remote_update_timeout_seconds)
}

pub fn check_remote_skill_updates_with_timeout(
    managed_root: impl AsRef<Path>,
    timeout_seconds: u32,
) -> Result<RemoteSkillUpdateCheck> {
    validate_remote_update_timeout_seconds(timeout_seconds)?;
    let paths = ensure_managed_layout(managed_root.as_ref().to_path_buf())?;
    let checked_at = operation_timestamp();
    let cached = read_remote_update_cache(&paths.database_path)?;
    let timeout = Duration::from_secs(timeout_seconds.into());
    let mut statuses = Vec::new();
    for batch in remote_skill_roots(&paths)?.chunks(REMOTE_UPDATE_CHECK_CONCURRENCY) {
        statuses.extend(
            check_remote_skill_update_batch(batch.to_vec(), timeout)
                .into_iter()
                .map(|status| preserve_cached_remote_status_on_failure(status, cached.as_ref())),
        );
    }
    let result = RemoteSkillUpdateCheck {
        checked_at: Some(checked_at),
        statuses,
    };

    write_remote_update_cache(&paths.database_path, &result)?;
    Ok(result)
}

pub fn check_remote_skill_update(
    managed_root: impl AsRef<Path>,
    skill_name: &str,
) -> Result<RemoteSkillUpdateCheck> {
    let preferences = managed_preferences(managed_root.as_ref())?;
    check_remote_skill_update_with_timeout(
        managed_root,
        skill_name,
        preferences.remote_update_timeout_seconds,
    )
}

pub fn check_remote_skill_update_with_timeout(
    managed_root: impl AsRef<Path>,
    skill_name: &str,
    timeout_seconds: u32,
) -> Result<RemoteSkillUpdateCheck> {
    validate_skill_name(skill_name)?;
    validate_remote_update_timeout_seconds(timeout_seconds)?;
    let paths = ensure_managed_layout(managed_root.as_ref().to_path_buf())?;
    let checked_at = operation_timestamp();
    let cached = read_remote_update_cache(&paths.database_path)?;
    let timeout = Duration::from_secs(timeout_seconds.into());
    let remote_root = paths.remote_skills_root.join(skill_name);
    let checked = check_one_remote_skill_update(skill_name, &remote_root, timeout);
    let checked = preserve_cached_remote_status_on_failure(checked, cached.as_ref());
    let mut statuses = Vec::new();

    for (name, remote_root) in remote_skill_roots(&paths)? {
        if name == skill_name {
            statuses.push(checked.clone());
            continue;
        }

        if !remote_root.join("source.json").exists() {
            statuses.push(no_source_remote_update_status(&name));
            continue;
        }

        if let Some(status) = cached.as_ref().and_then(|cached| {
            cached
                .statuses
                .iter()
                .find(|status| status.skill_name == name)
        }) {
            statuses.push(status.clone());
        }
    }

    if !statuses
        .iter()
        .any(|status| status.skill_name == skill_name)
    {
        statuses.push(checked);
    }

    let result = RemoteSkillUpdateCheck {
        checked_at: Some(checked_at),
        statuses,
    };

    write_remote_update_cache(&paths.database_path, &result)?;
    Ok(result)
}

pub fn cached_remote_skill_updates(
    managed_root: impl AsRef<Path>,
) -> Result<RemoteSkillUpdateCheck> {
    let paths = ensure_managed_layout(managed_root.as_ref().to_path_buf())?;
    let cached =
        read_remote_update_cache(&paths.database_path)?.unwrap_or(RemoteSkillUpdateCheck {
            checked_at: None,
            statuses: Vec::new(),
        });
    let mut statuses = Vec::new();

    for (skill_name, remote_root) in remote_skill_roots(&paths)? {
        if !remote_root.join("source.json").exists() {
            statuses.push(no_source_remote_update_status(&skill_name));
            continue;
        }

        let source_url = read_remote_source(&remote_root)
            .ok()
            .and_then(|source| remote_source_browser_url(&source));
        if let Some(status) = cached
            .statuses
            .iter()
            .find(|status| status.skill_name == skill_name)
        {
            let mut status = status.clone();
            status.source_url = source_url.or(status.source_url);
            statuses.push(status);
        }
    }

    Ok(RemoteSkillUpdateCheck {
        checked_at: cached.checked_at,
        statuses,
    })
}

pub fn preview_remote_source_binding(
    request: RemoteSourceBindingRequest,
    managed_root: impl AsRef<Path>,
) -> Result<RemoteSourceBindingPreview> {
    validate_skill_name(&request.skill_name)?;
    let paths = ensure_managed_layout(managed_root.as_ref().to_path_buf())?;
    let remote_root = paths.remote_skills_root.join(&request.skill_name);
    let local_current = remote_root.join("current");
    let local_skill = read_skill(&local_current)?;
    let current_version = current_remote_version(&paths, &request.skill_name)?;
    let existing_source = read_remote_source(&remote_root).ok();
    let installed_sha = existing_source
        .and_then(|source| source.installed_sha)
        .filter(|sha| sha == &current_version);
    let source = skillbox_github::parse_github_skill_url(&request.source_url)?;
    let temp = temporary_work_dir("source-binding");

    let result = (|| {
        let checkout = temp.join("checkout");
        let (latest_sha, resolved_path) = fetch_remote_source_skill_path(
            &source.repo_url,
            &source.reference,
            &source.path,
            &request.skill_name,
            &checkout,
        )?;
        let resolved_source_url = github_tree_source_url(
            &source.owner,
            &source.repo,
            &source.reference,
            &resolved_path,
        );
        let remote_skill_path = checkout.join(&resolved_path);
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
            source_url: resolved_source_url,
            repo_url: source.repo_url,
            owner: source.owner,
            repo: source.repo,
            path: resolved_path,
            reference: source.reference,
            ref_kind: Some(ref_kind),
            tracking,
            current_version,
            installed_sha,
            latest_sha: Some(latest_sha),
            validation,
            local_hash: local_skill.content_hash,
            remote_hash: Some(remote_skill.content_hash),
            message,
        })
    })();

    let _ = fs::remove_dir_all(&temp);
    result
}

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
    let operation_id = operation.id.clone();
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
                    id: operation_id,
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
                id: operation_id,
                status: OperationStatus::Failed,
                summary: format!("Bind {} rejected", request.skill_name),
                error: Some(preview.message.clone()),
                payload: serde_json::json!({"validation": "mismatch"}),
            },
            &managed_root,
        )?;
        return Err(preview.message);
    }

    let paths = ensure_managed_layout(managed_root.clone())?;
    let source_path = paths
        .remote_skills_root
        .join(&preview.skill_name)
        .join("source.json");
    if let Err(error) = write_github_source_metadata(&source_path, &preview) {
        let _ = finish_operation(
            OperationFinish {
                id: operation_id,
                status: OperationStatus::Failed,
                summary: format!("Bind {} failed", request.skill_name),
                error: Some(error.clone()),
                payload: serde_json::json!({}),
            },
            &managed_root,
        );
        return Err(error);
    }

    finish_operation(
        OperationFinish {
            id: operation_id.clone(),
            status: OperationStatus::Succeeded,
            summary: format!("Bound {} to GitHub source", preview.skill_name),
            error: None,
            payload: serde_json::json!({
                "validation": source_binding_validation_label(preview.validation),
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
        operation_id,
    })
}

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
        let path = entry.path();
        versions.push(RemoteSkillVersion {
            short_label: short_version_label(&version),
            kind: if version.starts_with("manual-") {
                "manual"
            } else {
                "github"
            }
            .to_string(),
            is_current: version == current_version,
            updated_at: file_modified_timestamp(&path),
            path,
            version,
        });
    }

    versions.sort_by(|left, right| {
        right
            .is_current
            .cmp(&left.is_current)
            .then(left.version.cmp(&right.version))
    });
    Ok(RemoteSkillVersionList {
        skill_name: skill_name.to_string(),
        current_version,
        versions,
    })
}

pub fn preview_remote_version_change(
    request: RemoteVersionChangeRequest,
    managed_root: impl AsRef<Path>,
) -> Result<RemoteVersionChangePreview> {
    validate_skill_name(&request.skill_name)?;
    let paths = ensure_managed_layout(managed_root.as_ref().to_path_buf())?;
    let from_version = current_remote_version(&paths, &request.skill_name)?;
    let to_version = resolve_remote_version_change_target(&paths, &request)?;
    let temp = temporary_work_dir("remote-preview");
    let result = (|| {
        let remote_root = paths.remote_skills_root.join(&request.skill_name);
        let from_path = remote_root.join("versions").join(&from_version);
        let to_path = remote_version_preview_target(&paths, &request, &to_version, &temp)?;
        let from_skill = read_skill(&from_path)?;
        let to_skill = read_skill(&to_path)?;
        if from_skill.name != to_skill.name || to_skill.name != request.skill_name {
            return Err(format!(
                "Version skill name does not match {}",
                request.skill_name
            ));
        }

        let git_files = skillbox_git::diff_no_index_tree(&from_path, &to_path)?;
        let files = git_files
            .into_iter()
            .map(|file| remote_diff_file(&from_path, &to_path, file))
            .collect::<Result<Vec<_>>>()?;
        let affected_deployments = classify_affected_deployments(&paths, &request.skill_name)?;
        let preview_id = remote_version_preview_id(
            &request.skill_name,
            request.action,
            &from_version,
            &to_version,
        );

        Ok(RemoteVersionChangePreview {
            preview_id,
            skill_name: request.skill_name,
            action: request.action,
            from_version,
            to_version,
            files,
            affected_deployments,
        })
    })();

    let _ = fs::remove_dir_all(&temp);
    result
}

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
            summary: format!(
                "Apply {} for {}",
                remote_version_action_label(request.action),
                request.skill_name
            ),
            payload: serde_json::json!({
                "targetVersion": request.target_version.clone(),
                "previewId": request.preview_id.clone()
            }),
        },
        &managed_root,
    )?;

    match apply_remote_version_change_inner(&request, &managed_root, operation.id.clone()) {
        Ok(result) => {
            finish_operation(
                OperationFinish {
                    id: operation.id.clone(),
                    status: OperationStatus::Succeeded,
                    summary: format!(
                        "Changed {} from {} to {}",
                        result.skill_name, result.from_version, result.to_version
                    ),
                    error: None,
                    payload: serde_json::json!({
                        "fromVersion": result.from_version.clone(),
                        "toVersion": result.to_version.clone(),
                        "affectedDeployments": result.affected_deployments.clone()
                    }),
                },
                &managed_root,
            )?;
            Ok(result)
        }
        Err(error) => {
            let _ = finish_operation(
                OperationFinish {
                    id: operation.id,
                    status: OperationStatus::Failed,
                    summary: format!("Remote version change failed for {}", request.skill_name),
                    error: Some(error.clone()),
                    payload: serde_json::json!({"targetVersion": request.target_version.clone()}),
                },
                &managed_root,
            );
            Err(error)
        }
    }
}

pub fn rank_remote_source_candidates(
    skill_name: &str,
    candidates: Vec<RemoteSourceCandidate>,
) -> Vec<RemoteSourceCandidate> {
    let normalized_skill = skill_name.to_ascii_lowercase();
    let mut ranked = candidates
        .into_iter()
        .map(|mut candidate| {
            let mut score = candidate.score;
            if candidate
                .name
                .as_deref()
                .map(|name| name.eq_ignore_ascii_case(skill_name))
                .unwrap_or(false)
            {
                score += 500;
                candidate
                    .match_reasons
                    .push("Exact skill name match".to_string());
            }
            if candidate
                .path
                .to_ascii_lowercase()
                .contains(&normalized_skill)
            {
                score += 300;
                candidate
                    .match_reasons
                    .push("Path contains skill name".to_string());
            }
            if candidate
                .description
                .as_deref()
                .map(|description| description.to_ascii_lowercase().contains(&normalized_skill))
                .unwrap_or(false)
            {
                score += 100;
                candidate
                    .match_reasons
                    .push("Description mentions skill name".to_string());
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
    ranked.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then(left.path.cmp(&right.path))
    });
    ranked
}

pub fn find_remote_source_candidates(
    skill_name: &str,
    managed_root: impl AsRef<Path>,
) -> Result<RemoteSourceCandidateSearch> {
    validate_skill_name(skill_name)?;
    let paths = ensure_managed_layout(managed_root.as_ref().to_path_buf())?;
    read_skill(paths.remote_skills_root.join(skill_name).join("current"))?;
    let response = claude_marketplace_api_get()?;
    let candidates = parse_claude_marketplace_skill_candidates(skill_name, &response)?;
    Ok(RemoteSourceCandidateSearch {
        skill_name: skill_name.to_string(),
        candidates: rank_remote_source_candidates(skill_name, candidates),
    })
}

pub fn scan_import_candidates(
    roots: &[PathBuf],
    managed_root: impl AsRef<Path>,
) -> Result<ImportCandidateScan> {
    let paths = ensure_managed_layout(managed_root.as_ref().to_path_buf())?;
    let imported_hashes = imported_skill_hashes(&paths)?;
    let scan = scan_skill_roots(roots)?;
    record_scanned_workspaces(&paths, &scan.roots)?;
    let mut candidates = Vec::new();

    for skill in scan.skills {
        let is_system = is_system_skill(&skill);
        let is_imported = skill_is_imported(&skill, &imported_hashes, &paths);
        let (suggested_type, suggestion_reason, default_selected) =
            infer_import_candidate_type(&skill, &paths);
        let (suggestion_reason, import_status, is_selected, conflict) = if is_system {
            (
                suggestion_reason,
                ImportCandidateStatus::System,
                false,
                None,
            )
        } else if is_imported {
            (
                imported_candidate_reason(&skill, &paths),
                ImportCandidateStatus::Imported,
                false,
                None,
            )
        } else {
            let conflict = managed_target_conflict(&paths, &skill, suggested_type)?;
            let is_selected = default_selected && conflict.is_none();
            (
                suggestion_reason,
                ImportCandidateStatus::Importable,
                is_selected,
                conflict,
            )
        };

        candidates.push(ImportCandidate {
            name: skill.name,
            description: skill.description,
            source_path: skill.path,
            source_root: skill.source_root,
            real_path: skill.real_path,
            content_hash: skill.content_hash,
            suggested_type,
            suggestion_reason,
            import_status,
            is_selected,
            conflict,
        });
    }

    candidates.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(ImportCandidateScan {
        roots: scan.roots,
        candidates,
        errors: scan.errors,
    })
}

pub fn import_candidates(
    items: Vec<ImportRequestItem>,
    managed_root: impl AsRef<Path>,
) -> Result<ImportBatchResult> {
    let paths = ensure_managed_layout(managed_root.as_ref().to_path_buf())?;
    let mut imported = Vec::new();
    let mut errors = Vec::new();

    for item in items {
        let source_path = item.source_path.clone();
        match import_one_candidate(&paths, item) {
            Ok(candidate) => imported.push(candidate),
            Err(error) => errors.push(ImportCandidateError { source_path, error }),
        }
    }

    Ok(ImportBatchResult { imported, errors })
}

fn managed_skill(skill: Skill, kind: SkillKind) -> ManagedSkill {
    ManagedSkill {
        name: skill.name,
        description: skill.description,
        version: skill.version,
        path: skill.path,
        skill_md_path: skill.skill_md_path,
        content_hash: skill.content_hash,
        source_root: skill.source_root,
        is_symlink: skill.is_symlink,
        real_path: skill.real_path,
        kind,
        status: match kind {
            SkillKind::User => "sync not checked",
            SkillKind::Remote => "update not checked",
        }
        .to_string(),
        deployments: Vec::new(),
    }
}

fn remote_skill_roots(paths: &ManagedPaths) -> Result<Vec<(String, PathBuf)>> {
    let mut remote_roots = fs::read_dir(&paths.remote_skills_root)
        .map_err(|error| error.to_string())?
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false))
        .map(|entry| {
            (
                entry.file_name().to_string_lossy().to_string(),
                entry.path(),
            )
        })
        .collect::<Vec<_>>();
    remote_roots.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(remote_roots)
}

fn no_source_remote_update_status(skill_name: &str) -> RemoteSkillUpdateStatus {
    RemoteSkillUpdateStatus {
        skill_name: skill_name.to_string(),
        source_type: None,
        source_url: None,
        current_version: None,
        installed_sha: None,
        latest_sha: None,
        ref_kind: None,
        tracking: false,
        update_available: false,
        state: RemoteSkillUpdateState::NoSource,
        message: Some("Remote source metadata is missing.".to_string()),
    }
}

fn remote_source_browser_url(source: &RemoteSkillSource) -> Option<String> {
    if let Some(url) = source
        .source_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Some(url.to_string());
    }

    let repo_url = source.repo_url.as_deref()?.trim();
    let repo = repo_url
        .strip_prefix("https://github.com/")?
        .trim_end_matches(".git")
        .trim_end_matches('/');
    let path = source.path.as_deref()?.trim().trim_matches('/');
    if repo.is_empty() || path.is_empty() {
        return None;
    }

    let reference = source
        .reference
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("main");
    Some(format!("https://github.com/{repo}/tree/{reference}/{path}"))
}

fn validate_remote_update_timeout_seconds(seconds: u32) -> Result<()> {
    if !(MIN_REMOTE_UPDATE_TIMEOUT_SECONDS..=MAX_REMOTE_UPDATE_TIMEOUT_SECONDS).contains(&seconds) {
        return Err(format!(
            "Remote update timeout must be between {MIN_REMOTE_UPDATE_TIMEOUT_SECONDS} and {MAX_REMOTE_UPDATE_TIMEOUT_SECONDS} seconds."
        ));
    }
    Ok(())
}

fn check_remote_skill_update_batch(
    batch: Vec<(String, PathBuf)>,
    timeout: Duration,
) -> Vec<RemoteSkillUpdateStatus> {
    let mut handles = Vec::new();
    for (skill_name, remote_root) in batch {
        let thread_skill_name = skill_name.clone();
        let handle = thread::spawn(move || {
            check_one_remote_skill_update(&thread_skill_name, &remote_root, timeout)
        });
        handles.push((skill_name, handle));
    }

    handles
        .into_iter()
        .map(|(skill_name, handle)| {
            handle.join().unwrap_or_else(|_| RemoteSkillUpdateStatus {
                skill_name,
                source_type: None,
                source_url: None,
                current_version: None,
                installed_sha: None,
                latest_sha: None,
                ref_kind: None,
                tracking: false,
                update_available: false,
                state: RemoteSkillUpdateState::CheckFailed,
                message: Some("Remote update check panicked.".to_string()),
            })
        })
        .collect()
}

fn preserve_cached_remote_status_on_failure(
    status: RemoteSkillUpdateStatus,
    cached: Option<&RemoteSkillUpdateCheck>,
) -> RemoteSkillUpdateStatus {
    if status.state != RemoteSkillUpdateState::CheckFailed {
        return status;
    }

    let Some(cached_status) = cached.and_then(|cached| {
        cached
            .statuses
            .iter()
            .find(|cached_status| cached_status.skill_name == status.skill_name)
    }) else {
        return status;
    };

    if matches!(
        cached_status.state,
        RemoteSkillUpdateState::CheckFailed | RemoteSkillUpdateState::NoSource
    ) {
        return status;
    }

    let mut preserved = cached_status.clone();
    preserved.source_url = status.source_url.or(preserved.source_url);
    let message = status
        .message
        .filter(|message| !message.trim().is_empty())
        .unwrap_or_else(|| "Remote update check failed.".to_string());
    preserved.message = Some(format!("Last check failed: {message}"));
    preserved
}

fn check_one_remote_skill_update(
    skill_name: &str,
    remote_root: &Path,
    timeout: Duration,
) -> RemoteSkillUpdateStatus {
    let source_path = remote_root.join("source.json");
    let source_content = match fs::read_to_string(&source_path) {
        Ok(content) => content,
        Err(_) => {
            return no_source_remote_update_status(skill_name);
        }
    };

    let source = match parse_remote_source_content(&source_content) {
        Ok(source) => source,
        Err(error) => {
            return RemoteSkillUpdateStatus {
                skill_name: skill_name.to_string(),
                source_type: None,
                source_url: None,
                current_version: None,
                installed_sha: None,
                latest_sha: None,
                ref_kind: None,
                tracking: false,
                update_available: false,
                state: RemoteSkillUpdateState::CheckFailed,
                message: Some(format!("Invalid source metadata: {error}")),
            };
        }
    };

    let current_version = source
        .current_version
        .clone()
        .or_else(|| source.installed_sha.clone());
    let installed_sha = source.installed_sha.clone();
    let latest_sha = source.latest_sha.clone();
    let ref_kind = source.ref_kind.clone();
    let source_url = remote_source_browser_url(&source);

    if source.source_type != "github" {
        return RemoteSkillUpdateStatus {
            skill_name: skill_name.to_string(),
            source_type: Some(source.source_type),
            source_url,
            current_version,
            installed_sha,
            latest_sha,
            ref_kind,
            tracking: false,
            update_available: false,
            state: RemoteSkillUpdateState::NotCheckable,
            message: Some("Only GitHub remote skills can be checked.".to_string()),
        };
    }

    let Some(repo_url) = source
        .repo_url
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    else {
        return RemoteSkillUpdateStatus {
            skill_name: skill_name.to_string(),
            source_type: Some(source.source_type),
            source_url,
            current_version,
            installed_sha,
            latest_sha,
            ref_kind,
            tracking: false,
            update_available: false,
            state: RemoteSkillUpdateState::CheckFailed,
            message: Some("GitHub source is missing repoUrl.".to_string()),
        };
    };
    let reference = source
        .reference
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("main");
    let ref_kind = ref_kind.or_else(|| {
        if skillbox_github::classify_ref_text(reference) == skillbox_github::GitHubRefKind::Commit {
            Some("commit".to_string())
        } else {
            None
        }
    });
    let ref_is_pinned = matches!(ref_kind.as_deref(), Some("tag") | Some("commit"));
    let tracking = !ref_is_pinned && source.tracking.unwrap_or(true);
    let status_ref_kind = ref_kind
        .clone()
        .or_else(|| tracking.then(|| "branch".to_string()));

    if !tracking {
        return RemoteSkillUpdateStatus {
            skill_name: skill_name.to_string(),
            source_type: Some(source.source_type),
            source_url,
            current_version,
            installed_sha,
            latest_sha,
            ref_kind: status_ref_kind,
            tracking,
            update_available: false,
            state: RemoteSkillUpdateState::Pinned,
            message: Some("Pinned GitHub source.".to_string()),
        };
    }

    match skillbox_git::ls_remote_with_timeout(repo_url, reference, timeout) {
        Ok(Some(latest_sha)) => {
            let active_version = current_version.as_deref().or(installed_sha.as_deref());
            let update_available = active_version != Some(latest_sha.as_str());
            RemoteSkillUpdateStatus {
                skill_name: skill_name.to_string(),
                source_type: Some(source.source_type),
                source_url,
                current_version,
                installed_sha,
                latest_sha: Some(latest_sha),
                ref_kind: status_ref_kind,
                tracking,
                update_available,
                state: if update_available {
                    RemoteSkillUpdateState::UpdateAvailable
                } else {
                    RemoteSkillUpdateState::UpToDate
                },
                message: None,
            }
        }
        Ok(None) => RemoteSkillUpdateStatus {
            skill_name: skill_name.to_string(),
            source_type: Some(source.source_type),
            source_url,
            current_version,
            installed_sha,
            latest_sha,
            ref_kind: status_ref_kind,
            tracking,
            update_available: false,
            state: RemoteSkillUpdateState::CheckFailed,
            message: Some(format!("Git ref not found: {reference}")),
        },
        Err(error) => RemoteSkillUpdateStatus {
            skill_name: skill_name.to_string(),
            source_type: Some(source.source_type),
            source_url,
            current_version,
            installed_sha,
            latest_sha,
            ref_kind: status_ref_kind,
            tracking,
            update_available: false,
            state: RemoteSkillUpdateState::CheckFailed,
            message: Some(format!("Git update check failed: {error}")),
        },
    }
}

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

fn source_binding_validation_label(validation: SourceBindingValidation) -> &'static str {
    match validation {
        SourceBindingValidation::ExactMatch => "exact_match",
        SourceBindingValidation::SameSkillChanged => "same_skill_changed",
        SourceBindingValidation::Mismatch => "mismatch",
    }
}

fn write_github_source_metadata(path: &Path, preview: &RemoteSourceBindingPreview) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| "Source metadata path has no parent.".to_string())?;
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
        "sourceLinkedAt": operation_timestamp()
    });
    let content = serde_json::to_string_pretty(&json).map_err(|error| error.to_string())?;
    fs::write(path, content).map_err(|error| error.to_string())
}

fn read_remote_source(remote_root: &Path) -> Result<RemoteSkillSource> {
    let source_path = remote_root.join("source.json");
    let content = fs::read_to_string(&source_path).map_err(|error| error.to_string())?;
    parse_remote_source_content(&content)
}

fn parse_remote_source_content(content: &str) -> Result<RemoteSkillSource> {
    let source: RemoteSkillSource =
        serde_json::from_str(content).map_err(|error| error.to_string())?;
    validate_remote_source(&source)?;
    Ok(source)
}

fn validate_remote_source(source: &RemoteSkillSource) -> Result<()> {
    if source.source_type != "github" {
        return Ok(());
    }

    if let Some(repo_url) = source.repo_url.as_deref() {
        validate_remote_source_repo_url(repo_url)?;
    }
    if let Some(path) = source.path.as_deref() {
        skillbox_github::validate_repo_relative_path(path)?;
    }
    if let Some(reference) = source.reference.as_deref() {
        skillbox_github::validate_git_reference(reference)?;
    }
    Ok(())
}

fn validate_remote_source_repo_url(repo_url: &str) -> Result<()> {
    #[cfg(test)]
    {
        if Path::new(repo_url).is_absolute() {
            return Ok(());
        }
    }
    skillbox_github::validate_github_repo_url(repo_url)
}

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
            if let Some(target) = request
                .target_version
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                return Ok(target.to_string());
            }
            let source = read_remote_source(&paths.remote_skills_root.join(&request.skill_name))?;
            source
                .latest_sha
                .ok_or_else(|| "No latest GitHub SHA is available.".to_string())
        }
    }
}

fn resolve_remote_version_prefix(
    paths: &ManagedPaths,
    skill_name: &str,
    input: &str,
) -> Result<String> {
    let versions_root = paths.remote_skills_root.join(skill_name).join("versions");
    let mut matches = Vec::new();
    for entry in fs::read_dir(&versions_root).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        if !entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false) {
            continue;
        }
        let version = entry.file_name().to_string_lossy().to_string();
        if version == input {
            return Ok(version);
        }
        if version.starts_with(input) {
            matches.push(version);
        }
    }

    match matches.len() {
        0 => Err(format!("Version not found: {input}")),
        1 => Ok(matches.remove(0)),
        _ => Err(format!("Version prefix is ambiguous: {input}")),
    }
}

fn remote_version_preview_target(
    paths: &ManagedPaths,
    request: &RemoteVersionChangeRequest,
    to_version: &str,
    temp: &Path,
) -> Result<PathBuf> {
    let remote_root = paths.remote_skills_root.join(&request.skill_name);
    let version_path = remote_root.join("versions").join(to_version);
    if version_path.exists() {
        return Ok(version_path);
    }

    if request.action == RemoteVersionChangeAction::Rollback {
        return Ok(version_path);
    }

    let source = read_remote_source(&remote_root)?;
    let repo_url = source
        .repo_url
        .ok_or_else(|| "GitHub source is missing repoUrl.".to_string())?;
    let source_path = source
        .path
        .ok_or_else(|| "GitHub source is missing path.".to_string())?;
    let checkout = temp.join("checkout");
    skillbox_git::fetch_ref_path(&repo_url, to_version, &source_path, &checkout)?;
    Ok(checkout.join(source_path))
}

fn short_version_label(version: &str) -> String {
    if version.starts_with("manual-") {
        version.to_string()
    } else {
        version.chars().take(12).collect()
    }
}

fn remote_diff_file(
    old_root: &Path,
    new_root: &Path,
    file: skillbox_git::GitDiffFile,
) -> Result<RemoteDiffFile> {
    let old_relative = file.old_path.as_deref().unwrap_or(&file.path);
    let old_path = old_root.join(old_relative);
    let new_path = new_root.join(&file.path);
    let old_metadata = file_metadata(&old_path)?;
    let new_metadata = file_metadata(&new_path)?;
    let binary = old_metadata.binary || new_metadata.binary;
    let too_large = old_metadata.too_large || new_metadata.too_large;

    Ok(RemoteDiffFile {
        path: file.path,
        old_path: file.old_path,
        status: file.status.clone(),
        label: remote_diff_label(&file.status).to_string(),
        diff: if binary || too_large {
            String::new()
        } else {
            file.diff
        },
        old_hash: old_metadata.hash,
        new_hash: new_metadata.hash,
        old_size: old_metadata.size,
        new_size: new_metadata.size,
        binary,
        too_large,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FileMetadata {
    hash: Option<String>,
    size: Option<u64>,
    binary: bool,
    too_large: bool,
}

fn file_metadata(path: &Path) -> Result<FileMetadata> {
    if !path.exists() {
        return Ok(FileMetadata {
            hash: None,
            size: None,
            binary: false,
            too_large: false,
        });
    }

    let bytes = fs::read(path).map_err(|error| error.to_string())?;
    let size = bytes.len() as u64;
    let too_large = bytes.len() > 120_000;
    let binary = std::str::from_utf8(&bytes).is_err();
    Ok(FileMetadata {
        hash: Some(sha256_bytes(&bytes)),
        size: Some(size),
        binary,
        too_large,
    })
}

fn remote_diff_label(status: &str) -> &'static str {
    match status.chars().next() {
        Some('A') => "Added",
        Some('D') => "Deleted",
        Some('M') => "Modified",
        Some('R') => "Renamed",
        Some('C') => "Copied",
        _ => "Changed",
    }
}

fn content_hash_text(text: &str) -> String {
    sha256_bytes(text.as_bytes())
}

fn classify_affected_deployments(
    paths: &ManagedPaths,
    skill_name: &str,
) -> Result<Vec<AffectedDeployment>> {
    let deployments = load_deployments(&paths.database_path)?;
    let current = paths.remote_skills_root.join(skill_name).join("current");
    let versions_root = paths.remote_skills_root.join(skill_name).join("versions");
    let mut affected = Vec::new();

    for deployment in deployments.get(skill_name).cloned().unwrap_or_default() {
        let link_target = fs::read_link(&deployment.target_path).ok();
        let state = if link_target.as_ref() == Some(&current) {
            "follows_current"
        } else if link_target
            .as_ref()
            .map(|target| target.starts_with(&versions_root))
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

    Ok(affected)
}

fn apply_remote_version_change_inner(
    request: &RemoteVersionChangeApplyRequest,
    managed_root: &Path,
    operation_id: String,
) -> Result<RemoteVersionChangeApplyResult> {
    let paths = ensure_managed_layout(managed_root.to_path_buf())?;
    let from_version = current_remote_version(&paths, &request.skill_name)?;
    let to_version = resolve_remote_version_apply_target(&paths, request)?;
    validate_remote_version_preview_id(request, &from_version, &to_version)?;
    let remote_root = paths.remote_skills_root.join(&request.skill_name);
    let to_path = match request.action {
        RemoteVersionChangeAction::Update => {
            ensure_github_version_snapshot(&paths, &request.skill_name, &to_version)?
        }
        RemoteVersionChangeAction::Rollback => remote_root.join("versions").join(&to_version),
    };
    let target_skill = read_skill(&to_path)?;
    if target_skill.name != request.skill_name {
        return Err(format!(
            "Version skill name does not match {}",
            request.skill_name
        ));
    }

    let affected_deployments = classify_affected_deployments(&paths, &request.skill_name)?;
    let current_path = remote_root.join("current");
    let old_current_target = fs::read_link(&current_path).map_err(|error| error.to_string())?;
    update_current_symlink(&remote_root, &to_path)?;

    if let Err(error) =
        update_remote_metadata_after_change(&remote_root, &to_version).and_then(|_| {
            index_skill(
                &paths.database_path,
                &target_skill,
                SkillKind::Remote,
                &to_path,
            )
        })
    {
        let restore_result = update_current_symlink(&remote_root, &old_current_target);
        let restore_message = match restore_result {
            Ok(()) => "restored current",
            Err(_) => "failed to restore current",
        };
        return Err(format!("{error}; {restore_message}"));
    }

    Ok(RemoteVersionChangeApplyResult {
        skill_name: request.skill_name.clone(),
        action: request.action,
        from_version,
        to_version,
        current_path,
        affected_deployments,
        operation_id,
    })
}

fn remote_version_preview_id(
    skill_name: &str,
    action: RemoteVersionChangeAction,
    from_version: &str,
    to_version: &str,
) -> String {
    content_hash_text(&format!(
        "{}:{}:{}:{}",
        skill_name,
        remote_version_action_label(action),
        from_version,
        to_version
    ))
}

fn validate_remote_version_preview_id(
    request: &RemoteVersionChangeApplyRequest,
    from_version: &str,
    to_version: &str,
) -> Result<()> {
    let Some(preview_id) = request.preview_id.as_deref() else {
        return Ok(());
    };
    let expected = remote_version_preview_id(
        &request.skill_name,
        request.action,
        from_version,
        to_version,
    );
    if preview_id != expected {
        return Err(
            "Remote version preview is stale. Re-open the preview and apply again.".to_string(),
        );
    }
    Ok(())
}

fn resolve_remote_version_apply_target(
    paths: &ManagedPaths,
    request: &RemoteVersionChangeApplyRequest,
) -> Result<String> {
    let target = request.target_version.trim();
    if target.is_empty() {
        return Err("Target version is required.".to_string());
    }

    match request.action {
        RemoteVersionChangeAction::Rollback => {
            resolve_remote_version_prefix(paths, &request.skill_name, target)
        }
        RemoteVersionChangeAction::Update => Ok(target.to_string()),
    }
}

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
    let repo_url = source
        .repo_url
        .ok_or_else(|| "GitHub source is missing repoUrl.".to_string())?;
    let source_path = source
        .path
        .ok_or_else(|| "GitHub source is missing path.".to_string())?;
    let temp = temporary_work_dir("remote-update");

    let result = (|| {
        let checkout = temp.join("checkout");
        skillbox_git::fetch_ref_path(&repo_url, target_sha, &source_path, &checkout)?;
        copy_skill_dir(&checkout.join(source_path), &version_path)?;
        read_skill(&version_path)?;
        Ok(version_path.clone())
    })();

    let _ = fs::remove_dir_all(&temp);
    if result.is_err() {
        let _ = fs::remove_dir_all(&version_path);
    }
    result
}

fn update_remote_metadata_after_change(remote_root: &Path, to_version: &str) -> Result<()> {
    let source_path = remote_root.join("source.json");
    if !source_path.exists() {
        return Ok(());
    }

    let content = fs::read_to_string(&source_path).map_err(|error| error.to_string())?;
    let mut value: serde_json::Value =
        serde_json::from_str(&content).map_err(|error| error.to_string())?;
    value["currentVersion"] = serde_json::Value::String(to_version.to_string());
    value["installedSha"] = if skillbox_github::classify_ref_text(to_version)
        == skillbox_github::GitHubRefKind::Commit
    {
        serde_json::Value::String(to_version.to_string())
    } else {
        serde_json::Value::Null
    };
    let content = serde_json::to_string_pretty(&value).map_err(|error| error.to_string())?;
    fs::write(source_path, content).map_err(|error| error.to_string())
}

fn remote_version_action_label(action: RemoteVersionChangeAction) -> &'static str {
    match action {
        RemoteVersionChangeAction::Update => "update",
        RemoteVersionChangeAction::Rollback => "rollback",
    }
}

fn claude_marketplace_api_get() -> Result<String> {
    let args = claude_marketplace_api_curl_args();
    let output = std::process::Command::new("curl")
        .args(&args)
        .output()
        .map_err(|error| error.to_string())?;

    if !output.status.success() {
        return Err(claude_marketplace_api_error_message(
            String::from_utf8_lossy(&output.stderr).trim(),
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn claude_marketplace_api_curl_args() -> Vec<String> {
    vec![
        "-fsSL".to_string(),
        "-H".to_string(),
        "Accept: application/json".to_string(),
        "-H".to_string(),
        "User-Agent: SkillBox".to_string(),
        CLAUDE_MARKETPLACE_SKILLS_API.to_string(),
    ]
}

fn claude_marketplace_api_error_message(stderr: &str) -> String {
    if stderr.trim().is_empty() {
        "Claude Marketplace source search failed.".to_string()
    } else {
        format!("Claude Marketplace source search failed: {}", stderr.trim())
    }
}

fn fetch_remote_source_skill_path(
    repo_url: &str,
    reference: &str,
    requested_path: &str,
    skill_name: &str,
    checkout: &Path,
) -> Result<(String, String)> {
    let candidates = remote_source_path_candidates(skill_name, requested_path);
    let mut first_error = None;

    for candidate in &candidates {
        if checkout.exists() {
            fs::remove_dir_all(checkout).map_err(|error| error.to_string())?;
        }

        match skillbox_git::fetch_ref_path(repo_url, reference, candidate, checkout) {
            Ok(sha) => return Ok((sha, candidate.clone())),
            Err(error) => {
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
        }
    }

    Err(format!(
        "{}\nTried source paths: {}",
        first_error.unwrap_or_else(|| "Unable to fetch source path.".to_string()),
        candidates.join(", ")
    ))
}

fn remote_source_path_candidates(skill_name: &str, requested_path: &str) -> Vec<String> {
    let requested = requested_path.trim_matches('/');
    let leaf = Path::new(requested)
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or(skill_name);
    let mut candidates = Vec::new();

    push_unique_candidate(&mut candidates, requested);
    if !requested.starts_with("skills/") {
        push_unique_candidate(&mut candidates, &format!("skills/{requested}"));
    }
    push_unique_candidate(&mut candidates, &format!("skills/{leaf}"));
    push_unique_candidate(&mut candidates, &format!("skills/{skill_name}"));
    if !requested.starts_with("skills/public/") {
        push_unique_candidate(&mut candidates, &format!("skills/public/{requested}"));
    }
    push_unique_candidate(&mut candidates, &format!("skills/public/{leaf}"));
    push_unique_candidate(&mut candidates, &format!("skills/public/{skill_name}"));
    if !requested.starts_with(".claude/skills/") {
        push_unique_candidate(&mut candidates, &format!(".claude/skills/{requested}"));
    }
    push_unique_candidate(&mut candidates, &format!(".claude/skills/{leaf}"));
    push_unique_candidate(&mut candidates, &format!(".claude/skills/{skill_name}"));

    candidates
}

fn push_unique_candidate(candidates: &mut Vec<String>, path: &str) {
    let path = path.trim_matches('/');
    if path.is_empty() || candidates.iter().any(|candidate| candidate == path) {
        return;
    }

    candidates.push(path.to_string());
}

fn github_tree_source_url(owner: &str, repo: &str, reference: &str, path: &str) -> String {
    format!("https://github.com/{owner}/{repo}/tree/{reference}/{path}")
}

fn parse_claude_marketplace_skill_candidates(
    skill_name: &str,
    response: &str,
) -> Result<Vec<RemoteSourceCandidate>> {
    let items: Vec<ClaudeMarketplaceSkill> =
        serde_json::from_str(response).map_err(|error| error.to_string())?;
    let mut exact_candidates = Vec::new();
    let mut fuzzy_candidates = Vec::new();

    for item in items {
        if !claude_marketplace_skill_is_listed(&item)
            || !claude_marketplace_skill_matches(skill_name, &item)
        {
            continue;
        }

        let Some(candidate) = claude_marketplace_skill_to_candidate(&item) else {
            continue;
        };

        if item
            .name
            .as_deref()
            .map(|name| name.eq_ignore_ascii_case(skill_name))
            .unwrap_or(false)
        {
            exact_candidates.push(candidate);
        } else {
            fuzzy_candidates.push(candidate);
        }
    }

    let candidates = if exact_candidates.is_empty() {
        fuzzy_candidates
    } else {
        exact_candidates
    };

    Ok(candidates.into_iter().take(20).collect())
}

fn claude_marketplace_skill_is_listed(item: &ClaudeMarketplaceSkill) -> bool {
    item.listing_status
        .as_deref()
        .map(|status| status.eq_ignore_ascii_case("listed"))
        .unwrap_or(true)
}

fn claude_marketplace_skill_matches(skill_name: &str, item: &ClaudeMarketplaceSkill) -> bool {
    let normalized_skill = skill_name.to_ascii_lowercase();
    let name = item
        .name
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let path = item
        .path
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();

    name == normalized_skill || name.contains(&normalized_skill) || path.contains(&normalized_skill)
}

fn claude_marketplace_skill_to_candidate(
    item: &ClaudeMarketplaceSkill,
) -> Option<RemoteSourceCandidate> {
    let repo_label = item.repo.as_deref()?.trim();
    let path = item.path.as_deref()?.trim().trim_matches('/');
    if repo_label.is_empty() || path.is_empty() || repo_label.chars().any(char::is_whitespace) {
        return None;
    }

    let source_url = format!("https://github.com/{repo_label}/tree/main/{path}");
    let source = skillbox_github::parse_github_skill_url(&source_url).ok()?;
    let mut match_reasons = vec!["Claude Marketplace listed skill".to_string()];
    if item.installs.unwrap_or(0) > 0 {
        match_reasons.push("Claude Marketplace install signal".to_string());
    }

    Some(RemoteSourceCandidate {
        owner: source.owner,
        repo: source.repo,
        path: source.path,
        reference: source.reference,
        source_url: source.url,
        repo_url: source.repo_url,
        name: item.name.clone(),
        description: item.description.clone(),
        stars: item
            .stars
            .and_then(|value| u32::try_from(value).ok())
            .unwrap_or(0),
        archived: false,
        fork: false,
        updated_at: item.last_updated.clone().unwrap_or_default(),
        match_reasons,
        score: claude_marketplace_popularity_score(item),
    })
}

fn claude_marketplace_popularity_score(item: &ClaudeMarketplaceSkill) -> i32 {
    let install_score = item.installs.unwrap_or(0).min(1_000_000) / 5_000;
    let star_score = item.stars.unwrap_or(0).min(100_000) / 1_000;
    i32::try_from(install_score + star_score).unwrap_or(0)
}

fn user_skills_git_status_for_repo(repo_path: PathBuf) -> Result<UserSkillsGitStatus> {
    let git_status = skillbox_git::status(&repo_path)?;
    let remote_url = if git_status.initialized {
        skillbox_git::origin_url(&repo_path)?
    } else {
        None
    };
    let changed_paths = if git_status.initialized {
        skillbox_git::changed_files(&repo_path)?
            .into_iter()
            .map(|file| file.path)
            .collect()
    } else {
        Vec::new()
    };
    let state = user_skills_git_state(git_status.initialized, git_status.dirty, &remote_url);

    Ok(UserSkillsGitStatus {
        repo_path,
        initialized: git_status.initialized,
        branch: git_status.branch,
        remote_url,
        dirty: git_status.dirty,
        raw_status: git_status.raw_status,
        changed_paths,
        state,
        last_error: None,
    })
}

fn user_skills_git_state(
    initialized: bool,
    dirty: bool,
    remote_url: &Option<String>,
) -> UserSkillsGitState {
    if !initialized || remote_url.is_none() {
        UserSkillsGitState::NotConfigured
    } else if dirty {
        UserSkillsGitState::Dirty
    } else {
        UserSkillsGitState::Clean
    }
}

fn user_skill_has_uncommitted_changes(repo: &Path, skill_name: &str) -> Result<bool> {
    if !skillbox_git::status(repo)?.initialized {
        return Ok(false);
    }

    Ok(skillbox_git::changed_files(repo)?
        .into_iter()
        .any(|file| git_path_belongs_to_skill(&file.path, skill_name)))
}

fn git_path_belongs_to_skill(path: &str, skill_name: &str) -> bool {
    path == skill_name
        || path
            .strip_prefix(skill_name)
            .and_then(|rest| rest.strip_prefix('/'))
            .is_some()
}

fn normalized_commit_message(message: Option<&str>) -> String {
    message
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("chore(github): sync user skills")
        .to_string()
}

fn validate_git_relative_paths(paths: &[String]) -> Result<Vec<String>> {
    if paths.is_empty() {
        return Err("Select at least one file to commit.".to_string());
    }

    paths
        .iter()
        .map(|path| validate_git_relative_path(path))
        .collect()
}

fn validate_git_relative_path(path: &str) -> Result<String> {
    let path = path.trim();
    if path.is_empty() {
        return Err("Selected file path cannot be empty.".to_string());
    }

    let relative = Path::new(path);
    if relative.is_absolute() {
        return Err("Selected file paths must be relative.".to_string());
    }

    for component in relative.components() {
        match component {
            Component::Normal(value) if value != ".git" => {}
            _ => return Err(format!("Invalid selected file path: {path}")),
        }
    }

    Ok(path.replace('\\', "/"))
}

fn collect_user_skill_files(root: &Path) -> Result<Vec<String>> {
    let mut files = Vec::new();
    collect_user_skill_files_rec(root, root, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_user_skill_files_rec(
    root: &Path,
    current: &Path,
    files: &mut Vec<String>,
) -> Result<()> {
    if !current.exists() {
        return Ok(());
    }

    for entry in fs::read_dir(current).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        let file_name = entry.file_name();
        if file_name.to_string_lossy() == ".git" {
            continue;
        }

        let file_type = entry.file_type().map_err(|error| error.to_string())?;
        if file_type.is_dir() {
            collect_user_skill_files_rec(root, &path, files)?;
            continue;
        }

        if file_type.is_file() {
            let relative = path
                .strip_prefix(root)
                .map_err(|error| error.to_string())?
                .to_string_lossy()
                .replace('\\', "/");
            files.push(relative);
        }
    }

    Ok(())
}

fn new_file_diff(repo: &Path, relative_path: &str) -> Result<String> {
    let relative_path = validate_git_relative_path(relative_path)?;
    let path = repo.join(&relative_path);
    let bytes = fs::read(&path).map_err(|error| error.to_string())?;

    if bytes.len() > 120_000 {
        return Ok(format!(
            "diff --git a/{relative_path} b/{relative_path}\nnew file mode 100644\n--- /dev/null\n+++ b/{relative_path}\n@@\n+Diff omitted because the file is larger than 120 KB.\n"
        ));
    }

    let content = match String::from_utf8(bytes) {
        Ok(content) => content,
        Err(_) => {
            return Ok(format!(
                "diff --git a/{relative_path} b/{relative_path}\nnew file mode 100644\n--- /dev/null\n+++ b/{relative_path}\n@@\n+Binary file content is not shown.\n"
            ))
        }
    };

    let mut diff = format!(
        "diff --git a/{relative_path} b/{relative_path}\nnew file mode 100644\n--- /dev/null\n+++ b/{relative_path}\n@@\n"
    );
    for line in content.lines() {
        diff.push('+');
        diff.push_str(line);
        diff.push('\n');
    }
    if content.is_empty() {
        diff.push_str("+\n");
    }
    Ok(diff)
}

fn import_one_candidate(
    paths: &ManagedPaths,
    item: ImportRequestItem,
) -> Result<ImportedCandidate> {
    let source_path = expand_home(item.source_path);
    let imported = import_skill(&source_path, item.skill_type, &paths.root)?;
    let deployment_target = match item.skill_type {
        SkillKind::User => imported.managed_path.clone(),
        SkillKind::Remote => paths
            .remote_skills_root
            .join(&imported.name)
            .join("current"),
    };
    let (backup_path, deployed_path) = if item.deploy_back_to_source {
        let backup_path = replace_source_with_symlink(
            &source_path,
            &deployment_target,
            paths,
            &imported.name,
            &imported.content_hash,
        )?;
        (backup_path, Some(source_path.clone()))
    } else {
        (None, None)
    };

    Ok(ImportedCandidate {
        name: imported.name,
        kind: imported.kind,
        source_path,
        managed_path: imported.managed_path,
        content_hash: imported.content_hash,
        backup_path,
        deployed_path,
    })
}

fn open_database(database_path: &Path) -> Result<Connection> {
    let connection = Connection::open(database_path).map_err(|error| error.to_string())?;
    connection
        .execute_batch(
            "
            PRAGMA busy_timeout = 5000;
            PRAGMA journal_mode = WAL;
            ",
        )
        .map_err(|error| error.to_string())?;
    Ok(connection)
}

fn init_database(database_path: &Path) -> Result<()> {
    if let Some(parent) = database_path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let connection = open_database(database_path)?;
    connection
        .execute_batch(
            "
            CREATE TABLE IF NOT EXISTS skills (
              name TEXT PRIMARY KEY,
              type TEXT NOT NULL,
              description TEXT NOT NULL DEFAULT '',
              version TEXT NOT NULL DEFAULT '',
              managed_path TEXT NOT NULL,
              status TEXT NOT NULL DEFAULT 'ok',
              content_hash TEXT NOT NULL DEFAULT '',
              updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS deployments (
              skill_name TEXT NOT NULL,
              target_root TEXT NOT NULL,
              target_path TEXT NOT NULL,
              mode TEXT NOT NULL,
              updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
              PRIMARY KEY (skill_name, target_root)
            );

            CREATE TABLE IF NOT EXISTS preferences (
              key TEXT PRIMARY KEY,
              value TEXT NOT NULL,
              updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS workspaces (
              canonical_path TEXT PRIMARY KEY,
              path TEXT NOT NULL,
              kind TEXT NOT NULL,
              source TEXT NOT NULL,
              agent_id TEXT,
              display_name TEXT NOT NULL,
              skill_count INTEGER NOT NULL DEFAULT 0,
              imported_skill_count INTEGER NOT NULL DEFAULT 0,
              last_scan_error_count INTEGER NOT NULL DEFAULT 0,
              last_scan_error TEXT,
              last_scanned_at TEXT,
              created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
              updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );

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
            ",
        )
        .map_err(|error| error.to_string())?;
    ensure_database_column(
        &connection,
        "workspaces",
        "imported_skill_count",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    Ok(())
}

fn ensure_database_column(
    connection: &Connection,
    table: &str,
    column: &str,
    definition: &str,
) -> Result<()> {
    let mut statement = connection
        .prepare(&format!("PRAGMA table_info({table})"))
        .map_err(|error| error.to_string())?;
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|error| error.to_string())?;

    for existing in columns {
        if existing.map_err(|error| error.to_string())? == column {
            return Ok(());
        }
    }

    connection
        .execute(
            &format!("ALTER TABLE {table} ADD COLUMN {column} {definition}"),
            [],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn operation_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    format!("op-{nanos}")
}

fn operation_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default();
    seconds.to_string()
}

fn file_modified_timestamp(path: &Path) -> String {
    use std::time::UNIX_EPOCH;

    fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_default()
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

fn read_bool_preference(database_path: &Path, key: &str) -> Result<Option<bool>> {
    let connection = open_database(database_path).map_err(|error| error.to_string())?;
    let value: Option<String> = connection
        .query_row(
            "SELECT value FROM preferences WHERE key = ?1",
            params![key],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;

    match value.as_deref() {
        None => Ok(None),
        Some("true") => Ok(Some(true)),
        Some("false") => Ok(Some(false)),
        Some(other) => Err(format!("Invalid boolean preference {key}: {other}")),
    }
}

fn write_bool_preference(database_path: &Path, key: &str, value: bool) -> Result<()> {
    let connection = open_database(database_path).map_err(|error| error.to_string())?;
    connection
        .execute(
            "
            INSERT INTO preferences (key, value)
            VALUES (?1, ?2)
            ON CONFLICT(key) DO UPDATE SET
              value = excluded.value,
              updated_at = CURRENT_TIMESTAMP
            ",
            params![key, if value { "true" } else { "false" }],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn read_u32_preference(database_path: &Path, key: &str) -> Result<Option<u32>> {
    let connection = open_database(database_path).map_err(|error| error.to_string())?;
    let value: Option<String> = connection
        .query_row(
            "SELECT value FROM preferences WHERE key = ?1",
            params![key],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;

    value
        .map(|raw| {
            raw.parse::<u32>()
                .map_err(|error| format!("Invalid numeric preference {key}: {error}"))
        })
        .transpose()
}

fn write_u32_preference(database_path: &Path, key: &str, value: u32) -> Result<()> {
    let connection = open_database(database_path).map_err(|error| error.to_string())?;
    connection
        .execute(
            "
            INSERT INTO preferences (key, value)
            VALUES (?1, ?2)
            ON CONFLICT(key) DO UPDATE SET
              value = excluded.value,
              updated_at = CURRENT_TIMESTAMP
            ",
            params![key, value.to_string()],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn read_remote_update_cache(database_path: &Path) -> Result<Option<RemoteSkillUpdateCheck>> {
    let connection = open_database(database_path).map_err(|error| error.to_string())?;
    let value: Option<String> = connection
        .query_row(
            "SELECT value FROM preferences WHERE key = ?1",
            params!["remote_skill_update_cache"],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;

    value
        .map(|raw| serde_json::from_str(&raw).map_err(|error| error.to_string()))
        .transpose()
}

fn write_remote_update_cache(database_path: &Path, result: &RemoteSkillUpdateCheck) -> Result<()> {
    let connection = open_database(database_path).map_err(|error| error.to_string())?;
    let value = serde_json::to_string(result).map_err(|error| error.to_string())?;
    connection
        .execute(
            "
            INSERT INTO preferences (key, value)
            VALUES (?1, ?2)
            ON CONFLICT(key) DO UPDATE SET
              value = excluded.value,
              updated_at = CURRENT_TIMESTAMP
            ",
            params!["remote_skill_update_cache", value],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn record_scanned_workspaces(paths: &ManagedPaths, roots: &[PathBuf]) -> Result<()> {
    let home = home_dir();
    for root in roots {
        if workspace_root_is_readable(root) {
            upsert_workspace(
                paths,
                root,
                infer_workspace_kind(root, &home),
                WorkspaceSource::Auto,
            )?;
        }
    }
    Ok(())
}

fn upsert_workspace(
    paths: &ManagedPaths,
    path: &Path,
    kind: WorkspaceKind,
    source: WorkspaceSource,
) -> Result<Workspace> {
    let path = expand_home(path.to_path_buf());
    let canonical_path = fs::canonicalize(&path).map_err(|error| error.to_string())?;
    let stats = scan_workspace_root(&path, paths)?;
    let agent_id = workspace_agent_id(&path);
    let display_name = workspace_display_name(&path, agent_id.as_deref(), kind);
    let connection = open_database(&paths.database_path).map_err(|error| error.to_string())?;

    connection
        .execute(
            "
            INSERT INTO workspaces (
              canonical_path,
              path,
              kind,
              source,
              agent_id,
              display_name,
              skill_count,
              imported_skill_count,
              last_scan_error_count,
              last_scan_error,
              last_scanned_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, CURRENT_TIMESTAMP)
            ON CONFLICT(canonical_path) DO UPDATE SET
              path = excluded.path,
              kind = CASE
                WHEN workspaces.source = 'manual' AND excluded.source = 'auto'
                THEN workspaces.kind
                ELSE excluded.kind
              END,
              source = CASE
                WHEN workspaces.source = 'manual' AND excluded.source = 'auto'
                THEN workspaces.source
                ELSE excluded.source
              END,
              agent_id = excluded.agent_id,
              display_name = excluded.display_name,
              skill_count = excluded.skill_count,
              imported_skill_count = excluded.imported_skill_count,
              last_scan_error_count = excluded.last_scan_error_count,
              last_scan_error = excluded.last_scan_error,
              last_scanned_at = CURRENT_TIMESTAMP,
              updated_at = CURRENT_TIMESTAMP
            ",
            params![
                canonical_path.to_string_lossy(),
                path.to_string_lossy(),
                kind.as_str(),
                source.as_str(),
                agent_id,
                display_name,
                stats.skill_count as i64,
                stats.imported_skill_count as i64,
                stats.error_count as i64,
                stats.last_error,
            ],
        )
        .map_err(|error| error.to_string())?;

    load_workspace_by_canonical_path(&paths.database_path, &canonical_path)?
        .ok_or_else(|| format!("Workspace was not saved: {}", path.display()))
}

fn load_workspaces(database_path: &Path) -> Result<Vec<Workspace>> {
    let connection = open_database(database_path).map_err(|error| error.to_string())?;
    let mut statement = connection
        .prepare(
            "
            SELECT
              canonical_path,
              path,
              kind,
              source,
              agent_id,
              display_name,
              skill_count,
              imported_skill_count,
              last_scan_error_count,
              last_scan_error,
              last_scanned_at
            FROM workspaces
            ORDER BY kind, display_name, path
            ",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], workspace_from_row)
        .map_err(|error| error.to_string())?;
    let mut workspaces = Vec::new();

    for row in rows {
        workspaces.push(row.map_err(|error| error.to_string())?);
    }

    Ok(workspaces)
}

fn load_workspace_by_canonical_path(
    database_path: &Path,
    canonical_path: &Path,
) -> Result<Option<Workspace>> {
    let connection = open_database(database_path).map_err(|error| error.to_string())?;
    connection
        .query_row(
            "
            SELECT
              canonical_path,
              path,
              kind,
              source,
              agent_id,
              display_name,
              skill_count,
              imported_skill_count,
              last_scan_error_count,
              last_scan_error,
              last_scanned_at
            FROM workspaces
            WHERE canonical_path = ?1
            ",
            params![canonical_path.to_string_lossy()],
            workspace_from_row,
        )
        .optional()
        .map_err(|error| error.to_string())
}

fn workspace_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Workspace> {
    let kind_raw: String = row.get(2)?;
    let source_raw: String = row.get(3)?;
    let skill_count: i64 = row.get(6)?;
    let imported_skill_count: i64 = row.get(7)?;
    let last_scan_error_count: i64 = row.get(8)?;

    Ok(Workspace {
        canonical_path: PathBuf::from(row.get::<_, String>(0)?),
        path: PathBuf::from(row.get::<_, String>(1)?),
        kind: workspace_kind_from_str(&kind_raw)
            .map_err(rusqlite::Error::ToSqlConversionFailure)?,
        source: workspace_source_from_str(&source_raw)
            .map_err(rusqlite::Error::ToSqlConversionFailure)?,
        agent_id: row.get(4)?,
        display_name: row.get(5)?,
        skill_count: usize::try_from(skill_count.max(0)).unwrap_or_default(),
        imported_skill_count: usize::try_from(imported_skill_count.max(0)).unwrap_or_default(),
        last_scan_error_count: usize::try_from(last_scan_error_count.max(0)).unwrap_or_default(),
        last_scan_error: row.get(9)?,
        last_scanned_at: row.get(10)?,
    })
}

fn workspace_kind_from_str(
    value: &str,
) -> std::result::Result<WorkspaceKind, Box<dyn std::error::Error + Send + Sync>> {
    match value {
        "global" => Ok(WorkspaceKind::Global),
        "user" => Ok(WorkspaceKind::User),
        other => Err(format!("Invalid workspace kind: {other}").into()),
    }
}

fn workspace_source_from_str(
    value: &str,
) -> std::result::Result<WorkspaceSource, Box<dyn std::error::Error + Send + Sync>> {
    match value {
        "auto" => Ok(WorkspaceSource::Auto),
        "manual" => Ok(WorkspaceSource::Manual),
        other => Err(format!("Invalid workspace source: {other}").into()),
    }
}

struct WorkspaceScanStats {
    skill_count: usize,
    imported_skill_count: usize,
    error_count: usize,
    last_error: Option<String>,
}

fn scan_workspace_root(root: &Path, paths: &ManagedPaths) -> Result<WorkspaceScanStats> {
    let scan = scan_skill_roots(&[root.to_path_buf()])?;
    let imported_hashes = imported_skill_hashes(paths)?;
    let imported_skill_count = scan
        .skills
        .iter()
        .filter(|skill| skill_is_imported(skill, &imported_hashes, paths))
        .count();

    Ok(WorkspaceScanStats {
        skill_count: scan.skills.len(),
        imported_skill_count,
        error_count: scan.errors.len(),
        last_error: scan.errors.first().map(format_scan_error),
    })
}

fn imported_skill_hashes(paths: &ManagedPaths) -> Result<HashSet<String>> {
    let managed_scan = scan_skill_roots(&[
        paths.user_skills_root.clone(),
        paths.remote_skills_root.clone(),
    ])?;
    Ok(managed_scan
        .skills
        .iter()
        .map(|skill| skill.content_hash.clone())
        .collect())
}

fn skill_is_imported(
    skill: &Skill,
    imported_hashes: &HashSet<String>,
    paths: &ManagedPaths,
) -> bool {
    imported_hashes.contains(&skill.content_hash) || is_under_path(&skill.real_path, &paths.root)
}

fn format_scan_error(error: &ScanError) -> String {
    match &error.path {
        Some(path) => format!("{}: {}", path.display(), error.error),
        None => format!("{}: {}", error.root.display(), error.error),
    }
}

fn workspace_root_is_readable(root: &Path) -> bool {
    root.is_dir() && fs::read_dir(root).is_ok()
}

fn infer_workspace_kind(root: &Path, home: &Path) -> WorkspaceKind {
    let canonical_root = fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());

    if direct_global_workspace_roots(home)
        .into_iter()
        .filter(|candidate| candidate.exists())
        .map(|candidate| fs::canonicalize(&candidate).unwrap_or(candidate))
        .any(|candidate| candidate == canonical_root)
    {
        WorkspaceKind::Global
    } else {
        WorkspaceKind::User
    }
}

fn direct_global_workspace_roots(home: &Path) -> Vec<PathBuf> {
    vec![
        home.join(".codex/skills"),
        home.join(".agents/skills"),
        home.join(".claude/skills"),
    ]
}

fn workspace_agent_id(path: &Path) -> Option<String> {
    match path
        .parent()
        .and_then(|parent| parent.file_name())
        .and_then(|name| name.to_str())
    {
        Some(".codex") => Some("codex".to_string()),
        Some(".agents") => Some("agents".to_string()),
        Some(".claude") => Some("claude".to_string()),
        _ => None,
    }
}

fn workspace_display_name(path: &Path, agent_id: Option<&str>, kind: WorkspaceKind) -> String {
    if kind == WorkspaceKind::User {
        if let Some(project_name) = workspace_project_name(path) {
            return project_name;
        }
    }

    workspace_agent_label(agent_id)
        .or_else(|| {
            path.file_name()
                .and_then(|name| name.to_str())
                .map(str::to_string)
        })
        .unwrap_or_else(|| "Local".to_string())
}

fn workspace_agent_label(agent_id: Option<&str>) -> Option<String> {
    let label = match agent_id {
        Some("codex") => "Codex",
        Some("agents") => "Agents",
        Some("claude") => "Claude",
        _ => return None,
    };

    Some(label.to_string())
}

fn workspace_project_name(path: &Path) -> Option<String> {
    let root_name = path.file_name()?.to_str()?;
    let parent = path.parent()?;
    let parent_name = parent
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");

    if root_name == "skills" && matches!(parent_name, ".codex" | ".agents" | ".claude") {
        parent
            .parent()
            .and_then(|project| project.file_name())
            .and_then(|name| name.to_str())
            .map(str::to_string)
    } else if root_name == "skills" {
        parent
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_string)
    } else {
        Some(root_name.to_string())
    }
}

fn index_skill(
    database_path: &Path,
    skill: &Skill,
    kind: SkillKind,
    managed_path: &Path,
) -> Result<()> {
    let connection = open_database(database_path).map_err(|error| error.to_string())?;
    connection
        .execute(
            "
            INSERT INTO skills (name, type, description, version, managed_path, status, content_hash)
            VALUES (?1, ?2, ?3, ?4, ?5, 'ok', ?6)
            ON CONFLICT(name) DO UPDATE SET
              type = excluded.type,
              description = excluded.description,
              version = excluded.version,
              managed_path = excluded.managed_path,
              content_hash = excluded.content_hash,
              updated_at = CURRENT_TIMESTAMP
            ",
            params![
                skill.name,
                kind.as_str(),
                skill.description,
                skill.version,
                managed_path.to_string_lossy(),
                skill.content_hash
            ],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn index_deployment(
    database_path: &Path,
    skill_name: &str,
    target_root: &Path,
    target_path: &Path,
) -> Result<()> {
    let connection = open_database(database_path).map_err(|error| error.to_string())?;
    connection
        .execute(
            "
            INSERT INTO deployments (skill_name, target_root, target_path, mode)
            VALUES (?1, ?2, ?3, 'symlink')
            ON CONFLICT(skill_name, target_root) DO UPDATE SET
              target_path = excluded.target_path,
              mode = excluded.mode,
              updated_at = CURRENT_TIMESTAMP
            ",
            params![
                skill_name,
                target_root.to_string_lossy(),
                target_path.to_string_lossy()
            ],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn remove_deployment(database_path: &Path, skill_name: &str, target_root: &Path) -> Result<()> {
    let connection = open_database(database_path).map_err(|error| error.to_string())?;
    connection
        .execute(
            "DELETE FROM deployments WHERE skill_name = ?1 AND target_root = ?2",
            params![skill_name, target_root.to_string_lossy()],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn load_deployments(database_path: &Path) -> Result<HashMap<String, Vec<ManagedSkillDeployment>>> {
    let connection = open_database(database_path).map_err(|error| error.to_string())?;
    let mut statement = connection
        .prepare(
            "
            SELECT skill_name, target_root, target_path, mode
            FROM deployments
            ORDER BY skill_name, target_root
            ",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                ManagedSkillDeployment {
                    target_root: PathBuf::from(row.get::<_, String>(1)?),
                    target_path: PathBuf::from(row.get::<_, String>(2)?),
                    mode: row.get::<_, String>(3)?,
                },
            ))
        })
        .map_err(|error| error.to_string())?;
    let mut deployments: HashMap<String, Vec<ManagedSkillDeployment>> = HashMap::new();

    for row in rows {
        let (skill_name, deployment) = row.map_err(|error| error.to_string())?;
        deployments.entry(skill_name).or_default().push(deployment);
    }

    Ok(deployments)
}

fn infer_import_candidate_type(skill: &Skill, paths: &ManagedPaths) -> (SkillKind, String, bool) {
    let path = skill.path.to_string_lossy();

    if path.contains("/.codex/skills/.system/") || path.ends_with("/.codex/skills/.system") {
        return (
            SkillKind::Remote,
            "inside ~/.codex/skills/.system".to_string(),
            false,
        );
    }

    if path.contains("/.agents/skills/") || path.ends_with("/.agents/skills") {
        return (SkillKind::User, "inside ~/.agents/skills".to_string(), true);
    }

    if path.contains("/.codex/skills/") || path.ends_with("/.codex/skills") {
        return (
            SkillKind::Remote,
            "inside ~/.codex/skills".to_string(),
            true,
        );
    }

    if skill_declares_github_source(&skill.skill_md_path) {
        return (
            SkillKind::Remote,
            "GitHub source metadata found".to_string(),
            true,
        );
    }

    if is_under_path(&skill.real_path, &paths.user_skills_root) {
        return (SkillKind::User, "inside user skill root".to_string(), true);
    }

    (SkillKind::User, "Needs confirm".to_string(), true)
}

fn is_system_skill(skill: &Skill) -> bool {
    let path = skill.path.to_string_lossy();
    path.contains("/.codex/skills/.system/") || path.ends_with("/.codex/skills/.system")
}

fn imported_candidate_reason(skill: &Skill, paths: &ManagedPaths) -> String {
    if skill.is_symlink && is_under_path(&skill.real_path, &paths.root) {
        return "Imported; source links to SkillBox".to_string();
    }

    "Already imported in SkillBox".to_string()
}

fn skill_declares_github_source(skill_md_path: &Path) -> bool {
    fs::read_to_string(skill_md_path)
        .map(|content| content.to_lowercase().contains("github.com/"))
        .unwrap_or(false)
}

fn managed_target_conflict(
    paths: &ManagedPaths,
    skill: &Skill,
    kind: SkillKind,
) -> Result<Option<String>> {
    match kind {
        SkillKind::User => {
            let target = paths.user_skills_root.join(&skill.name);
            if !target.exists() {
                return Ok(None);
            }
            if read_skill(&target)
                .map(|existing| existing.content_hash == skill.content_hash)
                .unwrap_or(false)
            {
                return Ok(None);
            }
            Ok(Some(format!("Managed target exists: {}", target.display())))
        }
        SkillKind::Remote => {
            let remote_root = paths.remote_skills_root.join(&skill.name);
            let version_target = remote_root
                .join("versions")
                .join(format!("manual-{}", &skill.content_hash[..12]));
            if version_target.exists() {
                return Ok(None);
            }
            if remote_root.exists() && !remote_root.is_dir() {
                return Ok(Some(format!(
                    "Managed target exists: {}",
                    remote_root.display()
                )));
            }
            Ok(None)
        }
    }
}

fn replace_source_with_symlink(
    source_path: &Path,
    target_path: &Path,
    paths: &ManagedPaths,
    skill_name: &str,
    content_hash: &str,
) -> Result<Option<PathBuf>> {
    let metadata = fs::symlink_metadata(source_path).map_err(|error| error.to_string())?;
    if metadata.file_type().is_symlink() {
        let linked = fs::canonicalize(source_path).map_err(|error| error.to_string())?;
        let expected = fs::canonicalize(target_path).map_err(|error| error.to_string())?;
        if linked == expected {
            return Ok(None);
        }
        return Err(format!(
            "Refusing to replace symlink pointing elsewhere: {}",
            source_path.display()
        ));
    }

    let backup_path = unique_backup_path(paths, skill_name, content_hash);
    if let Some(parent) = backup_path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }

    fs::rename(source_path, &backup_path).map_err(|error| error.to_string())?;
    if let Err(error) = symlink_dir(target_path, source_path) {
        let _ = fs::rename(&backup_path, source_path);
        return Err(error);
    }

    Ok(Some(backup_path))
}

fn unique_backup_path(paths: &ManagedPaths, skill_name: &str, content_hash: &str) -> PathBuf {
    let hash = &content_hash[..12];
    let base = paths
        .root
        .join("backups")
        .join("imports")
        .join(format!("{skill_name}-{hash}"));
    if !base.exists() {
        return base;
    }

    for index in 2..=10_000 {
        let candidate = paths
            .root
            .join("backups")
            .join("imports")
            .join(format!("{skill_name}-{hash}-{index}"));
        if !candidate.exists() {
            return candidate;
        }
    }

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    paths
        .root
        .join("backups")
        .join("imports")
        .join(format!("{skill_name}-{hash}-{nanos}"))
}

fn is_under_path(path: &Path, root: &Path) -> bool {
    let path = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let root = fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    path.starts_with(root)
}

fn find_skill_dirs(
    current: &Path,
    depth: usize,
    max_depth: usize,
    found: &mut Vec<PathBuf>,
) -> Result<()> {
    if depth > max_depth {
        return Ok(());
    }
    let current_metadata = fs::symlink_metadata(current).map_err(|error| error.to_string())?;
    if current_metadata.file_type().is_symlink() {
        return Ok(());
    }
    if current.join("SKILL.md").exists() {
        found.push(current.to_path_buf());
        return Ok(());
    }

    for entry in fs::read_dir(current).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|error| error.to_string())?;
        if !file_type.is_dir() {
            continue;
        }
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        if file_name.starts_with('.') && file_name != ".system" {
            continue;
        }
        find_skill_dirs(&path, depth + 1, max_depth, found)?;
    }

    Ok(())
}

fn resolve_managed_skill_path(paths: &ManagedPaths, skill_name: &str) -> Result<PathBuf> {
    let user_path = paths.user_skills_root.join(skill_name);
    if user_path.join("SKILL.md").exists() {
        return Ok(user_path);
    }

    let remote_current = paths.remote_skills_root.join(skill_name).join("current");
    if remote_current.join("SKILL.md").exists() {
        return Ok(remote_current);
    }

    Err(format!("Managed skill not found: {skill_name}"))
}

fn copy_skill_dir(source: &Path, destination: &Path) -> Result<()> {
    if destination.exists() {
        return Err(format!(
            "Destination already exists: {}",
            destination.display()
        ));
    }
    let temp_destination = temporary_sibling_path(destination, "copy")?;
    if temp_destination.exists() {
        return Err(format!(
            "Temporary destination already exists: {}",
            temp_destination.display()
        ));
    }
    let source_root = fs::canonicalize(source).map_err(|error| error.to_string())?;
    fs::create_dir_all(&temp_destination).map_err(|error| error.to_string())?;

    let result = (|| {
        for entry in fs::read_dir(source).map_err(|error| error.to_string())? {
            let entry = entry.map_err(|error| error.to_string())?;
            let file_name = entry.file_name();
            if file_name == ".git" {
                continue;
            }
            copy_recursively(
                &entry.path(),
                &temp_destination.join(file_name),
                &source_root,
            )?;
        }
        fs::rename(&temp_destination, destination).map_err(|error| error.to_string())
    })();

    if result.is_err() {
        let _ = fs::remove_dir_all(&temp_destination);
    }
    result
}

fn copy_recursively(source: &Path, destination: &Path, source_root: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(source).map_err(|error| error.to_string())?;
    if metadata.is_dir() {
        fs::create_dir_all(destination).map_err(|error| error.to_string())?;
        for entry in fs::read_dir(source).map_err(|error| error.to_string())? {
            let entry = entry.map_err(|error| error.to_string())?;
            copy_recursively(
                &entry.path(),
                &destination.join(entry.file_name()),
                source_root,
            )?;
        }
    } else if metadata.file_type().is_symlink() {
        let target = fs::read_link(source).map_err(|error| error.to_string())?;
        let checked_target = symlink_target_for_boundary_check(source, &target)?;
        if !checked_target.starts_with(source_root) {
            return Err(format!(
                "Refusing to copy symlink outside source root: {}",
                source.display()
            ));
        }
        symlink_any(&target, destination)?;
    } else {
        fs::copy(source, destination).map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn symlink_target_for_boundary_check(source: &Path, target: &Path) -> Result<PathBuf> {
    let absolute_target = if target.is_absolute() {
        target.to_path_buf()
    } else {
        source
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .join(target)
    };

    match fs::canonicalize(&absolute_target) {
        Ok(target) => Ok(target),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let lexical_target = if target.is_absolute() {
                absolute_target
            } else {
                let base = source
                    .parent()
                    .and_then(|parent| fs::canonicalize(parent).ok())
                    .unwrap_or_else(|| {
                        source
                            .parent()
                            .unwrap_or_else(|| Path::new(""))
                            .to_path_buf()
                    });
                base.join(target)
            };
            Ok(normalize_lexical_path(&lexical_target))
        }
        Err(error) => Err(error.to_string()),
    }
}

fn normalize_lexical_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            _ => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

fn temporary_sibling_path(destination: &Path, label: &str) -> Result<PathBuf> {
    let parent = destination
        .parent()
        .ok_or_else(|| format!("Destination has no parent: {}", destination.display()))?;
    let name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("skill");
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_nanos();
    Ok(parent.join(format!(".{name}.{label}-{nanos}.tmp")))
}

fn update_current_symlink(remote_root: &Path, version_path: &Path) -> Result<()> {
    fs::create_dir_all(remote_root).map_err(|error| error.to_string())?;
    let current = remote_root.join("current");
    match fs::symlink_metadata(&current) {
        Ok(metadata) => {
            if !metadata.file_type().is_symlink() {
                return Err(format!(
                    "Refusing to replace existing non-symlink current: {}",
                    current.display()
                ));
            }
            fs::remove_file(&current).map_err(|error| error.to_string())?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.to_string()),
    }
    symlink_dir(version_path, &current)
}

fn symlink_points_to_path(symlink: &Path, expected: &Path) -> Result<bool> {
    let target = fs::read_link(symlink).map_err(|error| error.to_string())?;
    let target = if target.is_absolute() {
        target
    } else {
        symlink
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .join(target)
    };
    Ok(target == expected)
}

fn validate_skill_name(name: &str) -> Result<()> {
    if name.is_empty() || name == "." || name == ".." || name.contains('/') || name.contains('\\') {
        return Err(format!("Invalid skill name: {name}"));
    }
    Ok(())
}

fn expand_home(path: PathBuf) -> PathBuf {
    let path_string = path.to_string_lossy();
    if path_string == "~" {
        return home_dir();
    }
    if let Some(rest) = path_string.strip_prefix("~/") {
        return home_dir().join(rest);
    }
    path
}

fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"))
}

fn unquote(value: &str) -> String {
    if value.len() >= 2
        && ((value.starts_with('"') && value.ends_with('"'))
            || (value.starts_with('\'') && value.ends_with('\'')))
    {
        value[1..value.len() - 1].to_string()
    } else {
        value.to_string()
    }
}

fn sha256(content: &str) -> String {
    sha256_bytes(content.as_bytes())
}

fn sha256_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(unix)]
fn symlink_dir(source: &Path, destination: &Path) -> Result<()> {
    std::os::unix::fs::symlink(source, destination).map_err(|error| error.to_string())
}

#[cfg(unix)]
fn symlink_any(source: &Path, destination: &Path) -> Result<()> {
    std::os::unix::fs::symlink(source, destination).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn parses_basic_skill_frontmatter() {
        let metadata = parse_skill_frontmatter(
            "---
name: demo
version: 0.1.0
description: \"Demo skill\"
---

# Demo
",
        );

        assert_eq!(metadata.name, "demo");
        assert_eq!(metadata.version, "0.1.0");
        assert_eq!(metadata.description, "Demo skill");
    }

    #[test]
    fn database_initialization_configures_busy_timeout_and_wal() {
        let source = include_str!("lib.rs");

        assert!(source.contains("PRAGMA busy_timeout = 5000"));
        assert!(source.contains("PRAGMA journal_mode = WAL"));
    }

    #[test]
    fn unique_backup_path_uses_bounded_suffix_search() {
        let source = include_str!("lib.rs");
        let start = source.find("fn unique_backup_path").unwrap();
        let end = start + source[start..].find("fn is_under_path").unwrap();
        let function_source = &source[start..end];

        assert!(!function_source.contains("for index in 2.. {"));
        assert!(!function_source.contains("unreachable!(\"backup suffix loop is unbounded\")"));
    }

    #[test]
    fn scans_nested_skill_directories() {
        let root = temp_dir("scan");
        make_skill(&root.join("alpha"), "alpha", "Alpha skill");
        make_skill(&root.join("group").join("beta"), "beta", "Beta skill");

        let scan = scan_skill_roots(&[root.clone()]).unwrap();

        assert_eq!(scan.errors.len(), 0);
        let names: Vec<_> = scan
            .skills
            .iter()
            .map(|skill| skill.name.as_str())
            .collect();
        assert_eq!(names, vec!["alpha", "beta"]);
    }

    #[test]
    fn scan_skill_roots_does_not_follow_symlinked_directories() {
        let root = temp_dir("scan-symlink-root");
        let outside = temp_dir("scan-symlink-outside");
        make_skill(&outside.join("leaked"), "leaked", "Leaked skill");
        symlink_dir(&outside, &root.join("linked")).unwrap();

        let scan = scan_skill_roots(&[root]).unwrap();

        assert_eq!(scan.errors.len(), 0);
        assert!(scan.skills.is_empty());
    }

    #[test]
    fn global_runtime_roots_include_project_local_skill_roots() {
        let root = temp_dir("global-runtime-roots");
        let project_agents_root = root
            .join("Library")
            .join("Mobile Documents")
            .join("iCloud~md~obsidian")
            .join("Documents")
            .join("Pandora")
            .join(".agents")
            .join("skills");
        let project_codex_root = root
            .join("zone")
            .join("project")
            .join(".codex")
            .join("skills");
        let global_claude_root = root.join(".claude").join("skills");
        let project_claude_root = root
            .join("Documents")
            .join("project")
            .join(".claude")
            .join("skills");

        make_skill(
            &project_agents_root.join("pandora-local"),
            "pandora-local",
            "Pandora local skill",
        );
        make_skill(
            &project_codex_root.join("project-remote"),
            "project-remote",
            "Project remote skill",
        );
        make_skill(
            &global_claude_root.join("claude-global"),
            "claude-global",
            "Claude global skill",
        );
        make_skill(
            &project_claude_root.join("claude-project"),
            "claude-project",
            "Claude project skill",
        );

        let roots = runtime_roots_under(&root);

        assert!(roots.contains(&root.join(".codex").join("skills")));
        assert!(roots.contains(&root.join(".agents").join("skills")));
        assert!(roots.contains(&global_claude_root));
        assert!(roots.contains(&project_agents_root));
        assert!(roots.contains(&project_codex_root));
        assert!(roots.contains(&project_claude_root));
    }

    #[test]
    fn default_managed_root_uses_hidden_skillbox_directory() {
        let previous = std::env::var_os("SKILLBOX_HOME");
        std::env::remove_var("SKILLBOX_HOME");

        let root = default_managed_root();

        match previous {
            Some(value) => std::env::set_var("SKILLBOX_HOME", value),
            None => std::env::remove_var("SKILLBOX_HOME"),
        }
        assert_eq!(
            root.file_name().and_then(|name| name.to_str()),
            Some(".skillbox")
        );
    }

    #[test]
    fn legacy_managed_root_is_linked_when_hidden_root_is_empty_stub() {
        let root = temp_dir("legacy-managed-root-link");
        let hidden_root = root.join(".skillbox");
        let legacy_root = root.join("SkillBox");
        fs::create_dir_all(hidden_root.join("user-skills")).unwrap();
        fs::create_dir_all(hidden_root.join("remote-skills")).unwrap();
        fs::write(hidden_root.join("skillbox.sqlite"), "").unwrap();
        make_skill(
            &legacy_root.join("user-skills").join("demo"),
            "demo",
            "Legacy demo",
        );

        let migrated = link_legacy_managed_root_if_needed(&hidden_root, &legacy_root).unwrap();
        let paths = ensure_managed_layout(&hidden_root).unwrap();
        let state = managed_state(&hidden_root).unwrap();

        assert!(migrated);
        assert_eq!(paths.root, hidden_root);
        assert!(fs::symlink_metadata(&hidden_root)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(fs::read_link(&hidden_root).unwrap(), legacy_root);
        assert_eq!(state.skills.len(), 1);
        assert_eq!(state.skills[0].name, "demo");
    }

    #[test]
    fn list_workspaces_initializes_empty_registry() {
        let managed_root = temp_dir("workspace-empty").join("SkillBox");

        let workspaces = list_workspaces(&managed_root).unwrap();

        assert!(workspaces.is_empty());
    }

    #[test]
    fn add_workspace_rejects_missing_directory() {
        let root = temp_dir("workspace-missing");
        let managed_root = root.join("SkillBox");

        let error = add_workspace(
            WorkspaceAddRequest {
                path: root.join("missing").join("skills"),
                kind: WorkspaceKind::User,
            },
            &managed_root,
        )
        .unwrap_err();

        assert!(error.contains("Workspace path does not exist"));
    }

    #[test]
    fn add_workspace_scans_existing_root_and_dedupes_by_canonical_path() {
        let root = temp_dir("workspace-add");
        let managed_root = root.join("SkillBox");
        let workspace_root = root.join("project").join(".agents").join("skills");
        make_skill(&workspace_root.join("alpha"), "alpha", "Alpha skill");

        let first = add_workspace(
            WorkspaceAddRequest {
                path: workspace_root.clone(),
                kind: WorkspaceKind::User,
            },
            &managed_root,
        )
        .unwrap();
        let second = add_workspace(
            WorkspaceAddRequest {
                path: workspace_root.join("."),
                kind: WorkspaceKind::User,
            },
            &managed_root,
        )
        .unwrap();
        let workspaces = list_workspaces(&managed_root).unwrap();

        assert_eq!(first.skill_count, 1);
        assert_eq!(first.last_scan_error_count, 0);
        assert_eq!(first.kind, WorkspaceKind::User);
        assert_eq!(first.source, WorkspaceSource::Manual);
        assert_eq!(first.agent_id.as_deref(), Some("agents"));
        assert_eq!(first.display_name, "project");
        assert_eq!(second.canonical_path, first.canonical_path);
        assert_eq!(workspaces.len(), 1);
    }

    #[test]
    fn add_workspace_counts_imported_skills() {
        let root = temp_dir("workspace-imported-count");
        let managed_root = root.join("SkillBox");
        let workspace_root = root.join("project").join(".agents").join("skills");
        let imported_source = workspace_root.join("alpha");
        make_skill(&imported_source, "alpha", "Alpha skill");
        make_skill(&workspace_root.join("beta"), "beta", "Beta skill");
        import_skill(&imported_source, SkillKind::User, &managed_root).unwrap();

        let workspace = add_workspace(
            WorkspaceAddRequest {
                path: workspace_root,
                kind: WorkspaceKind::User,
            },
            &managed_root,
        )
        .unwrap();

        assert_eq!(workspace.skill_count, 2);
        assert_eq!(workspace.imported_skill_count, 1);
    }

    #[test]
    fn scan_workspaces_discovers_global_and_user_roots() {
        let root = temp_dir("workspace-scan");
        let managed_root = root.join("SkillBox");
        let global_codex_root = root.join(".codex").join("skills");
        let global_claude_root = root.join(".claude").join("skills");
        let project_agents_root = root
            .join("Library")
            .join("Mobile Documents")
            .join("iCloud~md~obsidian")
            .join("Documents")
            .join("Pandora")
            .join(".agents")
            .join("skills");
        make_skill(
            &global_codex_root.join("find-skills"),
            "find-skills",
            "Find skills",
        );
        make_skill(
            &global_claude_root.join("claude-helper"),
            "claude-helper",
            "Claude helper",
        );
        make_skill(
            &project_agents_root.join("pandora-local"),
            "pandora-local",
            "Pandora local skill",
        );

        let result = scan_workspaces_under(&root, &managed_root).unwrap();
        let workspaces = list_workspaces(&managed_root).unwrap();
        let global_codex = workspace(&workspaces, &global_codex_root);
        let global_claude = workspace(&workspaces, &global_claude_root);
        let project_agents = workspace(&workspaces, &project_agents_root);

        assert_eq!(result.scanned_count, 3);
        assert_eq!(global_codex.kind, WorkspaceKind::Global);
        assert_eq!(global_codex.agent_id.as_deref(), Some("codex"));
        assert_eq!(global_codex.display_name, "Codex");
        assert_eq!(global_claude.kind, WorkspaceKind::Global);
        assert_eq!(global_claude.agent_id.as_deref(), Some("claude"));
        assert_eq!(global_claude.display_name, "Claude");
        assert_eq!(project_agents.kind, WorkspaceKind::User);
        assert_eq!(project_agents.agent_id.as_deref(), Some("agents"));
        assert_eq!(project_agents.display_name, "Pandora");
    }

    #[test]
    fn scan_import_candidates_records_scanned_workspaces() {
        let root = temp_dir("workspace-import-candidates");
        let managed_root = root.join("SkillBox");
        let workspace_root = root.join("project").join(".agents").join("skills");
        make_skill(
            &workspace_root.join("pandora-local"),
            "pandora-local",
            "Pandora local skill",
        );

        let candidates = scan_import_candidates(&[workspace_root.clone()], &managed_root).unwrap();
        let workspaces = list_workspaces(&managed_root).unwrap();
        let recorded = workspace(&workspaces, &workspace_root);

        assert_eq!(candidates.candidates.len(), 1);
        assert_eq!(recorded.kind, WorkspaceKind::User);
        assert_eq!(recorded.source, WorkspaceSource::Auto);
        assert_eq!(recorded.display_name, "project");
        assert_eq!(recorded.skill_count, 1);
    }

    #[test]
    fn scan_import_candidates_uses_discovered_project_local_roots() {
        let root = temp_dir("candidate-project-roots");
        let project_agents_root = root
            .join("Library")
            .join("Mobile Documents")
            .join("iCloud~md~obsidian")
            .join("Documents")
            .join("Pandora")
            .join(".agents")
            .join("skills");
        let managed_root = root.join("SkillBox");

        make_skill(
            &project_agents_root.join("pandora-local"),
            "pandora-local",
            "Pandora local skill",
        );

        let roots = runtime_roots_under(&root);
        let candidates = scan_import_candidates(&roots, &managed_root).unwrap();
        let candidate = candidate(&candidates.candidates, "pandora-local");

        assert_eq!(candidate.suggested_type, SkillKind::User);
        assert_eq!(candidate.source_root, Some(project_agents_root));
        assert!(candidate.is_selected);
    }

    #[test]
    fn imports_user_skill_and_deploys_symlink() {
        let root = temp_dir("import-deploy");
        let source = root.join("source").join("demo");
        let managed_root = root.join("SkillBox");
        let target_root = root.join("runtime");
        make_skill(&source, "demo", "Demo skill");

        let imported = import_skill(&source, SkillKind::User, &managed_root).unwrap();
        let deployment = deploy_skill("demo", &managed_root, &target_root).unwrap();

        assert_eq!(read_skill(&imported.managed_path).unwrap().name, "demo");
        assert!(fs::symlink_metadata(&deployment.target_path)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(
            fs::canonicalize(&deployment.target_path).unwrap(),
            fs::canonicalize(&imported.managed_path).unwrap()
        );

        let state = managed_state(&managed_root).unwrap();
        assert_eq!(state.skills.len(), 1);
        assert_eq!(state.skills[0].deployments.len(), 1);
        assert_eq!(state.skills[0].deployments[0].target_root, target_root);
        assert_eq!(
            state.skills[0].deployments[0].target_path,
            deployment.target_path
        );
        assert_eq!(state.skills[0].deployments[0].mode, "symlink");
    }

    #[test]
    fn deploys_remote_skill_to_current_symlink() {
        let root = temp_dir("remote-deploy-current");
        let source = root.join("source").join("remote-demo");
        let managed_root = root.join("SkillBox");
        let target_root = root.join("runtime");
        make_skill(&source, "remote-demo", "Remote demo skill");
        import_skill(&source, SkillKind::Remote, &managed_root).unwrap();

        let deployment = deploy_skill("remote-demo", &managed_root, &target_root).unwrap();
        let current = managed_root
            .join("remote-skills")
            .join("remote-demo")
            .join("current");

        assert!(fs::symlink_metadata(&deployment.target_path)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(fs::read_link(&deployment.target_path).unwrap(), current);
    }

    #[test]
    fn redeploys_remote_skill_version_symlink_to_current() {
        let root = temp_dir("remote-redeploy-current");
        let source = root.join("source").join("remote-demo");
        let managed_root = root.join("SkillBox");
        let target_root = root.join("runtime");
        let target_path = target_root.join("remote-demo");
        make_skill(&source, "remote-demo", "Remote demo skill");
        let imported = import_skill(&source, SkillKind::Remote, &managed_root).unwrap();
        fs::create_dir_all(&target_root).unwrap();
        symlink_dir(&imported.managed_path, &target_path).unwrap();

        deploy_skill("remote-demo", &managed_root, &target_root).unwrap();
        let current = managed_root
            .join("remote-skills")
            .join("remote-demo")
            .join("current");

        assert_eq!(fs::read_link(&target_path).unwrap(), current);
    }

    #[test]
    fn refuses_to_overwrite_existing_non_symlink_deployment_target() {
        let root = temp_dir("deploy-conflict");
        let source = root.join("source").join("demo");
        let managed_root = root.join("SkillBox");
        let target_root = root.join("runtime");
        make_skill(&source, "demo", "Demo skill");
        import_skill(&source, SkillKind::User, &managed_root).unwrap();
        fs::create_dir_all(target_root.join("demo")).unwrap();

        let error = deploy_skill("demo", &managed_root, &target_root).unwrap_err();

        assert!(error.contains("Refusing to overwrite existing non-symlink target"));
    }

    #[test]
    fn undeploys_managed_symlink_and_removes_deployment_index() {
        let root = temp_dir("undeploy-managed-link");
        let source = root.join("source").join("demo");
        let managed_root = root.join("SkillBox");
        let target_root = root.join("runtime");
        make_skill(&source, "demo", "Demo skill");
        import_skill(&source, SkillKind::User, &managed_root).unwrap();
        let deployment = deploy_skill("demo", &managed_root, &target_root).unwrap();

        let undeployment = undeploy_skill("demo", &managed_root, &target_root).unwrap();

        assert_eq!(undeployment.skill_name, "demo");
        assert_eq!(undeployment.target_root, target_root);
        assert_eq!(undeployment.target_path, deployment.target_path);
        assert!(!undeployment.target_path.exists());
        let state = managed_state(&managed_root).unwrap();
        assert_eq!(state.skills[0].deployments.len(), 0);
    }

    #[test]
    fn undeploy_missing_target_removes_stale_deployment_index() {
        let root = temp_dir("undeploy-missing-target");
        let source = root.join("source").join("demo");
        let managed_root = root.join("SkillBox");
        let target_root = root.join("runtime");
        make_skill(&source, "demo", "Demo skill");
        import_skill(&source, SkillKind::User, &managed_root).unwrap();
        let deployment = deploy_skill("demo", &managed_root, &target_root).unwrap();
        fs::remove_file(&deployment.target_path).unwrap();

        let undeployment = undeploy_skill("demo", &managed_root, &target_root).unwrap();

        assert_eq!(undeployment.target_path, deployment.target_path);
        let state = managed_state(&managed_root).unwrap();
        assert_eq!(state.skills[0].deployments.len(), 0);
    }

    #[test]
    fn undeploy_removes_workspace_alias_symlink() {
        let root = temp_dir("undeploy-alias-link");
        let source = root.join("source").join("dida-task-sync");
        let managed_root = root.join("SkillBox");
        let target_root = root.join("Pandora").join(".agents").join("skills");
        make_skill(&source, "dida-task-sync", "Dida sync skill");
        let imported = import_skill(&source, SkillKind::User, &managed_root).unwrap();
        fs::create_dir_all(&target_root).unwrap();
        add_workspace(
            WorkspaceAddRequest {
                path: target_root.clone(),
                kind: WorkspaceKind::User,
            },
            &managed_root,
        )
        .unwrap();
        let alias_path = target_root.join("dida-task-sync 2");
        symlink_dir(&imported.managed_path, &alias_path).unwrap();

        let state = managed_state(&managed_root).unwrap();
        assert_eq!(state.skills[0].deployments.len(), 1);

        let undeployment = undeploy_skill("dida-task-sync", &managed_root, &target_root).unwrap();

        assert_eq!(undeployment.target_path, alias_path);
        assert!(!undeployment.target_path.exists());
        let state = managed_state(&managed_root).unwrap();
        assert_eq!(state.skills[0].deployments.len(), 0);
    }

    #[test]
    fn undeploy_refuses_non_symlink_target() {
        let root = temp_dir("undeploy-non-symlink");
        let source = root.join("source").join("demo");
        let managed_root = root.join("SkillBox");
        let target_root = root.join("runtime");
        make_skill(&source, "demo", "Demo skill");
        import_skill(&source, SkillKind::User, &managed_root).unwrap();
        fs::create_dir_all(target_root.join("demo")).unwrap();

        let error = undeploy_skill("demo", &managed_root, &target_root).unwrap_err();

        assert!(error.contains("Refusing to remove existing non-symlink target"));
        assert!(target_root.join("demo").exists());
    }

    #[test]
    fn undeploy_refuses_symlink_pointing_elsewhere() {
        let root = temp_dir("undeploy-foreign-link");
        let source = root.join("source").join("demo");
        let managed_root = root.join("SkillBox");
        let target_root = root.join("runtime");
        let other_target = root.join("other").join("demo");
        make_skill(&source, "demo", "Demo skill");
        make_skill(&other_target, "demo", "Other demo skill");
        import_skill(&source, SkillKind::User, &managed_root).unwrap();
        fs::create_dir_all(&target_root).unwrap();
        symlink_dir(&other_target, &target_root.join("demo")).unwrap();

        let error = undeploy_skill("demo", &managed_root, &target_root).unwrap_err();

        assert!(error.contains("Refusing to remove symlink pointing elsewhere"));
        assert!(fs::symlink_metadata(target_root.join("demo"))
            .unwrap()
            .file_type()
            .is_symlink());
    }

    #[test]
    fn managed_state_is_first_use_when_managed_store_has_no_skills() {
        let root = temp_dir("managed-state-empty");
        let state = managed_state(&root.join("SkillBox")).unwrap();

        assert!(state.is_first_use);
        assert_eq!(state.skills.len(), 0);
    }

    #[test]
    fn managed_state_lists_remote_skill_current_once() {
        let root = temp_dir("managed-state-remote-once");
        let source = root.join("runtime").join("find-skills");
        let managed_root = root.join("SkillBox");
        make_skill(&source, "find-skills", "Find skills");
        import_skill(&source, SkillKind::Remote, &managed_root).unwrap();

        let state = managed_state(&managed_root).unwrap();

        assert_eq!(state.skills.len(), 1);
        assert_eq!(state.skills[0].name, "find-skills");
        assert_eq!(state.skills[0].kind, SkillKind::Remote);
        assert!(state.skills[0].path.ends_with("current"));
    }

    #[test]
    fn managed_state_infers_workspace_symlink_deployments_without_index() {
        let root = temp_dir("managed-state-inferred-deployment");
        let source = root.join("source").join("ui-ux-pro-max");
        let managed_root = root.join("SkillBox");
        let workspace_root = root
            .join("audio-dialogue-web")
            .join(".codex")
            .join("skills");
        make_skill(&source, "ui-ux-pro-max", "UI UX skill");
        import_skill(&source, SkillKind::Remote, &managed_root).unwrap();
        fs::create_dir_all(&workspace_root).unwrap();
        add_workspace(
            WorkspaceAddRequest {
                path: workspace_root.clone(),
                kind: WorkspaceKind::User,
            },
            &managed_root,
        )
        .unwrap();
        let managed_current = managed_root
            .join("remote-skills")
            .join("ui-ux-pro-max")
            .join("current");
        symlink_dir(&managed_current, &workspace_root.join("ui-ux-pro-max")).unwrap();

        let state = managed_state(&managed_root).unwrap();

        assert_eq!(state.skills.len(), 1);
        assert_eq!(state.skills[0].deployments.len(), 1);
        assert_eq!(state.skills[0].deployments[0].target_root, workspace_root);
        assert_eq!(
            state.skills[0].deployments[0].target_path,
            state.skills[0].deployments[0]
                .target_root
                .join("ui-ux-pro-max")
        );
        assert_eq!(state.skills[0].deployments[0].mode, "symlink");
    }

    #[test]
    fn managed_state_detects_workspace_alias_symlink_deployment() {
        let root = temp_dir("managed-state-alias-deployment");
        let source = root.join("source").join("dida-task-sync");
        let managed_root = root.join("SkillBox");
        let workspace_root = root.join("Pandora").join(".agents").join("skills");
        make_skill(&source, "dida-task-sync", "Dida sync skill");
        let imported = import_skill(&source, SkillKind::User, &managed_root).unwrap();
        fs::create_dir_all(&workspace_root).unwrap();
        add_workspace(
            WorkspaceAddRequest {
                path: workspace_root.clone(),
                kind: WorkspaceKind::User,
            },
            &managed_root,
        )
        .unwrap();
        let alias_path = workspace_root.join("dida-task-sync 2");
        symlink_dir(&imported.managed_path, &alias_path).unwrap();

        let state = managed_state(&managed_root).unwrap();

        assert_eq!(state.skills.len(), 1);
        assert_eq!(state.skills[0].deployments.len(), 1);
        assert_eq!(state.skills[0].deployments[0].target_root, workspace_root);
        assert_eq!(state.skills[0].deployments[0].target_path, alias_path);
        assert_eq!(state.skills[0].deployments[0].mode, "symlink");
    }

    #[test]
    fn managed_preferences_default_to_showing_local_import_confirmation() {
        let root = temp_dir("preferences-default");
        let preferences = managed_preferences(&root.join("SkillBox")).unwrap();

        assert!(!preferences.skip_local_import_confirmation);
        assert_eq!(preferences.status_refresh_interval_minutes, 5);
        assert_eq!(preferences.remote_update_timeout_seconds, 30);
    }

    #[test]
    fn managed_preferences_persist_skip_local_import_confirmation() {
        let root = temp_dir("preferences-persist");
        let managed_root = root.join("SkillBox");

        set_skip_local_import_confirmation(&managed_root, true).unwrap();
        let preferences = managed_preferences(&managed_root).unwrap();

        assert!(preferences.skip_local_import_confirmation);
        assert_eq!(preferences.status_refresh_interval_minutes, 5);
        assert_eq!(preferences.remote_update_timeout_seconds, 30);
    }

    #[test]
    fn managed_preferences_persist_status_refresh_interval() {
        let root = temp_dir("preferences-refresh-interval");
        let managed_root = root.join("SkillBox");

        let preferences = set_status_refresh_interval_minutes(&managed_root, 10).unwrap();

        assert_eq!(preferences.status_refresh_interval_minutes, 10);
        assert_eq!(
            managed_preferences(&managed_root)
                .unwrap()
                .status_refresh_interval_minutes,
            10
        );
    }

    #[test]
    fn managed_preferences_reject_invalid_status_refresh_interval() {
        let root = temp_dir("preferences-invalid-refresh-interval");
        let managed_root = root.join("SkillBox");

        let error = set_status_refresh_interval_minutes(&managed_root, 0).unwrap_err();

        assert!(error.contains("between 1 and 1440"));
    }

    #[test]
    fn managed_preferences_persist_remote_update_timeout() {
        let root = temp_dir("preferences-remote-timeout");
        let managed_root = root.join("SkillBox");

        let preferences = set_remote_update_timeout_seconds(&managed_root, 45).unwrap();

        assert_eq!(preferences.remote_update_timeout_seconds, 45);
        assert_eq!(
            managed_preferences(&managed_root)
                .unwrap()
                .remote_update_timeout_seconds,
            45
        );
    }

    #[test]
    fn managed_preferences_reject_invalid_remote_update_timeout() {
        let root = temp_dir("preferences-invalid-remote-timeout");
        let managed_root = root.join("SkillBox");

        let error = set_remote_update_timeout_seconds(&managed_root, 4).unwrap_err();

        assert!(error.contains("between 5 and 300"));
    }

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
                payload: serde_json::json!({
                    "sourceUrl": "https://github.com/acme/skills/tree/main/find-skills"
                }),
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
                payload: serde_json::json!({"validation": "same_skill_changed"}),
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
                payload: serde_json::json!({
                    "fromVersion": "manual-abc",
                    "toVersion": "123"
                }),
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
                payload: serde_json::json!({"restoredCurrent": true}),
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
                payload: serde_json::json!({"action": "rollback"}),
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
                payload: serde_json::json!({"cancelledBy": "user"}),
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

    #[test]
    fn user_skills_git_status_is_not_configured_without_origin() {
        let managed_root = temp_dir("user-skills-status").join("SkillBox");
        let status = user_skills_git_status(&managed_root).unwrap();

        assert_eq!(status.state, UserSkillsGitState::NotConfigured);
        assert!(!status.initialized);
        assert!(status.remote_url.is_none());
    }

    #[test]
    fn set_user_skills_git_remote_initializes_repo_and_sets_origin() {
        let managed_root = temp_dir("user-skills-remote-settings").join("SkillBox");
        let remote = bare_remote("user-skills-remote-settings-origin");
        let remote_url = remote.to_string_lossy().to_string();

        let status = set_user_skills_git_remote(
            UserSkillsGitRemoteRequest {
                remote_url: remote_url.clone(),
            },
            &managed_root,
        )
        .unwrap();

        assert!(status.initialized);
        assert_eq!(status.state, UserSkillsGitState::Clean);
        assert_eq!(status.remote_url.as_deref(), Some(remote_url.as_str()));
    }

    #[test]
    fn sync_user_skills_initializes_shared_repo_and_commits_all_skills() {
        let root = temp_dir("user-skills-sync");
        let managed_root = root.join("SkillBox");
        let paths = ensure_managed_layout(&managed_root).unwrap();
        make_skill(
            &paths.user_skills_root.join("alpha"),
            "alpha",
            "Alpha skill",
        );
        make_skill(&paths.user_skills_root.join("beta"), "beta", "Beta skill");
        let remote = bare_remote("user-skills-sync-remote");

        let result = sync_user_skills_git(
            UserSkillsSyncRequest {
                remote_url: Some(remote.to_string_lossy().to_string()),
                commit_message: Some("Sync user skills".to_string()),
                push: true,
                selected_paths: None,
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
        let root = temp_dir("user-skills-push-fail");
        let managed_root = root.join("SkillBox");
        let paths = ensure_managed_layout(&managed_root).unwrap();
        make_skill(
            &paths.user_skills_root.join("alpha"),
            "alpha",
            "Alpha skill",
        );

        let result = sync_user_skills_git(
            UserSkillsSyncRequest {
                remote_url: Some("/no/such/remote.git".to_string()),
                commit_message: Some("Sync user skills".to_string()),
                push: true,
                selected_paths: None,
            },
            &managed_root,
        )
        .unwrap();

        assert!(result.committed);
        assert!(!result.pushed);
        assert_eq!(result.state, UserSkillsGitState::PushFailed);
        assert!(result.message.contains("push"));
    }

    #[test]
    fn user_skills_git_changes_include_files_and_diff() {
        let root = temp_dir("user-skills-changes");
        let managed_root = root.join("SkillBox");
        let paths = ensure_managed_layout(&managed_root).unwrap();
        make_skill(
            &paths.user_skills_root.join("alpha"),
            "alpha",
            "Alpha skill",
        );
        sync_user_skills_git(
            UserSkillsSyncRequest {
                remote_url: None,
                commit_message: Some("Initial user skills".to_string()),
                push: false,
                selected_paths: None,
            },
            &managed_root,
        )
        .unwrap();
        fs::write(
            paths.user_skills_root.join("alpha").join("SKILL.md"),
            "---\nname: alpha\ndescription: Updated alpha skill\n---\n",
        )
        .unwrap();
        make_skill(&paths.user_skills_root.join("beta"), "beta", "Beta skill");

        let changes = user_skills_git_changes(&managed_root).unwrap();

        let paths: Vec<_> = changes
            .files
            .iter()
            .map(|file| file.path.as_str())
            .collect();
        assert!(paths.contains(&"alpha/SKILL.md"));
        assert!(paths.contains(&"beta/SKILL.md"));
        assert!(changes
            .files
            .iter()
            .any(|file| file.path == "alpha/SKILL.md" && file.diff.contains("Updated alpha")));
        assert!(changes
            .files
            .iter()
            .any(|file| file.path == "beta/SKILL.md" && file.diff.contains("Beta skill")));
    }

    #[test]
    fn user_skills_git_status_reports_changed_paths() {
        let root = temp_dir("user-skills-status-changed-paths");
        let managed_root = root.join("SkillBox");
        let paths = ensure_managed_layout(&managed_root).unwrap();
        make_skill(
            &paths.user_skills_root.join("alpha"),
            "alpha",
            "Alpha skill",
        );
        make_skill(&paths.user_skills_root.join("beta"), "beta", "Beta skill");
        let remote = bare_remote("user-skills-status-changed-paths-origin");
        sync_user_skills_git(
            UserSkillsSyncRequest {
                remote_url: Some(remote.to_string_lossy().to_string()),
                commit_message: Some("Initial user skills".to_string()),
                push: false,
                selected_paths: None,
            },
            &managed_root,
        )
        .unwrap();
        fs::write(
            paths.user_skills_root.join("alpha").join("SKILL.md"),
            "---\nname: alpha\ndescription: Updated alpha skill\n---\n",
        )
        .unwrap();

        let status = user_skills_git_status(&managed_root).unwrap();

        assert_eq!(status.state, UserSkillsGitState::Dirty);
        assert_eq!(status.changed_paths, vec!["alpha/SKILL.md".to_string()]);
    }

    #[test]
    fn sync_user_skills_commits_only_selected_paths() {
        let root = temp_dir("user-skills-selected-sync");
        let managed_root = root.join("SkillBox");
        let paths = ensure_managed_layout(&managed_root).unwrap();
        make_skill(
            &paths.user_skills_root.join("alpha"),
            "alpha",
            "Alpha skill",
        );
        make_skill(&paths.user_skills_root.join("beta"), "beta", "Beta skill");
        let remote = bare_remote("user-skills-selected-sync-remote");
        sync_user_skills_git(
            UserSkillsSyncRequest {
                remote_url: Some(remote.to_string_lossy().to_string()),
                commit_message: Some("Initial user skills".to_string()),
                push: false,
                selected_paths: None,
            },
            &managed_root,
        )
        .unwrap();
        fs::write(
            paths.user_skills_root.join("alpha").join("SKILL.md"),
            "---\nname: alpha\ndescription: Updated alpha skill\n---\n",
        )
        .unwrap();
        fs::write(
            paths.user_skills_root.join("beta").join("SKILL.md"),
            "---\nname: beta\ndescription: Updated beta skill\n---\n",
        )
        .unwrap();

        let result = sync_user_skills_git(
            UserSkillsSyncRequest {
                remote_url: None,
                commit_message: Some("Sync selected user skill".to_string()),
                push: false,
                selected_paths: Some(vec!["alpha/SKILL.md".to_string()]),
            },
            &managed_root,
        )
        .unwrap();

        assert!(result.committed);
        assert_eq!(result.state, UserSkillsGitState::Dirty);
        assert!(result.raw_status.contains("beta/SKILL.md"));
        assert!(!result.raw_status.contains("alpha/SKILL.md"));
    }

    #[test]
    fn user_skill_versions_include_current_worktree_and_git_history() {
        let root = temp_dir("user-skill-versions");
        let managed_root = root.join("SkillBox");
        let paths = ensure_managed_layout(&managed_root).unwrap();
        make_skill_with_body(
            &paths.user_skills_root.join("alpha"),
            "alpha",
            "Alpha skill",
            "version one\n",
        );
        make_skill_with_body(
            &paths.user_skills_root.join("beta"),
            "beta",
            "Beta skill",
            "beta version\n",
        );
        sync_user_skills_git(
            UserSkillsSyncRequest {
                remote_url: None,
                commit_message: Some("Initial user skills".to_string()),
                push: false,
                selected_paths: None,
            },
            &managed_root,
        )
        .unwrap();
        make_skill_with_body(
            &paths.user_skills_root.join("alpha"),
            "alpha",
            "Alpha skill",
            "version two\n",
        );
        sync_user_skills_git(
            UserSkillsSyncRequest {
                remote_url: None,
                commit_message: Some("Update alpha skill".to_string()),
                push: false,
                selected_paths: Some(vec!["alpha/SKILL.md".to_string()]),
            },
            &managed_root,
        )
        .unwrap();
        make_skill_with_body(
            &paths.user_skills_root.join("alpha"),
            "alpha",
            "Alpha skill",
            "work in progress\n",
        );

        let versions = list_user_skill_versions("alpha", &managed_root).unwrap();

        assert_eq!(versions.skill_name, "alpha");
        assert_eq!(versions.versions.len(), 3);
        assert!(versions.versions[0].is_current);
        assert_eq!(versions.versions[0].kind, "working");
        assert_eq!(versions.current_version, versions.versions[0].version);
        assert_eq!(versions.versions[1].kind, "git");
        assert_eq!(
            versions.versions[1].message.as_deref(),
            Some("Update alpha skill")
        );
        assert_eq!(
            versions.versions[2].message.as_deref(),
            Some("Initial user skills")
        );
        assert!(!versions
            .versions
            .iter()
            .any(|version| version.message.as_deref() == Some("Beta skill")));
    }

    #[test]
    fn check_remote_skill_updates_reports_update_available_and_up_to_date() {
        let root = temp_dir("remote-updates");
        let managed_root = root.join("SkillBox");
        let paths = ensure_managed_layout(&managed_root).unwrap();
        let remote = bare_remote_with_main("remote-updates-origin");
        let latest_sha = remote_head(&remote);

        write_remote_source(
            &paths.remote_skills_root.join("fresh"),
            &remote,
            &latest_sha,
        );
        write_remote_source(
            &paths.remote_skills_root.join("stale"),
            &remote,
            "0000000000000000000000000000000000000000",
        );

        let result = check_remote_skill_updates(&managed_root).unwrap();
        let fresh = remote_status(&result.statuses, "fresh");
        let stale = remote_status(&result.statuses, "stale");

        assert_eq!(fresh.state, RemoteSkillUpdateState::UpToDate);
        assert!(!fresh.update_available);
        assert_eq!(fresh.latest_sha.as_deref(), Some(latest_sha.as_str()));
        assert_eq!(stale.state, RemoteSkillUpdateState::UpdateAvailable);
        assert!(stale.update_available);
        assert_eq!(stale.latest_sha.as_deref(), Some(latest_sha.as_str()));
    }

    #[test]
    fn check_remote_skill_updates_marks_missing_source_separately_from_not_checkable() {
        let root = temp_dir("remote-not-checkable");
        let managed_root = root.join("SkillBox");
        let paths = ensure_managed_layout(&managed_root).unwrap();
        fs::create_dir_all(paths.remote_skills_root.join("missing-source")).unwrap();
        fs::create_dir_all(paths.remote_skills_root.join("manual-source")).unwrap();
        fs::write(
            paths
                .remote_skills_root
                .join("manual-source")
                .join("source.json"),
            r#"{"type":"manual","installedSha":"manual-abc123"}"#,
        )
        .unwrap();

        let result = check_remote_skill_updates(&managed_root).unwrap();
        let missing = remote_status(&result.statuses, "missing-source");
        let manual = remote_status(&result.statuses, "manual-source");

        assert_eq!(missing.state, RemoteSkillUpdateState::NoSource);
        assert_eq!(manual.state, RemoteSkillUpdateState::NotCheckable);
        assert!(!missing.update_available);
        assert!(!manual.update_available);
    }

    #[test]
    fn cached_remote_skill_updates_reuses_last_check_and_marks_missing_sources() {
        let root = temp_dir("remote-update-cache");
        let managed_root = root.join("SkillBox");
        let paths = ensure_managed_layout(&managed_root).unwrap();
        let remote = bare_remote_with_main("remote-update-cache-origin");
        let latest_sha = remote_head(&remote);

        write_remote_source(
            &paths.remote_skills_root.join("fresh"),
            &remote,
            &latest_sha,
        );
        fs::create_dir_all(paths.remote_skills_root.join("missing-source")).unwrap();

        let checked = check_remote_skill_updates(&managed_root).unwrap();
        let cached = cached_remote_skill_updates(&managed_root).unwrap();
        let fresh = remote_status(&cached.statuses, "fresh");
        let missing = remote_status(&cached.statuses, "missing-source");

        assert_eq!(cached.checked_at, checked.checked_at);
        assert_eq!(fresh.state, RemoteSkillUpdateState::UpToDate);
        assert_eq!(fresh.latest_sha.as_deref(), Some(latest_sha.as_str()));
        assert_eq!(missing.state, RemoteSkillUpdateState::NoSource);
    }

    #[test]
    fn check_remote_skill_updates_records_git_failures_per_skill() {
        let root = temp_dir("remote-check-failed");
        let managed_root = root.join("SkillBox");
        let paths = ensure_managed_layout(&managed_root).unwrap();
        write_remote_source(
            &paths.remote_skills_root.join("broken"),
            &root.join("missing.git"),
            "0000000000000000000000000000000000000000",
        );

        let result = check_remote_skill_updates(&managed_root).unwrap();
        let broken = remote_status(&result.statuses, "broken");

        assert_eq!(broken.state, RemoteSkillUpdateState::CheckFailed);
        assert!(!broken.update_available);
        assert!(broken.message.as_deref().unwrap_or("").contains("Git"));
    }

    #[test]
    fn check_remote_skill_update_preserves_cached_success_on_failure() {
        let root = temp_dir("remote-check-preserve-cache");
        let managed_root = root.join("SkillBox");
        let paths = ensure_managed_layout(&managed_root).unwrap();
        let remote = bare_remote_with_main("remote-check-preserve-cache-origin");
        let latest_sha = remote_head(&remote);
        let skill_root = paths.remote_skills_root.join("fresh");
        write_remote_source(&skill_root, &remote, &latest_sha);

        let checked = check_remote_skill_updates(&managed_root).unwrap();
        assert_eq!(
            remote_status(&checked.statuses, "fresh").state,
            RemoteSkillUpdateState::UpToDate
        );
        write_remote_source(&skill_root, &root.join("missing.git"), &latest_sha);

        let failed = check_remote_skill_updates(&managed_root).unwrap();
        let fresh = remote_status(&failed.statuses, "fresh");

        assert_eq!(fresh.state, RemoteSkillUpdateState::UpToDate);
        assert_eq!(fresh.latest_sha.as_deref(), Some(latest_sha.as_str()));
        assert!(fresh
            .message
            .as_deref()
            .unwrap_or("")
            .starts_with("Last check failed: Git update check failed:"));
    }

    #[test]
    fn check_single_remote_skill_update_only_refreshes_requested_skill() {
        let root = temp_dir("remote-check-one");
        let managed_root = root.join("SkillBox");
        let paths = ensure_managed_layout(&managed_root).unwrap();
        let remote = bare_remote_with_main("remote-check-one-origin");
        let latest_sha = remote_head(&remote);
        write_remote_source(
            &paths.remote_skills_root.join("target"),
            &remote,
            "0000000000000000000000000000000000000000",
        );
        write_remote_source(
            &paths.remote_skills_root.join("other"),
            &remote,
            &latest_sha,
        );
        check_remote_skill_updates(&managed_root).unwrap();
        write_remote_source(
            &paths.remote_skills_root.join("other"),
            &root.join("missing.git"),
            &latest_sha,
        );

        let result = check_remote_skill_update(&managed_root, "target").unwrap();
        let target = remote_status(&result.statuses, "target");
        let other = remote_status(&result.statuses, "other");

        assert_eq!(target.state, RemoteSkillUpdateState::UpdateAvailable);
        assert_eq!(other.state, RemoteSkillUpdateState::UpToDate);
        assert_eq!(other.message, None);
    }

    #[test]
    fn check_remote_skill_updates_uses_limited_concurrency() {
        let source = include_str!("lib.rs");
        let check_start = source.find("pub fn check_remote_skill_updates").unwrap();
        let cached_start = source.find("pub fn cached_remote_skill_updates").unwrap();
        let check_source = &source[check_start..cached_start];

        assert!(source.contains("const REMOTE_UPDATE_CHECK_CONCURRENCY: usize = 3;"));
        assert!(check_source.contains("check_remote_skill_update_batch"));
        assert!(check_source.contains("REMOTE_UPDATE_CHECK_CONCURRENCY"));
    }

    #[test]
    fn check_remote_skill_updates_marks_pinned_sources() {
        let root = temp_dir("remote-pinned-sources");
        let managed_root = root.join("SkillBox");
        let paths = ensure_managed_layout(&managed_root).unwrap();

        write_remote_source_with_json(
            &paths.remote_skills_root.join("tagged"),
            r#"{
              "type":"github",
              "url":"https://github.com/acme/skills/tree/v1.0.0/skills/tagged",
              "repoUrl":"https://github.com/acme/skills.git",
              "ref":"v1.0.0",
              "refKind":"tag",
              "tracking":true,
              "currentVersion":"0123456789abcdef0123456789abcdef01234567",
              "installedSha":"0123456789abcdef0123456789abcdef01234567"
            }"#,
        );
        write_remote_source_with_json(
            &paths.remote_skills_root.join("commit"),
            r#"{
              "type":"github",
              "repoUrl":"https://github.com/acme/skills.git",
              "ref":"0123456789abcdef0123456789abcdef01234567",
              "currentVersion":"0123456789abcdef0123456789abcdef01234567",
              "installedSha":"0123456789abcdef0123456789abcdef01234567"
            }"#,
        );

        let result = check_remote_skill_updates(&managed_root).unwrap();
        let tagged = remote_status(&result.statuses, "tagged");
        assert_eq!(tagged.state, RemoteSkillUpdateState::Pinned);
        assert!(!tagged.update_available);
        assert_eq!(
            tagged.source_url.as_deref(),
            Some("https://github.com/acme/skills/tree/v1.0.0/skills/tagged")
        );
        assert_eq!(tagged.message.as_deref(), Some("Pinned GitHub source."));
        assert!(!tagged.tracking);

        let commit = remote_status(&result.statuses, "commit");
        assert_eq!(commit.state, RemoteSkillUpdateState::Pinned);
        assert_eq!(commit.ref_kind.as_deref(), Some("commit"));
        assert!(!commit.tracking);
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
        assert_eq!(
            bound.current_version.as_deref(),
            Some("manual-abc123def456")
        );
        assert_eq!(bound.installed_sha, None);
    }

    #[test]
    fn source_binding_preview_detects_exact_match() {
        let root = temp_dir("source-binding-exact");
        let managed_root = root.join("SkillBox");
        let source = root.join("local").join("demo");
        make_skill(&source, "demo", "Demo skill");
        import_skill(&source, SkillKind::Remote, &managed_root).unwrap();
        let remote =
            bare_remote_with_skill_content("source-binding-exact-origin", "demo", "Demo skill", "");
        let _rewrite = github_repo_rewrite("acme", "source-binding-exact", &remote);

        let preview = preview_remote_source_binding(
            RemoteSourceBindingRequest {
                skill_name: "demo".to_string(),
                source_url: github_source_url("acme", "source-binding-exact", "demo"),
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
    fn source_binding_preview_resolves_marketplace_skill_path() {
        let root = temp_dir("source-binding-marketplace-path");
        let managed_root = root.join("SkillBox");
        let source = root.join("local").join("find-skills");
        make_skill(&source, "find-skills", "Find skills");
        import_skill(&source, SkillKind::Remote, &managed_root).unwrap();
        let remote = bare_remote_with_skill_content(
            "source-binding-marketplace-path-origin",
            "find-skills",
            "Find skills",
            "",
        );
        let _rewrite = github_repo_rewrite("acme", "source-binding-marketplace-path", &remote);

        let preview = preview_remote_source_binding(
            RemoteSourceBindingRequest {
                skill_name: "find-skills".to_string(),
                source_url:
                    "https://github.com/acme/source-binding-marketplace-path/tree/main/find-skills"
                        .to_string(),
                actor: "cli".to_string(),
            },
            &managed_root,
        )
        .unwrap();

        assert_eq!(preview.path, "skills/find-skills");
        assert_eq!(
            preview.source_url,
            "https://github.com/acme/source-binding-marketplace-path/tree/main/skills/find-skills"
        );
        assert_eq!(preview.validation, SourceBindingValidation::ExactMatch);
    }

    #[test]
    fn source_binding_changed_source_does_not_switch_current() {
        let root = temp_dir("source-binding-changed");
        let managed_root = root.join("SkillBox");
        let paths = ensure_managed_layout(&managed_root).unwrap();
        let source = root.join("local").join("find-skills");
        make_skill(&source, "find-skills", "Find skills");
        let imported = import_skill(&source, SkillKind::Remote, &managed_root).unwrap();
        let before_current =
            fs::read_link(paths.remote_skills_root.join("find-skills").join("current")).unwrap();
        let remote = bare_remote_with_skill_content(
            "source-binding-changed-origin",
            "find-skills",
            "Find skills",
            "Updated body\n",
        );
        let _rewrite = github_repo_rewrite("acme", "source-binding-changed", &remote);
        let source_url = github_source_url("acme", "source-binding-changed", "find-skills");
        let preview = preview_remote_source_binding(
            RemoteSourceBindingRequest {
                skill_name: "find-skills".to_string(),
                source_url: source_url.clone(),
                actor: "desktop".to_string(),
            },
            &managed_root,
        )
        .unwrap();

        assert_eq!(
            preview.validation,
            SourceBindingValidation::SameSkillChanged
        );
        let result = bind_remote_source(
            BindRemoteSourceRequest {
                skill_name: "find-skills".to_string(),
                source_url,
                actor: "desktop".to_string(),
            },
            &managed_root,
        )
        .unwrap();

        let after_current =
            fs::read_link(paths.remote_skills_root.join("find-skills").join("current")).unwrap();
        assert_eq!(after_current, before_current);
        assert_eq!(result.validation, SourceBindingValidation::SameSkillChanged);
        assert!(result.source_path.exists());
        let source_json: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&result.source_path).unwrap()).unwrap();
        assert_eq!(source_json["type"], "github");
        assert_eq!(source_json["refKind"], "branch");
        assert_eq!(source_json["tracking"], true);
        assert_eq!(
            source_json["currentVersion"],
            before_current
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap()
        );
        let latest_sha = result.latest_sha.clone().unwrap();
        assert!(!paths
            .remote_skills_root
            .join("find-skills")
            .join("versions")
            .join(latest_sha)
            .exists());
        assert!(imported.managed_path.exists());
        let operations = list_operations(OperationFilter::default(), &managed_root).unwrap();
        assert!(operations
            .operations
            .iter()
            .any(|operation| operation.operation_type == "bind_remote_source"
                && operation.status == OperationStatus::Succeeded));
    }

    #[test]
    fn source_binding_preview_rejects_name_mismatch() {
        let root = temp_dir("source-binding-mismatch");
        let managed_root = root.join("SkillBox");
        let source = root.join("local").join("alpha");
        make_skill(&source, "alpha", "Alpha skill");
        import_skill(&source, SkillKind::Remote, &managed_root).unwrap();
        let remote = bare_remote_with_skill_content(
            "source-binding-mismatch-origin",
            "beta",
            "Beta skill",
            "",
        );
        let _rewrite = github_repo_rewrite("acme", "source-binding-mismatch", &remote);

        let preview = preview_remote_source_binding(
            RemoteSourceBindingRequest {
                skill_name: "alpha".to_string(),
                source_url: github_source_url("acme", "source-binding-mismatch", "beta"),
                actor: "cli".to_string(),
            },
            &managed_root,
        )
        .unwrap();

        assert_eq!(preview.validation, SourceBindingValidation::Mismatch);
        assert!(preview
            .message
            .contains("Remote skill name beta does not match alpha"));

        let error = bind_remote_source(
            BindRemoteSourceRequest {
                skill_name: "alpha".to_string(),
                source_url: github_source_url("acme", "source-binding-mismatch", "beta"),
                actor: "cli".to_string(),
            },
            &managed_root,
        )
        .unwrap_err();
        assert!(error.contains("Remote skill name beta does not match alpha"));
        let operations = list_operations(OperationFilter::default(), &managed_root).unwrap();
        assert!(operations
            .operations
            .iter()
            .any(|operation| operation.operation_type == "bind_remote_source"
                && operation.status == OperationStatus::Failed));
    }

    #[test]
    fn remote_version_list_marks_current() {
        let root = temp_dir("remote-version-list");
        let managed_root = root.join("SkillBox");
        let source = root.join("local").join("demo");
        make_skill(&source, "demo", "Demo skill");
        import_skill(&source, SkillKind::Remote, &managed_root).unwrap();

        let versions = list_remote_skill_versions("demo", &managed_root).unwrap();

        assert_eq!(versions.skill_name, "demo");
        assert_eq!(versions.versions.len(), 1);
        assert!(versions.versions[0].is_current);
        assert!(versions.versions[0].version.starts_with("manual-"));
        assert!(!versions.versions[0].updated_at.is_empty());
        assert!(versions.versions[0]
            .updated_at
            .chars()
            .all(|character| character.is_ascii_digit()));
    }

    #[test]
    fn remote_version_preview_rollback_lists_every_changed_file() {
        let root = temp_dir("remote-preview-rollback");
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
        fs::write(
            v2_path.join("SKILL.md"),
            "---\nname: demo\ndescription: Demo skill\n---\nupdated\n",
        )
        .unwrap();
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

        assert_eq!(preview.from_version, v2);
        assert_eq!(preview.to_version, v1);
        assert!(preview.files.iter().any(|file| file.path == "SKILL.md"));
        assert!(preview.files.iter().any(|file| file.path == "extra.txt"));
        assert!(preview
            .files
            .iter()
            .any(|file| file.path == "extra.txt" && file.diff.contains("-extra")));
    }

    #[test]
    fn read_remote_source_rejects_untrusted_github_metadata() {
        let root = temp_dir("remote-source-validation");
        let remote_root = root.join("remote-skills").join("demo");

        write_remote_source_with_json(
            &remote_root,
            r#"{
              "type":"github",
              "repoUrl":"file:///tmp/repo.git",
              "ref":"main",
              "path":"skills/demo"
            }"#,
        );

        let error = read_remote_source(&remote_root).unwrap_err();
        assert!(error.contains("Only https://github.com remote URLs are supported"));

        write_remote_source_with_json(
            &remote_root,
            r#"{
              "type":"github",
              "repoUrl":"https://github.com/acme/repo.git",
              "ref":"main",
              "path":"skills/../../secret"
            }"#,
        );

        let error = read_remote_source(&remote_root).unwrap_err();
        assert!(error.contains("path must stay inside the repository"));
    }

    #[test]
    fn update_current_symlink_refuses_existing_non_symlink() {
        let root = temp_dir("current-non-symlink");
        let remote_root = root.join("remote");
        let version = remote_root.join("versions").join("v1");
        fs::create_dir_all(&version).unwrap();
        fs::create_dir_all(&remote_root).unwrap();
        fs::write(remote_root.join("current"), "not a symlink").unwrap();

        let error = update_current_symlink(&remote_root, &version).unwrap_err();

        assert!(error.contains("Refusing to replace existing non-symlink current"));
        assert_eq!(
            fs::read_to_string(remote_root.join("current")).unwrap(),
            "not a symlink"
        );
    }

    #[test]
    fn copy_skill_dir_rejects_symlinks_that_escape_source_root() {
        let root = temp_dir("copy-symlink-escape");
        let source = root.join("source");
        let outside = root.join("outside");
        let destination = root.join("destination");
        make_skill(&source, "demo", "Demo skill");
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("secret.txt"), "secret").unwrap();
        symlink_any(&outside.join("secret.txt"), &source.join("secret-link")).unwrap();

        let error = copy_skill_dir(&source, &destination).unwrap_err();

        assert!(error.contains("Refusing to copy symlink outside source root"));
        assert!(!destination.exists());
    }

    #[test]
    fn copy_skill_dir_preserves_internal_broken_symlink() {
        let root = temp_dir("copy-broken-symlink");
        let source = root.join("source");
        let destination = root.join("destination");
        make_skill(&source, "demo", "Demo skill");
        symlink_any(Path::new("missing.txt"), &source.join("missing-link")).unwrap();

        copy_skill_dir(&source, &destination).unwrap();

        assert!(fs::symlink_metadata(destination.join("missing-link"))
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(
            fs::read_link(destination.join("missing-link")).unwrap(),
            PathBuf::from("missing.txt")
        );
    }

    #[test]
    fn remote_version_preview_keeps_binary_file_metadata() {
        let root = temp_dir("remote-preview-binary");
        let managed_root = root.join("SkillBox");
        let paths = ensure_managed_layout(&managed_root).unwrap();
        let source_v1 = root.join("local-v1").join("demo");
        make_skill(&source_v1, "demo", "Demo skill");
        import_skill(&source_v1, SkillKind::Remote, &managed_root).unwrap();
        let v1 = current_remote_version(&paths, "demo").unwrap();
        let remote_root = paths.remote_skills_root.join("demo");
        let v2 = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let v2_path = remote_root.join("versions").join(v2);
        copy_skill_dir(&source_v1, &v2_path).unwrap();
        fs::write(v2_path.join("asset.bin"), [0xff, 0x00, 0x10]).unwrap();
        update_current_symlink(&remote_root, &v2_path).unwrap();

        let preview = preview_remote_version_change(
            RemoteVersionChangeRequest {
                skill_name: "demo".to_string(),
                action: RemoteVersionChangeAction::Rollback,
                target_version: Some(v1),
                actor: "cli".to_string(),
            },
            &managed_root,
        )
        .unwrap();

        let binary = preview
            .files
            .iter()
            .find(|file| file.path == "asset.bin")
            .unwrap();
        assert!(binary.binary);
        assert_eq!(binary.old_size, Some(3));
        assert!(binary.old_hash.is_some());
        assert_eq!(binary.diff, "");
    }

    #[test]
    fn remote_version_preview_update_uses_temp_snapshot_without_installing_version() {
        let root = temp_dir("remote-preview-update");
        let managed_root = root.join("SkillBox");
        let paths = ensure_managed_layout(&managed_root).unwrap();
        let source = root.join("local").join("find-skills");
        make_skill(&source, "find-skills", "Find skills");
        import_skill(&source, SkillKind::Remote, &managed_root).unwrap();
        let remote = bare_remote_with_skill_content(
            "remote-preview-update-origin",
            "find-skills",
            "Find skills",
            "Updated remote body\n",
        );
        let _rewrite = github_repo_rewrite("acme", "remote-preview-update", &remote);
        bind_remote_source(
            BindRemoteSourceRequest {
                skill_name: "find-skills".to_string(),
                source_url: github_source_url("acme", "remote-preview-update", "find-skills"),
                actor: "cli".to_string(),
            },
            &managed_root,
        )
        .unwrap();
        let latest_sha = read_remote_source(&paths.remote_skills_root.join("find-skills"))
            .unwrap()
            .latest_sha
            .unwrap();

        let preview = preview_remote_version_change(
            RemoteVersionChangeRequest {
                skill_name: "find-skills".to_string(),
                action: RemoteVersionChangeAction::Update,
                target_version: None,
                actor: "cli".to_string(),
            },
            &managed_root,
        )
        .unwrap();

        assert_eq!(preview.to_version, latest_sha);
        assert!(preview.files.iter().any(|file| file.path == "SKILL.md"));
        assert!(!paths
            .remote_skills_root
            .join("find-skills")
            .join("versions")
            .join(&preview.to_version)
            .exists());
    }

    #[test]
    fn remote_version_preview_update_honors_explicit_target_version() {
        let root = temp_dir("remote-preview-update-explicit-target");
        let managed_root = root.join("SkillBox");
        let paths = ensure_managed_layout(&managed_root).unwrap();
        let source = root.join("local").join("find-skills");
        make_skill(&source, "find-skills", "Find skills");
        import_skill(&source, SkillKind::Remote, &managed_root).unwrap();
        let current_version = current_remote_version(&paths, "find-skills").unwrap();
        let remote_root = paths.remote_skills_root.join("find-skills");
        let target_version = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
        let target_path = remote_root.join("versions").join(target_version);
        copy_skill_dir(&source, &target_path).unwrap();
        fs::write(
            target_path.join("SKILL.md"),
            "---\nname: find-skills\ndescription: Find skills\n---\nUpdated body\n",
        )
        .unwrap();
        write_remote_source_with_json(
            &remote_root,
            &format!(
                r#"{{
                  "type":"github",
                  "currentVersion":"{current_version}",
                  "latestSha":"{current_version}"
                }}"#
            ),
        );

        let preview = preview_remote_version_change(
            RemoteVersionChangeRequest {
                skill_name: "find-skills".to_string(),
                action: RemoteVersionChangeAction::Update,
                target_version: Some(target_version.to_string()),
                actor: "cli".to_string(),
            },
            &managed_root,
        )
        .unwrap();

        assert_eq!(preview.from_version, current_version);
        assert_eq!(preview.to_version, target_version);
        assert!(preview.files.iter().any(|file| file.path == "SKILL.md"));
    }

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
        fs::write(
            v2_path.join("SKILL.md"),
            "---\nname: demo\ndescription: Demo skill\n---\nupdated\n",
        )
        .unwrap();
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
        assert_eq!(
            current_remote_version(&paths, "demo").unwrap(),
            result.to_version
        );
        let operations = list_operations(OperationFilter::default(), &managed_root).unwrap();
        assert!(operations
            .operations
            .iter()
            .any(
                |operation| operation.operation_type == "rollback_remote_skill"
                    && operation.status == OperationStatus::Succeeded
            ));
    }

    #[test]
    fn apply_remote_version_change_rejects_stale_preview_id() {
        let root = temp_dir("apply-stale-preview");
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
        fs::write(
            v2_path.join("SKILL.md"),
            "---\nname: demo\ndescription: Demo skill\n---\nupdated\n",
        )
        .unwrap();
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

        let error = apply_remote_version_change(
            RemoteVersionChangeApplyRequest {
                skill_name: "demo".to_string(),
                action: RemoteVersionChangeAction::Rollback,
                target_version: v1,
                preview_id: Some(format!("{}-stale", preview.preview_id)),
                actor: "cli".to_string(),
            },
            &managed_root,
        )
        .unwrap_err();

        assert!(error.contains("Remote version preview is stale"));
        assert_eq!(current_remote_version(&paths, "demo").unwrap(), v2);
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
        let _rewrite = github_repo_rewrite("acme", "apply-update", &remote);
        let source_url = github_source_url("acme", "apply-update", "find-skills");
        bind_remote_source(
            BindRemoteSourceRequest {
                skill_name: "find-skills".to_string(),
                source_url,
                actor: "cli".to_string(),
            },
            &managed_root,
        )
        .unwrap();
        let latest_sha = read_remote_source(&paths.remote_skills_root.join("find-skills"))
            .unwrap()
            .latest_sha
            .unwrap();

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
        assert!(paths
            .remote_skills_root
            .join("find-skills")
            .join("versions")
            .join(&old_version)
            .exists());
        assert!(paths
            .remote_skills_root
            .join("find-skills")
            .join("versions")
            .join(&result.to_version)
            .exists());
        assert_eq!(
            current_remote_version(&paths, "find-skills").unwrap(),
            result.to_version
        );
        let source = read_remote_source(&paths.remote_skills_root.join("find-skills")).unwrap();
        assert_eq!(
            source.current_version.as_deref(),
            Some(result.to_version.as_str())
        );
        assert_eq!(
            source.installed_sha.as_deref(),
            Some(result.to_version.as_str())
        );
        let operations = list_operations(OperationFilter::default(), &managed_root).unwrap();
        assert!(operations
            .operations
            .iter()
            .any(
                |operation| operation.operation_type == "update_remote_skill"
                    && operation.status == OperationStatus::Succeeded
            ));
    }

    #[test]
    fn source_candidates_rank_by_name_path_trust_and_popularity() {
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
                    source_url: "https://github.com/acme/skills/tree/main/skills/find-skills"
                        .to_string(),
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
        assert!(candidates[0]
            .match_reasons
            .contains(&"Exact skill name match".to_string()));
    }

    #[test]
    fn parses_claude_marketplace_skill_candidates_with_exact_name_priority() {
        let response = r#"[
          {
            "id": "vercel-labs/skills/find-skills",
            "name": "find-skills",
            "description": "Discover and install specialized agent skills.",
            "repo": "vercel-labs/skills",
            "path": "find-skills",
            "stars": 18600,
            "installs": 1500000,
            "installCommand": "npx skills add https://github.com/vercel-labs/skills --skill find-skills",
            "lastUpdated": "2026-05-16T17:00:48.907+00:00",
            "listingStatus": "listed"
          },
          {
            "id": "example/misc/find-skills-helper",
            "name": "find-skills-helper",
            "description": "Helper",
            "repo": "example/misc",
            "path": ".claude/skills/find-skills-helper/SKILL.md",
            "stars": 1,
            "installs": 1,
            "listingStatus": "listed"
          }
        ]"#;

        let candidates =
            parse_claude_marketplace_skill_candidates("find-skills", response).unwrap();

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].owner, "vercel-labs");
        assert_eq!(candidates[0].repo, "skills");
        assert_eq!(candidates[0].path, "find-skills");
        assert_eq!(
            candidates[0].source_url,
            "https://github.com/vercel-labs/skills/tree/main/find-skills"
        );
        assert!(candidates[0]
            .match_reasons
            .contains(&"Claude Marketplace listed skill".to_string()));
    }

    #[test]
    fn claude_marketplace_api_curl_args_target_skills_api() {
        let args = claude_marketplace_api_curl_args();

        assert_eq!(
            args.last().map(String::as_str),
            Some(CLAUDE_MARKETPLACE_SKILLS_API)
        );
        assert!(args.iter().any(|arg| arg == "Accept: application/json"));
    }

    #[test]
    fn scan_import_candidates_infers_type_from_path_and_metadata() {
        let root = temp_dir("candidate-type");
        let agents_root = root.join(".agents").join("skills");
        let codex_root = root.join(".codex").join("skills");
        let system_root = codex_root.join(".system");
        let misc_root = root.join("Downloads").join("skills");
        let managed_root = root.join("SkillBox");

        make_skill(&agents_root.join("local"), "local", "Local skill");
        make_skill(&codex_root.join("remote"), "remote", "Remote skill");
        make_skill(&system_root.join("system"), "system", "System skill");
        make_skill_with_body(
            &misc_root.join("github-skill"),
            "github-skill",
            "GitHub skill",
            "source: https://github.com/acme/skills/tree/main/github-skill",
        );
        make_skill(&misc_root.join("unknown"), "unknown", "Unknown skill");

        let candidates =
            scan_import_candidates(&[agents_root, codex_root, misc_root], &managed_root).unwrap();

        let local = candidate(&candidates.candidates, "local");
        assert_eq!(local.suggested_type, SkillKind::User);
        assert_eq!(local.suggestion_reason, "inside ~/.agents/skills");
        assert!(local.is_selected);

        let remote = candidate(&candidates.candidates, "remote");
        assert_eq!(remote.suggested_type, SkillKind::Remote);
        assert_eq!(remote.suggestion_reason, "inside ~/.codex/skills");
        assert!(remote.is_selected);

        let system = candidate(&candidates.candidates, "system");
        assert_eq!(system.suggested_type, SkillKind::Remote);
        assert_eq!(system.suggestion_reason, "inside ~/.codex/skills/.system");
        assert_eq!(system.import_status, ImportCandidateStatus::System);
        assert!(!system.is_selected);

        let github = candidate(&candidates.candidates, "github-skill");
        assert_eq!(github.suggested_type, SkillKind::Remote);
        assert_eq!(github.suggestion_reason, "GitHub source metadata found");
        assert!(github.is_selected);

        let unknown = candidate(&candidates.candidates, "unknown");
        assert_eq!(unknown.suggested_type, SkillKind::User);
        assert_eq!(unknown.suggestion_reason, "Needs confirm");
        assert!(unknown.is_selected);
    }

    #[test]
    fn scan_import_candidates_excludes_already_imported_skills_by_hash() {
        let root = temp_dir("candidate-excludes-imported");
        let source = root.join("runtime").join("demo");
        let managed_root = root.join("SkillBox");
        make_skill(&source, "demo", "Demo skill");
        import_skill(&source, SkillKind::User, &managed_root).unwrap();

        let candidates = scan_import_candidates(&[root.join("runtime")], &managed_root).unwrap();

        assert_eq!(candidates.candidates.len(), 1);
        let demo = candidate(&candidates.candidates, "demo");
        assert_eq!(demo.import_status, ImportCandidateStatus::Imported);
        assert!(!demo.is_selected);
    }

    #[test]
    fn import_candidates_copies_user_skill_backs_up_original_and_symlinks_source() {
        let root = temp_dir("candidate-import-user");
        let source = root.join("runtime").join("demo");
        let managed_root = root.join("SkillBox");
        make_skill(&source, "demo", "Demo skill");

        let result = import_candidates(
            vec![ImportRequestItem {
                source_path: source.clone(),
                skill_type: SkillKind::User,
                deploy_back_to_source: true,
            }],
            &managed_root,
        )
        .unwrap();

        assert_eq!(result.errors.len(), 0);
        assert_eq!(result.imported.len(), 1);
        let imported = &result.imported[0];
        assert_eq!(imported.name, "demo");
        assert!(imported
            .backup_path
            .as_ref()
            .unwrap()
            .join("SKILL.md")
            .exists());
        assert!(fs::symlink_metadata(&source)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(
            fs::canonicalize(&source).unwrap(),
            fs::canonicalize(managed_root.join("user-skills").join("demo")).unwrap()
        );
    }

    #[test]
    fn scan_import_candidates_skips_symlinked_sources() {
        let root = temp_dir("candidate-imported-symlink");
        let runtime_root = root.join("runtime");
        let source = runtime_root.join("demo");
        let managed_root = root.join("SkillBox");
        make_skill(&source, "demo", "Demo skill");

        import_candidates(
            vec![ImportRequestItem {
                source_path: source.clone(),
                skill_type: SkillKind::User,
                deploy_back_to_source: true,
            }],
            &managed_root,
        )
        .unwrap();

        let candidates = scan_import_candidates(&[runtime_root], &managed_root).unwrap();

        assert!(candidates.candidates.is_empty());
    }

    #[test]
    fn import_candidates_copies_remote_skill_updates_current_and_symlinks_source_to_current() {
        let root = temp_dir("candidate-import-remote");
        let source = root.join("runtime").join("remote-demo");
        let managed_root = root.join("SkillBox");
        make_skill(&source, "remote-demo", "Remote demo skill");

        let result = import_candidates(
            vec![ImportRequestItem {
                source_path: source.clone(),
                skill_type: SkillKind::Remote,
                deploy_back_to_source: true,
            }],
            &managed_root,
        )
        .unwrap();

        assert_eq!(result.errors.len(), 0);
        assert_eq!(result.imported.len(), 1);
        let current = managed_root
            .join("remote-skills")
            .join("remote-demo")
            .join("current");
        assert!(fs::symlink_metadata(&current)
            .unwrap()
            .file_type()
            .is_symlink());
        assert!(current.join("SKILL.md").exists());
        assert!(fs::symlink_metadata(&source)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(
            fs::canonicalize(&source).unwrap(),
            fs::canonicalize(&current).unwrap()
        );
    }

    #[test]
    fn scan_import_candidates_reports_conflicting_managed_target() {
        let root = temp_dir("candidate-conflict");
        let source = root.join("runtime").join("demo");
        let managed_root = root.join("SkillBox");
        make_skill(&source, "demo", "Runtime version");
        make_skill(
            &managed_root.join("user-skills").join("demo"),
            "demo",
            "Managed version",
        );

        let candidates = scan_import_candidates(&[root.join("runtime")], &managed_root).unwrap();

        let demo = candidate(&candidates.candidates, "demo");
        assert!(demo
            .conflict
            .as_ref()
            .unwrap()
            .contains("Managed target exists"));
        assert!(!demo.is_selected);
    }

    fn temp_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("skillbox-{label}-{nanos}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn make_skill(path: &std::path::Path, name: &str, description: &str) {
        make_skill_with_body(path, name, description, "");
    }

    fn make_skill_with_body(
        path: &std::path::Path,
        name: &str,
        description: &str,
        extra_body: &str,
    ) {
        fs::create_dir_all(path).unwrap();
        fs::write(
            path.join("SKILL.md"),
            format!(
                "---
name: {name}
description: \"{description}\"
---

# {name}
{extra_body}
"
            ),
        )
        .unwrap();
    }

    fn candidate<'a>(candidates: &'a [ImportCandidate], name: &str) -> &'a ImportCandidate {
        candidates
            .iter()
            .find(|candidate| candidate.name == name)
            .unwrap_or_else(|| panic!("candidate not found: {name}"))
    }

    fn remote_status<'a>(
        statuses: &'a [RemoteSkillUpdateStatus],
        skill_name: &str,
    ) -> &'a RemoteSkillUpdateStatus {
        statuses
            .iter()
            .find(|status| status.skill_name == skill_name)
            .unwrap_or_else(|| panic!("remote status not found: {skill_name}"))
    }

    fn workspace<'a>(workspaces: &'a [Workspace], path: &std::path::Path) -> &'a Workspace {
        let canonical = fs::canonicalize(path).unwrap();
        workspaces
            .iter()
            .find(|workspace| workspace.canonical_path == canonical)
            .unwrap_or_else(|| panic!("workspace not found: {}", path.display()))
    }

    fn write_remote_source(
        remote_root: &std::path::Path,
        repo_url: &std::path::Path,
        installed_sha: &str,
    ) {
        fs::create_dir_all(remote_root).unwrap();
        fs::write(
            remote_root.join("source.json"),
            format!(
                r#"{{
  "type": "github",
  "repoUrl": "{}",
  "ref": "main",
  "installedSha": "{}"
}}"#,
                repo_url.display(),
                installed_sha
            ),
        )
        .unwrap();
    }

    fn write_remote_source_with_json(remote_root: &std::path::Path, json: &str) {
        fs::create_dir_all(remote_root).unwrap();
        fs::write(remote_root.join("source.json"), json).unwrap();
    }

    fn bare_remote(label: &str) -> PathBuf {
        let remote = temp_dir(label).join("remote.git");
        let output = std::process::Command::new("git")
            .arg("init")
            .arg("--bare")
            .arg(&remote)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        remote
    }

    fn bare_remote_with_main(label: &str) -> PathBuf {
        let remote = bare_remote(label);
        let work = temp_dir(&format!("{label}-work"));
        run_git(&work, &["init", "-b", "main"]);
        fs::write(work.join("README.md"), "remote").unwrap();
        run_git(&work, &["add", "."]);
        run_git(
            &work,
            &[
                "-c",
                "user.name=SkillBox",
                "-c",
                "user.email=skillbox@example.invalid",
                "commit",
                "-m",
                "Initial",
            ],
        );
        run_git(
            &work,
            &["remote", "add", "origin", remote.to_str().unwrap()],
        );
        run_git(&work, &["push", "origin", "main"]);
        remote
    }

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
            format!(
                "---
name: {skill_name}
description: \"{description}\"
---

# {skill_name}
{body}
"
            ),
        )
        .unwrap();
        run_git(&work, &["add", "."]);
        run_git(
            &work,
            &[
                "-c",
                "user.name=SkillBox",
                "-c",
                "user.email=skillbox@example.invalid",
                "commit",
                "-m",
                "Add skill",
            ],
        );
        run_git(
            &work,
            &["remote", "add", "origin", remote.to_str().unwrap()],
        );
        run_git(&work, &["push", "-u", "origin", "main"]);
        remote
    }

    fn github_source_url(owner: &str, repo: &str, skill_name: &str) -> String {
        format!("https://github.com/{owner}/{repo}/tree/main/skills/{skill_name}")
    }

    static GIT_CONFIG_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    struct GitConfigRewriteGuard {
        _lock: std::sync::MutexGuard<'static, ()>,
        previous: Vec<(&'static str, Option<std::ffi::OsString>)>,
    }

    impl Drop for GitConfigRewriteGuard {
        fn drop(&mut self) {
            for (key, value) in self.previous.drain(..) {
                match value {
                    Some(value) => std::env::set_var(key, value),
                    None => std::env::remove_var(key),
                }
            }
        }
    }

    fn github_repo_rewrite(
        owner: &str,
        repo: &str,
        remote: &std::path::Path,
    ) -> GitConfigRewriteGuard {
        let lock = GIT_CONFIG_LOCK.lock().unwrap();
        let keys = ["GIT_CONFIG_COUNT", "GIT_CONFIG_KEY_0", "GIT_CONFIG_VALUE_0"];
        let previous = keys
            .into_iter()
            .map(|key| (key, std::env::var_os(key)))
            .collect::<Vec<_>>();

        std::env::set_var("GIT_CONFIG_COUNT", "1");
        std::env::set_var(
            "GIT_CONFIG_KEY_0",
            format!("url.file://{}.insteadOf", remote.display()),
        );
        std::env::set_var(
            "GIT_CONFIG_VALUE_0",
            format!("https://github.com/{owner}/{repo}.git"),
        );

        GitConfigRewriteGuard {
            _lock: lock,
            previous,
        }
    }

    fn remote_head(remote: &std::path::Path) -> String {
        let output = std::process::Command::new("git")
            .arg("ls-remote")
            .arg(remote)
            .arg("main")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout)
            .split_whitespace()
            .next()
            .unwrap()
            .to_string()
    }

    fn run_git(cwd: &std::path::Path, args: &[&str]) {
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(cwd)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
