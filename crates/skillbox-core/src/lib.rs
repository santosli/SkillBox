use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

pub type Result<T> = std::result::Result<T, String>;

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
        .unwrap_or_else(|| home_dir().join("SkillBox"))
}

pub fn default_runtime_roots() -> Vec<PathBuf> {
    vec![
        home_dir().join(".codex/skills"),
        home_dir().join(".agents/skills"),
    ]
}

pub fn global_runtime_roots() -> Vec<PathBuf> {
    runtime_roots_under(&home_dir())
}

fn runtime_roots_under(home: &Path) -> Vec<PathBuf> {
    let mut roots = vec![home.join(".codex/skills"), home.join(".agents/skills")];
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
    for runtime_parent in [".agents", ".codex"] {
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
            Some(".agents" | ".codex")
        )
}

fn should_skip_runtime_root_search(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    if matches!(name, ".agents" | ".codex") {
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
    let paths = managed_paths(root);
    fs::create_dir_all(&paths.user_skills_root).map_err(|error| error.to_string())?;
    fs::create_dir_all(&paths.remote_skills_root).map_err(|error| error.to_string())?;
    init_database(&paths.database_path)?;
    Ok(paths)
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
    } else {
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

pub fn managed_state(managed_root: impl AsRef<Path>) -> Result<ManagedState> {
    let paths = ensure_managed_layout(managed_root.as_ref().to_path_buf())?;
    let mut skills = Vec::new();

    for skill in scan_skill_roots(std::slice::from_ref(&paths.user_skills_root))?.skills {
        skills.push(managed_skill(skill, SkillKind::User));
    }
    skills.extend(scan_managed_remote_skills(&paths)?);

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

pub fn managed_preferences(managed_root: impl AsRef<Path>) -> Result<ManagedPreferences> {
    let paths = ensure_managed_layout(managed_root.as_ref().to_path_buf())?;
    let skip_local_import_confirmation =
        read_bool_preference(&paths.database_path, "skip_local_import_confirmation")?
            .unwrap_or(false);

    Ok(ManagedPreferences {
        skip_local_import_confirmation,
    })
}

pub fn set_skip_local_import_confirmation(
    managed_root: impl AsRef<Path>,
    skip: bool,
) -> Result<ManagedPreferences> {
    let paths = ensure_managed_layout(managed_root.as_ref().to_path_buf())?;
    write_bool_preference(&paths.database_path, "skip_local_import_confirmation", skip)?;

    Ok(ManagedPreferences {
        skip_local_import_confirmation: skip,
    })
}

pub fn user_skills_git_status(managed_root: impl AsRef<Path>) -> Result<UserSkillsGitStatus> {
    let paths = ensure_managed_layout(managed_root.as_ref().to_path_buf())?;
    user_skills_git_status_for_repo(paths.user_skills_root)
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

    skillbox_git::add_all(&repo)?;
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

pub fn scan_import_candidates(
    roots: &[PathBuf],
    managed_root: impl AsRef<Path>,
) -> Result<ImportCandidateScan> {
    let paths = ensure_managed_layout(managed_root.as_ref().to_path_buf())?;
    let managed_scan = scan_skill_roots(&[
        paths.user_skills_root.clone(),
        paths.remote_skills_root.clone(),
    ])?;
    let imported_hashes: HashSet<String> = managed_scan
        .skills
        .iter()
        .map(|skill| skill.content_hash.clone())
        .collect();
    let scan = scan_skill_roots(roots)?;
    let mut candidates = Vec::new();

    for skill in scan.skills {
        let is_system = is_system_skill(&skill);
        let is_imported = imported_hashes.contains(&skill.content_hash)
            || is_under_path(&skill.real_path, &paths.root);
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
    }
}

fn user_skills_git_status_for_repo(repo_path: PathBuf) -> Result<UserSkillsGitStatus> {
    let git_status = skillbox_git::status(&repo_path)?;
    let remote_url = if git_status.initialized {
        skillbox_git::origin_url(&repo_path)?
    } else {
        None
    };
    let state = user_skills_git_state(git_status.initialized, git_status.dirty, &remote_url);

    Ok(UserSkillsGitStatus {
        repo_path,
        initialized: git_status.initialized,
        branch: git_status.branch,
        remote_url,
        dirty: git_status.dirty,
        raw_status: git_status.raw_status,
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

fn normalized_commit_message(message: Option<&str>) -> String {
    message
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("Sync user skills")
        .to_string()
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

fn init_database(database_path: &Path) -> Result<()> {
    if let Some(parent) = database_path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let connection = Connection::open(database_path).map_err(|error| error.to_string())?;
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
            ",
        )
        .map_err(|error| error.to_string())
}

fn read_bool_preference(database_path: &Path, key: &str) -> Result<Option<bool>> {
    let connection = Connection::open(database_path).map_err(|error| error.to_string())?;
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
    let connection = Connection::open(database_path).map_err(|error| error.to_string())?;
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

fn index_skill(
    database_path: &Path,
    skill: &Skill,
    kind: SkillKind,
    managed_path: &Path,
) -> Result<()> {
    let connection = Connection::open(database_path).map_err(|error| error.to_string())?;
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
    let connection = Connection::open(database_path).map_err(|error| error.to_string())?;
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

    for index in 2.. {
        let candidate = paths
            .root
            .join("backups")
            .join("imports")
            .join(format!("{skill_name}-{hash}-{index}"));
        if !candidate.exists() {
            return candidate;
        }
    }

    unreachable!("backup suffix loop is unbounded")
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
    if current.join("SKILL.md").exists() {
        found.push(current.to_path_buf());
        return Ok(());
    }

    for entry in fs::read_dir(current).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|error| error.to_string())?;
        let is_dir = file_type.is_dir()
            || (file_type.is_symlink()
                && fs::metadata(&path)
                    .map(|metadata| metadata.is_dir())
                    .unwrap_or(false));
        if !is_dir {
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
        return fs::canonicalize(remote_current).map_err(|error| error.to_string());
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
    fs::create_dir_all(destination).map_err(|error| error.to_string())?;

    for entry in fs::read_dir(source).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let file_name = entry.file_name();
        if file_name == ".git" {
            continue;
        }
        copy_recursively(&entry.path(), &destination.join(file_name))?;
    }

    Ok(())
}

fn copy_recursively(source: &Path, destination: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(source).map_err(|error| error.to_string())?;
    if metadata.is_dir() {
        fs::create_dir_all(destination).map_err(|error| error.to_string())?;
        for entry in fs::read_dir(source).map_err(|error| error.to_string())? {
            let entry = entry.map_err(|error| error.to_string())?;
            copy_recursively(&entry.path(), &destination.join(entry.file_name()))?;
        }
    } else if metadata.file_type().is_symlink() {
        let target = fs::read_link(source).map_err(|error| error.to_string())?;
        symlink_any(&target, destination)?;
    } else {
        fs::copy(source, destination).map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn update_current_symlink(remote_root: &Path, version_path: &Path) -> Result<()> {
    fs::create_dir_all(remote_root).map_err(|error| error.to_string())?;
    let current = remote_root.join("current");
    let _ = fs::remove_file(&current);
    symlink_dir(version_path, &current)
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
    format!("{:x}", Sha256::digest(content.as_bytes()))
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

        let roots = runtime_roots_under(&root);

        assert!(roots.contains(&root.join(".codex").join("skills")));
        assert!(roots.contains(&root.join(".agents").join("skills")));
        assert!(roots.contains(&project_agents_root));
        assert!(roots.contains(&project_codex_root));
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
    fn managed_preferences_default_to_showing_local_import_confirmation() {
        let root = temp_dir("preferences-default");
        let preferences = managed_preferences(&root.join("SkillBox")).unwrap();

        assert!(!preferences.skip_local_import_confirmation);
    }

    #[test]
    fn managed_preferences_persist_skip_local_import_confirmation() {
        let root = temp_dir("preferences-persist");
        let managed_root = root.join("SkillBox");

        set_skip_local_import_confirmation(&managed_root, true).unwrap();
        let preferences = managed_preferences(&managed_root).unwrap();

        assert!(preferences.skip_local_import_confirmation);
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
    fn scan_import_candidates_marks_symlinked_source_as_imported() {
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

        assert_eq!(candidates.candidates.len(), 1);
        let demo = candidate(&candidates.candidates, "demo");
        assert_eq!(demo.import_status, ImportCandidateStatus::Imported);
        assert!(!demo.is_selected);
        assert_eq!(fs::canonicalize(&demo.source_path).unwrap(), demo.real_path);
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
}
