use crate::*;
use rustix::fs::{AtFlags, Mode, OFlags, RenameFlags};
use std::fs::{File, OpenOptions};
use std::os::fd::{AsFd, OwnedFd};
use std::os::unix::fs::MetadataExt;

const USER_SKILLS_REMOTE_BRANCH: &str = "main";
const MAX_INBOUND_FILE_BYTES: u64 = 10 * 1024 * 1024;
const MAX_INBOUND_TREE_BYTES: u64 = 100 * 1024 * 1024;
const MAX_INBOUND_TREE_ENTRIES: usize = 10_000;

#[derive(Debug, Clone)]
struct InboundTreeSnapshot {
    skills: Vec<Skill>,
    entries: HashMap<String, skillbox_git::GitTreeEntry>,
}

#[derive(Debug)]
struct InboundMutationReceipt {
    repo_dir: OwnedFd,
    old_sha: Option<String>,
    new_sha: String,
    old_entries: HashMap<String, skillbox_git::GitTreeEntry>,
    new_entries: HashMap<String, skillbox_git::GitTreeEntry>,
    touched_paths: Vec<String>,
    written_paths: HashSet<String>,
    deleted_paths: HashSet<String>,
    created_dirs: HashSet<String>,
    path_backups: HashMap<String, InboundBackupSlot>,
    old_index: Option<Vec<u8>>,
    index_replaced: bool,
    index_lock: Option<GitIndexLock>,
    ref_advanced: bool,
    restore_generated_gitignore: bool,
}

#[derive(Debug, Default)]
struct InboundMutationOptions {
    fail_before_index_prepare: bool,
    fail_after_writes: Option<usize>,
    fail_after_index_replace: bool,
    pause_before_materialization: Option<PathBuf>,
    pause_before_ref_update: Option<PathBuf>,
    pause_before_backup_rename: Option<PathBuf>,
    pause_before_compensation: Option<PathBuf>,
}

#[derive(Debug)]
struct GitIndexLock {
    git_dir: OwnedFd,
    file: File,
    device: u64,
    inode: u64,
}

#[derive(Debug)]
struct InboundBackupSlot {
    parent: OwnedFd,
    file_name: String,
}

impl Drop for GitIndexLock {
    fn drop(&mut self) {
        let _ = self.release_if_owned();
    }
}

impl GitIndexLock {
    fn release_if_owned(&mut self) -> Result<()> {
        let expected = self.file.metadata().map_err(|error| error.to_string())?;
        let current = rustix::fs::statat(&self.git_dir, "index.lock", AtFlags::SYMLINK_NOFOLLOW)
            .map_err(|error| error.to_string())?;
        if current.st_dev as u64 != self.device
            || current.st_ino as u64 != self.inode
            || expected.dev() != self.device
            || expected.ino() != self.inode
        {
            return Err(
                "Git index lock ownership changed; refusing to remove another process's lock."
                    .to_string(),
            );
        }
        rustix::fs::unlinkat(&self.git_dir, "index.lock", AtFlags::empty())
            .map_err(|error| error.to_string())
    }
}

#[derive(Debug, Default)]
struct InboundApplyAudit {
    old_sha: Option<String>,
    new_sha: Option<String>,
    backup_ref: Option<String>,
    phase: String,
    compensation_attempted: bool,
    compensation_succeeded: Option<bool>,
    compensation_error: Option<String>,
}

impl InboundApplyAudit {
    fn payload(&self) -> serde_json::Value {
        serde_json::json!({
            "oldSha": self.old_sha,
            "newSha": self.new_sha,
            "backupRef": self.backup_ref,
            "mutationPhase": self.phase,
            "compensation": {
                "attempted": self.compensation_attempted,
                "succeeded": self.compensation_succeeded,
                "error": self.compensation_error
            }
        })
    }
}

type InboundDeployments = HashMap<String, Vec<ManagedSkillDeployment>>;
type InboundDeploymentProfiles = HashMap<PathBuf, (String, String)>;

pub fn check_user_skills_inbound(
    managed_root: impl AsRef<Path>,
) -> Result<UserSkillsInboundStatus> {
    let managed_root = managed_root.as_ref().to_path_buf();
    let _mutation_lock = acquire_user_skills_mutation_lock(&managed_root)?;
    check_user_skills_inbound_unlocked(&managed_root)
}

fn check_user_skills_inbound_unlocked(managed_root: &Path) -> Result<UserSkillsInboundStatus> {
    let paths = managed_paths(managed_root.to_path_buf());
    let repo = paths.user_skills_root;
    let git = skillbox_git::GitService::new();
    let status = git.status_hardened(&repo)?;
    if !status.initialized {
        return Err(
            "User skills Git repository is not initialized. Configure a remote first.".to_string(),
        );
    }
    let remote_url = git.origin_url(&repo)?;
    let Some(remote_url_value) = remote_url.as_deref() else {
        return Ok(inbound_unknown_status(
            repo,
            status,
            None,
            "Configure the origin remote before checking incoming changes.",
            None,
        ));
    };
    let fetched_at = operation_timestamp();
    if let Err(error) = git.fetch_origin_main(&repo) {
        return Ok(inbound_unknown_status(
            repo,
            status,
            remote_url.as_deref().map(sanitize_git_remote_url),
            "Unable to fetch origin/main. Check the remote and try again.",
            Some(sanitize_git_error(&error, remote_url_value)),
        ));
    }
    inbound_status_from_refs(&repo, Some(fetched_at))
}

pub fn preview_user_skills_inbound(
    managed_root: impl AsRef<Path>,
) -> Result<UserSkillsInboundPreview> {
    let managed_root = managed_root.as_ref().to_path_buf();
    let _mutation_lock = acquire_user_skills_mutation_lock(&managed_root)?;
    let checked = check_user_skills_inbound_unlocked(&managed_root)?;
    if let Some(error) = checked.fetch_error {
        return Err(error);
    }
    let paths = managed_paths(managed_root);
    preview_user_skills_inbound_for_paths(&paths, checked.fetched_at)
}

pub fn apply_user_skills_inbound(
    request: UserSkillsInboundApplyRequest,
    managed_root: impl AsRef<Path>,
) -> Result<UserSkillsInboundApplyResult> {
    let managed_root = managed_root.as_ref().to_path_buf();
    let preview_id = request
        .preview_id
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            "Inbound preview is required. Check remote and review incoming changes first."
                .to_string()
        })?
        .to_string();
    let operation = start_operation(
        OperationStart {
            operation_type: "apply_user_skills_inbound".to_string(),
            actor: request.actor,
            entity_type: "user_skills_repo".to_string(),
            entity_name: "user-skills".to_string(),
            summary: "Apply reviewed user skills fast-forward".to_string(),
            payload: serde_json::json!({"previewId": preview_id}),
        },
        &managed_root,
    )?;
    let mut audit = InboundApplyAudit {
        phase: "preflight".to_string(),
        ..Default::default()
    };
    let result =
        apply_user_skills_inbound_inner(&managed_root, &preview_id, &operation.id, &mut audit);
    match &result {
        Ok(applied) => {
            if let Err(finish_error) = finish_operation(
                OperationFinish {
                    id: operation.id.clone(),
                    status: OperationStatus::Succeeded,
                    summary: format!("Fast-forwarded user skills to {}", applied.new_sha),
                    error: None,
                    payload: serde_json::json!({
                        "oldSha": applied.old_sha,
                        "newSha": applied.new_sha,
                        "backupRef": applied.backup_ref,
                        "changedSkillCount": applied.changed_skill_count,
                        "changedFileCount": applied.changed_file_count
                    }),
                },
                &managed_root,
            ) {
                return Err(format!(
                    "Inbound apply succeeded, but operation history could not be finalized: {finish_error}"
                ));
            }
        }
        Err(error) => {
            if let Err(finish_error) = finish_operation(
                OperationFinish {
                    id: operation.id,
                    status: OperationStatus::Failed,
                    summary: "User skills fast-forward failed".to_string(),
                    error: Some(error.clone()),
                    payload: audit.payload(),
                },
                &managed_root,
            ) {
                return Err(format!(
                    "{error} Operation history also failed to record recovery details: {finish_error}"
                ));
            }
        }
    }
    result
}

fn inbound_unknown_status(
    repo_path: PathBuf,
    status: skillbox_git::GitStatus,
    remote_url: Option<String>,
    message: &str,
    fetch_error: Option<String>,
) -> UserSkillsInboundStatus {
    let git = skillbox_git::GitService::new();
    let local_sha = git.rev_parse_optional(&repo_path, "HEAD").ok().flatten();
    let remote_sha = git
        .rev_parse_optional(&repo_path, "refs/remotes/origin/main")
        .ok()
        .flatten();
    UserSkillsInboundStatus {
        repo_path,
        branch: status.branch,
        remote_url,
        worktree_state: if status.dirty {
            UserSkillsInboundWorktreeState::Dirty
        } else {
            UserSkillsInboundWorktreeState::Clean
        },
        relation: UserSkillsInboundRelation::Unknown,
        local_sha,
        remote_sha,
        merge_base_sha: None,
        ahead_count: 0,
        behind_count: 0,
        fetched_at: None,
        fetch_error,
        message: message.to_string(),
    }
}

fn sanitize_git_remote_url(remote_url: &str) -> String {
    let trimmed = remote_url.trim();
    if let Some(scheme_index) = trimmed.find("://") {
        let without_secret_suffix = trimmed.split(['?', '#']).next().unwrap_or(trimmed);
        let authority_start = scheme_index + 3;
        let authority_end = without_secret_suffix[authority_start..]
            .find('/')
            .map_or(without_secret_suffix.len(), |offset| {
                authority_start + offset
            });
        let authority = &without_secret_suffix[authority_start..authority_end];
        let host = authority
            .rsplit_once('@')
            .map(|(_, host)| host)
            .unwrap_or(authority);
        return format!(
            "{}{}{}",
            &without_secret_suffix[..authority_start],
            host,
            &without_secret_suffix[authority_end..]
        );
    }
    if let Some((identity, path)) = trimmed.split_once(':') {
        if identity.contains('@') && !path.is_empty() {
            let path = path.split(['?', '#']).next().unwrap_or(path);
            let host = identity
                .rsplit_once('@')
                .map(|(_, host)| host)
                .unwrap_or(identity);
            return format!("{host}:{path}");
        }
    }
    trimmed.to_string()
}

fn sanitize_git_error(error: &str, remote_url: &str) -> String {
    error.replace(remote_url, &sanitize_git_remote_url(remote_url))
}

fn inbound_status_from_refs(
    repo: &Path,
    fetched_at: Option<String>,
) -> Result<UserSkillsInboundStatus> {
    let git = skillbox_git::GitService::new();
    let git_status = git.status_hardened(repo)?;
    let remote_url = git.origin_url(repo)?;
    let local_sha = git.rev_parse_optional(repo, "HEAD")?;
    let remote_ref = format!("refs/remotes/origin/{USER_SKILLS_REMOTE_BRANCH}");
    let remote_sha = git.rev_parse_optional(repo, &remote_ref)?;
    let on_main = git_status.branch == USER_SKILLS_REMOTE_BRANCH
        || (git_status.branch.is_empty() && local_sha.is_none());
    let (relation, merge_base_sha, ahead_count, behind_count, message) = if on_main {
        inbound_relation(&git, repo, local_sha.as_deref(), remote_sha.as_deref())?
    } else {
        (
            UserSkillsInboundRelation::Unknown,
            None,
            0,
            0,
            if git_status.branch.is_empty() {
                "Detached HEAD is not supported. Check out main with Git before checking incoming changes."
                    .to_string()
            } else {
                format!(
                    "User skills inbound sync requires main; current branch is '{}'.",
                    git_status.branch
                )
            },
        )
    };
    Ok(UserSkillsInboundStatus {
        repo_path: repo.to_path_buf(),
        branch: if git_status.branch.is_empty() && local_sha.is_none() {
            USER_SKILLS_REMOTE_BRANCH.to_string()
        } else if git_status.branch.is_empty() {
            "detached".to_string()
        } else {
            git_status.branch
        },
        remote_url: remote_url.as_deref().map(sanitize_git_remote_url),
        worktree_state: if git_status.dirty {
            UserSkillsInboundWorktreeState::Dirty
        } else {
            UserSkillsInboundWorktreeState::Clean
        },
        relation,
        local_sha,
        remote_sha,
        merge_base_sha,
        ahead_count,
        behind_count,
        fetched_at,
        fetch_error: None,
        message,
    })
}

fn inbound_relation(
    git: &skillbox_git::GitService,
    repo: &Path,
    local_sha: Option<&str>,
    remote_sha: Option<&str>,
) -> Result<(UserSkillsInboundRelation, Option<String>, u32, u32, String)> {
    match (local_sha, remote_sha) {
        (None, None) => Ok((
            UserSkillsInboundRelation::NoRemoteBranch,
            None,
            0,
            0,
            "origin/main has no history yet.".to_string(),
        )),
        (None, Some(remote)) => Ok((
            UserSkillsInboundRelation::RemoteOnly,
            None,
            0,
            git.commit_count(repo, remote)?,
            "Remote history is ready for reviewed initialization.".to_string(),
        )),
        (Some(local), None) => Ok((
            UserSkillsInboundRelation::Ahead,
            None,
            git.commit_count(repo, local)?,
            0,
            "Local history is ahead because origin/main has no commits.".to_string(),
        )),
        (Some(local), Some(remote)) if local == remote => Ok((
            UserSkillsInboundRelation::Synced,
            Some(local.to_string()),
            0,
            0,
            "Local and origin/main are synchronized.".to_string(),
        )),
        (Some(local), Some(remote)) => {
            let merge_base = git.merge_base(repo, local, remote)?;
            let (ahead, behind) = git.ahead_behind(repo, local, remote)?;
            let ahead = ahead as u32;
            let behind = behind as u32;
            let relation = if git.is_ancestor(repo, local, remote)? {
                UserSkillsInboundRelation::Behind
            } else if git.is_ancestor(repo, remote, local)? {
                UserSkillsInboundRelation::Ahead
            } else {
                UserSkillsInboundRelation::Diverged
            };
            let message = match relation {
                UserSkillsInboundRelation::Behind => {
                    format!("{behind} incoming commit(s) can be fast-forwarded after review.")
                }
                UserSkillsInboundRelation::Ahead => {
                    format!("Local history is {ahead} commit(s) ahead of origin/main.")
                }
                UserSkillsInboundRelation::Diverged => format!(
                    "Local and origin/main have diverged ({ahead} local, {behind} remote). Resolve with Git outside SkillBox."
                ),
                _ => unreachable!(),
            };
            Ok((relation, merge_base, ahead, behind, message))
        }
    }
}

fn preview_user_skills_inbound_for_paths(
    paths: &ManagedPaths,
    fetched_at: Option<String>,
) -> Result<UserSkillsInboundPreview> {
    let repo = &paths.user_skills_root;
    let git = skillbox_git::GitService::new();
    let status = inbound_status_from_refs(repo, fetched_at)?;
    let raw_remote_url = git.origin_url(repo)?.unwrap_or_default();
    let mut safety_issues = Vec::new();
    let mut old_snapshot = match status.local_sha.as_deref() {
        Some(local_sha) => {
            let mut historical_issues = Vec::new();
            validate_inbound_git_tree(&git, repo, local_sha, &mut historical_issues)?
        }
        None => InboundTreeSnapshot {
            skills: Vec::new(),
            entries: HashMap::new(),
        },
    };
    let mut new_snapshot = match status.remote_sha.as_deref() {
        Some(remote_sha) => validate_inbound_git_tree(&git, repo, remote_sha, &mut safety_issues)?,
        None => InboundTreeSnapshot {
            skills: Vec::new(),
            entries: HashMap::new(),
        },
    };
    old_snapshot
        .skills
        .sort_by(|left, right| left.name.cmp(&right.name));
    new_snapshot
        .skills
        .sort_by(|left, right| left.name.cmp(&right.name));
    let mut git_files = match (status.local_sha.as_deref(), status.remote_sha.as_deref()) {
        (Some(local_sha), Some(remote_sha)) => git.diff_refs(repo, local_sha, remote_sha)?,
        (None, Some(_)) => {
            let mut paths = new_snapshot.entries.keys().cloned().collect::<Vec<_>>();
            paths.sort();
            paths
                .into_iter()
                .map(|path| skillbox_git::GitDiffFile {
                    path: path.clone(),
                    old_path: None,
                    status: "A".to_string(),
                    diff: String::new(),
                })
                .collect()
        }
        _ => Vec::new(),
    };
    git_files.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then_with(|| left.old_path.cmp(&right.old_path))
            .then_with(|| left.status.cmp(&right.status))
    });
    append_local_content_collision_issues(
        &git,
        repo,
        &status,
        &old_snapshot.entries,
        &new_snapshot.entries,
        &mut safety_issues,
    )?;
    let files = git_files
        .iter()
        .cloned()
        .map(|file| {
            inbound_remote_diff_file(
                &git,
                repo,
                status.local_sha.as_deref(),
                status.remote_sha.as_deref(),
                &old_snapshot.entries,
                &new_snapshot.entries,
                file,
            )
        })
        .collect::<Result<Vec<_>>>()?;
    let (deployments, deployment_profiles) = inbound_user_skill_deployments(paths)?;
    let remote_names = scan_managed_remote_skills(paths)?
        .into_iter()
        .map(|skill| skill.name)
        .collect::<HashSet<_>>();
    for skill in &new_snapshot.skills {
        if remote_names.contains(&skill.name) {
            safety_issues.push(UserSkillsInboundSafetyIssue {
                code: "managed_skill_name_conflict".to_string(),
                message: format!(
                    "Incoming user skill '{}' conflicts with a managed remote skill.",
                    skill.name
                ),
                path: Some(skill.name.clone()),
                blocking: true,
            });
        }
    }
    let mut skill_changes = inbound_skill_changes(
        &old_snapshot.skills,
        &new_snapshot.skills,
        &git_files,
        &deployments,
        &deployment_profiles,
        &paths.user_skills_root,
        &mut safety_issues,
    );
    skill_changes.sort_by(|left, right| {
        left.skill_name
            .cmp(&right.skill_name)
            .then_with(|| left.previous_name.cmp(&right.previous_name))
    });
    let skill_names = old_snapshot
        .skills
        .iter()
        .chain(new_snapshot.skills.iter())
        .map(|skill| skill.name.as_str())
        .collect::<HashSet<_>>();
    let mut repository_files = files
        .iter()
        .filter(|file| {
            file.path
                .split('/')
                .next()
                .map(|root| !skill_names.contains(root))
                .unwrap_or(true)
        })
        .map(|file| file.path.clone())
        .collect::<Vec<_>>();
    repository_files.sort();
    let conflict_analysis = (status.relation == UserSkillsInboundRelation::Diverged)
        .then(|| inbound_conflict_analysis(&git, repo, &status))
        .transpose()?;
    if status.relation == UserSkillsInboundRelation::RemoteOnly
        && !remote_only_worktree_is_safe(repo)?
    {
        safety_issues.push(UserSkillsInboundSafetyIssue {
                code: "bootstrap_local_content".to_string(),
                message: "The local user-skills repository contains content. Move or commit it before initializing from origin/main.".to_string(),
                path: None,
                blocking: true,
            });
    }
    safety_issues.sort_by(|left, right| {
        left.code
            .cmp(&right.code)
            .then_with(|| left.path.cmp(&right.path))
            .then_with(|| left.message.cmp(&right.message))
            .then_with(|| left.blocking.cmp(&right.blocking))
    });
    let relation_allows_apply = matches!(
        status.relation,
        UserSkillsInboundRelation::Behind | UserSkillsInboundRelation::RemoteOnly
    );
    let clean = status.worktree_state == UserSkillsInboundWorktreeState::Clean
        || (status.relation == UserSkillsInboundRelation::RemoteOnly
            && remote_only_worktree_is_safe(repo)?);
    let blocked_reason = inbound_blocked_reason(&status, clean, &safety_issues);
    let can_apply =
        relation_allows_apply && clean && !safety_issues.iter().any(|issue| issue.blocking);
    let preview_id = inbound_preview_id(
        repo,
        &raw_remote_url,
        &status,
        &files,
        &skill_changes,
        &safety_issues,
    )?;
    Ok(UserSkillsInboundPreview {
        preview_id,
        status,
        files,
        skill_changes,
        repository_files,
        safety_issues,
        conflict_analysis,
        can_apply,
        blocked_reason,
    })
}

fn apply_user_skills_inbound_inner(
    managed_root: &Path,
    preview_id: &str,
    operation_id: &str,
    audit: &mut InboundApplyAudit,
) -> Result<UserSkillsInboundApplyResult> {
    let paths = managed_paths(managed_root.to_path_buf());
    let _lock = acquire_user_skills_mutation_lock(managed_root)?;
    let checked = check_user_skills_inbound_unlocked(managed_root)?;
    if checked.fetch_error.is_some() {
        return Err(checked
            .fetch_error
            .unwrap_or_else(|| "Unable to refresh origin/main.".to_string()));
    }
    let preview = preview_user_skills_inbound_for_paths(&paths, checked.fetched_at.clone())?;
    if preview.preview_id != preview_id {
        return Err(
            "Inbound preview is stale. Check remote and review incoming changes again.".to_string(),
        );
    }
    if !preview.can_apply {
        return Err(preview
            .blocked_reason
            .unwrap_or_else(|| "Incoming changes cannot be applied safely.".to_string()));
    }
    let new_sha = preview
        .status
        .remote_sha
        .clone()
        .ok_or_else(|| "origin/main has no commit to apply.".to_string())?;
    let old_sha = preview.status.local_sha.clone();
    audit.old_sha.clone_from(&old_sha);
    audit.new_sha = Some(new_sha.clone());
    let backup_ref = old_sha.as_deref().map(|_| {
        format!(
            "refs/skillbox/backups/inbound/{}",
            sanitize_operation_ref_component(operation_id)
        )
    });
    if let (Some(reference), Some(old)) = (backup_ref.as_deref(), old_sha.as_deref()) {
        skillbox_git::GitService::new().create_backup_ref(
            &paths.user_skills_root,
            reference,
            old,
        )?;
    }
    audit.backup_ref.clone_from(&backup_ref);
    audit.phase = "backup_created".to_string();
    let git = skillbox_git::GitService::new();
    let old_snapshot = match old_sha.as_deref() {
        Some(sha) => {
            validate_inbound_git_tree(&git, &paths.user_skills_root, sha, &mut Vec::new())?
        }
        None => InboundTreeSnapshot {
            skills: Vec::new(),
            entries: HashMap::new(),
        },
    };
    let new_snapshot =
        validate_inbound_git_tree(&git, &paths.user_skills_root, &new_sha, &mut Vec::new())?;
    audit.phase = "materializing".to_string();
    let mut mutation = apply_inbound_tree(
        &git,
        &paths.user_skills_root,
        old_sha.as_deref(),
        &new_sha,
        old_snapshot.entries,
        new_snapshot.entries,
        operation_id,
        &InboundMutationOptions::default(),
        audit,
    )?;
    audit.phase = "ref_updated".to_string();
    let post_mutation = (|| -> Result<()> {
        let applied_head = git
            .rev_parse_optional(&paths.user_skills_root, "HEAD")?
            .ok_or_else(|| "Fast-forward did not produce a local HEAD.".to_string())?;
        if applied_head != new_sha || git.status_hardened(&paths.user_skills_root)?.dirty {
            return Err(
                "Fast-forward target changed or the worktree became dirty before reindex."
                    .to_string(),
            );
        }
        let scan = scan_skill_roots(std::slice::from_ref(&paths.user_skills_root))?;
        if !scan.errors.is_empty() {
            return Err(format!(
                "Fast-forwarded tree failed reindex validation: {}",
                scan.errors
                    .iter()
                    .map(|item| item.error.as_str())
                    .collect::<Vec<_>>()
                    .join("; ")
            ));
        }
        audit.phase = "reindexing".to_string();
        reindex_user_skills(&paths.database_path, &scan.skills, &paths.user_skills_root)
            .map_err(|error| format!("Unable to reindex fast-forwarded user skills: {error}"))
    })();
    if let Err(error) = post_mutation {
        return rollback_inbound_after_failure(
            &git,
            &paths.user_skills_root,
            &mut mutation,
            &error,
            audit,
        );
    }
    if let Some(mut index_lock) = mutation.index_lock.take() {
        index_lock.release_if_owned().map_err(|error| {
            format!(
                "Inbound apply completed, but the Git index lock could not be released safely: {error}"
            )
        })?;
    }
    audit.phase = "completed".to_string();
    Ok(UserSkillsInboundApplyResult {
        repo_path: paths.user_skills_root,
        old_sha,
        new_sha,
        backup_ref,
        changed_skill_count: preview.skill_changes.len(),
        changed_file_count: preview.files.len(),
        operation_id: operation_id.to_string(),
    })
}

fn validate_inbound_git_tree(
    git: &skillbox_git::GitService,
    repo: &Path,
    revision: &str,
    issues: &mut Vec<UserSkillsInboundSafetyIssue>,
) -> Result<InboundTreeSnapshot> {
    let entries = git.list_tree(repo, revision)?;
    if entries.len() > MAX_INBOUND_TREE_ENTRIES {
        return Err(format!(
            "Incoming repository exceeds the {MAX_INBOUND_TREE_ENTRIES}-entry safety limit."
        ));
    }
    let mut total_bytes = 0u64;
    let mut entries_by_path = HashMap::new();
    for entry in entries {
        if !inbound_tree_path_is_safe(&entry.path) {
            issues.push(UserSkillsInboundSafetyIssue {
                code: "unsafe_path".to_string(),
                message: "Incoming repository contains an unsafe path.".to_string(),
                path: Some(entry.path.clone()),
                blocking: true,
            });
            continue;
        }
        if entry.object_type != "blob" || !matches!(entry.mode.as_str(), "100644" | "100755") {
            let (code, message) = if entry.mode == "120000" {
                (
                    "unsafe_symlink",
                    "Incoming symbolic links are not accepted in user-skills repositories.",
                )
            } else if entry.mode == "160000" || entry.object_type == "commit" {
                (
                    "unsafe_gitlink",
                    "Incoming Git submodules are not accepted in user-skills repositories.",
                )
            } else {
                (
                    "unsafe_file_type",
                    "Incoming repository contains an unsupported Git file type.",
                )
            };
            issues.push(UserSkillsInboundSafetyIssue {
                code: code.to_string(),
                message: message.to_string(),
                path: Some(entry.path.clone()),
                blocking: true,
            });
            continue;
        }
        let size = entry.size.unwrap_or(0);
        if size > MAX_INBOUND_FILE_BYTES {
            issues.push(UserSkillsInboundSafetyIssue {
                code: "file_too_large".to_string(),
                message: format!(
                    "Incoming file exceeds the {MAX_INBOUND_FILE_BYTES}-byte safety limit."
                ),
                path: Some(entry.path.clone()),
                blocking: true,
            });
        }
        total_bytes = total_bytes.saturating_add(size);
        if total_bytes > MAX_INBOUND_TREE_BYTES {
            return Err(format!(
                "Incoming repository exceeds the {MAX_INBOUND_TREE_BYTES}-byte safety limit."
            ));
        }
        entries_by_path.insert(entry.path.clone(), entry);
    }
    if entries_by_path.contains_key("SKILL.md") {
        issues.push(UserSkillsInboundSafetyIssue {
            code: "repository_root_skill".to_string(),
            message: "A shared user-skills repository cannot also be a repository-root skill."
                .to_string(),
            path: Some("SKILL.md".to_string()),
            blocking: true,
        });
    }
    let mut skills = Vec::new();
    let mut seen_names = HashSet::new();
    let mut skill_paths = entries_by_path
        .keys()
        .filter(|path| path.ends_with("/SKILL.md"))
        .cloned()
        .collect::<Vec<_>>();
    skill_paths.sort();
    for skill_md_path in skill_paths {
        let components = skill_md_path.split('/').collect::<Vec<_>>();
        if components.len() != 2 {
            issues.push(UserSkillsInboundSafetyIssue {
                code: "nested_skill".to_string(),
                message: "User skills must be top-level directories in the shared repository."
                    .to_string(),
                path: Some(skill_md_path),
                blocking: true,
            });
            continue;
        }
        let directory_name = components[0];
        if entries_by_path
            .get(&skill_md_path)
            .and_then(|entry| entry.size)
            .is_some_and(|size| size > MAX_TEXT_DIFF_PREVIEW_BYTES as u64)
        {
            issues.push(UserSkillsInboundSafetyIssue {
                code: "skill_file_too_large".to_string(),
                message: "Incoming SKILL.md exceeds the bounded validation limit.".to_string(),
                path: Some(skill_md_path),
                blocking: true,
            });
            continue;
        }
        let Some(bytes) = git.show_file(repo, revision, &skill_md_path)? else {
            issues.push(UserSkillsInboundSafetyIssue {
                code: "missing_skill_file".to_string(),
                message: "Incoming SKILL.md could not be read from the reviewed commit."
                    .to_string(),
                path: Some(skill_md_path),
                blocking: true,
            });
            continue;
        };
        let content = match std::str::from_utf8(&bytes) {
            Ok(content) => content,
            Err(_) => {
                issues.push(UserSkillsInboundSafetyIssue {
                    code: "invalid_skill_encoding".to_string(),
                    message: "Incoming SKILL.md must be UTF-8 text.".to_string(),
                    path: Some(skill_md_path),
                    blocking: true,
                });
                continue;
            }
        };
        let document = match parse_skill_frontmatter_document(content) {
            Ok(document) => document,
            Err(error) => {
                issues.push(UserSkillsInboundSafetyIssue {
                    code: "invalid_skill_frontmatter".to_string(),
                    message: error,
                    path: Some(skill_md_path),
                    blocking: true,
                });
                continue;
            }
        };
        let skill_name = if document.metadata.name.is_empty() {
            directory_name.to_string()
        } else {
            document.metadata.name.clone()
        };
        if let Err(error) = validate_skill_name(&skill_name) {
            issues.push(UserSkillsInboundSafetyIssue {
                code: "invalid_skill_name".to_string(),
                message: error,
                path: Some(directory_name.to_string()),
                blocking: true,
            });
            continue;
        }
        if skill_name != directory_name {
            issues.push(UserSkillsInboundSafetyIssue {
                code: "skill_directory_mismatch".to_string(),
                message: format!(
                    "Skill '{skill_name}' must be stored in a matching top-level directory."
                ),
                path: Some(directory_name.to_string()),
                blocking: true,
            });
            continue;
        }
        if !seen_names.insert(skill_name.clone()) {
            issues.push(UserSkillsInboundSafetyIssue {
                code: "duplicate_skill_name".to_string(),
                message: format!("Incoming repository contains duplicate skill '{skill_name}'."),
                path: Some(directory_name.to_string()),
                blocking: true,
            });
            continue;
        }
        let skill_path = repo.join(directory_name);
        skills.push(Skill {
            name: skill_name,
            description: document.metadata.description,
            version: document.metadata.version,
            path: skill_path.clone(),
            skill_md_path: skill_path.join("SKILL.md"),
            content_hash: sha256_bytes(&bytes),
            source_root: Some(repo.to_path_buf()),
            is_symlink: false,
            real_path: skill_path,
        });
    }
    Ok(InboundTreeSnapshot {
        skills,
        entries: entries_by_path,
    })
}

fn inbound_tree_path_is_safe(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.contains(['\\', ':'])
        && !path.chars().any(char::is_control)
        && path.split('/').all(|component| {
            !component.is_empty() && component != "." && component != ".." && component != ".git"
        })
}

#[allow(clippy::too_many_arguments)]
fn inbound_remote_diff_file(
    git: &skillbox_git::GitService,
    repo: &Path,
    old_revision: Option<&str>,
    new_revision: Option<&str>,
    old_entries: &HashMap<String, skillbox_git::GitTreeEntry>,
    new_entries: &HashMap<String, skillbox_git::GitTreeEntry>,
    file: skillbox_git::GitDiffFile,
) -> Result<RemoteDiffFile> {
    let old_path = file.old_path.as_deref().unwrap_or(&file.path);
    let old_entry = old_entries.get(old_path);
    let new_entry = new_entries.get(&file.path);
    let old_size = old_entry.and_then(|entry| entry.size);
    let new_size = new_entry.and_then(|entry| entry.size);
    let too_large = old_size
        .into_iter()
        .chain(new_size)
        .any(|size| size > MAX_TEXT_DIFF_PREVIEW_BYTES as u64);
    let old_bytes = if !too_large {
        match (old_revision, old_entry) {
            (Some(revision), Some(_)) => git.show_file(repo, revision, old_path)?,
            _ => None,
        }
    } else {
        None
    };
    let new_bytes = if !too_large {
        match (new_revision, new_entry) {
            (Some(revision), Some(_)) => git.show_file(repo, revision, &file.path)?,
            _ => None,
        }
    } else {
        None
    };
    let binary = old_bytes
        .as_deref()
        .into_iter()
        .chain(new_bytes.as_deref())
        .any(|bytes| std::str::from_utf8(bytes).is_err());
    let diff = if binary || too_large {
        String::new()
    } else if !file.diff.is_empty() {
        file.diff
    } else if let Some(bytes) = new_bytes.as_deref() {
        let content = std::str::from_utf8(bytes).unwrap_or_default();
        let mut generated = format!("--- /dev/null\n+++ b/{}\n", file.path);
        for line in content.lines() {
            generated.push('+');
            generated.push_str(line);
            generated.push('\n');
            if generated.len() >= MAX_TEXT_DIFF_PREVIEW_BYTES {
                generated.truncate(MAX_TEXT_DIFF_PREVIEW_BYTES);
                break;
            }
        }
        generated
    } else {
        String::new()
    };
    Ok(RemoteDiffFile {
        path: file.path,
        old_path: file.old_path,
        status: file.status.clone(),
        label: remote_diff_label(&file.status).to_string(),
        diff,
        old_hash: old_entry.map(|entry| entry.object_id.clone()),
        new_hash: new_entry.map(|entry| entry.object_id.clone()),
        old_size,
        new_size,
        binary,
        too_large,
    })
}

fn inbound_user_skill_deployments(
    paths: &ManagedPaths,
) -> Result<(InboundDeployments, InboundDeploymentProfiles)> {
    let mut deployments = load_deployments(&paths.database_path)?;
    let workspaces = load_workspaces(&paths.database_path)?;
    let profiles = workspaces
        .iter()
        .map(|workspace| {
            (
                workspace.path.clone(),
                (workspace.profile_id.clone(), workspace.profile_name.clone()),
            )
        })
        .collect();
    let skills = scan_skill_roots(std::slice::from_ref(&paths.user_skills_root))?
        .skills
        .into_iter()
        .map(|skill| managed_skill(skill, SkillKind::User))
        .collect::<Vec<_>>();
    merge_workspace_symlink_deployments(&workspaces, &skills, &mut deployments);
    let user_names = skills
        .iter()
        .map(|skill| skill.name.as_str())
        .collect::<HashSet<_>>();
    deployments.retain(|name, _| user_names.contains(name.as_str()));
    Ok((deployments, profiles))
}

fn inbound_skill_changes(
    old_skills: &[Skill],
    new_skills: &[Skill],
    files: &[skillbox_git::GitDiffFile],
    deployments: &InboundDeployments,
    deployment_profiles: &InboundDeploymentProfiles,
    user_skills_root: &Path,
    issues: &mut Vec<UserSkillsInboundSafetyIssue>,
) -> Vec<UserSkillsInboundSkillChange> {
    let old_names = old_skills
        .iter()
        .map(|skill| skill.name.as_str())
        .collect::<HashSet<_>>();
    let new_names = new_skills
        .iter()
        .map(|skill| skill.name.as_str())
        .collect::<HashSet<_>>();
    let mut renamed = HashMap::new();
    for file in files {
        if !file.status.starts_with('R') {
            continue;
        }
        let Some(old_path) = file.old_path.as_deref() else {
            continue;
        };
        let Some(old_root) = old_path.split('/').next() else {
            continue;
        };
        let Some(new_root) = file.path.split('/').next() else {
            continue;
        };
        if old_root != new_root
            && old_names.contains(old_root)
            && new_names.contains(new_root)
            && !new_names.contains(old_root)
            && !old_names.contains(new_root)
        {
            renamed.insert(old_root.to_string(), new_root.to_string());
        }
    }
    let mut changes = Vec::new();
    for skill in old_skills {
        if let Some(new_name) = renamed.get(&skill.name) {
            let affected = inbound_affected_deployments(
                deployments,
                deployment_profiles,
                user_skills_root,
                &skill.name,
            );
            if !affected.is_empty() {
                issues.push(UserSkillsInboundSafetyIssue {
                    code: "deployed_skill_renamed".to_string(),
                    message: format!(
                        "Undeploy '{}' before applying an incoming rename.",
                        skill.name
                    ),
                    path: Some(skill.name.clone()),
                    blocking: true,
                });
            }
            changes.push(UserSkillsInboundSkillChange {
                skill_name: new_name.clone(),
                previous_name: Some(skill.name.clone()),
                kind: UserSkillsInboundSkillChangeKind::Renamed,
                files: inbound_files_for_skill(files, &skill.name, Some(new_name.as_str())),
                affected_deployments: affected,
            });
        } else if !new_names.contains(skill.name.as_str()) {
            let affected = inbound_affected_deployments(
                deployments,
                deployment_profiles,
                user_skills_root,
                &skill.name,
            );
            if !affected.is_empty() {
                issues.push(UserSkillsInboundSafetyIssue {
                    code: "deployed_skill_deleted".to_string(),
                    message: format!(
                        "Undeploy '{}' before applying an incoming deletion.",
                        skill.name
                    ),
                    path: Some(skill.name.clone()),
                    blocking: true,
                });
            }
            changes.push(UserSkillsInboundSkillChange {
                skill_name: skill.name.clone(),
                previous_name: None,
                kind: UserSkillsInboundSkillChangeKind::Deleted,
                files: inbound_files_for_skill(files, &skill.name, None),
                affected_deployments: affected,
            });
        }
    }
    for skill in new_skills {
        if renamed.values().any(|name| name == &skill.name) {
            continue;
        }
        if !old_names.contains(skill.name.as_str()) {
            changes.push(UserSkillsInboundSkillChange {
                skill_name: skill.name.clone(),
                previous_name: None,
                kind: UserSkillsInboundSkillChangeKind::Added,
                files: inbound_files_for_skill(files, &skill.name, None),
                affected_deployments: Vec::new(),
            });
        } else {
            let changed_files = inbound_files_for_skill(files, &skill.name, None);
            if !changed_files.is_empty() {
                changes.push(UserSkillsInboundSkillChange {
                    skill_name: skill.name.clone(),
                    previous_name: None,
                    kind: UserSkillsInboundSkillChangeKind::Updated,
                    files: changed_files,
                    affected_deployments: inbound_affected_deployments(
                        deployments,
                        deployment_profiles,
                        user_skills_root,
                        &skill.name,
                    ),
                });
            }
        }
    }
    changes.sort_by(|left, right| left.skill_name.cmp(&right.skill_name));
    changes
}

fn inbound_files_for_skill(
    files: &[skillbox_git::GitDiffFile],
    skill_name: &str,
    renamed_to: Option<&str>,
) -> Vec<String> {
    let mut paths = files
        .iter()
        .filter(|file| {
            git_path_belongs_to_skill(&file.path, skill_name)
                || file
                    .old_path
                    .as_deref()
                    .map(|path| git_path_belongs_to_skill(path, skill_name))
                    .unwrap_or(false)
                || renamed_to
                    .map(|name| git_path_belongs_to_skill(&file.path, name))
                    .unwrap_or(false)
        })
        .map(|file| file.path.clone())
        .collect::<Vec<_>>();
    paths.sort();
    paths.dedup();
    paths
}

fn inbound_affected_deployments(
    deployments: &HashMap<String, Vec<ManagedSkillDeployment>>,
    deployment_profiles: &HashMap<PathBuf, (String, String)>,
    user_skills_root: &Path,
    skill_name: &str,
) -> Vec<UserSkillsInboundAffectedDeployment> {
    let mut affected = deployments
        .get(skill_name)
        .into_iter()
        .flatten()
        .map(|deployment| {
            let (profile_id, profile_name) = deployment_profiles
                .get(&deployment.target_root)
                .cloned()
                .unwrap_or_else(|| {
                    let profile = resolve_runtime_profile_for_root(&deployment.target_root).0;
                    (profile.id, profile.display_name)
                });
            let expected = user_skills_root.join(skill_name);
            let follows_user_skill = fs::symlink_metadata(&deployment.target_path)
                .map(|metadata| metadata.file_type().is_symlink())
                .unwrap_or(false)
                && fs::canonicalize(&deployment.target_path)
                    .ok()
                    .zip(fs::canonicalize(&expected).ok())
                    .is_some_and(|(actual, expected)| actual == expected);
            UserSkillsInboundAffectedDeployment {
                target_root: deployment.target_root.clone(),
                target_path: deployment.target_path.clone(),
                mode: deployment.mode.clone(),
                profile_id,
                profile_name,
                state: if follows_user_skill {
                    "follows_user_skill"
                } else {
                    "target_changed"
                }
                .to_string(),
                message: if follows_user_skill {
                    "This runtime target follows the managed user skill."
                } else {
                    "The recorded runtime target no longer matches the managed user skill."
                }
                .to_string(),
            }
        })
        .collect::<Vec<_>>();
    affected.sort_by(|left, right| {
        left.target_root
            .cmp(&right.target_root)
            .then_with(|| left.target_path.cmp(&right.target_path))
            .then_with(|| left.profile_id.cmp(&right.profile_id))
    });
    affected
}

fn inbound_conflict_analysis(
    git: &skillbox_git::GitService,
    repo: &Path,
    status: &UserSkillsInboundStatus,
) -> Result<UserSkillsInboundConflictAnalysis> {
    let Some(base) = status.merge_base_sha.as_deref() else {
        return Ok(UserSkillsInboundConflictAnalysis {
            local_only_commits: status.ahead_count,
            remote_only_commits: status.behind_count,
            ..Default::default()
        });
    };
    let local = status
        .local_sha
        .as_deref()
        .ok_or_else(|| "Local SHA is missing for conflict analysis.".to_string())?;
    let remote = status
        .remote_sha
        .as_deref()
        .ok_or_else(|| "Remote SHA is missing for conflict analysis.".to_string())?;
    let local_changes = changed_path_statuses(git.diff_refs(repo, base, local)?);
    let remote_changes = changed_path_statuses(git.diff_refs(repo, base, remote)?);
    let local_files = local_changes.keys().cloned().collect::<HashSet<_>>();
    let remote_files = remote_changes.keys().cloned().collect::<HashSet<_>>();
    let mut both_changed_files = local_files
        .intersection(&remote_files)
        .cloned()
        .collect::<Vec<_>>();
    both_changed_files.sort();
    let local_skills = validate_inbound_git_tree(git, repo, local, &mut Vec::new())?;
    let remote_skills = validate_inbound_git_tree(git, repo, remote, &mut Vec::new())?;
    let mut valid_skill_names = local_skills
        .skills
        .iter()
        .chain(remote_skills.skills.iter())
        .map(|skill| skill.name.as_str())
        .map(str::to_string)
        .collect::<HashSet<_>>();
    let base_candidates = local_files
        .iter()
        .chain(remote_files.iter())
        .filter_map(|path| path.split('/').next())
        .filter(|name| !valid_skill_names.contains(*name) && validate_skill_name(name).is_ok())
        .map(str::to_string)
        .collect::<HashSet<_>>();
    add_base_skill_names(&mut valid_skill_names, base_candidates, |name| {
        git.tree_path_exists(repo, base, &format!("{name}/SKILL.md"))
    })?;
    let local_changed_skills = local_files
        .iter()
        .filter_map(|path| path.split('/').next())
        .filter(|name| valid_skill_names.contains(*name))
        .collect::<HashSet<_>>();
    let remote_changed_skills = remote_files
        .iter()
        .filter_map(|path| path.split('/').next())
        .filter(|name| valid_skill_names.contains(*name))
        .collect::<HashSet<_>>();
    let mut both_changed_skills = local_changed_skills
        .intersection(&remote_changed_skills)
        .map(|name| (*name).to_string())
        .collect::<Vec<_>>();
    both_changed_skills.sort();
    both_changed_skills.dedup();
    let likely_conflict_files = both_changed_files
        .iter()
        .filter(|path| {
            !matches!(
                (
                    local_changes.get(path.as_str()).map(String::as_str),
                    remote_changes.get(path.as_str()).map(String::as_str)
                ),
                (Some("D"), Some("D"))
            )
        })
        .cloned()
        .collect();
    Ok(UserSkillsInboundConflictAnalysis {
        local_only_commits: status.ahead_count,
        remote_only_commits: status.behind_count,
        likely_conflict_files,
        both_changed_files,
        both_changed_skills,
    })
}

fn changed_path_statuses(files: Vec<skillbox_git::GitDiffFile>) -> HashMap<String, String> {
    let mut changed = HashMap::new();
    for file in files {
        let status = file.status.chars().next().unwrap_or('M').to_string();
        if let Some(old_path) = file.old_path {
            if status == "R" {
                changed.insert(old_path, "D".to_string());
                changed.insert(file.path, "A".to_string());
                continue;
            }
            changed.entry(old_path).or_insert_with(|| status.clone());
        }
        changed.insert(file.path, status);
    }
    changed
}

fn add_base_skill_names<F>(
    valid_skill_names: &mut HashSet<String>,
    candidates: HashSet<String>,
    mut tree_path_exists: F,
) -> Result<()>
where
    F: FnMut(&str) -> Result<bool>,
{
    let mut candidates = candidates.into_iter().collect::<Vec<_>>();
    candidates.sort();
    for name in candidates {
        if tree_path_exists(&name)? {
            valid_skill_names.insert(name);
        }
    }
    Ok(())
}

fn remote_only_worktree_is_safe(repo: &Path) -> Result<bool> {
    for entry in fs::read_dir(repo).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let name = entry.file_name();
        if name == ".git" {
            continue;
        }
        if name == ".gitignore"
            && is_real_regular_file(&entry.path())?
            && fs::read_to_string(entry.path()).map_err(|error| error.to_string())?
                == DEFAULT_USER_SKILLS_GITIGNORE
        {
            continue;
        }
        return Ok(false);
    }
    Ok(true)
}

fn append_local_content_collision_issues(
    git: &skillbox_git::GitService,
    repo: &Path,
    status: &UserSkillsInboundStatus,
    old_entries: &HashMap<String, skillbox_git::GitTreeEntry>,
    new_entries: &HashMap<String, skillbox_git::GitTreeEntry>,
    issues: &mut Vec<UserSkillsInboundSafetyIssue>,
) -> Result<()> {
    let mut local_paths = git.untracked_and_ignored_paths(repo)?;
    if status.relation == UserSkillsInboundRelation::RemoteOnly
        && is_real_regular_file(&repo.join(".gitignore"))?
        && fs::read_to_string(repo.join(".gitignore")).ok().as_deref()
            == Some(DEFAULT_USER_SKILLS_GITIGNORE)
    {
        local_paths.retain(|path| path != ".gitignore");
    }
    let incoming_paths = new_entries
        .keys()
        .filter(|path| !old_entries.contains_key(path.as_str()))
        .collect::<Vec<_>>();
    let mut collisions = HashSet::new();
    for local in &local_paths {
        for incoming in &incoming_paths {
            if paths_overlap(local, incoming)
                || share_untracked_directory(local, incoming, old_entries)
            {
                collisions.insert(local.clone());
            }
        }
    }
    let mut collisions = collisions.into_iter().collect::<Vec<_>>();
    collisions.sort();
    for path in collisions {
        issues.push(UserSkillsInboundSafetyIssue {
            code: "local_content_collision".to_string(),
            message: "Incoming changes collide with ignored or untracked local content. Move or commit the local content before applying.".to_string(),
            path: Some(path),
            blocking: true,
        });
    }
    Ok(())
}

fn is_real_regular_file(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => Ok(metadata.is_file() && !metadata.file_type().is_symlink()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.to_string()),
    }
}

fn paths_overlap(left: &str, right: &str) -> bool {
    left == right
        || left
            .strip_prefix(right)
            .is_some_and(|suffix| suffix.starts_with('/'))
        || right
            .strip_prefix(left)
            .is_some_and(|suffix| suffix.starts_with('/'))
}

fn share_untracked_directory(
    left: &str,
    right: &str,
    old_entries: &HashMap<String, skillbox_git::GitTreeEntry>,
) -> bool {
    let common = left
        .split('/')
        .zip(right.split('/'))
        .take_while(|(left, right)| left == right)
        .map(|(part, _)| part)
        .collect::<Vec<_>>();
    if common.is_empty() {
        return false;
    }
    let prefix = format!("{}/", common.join("/"));
    !old_entries.keys().any(|path| path.starts_with(&prefix))
}

fn inbound_blocked_reason(
    status: &UserSkillsInboundStatus,
    clean: bool,
    issues: &[UserSkillsInboundSafetyIssue],
) -> Option<String> {
    if !clean {
        return Some(
            "Local changes are present. Commit or discard them with Git before applying incoming changes."
                .to_string(),
        );
    }
    if issues.iter().any(|issue| issue.blocking) {
        return Some("Incoming changes failed the user-skills safety review.".to_string());
    }
    match status.relation {
        UserSkillsInboundRelation::Behind | UserSkillsInboundRelation::RemoteOnly => None,
        UserSkillsInboundRelation::Diverged => Some(
            "Histories diverged. Resolve with normal Git tooling outside SkillBox, then refresh."
                .to_string(),
        ),
        UserSkillsInboundRelation::Ahead => {
            Some("Local history is ahead; there are no fast-forward changes to apply.".to_string())
        }
        UserSkillsInboundRelation::Synced => {
            Some("Local and origin/main are already synchronized.".to_string())
        }
        UserSkillsInboundRelation::NoRemoteBranch => {
            Some("origin/main has no history to apply.".to_string())
        }
        UserSkillsInboundRelation::Unknown => {
            Some("Check origin/main before reviewing incoming changes.".to_string())
        }
    }
}

fn inbound_preview_id(
    repo: &Path,
    remote_url: &str,
    status: &UserSkillsInboundStatus,
    files: &[RemoteDiffFile],
    skills: &[UserSkillsInboundSkillChange],
    issues: &[UserSkillsInboundSafetyIssue],
) -> Result<String> {
    let seed = serde_json::json!({
        "schema": "user-skills-inbound-v1",
        "repo": fs::canonicalize(repo).unwrap_or_else(|_| repo.to_path_buf()),
        "remoteIdentity": sanitize_git_remote_url(remote_url),
        "branch": USER_SKILLS_REMOTE_BRANCH,
        "localSha": status.local_sha,
        "remoteSha": status.remote_sha,
        "mergeBaseSha": status.merge_base_sha,
        "relation": status.relation,
        "worktreeState": status.worktree_state,
        "worktreeFingerprint": content_hash_text(&skillbox_git::GitService::new().status_hardened(repo)?.raw_status),
        "files": files.iter().map(|file| (&file.path, &file.old_path, &file.status, &file.old_hash, &file.new_hash)).collect::<Vec<_>>(),
        "skills": skills.iter().map(|skill| (&skill.skill_name, &skill.previous_name, skill.kind, skill.affected_deployments.iter().map(|deployment| (&deployment.target_root, &deployment.target_path, &deployment.mode, &deployment.profile_id, &deployment.state)).collect::<Vec<_>>())).collect::<Vec<_>>(),
        "issues": issues.iter().map(|issue| (&issue.code, &issue.path, issue.blocking)).collect::<Vec<_>>()
    });
    serde_json::to_string(&seed)
        .map(|value| content_hash_text(&value))
        .map_err(|error| error.to_string())
}

fn sanitize_operation_ref_component(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                character
            } else {
                '-'
            }
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn apply_inbound_tree(
    git: &skillbox_git::GitService,
    repo: &Path,
    old_sha: Option<&str>,
    new_sha: &str,
    old_entries: HashMap<String, skillbox_git::GitTreeEntry>,
    new_entries: HashMap<String, skillbox_git::GitTreeEntry>,
    operation_id: &str,
    options: &InboundMutationOptions,
    audit: &mut InboundApplyAudit,
) -> Result<InboundMutationReceipt> {
    if git.rev_parse_optional(repo, "HEAD")?.as_deref() != old_sha {
        return Err("Local HEAD changed before inbound materialization.".to_string());
    }
    let index_before_lock = read_optional_file(&repo.join(".git/index"))?;
    let index_lock = acquire_git_index_lock(repo)?;
    if read_optional_file(&repo.join(".git/index"))? != index_before_lock {
        return Err(
            "Git index changed while inbound apply was starting. Review again.".to_string(),
        );
    }
    if old_sha.is_some() && git.status_hardened(repo)?.dirty {
        return Err(
            "Local user-skills changes appeared before inbound apply. Review again.".to_string(),
        );
    }
    if old_sha.is_none() && !remote_only_worktree_is_safe(repo)? {
        return Err(
            "Remote-only initialization requires an empty repository with only SkillBox's generated .gitignore."
                .to_string(),
        );
    }
    let mut touched_paths = old_entries
        .keys()
        .chain(new_entries.keys())
        .filter(|path| {
            old_entries
                .get(path.as_str())
                .map(|entry| (&entry.object_id, &entry.mode))
                != new_entries
                    .get(path.as_str())
                    .map(|entry| (&entry.object_id, &entry.mode))
        })
        .cloned()
        .collect::<Vec<_>>();
    touched_paths.sort();
    touched_paths.dedup();
    verify_worktree_matches_entries(git, repo, old_sha, &old_entries, &touched_paths)?;
    verify_added_paths_absent(repo, &old_entries, &new_entries, &touched_paths)?;
    let restore_generated_gitignore = old_sha.is_none()
        && is_real_regular_file(&repo.join(".gitignore"))?
        && fs::read_to_string(repo.join(".gitignore")).ok().as_deref()
            == Some(DEFAULT_USER_SKILLS_GITIGNORE);
    let index_temporary = repo.join(".git").join(format!(
        "skillbox-inbound-index-{}.tmp",
        sanitize_operation_ref_component(operation_id)
    ));
    let mut receipt = InboundMutationReceipt {
        repo_dir: open_real_directory(repo)?,
        old_sha: old_sha.map(str::to_string),
        new_sha: new_sha.to_string(),
        old_entries,
        new_entries,
        touched_paths,
        written_paths: HashSet::new(),
        deleted_paths: HashSet::new(),
        created_dirs: HashSet::new(),
        path_backups: HashMap::new(),
        old_index: read_optional_file(&repo.join(".git/index"))?,
        index_replaced: false,
        index_lock: Some(index_lock),
        ref_advanced: false,
        restore_generated_gitignore,
    };
    let result = (|| -> Result<()> {
        if restore_generated_gitignore {
            fs::remove_file(repo.join(".gitignore")).map_err(|error| error.to_string())?;
        }
        if options.fail_before_index_prepare {
            return Err("Injected inbound index preparation failure.".to_string());
        }
        git.prepare_index_tree(repo, Some(new_sha), &index_temporary)?;
        if let Some(barrier) = options.pause_before_materialization.as_deref() {
            wait_for_test_barrier(barrier)?;
        }
        Ok(())
    })()
    .and_then(|()| apply_inbound_tree_changes(git, repo, &mut receipt, operation_id, options))
    .and_then(|()| verify_operation_owned_paths(git, repo, &receipt))
    .and_then(|()| verify_inbound_backups(git, repo, &receipt))
    .and_then(|()| replace_git_index(repo, &index_temporary, &mut receipt))
    .and_then(|()| {
        if options.fail_after_index_replace {
            return Err("Injected inbound failure after index replacement.".to_string());
        }
        if let Some(barrier) = options.pause_before_ref_update.as_deref() {
            wait_for_test_barrier(barrier)?;
        }
        git.update_main_ref_cas(repo, new_sha, old_sha)?;
        receipt.ref_advanced = true;
        verify_operation_owned_paths(git, repo, &receipt)?;
        verify_inbound_backups(git, repo, &receipt)?;
        Ok(())
    });
    let _ = fs::remove_file(&index_temporary);
    if let Err(error) = result {
        if let Some(barrier) = options.pause_before_compensation.as_deref() {
            wait_for_test_barrier(barrier)?;
        }
        audit.compensation_attempted = true;
        return match compensate_inbound_tree(git, repo, &mut receipt, operation_id) {
            Ok(()) => {
                audit.compensation_succeeded = Some(true);
                Err(format!("{error} The previous Git state was restored."))
            }
            Err(compensation) => {
                audit.compensation_succeeded = Some(false);
                audit.compensation_error = Some(compensation.clone());
                Err(format!(
                    "{error} Automatic recovery was refused or failed: {compensation}."
                ))
            }
        };
    }
    Ok(receipt)
}

fn apply_inbound_tree_changes(
    git: &skillbox_git::GitService,
    repo: &Path,
    receipt: &mut InboundMutationReceipt,
    operation_id: &str,
    options: &InboundMutationOptions,
) -> Result<()> {
    let mut deletions = receipt
        .touched_paths
        .iter()
        .filter(|path| !receipt.new_entries.contains_key(path.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    deletions.sort_by_key(|path| std::cmp::Reverse(path.matches('/').count()));
    for path in deletions {
        let target = repo.join(&path);
        let backup =
            move_to_inbound_backup(receipt, &path, operation_id, options).map_err(|error| {
                format!("Unable to preserve incoming-deleted path '{path}': {error}")
            })?;
        receipt.path_backups.insert(path.clone(), backup);
        receipt.deleted_paths.insert(path);
        remove_empty_inbound_parents(&target, repo)?;
    }

    let mut writes = receipt
        .touched_paths
        .iter()
        .filter_map(|path| {
            receipt
                .new_entries
                .get(path)
                .map(|entry| (path.clone(), entry.clone()))
        })
        .collect::<Vec<_>>();
    writes.sort_by(|left, right| left.0.cmp(&right.0));
    for (index, (path, entry)) in writes.into_iter().enumerate() {
        if options.fail_after_writes == Some(index) {
            return Err("Injected inbound materialization failure.".to_string());
        }
        let bytes = git
            .show_file(repo, &receipt.new_sha, &path)?
            .ok_or_else(|| format!("Reviewed Git blob disappeared: {path}"))?;
        let replace_existing = receipt.old_entries.contains_key(&path);
        if replace_existing && !receipt.path_backups.contains_key(&path) {
            let backup = move_to_inbound_backup(receipt, &path, operation_id, options)
                .map_err(|error| format!("Unable to preserve tracked path '{path}': {error}"))?;
            receipt.path_backups.insert(path.clone(), backup);
        }
        write_inbound_file(
            repo,
            &path,
            &bytes,
            entry.mode == "100755",
            operation_id,
            &mut receipt.created_dirs,
        )?;
        receipt.written_paths.insert(path);
    }
    Ok(())
}

fn verify_worktree_matches_entries(
    git: &skillbox_git::GitService,
    repo: &Path,
    old_sha: Option<&str>,
    entries: &HashMap<String, skillbox_git::GitTreeEntry>,
    touched_paths: &[String],
) -> Result<()> {
    for path in touched_paths {
        let Some(_entry) = entries.get(path) else {
            continue;
        };
        let target = repo.join(path);
        let metadata = fs::symlink_metadata(&target)
            .map_err(|_| format!("Tracked path changed before apply: {path}"))?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(format!("Tracked path changed type before apply: {path}"));
        }
        let expected = git
            .show_file(
                repo,
                old_sha
                    .ok_or_else(|| "Missing old SHA for tracked path verification.".to_string())?,
                path,
            )?
            .ok_or_else(|| format!("Unable to read tracked blob for {path}"))?;
        let actual = fs::read(&target)
            .map_err(|error| format!("Unable to verify tracked path '{path}': {error}"))?;
        if actual != expected {
            return Err(format!("Tracked path changed before apply: {path}"));
        }
    }
    Ok(())
}

fn verify_added_paths_absent(
    repo: &Path,
    old_entries: &HashMap<String, skillbox_git::GitTreeEntry>,
    new_entries: &HashMap<String, skillbox_git::GitTreeEntry>,
    touched_paths: &[String],
) -> Result<()> {
    for path in touched_paths {
        if old_entries.contains_key(path) || !new_entries.contains_key(path) {
            continue;
        }
        if fs::symlink_metadata(repo.join(path)).is_ok() {
            return Err(format!(
                "Incoming path collides with local content before apply: {path}"
            ));
        }
        ensure_inbound_parent_chain(repo, path, false, None)?;
    }
    Ok(())
}

fn write_inbound_file(
    repo: &Path,
    relative_path: &str,
    bytes: &[u8],
    executable: bool,
    operation_id: &str,
    created_dirs: &mut HashSet<String>,
) -> Result<()> {
    let parent = ensure_inbound_parent_chain(repo, relative_path, true, Some(created_dirs))?;
    let file_name = Path::new(relative_path)
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("Incoming path has no file name: {relative_path}"))?;
    let temporary = parent.join(format!(
        ".{file_name}.skillbox-inbound-{}.tmp",
        sanitize_operation_ref_component(operation_id)
    ));
    let mut options = fs::OpenOptions::new();
    options.create_new(true).write(true);
    let result = (|| -> Result<()> {
        let mut file = options
            .open(&temporary)
            .map_err(|error| format!("Unable to create inbound temporary file: {error}"))?;
        use std::io::Write;
        file.write_all(bytes).map_err(|error| error.to_string())?;
        file.sync_all().map_err(|error| error.to_string())?;
        fs::set_permissions(
            &temporary,
            fs::Permissions::from_mode(if executable { 0o755 } else { 0o644 }),
        )
        .map_err(|error| error.to_string())?;
        let target = repo.join(relative_path);
        rename_inbound_no_replace(&temporary, &target).map_err(|error| {
            format!(
                "Incoming path collided with local content during apply: {relative_path}: {error}"
            )
        })
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn rename_inbound_no_replace(source: &Path, target: &Path) -> std::io::Result<()> {
    fs::hard_link(source, target)?;
    if let Err(error) = fs::remove_file(source) {
        let _ = fs::remove_file(target);
        return Err(error);
    }
    Ok(())
}

fn acquire_git_index_lock(repo: &Path) -> Result<GitIndexLock> {
    let repo_dir = open_real_directory(repo)?;
    let git_dir = open_real_child_directory(&repo_dir, ".git")?;
    let fd = rustix::fs::openat(
        &git_dir,
        "index.lock",
        OFlags::CREATE
            | OFlags::EXCL
            | OFlags::RDWR
            | OFlags::CLOEXEC
            | OFlags::NOFOLLOW,
        Mode::from_bits_truncate(0o600),
    )
    .map_err(|error| {
            format!(
                "Git index is being changed by another process. Wait for it to finish and retry: {error}"
            )
        })?;
    let file = File::from(fd);
    let metadata = file.metadata().map_err(|error| error.to_string())?;
    Ok(GitIndexLock {
        git_dir,
        device: metadata.dev(),
        inode: metadata.ino(),
        file,
    })
}

fn replace_git_index(
    repo: &Path,
    prepared_index: &Path,
    receipt: &mut InboundMutationReceipt,
) -> Result<()> {
    fs::rename(prepared_index, repo.join(".git/index"))
        .map_err(|error| format!("Unable to install reviewed Git index: {error}"))?;
    receipt.index_replaced = true;
    Ok(())
}

fn restore_git_index(repo: &Path, receipt: &mut InboundMutationReceipt) -> Result<()> {
    if !receipt.index_replaced {
        return Ok(());
    }
    let index = repo.join(".git/index");
    match receipt.old_index.as_deref() {
        Some(bytes) => {
            let temporary = repo.join(".git").join(format!(
                "skillbox-index-restore-{}.tmp",
                sanitize_operation_ref_component(&receipt.new_sha)
            ));
            fs::write(&temporary, bytes).map_err(|error| error.to_string())?;
            fs::rename(&temporary, &index).map_err(|error| error.to_string())?;
        }
        None => match fs::remove_file(&index) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.to_string()),
        },
    }
    receipt.index_replaced = false;
    Ok(())
}

fn read_optional_file(path: &Path) -> Result<Option<Vec<u8>>> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}

fn move_to_inbound_backup(
    receipt: &InboundMutationReceipt,
    relative_path: &str,
    operation_id: &str,
    options: &InboundMutationOptions,
) -> Result<InboundBackupSlot> {
    let operation = sanitize_operation_ref_component(operation_id);
    if operation.is_empty() {
        return Err("Inbound recovery operation id is empty.".to_string());
    }
    let mut parent = open_real_child_directory(&receipt.repo_dir, ".git")?;
    for component in ["skillbox", "inbound-worktree-backups", operation.as_str()] {
        parent = open_or_create_real_child_directory(&parent, component)?;
    }
    let relative_parent = Path::new(relative_path)
        .parent()
        .ok_or_else(|| "Inbound recovery backup has no parent.".to_string())?;
    for component in relative_parent.components() {
        let std::path::Component::Normal(component) = component else {
            return Err("Inbound recovery path is unsafe.".to_string());
        };
        let component = component
            .to_str()
            .ok_or_else(|| "Inbound recovery path is not valid UTF-8.".to_string())?;
        parent = open_or_create_real_child_directory(&parent, component)?;
    }
    let file_name = Path::new(relative_path)
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "Inbound recovery backup has no file name.".to_string())?;
    let source_parent = open_relative_parent(&receipt.repo_dir, relative_path, false)?;
    if let Some(barrier) = options.pause_before_backup_rename.as_deref() {
        wait_for_test_barrier(barrier)?;
    }
    rustix::fs::renameat_with(
        &source_parent,
        file_name,
        &parent,
        file_name,
        RenameFlags::NOREPLACE,
    )
    .map_err(|error| error.to_string())?;
    Ok(InboundBackupSlot {
        parent,
        file_name: file_name.to_string(),
    })
}

fn open_real_directory(path: &Path) -> Result<OwnedFd> {
    rustix::fs::open(
        path,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|error| format!("Unable to open real directory {}: {error}", path.display()))
}

fn open_real_child_directory(parent: impl AsFd, child: &str) -> Result<OwnedFd> {
    rustix::fs::openat(
        parent,
        child,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|error| format!("Inbound directory '{child}' is not a real directory: {error}"))
}

fn open_or_create_real_child_directory(parent: impl AsFd, child: &str) -> Result<OwnedFd> {
    match open_real_child_directory(parent.as_fd(), child) {
        Ok(directory) => Ok(directory),
        Err(_) => {
            match rustix::fs::mkdirat(parent.as_fd(), child, Mode::from_bits_truncate(0o700)) {
                Ok(()) => {}
                Err(error) if error == rustix::io::Errno::EXIST => {}
                Err(error) => {
                    return Err(format!(
                        "Unable to create inbound recovery directory '{child}': {error}"
                    ))
                }
            }
            open_real_child_directory(parent, child)
        }
    }
}

fn open_relative_parent(repo_dir: impl AsFd, relative_path: &str, create: bool) -> Result<OwnedFd> {
    let mut current = rustix::io::dup(repo_dir).map_err(|error| error.to_string())?;
    let parent = Path::new(relative_path)
        .parent()
        .ok_or_else(|| "Inbound path has no parent.".to_string())?;
    for component in parent.components() {
        let std::path::Component::Normal(component) = component else {
            return Err("Inbound path parent is unsafe.".to_string());
        };
        let component = component
            .to_str()
            .ok_or_else(|| "Inbound path parent is not valid UTF-8.".to_string())?;
        current = if create {
            open_or_create_real_child_directory(&current, component)?
        } else {
            open_real_child_directory(&current, component)?
        };
    }
    Ok(current)
}

fn read_backup(slot: &InboundBackupSlot) -> Result<(Vec<u8>, bool)> {
    let fd = rustix::fs::openat(
        &slot.parent,
        slot.file_name.as_str(),
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|error| error.to_string())?;
    let mut file = File::from(fd);
    let metadata = file.metadata().map_err(|error| error.to_string())?;
    let mut bytes = Vec::new();
    use std::io::Read;
    file.read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    Ok((bytes, metadata.permissions().mode() & 0o111 != 0))
}

fn verify_inbound_backups(
    git: &skillbox_git::GitService,
    repo: &Path,
    receipt: &InboundMutationReceipt,
) -> Result<()> {
    if receipt.path_backups.is_empty() {
        return Ok(());
    }
    let old_sha = receipt
        .old_sha
        .as_deref()
        .ok_or_else(|| "Missing old SHA for inbound recovery backups.".to_string())?;
    for (path, backup) in &receipt.path_backups {
        let expected = git
            .show_file(repo, old_sha, path)?
            .ok_or_else(|| format!("Unable to read original tracked blob for recovery: {path}"))?;
        let (actual, actual_executable) = read_backup(backup)?;
        let expected_executable = receipt
            .old_entries
            .get(path)
            .is_some_and(|entry| entry.mode == "100755");
        if actual != expected || actual_executable != expected_executable {
            return Err(format!(
                "Tracked path changed during inbound apply; the local version was restored: {path}"
            ));
        }
    }
    Ok(())
}

fn ensure_inbound_parent_chain(
    repo: &Path,
    relative_path: &str,
    create: bool,
    mut created_dirs: Option<&mut HashSet<String>>,
) -> Result<PathBuf> {
    let parent = Path::new(relative_path)
        .parent()
        .unwrap_or_else(|| Path::new(""));
    let mut current = repo.to_path_buf();
    for component in parent.components() {
        let Component::Normal(component) = component else {
            return Err("Incoming path has an unsafe parent.".to_string());
        };
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {}
            Ok(_) => {
                return Err(format!(
                    "Incoming path parent changed type: {}",
                    current.display()
                ))
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound && create => {
                fs::create_dir(&current).map_err(|error| error.to_string())?;
                if let Some(created_dirs) = created_dirs.as_deref_mut() {
                    let relative = current
                        .strip_prefix(repo)
                        .map_err(|error| error.to_string())?
                        .to_string_lossy()
                        .to_string();
                    created_dirs.insert(relative);
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
            Err(error) => return Err(error.to_string()),
        }
    }
    Ok(current)
}

fn compensate_inbound_tree(
    git: &skillbox_git::GitService,
    repo: &Path,
    receipt: &mut InboundMutationReceipt,
    operation_id: &str,
) -> Result<()> {
    let current_head = git.rev_parse_optional(repo, "HEAD")?;
    let expected_head = if receipt.ref_advanced {
        Some(receipt.new_sha.as_str())
    } else {
        receipt.old_sha.as_deref()
    };
    if current_head.as_deref() != expected_head {
        return Err(
            "Git HEAD changed after inbound mutation; refusing to discard unrelated work."
                .to_string(),
        );
    }
    verify_operation_owned_paths(git, repo, receipt)?;
    if receipt.ref_advanced {
        match receipt.old_sha.as_deref() {
            Some(old) => {
                git.update_main_ref_cas(repo, old, Some(&receipt.new_sha))?;
            }
            None => git.delete_main_ref_cas(repo, &receipt.new_sha)?,
        }
        receipt.ref_advanced = false;
    }
    let mut errors = Vec::new();
    if let Err(error) = restore_git_index(repo, receipt) {
        errors.push(format!("index: {error}"));
    }
    if let Err(error) = restore_touched_paths(git, repo, receipt, operation_id) {
        errors.push(format!("worktree: {error}"));
    }
    if receipt.restore_generated_gitignore {
        if let Err(error) = restore_generated_gitignore_no_replace(repo) {
            errors.push(format!("generated .gitignore: {error}"));
        }
    }
    if let Some(mut index_lock) = receipt.index_lock.take() {
        if let Err(error) = index_lock.release_if_owned() {
            errors.push(format!("index lock: {error}"));
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

fn restore_generated_gitignore_no_replace(repo: &Path) -> Result<()> {
    let path = repo.join(".gitignore");
    match fs::symlink_metadata(&path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(
                    "another process created a non-regular .gitignore; it was preserved."
                        .to_string(),
                );
            }
            let current = fs::read_to_string(&path).map_err(|error| error.to_string())?;
            if current == DEFAULT_USER_SKILLS_GITIGNORE {
                Ok(())
            } else {
                Err("another process changed .gitignore; its content was preserved.".to_string())
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let mut file = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&path)
                .map_err(|error| error.to_string())?;
            use std::io::Write;
            file.write_all(DEFAULT_USER_SKILLS_GITIGNORE.as_bytes())
                .map_err(|error| error.to_string())
        }
        Err(error) => Err(error.to_string()),
    }
}

fn verify_operation_owned_paths(
    git: &skillbox_git::GitService,
    repo: &Path,
    receipt: &InboundMutationReceipt,
) -> Result<()> {
    for path in &receipt.written_paths {
        let Some(entry) = receipt.new_entries.get(path) else {
            continue;
        };
        let expected = git
            .show_file(repo, &receipt.new_sha, path)?
            .ok_or_else(|| format!("Unable to read applied blob for {path}"))?;
        let actual = fs::read(repo.join(path))
            .map_err(|_| format!("Applied path changed before recovery: {path}"))?;
        if actual != expected {
            return Err(format!(
                "Applied path changed after inbound mutation: {path}"
            ));
        }
        let metadata = fs::symlink_metadata(repo.join(path)).map_err(|error| error.to_string())?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(format!("Applied path changed type before recovery: {path}"));
        }
        if (entry.mode == "100755") != (metadata.permissions().mode() & 0o111 != 0) {
            return Err(format!("Applied path mode changed before recovery: {path}"));
        }
    }
    for path in &receipt.deleted_paths {
        if fs::symlink_metadata(repo.join(path)).is_ok() {
            return Err(format!(
                "Deleted path was recreated before recovery: {path}"
            ));
        }
    }
    Ok(())
}

fn restore_touched_paths(
    git: &skillbox_git::GitService,
    repo: &Path,
    receipt: &InboundMutationReceipt,
    operation_id: &str,
) -> Result<()> {
    let mut errors = Vec::new();
    let mut remove = receipt
        .written_paths
        .iter()
        .filter(|path| !receipt.old_entries.contains_key(path.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    remove.sort_by_key(|path| std::cmp::Reverse(path.matches('/').count()));
    for path in remove {
        if let Err(error) =
            preserve_then_remove_applied_path(git, repo, receipt, &path, operation_id)
        {
            errors.push(format!("{path}: {error}"));
        }
    }
    let mut restore = receipt.path_backups.iter().collect::<Vec<_>>();
    restore.sort_by(|left, right| left.0.cmp(right.0));
    for (path, backup) in restore {
        if let Err(error) =
            restore_one_inbound_backup(git, repo, receipt, path, backup, operation_id)
        {
            errors.push(format!("{path}: {error}"));
        }
    }
    let mut created_dirs = receipt.created_dirs.iter().cloned().collect::<Vec<_>>();
    created_dirs.sort_by_key(|path| std::cmp::Reverse(path.matches('/').count()));
    for path in created_dirs {
        match fs::remove_dir(repo.join(&path)) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::DirectoryNotEmpty => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => errors.push(format!("{path}: {error}")),
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

fn restore_one_inbound_backup(
    git: &skillbox_git::GitService,
    repo: &Path,
    receipt: &InboundMutationReceipt,
    path: &str,
    backup: &InboundBackupSlot,
    operation_id: &str,
) -> Result<()> {
    let target_parent = open_relative_parent(&receipt.repo_dir, path, true)?;
    let file_name = Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "Inbound recovery target has no file name.".to_string())?;
    if rustix::fs::statat(&target_parent, file_name, AtFlags::SYMLINK_NOFOLLOW).is_ok() {
        let recovery_name = format!(
            ".{file_name}.skillbox-inbound-{}.recovery",
            sanitize_operation_ref_component(operation_id)
        );
        rustix::fs::renameat_with(
            &target_parent,
            file_name,
            &backup.parent,
            recovery_name.as_str(),
            RenameFlags::NOREPLACE,
        )
        .map_err(|error| format!("Unable to preserve applied path during recovery: {error}"))?;
        let incoming = InboundBackupSlot {
            parent: rustix::io::dup(&backup.parent).map_err(|error| error.to_string())?,
            file_name: recovery_name.clone(),
        };
        let expected = receipt
            .new_entries
            .get(path)
            .and_then(|_| git.show_file(repo, &receipt.new_sha, path).ok().flatten());
        let actual = read_backup(&incoming).ok().map(|(bytes, _)| bytes);
        if expected.as_deref() != actual.as_deref() {
            return Err(
                "Applied path changed during recovery; both versions were preserved.".to_string(),
            );
        }
        rustix::fs::unlinkat(&backup.parent, recovery_name.as_str(), AtFlags::empty())
            .map_err(|error| error.to_string())?;
    }
    rustix::fs::renameat_with(
        &backup.parent,
        backup.file_name.as_str(),
        &target_parent,
        file_name,
        RenameFlags::NOREPLACE,
    )
    .map_err(|error| format!("Unable to restore tracked path: {error}"))
}

fn preserve_then_remove_applied_path(
    git: &skillbox_git::GitService,
    repo: &Path,
    receipt: &InboundMutationReceipt,
    path: &str,
    operation_id: &str,
) -> Result<()> {
    let parent = open_relative_parent(&receipt.repo_dir, path, false)?;
    let file_name = Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "Applied inbound path has no file name.".to_string())?;
    if rustix::fs::statat(&parent, file_name, AtFlags::SYMLINK_NOFOLLOW).is_err() {
        return Ok(());
    }
    let preserved = format!(
        ".{file_name}.skillbox-inbound-{}.recovery",
        sanitize_operation_ref_component(operation_id)
    );
    rustix::fs::renameat_with(
        &parent,
        file_name,
        &parent,
        preserved.as_str(),
        RenameFlags::NOREPLACE,
    )
    .map_err(|error| format!("Unable to preserve applied path during recovery: {error}"))?;
    let expected = git
        .show_file(repo, &receipt.new_sha, path)?
        .ok_or_else(|| format!("Unable to read applied blob for {path}"))?;
    let preserved_slot = InboundBackupSlot {
        parent: rustix::io::dup(&parent).map_err(|error| error.to_string())?,
        file_name: preserved.clone(),
    };
    let actual = read_backup(&preserved_slot)?.0;
    if actual != expected {
        return Err(format!(
            "Applied path changed during recovery; preserved it for manual review: {path}"
        ));
    }
    rustix::fs::unlinkat(&parent, preserved.as_str(), AtFlags::empty())
        .map_err(|error| error.to_string())
}

fn remove_empty_inbound_parents(path: &Path, repo: &Path) -> Result<()> {
    let mut current = path.parent();
    while let Some(directory) = current {
        if directory == repo {
            break;
        }
        match fs::remove_dir(directory) {
            Ok(()) => current = directory.parent(),
            Err(error) if error.kind() == std::io::ErrorKind::DirectoryNotEmpty => break,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                current = directory.parent()
            }
            Err(error) => return Err(error.to_string()),
        }
    }
    Ok(())
}

fn wait_for_test_barrier(path: &Path) -> Result<()> {
    fs::write(path.with_extension("ready"), b"ready").map_err(|error| error.to_string())?;
    let started = std::time::Instant::now();
    while !path.exists() {
        if started.elapsed() > Duration::from_secs(5) {
            return Err("Timed out waiting for inbound test barrier.".to_string());
        }
        thread::sleep(Duration::from_millis(10));
    }
    Ok(())
}

fn rollback_inbound_after_failure(
    git: &skillbox_git::GitService,
    repo: &Path,
    receipt: &mut InboundMutationReceipt,
    cause: &str,
    audit: &mut InboundApplyAudit,
) -> Result<UserSkillsInboundApplyResult> {
    audit.compensation_attempted = true;
    match compensate_inbound_tree(git, repo, receipt, "reindex-recovery") {
        Ok(()) => {
            audit.compensation_succeeded = Some(true);
            Err(format!("{cause} The previous Git state was restored."))
        }
        Err(rollback_error) => {
            audit.compensation_succeeded = Some(false);
            audit.compensation_error = Some(rollback_error.clone());
            Err(format!(
                "{cause} Automatic recovery failed: {rollback_error}. Use the recorded backup ref and normal Git tooling before retrying."
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn inbound_tree_paths_reject_cross_platform_unsafe_syntax() {
        assert!(inbound_tree_path_is_safe("demo/SKILL.md"));
        assert!(!inbound_tree_path_is_safe("../demo/SKILL.md"));
        assert!(!inbound_tree_path_is_safe("demo\\.git\\config"));
        assert!(!inbound_tree_path_is_safe("demo:stream/SKILL.md"));
        assert!(!inbound_tree_path_is_safe("demo/\nSKILL.md"));
    }

    #[test]
    fn remote_display_identity_removes_credentials_query_and_fragment() {
        assert_eq!(
            sanitize_git_remote_url(
                "https://user:password@example.com/acme/skills.git?access_token=secret#private"
            ),
            "https://example.com/acme/skills.git"
        );
        assert_eq!(
            sanitize_git_remote_url("git@example.com:acme/skills.git"),
            "example.com:acme/skills.git"
        );
        assert_eq!(
            sanitize_git_remote_url("git@example.com:acme/skills.git?access_token=secret#private"),
            "example.com:acme/skills.git"
        );
        assert_eq!(
            sanitize_git_error(
                "fetch failed for https://user@example.com/acme/skills.git?token=secret#fragment",
                "https://user@example.com/acme/skills.git?token=secret#fragment",
            ),
            "fetch failed for https://example.com/acme/skills.git"
        );
    }

    #[test]
    fn remote_only_preview_is_read_only_and_apply_bootstraps_reviewed_tree() {
        let managed_root = temp_dir("inbound-remote-only-managed");
        let (remote, work) = remote_with_skill("inbound-remote-only");
        let git = skillbox_git::GitService::new();
        fs::write(work.join("demo/README.md"), "Reviewed remote file\n").unwrap();
        git.add_all(&work).unwrap();
        git.commit(&work, "Add reviewed remote file").unwrap();
        git.push_origin_main(&work, false).unwrap();
        set_user_skills_git_remote(
            UserSkillsGitRemoteRequest {
                remote_url: remote.to_string_lossy().to_string(),
            },
            &managed_root,
        )
        .unwrap();
        let repo = managed_paths(&managed_root).user_skills_root;

        let status = check_user_skills_inbound(&managed_root).unwrap();
        assert_eq!(status.relation, UserSkillsInboundRelation::RemoteOnly);
        let preview = preview_user_skills_inbound(&managed_root).unwrap();
        let repeated_preview = preview_user_skills_inbound(&managed_root).unwrap();
        assert_eq!(
            preview.preview_id, repeated_preview.preview_id,
            "an unchanged multi-file remote-only preview must be deterministic"
        );
        assert!(preview.can_apply);
        assert!(preview
            .files
            .iter()
            .any(|file| file.path == "demo/SKILL.md"));
        assert_eq!(git.rev_parse_optional(&repo, "HEAD").unwrap(), None);
        assert!(!repo.join("demo").exists());

        let result = apply_user_skills_inbound(
            UserSkillsInboundApplyRequest {
                preview_id: Some(preview.preview_id),
                actor: "test".to_string(),
            },
            &managed_root,
        )
        .unwrap();
        assert_eq!(result.old_sha, None);
        assert_eq!(
            git.rev_parse_optional(&repo, "HEAD").unwrap(),
            Some(result.new_sha)
        );
        assert!(repo.join("demo/SKILL.md").is_file());
        assert!(repo.join("demo/README.md").is_file());
        assert!(managed_state(&managed_root)
            .unwrap()
            .skills
            .iter()
            .any(|skill| skill.name == "demo" && skill.kind == SkillKind::User));
    }

    #[test]
    fn missing_origin_and_unborn_remote_return_actionable_non_apply_states() {
        let no_origin_root = temp_dir("inbound-no-origin");
        let paths = ensure_managed_layout(&no_origin_root).unwrap();
        skillbox_git::GitService::new()
            .init_main(&paths.user_skills_root)
            .unwrap();
        let no_origin = check_user_skills_inbound(&no_origin_root).unwrap();
        assert_eq!(no_origin.relation, UserSkillsInboundRelation::Unknown);
        assert!(no_origin.message.contains("Configure"));

        let empty_remote = temp_dir("inbound-empty-remote").join("remote.git");
        Command::new("git")
            .args(["init", "--bare"])
            .arg(&empty_remote)
            .output()
            .unwrap();
        set_user_skills_git_remote(
            UserSkillsGitRemoteRequest {
                remote_url: empty_remote.to_string_lossy().to_string(),
            },
            &no_origin_root,
        )
        .unwrap();
        let empty = check_user_skills_inbound(&no_origin_root).unwrap();
        assert_eq!(empty.relation, UserSkillsInboundRelation::NoRemoteBranch);
        assert!(
            !preview_user_skills_inbound(&no_origin_root)
                .unwrap()
                .can_apply
        );
    }

    #[test]
    fn remote_advance_rejects_stale_preview_without_installing_state() {
        let managed_root = temp_dir("inbound-stale-managed");
        let (remote, work) = remote_with_skill("inbound-stale");
        set_user_skills_git_remote(
            UserSkillsGitRemoteRequest {
                remote_url: remote.to_string_lossy().to_string(),
            },
            &managed_root,
        )
        .unwrap();
        let preview = preview_user_skills_inbound(&managed_root).unwrap();
        fs::write(work.join("demo/README.md"), "advanced\n").unwrap();
        let git = skillbox_git::GitService::new();
        git.add_all(&work).unwrap();
        git.commit(&work, "Advance remote").unwrap();
        git.push_origin_main(&work, false).unwrap();

        let error = apply_user_skills_inbound(
            UserSkillsInboundApplyRequest {
                preview_id: Some(preview.preview_id),
                actor: "test".to_string(),
            },
            &managed_root,
        )
        .unwrap_err();
        assert!(error.contains("stale"));
        let paths = managed_paths(&managed_root);
        assert_eq!(
            git.rev_parse_optional(&paths.user_skills_root, "HEAD")
                .unwrap(),
            None
        );
        assert!(!paths.user_skills_root.join("demo").exists());
        assert!(managed_state(&managed_root).unwrap().skills.is_empty());
    }

    #[test]
    fn behind_apply_creates_backup_ref_and_reindexes_changes() {
        let managed_root = temp_dir("inbound-behind-managed");
        let (remote, work) = remote_with_skill("inbound-behind");
        set_user_skills_git_remote(
            UserSkillsGitRemoteRequest {
                remote_url: remote.to_string_lossy().to_string(),
            },
            &managed_root,
        )
        .unwrap();
        let first = preview_user_skills_inbound(&managed_root).unwrap();
        apply_user_skills_inbound(
            UserSkillsInboundApplyRequest {
                preview_id: Some(first.preview_id),
                actor: "test".to_string(),
            },
            &managed_root,
        )
        .unwrap();
        let paths = managed_paths(&managed_root);
        let git = skillbox_git::GitService::new();
        assert!(
            !paths.user_skills_root.join(".gitignore").exists(),
            "inbound apply must not leave a generated untracked .gitignore"
        );
        let old_head = git
            .rev_parse_optional(&paths.user_skills_root, "HEAD")
            .unwrap()
            .unwrap();

        fs::create_dir_all(work.join("second")).unwrap();
        fs::write(
            work.join("second/SKILL.md"),
            "---\nname: second\ndescription: Second\n---\n",
        )
        .unwrap();
        git.add_all(&work).unwrap();
        git.commit(&work, "Add second skill").unwrap();
        git.push_origin_main(&work, false).unwrap();

        let checked = check_user_skills_inbound(&managed_root).unwrap();
        assert_eq!(checked.relation, UserSkillsInboundRelation::Behind);
        let preview = preview_user_skills_inbound(&managed_root).unwrap();
        let repeated = preview_user_skills_inbound(&managed_root).unwrap();
        assert_eq!(preview.preview_id, repeated.preview_id);
        let applied = apply_user_skills_inbound(
            UserSkillsInboundApplyRequest {
                preview_id: Some(preview.preview_id),
                actor: "test".to_string(),
            },
            &managed_root,
        )
        .unwrap();
        let backup_ref = applied.backup_ref.unwrap();
        assert_eq!(
            git.rev_parse_optional(&paths.user_skills_root, &backup_ref)
                .unwrap(),
            Some(old_head)
        );
        assert!(managed_state(&managed_root)
            .unwrap()
            .skills
            .iter()
            .any(|skill| skill.name == "second"));
    }

    #[test]
    fn dirty_behind_can_review_but_cannot_apply() {
        let managed_root = temp_dir("inbound-dirty-managed");
        let (remote, work) = remote_with_skill("inbound-dirty");
        configure_and_bootstrap(&managed_root, &remote);
        let paths = managed_paths(&managed_root);
        fs::write(work.join("demo/README.md"), "incoming\n").unwrap();
        let git = skillbox_git::GitService::new();
        git.add_all(&work).unwrap();
        git.commit(&work, "Incoming change").unwrap();
        git.push_origin_main(&work, false).unwrap();
        fs::write(paths.user_skills_root.join("local-note.txt"), "dirty\n").unwrap();

        let checked = check_user_skills_inbound(&managed_root).unwrap();
        assert_eq!(checked.relation, UserSkillsInboundRelation::Behind);
        assert_eq!(
            checked.worktree_state,
            UserSkillsInboundWorktreeState::Dirty
        );
        let preview = preview_user_skills_inbound(&managed_root).unwrap();
        assert!(!preview.can_apply);
        assert!(preview
            .blocked_reason
            .as_deref()
            .unwrap()
            .contains("Local changes"));
    }

    #[test]
    fn local_head_change_rejects_stale_preview_without_fast_forward() {
        let managed_root = temp_dir("inbound-local-head-stale-managed");
        let (remote, work) = remote_with_skill("inbound-local-head-stale");
        configure_and_bootstrap(&managed_root, &remote);
        let paths = managed_paths(&managed_root);
        let git = skillbox_git::GitService::new();
        fs::write(work.join("demo/README.md"), "incoming\n").unwrap();
        git.add_all(&work).unwrap();
        git.commit(&work, "Incoming change").unwrap();
        git.push_origin_main(&work, false).unwrap();
        let preview = preview_user_skills_inbound(&managed_root).unwrap();
        fs::write(paths.user_skills_root.join("local.txt"), "local\n").unwrap();
        git.add_all(&paths.user_skills_root).unwrap();
        let local_head = git
            .commit(&paths.user_skills_root, "Local change after preview")
            .unwrap();

        let error = apply_user_skills_inbound(
            UserSkillsInboundApplyRequest {
                preview_id: Some(preview.preview_id),
                actor: "test".to_string(),
            },
            &managed_root,
        )
        .unwrap_err();
        assert!(error.contains("stale"));
        assert_eq!(
            git.rev_parse_optional(&paths.user_skills_root, "HEAD")
                .unwrap(),
            Some(local_head)
        );
        assert!(!paths.user_skills_root.join("demo/README.md").exists());
    }

    #[test]
    fn ignored_and_untracked_incoming_collisions_block_before_mutation() {
        for mode in ["exact", "ignored-directory", "type"] {
            let managed_root = temp_dir(&format!("inbound-collision-{mode}"));
            let (remote, work) = remote_with_skill(&format!("inbound-collision-{mode}"));
            configure_and_bootstrap(&managed_root, &remote);
            let paths = managed_paths(&managed_root);
            let local_path = match mode {
                "exact" => ".venv/secret.txt",
                "ignored-directory" => ".venv/local-secret.txt",
                "type" => "cache",
                _ => unreachable!(),
            };
            if let Some(parent) = Path::new(local_path).parent() {
                fs::create_dir_all(paths.user_skills_root.join(parent)).unwrap();
            }
            if mode == "type" {
                fs::write(
                    paths.user_skills_root.join(".git/info/exclude"),
                    format!("{DEFAULT_USER_SKILLS_GITIGNORE}\ncache\n"),
                )
                .unwrap();
            }
            fs::write(paths.user_skills_root.join(local_path), b"local secret").unwrap();

            let remote_path = match mode {
                "exact" => ".venv/secret.txt",
                "ignored-directory" => ".venv/remote.txt",
                "type" => "cache/data.txt",
                _ => unreachable!(),
            };
            fs::create_dir_all(work.join(remote_path).parent().unwrap()).unwrap();
            fs::write(work.join(remote_path), b"remote content").unwrap();
            let add = Command::new("git")
                .arg("-C")
                .arg(&work)
                .args(["add", "-f", "--", remote_path])
                .output()
                .unwrap();
            assert!(add.status.success());
            let git = skillbox_git::GitService::new();
            git.commit(&work, "Incoming collision").unwrap();
            git.push_origin_main(&work, false).unwrap();

            let preview = preview_user_skills_inbound(&managed_root).unwrap();
            assert!(!preview.can_apply, "{mode}");
            assert!(preview
                .safety_issues
                .iter()
                .any(|issue| issue.code == "local_content_collision"));
            assert_eq!(
                fs::read(paths.user_skills_root.join(local_path)).unwrap(),
                b"local secret"
            );
        }
    }

    #[test]
    fn ignored_content_created_after_preview_invalidates_apply_without_overwrite() {
        let managed_root = temp_dir("inbound-collision-after-preview");
        let (remote, work) = remote_with_skill("inbound-collision-after-preview");
        configure_and_bootstrap(&managed_root, &remote);
        let paths = managed_paths(&managed_root);
        fs::create_dir_all(work.join(".venv")).unwrap();
        fs::write(work.join(".venv/secret.txt"), b"remote content").unwrap();
        let add = Command::new("git")
            .arg("-C")
            .arg(&work)
            .args(["add", "-f", "--", ".venv/secret.txt"])
            .output()
            .unwrap();
        assert!(add.status.success());
        let git = skillbox_git::GitService::new();
        git.commit(&work, "Add incoming ignored path").unwrap();
        git.push_origin_main(&work, false).unwrap();
        let preview = preview_user_skills_inbound(&managed_root).unwrap();
        assert!(preview.can_apply);

        fs::create_dir_all(paths.user_skills_root.join(".venv")).unwrap();
        fs::write(
            paths.user_skills_root.join(".venv/secret.txt"),
            b"local secret",
        )
        .unwrap();
        let error = apply_user_skills_inbound(
            UserSkillsInboundApplyRequest {
                preview_id: Some(preview.preview_id),
                actor: "test".to_string(),
            },
            &managed_root,
        )
        .unwrap_err();

        assert!(error.contains("stale"));
        assert_eq!(
            fs::read(paths.user_skills_root.join(".venv/secret.txt")).unwrap(),
            b"local secret"
        );
        assert!(!paths.user_skills_root.join("demo/README.md").exists());
    }

    #[test]
    fn final_materialization_never_replaces_a_late_local_file_or_leaves_temp_files() {
        let repo = temp_dir("inbound-final-no-clobber");
        fs::create_dir_all(repo.join(".venv")).unwrap();
        fs::write(repo.join(".venv/secret.txt"), b"local secret").unwrap();
        let mut created_dirs = HashSet::new();

        let error = write_inbound_file(
            &repo,
            ".venv/secret.txt",
            b"remote content",
            false,
            "no-clobber",
            &mut created_dirs,
        )
        .unwrap_err();

        assert!(error.contains("collided with local content"));
        assert_eq!(
            fs::read(repo.join(".venv/secret.txt")).unwrap(),
            b"local secret"
        );
        assert!(fs::read_dir(repo.join(".venv")).unwrap().all(|entry| !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .contains(".skillbox-inbound-")));
        assert!(created_dirs.is_empty());
    }

    #[test]
    fn partial_behind_materialization_restores_head_index_and_worktree() {
        let managed_root = temp_dir("inbound-partial-behind");
        let (remote, work) = remote_with_skill("inbound-partial-behind");
        configure_and_bootstrap(&managed_root, &remote);
        let paths = managed_paths(&managed_root);
        let git = skillbox_git::GitService::new();
        fs::write(work.join("demo/README.md"), "incoming\n").unwrap();
        fs::write(work.join("demo/notes.md"), "second\n").unwrap();
        git.add_all(&work).unwrap();
        git.commit(&work, "Incoming files").unwrap();
        git.push_origin_main(&work, false).unwrap();
        let preview = preview_user_skills_inbound(&managed_root).unwrap();
        let old_sha = preview.status.local_sha.clone().unwrap();
        let new_sha = preview.status.remote_sha.clone().unwrap();
        let old_snapshot =
            validate_inbound_git_tree(&git, &paths.user_skills_root, &old_sha, &mut Vec::new())
                .unwrap();
        let new_snapshot =
            validate_inbound_git_tree(&git, &paths.user_skills_root, &new_sha, &mut Vec::new())
                .unwrap();
        let mut audit = InboundApplyAudit::default();
        let error = apply_inbound_tree(
            &git,
            &paths.user_skills_root,
            Some(&old_sha),
            &new_sha,
            old_snapshot.entries,
            new_snapshot.entries,
            "partial-behind",
            &InboundMutationOptions {
                fail_after_writes: Some(1),
                ..Default::default()
            },
            &mut audit,
        )
        .unwrap_err();
        assert!(error.contains("restored"));
        assert_eq!(
            git.rev_parse_optional(&paths.user_skills_root, "HEAD")
                .unwrap()
                .as_deref(),
            Some(old_sha.as_str())
        );
        assert!(!git.status_hardened(&paths.user_skills_root).unwrap().dirty);
        assert!(!paths.user_skills_root.join("demo/README.md").exists());
    }

    #[test]
    fn fd_relative_backup_and_restore_ignore_a_swapped_backup_path() {
        let managed_root = temp_dir("inbound-fd-backup-swap");
        let (remote, work) = remote_with_skill("inbound-fd-backup-swap");
        configure_and_bootstrap(&managed_root, &remote);
        let paths = managed_paths(&managed_root);
        let git = skillbox_git::GitService::new();
        fs::write(work.join("demo/README.md"), "old readme\n").unwrap();
        git.add_all(&work).unwrap();
        git.commit(&work, "Add readme").unwrap();
        git.push_origin_main(&work, false).unwrap();
        let bootstrap = preview_user_skills_inbound(&managed_root).unwrap();
        apply_user_skills_inbound(
            UserSkillsInboundApplyRequest {
                preview_id: Some(bootstrap.preview_id),
                actor: "test".to_string(),
            },
            &managed_root,
        )
        .unwrap();
        let old_readme = fs::read(paths.user_skills_root.join("demo/README.md")).unwrap();
        let old_skill = fs::read(paths.user_skills_root.join("demo/SKILL.md")).unwrap();
        fs::write(work.join("demo/README.md"), "incoming readme\n").unwrap();
        fs::write(
            work.join("demo/SKILL.md"),
            "---\nname: demo\ndescription: Incoming\n---\n",
        )
        .unwrap();
        git.add_all(&work).unwrap();
        git.commit(&work, "Incoming files").unwrap();
        git.push_origin_main(&work, false).unwrap();
        let preview = preview_user_skills_inbound(&managed_root).unwrap();
        let old_sha = preview.status.local_sha.clone().unwrap();
        let new_sha = preview.status.remote_sha.clone().unwrap();
        let old_snapshot =
            validate_inbound_git_tree(&git, &paths.user_skills_root, &old_sha, &mut Vec::new())
                .unwrap();
        let new_snapshot =
            validate_inbound_git_tree(&git, &paths.user_skills_root, &new_sha, &mut Vec::new())
                .unwrap();
        let barrier = temp_dir("inbound-fd-swap-barrier").join("continue");
        let ready = barrier.with_extension("ready");
        let repo = paths.user_skills_root.clone();
        let worker_barrier = barrier.clone();
        let old_sha_worker = old_sha.clone();
        let worker = std::thread::spawn(move || {
            let mut audit = InboundApplyAudit::default();
            apply_inbound_tree(
                &skillbox_git::GitService::new(),
                &repo,
                Some(&old_sha_worker),
                &new_sha,
                old_snapshot.entries,
                new_snapshot.entries,
                "fd-swap",
                &InboundMutationOptions {
                    fail_after_writes: Some(1),
                    pause_before_backup_rename: Some(worker_barrier),
                    ..Default::default()
                },
                &mut audit,
            )
        });
        while !ready.exists() {
            std::thread::sleep(Duration::from_millis(10));
        }
        let backup_root = paths
            .user_skills_root
            .join(".git/skillbox/inbound-worktree-backups/fd-swap");
        let detached = backup_root.with_file_name("fd-swap-detached");
        let outside = temp_dir("inbound-fd-swap-outside");
        fs::rename(&backup_root, &detached).unwrap();
        std::os::unix::fs::symlink(&outside, &backup_root).unwrap();
        fs::write(&barrier, b"continue").unwrap();
        let error = worker.join().unwrap().unwrap_err();
        assert!(error.contains("restored"));
        assert_eq!(
            git.rev_parse_optional(&paths.user_skills_root, "HEAD")
                .unwrap()
                .as_deref(),
            Some(old_sha.as_str())
        );
        assert_eq!(
            fs::read(paths.user_skills_root.join("demo/README.md")).unwrap(),
            old_readme
        );
        assert_eq!(
            fs::read(paths.user_skills_root.join("demo/SKILL.md")).unwrap(),
            old_skill
        );
        assert_eq!(fs::read_dir(outside).unwrap().count(), 0);
    }

    #[test]
    fn remote_only_recovery_preserves_concurrent_gitignore_content() {
        let managed_root = temp_dir("inbound-gitignore-concurrent");
        let (remote, _) = remote_with_skill("inbound-gitignore-concurrent");
        set_user_skills_git_remote(
            UserSkillsGitRemoteRequest {
                remote_url: remote.to_string_lossy().to_string(),
            },
            &managed_root,
        )
        .unwrap();
        let paths = managed_paths(&managed_root);
        let preview = preview_user_skills_inbound(&managed_root).unwrap();
        let git = skillbox_git::GitService::new();
        let new_sha = preview.status.remote_sha.clone().unwrap();
        let new_snapshot =
            validate_inbound_git_tree(&git, &paths.user_skills_root, &new_sha, &mut Vec::new())
                .unwrap();
        let barrier = temp_dir("inbound-gitignore-concurrent-barrier").join("continue");
        let ready = barrier.with_extension("ready");
        let repo = paths.user_skills_root.clone();
        let worker_barrier = barrier.clone();
        let worker = std::thread::spawn(move || {
            let mut audit = InboundApplyAudit::default();
            apply_inbound_tree(
                &skillbox_git::GitService::new(),
                &repo,
                None,
                &new_sha,
                HashMap::new(),
                new_snapshot.entries,
                "gitignore-concurrent",
                &InboundMutationOptions {
                    fail_after_index_replace: true,
                    pause_before_compensation: Some(worker_barrier),
                    ..Default::default()
                },
                &mut audit,
            )
        });
        while !ready.exists() {
            std::thread::sleep(Duration::from_millis(10));
        }
        let index = paths.user_skills_root.join(".git/index");
        fs::remove_file(&index).unwrap();
        fs::create_dir(&index).unwrap();
        fs::write(
            paths.user_skills_root.join(".gitignore"),
            "external concurrent content\n",
        )
        .unwrap();
        fs::write(&barrier, b"continue").unwrap();
        let error = worker.join().unwrap().unwrap_err();
        assert!(error.contains("preserved"));
        assert!(error.contains("index"));
        assert_eq!(
            fs::read_to_string(paths.user_skills_root.join(".gitignore")).unwrap(),
            "external concurrent content\n"
        );
        assert_eq!(
            git.rev_parse_optional(&paths.user_skills_root, "HEAD")
                .unwrap(),
            None
        );
        assert!(
            !paths.user_skills_root.join("demo").exists(),
            "worktree cleanup must continue after index restoration fails"
        );
    }

    #[test]
    fn partial_remote_only_materialization_clears_index_and_can_retry() {
        let managed_root = temp_dir("inbound-partial-unborn");
        let (remote, work) = remote_with_skill("inbound-partial-unborn");
        let git = skillbox_git::GitService::new();
        fs::write(work.join("demo/README.md"), "incoming\n").unwrap();
        git.add_all(&work).unwrap();
        git.commit(&work, "Add second remote file").unwrap();
        git.push_origin_main(&work, false).unwrap();
        set_user_skills_git_remote(
            UserSkillsGitRemoteRequest {
                remote_url: remote.to_string_lossy().to_string(),
            },
            &managed_root,
        )
        .unwrap();
        let paths = managed_paths(&managed_root);
        let preview = preview_user_skills_inbound(&managed_root).unwrap();
        let new_sha = preview.status.remote_sha.clone().unwrap();
        let new_snapshot =
            validate_inbound_git_tree(&git, &paths.user_skills_root, &new_sha, &mut Vec::new())
                .unwrap();
        let mut audit = InboundApplyAudit::default();
        let error = apply_inbound_tree(
            &git,
            &paths.user_skills_root,
            None,
            &new_sha,
            HashMap::new(),
            new_snapshot.entries,
            "partial-unborn",
            &InboundMutationOptions {
                fail_after_writes: Some(1),
                ..Default::default()
            },
            &mut audit,
        )
        .unwrap_err();
        assert!(error.contains("restored"));
        assert!(git
            .rev_parse_optional(&paths.user_skills_root, "HEAD")
            .unwrap()
            .is_none());
        let staged = Command::new("git")
            .arg("-C")
            .arg(&paths.user_skills_root)
            .args(["diff", "--cached", "--name-only"])
            .output()
            .unwrap();
        assert!(staged.status.success());
        assert!(staged.stdout.is_empty());
        assert_eq!(
            fs::read_to_string(paths.user_skills_root.join(".gitignore")).unwrap(),
            DEFAULT_USER_SKILLS_GITIGNORE
        );
        assert!(git
            .status_hardened(&paths.user_skills_root)
            .unwrap()
            .raw_status
            .lines()
            .filter(|line| !line.starts_with("##"))
            .all(|line| line == "?? .gitignore"));
        let retry = preview_user_skills_inbound(&managed_root).unwrap();
        apply_user_skills_inbound(
            UserSkillsInboundApplyRequest {
                preview_id: Some(retry.preview_id),
                actor: "test".to_string(),
            },
            &managed_root,
        )
        .unwrap();
    }

    #[test]
    fn shared_mutation_lock_blocks_concurrent_deployment_changes() {
        let managed_root = temp_dir("inbound-shared-lock");
        let (remote, _) = remote_with_skill("inbound-shared-lock");
        configure_and_bootstrap(&managed_root, &remote);
        let runtime = temp_dir("inbound-shared-lock-runtime");
        deploy_skill("demo", &managed_root, &runtime).unwrap();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let lock_root = managed_root.clone();
        let lock_barrier = barrier.clone();
        let holder = std::thread::spawn(move || {
            let _lock = acquire_user_skills_mutation_lock(&lock_root).unwrap();
            lock_barrier.wait();
            lock_barrier.wait();
        });
        barrier.wait();

        let deploy_error = undeploy_skill("demo", &managed_root, &runtime).unwrap_err();
        assert!(deploy_error.contains("Another user-skills mutation"));
        let remote_error = set_user_skills_git_remote(
            UserSkillsGitRemoteRequest {
                remote_url: remote.to_string_lossy().to_string(),
            },
            &managed_root,
        )
        .unwrap_err();
        assert!(remote_error.contains("Another user-skills mutation"));
        let outbound_error = sync_user_skills_git(
            UserSkillsSyncRequest {
                remote_url: None,
                commit_message: None,
                push: false,
                selected_paths: None,
            },
            &managed_root,
        )
        .unwrap_err();
        assert!(outbound_error.contains("Another user-skills mutation"));

        barrier.wait();
        holder.join().unwrap();
        assert!(runtime.join("demo").is_symlink());
    }

    #[test]
    fn malformed_frontmatter_and_symlink_tree_are_blocked_before_apply() {
        let managed_root = temp_dir("inbound-unsafe-managed");
        let (remote, work) = remote_with_skill("inbound-unsafe");
        fs::write(
            work.join("demo/SKILL.md"),
            "---\nname: [not, a, string]\n---\n",
        )
        .unwrap();
        std::os::unix::fs::symlink("/tmp/outside", work.join("demo/escape")).unwrap();
        let git = skillbox_git::GitService::new();
        git.add_all(&work).unwrap();
        git.commit(&work, "Unsafe tree").unwrap();
        git.push_origin_main(&work, false).unwrap();
        set_user_skills_git_remote(
            UserSkillsGitRemoteRequest {
                remote_url: remote.to_string_lossy().to_string(),
            },
            &managed_root,
        )
        .unwrap();

        let preview = preview_user_skills_inbound(&managed_root).unwrap();
        assert!(!preview.can_apply);
        assert!(preview
            .safety_issues
            .iter()
            .any(|issue| issue.code == "invalid_skill_frontmatter"));
        assert!(preview
            .safety_issues
            .iter()
            .any(|issue| issue.code == "unsafe_symlink"));
        assert!(!managed_paths(&managed_root)
            .user_skills_root
            .join("demo")
            .exists());
    }

    #[test]
    fn deleting_deployed_skill_is_blocked_but_undeployed_deletion_applies() {
        let managed_root = temp_dir("inbound-delete-managed");
        let (remote, work) = remote_with_skill("inbound-delete");
        configure_and_bootstrap(&managed_root, &remote);
        let runtime = temp_dir("inbound-delete-runtime");
        deploy_skill("demo", &managed_root, &runtime).unwrap();
        fs::remove_dir_all(work.join("demo")).unwrap();
        let git = skillbox_git::GitService::new();
        git.add_all(&work).unwrap();
        git.commit(&work, "Delete demo").unwrap();
        git.push_origin_main(&work, false).unwrap();

        let blocked = preview_user_skills_inbound(&managed_root).unwrap();
        assert!(!blocked.can_apply);
        let deleted = blocked
            .skill_changes
            .iter()
            .find(|change| change.skill_name == "demo")
            .unwrap();
        assert_eq!(deleted.affected_deployments.len(), 1);
        assert_eq!(
            deleted.affected_deployments[0].profile_id,
            CUSTOM_SKILL_MD_PROFILE_ID
        );
        assert!(blocked
            .safety_issues
            .iter()
            .any(|issue| issue.code == "deployed_skill_deleted"));
        undeploy_skill("demo", &managed_root, &runtime).unwrap();

        let allowed = preview_user_skills_inbound(&managed_root).unwrap();
        assert!(allowed.can_apply);
        apply_user_skills_inbound(
            UserSkillsInboundApplyRequest {
                preview_id: Some(allowed.preview_id),
                actor: "test".to_string(),
            },
            &managed_root,
        )
        .unwrap();
        assert!(!managed_paths(&managed_root)
            .user_skills_root
            .join("demo")
            .exists());
        assert!(managed_state(&managed_root).unwrap().skills.is_empty());
    }

    #[test]
    fn changed_deployment_target_rejects_reviewed_update_as_stale() {
        let managed_root = temp_dir("inbound-target-stale-managed");
        let (remote, work) = remote_with_skill("inbound-target-stale");
        configure_and_bootstrap(&managed_root, &remote);
        let runtime = temp_dir("inbound-target-stale-runtime");
        let deployment = deploy_skill("demo", &managed_root, &runtime).unwrap();
        let git = skillbox_git::GitService::new();
        fs::write(work.join("demo/README.md"), "incoming\n").unwrap();
        git.add_all(&work).unwrap();
        git.commit(&work, "Update deployed skill").unwrap();
        git.push_origin_main(&work, false).unwrap();
        let preview = preview_user_skills_inbound(&managed_root).unwrap();
        assert!(preview.can_apply);
        fs::remove_file(&deployment.target_path).unwrap();

        let error = apply_user_skills_inbound(
            UserSkillsInboundApplyRequest {
                preview_id: Some(preview.preview_id),
                actor: "test".to_string(),
            },
            &managed_root,
        )
        .unwrap_err();
        assert!(error.contains("stale"));
        assert!(!managed_paths(&managed_root)
            .user_skills_root
            .join("demo/README.md")
            .exists());
    }

    #[test]
    fn reindex_failure_restores_previous_head_and_worktree() {
        let managed_root = temp_dir("inbound-reindex-rollback-managed");
        let (remote, work) = remote_with_skill("inbound-reindex-rollback");
        configure_and_bootstrap(&managed_root, &remote);
        let paths = managed_paths(&managed_root);
        let git = skillbox_git::GitService::new();
        let old_head = git
            .rev_parse_optional(&paths.user_skills_root, "HEAD")
            .unwrap();
        fs::write(work.join("demo/README.md"), "incoming\n").unwrap();
        git.add_all(&work).unwrap();
        git.commit(&work, "Incoming update").unwrap();
        git.push_origin_main(&work, false).unwrap();
        let preview = preview_user_skills_inbound(&managed_root).unwrap();
        fs::write(
            paths.database_path.with_extension("fail-inbound-reindex"),
            "fail",
        )
        .unwrap();

        let error = apply_user_skills_inbound(
            UserSkillsInboundApplyRequest {
                preview_id: Some(preview.preview_id),
                actor: "test".to_string(),
            },
            &managed_root,
        )
        .unwrap_err();
        assert!(error.contains("previous Git state was restored"));
        assert_eq!(
            git.rev_parse_optional(&paths.user_skills_root, "HEAD")
                .unwrap(),
            old_head
        );
        assert!(!paths.user_skills_root.join("demo/README.md").exists());
        assert!(!git.status(&paths.user_skills_root).unwrap().dirty);
        let failed = list_operations(
            OperationFilter {
                status: Some(OperationStatus::Failed),
                ..Default::default()
            },
            &managed_root,
        )
        .unwrap()
        .operations
        .into_iter()
        .find(|operation| operation.operation_type == "apply_user_skills_inbound")
        .unwrap();
        assert_eq!(failed.payload["oldSha"].as_str(), old_head.as_deref());
        assert!(failed.payload["newSha"].as_str().is_some());
        assert!(failed.payload["backupRef"].as_str().is_some());
        assert_eq!(failed.payload["mutationPhase"], "reindexing");
        assert_eq!(failed.payload["compensation"]["attempted"], true);
        assert_eq!(failed.payload["compensation"]["succeeded"], true);
        assert!(failed
            .error
            .unwrap()
            .contains("previous Git state was restored"));
    }

    #[test]
    fn diverged_history_reports_read_only_conflict_analysis() {
        let managed_root = temp_dir("inbound-diverged-managed");
        let (remote, work) = remote_with_skill("inbound-diverged");
        configure_and_bootstrap(&managed_root, &remote);
        let paths = managed_paths(&managed_root);
        let git = skillbox_git::GitService::new();
        fs::write(
            paths.user_skills_root.join("demo/SKILL.md"),
            "---\nname: demo\ndescription: Local\n---\n",
        )
        .unwrap();
        git.add_all(&paths.user_skills_root).unwrap();
        git.commit(&paths.user_skills_root, "Local change").unwrap();
        fs::write(
            work.join("demo/SKILL.md"),
            "---\nname: demo\ndescription: Remote\n---\n",
        )
        .unwrap();
        git.add_all(&work).unwrap();
        git.commit(&work, "Remote change").unwrap();
        git.push_origin_main(&work, false).unwrap();
        let local_before = git
            .rev_parse_optional(&paths.user_skills_root, "HEAD")
            .unwrap();

        let preview = preview_user_skills_inbound(&managed_root).unwrap();
        assert_eq!(preview.status.relation, UserSkillsInboundRelation::Diverged);
        assert!(!preview.can_apply);
        let analysis = preview.conflict_analysis.unwrap();
        assert_eq!(analysis.local_only_commits, 1);
        assert_eq!(analysis.remote_only_commits, 1);
        assert!(analysis
            .likely_conflict_files
            .iter()
            .any(|path| path == "demo/SKILL.md"));
        assert_eq!(
            git.rev_parse_optional(&paths.user_skills_root, "HEAD")
                .unwrap(),
            local_before
        );
    }

    #[test]
    fn diverged_history_marks_a_skill_changed_on_different_files_by_both_sides() {
        let managed_root = temp_dir("inbound-diverged-skill-files");
        let (remote, work) = remote_with_skill("inbound-diverged-skill-files");
        configure_and_bootstrap(&managed_root, &remote);
        let paths = managed_paths(&managed_root);
        let git = skillbox_git::GitService::new();
        fs::write(
            paths.user_skills_root.join("demo/local-notes.md"),
            "local\n",
        )
        .unwrap();
        git.add_all(&paths.user_skills_root).unwrap();
        git.commit(&paths.user_skills_root, "Local skill notes")
            .unwrap();
        fs::write(work.join("demo/remote-notes.md"), "remote\n").unwrap();
        git.add_all(&work).unwrap();
        git.commit(&work, "Remote skill notes").unwrap();
        git.push_origin_main(&work, false).unwrap();

        let preview = preview_user_skills_inbound(&managed_root).unwrap();
        let analysis = preview.conflict_analysis.unwrap();
        assert!(analysis.both_changed_files.is_empty());
        assert_eq!(analysis.both_changed_skills, vec!["demo"]);
    }

    #[test]
    fn conflict_analysis_does_not_label_clean_delete_delete_as_likely_conflict() {
        let managed_root = temp_dir("inbound-delete-delete");
        let (remote, work) = remote_with_skill("inbound-delete-delete");
        fs::write(work.join("demo/references.md"), "shared\n").unwrap();
        let git = skillbox_git::GitService::new();
        git.add_all(&work).unwrap();
        git.commit(&work, "Add shared reference").unwrap();
        git.push_origin_main(&work, false).unwrap();
        configure_and_bootstrap(&managed_root, &remote);
        let paths = managed_paths(&managed_root);
        fs::remove_file(paths.user_skills_root.join("demo/references.md")).unwrap();
        git.add_all(&paths.user_skills_root).unwrap();
        git.commit(&paths.user_skills_root, "Delete local reference")
            .unwrap();
        fs::remove_file(work.join("demo/references.md")).unwrap();
        git.add_all(&work).unwrap();
        git.commit(&work, "Delete remote reference").unwrap();
        git.push_origin_main(&work, false).unwrap();

        let analysis = preview_user_skills_inbound(&managed_root)
            .unwrap()
            .conflict_analysis
            .unwrap();
        assert!(analysis
            .both_changed_files
            .iter()
            .any(|path| path == "demo/references.md"));
        assert!(!analysis
            .likely_conflict_files
            .iter()
            .any(|path| path == "demo/references.md"));
        assert!(analysis
            .both_changed_skills
            .iter()
            .any(|name| name == "demo"));
    }

    #[test]
    fn base_skill_lookup_deduplicates_top_level_candidates() {
        let mut valid = HashSet::new();
        let candidates = (0..1_000)
            .map(|_| "historical".to_string())
            .chain((0..1_000).map(|_| "not-a-skill".to_string()))
            .collect::<HashSet<_>>();
        let mut calls = HashMap::<String, usize>::new();
        add_base_skill_names(&mut valid, candidates, |name| {
            *calls.entry(name.to_string()).or_default() += 1;
            Ok(name == "historical")
        })
        .unwrap();
        assert_eq!(calls.get("historical"), Some(&1));
        assert_eq!(calls.get("not-a-skill"), Some(&1));
        assert!(valid.contains("historical"));
    }

    #[test]
    fn remote_only_generated_gitignore_must_not_be_a_symlink() {
        let managed_root = temp_dir("inbound-gitignore-symlink");
        let (remote, _) = remote_with_skill("inbound-gitignore-symlink");
        set_user_skills_git_remote(
            UserSkillsGitRemoteRequest {
                remote_url: remote.to_string_lossy().to_string(),
            },
            &managed_root,
        )
        .unwrap();
        let paths = managed_paths(&managed_root);
        let target = temp_dir("inbound-gitignore-target").join("defaults");
        fs::write(&target, DEFAULT_USER_SKILLS_GITIGNORE).unwrap();
        fs::remove_file(paths.user_skills_root.join(".gitignore")).unwrap();
        std::os::unix::fs::symlink(&target, paths.user_skills_root.join(".gitignore")).unwrap();

        let preview = preview_user_skills_inbound(&managed_root).unwrap();
        assert!(!preview.can_apply);
        assert_eq!(
            fs::read_to_string(&target).unwrap(),
            DEFAULT_USER_SKILLS_GITIGNORE
        );
    }

    #[test]
    fn index_lock_drop_preserves_a_replacement_lock() {
        let repo = temp_dir("inbound-index-lock-owner");
        let git = skillbox_git::GitService::new();
        git.init_main(&repo).unwrap();
        let lock = acquire_git_index_lock(&repo).unwrap();
        fs::rename(
            repo.join(".git/index.lock"),
            repo.join(".git/index.lock.original"),
        )
        .unwrap();
        fs::write(repo.join(".git/index.lock"), "external").unwrap();
        drop(lock);
        assert_eq!(
            fs::read_to_string(repo.join(".git/index.lock")).unwrap(),
            "external"
        );
    }

    #[test]
    fn inbound_flow_never_executes_hooks_filters_or_merge_drivers() {
        let managed_root = temp_dir("inbound-no-external-git-programs");
        let (remote, work) = remote_with_skill("inbound-no-external-git-programs");
        configure_and_bootstrap(&managed_root, &remote);
        let paths = managed_paths(&managed_root);
        let marker_root = temp_dir("inbound-external-markers");
        let hook_marker = marker_root.join("hook");
        let clean_filter_marker = marker_root.join("clean-filter");
        let smudge_filter_marker = marker_root.join("smudge-filter");
        let merge_marker = marker_root.join("merge");
        let hooks = paths.user_skills_root.join(".git/hooks");
        fs::create_dir_all(&hooks).unwrap();
        let hook = hooks.join("post-merge");
        fs::write(
            &hook,
            format!("#!/bin/sh\nprintf invoked > '{}'\n", hook_marker.display()),
        )
        .unwrap();
        fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
        for (key, value) in [
            (
                "filter.evil.smudge",
                format!(
                    "sh -c \"printf invoked > '{}'; cat\"",
                    smudge_filter_marker.display()
                ),
            ),
            (
                "filter.evil.clean",
                format!(
                    "sh -c \"printf invoked > '{}'; cat\"",
                    clean_filter_marker.display()
                ),
            ),
            (
                "merge.evil.driver",
                format!(
                    "sh -c \"printf invoked > '{}'; exit 1\"",
                    merge_marker.display()
                ),
            ),
        ] {
            let output = Command::new("git")
                .arg("-C")
                .arg(&paths.user_skills_root)
                .args(["config", key, &value])
                .output()
                .unwrap();
            assert!(output.status.success());
        }
        fs::write(
            work.join(".gitattributes"),
            "*.md filter=evil\n*.txt merge=evil\n",
        )
        .unwrap();
        fs::write(work.join("demo/README.md"), "incoming\n").unwrap();
        let git = skillbox_git::GitService::new();
        git.add_all(&work).unwrap();
        git.commit(&work, "Incoming attributed files").unwrap();
        git.push_origin_main(&work, false).unwrap();
        let preview = preview_user_skills_inbound(&managed_root).unwrap();
        apply_user_skills_inbound(
            UserSkillsInboundApplyRequest {
                preview_id: Some(preview.preview_id),
                actor: "test".to_string(),
            },
            &managed_root,
        )
        .unwrap();
        assert!(!hook_marker.exists());
        assert!(!clean_filter_marker.exists());
        assert!(!smudge_filter_marker.exists());

        fs::write(paths.user_skills_root.join("demo/references.md"), "local\n").unwrap();
        git.add_all(&paths.user_skills_root).unwrap();
        git.commit(&paths.user_skills_root, "Local references")
            .unwrap();
        fs::write(work.join("demo/references.md"), "remote\n").unwrap();
        git.add_all(&work).unwrap();
        git.commit(&work, "Remote references").unwrap();
        git.push_origin_main(&work, false).unwrap();
        let diverged = preview_user_skills_inbound(&managed_root).unwrap();
        assert_eq!(
            diverged.status.relation,
            UserSkillsInboundRelation::Diverged
        );
        assert!(diverged
            .conflict_analysis
            .as_ref()
            .unwrap()
            .both_changed_skills
            .iter()
            .any(|name| name == "demo"));
        assert!(!merge_marker.exists());
    }

    #[test]
    fn concurrent_git_commit_is_blocked_while_inbound_apply_holds_index_lock() {
        let managed_root = temp_dir("inbound-cas-concurrent");
        let (remote, work) = remote_with_skill("inbound-cas-concurrent");
        configure_and_bootstrap(&managed_root, &remote);
        let paths = managed_paths(&managed_root);
        let git = skillbox_git::GitService::new();
        fs::write(work.join("demo/README.md"), "incoming\n").unwrap();
        git.add_all(&work).unwrap();
        git.commit(&work, "Incoming").unwrap();
        git.push_origin_main(&work, false).unwrap();
        let preview = preview_user_skills_inbound(&managed_root).unwrap();
        let old_sha = preview.status.local_sha.clone().unwrap();
        let new_sha = preview.status.remote_sha.clone().unwrap();
        let old_snapshot =
            validate_inbound_git_tree(&git, &paths.user_skills_root, &old_sha, &mut Vec::new())
                .unwrap();
        let new_snapshot =
            validate_inbound_git_tree(&git, &paths.user_skills_root, &new_sha, &mut Vec::new())
                .unwrap();
        let barrier = temp_dir("inbound-cas-barrier").join("continue");
        let ready = barrier.with_extension("ready");
        let repo = paths.user_skills_root.clone();
        let barrier_for_thread = barrier.clone();
        let old_sha_for_worker = old_sha.clone();
        let worker = std::thread::spawn(move || {
            let mut audit = InboundApplyAudit::default();
            apply_inbound_tree(
                &skillbox_git::GitService::new(),
                &repo,
                Some(&old_sha_for_worker),
                &new_sha,
                old_snapshot.entries,
                new_snapshot.entries,
                "cas-concurrent",
                &InboundMutationOptions {
                    pause_before_ref_update: Some(barrier_for_thread),
                    ..Default::default()
                },
                &mut audit,
            )
        });
        while !ready.exists() {
            std::thread::sleep(Duration::from_millis(10));
        }
        let add = Command::new("git")
            .arg("-C")
            .arg(&paths.user_skills_root)
            .args(["add", "--all"])
            .output()
            .unwrap();
        assert!(!add.status.success());
        assert!(String::from_utf8_lossy(&add.stderr).contains("index.lock"));
        let commit = Command::new("git")
            .arg("-C")
            .arg(&paths.user_skills_root)
            .args([
                "-c",
                "user.name=External",
                "-c",
                "user.email=external@example.invalid",
                "commit",
                "-m",
                "Concurrent commit",
            ])
            .output()
            .unwrap();
        assert!(!commit.status.success());
        assert!(String::from_utf8_lossy(&commit.stderr).contains("index.lock"));
        let old_tree = Command::new("git")
            .arg("-C")
            .arg(&paths.user_skills_root)
            .args(["rev-parse", &format!("{old_sha}^{{tree}}")])
            .output()
            .unwrap();
        assert!(old_tree.status.success());
        let concurrent_commit = Command::new("git")
            .arg("-C")
            .arg(&paths.user_skills_root)
            .args([
                "-c",
                "user.name=External",
                "-c",
                "user.email=external@example.invalid",
                "commit-tree",
                String::from_utf8_lossy(&old_tree.stdout).trim(),
                "-p",
                &old_sha,
                "-m",
                "Concurrent commit",
            ])
            .output()
            .unwrap();
        assert!(concurrent_commit.status.success());
        let concurrent_head = String::from_utf8(concurrent_commit.stdout)
            .unwrap()
            .trim()
            .to_string();
        let update_ref = Command::new("git")
            .arg("-C")
            .arg(&paths.user_skills_root)
            .args(["update-ref", "refs/heads/main", &concurrent_head, &old_sha])
            .output()
            .unwrap();
        assert!(update_ref.status.success());
        fs::write(&barrier, b"continue").unwrap();
        let error = worker.join().unwrap().unwrap_err();
        assert!(error.contains("refused") || error.contains("changed"));
        assert_eq!(
            git.rev_parse_optional(&paths.user_skills_root, "HEAD")
                .unwrap()
                .as_deref(),
            Some(concurrent_head.as_str())
        );
    }

    #[test]
    fn tracked_edit_during_materialization_is_restored_without_advancing_head() {
        let managed_root = temp_dir("inbound-tracked-edit");
        let (remote, work) = remote_with_skill("inbound-tracked-edit");
        configure_and_bootstrap(&managed_root, &remote);
        let paths = managed_paths(&managed_root);
        let git = skillbox_git::GitService::new();
        fs::write(
            work.join("demo/SKILL.md"),
            "---\nname: demo\ndescription: Remote edit\n---\n",
        )
        .unwrap();
        git.add_all(&work).unwrap();
        git.commit(&work, "Remote edit").unwrap();
        git.push_origin_main(&work, false).unwrap();
        let preview = preview_user_skills_inbound(&managed_root).unwrap();
        let old_sha = preview.status.local_sha.clone().unwrap();
        let new_sha = preview.status.remote_sha.clone().unwrap();
        let old_snapshot =
            validate_inbound_git_tree(&git, &paths.user_skills_root, &old_sha, &mut Vec::new())
                .unwrap();
        let new_snapshot =
            validate_inbound_git_tree(&git, &paths.user_skills_root, &new_sha, &mut Vec::new())
                .unwrap();
        let barrier = temp_dir("inbound-tracked-edit-barrier").join("continue");
        let ready = barrier.with_extension("ready");
        let repo = paths.user_skills_root.clone();
        let barrier_for_thread = barrier.clone();
        let old_sha_for_worker = old_sha.clone();
        let worker = std::thread::spawn(move || {
            let mut audit = InboundApplyAudit::default();
            apply_inbound_tree(
                &skillbox_git::GitService::new(),
                &repo,
                Some(&old_sha_for_worker),
                &new_sha,
                old_snapshot.entries,
                new_snapshot.entries,
                "tracked-edit",
                &InboundMutationOptions {
                    pause_before_materialization: Some(barrier_for_thread),
                    ..Default::default()
                },
                &mut audit,
            )
        });
        while !ready.exists() {
            std::thread::sleep(Duration::from_millis(10));
        }
        let local_edit = b"---\nname: demo\ndescription: Local concurrent edit\n---\n";
        fs::write(paths.user_skills_root.join("demo/SKILL.md"), local_edit).unwrap();
        fs::write(&barrier, b"continue").unwrap();
        let error = worker.join().unwrap().unwrap_err();
        assert!(error.contains("changed during inbound apply"));
        assert_eq!(
            fs::read(paths.user_skills_root.join("demo/SKILL.md")).unwrap(),
            local_edit
        );
        assert_eq!(
            git.rev_parse_optional(&paths.user_skills_root, "HEAD")
                .unwrap()
                .as_deref(),
            Some(old_sha.as_str())
        );
    }

    #[test]
    fn mutation_receipt_holds_index_lock_until_reindex_window_finishes() {
        let managed_root = temp_dir("inbound-index-lock-lifetime");
        let (remote, work) = remote_with_skill("inbound-index-lock-lifetime");
        configure_and_bootstrap(&managed_root, &remote);
        let paths = managed_paths(&managed_root);
        let git = skillbox_git::GitService::new();
        fs::write(work.join("demo/README.md"), "incoming\n").unwrap();
        git.add_all(&work).unwrap();
        git.commit(&work, "Incoming").unwrap();
        git.push_origin_main(&work, false).unwrap();
        let preview = preview_user_skills_inbound(&managed_root).unwrap();
        let old_sha = preview.status.local_sha.clone().unwrap();
        let new_sha = preview.status.remote_sha.clone().unwrap();
        let old_snapshot =
            validate_inbound_git_tree(&git, &paths.user_skills_root, &old_sha, &mut Vec::new())
                .unwrap();
        let new_snapshot =
            validate_inbound_git_tree(&git, &paths.user_skills_root, &new_sha, &mut Vec::new())
                .unwrap();
        let mut audit = InboundApplyAudit::default();
        let receipt = apply_inbound_tree(
            &git,
            &paths.user_skills_root,
            Some(&old_sha),
            &new_sha,
            old_snapshot.entries,
            new_snapshot.entries,
            "index-lock-lifetime",
            &InboundMutationOptions::default(),
            &mut audit,
        )
        .unwrap();
        assert!(paths.user_skills_root.join(".git/index.lock").exists());
        let add_while_locked = Command::new("git")
            .arg("-C")
            .arg(&paths.user_skills_root)
            .args(["add", "--all"])
            .output()
            .unwrap();
        assert!(!add_while_locked.status.success());
        drop(receipt);
        assert!(!paths.user_skills_root.join(".git/index.lock").exists());
        let add_after_drop = Command::new("git")
            .arg("-C")
            .arg(&paths.user_skills_root)
            .args(["add", "--all"])
            .output()
            .unwrap();
        assert!(add_after_drop.status.success());
    }

    #[test]
    fn remote_only_failure_before_index_prepare_restores_generated_setup_state() {
        let managed_root = temp_dir("inbound-remote-only-pre-index-failure");
        let (remote, _) = remote_with_skill("inbound-remote-only-pre-index-failure");
        set_user_skills_git_remote(
            UserSkillsGitRemoteRequest {
                remote_url: remote.to_string_lossy().to_string(),
            },
            &managed_root,
        )
        .unwrap();
        let preview = preview_user_skills_inbound(&managed_root).unwrap();
        let paths = managed_paths(&managed_root);
        let git = skillbox_git::GitService::new();
        let new_sha = preview.status.remote_sha.clone().unwrap();
        let new_snapshot =
            validate_inbound_git_tree(&git, &paths.user_skills_root, &new_sha, &mut Vec::new())
                .unwrap();
        let index_before = read_optional_file(&paths.user_skills_root.join(".git/index")).unwrap();
        let mut audit = InboundApplyAudit::default();

        let error = apply_inbound_tree(
            &git,
            &paths.user_skills_root,
            None,
            &new_sha,
            HashMap::new(),
            new_snapshot.entries,
            "pre-index-failure",
            &InboundMutationOptions {
                fail_before_index_prepare: true,
                ..Default::default()
            },
            &mut audit,
        )
        .unwrap_err();

        assert!(error.contains("previous Git state was restored"));
        assert_eq!(
            fs::read_to_string(paths.user_skills_root.join(".gitignore")).unwrap(),
            DEFAULT_USER_SKILLS_GITIGNORE
        );
        assert_eq!(
            read_optional_file(&paths.user_skills_root.join(".git/index")).unwrap(),
            index_before
        );
        assert_eq!(
            git.rev_parse_optional(&paths.user_skills_root, "HEAD")
                .unwrap(),
            None
        );
        let retry = preview_user_skills_inbound(&managed_root).unwrap();
        apply_user_skills_inbound(
            UserSkillsInboundApplyRequest {
                preview_id: Some(retry.preview_id),
                actor: "test".to_string(),
            },
            &managed_root,
        )
        .unwrap();
    }

    #[test]
    fn inbound_backup_parent_symlink_escape_is_rejected_without_mutation() {
        let managed_root = temp_dir("inbound-backup-symlink");
        let (remote, work) = remote_with_skill("inbound-backup-symlink");
        configure_and_bootstrap(&managed_root, &remote);
        let paths = managed_paths(&managed_root);
        let git = skillbox_git::GitService::new();
        fs::write(
            work.join("demo/SKILL.md"),
            "---\nname: demo\ndescription: Incoming\n---\n",
        )
        .unwrap();
        git.add_all(&work).unwrap();
        git.commit(&work, "Incoming").unwrap();
        git.push_origin_main(&work, false).unwrap();
        let preview = preview_user_skills_inbound(&managed_root).unwrap();
        let old_sha = preview.status.local_sha.clone().unwrap();
        let new_sha = preview.status.remote_sha.clone().unwrap();
        let old_snapshot =
            validate_inbound_git_tree(&git, &paths.user_skills_root, &old_sha, &mut Vec::new())
                .unwrap();
        let new_snapshot =
            validate_inbound_git_tree(&git, &paths.user_skills_root, &new_sha, &mut Vec::new())
                .unwrap();
        let old_index = read_optional_file(&paths.user_skills_root.join(".git/index")).unwrap();
        let old_skill = fs::read(paths.user_skills_root.join("demo/SKILL.md")).unwrap();
        let outside = temp_dir("inbound-backup-symlink-outside");
        std::os::unix::fs::symlink(&outside, paths.user_skills_root.join(".git/skillbox")).unwrap();
        let mut audit = InboundApplyAudit::default();

        let error = apply_inbound_tree(
            &git,
            &paths.user_skills_root,
            Some(&old_sha),
            &new_sha,
            old_snapshot.entries,
            new_snapshot.entries,
            "backup-symlink",
            &InboundMutationOptions::default(),
            &mut audit,
        )
        .unwrap_err();

        assert!(error.contains("not a real directory"));
        assert_eq!(
            git.rev_parse_optional(&paths.user_skills_root, "HEAD")
                .unwrap()
                .as_deref(),
            Some(old_sha.as_str())
        );
        assert_eq!(
            read_optional_file(&paths.user_skills_root.join(".git/index")).unwrap(),
            old_index
        );
        assert_eq!(
            fs::read(paths.user_skills_root.join("demo/SKILL.md")).unwrap(),
            old_skill
        );
        assert!(fs::read_dir(outside).unwrap().next().is_none());
    }

    #[test]
    fn conflict_analysis_includes_skill_that_exists_only_in_merge_base() {
        let managed_root = temp_dir("inbound-base-only-conflict-skill");
        let (remote, work) = remote_with_skill("inbound-base-only-conflict-skill");
        configure_and_bootstrap(&managed_root, &remote);
        let paths = managed_paths(&managed_root);
        let git = skillbox_git::GitService::new();

        fs::write(
            work.join("demo/SKILL.md"),
            vec![b'x'; MAX_INBOUND_FILE_BYTES as usize + 1],
        )
        .unwrap();
        git.add_all(&work).unwrap();
        git.commit(&work, "Create historical oversized skill")
            .unwrap();
        git.push_origin_main(&work, false).unwrap();
        let historical_base = git
            .fetch_origin_main(&paths.user_skills_root)
            .unwrap()
            .unwrap();
        git.fast_forward_only(&paths.user_skills_root, &historical_base)
            .unwrap();

        fs::remove_dir_all(paths.user_skills_root.join("demo")).unwrap();
        git.add_all(&paths.user_skills_root).unwrap();
        git.commit(&paths.user_skills_root, "Delete local demo")
            .unwrap();
        fs::remove_dir_all(work.join("demo")).unwrap();
        git.add_all(&work).unwrap();
        git.commit(&work, "Delete remote demo").unwrap();
        git.push_origin_main(&work, false).unwrap();

        let preview = preview_user_skills_inbound(&managed_root).unwrap();
        assert_eq!(preview.status.relation, UserSkillsInboundRelation::Diverged);
        assert!(preview
            .conflict_analysis
            .unwrap()
            .both_changed_skills
            .iter()
            .any(|name| name == "demo"));
    }

    fn configure_and_bootstrap(managed_root: &Path, remote: &Path) {
        set_user_skills_git_remote(
            UserSkillsGitRemoteRequest {
                remote_url: remote.to_string_lossy().to_string(),
            },
            managed_root,
        )
        .unwrap();
        let preview = preview_user_skills_inbound(managed_root).unwrap();
        apply_user_skills_inbound(
            UserSkillsInboundApplyRequest {
                preview_id: Some(preview.preview_id),
                actor: "test".to_string(),
            },
            managed_root,
        )
        .unwrap();
    }

    fn remote_with_skill(label: &str) -> (PathBuf, PathBuf) {
        let remote = temp_dir(&format!("{label}-bare")).join("remote.git");
        Command::new("git")
            .args(["init", "--bare"])
            .arg(&remote)
            .output()
            .unwrap();
        let work = temp_dir(&format!("{label}-work"));
        let git = skillbox_git::GitService::new();
        git.init_main(&work).unwrap();
        fs::create_dir_all(work.join("demo")).unwrap();
        fs::write(
            work.join("demo/SKILL.md"),
            "---\nname: demo\ndescription: Demo\n---\n",
        )
        .unwrap();
        git.add_all(&work).unwrap();
        git.commit(&work, "Initial skill").unwrap();
        git.set_origin_url(&work, remote.to_str().unwrap()).unwrap();
        git.push_origin_main(&work, true).unwrap();
        (remote, work)
    }

    fn temp_dir(prefix: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("{prefix}-{nonce}"));
        fs::create_dir_all(&path).unwrap();
        path
    }
}
