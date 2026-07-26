use crate::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ManagedPaths {
    pub root: PathBuf,
    pub user_skills_root: PathBuf,
    pub remote_skills_root: PathBuf,
    pub database_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillMetadata {
    pub name: String,
    pub description: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillFrontmatterDocument {
    pub present: bool,
    pub metadata: SkillMetadata,
    pub fields: BTreeMap<String, serde_json::Value>,
    pub unknown_fields: Vec<String>,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeleteSkillRequest {
    pub skill_name: String,
    pub preview_id: String,
    pub confirmed_skill_name: String,
    pub actor: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DeleteSkillPreview {
    pub preview_id: String,
    pub skill_name: String,
    pub kind: SkillKind,
    pub managed_path: PathBuf,
    pub deployments: Vec<ManagedSkillDeployment>,
    pub blockers: Vec<String>,
    pub can_delete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DeleteSkillResult {
    pub skill_name: String,
    pub kind: SkillKind,
    pub managed_path: PathBuf,
    pub backup_path: PathBuf,
    pub removed_deployments: Vec<ManagedSkillDeployment>,
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
    pub confirmed_count: usize,
    pub inferred_count: usize,
    pub reference_count: usize,
    pub last_referenced_at: Option<String>,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppUpdateCheckCache {
    pub current_version: String,
    pub available: bool,
    pub version: String,
    pub date: String,
    pub body: String,
    pub checked_at: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillUserMetadata {
    pub skill_name: String,
    pub favorite: bool,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillUserMetadataUpdate {
    pub skill_name: String,
    pub favorite: bool,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DoctorIssueSeverity {
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct DoctorRequest {
    #[serde(default)]
    pub repair_preview: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DoctorIssue {
    pub code: String,
    pub severity: DoctorIssueSeverity,
    pub entity_name: Option<String>,
    pub path: Option<PathBuf>,
    pub message: String,
    pub repairable: bool,
    pub suggested_action: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DoctorReport {
    pub checked_at: String,
    pub schema_version: i64,
    pub latest_schema_version: i64,
    pub healthy: bool,
    pub error_count: usize,
    pub warning_count: usize,
    pub repair_preview: bool,
    pub issues: Vec<DoctorIssue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DoctorRepairResult {
    pub removed_deployment_records: usize,
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
    pub profile_id: String,
    pub profile_name: String,
    pub root_key: String,
    pub format: RuntimeFormat,
    pub display_name: String,
    pub skill_count: usize,
    pub imported_skill_count: usize,
    pub usage_count: usize,
    pub reference_count: usize,
    pub last_scan_error_count: usize,
    pub last_scan_error: Option<String>,
    pub last_scanned_at: Option<String>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeFormat {
    SkillMd,
    Unsupported,
}

impl RuntimeFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            RuntimeFormat::SkillMd => "skill_md",
            RuntimeFormat::Unsupported => "unsupported",
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeRootScope {
    Project,
    Exact,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeRootSpec {
    pub key: String,
    pub relative_path: String,
    pub scope: RuntimeRootScope,
    pub precedence: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrontmatterPolicy {
    pub supported_fields: Vec<String>,
    pub required_fields: Vec<String>,
    pub preserve_unknown_fields: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeProfile {
    pub id: String,
    pub registry_version: u32,
    pub display_name: String,
    pub format: RuntimeFormat,
    pub roots: Vec<RuntimeRootSpec>,
    pub deployment_modes: Vec<String>,
    pub frontmatter_policy: FrontmatterPolicy,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CompatibilityStatus {
    Compatible,
    Warnings,
    Blocked,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CompatibilityIssueSeverity {
    Warning,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompatibilityIssue {
    pub code: String,
    pub severity: CompatibilityIssueSeverity,
    pub field: Option<String>,
    pub message: String,
    pub suggested_action: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeploymentCompatibilityPreviewRequest {
    pub skill_name: String,
    pub target_root: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeploymentCompatibilityApplyRequest {
    pub skill_name: String,
    pub target_root: PathBuf,
    pub preview_id: String,
    #[serde(default)]
    pub confirm_warnings: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompatibilityReport {
    pub preview_id: String,
    pub skill_name: String,
    pub target_root: PathBuf,
    pub profile: RuntimeProfile,
    pub root_key: String,
    pub format: RuntimeFormat,
    pub deployment_mode: String,
    pub status: CompatibilityStatus,
    pub issues: Vec<CompatibilityIssue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceAddRequest {
    pub path: PathBuf,
    pub kind: WorkspaceKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceSetupPreviewRequest {
    pub selected_path: PathBuf,
    pub kind: WorkspaceKind,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceSetupMode {
    ExistingRoot,
    ProjectWithRoots,
    ProjectWithoutRoots,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceSetupRootOption {
    pub path: PathBuf,
    pub relative_path: String,
    pub agent_id: String,
    pub profile_id: String,
    pub profile_name: String,
    pub root_key: String,
    pub format: RuntimeFormat,
    pub label: String,
    pub exists: bool,
    pub recommended: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceSetupPreview {
    pub preview_id: String,
    pub selected_path: PathBuf,
    pub kind: WorkspaceKind,
    pub mode: WorkspaceSetupMode,
    pub roots: Vec<WorkspaceSetupRootOption>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceSetupApplyRequest {
    pub selected_path: PathBuf,
    pub kind: WorkspaceKind,
    pub selected_root: PathBuf,
    pub create_missing: bool,
    pub preview_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WorkspaceSetupApplyResult {
    pub workspace: Workspace,
    pub created_path: Option<PathBuf>,
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
    UsageReference,
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
    pub skill_reference_count: usize,
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
    pub root: bool,
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
    pub preview_id: Option<String>,
    #[serde(default)]
    pub confirm_warnings: bool,
    pub actor: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreviewGithubRemoteSkillInstallRequest {
    pub source_url: String,
    pub target_root: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GithubRemoteSkillInstallPreview {
    pub preview_id: String,
    pub skill_name: String,
    pub source_url: String,
    pub repo_url: String,
    pub owner: String,
    pub repo: String,
    pub path: String,
    pub root: bool,
    pub reference: String,
    pub ref_kind: Option<String>,
    pub tracking: bool,
    pub installed_sha: String,
    pub files: Vec<RemoteDiffFile>,
    pub target_root: Option<PathBuf>,
    pub compatibility: Option<CompatibilityReport>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InstallGithubRemoteSkillResult {
    pub skill_name: String,
    pub source_url: String,
    pub repo_url: String,
    pub owner: String,
    pub repo: String,
    pub path: String,
    pub root: bool,
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
    #[serde(default)]
    pub(crate) root: bool,
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
    pub additional_source_paths: Vec<PathBuf>,
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
    pub evidence_class: SkillUsageEvidenceClass,
    pub upgraded: bool,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SkillUsageEvidenceClass {
    Confirmed,
    Inferred,
    #[default]
    Reference,
}

impl SkillUsageEvidenceClass {
    pub fn as_str(self) -> &'static str {
        match self {
            SkillUsageEvidenceClass::Confirmed => "confirmed",
            SkillUsageEvidenceClass::Inferred => "inferred",
            SkillUsageEvidenceClass::Reference => "reference",
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SkillUsageRankingRange {
    #[serde(rename = "last_7_days")]
    Last7Days,
    #[default]
    #[serde(rename = "last_30_days")]
    Last30Days,
    AllTime,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillUsageRankingSkillType {
    User,
    Remote,
    System,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SkillUsageRankingRequest {
    #[serde(default)]
    pub range: SkillUsageRankingRange,
    #[serde(alias = "skillType")]
    pub skill_type: Option<SkillUsageRankingSkillType>,
    #[serde(alias = "agentId")]
    pub agent_id: Option<String>,
    #[serde(alias = "workspaceRoot")]
    pub workspace_root: Option<PathBuf>,
    #[serde(default, alias = "includeUnmanaged")]
    pub include_unmanaged: bool,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SkillUsageSourceKind {
    #[default]
    Regular,
    System,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreviewUsageSkillImportRequest {
    #[serde(alias = "skillName")]
    pub skill_name: String,
    #[serde(default, alias = "sourceKind")]
    pub source_kind: Option<SkillUsageSourceKind>,
    #[serde(default, alias = "sourceId")]
    pub source_id: Option<String>,
    #[serde(default, alias = "sourceRuntimeRoots")]
    pub source_runtime_roots: Vec<PathBuf>,
    #[serde(default, alias = "rankingRequest")]
    pub ranking_request: Option<SkillUsageRankingRequest>,
    #[serde(default, alias = "rankingGeneratedAt")]
    pub ranking_generated_at: Option<String>,
    #[serde(default, alias = "runtimeRoot")]
    pub runtime_root: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SkillUsageRankingRow {
    pub rank: usize,
    pub skill_name: String,
    pub kind: Option<SkillKind>,
    pub managed: bool,
    #[serde(default)]
    pub system: bool,
    #[serde(default)]
    pub source_missing: bool,
    pub source_kind: SkillUsageSourceKind,
    pub source_id: String,
    pub source_runtime_roots: Vec<PathBuf>,
    pub usage_count: usize,
    pub last_used_at: Option<String>,
    pub confirmed_count: usize,
    pub inferred_count: usize,
    pub reference_count: usize,
    pub last_referenced_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SkillUsageRankingResult {
    pub generated_at: String,
    pub range: SkillUsageRankingRange,
    pub range_start: Option<String>,
    pub range_end: String,
    pub agent_id: Option<String>,
    pub skill_type: Option<SkillUsageRankingSkillType>,
    pub workspace_root: Option<PathBuf>,
    pub total_calls: usize,
    /// Backward-compatible alias for total_calls.
    pub total_observed_calls: usize,
    pub total_confirmed_calls: usize,
    pub total_inferred_calls: usize,
    pub total_history_references: usize,
    pub coverage: SkillUsageCoverage,
    pub rows: Vec<SkillUsageRankingRow>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct SkillUsageCoverage {
    pub earliest_event_at: Option<String>,
    pub latest_event_at: Option<String>,
    pub earliest_confirmed_at: Option<String>,
    pub latest_confirmed_at: Option<String>,
    pub earliest_inferred_at: Option<String>,
    pub latest_inferred_at: Option<String>,
    pub earliest_reference_at: Option<String>,
    pub latest_reference_at: Option<String>,
    pub confirmed_calls: usize,
    pub inferred_calls: usize,
    pub history_references: usize,
    pub source_counts: Vec<SkillUsageEvidenceSourceCount>,
    /// Backward-compatible source totals. Use source_counts for evidence-aware UI.
    pub agent_hook_calls: usize,
    pub codex_session_backfill_calls: usize,
    pub claude_code_session_backfill_calls: usize,
    pub cursor_session_backfill_calls: usize,
    pub other_observed_calls: usize,
    pub scanned_codex_session_files: usize,
    pub scanned_codex_turns: usize,
    pub scanned_claude_code_session_files: usize,
    pub scanned_cursor_sessions: usize,
    pub scanned_cursor_transcript_files: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SkillUsageEvidenceSourceCount {
    pub source: String,
    pub evidence_class: SkillUsageEvidenceClass,
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct UsageBackfillAudit {
    pub source: String,
    pub scanned: usize,
    pub discovered: usize,
    pub recorded: usize,
    pub deduplicated: usize,
    pub upgraded: usize,
    pub skipped: usize,
    pub errors: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct SkillUsageAudit {
    pub total_calls: usize,
    pub confirmed_calls: usize,
    pub inferred_calls: usize,
    pub history_references: usize,
    pub earliest_confirmed_at: Option<String>,
    pub latest_confirmed_at: Option<String>,
    pub earliest_inferred_at: Option<String>,
    pub latest_inferred_at: Option<String>,
    pub earliest_reference_at: Option<String>,
    pub latest_reference_at: Option<String>,
    pub source_counts: Vec<SkillUsageEvidenceSourceCount>,
    pub scanned_codex_session_files: usize,
    pub scanned_codex_turns: usize,
    pub scanned_claude_code_session_files: usize,
    pub scanned_cursor_sessions: usize,
    pub scanned_cursor_transcript_files: usize,
    pub confirmed_cursor_transcript_reads: usize,
    pub codex_provider_reported_total: Option<usize>,
    pub codex_remaining_gap: Option<isize>,
    pub known_limitations: Vec<String>,
    pub last_backfills: Vec<UsageBackfillAudit>,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct BackfillCodexSessionUsageRequest {
    #[serde(default, alias = "includeArchived")]
    pub include_archived: bool,
    #[serde(default, alias = "sessionsRoot")]
    pub sessions_root: Option<PathBuf>,
    #[serde(default, alias = "archivedSessionsRoot")]
    pub archived_sessions_root: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct BackfillClaudeCodeSessionUsageRequest {
    #[serde(default, alias = "projectsRoot")]
    pub projects_root: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct BackfillCursorSessionUsageRequest {
    #[serde(default, alias = "databasePath")]
    pub database_path: Option<PathBuf>,
    #[serde(default, alias = "projectsRoot")]
    pub projects_root: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct BackfillCodexSessionUsageResult {
    pub scanned_files: usize,
    pub scanned_turns: usize,
    pub discovered: usize,
    pub recorded: usize,
    pub deduplicated: usize,
    pub upgraded: usize,
    pub skipped: usize,
    pub errors: Vec<String>,
    pub scanned_cursor_state_sessions: usize,
    pub cursor_state_references: usize,
    pub scanned_cursor_transcript_files: usize,
    pub confirmed_cursor_transcript_reads: usize,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct UsageSummary {
    pub(crate) usage_count: usize,
    pub(crate) last_used_at: Option<String>,
    pub(crate) confirmed_count: usize,
    pub(crate) inferred_count: usize,
    pub(crate) reference_count: usize,
    pub(crate) last_referenced_at: Option<String>,
}
