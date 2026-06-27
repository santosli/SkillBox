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

pub fn change_skill_kind(
    skill_name: &str,
    kind: SkillKind,
    managed_root: impl AsRef<Path>,
) -> Result<ImportedSkill> {
    validate_skill_name(skill_name)?;
    let paths = ensure_managed_layout(managed_root.as_ref().to_path_buf())?;
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
    Ok(expected_paths.iter().any(|expected| *expected == target))
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
