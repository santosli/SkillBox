use crate::*;

pub fn user_skills_git_status(managed_root: impl AsRef<Path>) -> Result<UserSkillsGitStatus> {
    let paths = ensure_managed_layout(managed_root.as_ref().to_path_buf())?;
    user_skills_git_status_for_repo(paths.user_skills_root)
}

pub fn user_skills_git_changes(managed_root: impl AsRef<Path>) -> Result<UserSkillsGitChanges> {
    let paths = ensure_managed_layout(managed_root.as_ref().to_path_buf())?;
    let repo = paths.user_skills_root;
    let status = user_skills_git_status_for_repo(repo.clone())?;
    let git = skillbox_git::GitService::new();
    let files = if status.initialized {
        git.changed_files(&repo)?
            .into_iter()
            .map(|file| {
                let diff = if file.status == "??" || !git.has_head(&repo) {
                    new_file_diff(&repo, &file.path)
                } else {
                    git.diff_head_path(&repo, &file.path)
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

    let git = skillbox_git::GitService::new();
    let log_entries = git.log_path(&repo, skill_name, 20)?;
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
    let git = skillbox_git::GitService::new();
    if !git.status(&repo)?.initialized {
        git.init_main(&repo)?;
    }
    git.set_origin_url(&repo, remote_url)?;
    user_skills_git_status_for_repo(repo)
}

pub fn sync_user_skills_git(
    request: UserSkillsSyncRequest,
    managed_root: impl AsRef<Path>,
) -> Result<UserSkillsSyncResult> {
    let paths = ensure_managed_layout(managed_root.as_ref().to_path_buf())?;
    let repo = paths.user_skills_root;
    let git = skillbox_git::GitService::new();
    let before = git.status(&repo)?;
    let initialized = !before.initialized;

    if initialized {
        git.init_main(&repo)?;
    }

    let mut remote_updated = false;
    let current_remote = if repo.join(".git").exists() {
        git.origin_url(&repo)?
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
            git.set_origin_url(&repo, remote_url)?;
            remote_updated = true;
        }
    } else if request
        .remote_url
        .as_deref()
        .is_some_and(|value| value.trim().is_empty())
    {
        return Err("Git remote URL cannot be empty.".to_string());
    }

    if request.push && git.origin_url(&repo)?.is_none() {
        return Err("Git remote URL is required before syncing user skills.".to_string());
    }

    if let Some(paths) = &request.selected_paths {
        let selected_paths = validate_git_relative_paths(paths)?;
        git.add_paths(&repo, &selected_paths)?;
    } else {
        git.add_all(&repo)?;
    }
    let has_staged_changes = git.staged_changes(&repo)?;
    let commit_message = normalized_commit_message(request.commit_message.as_deref());
    let commit_sha = if has_staged_changes {
        Some(git.commit(&repo, &commit_message)?)
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
        match git.push_origin_main(&repo, true) {
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

pub(crate) fn user_skills_git_status_for_repo(repo_path: PathBuf) -> Result<UserSkillsGitStatus> {
    let git = skillbox_git::GitService::new();
    let git_status = git.status(&repo_path)?;
    let remote_url = if git_status.initialized {
        git.origin_url(&repo_path)?
    } else {
        None
    };
    let changed_paths = if git_status.initialized {
        git.changed_files(&repo_path)?
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

pub(crate) fn user_skills_git_state(
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

pub(crate) fn user_skill_has_uncommitted_changes(repo: &Path, skill_name: &str) -> Result<bool> {
    let git = skillbox_git::GitService::new();
    if !git.status(repo)?.initialized {
        return Ok(false);
    }

    Ok(git
        .changed_files(repo)?
        .into_iter()
        .any(|file| git_path_belongs_to_skill(&file.path, skill_name)))
}

pub(crate) fn git_path_belongs_to_skill(path: &str, skill_name: &str) -> bool {
    path == skill_name
        || path
            .strip_prefix(skill_name)
            .and_then(|rest| rest.strip_prefix('/'))
            .is_some()
}

pub(crate) fn normalized_commit_message(message: Option<&str>) -> String {
    message
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("chore(github): sync user skills")
        .to_string()
}

pub(crate) fn validate_git_relative_paths(paths: &[String]) -> Result<Vec<String>> {
    if paths.is_empty() {
        return Err("Select at least one file to commit.".to_string());
    }

    paths
        .iter()
        .map(|path| validate_git_relative_path(path))
        .collect()
}

pub(crate) fn validate_git_relative_path(path: &str) -> Result<String> {
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

pub(crate) fn collect_user_skill_files(root: &Path) -> Result<Vec<String>> {
    let mut files = Vec::new();
    collect_user_skill_files_rec(root, root, &mut files)?;
    files.sort();
    Ok(files)
}

pub(crate) fn collect_user_skill_files_rec(
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

pub(crate) fn new_file_diff(repo: &Path, relative_path: &str) -> Result<String> {
    let relative_path = validate_git_relative_path(relative_path)?;
    let path = repo.join(&relative_path);
    let bytes = fs::read(&path).map_err(|error| error.to_string())?;

    if bytes.len() > MAX_TEXT_DIFF_PREVIEW_BYTES {
        return Ok(format!(
            "diff --git a/{relative_path} b/{relative_path}\nnew file mode 100644\n--- /dev/null\n+++ b/{relative_path}\n@@\n+Diff omitted because the file is larger than 1 MB.\n"
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
