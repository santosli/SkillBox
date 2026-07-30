use crate::*;
use fs2::FileExt;
use std::fs::{File, OpenOptions};

const USER_SKILLS_REMOTE_BRANCH: &str = "main";
const MAX_INBOUND_FILE_BYTES: u64 = 10 * 1024 * 1024;
const MAX_INBOUND_TREE_BYTES: u64 = 100 * 1024 * 1024;
const MAX_INBOUND_TREE_ENTRIES: usize = 10_000;

#[derive(Debug, Clone)]
struct InboundTreeSnapshot {
    skills: Vec<Skill>,
    entries: HashMap<String, skillbox_git::GitTreeEntry>,
}

type InboundDeployments = HashMap<String, Vec<ManagedSkillDeployment>>;
type InboundDeploymentProfiles = HashMap<PathBuf, (String, String)>;

pub fn check_user_skills_inbound(
    managed_root: impl AsRef<Path>,
) -> Result<UserSkillsInboundStatus> {
    let paths = managed_paths(managed_root.as_ref().to_path_buf());
    let repo = paths.user_skills_root;
    let git = skillbox_git::GitService::new();
    let status = git.status(&repo)?;
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
    let checked = check_user_skills_inbound(&managed_root)?;
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
    let result = apply_user_skills_inbound_inner(&managed_root, &preview_id, &operation.id);
    match &result {
        Ok(applied) => {
            finish_operation(
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
            )?;
        }
        Err(error) => {
            finish_operation(
                OperationFinish {
                    id: operation.id,
                    status: OperationStatus::Failed,
                    summary: "User skills fast-forward failed".to_string(),
                    error: Some(error.clone()),
                    payload: serde_json::json!({}),
                },
                &managed_root,
            )?;
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
    let Some(scheme_index) = trimmed.find("://") else {
        return trimmed.to_string();
    };
    let authority_start = scheme_index + 3;
    let Some(path_offset) = trimmed[authority_start..].find('/') else {
        return trimmed.to_string();
    };
    let authority_end = authority_start + path_offset;
    let authority = &trimmed[authority_start..authority_end];
    let Some(at_index) = authority.rfind('@') else {
        return trimmed.to_string();
    };
    format!(
        "{}{}",
        &trimmed[..authority_start],
        &trimmed[authority_start + at_index + 1..]
    )
}

fn sanitize_git_error(error: &str, remote_url: &str) -> String {
    error.replace(remote_url, &sanitize_git_remote_url(remote_url))
}

fn inbound_status_from_refs(
    repo: &Path,
    fetched_at: Option<String>,
) -> Result<UserSkillsInboundStatus> {
    let git = skillbox_git::GitService::new();
    let git_status = git.status(repo)?;
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
    let git_files = match (status.local_sha.as_deref(), status.remote_sha.as_deref()) {
        (Some(local_sha), Some(remote_sha)) => git.diff_refs(repo, local_sha, remote_sha)?,
        (None, Some(_)) => new_snapshot
            .entries
            .keys()
            .map(|path| skillbox_git::GitDiffFile {
                path: path.clone(),
                old_path: None,
                status: "A".to_string(),
                diff: String::new(),
            })
            .collect(),
        _ => Vec::new(),
    };
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
    let skill_changes = inbound_skill_changes(
        &old_snapshot.skills,
        &new_snapshot.skills,
        &git_files,
        &deployments,
        &deployment_profiles,
        &paths.user_skills_root,
        &mut safety_issues,
    );
    let skill_names = old_snapshot
        .skills
        .iter()
        .chain(new_snapshot.skills.iter())
        .map(|skill| skill.name.as_str())
        .collect::<HashSet<_>>();
    let repository_files = files
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
) -> Result<UserSkillsInboundApplyResult> {
    let paths = managed_paths(managed_root.to_path_buf());
    let _lock = acquire_inbound_lock(&paths.user_skills_root)?;
    let checked = check_user_skills_inbound(managed_root)?;
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
    let git = skillbox_git::GitService::new();
    match preview.status.relation {
        UserSkillsInboundRelation::Behind => {
            git.fast_forward_only(&paths.user_skills_root, &new_sha)?;
        }
        UserSkillsInboundRelation::RemoteOnly => {
            prepare_remote_only_bootstrap(&paths.user_skills_root)?;
            if let Err(error) =
                git.bootstrap_from_ref(&paths.user_skills_root, USER_SKILLS_REMOTE_BRANCH, &new_sha)
            {
                let _ = fs::write(
                    paths.user_skills_root.join(".gitignore"),
                    DEFAULT_USER_SKILLS_GITIGNORE,
                );
                return Err(error);
            }
        }
        _ => {
            return Err(
                "Only reviewed behind or remote-only histories can be fast-forwarded.".to_string(),
            );
        }
    }
    let applied_head = git
        .rev_parse_optional(&paths.user_skills_root, "HEAD")?
        .ok_or_else(|| "Fast-forward did not produce a local HEAD.".to_string())?;
    if applied_head != new_sha || git.status(&paths.user_skills_root)?.dirty {
        let cause =
            "Fast-forward target changed or the worktree became dirty before reindex.".to_string();
        return rollback_inbound_after_failure(
            &git,
            &paths.user_skills_root,
            old_sha.as_deref(),
            &cause,
        );
    }
    let scan = scan_skill_roots(std::slice::from_ref(&paths.user_skills_root))?;
    if !scan.errors.is_empty() {
        let error = format!(
            "Fast-forwarded tree failed reindex validation: {}",
            scan.errors
                .iter()
                .map(|item| item.error.as_str())
                .collect::<Vec<_>>()
                .join("; ")
        );
        return rollback_inbound_after_failure(
            &git,
            &paths.user_skills_root,
            old_sha.as_deref(),
            &error,
        );
    }
    if let Err(error) =
        reindex_user_skills(&paths.database_path, &scan.skills, &paths.user_skills_root)
    {
        return rollback_inbound_after_failure(
            &git,
            &paths.user_skills_root,
            old_sha.as_deref(),
            &format!("Unable to reindex fast-forwarded user skills: {error}"),
        );
    }
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

fn acquire_inbound_lock(repo: &Path) -> Result<File> {
    let lock_path = repo.join(".git").join("skillbox-inbound.lock");
    let lock = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(lock_path)
        .map_err(|error| format!("Unable to open inbound sync lock: {error}"))?;
    lock.try_lock_exclusive()
        .map_err(|_| "Another user-skills Git operation is already running.".to_string())?;
    Ok(lock)
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
    let skill_paths = entries_by_path
        .keys()
        .filter(|path| path.ends_with("/SKILL.md"))
        .cloned()
        .collect::<Vec<_>>();
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
    deployments
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
        .collect()
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
    let local_files = git
        .diff_refs(repo, base, local)?
        .into_iter()
        .flat_map(|file| [Some(file.path), file.old_path])
        .flatten()
        .collect::<HashSet<_>>();
    let remote_files = git
        .diff_refs(repo, base, remote)?
        .into_iter()
        .flat_map(|file| [Some(file.path), file.old_path])
        .flatten()
        .collect::<HashSet<_>>();
    let mut both_changed_files = local_files
        .intersection(&remote_files)
        .cloned()
        .collect::<Vec<_>>();
    both_changed_files.sort();
    let mut both_changed_skills = both_changed_files
        .iter()
        .filter_map(|path| {
            path.split_once('/')
                .filter(|(_, rest)| *rest == "SKILL.md" || rest.ends_with("/SKILL.md"))
                .map(|(name, _)| name.to_string())
        })
        .collect::<Vec<_>>();
    both_changed_skills.sort();
    both_changed_skills.dedup();
    let mut likely_conflict_files = git.merge_tree_analysis(repo, local, remote)?.conflict_files;
    likely_conflict_files.sort();
    likely_conflict_files.dedup();
    Ok(UserSkillsInboundConflictAnalysis {
        local_only_commits: status.ahead_count,
        remote_only_commits: status.behind_count,
        likely_conflict_files,
        both_changed_files,
        both_changed_skills,
    })
}

fn remote_only_worktree_is_safe(repo: &Path) -> Result<bool> {
    for entry in fs::read_dir(repo).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let name = entry.file_name();
        if name == ".git" {
            continue;
        }
        if name == ".gitignore"
            && entry.path().is_file()
            && fs::read_to_string(entry.path()).map_err(|error| error.to_string())?
                == DEFAULT_USER_SKILLS_GITIGNORE
        {
            continue;
        }
        return Ok(false);
    }
    Ok(true)
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
        "worktreeFingerprint": content_hash_text(&skillbox_git::GitService::new().status(repo)?.raw_status),
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

fn prepare_remote_only_bootstrap(repo: &Path) -> Result<()> {
    if !remote_only_worktree_is_safe(repo)? {
        return Err(
            "Remote-only initialization requires an empty repository with only SkillBox's generated .gitignore."
                .to_string(),
        );
    }
    let gitignore = repo.join(".gitignore");
    if gitignore.exists() {
        fs::remove_file(gitignore).map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn rollback_inbound_worktree(
    git: &skillbox_git::GitService,
    repo: &Path,
    old_sha: Option<&str>,
) -> Result<()> {
    match old_sha {
        Some(old) => git
            .restore_worktree_to_ref(repo, USER_SKILLS_REMOTE_BRANCH, old)
            .map(|_| ()),
        None => {
            let current = git
                .rev_parse_optional(repo, "HEAD")?
                .ok_or_else(|| "Git repository is already unborn.".to_string())?;
            git.restore_unborn_main(repo, &current)?;
            fs::write(repo.join(".gitignore"), DEFAULT_USER_SKILLS_GITIGNORE)
                .map_err(|error| error.to_string())
        }
    }
}

fn rollback_inbound_after_failure(
    git: &skillbox_git::GitService,
    repo: &Path,
    old_sha: Option<&str>,
    cause: &str,
) -> Result<UserSkillsInboundApplyResult> {
    match rollback_inbound_worktree(git, repo, old_sha) {
        Ok(()) => Err(format!("{cause} The previous Git state was restored.")),
        Err(rollback_error) => Err(format!(
            "{cause} Automatic recovery failed: {rollback_error}. Use the recorded backup ref and normal Git tooling before retrying."
        )),
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
    fn remote_only_preview_is_read_only_and_apply_bootstraps_reviewed_tree() {
        let managed_root = temp_dir("inbound-remote-only-managed");
        let (remote, _) = remote_with_skill("inbound-remote-only");
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
        assert!(preview.can_apply);
        assert!(preview
            .files
            .iter()
            .any(|file| file.path == "demo/SKILL.md"));
        assert_eq!(
            skillbox_git::GitService::new()
                .rev_parse_optional(&repo, "HEAD")
                .unwrap(),
            None
        );
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
            skillbox_git::GitService::new()
                .rev_parse_optional(&repo, "HEAD")
                .unwrap(),
            Some(result.new_sha)
        );
        assert!(repo.join("demo/SKILL.md").is_file());
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
