use crate::*;

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

pub fn install_github_remote_skill(
    request: InstallGithubRemoteSkillRequest,
    managed_root: impl AsRef<Path>,
) -> Result<InstallGithubRemoteSkillResult> {
    let managed_root = managed_root.as_ref().to_path_buf();
    let source = skillbox_github::parse_github_skill_url(&request.source_url)?;
    let source_url = request.source_url.clone();
    let operation = start_operation(
        OperationStart {
            operation_type: "install_remote_skill".to_string(),
            actor: request.actor.clone(),
            entity_type: "source".to_string(),
            entity_name: source.url.clone(),
            summary: format!("Install remote skill from {}", source.url),
            payload: serde_json::json!({"sourceUrl": request.source_url}),
        },
        &managed_root,
    )?;
    let operation_id = operation.id.clone();

    match install_github_remote_skill_inner(request, source, &managed_root, operation_id.clone()) {
        Ok(result) => {
            finish_operation(
                OperationFinish {
                    id: operation_id,
                    status: OperationStatus::Succeeded,
                    summary: format!("Installed remote skill {}", result.skill_name),
                    error: None,
                    payload: serde_json::json!({
                        "skillName": result.skill_name.clone(),
                        "installedSha": result.installed_sha.clone(),
                        "sourceUrl": result.source_url.clone()
                    }),
                },
                &managed_root,
            )?;
            Ok(result)
        }
        Err(error) => {
            let _ = finish_operation(
                OperationFinish {
                    id: operation_id,
                    status: OperationStatus::Failed,
                    summary: "Install remote skill failed".to_string(),
                    error: Some(error.clone()),
                    payload: serde_json::json!({"sourceUrl": source_url}),
                },
                &managed_root,
            );
            Err(error)
        }
    }
}

fn install_github_remote_skill_inner(
    request: InstallGithubRemoteSkillRequest,
    source: skillbox_github::GitHubSkillSource,
    managed_root: &Path,
    operation_id: String,
) -> Result<InstallGithubRemoteSkillResult> {
    let paths = ensure_managed_layout(managed_root.to_path_buf())?;
    let temp = temporary_work_dir("github-install");

    let result = (|| {
        let checkout = temp.join("checkout");
        let installed_sha = skillbox_git::GitService::new().fetch_ref_path(
            &source.repo_url,
            &source.reference,
            &source.path,
            &checkout,
        )?;
        let skill_source_path = checkout.join(&source.path);
        let skill = read_skill(&skill_source_path)?;
        validate_skill_name(&skill.name)?;
        let remote_root = paths.remote_skills_root.join(&skill.name);
        let version_path = remote_root.join("versions").join(&installed_sha);

        let created_snapshot =
            install_github_version_snapshot(&skill_source_path, &version_path, &skill.name)?;
        let current_path = remote_root.join("current");
        let old_current_target = fs::read_link(&current_path).ok();
        if let Err(error) = update_current_symlink(&remote_root, &version_path) {
            if created_snapshot {
                let _ = remove_path_if_exists(&version_path);
            }
            return Err(error);
        }

        let ref_kind = resolve_ref_kind(&source.repo_url, &source.reference)?;
        let tracking = ref_kind == "branch";
        let preview = RemoteSourceBindingPreview {
            skill_name: skill.name.clone(),
            source_url: source.url.clone(),
            repo_url: source.repo_url.clone(),
            owner: source.owner.clone(),
            repo: source.repo.clone(),
            path: source.path.clone(),
            reference: source.reference.clone(),
            ref_kind: Some(ref_kind.clone()),
            tracking,
            current_version: installed_sha.clone(),
            installed_sha: Some(installed_sha.clone()),
            latest_sha: Some(installed_sha.clone()),
            validation: SourceBindingValidation::ExactMatch,
            local_hash: skill.content_hash.clone(),
            remote_hash: Some(skill.content_hash.clone()),
            message: "Installed remote skill from GitHub.".to_string(),
        };
        let source_path = remote_root.join("source.json");
        if let Err(error) = write_github_source_metadata(&source_path, &preview).and_then(|_| {
            index_skill(
                &paths.database_path,
                &skill,
                SkillKind::Remote,
                &version_path,
            )
        }) {
            let _ = restore_current_after_failed_install(&remote_root, old_current_target.as_ref());
            if created_snapshot {
                let _ = remove_path_if_exists(&version_path);
            }
            return Err(error);
        }

        let deployment = request
            .target_root
            .map(|target_root| deploy_skill(&skill.name, managed_root, target_root))
            .transpose()?;

        Ok(InstallGithubRemoteSkillResult {
            skill_name: skill.name,
            source_url: source.url,
            repo_url: source.repo_url,
            owner: source.owner,
            repo: source.repo,
            path: source.path,
            reference: source.reference,
            ref_kind: Some(ref_kind),
            tracking,
            installed_sha,
            version_path,
            current_path,
            source_path,
            deployment,
            operation_id,
        })
    })();

    let _ = fs::remove_dir_all(&temp);
    result
}

fn install_github_version_snapshot(
    skill_source_path: &Path,
    version_path: &Path,
    expected_skill_name: &str,
) -> Result<bool> {
    if version_path.exists() {
        if let Ok(existing) = read_skill(version_path) {
            if existing.name == expected_skill_name {
                return Ok(false);
            }
            return Err(format!(
                "Existing version skill name does not match {expected_skill_name}"
            ));
        }
    }

    match copy_skill_dir(skill_source_path, version_path) {
        Ok(()) => Ok(true),
        Err(error) => {
            let _ = remove_path_if_exists(version_path);
            Err(error)
        }
    }
}

fn restore_current_after_failed_install(
    remote_root: &Path,
    old_current_target: Option<&PathBuf>,
) -> Result<()> {
    match old_current_target {
        Some(target) => update_current_symlink(remote_root, target),
        None => remove_path_if_exists(&remote_root.join("current")),
    }
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

        let git = skillbox_git::GitService::new();
        let git_files = git.diff_no_index_tree(&from_path, &to_path)?;
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

pub(crate) fn managed_skill(skill: Skill, kind: SkillKind) -> ManagedSkill {
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
        usage_count: 0,
        last_used_at: None,
    }
}

pub(crate) fn remote_skill_roots(paths: &ManagedPaths) -> Result<Vec<(String, PathBuf)>> {
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

pub(crate) fn no_source_remote_update_status(skill_name: &str) -> RemoteSkillUpdateStatus {
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

pub(crate) fn remote_source_browser_url(source: &RemoteSkillSource) -> Option<String> {
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

pub(crate) fn validate_remote_update_timeout_seconds(seconds: u32) -> Result<()> {
    if !(MIN_REMOTE_UPDATE_TIMEOUT_SECONDS..=MAX_REMOTE_UPDATE_TIMEOUT_SECONDS).contains(&seconds) {
        return Err(format!(
            "Remote update timeout must be between {MIN_REMOTE_UPDATE_TIMEOUT_SECONDS} and {MAX_REMOTE_UPDATE_TIMEOUT_SECONDS} seconds."
        ));
    }
    Ok(())
}

pub(crate) fn check_remote_skill_update_batch(
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

pub(crate) fn preserve_cached_remote_status_on_failure(
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

pub(crate) fn check_one_remote_skill_update(
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
    let source_repo_path = source.path.clone();

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

    let git = skillbox_git::GitService::new();
    match git.ls_remote_with_timeout(repo_url, reference, timeout) {
        Ok(Some(latest_sha)) => {
            let active_version = current_version.as_deref().or(installed_sha.as_deref());
            let update_available = if active_version == Some(latest_sha.as_str()) {
                false
            } else if let Some(source_repo_path) = source_repo_path.as_deref() {
                match remote_skill_path_changed(
                    &git,
                    remote_root,
                    repo_url,
                    &latest_sha,
                    source_repo_path,
                    timeout,
                ) {
                    Ok(Some(changed)) => changed,
                    Ok(None) => true,
                    Err(error) => {
                        return RemoteSkillUpdateStatus {
                            skill_name: skill_name.to_string(),
                            source_type: Some(source.source_type),
                            source_url,
                            current_version,
                            installed_sha,
                            latest_sha: Some(latest_sha),
                            ref_kind: status_ref_kind,
                            tracking,
                            update_available: false,
                            state: RemoteSkillUpdateState::CheckFailed,
                            message: Some(format!("Git path update check failed: {error}")),
                        };
                    }
                }
            } else {
                true
            };
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

pub(crate) fn remote_skill_path_changed(
    git: &skillbox_git::GitService,
    remote_root: &Path,
    repo_url: &str,
    latest_sha: &str,
    source_repo_path: &str,
    timeout: Duration,
) -> Result<Option<bool>> {
    if !is_full_git_sha(latest_sha) {
        return Ok(None);
    }

    let temp = temporary_work_dir("remote-update-check");
    let result = (|| {
        let Some(current_path) = current_remote_skill_path(remote_root)? else {
            return Ok(None);
        };
        let checkout = temp.join("checkout");
        git.fetch_ref_path_with_timeout(
            repo_url,
            latest_sha,
            source_repo_path,
            &checkout,
            timeout,
        )?;
        let latest_path = checkout.join(source_repo_path);
        let files = git.diff_no_index_tree(current_path, latest_path)?;
        Ok(Some(!files.is_empty()))
    })();
    let _ = fs::remove_dir_all(&temp);
    result
}

pub(crate) fn is_full_git_sha(value: &str) -> bool {
    value.len() == 40 && value.chars().all(|ch| ch.is_ascii_hexdigit())
}

pub(crate) fn current_remote_skill_path(remote_root: &Path) -> Result<Option<PathBuf>> {
    let current = remote_root.join("current");
    let target = match fs::read_link(&current) {
        Ok(target) => target,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.to_string()),
    };
    let current_path = if target.is_absolute() {
        target
    } else {
        current
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .join(target)
    };
    if current_path.exists() {
        Ok(Some(current_path))
    } else {
        Ok(None)
    }
}

pub(crate) fn current_remote_version(paths: &ManagedPaths, skill_name: &str) -> Result<String> {
    let current = paths.remote_skills_root.join(skill_name).join("current");
    let target = fs::read_link(&current).map_err(|error| error.to_string())?;
    target
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string)
        .ok_or_else(|| format!("Current version target is invalid: {}", current.display()))
}

pub(crate) fn temporary_work_dir(label: &str) -> PathBuf {
    use std::time::{SystemTime, UNIX_EPOCH};

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    std::env::temp_dir().join(format!("skillbox-{label}-{nanos}"))
}

pub(crate) fn remove_path_if_exists(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {
            fs::remove_dir_all(path).map_err(|error| error.to_string())
        }
        Ok(_) => fs::remove_file(path).map_err(|error| error.to_string()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

pub(crate) fn resolve_ref_kind(repo_url: &str, reference: &str) -> Result<String> {
    if skillbox_github::classify_ref_text(reference) == skillbox_github::GitHubRefKind::Commit {
        return Ok("commit".to_string());
    }
    let git = skillbox_git::GitService::new();
    if git
        .ls_remote(repo_url, &format!("refs/heads/{reference}"))?
        .is_some()
    {
        return Ok("branch".to_string());
    }
    if git
        .ls_remote(repo_url, &format!("refs/tags/{reference}"))?
        .is_some()
    {
        return Ok("tag".to_string());
    }
    Ok("branch".to_string())
}

pub(crate) fn source_binding_message(
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

pub(crate) fn source_binding_validation_label(validation: SourceBindingValidation) -> &'static str {
    match validation {
        SourceBindingValidation::ExactMatch => "exact_match",
        SourceBindingValidation::SameSkillChanged => "same_skill_changed",
        SourceBindingValidation::Mismatch => "mismatch",
    }
}

pub(crate) fn write_github_source_metadata(
    path: &Path,
    preview: &RemoteSourceBindingPreview,
) -> Result<()> {
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

pub(crate) fn read_remote_source(remote_root: &Path) -> Result<RemoteSkillSource> {
    let source_path = remote_root.join("source.json");
    let content = fs::read_to_string(&source_path).map_err(|error| error.to_string())?;
    parse_remote_source_content(&content)
}

pub(crate) fn parse_remote_source_content(content: &str) -> Result<RemoteSkillSource> {
    let source: RemoteSkillSource =
        serde_json::from_str(content).map_err(|error| error.to_string())?;
    validate_remote_source(&source)?;
    Ok(source)
}

pub(crate) fn validate_remote_source(source: &RemoteSkillSource) -> Result<()> {
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

pub(crate) fn validate_remote_source_repo_url(repo_url: &str) -> Result<()> {
    #[cfg(test)]
    {
        if Path::new(repo_url).is_absolute() {
            return Ok(());
        }
    }
    skillbox_github::validate_github_repo_url(repo_url)
}

pub(crate) fn resolve_remote_version_change_target(
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

pub(crate) fn resolve_remote_version_prefix(
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

pub(crate) fn remote_version_preview_target(
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
    let git = skillbox_git::GitService::new();
    git.fetch_ref_path(&repo_url, to_version, &source_path, &checkout)?;
    Ok(checkout.join(source_path))
}

pub(crate) fn short_version_label(version: &str) -> String {
    if version.starts_with("manual-") {
        version.to_string()
    } else {
        version.chars().take(12).collect()
    }
}

pub(crate) fn remote_diff_file(
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
pub(crate) struct FileMetadata {
    hash: Option<String>,
    size: Option<u64>,
    binary: bool,
    too_large: bool,
}

pub(crate) fn file_metadata(path: &Path) -> Result<FileMetadata> {
    if !path.exists() {
        return Ok(FileMetadata {
            hash: None,
            size: None,
            binary: false,
            too_large: false,
        });
    }

    if path.is_dir() {
        return Ok(FileMetadata {
            hash: None,
            size: None,
            binary: false,
            too_large: false,
        });
    }

    let bytes = fs::read(path).map_err(|error| error.to_string())?;
    let size = bytes.len() as u64;
    let too_large = bytes.len() > MAX_TEXT_DIFF_PREVIEW_BYTES;
    let binary = std::str::from_utf8(&bytes).is_err();
    Ok(FileMetadata {
        hash: Some(sha256_bytes(&bytes)),
        size: Some(size),
        binary,
        too_large,
    })
}

pub(crate) fn remote_diff_label(status: &str) -> &'static str {
    match status.chars().next() {
        Some('A') => "Added",
        Some('D') => "Deleted",
        Some('M') => "Modified",
        Some('R') => "Renamed",
        Some('C') => "Copied",
        _ => "Changed",
    }
}

pub(crate) fn content_hash_text(text: &str) -> String {
    sha256_bytes(text.as_bytes())
}

pub(crate) fn classify_affected_deployments(
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

pub(crate) fn apply_remote_version_change_inner(
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

pub(crate) fn remote_version_preview_id(
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

pub(crate) fn validate_remote_version_preview_id(
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

pub(crate) fn resolve_remote_version_apply_target(
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

pub(crate) fn ensure_github_version_snapshot(
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
        let git = skillbox_git::GitService::new();
        git.fetch_ref_tree(&repo_url, target_sha, &checkout)?;
        copy_skill_dir_from_checkout(&checkout.join(source_path), &version_path, &checkout)?;
        read_skill(&version_path)?;
        Ok(version_path.clone())
    })();

    let _ = fs::remove_dir_all(&temp);
    if result.is_err() {
        let _ = fs::remove_dir_all(&version_path);
    }
    result
}

pub(crate) fn update_remote_metadata_after_change(
    remote_root: &Path,
    to_version: &str,
) -> Result<()> {
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

pub(crate) fn remote_version_action_label(action: RemoteVersionChangeAction) -> &'static str {
    match action {
        RemoteVersionChangeAction::Update => "update",
        RemoteVersionChangeAction::Rollback => "rollback",
    }
}
