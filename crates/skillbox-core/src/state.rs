use crate::*;

pub fn managed_state(managed_root: impl AsRef<Path>) -> Result<ManagedState> {
    let paths = ensure_managed_layout(managed_root.as_ref().to_path_buf())?;
    let mut deployments = load_deployments(&paths.database_path)?;
    let usage_by_skill = load_usage_by_skill(&paths.database_path)?;
    let mut skills = Vec::new();

    for skill in scan_skill_roots(std::slice::from_ref(&paths.user_skills_root))?.skills {
        skills.push(managed_skill(skill, SkillKind::User));
    }
    skills.extend(scan_managed_remote_skills(&paths)?);
    let workspaces = load_workspaces(&paths.database_path)?;
    merge_workspace_symlink_deployments(&workspaces, &skills, &mut deployments);

    for skill in skills.iter_mut() {
        skill.deployments = deployments.get(&skill.name).cloned().unwrap_or_default();
        if let Some(usage) = usage_by_skill.get(&skill.name) {
            skill.usage_count = usage.usage_count;
            skill.last_used_at = usage.last_used_at.clone();
        }
    }

    skills.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(ManagedState {
        is_first_use: skills.is_empty(),
        paths,
        skills,
    })
}

pub(crate) fn scan_managed_remote_skills(paths: &ManagedPaths) -> Result<Vec<ManagedSkill>> {
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

pub(crate) fn merge_workspace_symlink_deployments(
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

pub(crate) fn workspace_symlink_paths_to_managed_skill(
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

pub(crate) fn workspace_target_is_current_symlink(target_path: &Path, managed_path: &Path) -> bool {
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
