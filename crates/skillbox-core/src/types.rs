use crate::*;

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
    pub usage_count: usize,
    pub last_used_at: Option<String>,
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
    pub usage_count: usize,
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
    pub(crate) fn as_str(self) -> &'static str {
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
pub enum HistoryEntryKind {
    SkillUsage,
    Operation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct HistoryFilter {
    pub kind: Option<HistoryEntryKind>,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HistoryEntry {
    pub id: String,
    pub kind: HistoryEntryKind,
    pub timestamp: String,
    pub title: String,
    pub subtitle: String,
    pub prompt_excerpt: Option<String>,
    pub status: Option<OperationStatus>,
    pub skill_name: Option<String>,
    pub agent_id: Option<String>,
    pub runtime_root: Option<PathBuf>,
    pub operation_type: Option<String>,
    pub actor: Option<String>,
    pub entity_type: Option<String>,
    pub entity_name: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HistoryList {
    pub entries: Vec<HistoryEntry>,
    pub skill_usage_count: usize,
    pub operation_count: usize,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallGithubRemoteSkillRequest {
    pub source_url: String,
    pub target_root: Option<PathBuf>,
    pub actor: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InstallGithubRemoteSkillResult {
    pub skill_name: String,
    pub source_url: String,
    pub repo_url: String,
    pub owner: String,
    pub repo: String,
    pub path: String,
    pub reference: String,
    pub ref_kind: Option<String>,
    pub tracking: bool,
    pub installed_sha: String,
    pub version_path: PathBuf,
    pub current_path: PathBuf,
    pub source_path: PathBuf,
    pub deployment: Option<Deployment>,
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
pub(crate) struct ClaudeMarketplaceSkill {
    pub(crate) name: Option<String>,
    pub(crate) description: Option<String>,
    pub(crate) repo: Option<String>,
    pub(crate) path: Option<String>,
    pub(crate) stars: Option<u64>,
    pub(crate) installs: Option<u64>,
    #[serde(rename = "lastUpdated", alias = "last_updated")]
    pub(crate) last_updated: Option<String>,
    #[serde(rename = "listingStatus", alias = "listing_status")]
    pub(crate) listing_status: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(crate) struct RemoteSkillSource {
    #[serde(rename = "type")]
    pub(crate) source_type: String,
    #[serde(rename = "url", alias = "sourceUrl", alias = "source_url")]
    pub(crate) source_url: Option<String>,
    pub(crate) path: Option<String>,
    #[serde(rename = "repoUrl", alias = "repo_url")]
    pub(crate) repo_url: Option<String>,
    #[serde(rename = "ref", alias = "reference")]
    pub(crate) reference: Option<String>,
    #[serde(rename = "refKind", alias = "ref_kind")]
    pub(crate) ref_kind: Option<String>,
    pub(crate) tracking: Option<bool>,
    #[serde(rename = "currentVersion", alias = "current_version")]
    pub(crate) current_version: Option<String>,
    #[serde(rename = "installedSha", alias = "installed_sha")]
    pub(crate) installed_sha: Option<String>,
    #[serde(rename = "latestSha", alias = "latest_sha")]
    pub(crate) latest_sha: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ImportCandidate {
    pub name: String,
    pub description: String,
    pub source_path: PathBuf,
    pub source_root: Option<PathBuf>,
    pub real_path: PathBuf,
    pub is_symlink: bool,
    pub symlink_target_path: Option<PathBuf>,
    pub content_hash: String,
    pub suggested_type: SkillKind,
    pub suggestion_reason: String,
    pub import_status: ImportCandidateStatus,
    pub is_selected: bool,
    pub conflict: Option<String>,
    pub usage_count: usize,
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

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportRecordStatus {
    Active,
    Reverted,
    Failed,
}

impl ImportRecordStatus {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            ImportRecordStatus::Active => "active",
            ImportRecordStatus::Reverted => "reverted",
            ImportRecordStatus::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ImportRecordFilter {
    pub skill_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ImportRecord {
    pub id: String,
    pub skill_name: String,
    pub kind: SkillKind,
    pub source_path: PathBuf,
    pub source_root: Option<PathBuf>,
    pub managed_path: PathBuf,
    pub content_hash: String,
    pub backup_path: PathBuf,
    pub deployed_path: PathBuf,
    pub status: ImportRecordStatus,
    pub legacy: bool,
    pub imported_at: String,
    pub reverted_at: Option<String>,
    pub can_revert: bool,
    pub revert_block_reason: Option<String>,
    pub affected_deployment_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ImportRecordList {
    pub records: Vec<ImportRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevertImportRequest {
    pub import_record_id: String,
    pub actor: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RevertImportResult {
    pub record: ImportRecord,
    pub restored_path: PathBuf,
    pub removed_managed_path: Option<PathBuf>,
    pub operation_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecordSkillUsageRequest {
    pub skill_name: String,
    pub agent_id: String,
    pub runtime_root: PathBuf,
    pub event_id: Option<String>,
    pub used_at: Option<String>,
    #[serde(default)]
    pub prompt_excerpt: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SkillUsageRecordResult {
    pub skill_name: String,
    pub agent_id: String,
    pub runtime_root: PathBuf,
    pub event_id: Option<String>,
    pub used_at: String,
    pub recorded_at: String,
    pub usage_count: usize,
    pub last_used_at: String,
    pub deduplicated: bool,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UsageHookTarget {
    CodexApp,
    CodexCli,
    ClaudeCodeCli,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UsageHookStatus {
    pub target: UsageHookTarget,
    pub label: String,
    pub config_path: PathBuf,
    pub command: String,
    pub installed: bool,
    pub trust_required: bool,
    pub activation_note: Option<String>,
    pub shared_config_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UsageHookInstallResult {
    pub target: UsageHookTarget,
    pub installed: bool,
    pub backup_path: Option<PathBuf>,
    pub status: UsageHookStatus,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct UsageHookRecordResult {
    pub recorded: Vec<SkillUsageRecordResult>,
    pub skipped: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct UsageSummary {
    pub(crate) usage_count: usize,
    pub(crate) last_used_at: Option<String>,
}
