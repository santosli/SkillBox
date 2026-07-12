use crate::*;

pub fn parse_skill_frontmatter(input: &str) -> SkillMetadata {
    let mut metadata = SkillMetadata {
        name: String::new(),
        description: String::new(),
        version: String::new(),
    };
    let mut lines = input.lines().peekable();
    if lines.next() != Some("---") {
        return metadata;
    }

    while let Some(line) = lines.next() {
        if line == "---" {
            break;
        }
        if line.starts_with(' ') || line.starts_with('\t') {
            continue;
        }
        if let Some((key, value)) = line.split_once(':') {
            let value = parse_frontmatter_value(value.trim(), &mut lines);
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

pub(crate) fn parse_frontmatter_value<'a, I>(
    value: &str,
    lines: &mut std::iter::Peekable<I>,
) -> String
where
    I: Iterator<Item = &'a str>,
{
    if value.starts_with('>') {
        return frontmatter_block_lines(lines)
            .into_iter()
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
    }
    if value.starts_with('|') {
        return frontmatter_block_lines(lines).join("\n");
    }

    unquote(value)
}

pub(crate) fn frontmatter_block_lines<'a, I>(lines: &mut std::iter::Peekable<I>) -> Vec<String>
where
    I: Iterator<Item = &'a str>,
{
    let mut block_lines = Vec::new();

    while let Some(line) = lines.peek().copied() {
        if line == "---" {
            break;
        }
        if !line.is_empty() && !line.starts_with(' ') && !line.starts_with('\t') {
            break;
        }
        block_lines.push(line.trim().to_string());
        lines.next();
    }

    while block_lines.last().is_some_and(|line| line.is_empty()) {
        block_lines.pop();
    }

    block_lines
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
    let source_dir = expand_home(source_dir.as_ref().to_path_buf());
    let managed_root = managed_root.as_ref().to_path_buf();
    let entity_name = source_dir
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("unknown")
        .to_string();
    audited_operation(
        OperationStart {
            operation_type: "import_skill".to_string(),
            actor: "core".to_string(),
            entity_type: "skill".to_string(),
            entity_name,
            summary: "Import managed skill".to_string(),
            payload: serde_json::json!({
                "sourcePath": source_dir,
                "skillType": kind.as_str()
            }),
        },
        &managed_root,
        || import_skill_unlogged(&source_dir, kind, &managed_root),
        |result| {
            (
                format!("Imported {}", result.name),
                serde_json::json!({
                    "managedPath": result.managed_path,
                    "skillType": result.kind.as_str(),
                    "contentHash": result.content_hash
                }),
            )
        },
    )
}

pub(crate) fn import_skill_unlogged(
    source_dir: &Path,
    kind: SkillKind,
    managed_root: &Path,
) -> Result<ImportedSkill> {
    let paths = ensure_managed_layout(managed_root.to_path_buf())?;
    let skill = read_skill(source_dir)?;
    validate_skill_name(&skill.name)?;

    let managed_path = match kind {
        SkillKind::User => paths.user_skills_root.join(&skill.name),
        SkillKind::Remote => paths
            .remote_skills_root
            .join(&skill.name)
            .join("versions")
            .join(format!("manual-{}", &skill.content_hash[..12])),
    };

    match kind {
        SkillKind::User => copy_skill_dir(&skill.path, &managed_path)?,
        SkillKind::Remote => {
            if managed_path.exists() {
                let existing = read_skill(&managed_path)?;
                if existing.name != skill.name || existing.content_hash != skill.content_hash {
                    return Err(format!(
                        "Existing remote version does not match {}",
                        skill.name
                    ));
                }
            } else {
                copy_skill_dir(&skill.path, &managed_path)?;
            }
            update_current_symlink(&paths.remote_skills_root.join(&skill.name), &managed_path)?;
        }
    }

    index_skill(&paths.database_path, &skill, kind, &managed_path)?;
    Ok(ImportedSkill {
        name: skill.name,
        kind,
        managed_path,
        content_hash: skill.content_hash,
    })
}

pub fn change_skill_kind(
    skill_name: &str,
    kind: SkillKind,
    managed_root: impl AsRef<Path>,
) -> Result<ImportedSkill> {
    let managed_root = managed_root.as_ref().to_path_buf();
    audited_operation(
        OperationStart {
            operation_type: "change_skill_kind".to_string(),
            actor: "core".to_string(),
            entity_type: "skill".to_string(),
            entity_name: skill_name.to_string(),
            summary: format!("Change {skill_name} skill type"),
            payload: serde_json::json!({"targetType": kind.as_str()}),
        },
        &managed_root,
        || change_skill_kind_unlogged(skill_name, kind, &managed_root),
        |result| {
            (
                format!("Changed {} to {}", result.name, result.kind.as_str()),
                serde_json::json!({
                    "skillType": result.kind.as_str(),
                    "managedPath": result.managed_path,
                    "contentHash": result.content_hash
                }),
            )
        },
    )
}

fn change_skill_kind_unlogged(
    skill_name: &str,
    kind: SkillKind,
    managed_root: &Path,
) -> Result<ImportedSkill> {
    validate_skill_name(skill_name)?;
    let paths = ensure_managed_layout(managed_root.to_path_buf())?;
    let location = resolve_managed_skill_kind_location(&paths, skill_name)?;
    let skill = read_skill(&location.storage_path)?;

    if location.kind == kind {
        return Ok(ImportedSkill {
            name: skill.name,
            kind,
            managed_path: location.storage_path,
            content_hash: skill.content_hash,
        });
    }

    let target_storage_path = managed_skill_kind_destination(&paths, &skill, kind)?;
    if fs::symlink_metadata(&target_storage_path).is_ok() {
        return Err(format!(
            "Destination already exists: {}",
            target_storage_path.display()
        ));
    }
    if kind == SkillKind::Remote
        && fs::symlink_metadata(paths.remote_skills_root.join(skill_name).join("current")).is_ok()
    {
        return Err(format!("Remote skill already exists: {skill_name}"));
    }

    let deployment_target_paths = collect_skill_deployment_target_paths(
        &paths,
        skill_name,
        &location.deployment_target_path,
    )?;
    let old_reference_paths = managed_skill_kind_reference_paths(&location);
    if let Some(parent) = target_storage_path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }

    fs::rename(&location.storage_path, &target_storage_path).map_err(|error| error.to_string())?;
    if location.kind == SkillKind::Remote {
        remove_remote_current_symlink(&paths, skill_name)?;
        remove_empty_remote_skill_dirs(&paths, skill_name);
    }

    let new_deployment_target = match kind {
        SkillKind::User => target_storage_path.clone(),
        SkillKind::Remote => {
            let remote_root = paths.remote_skills_root.join(skill_name);
            update_current_symlink(&remote_root, &target_storage_path)?;
            remote_root.join("current")
        }
    };
    let moved_skill = read_skill(&target_storage_path)?;
    index_skill(
        &paths.database_path,
        &moved_skill,
        kind,
        &target_storage_path,
    )?;
    retarget_skill_deployment_symlinks(
        &deployment_target_paths,
        &old_reference_paths,
        &new_deployment_target,
    )?;

    Ok(ImportedSkill {
        name: moved_skill.name,
        kind,
        managed_path: target_storage_path,
        content_hash: moved_skill.content_hash,
    })
}

struct ManagedSkillKindLocation {
    kind: SkillKind,
    storage_path: PathBuf,
    deployment_target_path: PathBuf,
}

fn resolve_managed_skill_kind_location(
    paths: &ManagedPaths,
    skill_name: &str,
) -> Result<ManagedSkillKindLocation> {
    let user_path = paths.user_skills_root.join(skill_name);
    let user_exists = user_path.join("SKILL.md").exists();
    let remote_current = paths.remote_skills_root.join(skill_name).join("current");
    let remote_exists = remote_current.join("SKILL.md").exists();

    match (user_exists, remote_exists) {
        (true, true) => Err(format!(
            "Managed skill exists as both user and remote: {skill_name}"
        )),
        (true, false) => Ok(ManagedSkillKindLocation {
            kind: SkillKind::User,
            storage_path: user_path.clone(),
            deployment_target_path: user_path,
        }),
        (false, true) => Ok(ManagedSkillKindLocation {
            kind: SkillKind::Remote,
            storage_path: fs::canonicalize(&remote_current).map_err(|error| error.to_string())?,
            deployment_target_path: remote_current,
        }),
        (false, false) => Err(format!("Managed skill not found: {skill_name}")),
    }
}

fn resolve_delete_skill_location(
    paths: &ManagedPaths,
    skill_name: &str,
) -> Result<ManagedSkillKindLocation> {
    let user_path = paths.user_skills_root.join(skill_name);
    let remote_root = paths.remote_skills_root.join(skill_name);
    let user_exists = managed_entry_exists(&user_path)?;
    let remote_exists = managed_entry_exists(&remote_root)?;

    match (user_exists, remote_exists) {
        (true, true) => Err(format!(
            "Managed skill exists as both user and remote: {skill_name}"
        )),
        (true, false) => Ok(ManagedSkillKindLocation {
            kind: SkillKind::User,
            storage_path: user_path.clone(),
            deployment_target_path: user_path,
        }),
        (false, true) => Ok(ManagedSkillKindLocation {
            kind: SkillKind::Remote,
            storage_path: remote_root.clone(),
            deployment_target_path: remote_root.join("current"),
        }),
        (false, false) => Err(format!("Managed skill not found: {skill_name}")),
    }
}

fn managed_entry_exists(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!(
            "Cannot inspect managed path {}: {error}",
            path.display()
        )),
    }
}

fn managed_skill_kind_destination(
    paths: &ManagedPaths,
    skill: &Skill,
    kind: SkillKind,
) -> Result<PathBuf> {
    match kind {
        SkillKind::User => Ok(paths.user_skills_root.join(&skill.name)),
        SkillKind::Remote => {
            let version_hash = skill.content_hash.get(..12).unwrap_or(&skill.content_hash);
            Ok(paths
                .remote_skills_root
                .join(&skill.name)
                .join("versions")
                .join(format!("manual-{version_hash}")))
        }
    }
}

fn managed_skill_kind_reference_paths(location: &ManagedSkillKindLocation) -> Vec<PathBuf> {
    let mut paths = vec![
        normalize_lexical_path(&location.storage_path),
        normalize_lexical_path(&location.deployment_target_path),
    ];
    for path in [&location.storage_path, &location.deployment_target_path] {
        if let Ok(canonical) = fs::canonicalize(path) {
            paths.push(normalize_lexical_path(&canonical));
        }
    }
    paths.sort();
    paths.dedup();
    paths
}

fn delete_skill_reference_paths(
    paths: &ManagedPaths,
    skill_name: &str,
    location: &ManagedSkillKindLocation,
) -> Vec<PathBuf> {
    let mut references = managed_skill_kind_reference_paths(location);
    if location.kind == SkillKind::Remote {
        let versions_root = paths.remote_skills_root.join(skill_name).join("versions");
        if let Ok(entries) = fs::read_dir(&versions_root) {
            for entry in entries.filter_map(|entry| entry.ok()) {
                if !entry
                    .file_type()
                    .map(|file_type| file_type.is_dir())
                    .unwrap_or(false)
                {
                    continue;
                }
                let path = entry.path();
                references.push(normalize_lexical_path(&path));
                if let Ok(canonical) = fs::canonicalize(path) {
                    references.push(normalize_lexical_path(&canonical));
                }
            }
        }
    }
    references.sort();
    references.dedup();
    references
}

fn collect_skill_deployment_target_paths(
    paths: &ManagedPaths,
    skill_name: &str,
    old_managed_path: &Path,
) -> Result<Vec<PathBuf>> {
    let mut target_paths = Vec::new();
    let mut seen = HashSet::new();
    let deployments = load_deployments(&paths.database_path)?;

    for deployment in deployments.get(skill_name).cloned().unwrap_or_default() {
        push_unique_path(&mut target_paths, &mut seen, deployment.target_path);
    }

    for workspace in load_workspaces(&paths.database_path)? {
        let exact_target_path = workspace.path.join(skill_name);
        if workspace_target_is_current_symlink(&exact_target_path, old_managed_path) {
            push_unique_path(&mut target_paths, &mut seen, exact_target_path);
        }
        for target_path in
            workspace_symlink_paths_to_managed_skill(&workspace.path, old_managed_path)
        {
            push_unique_path(&mut target_paths, &mut seen, target_path);
        }
    }

    target_paths.sort();
    Ok(target_paths)
}

fn push_unique_path(target_paths: &mut Vec<PathBuf>, seen: &mut HashSet<PathBuf>, path: PathBuf) {
    if seen.insert(path.clone()) {
        target_paths.push(path);
    }
}

fn remove_remote_current_symlink(paths: &ManagedPaths, skill_name: &str) -> Result<()> {
    let current = paths.remote_skills_root.join(skill_name).join("current");
    match fs::symlink_metadata(&current) {
        Ok(metadata) => {
            if !metadata.file_type().is_symlink() {
                return Err(format!(
                    "Refusing to remove existing non-symlink current: {}",
                    current.display()
                ));
            }
            fs::remove_file(current).map_err(|error| error.to_string())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

fn remove_empty_remote_skill_dirs(paths: &ManagedPaths, skill_name: &str) {
    let remote_root = paths.remote_skills_root.join(skill_name);
    let _ = fs::remove_dir(remote_root.join("versions"));
    let _ = fs::remove_dir(remote_root);
}

fn retarget_skill_deployment_symlinks(
    target_paths: &[PathBuf],
    old_reference_paths: &[PathBuf],
    new_target: &Path,
) -> Result<()> {
    for target_path in target_paths {
        let Ok(metadata) = fs::symlink_metadata(target_path) else {
            continue;
        };
        if !metadata.file_type().is_symlink()
            || !symlink_targets_any_path(target_path, old_reference_paths)?
        {
            continue;
        }

        fs::remove_file(target_path).map_err(|error| error.to_string())?;
        symlink_dir(new_target, target_path)?;
    }
    Ok(())
}

fn symlink_targets_any_path(symlink: &Path, expected_paths: &[PathBuf]) -> Result<bool> {
    let target = fs::read_link(symlink).map_err(|error| error.to_string())?;
    let target = if target.is_absolute() {
        target
    } else {
        symlink
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .join(target)
    };
    let target = normalize_lexical_path(&target);
    Ok(expected_paths.contains(&target))
}

pub fn deploy_skill(
    skill_name: &str,
    managed_root: impl AsRef<Path>,
    target_root: impl AsRef<Path>,
) -> Result<Deployment> {
    let managed_root = managed_root.as_ref().to_path_buf();
    let target_root = expand_home(target_root.as_ref().to_path_buf());
    audited_operation(
        OperationStart {
            operation_type: "deploy_skill".to_string(),
            actor: "core".to_string(),
            entity_type: "skill".to_string(),
            entity_name: skill_name.to_string(),
            summary: format!("Deploy {skill_name}"),
            payload: serde_json::json!({"targetRoot": target_root}),
        },
        &managed_root,
        || deploy_skill_unlogged(skill_name, &managed_root, &target_root),
        |result| {
            (
                format!("Deployed {}", result.skill_name),
                serde_json::json!({
                    "targetRoot": result.target_root,
                    "targetPath": result.target_path,
                    "mode": result.mode
                }),
            )
        },
    )
}

fn deploy_skill_unlogged(
    skill_name: &str,
    managed_root: &Path,
    target_root: &Path,
) -> Result<Deployment> {
    validate_skill_name(skill_name)?;
    let paths = ensure_managed_layout(managed_root.to_path_buf())?;
    let managed_path = resolve_managed_skill_path(&paths, skill_name)?;
    let target_root = target_root.to_path_buf();
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
    let managed_root = managed_root.as_ref().to_path_buf();
    let target_root = expand_home(target_root.as_ref().to_path_buf());
    audited_operation(
        OperationStart {
            operation_type: "undeploy_skill".to_string(),
            actor: "core".to_string(),
            entity_type: "skill".to_string(),
            entity_name: skill_name.to_string(),
            summary: format!("Undeploy {skill_name}"),
            payload: serde_json::json!({"targetRoot": target_root}),
        },
        &managed_root,
        || undeploy_skill_unlogged(skill_name, &managed_root, &target_root),
        |result| {
            (
                format!("Undeployed {}", result.skill_name),
                serde_json::json!({
                    "targetRoot": result.target_root,
                    "targetPath": result.target_path,
                    "mode": result.mode
                }),
            )
        },
    )
}

pub fn preview_delete_skill(
    skill_name: &str,
    managed_root: impl AsRef<Path>,
) -> Result<DeleteSkillPreview> {
    let paths = ensure_managed_layout(managed_root.as_ref().to_path_buf())?;
    build_delete_skill_preview(&paths, skill_name)
}

pub fn delete_skill(
    request: DeleteSkillRequest,
    managed_root: impl AsRef<Path>,
) -> Result<DeleteSkillResult> {
    let managed_root = managed_root.as_ref().to_path_buf();
    let skill_name = request.skill_name.clone();
    let actor = request.actor.clone();
    audited_operation(
        OperationStart {
            operation_type: "delete_skill".to_string(),
            actor,
            entity_type: "skill".to_string(),
            entity_name: skill_name.clone(),
            summary: format!("Delete {skill_name} from SkillBox"),
            payload: serde_json::json!({"previewId": request.preview_id}),
        },
        &managed_root,
        || delete_skill_unlogged(&request, &managed_root),
        |result| {
            (
                format!("Deleted {} from SkillBox", result.skill_name),
                serde_json::json!({
                    "type": result.kind,
                    "managedPath": result.managed_path,
                    "backupPath": result.backup_path,
                    "removedDeployments": result.removed_deployments
                }),
            )
        },
    )
}

fn build_delete_skill_preview(
    paths: &ManagedPaths,
    skill_name: &str,
) -> Result<DeleteSkillPreview> {
    validate_skill_name(skill_name)?;
    let location = resolve_delete_skill_location(paths, skill_name)?;
    validate_delete_managed_location(paths, skill_name, &location)?;
    let managed_snapshot_hash = directory_snapshot_hash(&location.storage_path)?;
    let deployments = collect_delete_skill_deployments(paths, skill_name, &location)?;
    let mut blockers = Vec::new();

    let active_import_count = load_import_records(
        &paths.database_path,
        &ImportRecordFilter {
            skill_name: Some(skill_name.to_string()),
        },
    )?
    .into_iter()
    .filter(|record| record.status == ImportRecordStatus::Active)
    .count();
    if active_import_count > 0 {
        blockers.push(
            "Cannot delete while an active import record exists. Revert the import first."
                .to_string(),
        );
    }

    let reference_paths = delete_skill_reference_paths(paths, skill_name, &location);
    for deployment in &deployments {
        match fs::symlink_metadata(&deployment.target_path) {
            Ok(metadata) if !metadata.file_type().is_symlink() => blockers.push(format!(
                "Refusing to remove existing non-symlink target: {}",
                deployment.target_path.display()
            )),
            Ok(_) => match symlink_targets_any_path(&deployment.target_path, &reference_paths) {
                Ok(true) => {}
                Ok(false) => blockers.push(format!(
                    "Refusing to remove symlink pointing elsewhere: {}",
                    deployment.target_path.display()
                )),
                Err(error) => blockers.push(error),
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => blockers.push(error.to_string()),
        }
    }

    blockers.sort();
    blockers.dedup();
    let preview_seed = serde_json::json!({
        "skillName": skill_name,
        "kind": location.kind,
        "managedPath": location.storage_path,
        "managedSnapshotHash": managed_snapshot_hash,
        "deployments": deployments,
        "blockers": blockers
    });
    let preview_id =
        sha256(&serde_json::to_string(&preview_seed).map_err(|error| error.to_string())?);

    Ok(DeleteSkillPreview {
        preview_id,
        skill_name: skill_name.to_string(),
        kind: location.kind,
        managed_path: location.storage_path,
        deployments,
        can_delete: blockers.is_empty(),
        blockers,
    })
}

fn validate_delete_managed_location(
    paths: &ManagedPaths,
    skill_name: &str,
    location: &ManagedSkillKindLocation,
) -> Result<()> {
    match location.kind {
        SkillKind::User => {
            let expected = paths.user_skills_root.join(skill_name);
            let metadata = fs::symlink_metadata(&expected).map_err(|error| error.to_string())?;
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err(format!(
                    "Refusing to delete unsafe managed user path: {}",
                    expected.display()
                ));
            }
        }
        SkillKind::Remote => {
            let remote_root = paths.remote_skills_root.join(skill_name);
            let metadata = fs::symlink_metadata(&remote_root).map_err(|error| error.to_string())?;
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err(format!(
                    "Refusing to delete unsafe managed remote path: {}",
                    remote_root.display()
                ));
            }
        }
    }
    Ok(())
}

fn collect_delete_skill_deployments(
    paths: &ManagedPaths,
    skill_name: &str,
    location: &ManagedSkillKindLocation,
) -> Result<Vec<ManagedSkillDeployment>> {
    let mut deployments = load_deployments(&paths.database_path)?
        .remove(skill_name)
        .unwrap_or_default();
    let mut seen = deployments
        .iter()
        .map(|deployment| deployment.target_path.clone())
        .collect::<HashSet<_>>();
    let reference_paths = delete_skill_reference_paths(paths, skill_name, location);
    for workspace in load_workspaces(&paths.database_path)? {
        for target_path in workspace_symlink_paths_to_references(&workspace.path, &reference_paths)
        {
            if seen.insert(target_path.clone()) {
                deployments.push(ManagedSkillDeployment {
                    target_root: workspace.path.clone(),
                    target_path,
                    mode: "symlink".to_string(),
                });
            }
        }
    }
    deployments.sort_by(|left, right| left.target_path.cmp(&right.target_path));
    Ok(deployments)
}

fn workspace_symlink_paths_to_references(
    workspace_path: &Path,
    reference_paths: &[PathBuf],
) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(workspace_path) else {
        return Vec::new();
    };
    let mut paths = entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            fs::symlink_metadata(path)
                .map(|metadata| metadata.file_type().is_symlink())
                .unwrap_or(false)
                && symlink_targets_any_path(path, reference_paths).unwrap_or(false)
        })
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

fn delete_skill_unlogged(
    request: &DeleteSkillRequest,
    managed_root: &Path,
) -> Result<DeleteSkillResult> {
    if request.confirmed_skill_name != request.skill_name {
        return Err("Skill name confirmation does not match.".to_string());
    }
    let paths = ensure_managed_layout(managed_root.to_path_buf())?;
    let preview = build_delete_skill_preview(&paths, &request.skill_name)?;
    if preview.preview_id != request.preview_id {
        return Err("Skill deletion state changed. Review the deletion again.".to_string());
    }
    if !preview.can_delete {
        return Err(preview.blockers.join(" "));
    }

    let location = resolve_delete_skill_location(&paths, &request.skill_name)?;
    let deletion_root = location.storage_path.clone();
    let backup_path = unique_skill_deletion_backup_path(&paths, &request.skill_name);
    if let Some(parent) = backup_path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }

    let reference_paths = delete_skill_reference_paths(&paths, &request.skill_name, &location);
    let conflict_root = paths.root.join("backups").join("deletion-conflicts");
    let removed_paths = remove_skill_deployment_symlinks_with(
        &preview.deployments,
        &reference_paths,
        &conflict_root,
        &location.deployment_target_path,
        remove_owned_skill_symlink,
    )
    .map_err(|error| delete_skill_rollback_error(error, Ok(()), &backup_path))?;

    if let Err(error) = fs::rename(&deletion_root, &backup_path) {
        let rollback =
            restore_deleted_skill_symlinks(&location.deployment_target_path, &removed_paths);
        return Err(delete_skill_rollback_error(
            error.to_string(),
            rollback,
            &backup_path,
        ));
    }

    if let Err(error) =
        remove_deleted_skill_active_records(&paths.database_path, &request.skill_name)
    {
        let rollback = rollback_deleted_skill_files(
            &deletion_root,
            &backup_path,
            &location.deployment_target_path,
            &removed_paths,
        );
        return Err(delete_skill_rollback_error(error, rollback, &backup_path));
    }

    Ok(DeleteSkillResult {
        skill_name: request.skill_name.clone(),
        kind: location.kind,
        managed_path: location.storage_path,
        backup_path,
        removed_deployments: preview.deployments,
    })
}

pub(crate) fn remove_skill_deployment_symlinks_with<F>(
    deployments: &[ManagedSkillDeployment],
    reference_paths: &[PathBuf],
    conflict_root: &Path,
    deployment_target_path: &Path,
    mut remove: F,
) -> Result<Vec<PathBuf>>
where
    F: FnMut(&Path, &[PathBuf], &Path) -> Result<bool>,
{
    let mut removed_paths = Vec::new();
    for deployment in deployments {
        match remove(&deployment.target_path, reference_paths, conflict_root) {
            Ok(true) => removed_paths.push(deployment.target_path.clone()),
            Ok(false) => {}
            Err(error) => {
                return match restore_deleted_skill_symlinks(deployment_target_path, &removed_paths)
                {
                    Ok(()) => Err(error),
                    Err(rollback_error) => Err(format!(
                        "{error} Deployment rollback was incomplete: {rollback_error}"
                    )),
                };
            }
        }
    }
    Ok(removed_paths)
}

pub(crate) fn remove_owned_skill_symlink(
    target_path: &Path,
    reference_paths: &[PathBuf],
    conflict_root: &Path,
) -> Result<bool> {
    match fs::symlink_metadata(target_path) {
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.to_string()),
    }

    let quarantine_path = temporary_sibling_path(target_path, "delete-check")?;
    fs::rename(target_path, &quarantine_path).map_err(|error| error.to_string())?;
    let owned = fs::symlink_metadata(&quarantine_path)
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false)
        && symlink_targets_any_path(&quarantine_path, reference_paths).unwrap_or(false);

    if owned {
        return match fs::remove_file(&quarantine_path) {
            Ok(()) => Ok(true),
            Err(remove_error) if fs::symlink_metadata(target_path).is_err() => {
                match fs::rename(&quarantine_path, target_path) {
                    Ok(()) => Err(remove_error.to_string()),
                    Err(restore_error) => Err(format!(
                        "{remove_error}; failed to restore deployment from {}: {restore_error}",
                        quarantine_path.display()
                    )),
                }
            }
            Err(remove_error) => Err(preserve_quarantined_deletion_conflict(
                &quarantine_path,
                target_path,
                conflict_root,
                &remove_error.to_string(),
            )),
        };
    }

    if fs::symlink_metadata(target_path).is_ok() {
        return Err(preserve_quarantined_deletion_conflict(
            &quarantine_path,
            target_path,
            conflict_root,
            "Skill deletion state changed",
        ));
    }
    fs::rename(&quarantine_path, target_path).map_err(|error| {
        format!(
            "Skill deletion state changed at {} and the unexpected target could not be restored from {}: {error}",
            target_path.display(),
            quarantine_path.display()
        )
    })?;
    Err(format!(
        "Skill deletion state changed at {}. Review the deletion again.",
        target_path.display()
    ))
}

fn preserve_quarantined_deletion_conflict(
    quarantine_path: &Path,
    target_path: &Path,
    conflict_root: &Path,
    reason: &str,
) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let target_name = target_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("skill");
    let preserved_path = conflict_root.join(format!("{target_name}-{nanos}"));
    if let Err(error) = fs::create_dir_all(conflict_root) {
        return format!(
            "{reason} at {}. The unexpected target remains at {} because the recovery directory could not be created: {error}",
            target_path.display(),
            quarantine_path.display()
        );
    }
    match fs::rename(quarantine_path, &preserved_path) {
        Ok(()) => format!(
            "{reason} at {}. The unexpected target was preserved at {}.",
            target_path.display(),
            preserved_path.display()
        ),
        Err(error) => format!(
            "{reason} at {}. The unexpected target remains at {} because it could not be moved to {}: {error}",
            target_path.display(),
            quarantine_path.display(),
            preserved_path.display()
        ),
    }
}

fn directory_snapshot_hash(root: &Path) -> Result<String> {
    fn collect(root: &Path, current: &Path, entries: &mut Vec<serde_json::Value>) -> Result<()> {
        let mut children = fs::read_dir(current)
            .map_err(|error| error.to_string())?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;
        children.sort_by_key(|entry| entry.file_name());

        for entry in children {
            let path = entry.path();
            let relative = path
                .strip_prefix(root)
                .map_err(|error| error.to_string())?
                .to_string_lossy()
                .to_string();
            let file_type = entry.file_type().map_err(|error| error.to_string())?;
            if file_type.is_symlink() {
                entries.push(serde_json::json!({
                    "path": relative,
                    "type": "symlink",
                    "target": fs::read_link(&path).map_err(|error| error.to_string())?
                }));
            } else if file_type.is_dir() {
                entries.push(serde_json::json!({"path": relative, "type": "directory"}));
                collect(root, &path, entries)?;
            } else if file_type.is_file() {
                entries.push(serde_json::json!({
                    "path": relative,
                    "type": "file",
                    "hash": sha256_bytes(&fs::read(&path).map_err(|error| error.to_string())?)
                }));
            } else {
                entries.push(serde_json::json!({"path": relative, "type": "other"}));
            }
        }
        Ok(())
    }

    let mut entries = Vec::new();
    collect(root, root, &mut entries)?;
    Ok(sha256(
        &serde_json::to_string(&entries).map_err(|error| error.to_string())?,
    ))
}

fn rollback_deleted_skill_files(
    deletion_root: &Path,
    backup_path: &Path,
    deployment_target_path: &Path,
    removed_paths: &[PathBuf],
) -> Result<()> {
    let mut errors = Vec::new();
    if let Err(error) = fs::rename(backup_path, deletion_root) {
        errors.push(format!(
            "failed to restore managed skill from {}: {error}",
            backup_path.display()
        ));
    }
    if let Err(error) = restore_deleted_skill_symlinks(deployment_target_path, removed_paths) {
        errors.push(error);
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

fn restore_deleted_skill_symlinks(
    deployment_target_path: &Path,
    removed_paths: &[PathBuf],
) -> Result<()> {
    let mut errors = Vec::new();
    for target_path in removed_paths {
        if let Err(error) = symlink_dir(deployment_target_path, target_path) {
            errors.push(format!(
                "failed to restore {}: {error}",
                target_path.display()
            ));
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

fn delete_skill_rollback_error(
    primary_error: String,
    rollback: Result<()>,
    backup_path: &Path,
) -> String {
    match rollback {
        Ok(()) => primary_error,
        Err(rollback_error) => format!(
            "{primary_error} Rollback was incomplete: {rollback_error}. Inspect recovery path: {}",
            backup_path.display()
        ),
    }
}

fn unique_skill_deletion_backup_path(paths: &ManagedPaths, skill_name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    paths
        .root
        .join("backups")
        .join("deletions")
        .join(format!("{skill_name}-{nanos}"))
}

fn undeploy_skill_unlogged(
    skill_name: &str,
    managed_root: &Path,
    target_root: &Path,
) -> Result<Deployment> {
    validate_skill_name(skill_name)?;
    let paths = ensure_managed_layout(managed_root.to_path_buf())?;
    let managed_path = resolve_managed_skill_path(&paths, skill_name)?;
    let target_root = target_root.to_path_buf();
    let target_path = target_root.join(skill_name);
    let removes_active_import_source = load_import_records(
        &paths.database_path,
        &ImportRecordFilter {
            skill_name: Some(skill_name.to_string()),
        },
    )?
    .into_iter()
    .any(|record| {
        record.status == ImportRecordStatus::Active
            && [
                record.source_root.as_deref(),
                record.source_path.parent(),
                record.deployed_path.parent(),
            ]
            .into_iter()
            .flatten()
            .any(|candidate| paths_refer_to_same_location(candidate, &target_root))
    });
    if removes_active_import_source {
        return Err(
            "Cannot remove the active import source deployment. Revert the import first."
                .to_string(),
        );
    }
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

fn paths_refer_to_same_location(left: &Path, right: &Path) -> bool {
    match (fs::canonicalize(left), fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => {
            let absolute = |path: &Path| {
                let path = expand_home(path.to_path_buf());
                if path.is_absolute() {
                    path
                } else {
                    std::env::current_dir()
                        .map(|current| current.join(&path))
                        .unwrap_or(path)
                }
            };
            normalize_lexical_path(&absolute(left)) == normalize_lexical_path(&absolute(right))
        }
    }
}
