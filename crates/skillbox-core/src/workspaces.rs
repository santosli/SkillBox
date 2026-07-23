use crate::*;
use std::os::unix::fs::MetadataExt;

const PROJECT_WORKSPACE_ROOTS: [(&str, &str, &str); 3] = [
    (".agents/skills", "agents", "Agents"),
    (".codex/skills", "codex", "Codex"),
    (".claude/skills", "claude", "Claude Code"),
];

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
    let managed_root = managed_root.as_ref().to_path_buf();
    let workspace_path = expand_home(request.path.clone());
    audited_operation(
        OperationStart {
            operation_type: "add_workspace".to_string(),
            actor: "core".to_string(),
            entity_type: "workspace".to_string(),
            entity_name: workspace_path.to_string_lossy().to_string(),
            summary: "Add workspace".to_string(),
            payload: serde_json::json!({
                "path": workspace_path,
                "kind": request.kind.as_str()
            }),
        },
        &managed_root,
        || add_workspace_unlogged(request, &managed_root),
        |workspace| {
            (
                format!("Added workspace {}", workspace.display_name),
                serde_json::json!({
                    "path": workspace.path,
                    "kind": workspace.kind.as_str(),
                    "source": workspace.source.as_str()
                }),
            )
        },
    )
}

pub fn preview_workspace_setup(
    request: WorkspaceSetupPreviewRequest,
    managed_root: impl AsRef<Path>,
) -> Result<WorkspaceSetupPreview> {
    let selected_path = expand_home(request.selected_path);
    let selected_metadata = fs::symlink_metadata(&selected_path).map_err(|error| {
        format!(
            "Project or skills folder cannot be read: {} ({error})",
            selected_path.display()
        )
    })?;
    let exact_root = request.kind == WorkspaceKind::Global
        || selected_path.file_name().and_then(|name| name.to_str()) == Some("skills");
    if selected_metadata.file_type().is_symlink() && !exact_root {
        return Err(format!(
            "Project directory cannot be a symlink: {}",
            selected_path.display()
        ));
    }
    let selected_path = fs::canonicalize(&selected_path).map_err(|error| error.to_string())?;
    let metadata = fs::metadata(&selected_path).map_err(|error| error.to_string())?;
    if !metadata.is_dir() {
        return Err(format!(
            "Project or skills folder is not a directory: {}",
            selected_path.display()
        ));
    }
    if metadata.permissions().mode() & 0o444 == 0 || metadata.permissions().mode() & 0o111 == 0 {
        return Err(format!(
            "Project or skills folder is not readable: {}",
            selected_path.display()
        ));
    }
    fs::read_dir(&selected_path).map_err(|error| {
        format!(
            "Project or skills folder is not readable: {} ({error})",
            selected_path.display()
        )
    })?;
    let (mode, roots) = if exact_root {
        (
            WorkspaceSetupMode::ExistingRoot,
            vec![workspace_setup_exact_root(&selected_path)],
        )
    } else {
        validate_workspace_setup_project(&selected_path, managed_root.as_ref())?;
        let roots = project_workspace_root_options(&selected_path)?;
        let mode = if roots.iter().any(|root| root.exists) {
            WorkspaceSetupMode::ProjectWithRoots
        } else {
            WorkspaceSetupMode::ProjectWithoutRoots
        };
        (mode, roots)
    };

    let preview_id = workspace_setup_preview_id(&selected_path, request.kind, mode, &roots);
    Ok(WorkspaceSetupPreview {
        preview_id,
        selected_path,
        kind: request.kind,
        mode,
        roots,
    })
}

pub fn apply_workspace_setup(
    request: WorkspaceSetupApplyRequest,
    managed_root: impl AsRef<Path>,
) -> Result<WorkspaceSetupApplyResult> {
    let managed_root = managed_root.as_ref().to_path_buf();
    let selected_root = request.selected_root.clone();
    audited_operation(
        OperationStart {
            operation_type: "add_workspace".to_string(),
            actor: "core".to_string(),
            entity_type: "workspace".to_string(),
            entity_name: selected_root.to_string_lossy().to_string(),
            summary: "Set up workspace".to_string(),
            payload: serde_json::json!({
                "selectedPath": request.selected_path,
                "selectedRoot": request.selected_root,
                "kind": request.kind.as_str(),
                "createMissing": request.create_missing,
                "previewId": request.preview_id
            }),
        },
        &managed_root,
        || apply_workspace_setup_unlogged(request, &managed_root),
        |result| {
            (
                format!("Added workspace {}", result.workspace.display_name),
                serde_json::json!({
                    "path": result.workspace.path,
                    "kind": result.workspace.kind.as_str(),
                    "source": result.workspace.source.as_str(),
                    "createdPath": result.created_path
                }),
            )
        },
    )
}

fn apply_workspace_setup_unlogged(
    request: WorkspaceSetupApplyRequest,
    managed_root: &Path,
) -> Result<WorkspaceSetupApplyResult> {
    apply_workspace_setup_with_register(request, managed_root, |workspace_request| {
        add_workspace_unlogged(workspace_request, managed_root)
    })
}

pub(crate) fn apply_workspace_setup_with_register<F>(
    request: WorkspaceSetupApplyRequest,
    managed_root: &Path,
    register: F,
) -> Result<WorkspaceSetupApplyResult>
where
    F: FnOnce(WorkspaceAddRequest) -> Result<Workspace>,
{
    let preview = preview_workspace_setup(
        WorkspaceSetupPreviewRequest {
            selected_path: request.selected_path.clone(),
            kind: request.kind,
        },
        managed_root,
    )?;
    if preview.preview_id != request.preview_id {
        return Err(
            "Workspace setup preview is stale. Preview the folder again before continuing."
                .to_string(),
        );
    }

    let selected_root = expand_home(request.selected_root);
    let selected_option = preview
        .roots
        .iter()
        .find(|option| option.path == selected_root)
        .ok_or_else(|| {
            "Selected skills folder is not part of this workspace preview.".to_string()
        })?;
    if request.create_missing == selected_option.exists {
        return Err(
            "Workspace setup selection changed. Preview the folder again before continuing."
                .to_string(),
        );
    }
    if request.kind == WorkspaceKind::Global && request.create_missing {
        return Err("Global skills folders must already exist.".to_string());
    }

    let mut created = Vec::new();
    let workspace_path = if request.create_missing {
        create_project_workspace_root(&preview.selected_path, selected_option, &mut created)?
    } else {
        validate_existing_workspace_root(&preview.selected_path, &selected_root, preview.mode)?
    };

    let result = register(WorkspaceAddRequest {
        path: workspace_path.clone(),
        kind: request.kind,
    });
    match result {
        Ok(workspace) => Ok(WorkspaceSetupApplyResult {
            workspace,
            created_path: request.create_missing.then_some(workspace_path),
        }),
        Err(error) => {
            cleanup_created_workspace_dirs(&created);
            Err(error)
        }
    }
}

fn workspace_setup_exact_root(path: &Path) -> WorkspaceSetupRootOption {
    let agent_id = workspace_agent_id(path).unwrap_or_else(|| "custom".to_string());
    let label =
        workspace_agent_label(Some(&agent_id)).unwrap_or_else(|| "Skills folder".to_string());
    WorkspaceSetupRootOption {
        path: path.to_path_buf(),
        relative_path: path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("skills")
            .to_string(),
        agent_id,
        label,
        exists: true,
        recommended: true,
    }
}

fn project_workspace_root_options(project: &Path) -> Result<Vec<WorkspaceSetupRootOption>> {
    let mut roots = Vec::new();
    for (index, (relative_path, agent_id, label)) in PROJECT_WORKSPACE_ROOTS.iter().enumerate() {
        let candidate = project.join(relative_path);
        validate_project_workspace_root_chain(project, Path::new(relative_path))?;
        let exists = match fs::symlink_metadata(&candidate) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() {
                    return Err(format!(
                        "Supported skills folder cannot be a symlink: {}",
                        candidate.display()
                    ));
                }
                if !metadata.is_dir() {
                    return Err(format!(
                        "Supported skills folder is not a directory: {}",
                        candidate.display()
                    ));
                }
                if metadata.permissions().mode() & 0o444 == 0
                    || metadata.permissions().mode() & 0o111 == 0
                {
                    return Err(format!(
                        "Supported skills folder is not readable: {}",
                        candidate.display()
                    ));
                }
                let canonical = fs::canonicalize(&candidate).map_err(|error| error.to_string())?;
                if !canonical.starts_with(project) {
                    return Err(format!(
                        "Supported skills folder escapes the selected project: {}",
                        candidate.display()
                    ));
                }
                fs::read_dir(&candidate).map_err(|error| {
                    format!(
                        "Supported skills folder is not readable: {} ({error})",
                        candidate.display()
                    )
                })?;
                true
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
            Err(error) => return Err(error.to_string()),
        };
        roots.push(WorkspaceSetupRootOption {
            path: candidate,
            relative_path: (*relative_path).to_string(),
            agent_id: (*agent_id).to_string(),
            label: (*label).to_string(),
            exists,
            recommended: index == 0,
        });
    }

    let recommended_index = roots.iter().position(|root| root.exists).or_else(|| {
        roots.iter().position(|root| {
            let marker = root.relative_path.split('/').next().unwrap_or_default();
            project.join(marker).is_dir()
        })
    });
    if let Some(recommended_index) = recommended_index {
        for (index, root) in roots.iter_mut().enumerate() {
            root.recommended = index == recommended_index;
        }
    }
    Ok(roots)
}

fn validate_workspace_setup_project(project: &Path, managed_root: &Path) -> Result<()> {
    let home = fs::canonicalize(home_dir()).ok();
    if project.parent().is_none() || home.as_deref() == Some(project) {
        return Err(
            "Choose a project directory, not the filesystem or home directory.".to_string(),
        );
    }

    let managed_root = expand_home(managed_root.to_path_buf());
    let managed_root = fs::canonicalize(&managed_root).unwrap_or(managed_root);
    if project == managed_root || project.starts_with(&managed_root) {
        return Err("The SkillBox managed store cannot be initialized as a workspace.".to_string());
    }
    Ok(())
}

fn validate_project_workspace_root_chain(project: &Path, relative: &Path) -> Result<()> {
    let mut current = project.to_path_buf();
    for component in relative.components() {
        let Component::Normal(name) = component else {
            return Err("Workspace root contains an unsafe path component.".to_string());
        };
        current.push(name);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(format!(
                    "Supported workspace path cannot be a symlink: {}",
                    current.display()
                ));
            }
            Ok(metadata) if !metadata.is_dir() => {
                return Err(format!(
                    "Supported workspace path is not a directory: {}",
                    current.display()
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
            Err(error) => return Err(error.to_string()),
        }
    }
    Ok(())
}

fn workspace_setup_preview_id(
    selected_path: &Path,
    kind: WorkspaceKind,
    mode: WorkspaceSetupMode,
    roots: &[WorkspaceSetupRootOption],
) -> String {
    let selected_identity = fs::metadata(selected_path)
        .map(|metadata| format!("{}:{}", metadata.dev(), metadata.ino()))
        .unwrap_or_else(|_| "missing".to_string());
    let roots = roots
        .iter()
        .map(|root| {
            let identity = fs::metadata(&root.path)
                .map(|metadata| format!("{}:{}", metadata.dev(), metadata.ino()))
                .unwrap_or_else(|_| "missing".to_string());
            format!("{}:{}:{identity}", root.path.display(), root.exists)
        })
        .collect::<Vec<_>>()
        .join("|");
    content_hash_text(&format!(
        "workspace-setup-v1\n{}\n{}\n{}\n{:?}\n{}",
        selected_path.display(),
        selected_identity,
        kind.as_str(),
        mode,
        roots
    ))
}

fn validate_existing_workspace_root(
    selected_path: &Path,
    selected_root: &Path,
    mode: WorkspaceSetupMode,
) -> Result<PathBuf> {
    let metadata = fs::symlink_metadata(selected_root).map_err(|error| error.to_string())?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(format!(
            "Skills folder is no longer a safe directory: {}",
            selected_root.display()
        ));
    }
    let canonical = fs::canonicalize(selected_root).map_err(|error| error.to_string())?;
    if mode != WorkspaceSetupMode::ExistingRoot && !canonical.starts_with(selected_path) {
        return Err(format!(
            "Skills folder escapes the selected project: {}",
            selected_root.display()
        ));
    }
    fs::read_dir(&canonical).map_err(|error| error.to_string())?;
    Ok(canonical)
}

fn create_project_workspace_root(
    project: &Path,
    option: &WorkspaceSetupRootOption,
    created: &mut Vec<PathBuf>,
) -> Result<PathBuf> {
    if option.exists
        || !PROJECT_WORKSPACE_ROOTS
            .iter()
            .any(|(relative, _, _)| *relative == option.relative_path)
    {
        return Err("Unsupported workspace root choice.".to_string());
    }
    let relative = Path::new(&option.relative_path);
    if relative.is_absolute()
        || relative.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err("Workspace root contains an unsafe path component.".to_string());
    }

    let mut current = project.to_path_buf();
    for component in relative.components() {
        let Component::Normal(name) = component else {
            return Err("Workspace root contains an unsafe path component.".to_string());
        };
        current.push(name);
        match fs::symlink_metadata(&current) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() || !metadata.is_dir() {
                    cleanup_created_workspace_dirs(created);
                    return Err(format!(
                        "Refusing to create skills folder through a non-directory or symlink: {}",
                        current.display()
                    ));
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if let Err(error) = fs::create_dir(&current) {
                    cleanup_created_workspace_dirs(created);
                    return Err(format!("Unable to create {}: {error}", current.display()));
                }
                created.push(current.clone());
            }
            Err(error) => {
                cleanup_created_workspace_dirs(created);
                return Err(error.to_string());
            }
        }
    }

    let canonical = match fs::canonicalize(&current) {
        Ok(canonical) => canonical,
        Err(error) => {
            cleanup_created_workspace_dirs(created);
            return Err(error.to_string());
        }
    };
    if !canonical.starts_with(project) {
        cleanup_created_workspace_dirs(created);
        return Err(format!(
            "Created skills folder escapes the selected project: {}",
            current.display()
        ));
    }
    Ok(canonical)
}

fn cleanup_created_workspace_dirs(created: &[PathBuf]) {
    for path in created.iter().rev() {
        let _ = fs::remove_dir(path);
    }
}

fn add_workspace_unlogged(request: WorkspaceAddRequest, managed_root: &Path) -> Result<Workspace> {
    let paths = ensure_managed_layout(managed_root.to_path_buf())?;
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
    let managed_root = managed_root.as_ref().to_path_buf();
    let workspace_path = expand_home(path.as_ref().to_path_buf());
    audited_operation(
        OperationStart {
            operation_type: "forget_workspace".to_string(),
            actor: "core".to_string(),
            entity_type: "workspace".to_string(),
            entity_name: workspace_path.to_string_lossy().to_string(),
            summary: "Forget workspace".to_string(),
            payload: serde_json::json!({"path": workspace_path}),
        },
        &managed_root,
        || forget_workspace_unlogged(&workspace_path, &managed_root),
        |_| {
            (
                "Forgot workspace".to_string(),
                serde_json::json!({"path": workspace_path}),
            )
        },
    )
}

fn forget_workspace_unlogged(path: &Path, managed_root: &Path) -> Result<Vec<Workspace>> {
    let paths = ensure_managed_layout(managed_root.to_path_buf())?;
    let workspace_path = path.to_path_buf();
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
    _imported_hashes: &HashSet<String>,
    paths: &ManagedPaths,
) -> bool {
    skill.is_symlink && is_under_path(&skill.real_path, &paths.root)
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
