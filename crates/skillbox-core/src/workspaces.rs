use crate::*;

pub fn list_workspaces(managed_root: impl AsRef<Path>) -> Result<Vec<Workspace>> {
    let paths = ensure_managed_layout(managed_root.as_ref().to_path_buf())?;
    load_workspaces_with_visible_usage(&paths)
}

pub fn scan_workspaces(managed_root: impl AsRef<Path>) -> Result<WorkspaceScanResult> {
    scan_workspaces_under(&home_dir(), managed_root)
}

pub(crate) fn scan_workspaces_under(
    home: &Path,
    managed_root: impl AsRef<Path>,
) -> Result<WorkspaceScanResult> {
    let paths = ensure_managed_layout(managed_root.as_ref().to_path_buf())?;
    let roots = runtime_roots_under(home)
        .into_iter()
        .filter(|root| workspace_root_is_readable(root))
        .collect::<Vec<_>>();
    let mut active_auto_workspace_paths = HashSet::new();
    let mut scanned_count = 0;
    let mut error_count = 0;

    for root in roots {
        let kind = infer_workspace_kind(&root, home);
        let workspace = upsert_workspace(&paths, &root, kind, WorkspaceSource::Auto)?;
        active_auto_workspace_paths.insert(workspace.canonical_path);
        scanned_count += 1;
        error_count += workspace.last_scan_error_count;
    }
    prune_stale_auto_workspaces(&paths.database_path, &active_auto_workspace_paths)?;

    Ok(WorkspaceScanResult {
        workspaces: load_workspaces_with_visible_usage(&paths)?,
        scanned_count,
        error_count,
    })
}

pub fn add_workspace(
    request: WorkspaceAddRequest,
    managed_root: impl AsRef<Path>,
) -> Result<Workspace> {
    let paths = ensure_managed_layout(managed_root.as_ref().to_path_buf())?;
    let workspace_path = expand_home(request.path);

    if !workspace_path.exists() {
        return Err(format!(
            "Workspace path does not exist: {}",
            workspace_path.display()
        ));
    }
    if !workspace_path.is_dir() {
        return Err(format!(
            "Workspace path is not a directory: {}",
            workspace_path.display()
        ));
    }

    upsert_workspace(
        &paths,
        &workspace_path,
        request.kind,
        WorkspaceSource::Manual,
    )
}

pub fn forget_workspace(
    path: impl AsRef<Path>,
    managed_root: impl AsRef<Path>,
) -> Result<Vec<Workspace>> {
    let paths = ensure_managed_layout(managed_root.as_ref().to_path_buf())?;
    let workspace_path = expand_home(path.as_ref().to_path_buf());
    let canonical_path = fs::canonicalize(&workspace_path).map_err(|error| {
        format!(
            "Workspace path cannot be resolved: {} ({error})",
            workspace_path.display()
        )
    })?;
    let existing = load_workspace_by_canonical_path(&paths.database_path, &canonical_path)?
        .ok_or_else(|| format!("Workspace is not registered: {}", workspace_path.display()))?;

    if existing.source != WorkspaceSource::Manual {
        return Err("Only manually added workspaces can be forgotten.".to_string());
    }

    let connection = open_database(&paths.database_path).map_err(|error| error.to_string())?;
    connection
        .execute(
            "DELETE FROM workspaces WHERE canonical_path = ?1 AND source = 'manual'",
            params![canonical_path.to_string_lossy()],
        )
        .map_err(|error| error.to_string())?;

    load_workspaces_with_visible_usage(&paths)
}

pub(crate) fn record_scanned_workspaces(paths: &ManagedPaths, roots: &[PathBuf]) -> Result<()> {
    let home = home_dir();
    for root in roots {
        if workspace_root_is_readable(root) {
            upsert_workspace(
                paths,
                root,
                infer_workspace_kind(root, &home),
                WorkspaceSource::Auto,
            )?;
        }
    }
    Ok(())
}

pub(crate) fn upsert_workspace(
    paths: &ManagedPaths,
    path: &Path,
    kind: WorkspaceKind,
    source: WorkspaceSource,
) -> Result<Workspace> {
    let path = expand_home(path.to_path_buf());
    let canonical_path = fs::canonicalize(&path).map_err(|error| error.to_string())?;
    let stats = scan_workspace_root(&path, paths)?;
    let agent_id = workspace_agent_id(&path);
    let display_name = workspace_display_name(&path, agent_id.as_deref(), kind);
    let connection = open_database(&paths.database_path).map_err(|error| error.to_string())?;

    connection
        .execute(
            "
            INSERT INTO workspaces (
              canonical_path,
              path,
              kind,
              source,
              agent_id,
              display_name,
              skill_count,
              imported_skill_count,
              last_scan_error_count,
              last_scan_error,
              last_scanned_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, CURRENT_TIMESTAMP)
            ON CONFLICT(canonical_path) DO UPDATE SET
              path = excluded.path,
              kind = CASE
                WHEN workspaces.source = 'manual' AND excluded.source = 'auto'
                THEN workspaces.kind
                ELSE excluded.kind
              END,
              source = CASE
                WHEN workspaces.source = 'manual' AND excluded.source = 'auto'
                THEN workspaces.source
                ELSE excluded.source
              END,
              agent_id = excluded.agent_id,
              display_name = excluded.display_name,
              skill_count = excluded.skill_count,
              imported_skill_count = excluded.imported_skill_count,
              last_scan_error_count = excluded.last_scan_error_count,
              last_scan_error = excluded.last_scan_error,
              last_scanned_at = CURRENT_TIMESTAMP,
              updated_at = CURRENT_TIMESTAMP
            ",
            params![
                canonical_path.to_string_lossy(),
                path.to_string_lossy(),
                kind.as_str(),
                source.as_str(),
                agent_id,
                display_name,
                stats.skill_count as i64,
                stats.imported_skill_count as i64,
                stats.error_count as i64,
                stats.last_error,
            ],
        )
        .map_err(|error| error.to_string())?;

    load_workspace_by_canonical_path_with_visible_usage(paths, &canonical_path)?
        .ok_or_else(|| format!("Workspace was not saved: {}", path.display()))
}

pub(crate) fn prune_stale_auto_workspaces(
    database_path: &Path,
    active_canonical_paths: &HashSet<PathBuf>,
) -> Result<()> {
    let connection = open_database(database_path).map_err(|error| error.to_string())?;
    let mut statement = connection
        .prepare("SELECT canonical_path FROM workspaces WHERE source = 'auto'")
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|error| error.to_string())?;
    let mut stale_paths = Vec::new();

    for row in rows {
        let canonical_path = row.map_err(|error| error.to_string())?;
        if !active_canonical_paths.contains(&PathBuf::from(&canonical_path)) {
            stale_paths.push(canonical_path);
        }
    }

    for canonical_path in stale_paths {
        connection
            .execute(
                "DELETE FROM workspaces WHERE canonical_path = ?1 AND source = 'auto'",
                params![canonical_path],
            )
            .map_err(|error| error.to_string())?;
    }

    Ok(())
}

pub(crate) fn load_workspaces(database_path: &Path) -> Result<Vec<Workspace>> {
    let usage_by_runtime = load_usage_by_runtime(database_path)?;
    let connection = open_database(database_path).map_err(|error| error.to_string())?;
    let mut statement = connection
        .prepare(
            "
            SELECT
              canonical_path,
              path,
              kind,
              source,
              agent_id,
              display_name,
              skill_count,
              imported_skill_count,
              last_scan_error_count,
              last_scan_error,
              last_scanned_at
            FROM workspaces
            ORDER BY kind, display_name, path
            ",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], workspace_from_row)
        .map_err(|error| error.to_string())?;
    let mut workspaces = Vec::new();

    for row in rows {
        let mut workspace = row.map_err(|error| error.to_string())?;
        if let Some(usage) = usage_by_runtime.get(&usage_runtime_key(&workspace.canonical_path)) {
            workspace.usage_count = usage.usage_count;
        }
        workspaces.push(workspace);
    }

    Ok(workspaces)
}

pub(crate) fn load_workspaces_with_visible_usage(paths: &ManagedPaths) -> Result<Vec<Workspace>> {
    let mut workspaces = load_workspaces(&paths.database_path)?;
    apply_visible_workspace_usage(paths, &mut workspaces)?;
    Ok(workspaces)
}

pub(crate) fn load_workspace_by_canonical_path(
    database_path: &Path,
    canonical_path: &Path,
) -> Result<Option<Workspace>> {
    let usage_by_runtime = load_usage_by_runtime(database_path)?;
    let connection = open_database(database_path).map_err(|error| error.to_string())?;
    let workspace = connection
        .query_row(
            "
            SELECT
              canonical_path,
              path,
              kind,
              source,
              agent_id,
              display_name,
              skill_count,
              imported_skill_count,
              last_scan_error_count,
              last_scan_error,
              last_scanned_at
            FROM workspaces
            WHERE canonical_path = ?1
            ",
            params![canonical_path.to_string_lossy()],
            workspace_from_row,
        )
        .optional()
        .map_err(|error| error.to_string())?;

    Ok(workspace.map(|mut workspace| {
        if let Some(usage) = usage_by_runtime.get(&usage_runtime_key(&workspace.canonical_path)) {
            workspace.usage_count = usage.usage_count;
        }
        workspace
    }))
}

pub(crate) fn load_workspace_by_canonical_path_with_visible_usage(
    paths: &ManagedPaths,
    canonical_path: &Path,
) -> Result<Option<Workspace>> {
    let mut workspace = load_workspace_by_canonical_path(&paths.database_path, canonical_path)?;
    if let Some(workspace) = workspace.as_mut() {
        let usage_by_skill_runtime = load_usage_by_skill_runtime(&paths.database_path)?;
        if let Ok(usage_count) =
            workspace_visible_usage_count(&workspace.path, paths, &usage_by_skill_runtime)
        {
            workspace.usage_count = usage_count;
        }
    }
    Ok(workspace)
}

pub(crate) fn apply_visible_workspace_usage(
    paths: &ManagedPaths,
    workspaces: &mut [Workspace],
) -> Result<()> {
    let usage_by_skill_runtime = load_usage_by_skill_runtime(&paths.database_path)?;

    for workspace in workspaces {
        if let Ok(usage_count) =
            workspace_visible_usage_count(&workspace.path, paths, &usage_by_skill_runtime)
        {
            workspace.usage_count = usage_count;
        }
    }

    Ok(())
}

pub(crate) fn workspace_visible_usage_count(
    root: &Path,
    paths: &ManagedPaths,
    usage_by_skill_runtime: &HashMap<(String, String), UsageSummary>,
) -> Result<usize> {
    let scan = scan_skill_roots_for_import(&[root.to_path_buf()], paths)?;
    let mut seen = HashSet::new();
    let mut usage_count = 0;

    for skill in scan.skills {
        for runtime_key in skill_usage_runtime_keys(&skill, paths) {
            if !seen.insert((skill.name.clone(), runtime_key.clone())) {
                continue;
            }
            if let Some(usage) = usage_by_skill_runtime.get(&(skill.name.clone(), runtime_key)) {
                usage_count += usage.usage_count;
            }
        }
    }

    Ok(usage_count)
}

pub(crate) fn workspace_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Workspace> {
    let kind_raw: String = row.get(2)?;
    let source_raw: String = row.get(3)?;
    let skill_count: i64 = row.get(6)?;
    let imported_skill_count: i64 = row.get(7)?;
    let last_scan_error_count: i64 = row.get(8)?;

    Ok(Workspace {
        canonical_path: PathBuf::from(row.get::<_, String>(0)?),
        path: PathBuf::from(row.get::<_, String>(1)?),
        kind: workspace_kind_from_str(&kind_raw)
            .map_err(rusqlite::Error::ToSqlConversionFailure)?,
        source: workspace_source_from_str(&source_raw)
            .map_err(rusqlite::Error::ToSqlConversionFailure)?,
        agent_id: row.get(4)?,
        display_name: row.get(5)?,
        skill_count: usize::try_from(skill_count.max(0)).unwrap_or_default(),
        imported_skill_count: usize::try_from(imported_skill_count.max(0)).unwrap_or_default(),
        usage_count: 0,
        last_scan_error_count: usize::try_from(last_scan_error_count.max(0)).unwrap_or_default(),
        last_scan_error: row.get(9)?,
        last_scanned_at: row.get(10)?,
    })
}

pub(crate) fn workspace_kind_from_str(
    value: &str,
) -> std::result::Result<WorkspaceKind, Box<dyn std::error::Error + Send + Sync>> {
    match value {
        "global" => Ok(WorkspaceKind::Global),
        "user" => Ok(WorkspaceKind::User),
        other => Err(format!("Invalid workspace kind: {other}").into()),
    }
}

pub(crate) fn workspace_source_from_str(
    value: &str,
) -> std::result::Result<WorkspaceSource, Box<dyn std::error::Error + Send + Sync>> {
    match value {
        "auto" => Ok(WorkspaceSource::Auto),
        "manual" => Ok(WorkspaceSource::Manual),
        other => Err(format!("Invalid workspace source: {other}").into()),
    }
}

pub(crate) struct WorkspaceScanStats {
    skill_count: usize,
    imported_skill_count: usize,
    error_count: usize,
    last_error: Option<String>,
}

pub(crate) fn scan_workspace_root(root: &Path, paths: &ManagedPaths) -> Result<WorkspaceScanStats> {
    let scan = scan_skill_roots_for_import(&[root.to_path_buf()], paths)?;
    let imported_hashes = imported_skill_hashes(paths)?;
    let imported_skill_count = scan
        .skills
        .iter()
        .filter(|skill| skill_is_imported(skill, &imported_hashes, paths))
        .count();

    Ok(WorkspaceScanStats {
        skill_count: scan.skills.len(),
        imported_skill_count,
        error_count: scan.errors.len(),
        last_error: scan.errors.first().map(format_scan_error),
    })
}

pub(crate) fn imported_skill_hashes(paths: &ManagedPaths) -> Result<HashSet<String>> {
    let managed_scan = scan_skill_roots(&[
        paths.user_skills_root.clone(),
        paths.remote_skills_root.clone(),
    ])?;
    Ok(managed_scan
        .skills
        .iter()
        .map(|skill| skill.content_hash.clone())
        .collect())
}

pub(crate) fn skill_is_imported(
    skill: &Skill,
    imported_hashes: &HashSet<String>,
    paths: &ManagedPaths,
) -> bool {
    imported_hashes.contains(&skill.content_hash) || is_under_path(&skill.real_path, &paths.root)
}

pub(crate) fn scan_skill_roots_for_import(
    roots: &[PathBuf],
    paths: &ManagedPaths,
) -> Result<ScanResult> {
    let mut scan = scan_skill_roots(roots)?;
    let mut seen_paths: HashSet<PathBuf> =
        scan.skills.iter().map(|skill| skill.path.clone()).collect();
    let trusted_symlink_roots = trusted_skill_symlink_roots(roots, paths);

    for root in scan.roots.clone() {
        if !root.exists() {
            continue;
        }

        let mut symlink_dirs = Vec::new();
        if let Err(error) =
            find_trusted_skill_symlink_dirs(&root, 0, 3, &trusted_symlink_roots, &mut symlink_dirs)
        {
            scan.errors.push(ScanError {
                root,
                path: None,
                error,
            });
            continue;
        }

        for skill_dir in symlink_dirs {
            if !seen_paths.insert(skill_dir.clone()) {
                continue;
            }

            match read_skill(&skill_dir) {
                Ok(mut skill) => {
                    skill.source_root = Some(root.clone());
                    skill.is_symlink = true;
                    scan.skills.push(skill);
                }
                Err(error) => scan.errors.push(ScanError {
                    root: root.clone(),
                    path: Some(skill_dir),
                    error,
                }),
            }
        }
    }

    scan.skills
        .sort_by(|left, right| left.name.cmp(&right.name));
    Ok(scan)
}

pub(crate) fn trusted_skill_symlink_roots(roots: &[PathBuf], paths: &ManagedPaths) -> Vec<PathBuf> {
    let mut trusted_roots = vec![paths.root.clone()];

    for root in roots {
        trusted_roots.push(root.clone());
        if let Some(base) = runtime_workspace_base(root) {
            for runtime_parent in [".agents", ".codex", ".claude"] {
                let runtime_root = base.join(runtime_parent).join("skills");
                if runtime_root.is_dir() {
                    trusted_roots.push(runtime_root);
                }
            }
        }
    }

    dedupe_runtime_roots(trusted_roots)
}

pub(crate) fn runtime_workspace_base(root: &Path) -> Option<PathBuf> {
    let root_name = root.file_name()?.to_str()?;
    let parent = root.parent()?;
    let parent_name = parent.file_name()?.to_str()?;

    if root_name == "skills" && matches!(parent_name, ".agents" | ".codex" | ".claude") {
        parent.parent().map(Path::to_path_buf)
    } else {
        None
    }
}

pub(crate) fn find_trusted_skill_symlink_dirs(
    current: &Path,
    depth: usize,
    max_depth: usize,
    trusted_roots: &[PathBuf],
    found: &mut Vec<PathBuf>,
) -> Result<()> {
    if depth > max_depth {
        return Ok(());
    }

    let current_metadata = fs::symlink_metadata(current).map_err(|error| error.to_string())?;
    if current_metadata.file_type().is_symlink() {
        if current.join("SKILL.md").exists()
            && trusted_roots
                .iter()
                .any(|trusted_root| is_under_path(current, trusted_root))
        {
            found.push(current.to_path_buf());
        }
        return Ok(());
    }

    if current.join("SKILL.md").exists() {
        return Ok(());
    }

    for entry in fs::read_dir(current).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        if file_name.starts_with('.') && file_name != ".system" {
            continue;
        }

        let file_type = entry.file_type().map_err(|error| error.to_string())?;
        if file_type.is_dir() || file_type.is_symlink() {
            find_trusted_skill_symlink_dirs(&path, depth + 1, max_depth, trusted_roots, found)?;
        }
    }

    Ok(())
}

pub(crate) fn format_scan_error(error: &ScanError) -> String {
    match &error.path {
        Some(path) => format!("{}: {}", path.display(), error.error),
        None => format!("{}: {}", error.root.display(), error.error),
    }
}

pub(crate) fn workspace_root_is_readable(root: &Path) -> bool {
    root.is_dir() && fs::read_dir(root).is_ok()
}

pub(crate) fn infer_workspace_kind(root: &Path, home: &Path) -> WorkspaceKind {
    let canonical_root = fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());

    if direct_global_workspace_roots(home)
        .into_iter()
        .filter(|candidate| candidate.exists())
        .map(|candidate| fs::canonicalize(&candidate).unwrap_or(candidate))
        .any(|candidate| candidate == canonical_root)
    {
        WorkspaceKind::Global
    } else {
        WorkspaceKind::User
    }
}

pub(crate) fn direct_global_workspace_roots(home: &Path) -> Vec<PathBuf> {
    vec![
        home.join(".codex/skills"),
        home.join(".agents/skills"),
        home.join(".claude/skills"),
    ]
}

pub(crate) fn workspace_agent_id(path: &Path) -> Option<String> {
    match path
        .parent()
        .and_then(|parent| parent.file_name())
        .and_then(|name| name.to_str())
    {
        Some(".codex") => Some("codex".to_string()),
        Some(".agents") => Some("agents".to_string()),
        Some(".claude") => Some("claude".to_string()),
        _ => None,
    }
}

pub(crate) fn workspace_display_name(
    path: &Path,
    agent_id: Option<&str>,
    kind: WorkspaceKind,
) -> String {
    if kind == WorkspaceKind::User {
        if let Some(project_name) = workspace_project_name(path) {
            return project_name;
        }
    }

    workspace_agent_label(agent_id)
        .or_else(|| {
            path.file_name()
                .and_then(|name| name.to_str())
                .map(str::to_string)
        })
        .unwrap_or_else(|| "Local".to_string())
}

pub(crate) fn workspace_agent_label(agent_id: Option<&str>) -> Option<String> {
    let label = match agent_id {
        Some("codex") => "Codex",
        Some("agents") => "Agents",
        Some("claude") => "Claude Code",
        _ => return None,
    };

    Some(label.to_string())
}

pub(crate) fn workspace_project_name(path: &Path) -> Option<String> {
    let root_name = path.file_name()?.to_str()?;
    let parent = path.parent()?;
    let parent_name = parent
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");

    if root_name == "skills" && matches!(parent_name, ".codex" | ".agents" | ".claude") {
        parent
            .parent()
            .and_then(|project| project.file_name())
            .and_then(|name| name.to_str())
            .map(str::to_string)
    } else if root_name == "skills" {
        parent
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_string)
    } else {
        Some(root_name.to_string())
    }
}
