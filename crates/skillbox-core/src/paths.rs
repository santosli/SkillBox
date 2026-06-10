use crate::*;

pub fn default_managed_root() -> PathBuf {
    std::env::var_os("SKILLBOX_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(default_hidden_managed_root)
}

pub(crate) fn default_hidden_managed_root() -> PathBuf {
    home_dir().join(".skillbox")
}

pub(crate) fn legacy_managed_root() -> PathBuf {
    home_dir().join("SkillBox")
}

pub fn default_runtime_roots() -> Vec<PathBuf> {
    vec![
        home_dir().join(".codex/skills"),
        home_dir().join(".agents/skills"),
        home_dir().join(".claude/skills"),
    ]
}

pub fn global_runtime_roots() -> Vec<PathBuf> {
    runtime_roots_under(&home_dir())
}

pub(crate) fn runtime_roots_under(home: &Path) -> Vec<PathBuf> {
    let mut roots = vec![
        home.join(".codex/skills"),
        home.join(".agents/skills"),
        home.join(".claude/skills"),
    ];
    roots.extend(discover_runtime_roots_under(home));
    dedupe_runtime_roots(roots)
}

pub(crate) fn discover_runtime_roots_under(home: &Path) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    discover_runtime_roots(home, 0, 3, &mut roots);
    for base in runtime_root_search_bases(home) {
        discover_runtime_roots(&base, 0, 8, &mut roots);
    }
    dedupe_runtime_roots(roots)
}

pub(crate) fn runtime_root_search_bases(home: &Path) -> Vec<PathBuf> {
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

pub(crate) fn discover_runtime_roots(
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
    for runtime_parent in [".agents", ".codex", ".claude"] {
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

pub(crate) fn is_runtime_skill_root(path: &Path) -> bool {
    path.file_name().and_then(|name| name.to_str()) == Some("skills")
        && matches!(
            path.parent()
                .and_then(|parent| parent.file_name())
                .and_then(|name| name.to_str()),
            Some(".agents" | ".codex" | ".claude")
        )
}

pub(crate) fn should_skip_runtime_root_search(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    if matches!(name, ".agents" | ".codex" | ".claude") {
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

pub(crate) fn dedupe_runtime_roots(roots: Vec<PathBuf>) -> Vec<PathBuf> {
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
    let root = expand_home(root.into());
    maybe_link_legacy_default_managed_root(&root)?;
    let paths = managed_paths(root);
    fs::create_dir_all(&paths.user_skills_root).map_err(|error| error.to_string())?;
    ensure_default_user_skills_gitignore(&paths.user_skills_root)?;
    fs::create_dir_all(&paths.remote_skills_root).map_err(|error| error.to_string())?;
    init_database(&paths.database_path)?;
    Ok(paths)
}

pub(crate) fn ensure_default_user_skills_gitignore(user_skills_root: &Path) -> Result<()> {
    let gitignore_path = user_skills_root.join(".gitignore");
    if gitignore_path.exists() {
        return Ok(());
    }
    fs::write(gitignore_path, DEFAULT_USER_SKILLS_GITIGNORE).map_err(|error| error.to_string())
}

pub(crate) fn maybe_link_legacy_default_managed_root(root: &Path) -> Result<()> {
    if std::env::var_os("SKILLBOX_HOME").is_some() || root != default_hidden_managed_root() {
        return Ok(());
    }
    link_legacy_managed_root_if_needed(root, &legacy_managed_root()).map(|_| ())
}

pub(crate) fn link_legacy_managed_root_if_needed(
    hidden_root: &Path,
    legacy_root: &Path,
) -> Result<bool> {
    if hidden_root == legacy_root || !managed_root_has_content(legacy_root)? {
        return Ok(false);
    }

    match fs::symlink_metadata(hidden_root) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() {
                return Ok(false);
            }
            if !is_empty_managed_stub(hidden_root)? {
                return Ok(false);
            }
            let backup_path = next_empty_root_backup_path(hidden_root);
            fs::rename(hidden_root, &backup_path).map_err(|error| {
                format!(
                    "Failed to back up empty managed root {} to {}: {error}",
                    hidden_root.display(),
                    backup_path.display()
                )
            })?;
            if let Err(error) = symlink_dir(legacy_root, hidden_root) {
                let _ = fs::rename(&backup_path, hidden_root);
                return Err(format!(
                    "Failed to link {} to legacy SkillBox root {}: {error}",
                    hidden_root.display(),
                    legacy_root.display()
                ));
            }
            Ok(true)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            if let Some(parent) = hidden_root.parent() {
                fs::create_dir_all(parent).map_err(|error| error.to_string())?;
            }
            symlink_dir(legacy_root, hidden_root).map_err(|error| {
                format!(
                    "Failed to link {} to legacy SkillBox root {}: {error}",
                    hidden_root.display(),
                    legacy_root.display()
                )
            })?;
            Ok(true)
        }
        Err(error) => Err(error.to_string()),
    }
}

pub(crate) fn managed_root_has_content(root: &Path) -> Result<bool> {
    if !root.is_dir() {
        return Ok(false);
    }
    Ok(directory_has_entries(&root.join("user-skills"))?
        || directory_has_entries(&root.join("remote-skills"))?
        || directory_has_entries(&root.join("backups"))?)
}

pub(crate) fn is_empty_managed_stub(root: &Path) -> Result<bool> {
    if !root.is_dir() {
        return Ok(false);
    }
    for entry in fs::read_dir(root).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        let file_type = entry.file_type().map_err(|error| error.to_string())?;
        match name.as_str() {
            "user-skills" | "remote-skills" => {
                if !file_type.is_dir() || directory_has_entries(&path)? {
                    return Ok(false);
                }
            }
            "skillbox.sqlite" => {
                if !file_type.is_file() {
                    return Ok(false);
                }
            }
            _ => return Ok(false),
        }
    }
    Ok(true)
}

pub(crate) fn directory_has_entries(path: &Path) -> Result<bool> {
    if !path.exists() {
        return Ok(false);
    }
    if !path.is_dir() {
        return Ok(true);
    }
    Ok(fs::read_dir(path)
        .map_err(|error| error.to_string())?
        .next()
        .is_some())
}

pub(crate) fn next_empty_root_backup_path(root: &Path) -> PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_millis();
    let name = root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("skillbox");
    for attempt in 0..100 {
        let suffix = if attempt == 0 {
            format!("{name}.empty-backup-{timestamp}")
        } else {
            format!("{name}.empty-backup-{timestamp}-{attempt}")
        };
        let candidate = root.with_file_name(suffix);
        if !candidate.exists() {
            return candidate;
        }
    }
    root.with_file_name(format!("{name}.empty-backup-{timestamp}-fallback"))
}
