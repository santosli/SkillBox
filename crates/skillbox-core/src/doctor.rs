use crate::*;

pub fn run_doctor(request: DoctorRequest, managed_root: impl AsRef<Path>) -> Result<DoctorReport> {
    let paths = ensure_managed_layout(managed_root.as_ref().to_path_buf())?;
    let connection = open_database(&paths.database_path)?;
    let schema_version = current_database_schema_version(&connection)?;
    let mut issues = Vec::new();

    if let Err(error) = validate_database_integrity(&connection) {
        push_doctor_issue(
            &mut issues,
            &request,
            "database_integrity_failed",
            DoctorIssueSeverity::Error,
            None,
            Some(paths.database_path.clone()),
            error,
            false,
            None,
        );
    }
    if schema_version != LATEST_DATABASE_SCHEMA_VERSION {
        push_doctor_issue(
            &mut issues,
            &request,
            "database_schema_outdated",
            DoctorIssueSeverity::Error,
            None,
            Some(paths.database_path.clone()),
            format!(
                "Database schema is v{schema_version}; SkillBox expects v{LATEST_DATABASE_SCHEMA_VERSION}."
            ),
            true,
            Some("Run SkillBox again to retry the pending database migrations."),
        );
    }

    check_managed_skill_layout(&paths, &request, &mut issues)?;
    check_database_skill_index(&paths, &request, &mut issues)?;
    check_deployments(&paths, &request, &mut issues)?;
    check_workspaces(&paths, &request, &mut issues)?;
    check_import_records(&paths, &request, &mut issues)?;
    check_skill_user_metadata(&paths, &request, &mut issues)?;

    issues.sort_by(|left, right| {
        doctor_severity_rank(right.severity)
            .cmp(&doctor_severity_rank(left.severity))
            .then_with(|| left.code.cmp(&right.code))
            .then_with(|| left.entity_name.cmp(&right.entity_name))
    });
    let error_count = issues
        .iter()
        .filter(|issue| issue.severity == DoctorIssueSeverity::Error)
        .count();
    let warning_count = issues.len() - error_count;

    Ok(DoctorReport {
        checked_at: current_rfc3339_timestamp(),
        schema_version,
        latest_schema_version: LATEST_DATABASE_SCHEMA_VERSION,
        healthy: issues.is_empty(),
        error_count,
        warning_count,
        repair_preview: request.repair_preview,
        issues,
    })
}

pub fn repair_stale_deployment_records(
    managed_root: impl AsRef<Path>,
) -> Result<DoctorRepairResult> {
    let managed_root = managed_root.as_ref().to_path_buf();
    audited_operation(
        OperationStart {
            operation_type: "repair_stale_deployments".to_string(),
            actor: "core".to_string(),
            entity_type: "managed_store".to_string(),
            entity_name: "deployments".to_string(),
            summary: "Clean stale deployment records".to_string(),
            payload: serde_json::json!({}),
        },
        &managed_root,
        || repair_stale_deployment_records_unlogged(&managed_root),
        |result| {
            (
                format!(
                    "Removed {} stale deployment record(s)",
                    result.removed_deployment_records
                ),
                serde_json::json!({
                    "removedDeploymentRecords": result.removed_deployment_records
                }),
            )
        },
    )
}

fn repair_stale_deployment_records_unlogged(managed_root: &Path) -> Result<DoctorRepairResult> {
    let paths = ensure_managed_layout(managed_root.to_path_buf())?;
    let deployments = load_deployments(&paths.database_path)?;
    let mut removed_deployment_records = 0;

    for (skill_name, skill_deployments) in deployments {
        if resolve_managed_skill_path(&paths, &skill_name).is_ok() {
            continue;
        }
        for deployment in skill_deployments {
            match fs::symlink_metadata(&deployment.target_path) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    if resolve_managed_skill_path(&paths, &skill_name).is_err()
                        && matches!(
                            fs::symlink_metadata(&deployment.target_path),
                            Err(recheck) if recheck.kind() == std::io::ErrorKind::NotFound
                        )
                    {
                        remove_deployment(
                            &paths.database_path,
                            &skill_name,
                            &deployment.target_root,
                        )?;
                        removed_deployment_records += 1;
                    }
                }
                Ok(_) => {}
                Err(error) => return Err(error.to_string()),
            }
        }
    }

    Ok(DoctorRepairResult {
        removed_deployment_records,
    })
}

fn check_managed_skill_layout(
    paths: &ManagedPaths,
    request: &DoctorRequest,
    issues: &mut Vec<DoctorIssue>,
) -> Result<()> {
    for entry in readable_directory_entries(&paths.user_skills_root)? {
        let path = entry.path();
        if entry.file_name().to_string_lossy().starts_with('.') || !path.is_dir() {
            continue;
        }
        if !path.join("SKILL.md").is_file() {
            push_doctor_issue(
                issues,
                request,
                "user_skill_missing_manifest",
                DoctorIssueSeverity::Error,
                path.file_name()
                    .map(|name| name.to_string_lossy().to_string()),
                Some(path.clone()),
                format!("User skill is missing SKILL.md: {}", path.display()),
                false,
                None,
            );
        }
    }

    for entry in readable_directory_entries(&paths.remote_skills_root)? {
        let remote_root = entry.path();
        if entry.file_name().to_string_lossy().starts_with('.') || !remote_root.is_dir() {
            continue;
        }
        let skill_name = entry.file_name().to_string_lossy().to_string();
        let current = remote_root.join("current");
        match fs::symlink_metadata(&current) {
            Ok(metadata) if !metadata.file_type().is_symlink() => push_doctor_issue(
                issues,
                request,
                "remote_current_not_symlink",
                DoctorIssueSeverity::Error,
                Some(skill_name),
                Some(current),
                "Remote skill current target is not a symlink.".to_string(),
                false,
                None,
            ),
            Ok(_) => {
                let resolved = match fs::canonicalize(&current) {
                    Ok(path) => path,
                    Err(_) => {
                        push_doctor_issue(
                            issues,
                            request,
                            "remote_current_invalid",
                            DoctorIssueSeverity::Error,
                            Some(skill_name),
                            Some(current),
                            "Remote skill current symlink is broken.".to_string(),
                            true,
                            Some("Choose a valid stored version and restore the current symlink."),
                        );
                        continue;
                    }
                };
                let versions = match fs::canonicalize(remote_root.join("versions")) {
                    Ok(path) => path,
                    Err(_) => {
                        push_doctor_issue(
                            issues,
                            request,
                            "remote_versions_missing",
                            DoctorIssueSeverity::Error,
                            Some(skill_name),
                            Some(remote_root.join("versions")),
                            "Remote skill versions directory is missing.".to_string(),
                            false,
                            None,
                        );
                        continue;
                    }
                };
                if !is_under_path(&resolved, &versions) || !resolved.join("SKILL.md").is_file() {
                    push_doctor_issue(
                        issues,
                        request,
                        "remote_current_invalid",
                        DoctorIssueSeverity::Error,
                        Some(skill_name),
                        Some(current),
                        "Remote skill current does not resolve to a valid managed version."
                            .to_string(),
                        true,
                        Some("Choose a valid stored version and restore the current symlink."),
                    );
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => push_doctor_issue(
                issues,
                request,
                "remote_current_missing",
                DoctorIssueSeverity::Error,
                Some(skill_name),
                Some(current),
                "Remote skill has no current version symlink.".to_string(),
                true,
                Some("Choose a valid stored version and restore the current symlink."),
            ),
            Err(error) => return Err(error.to_string()),
        }
    }
    Ok(())
}

fn check_database_skill_index(
    paths: &ManagedPaths,
    request: &DoctorRequest,
    issues: &mut Vec<DoctorIssue>,
) -> Result<()> {
    let connection = open_database(&paths.database_path)?;
    let mut statement = connection
        .prepare("SELECT name, managed_path FROM skills ORDER BY name")
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                PathBuf::from(row.get::<_, String>(1)?),
            ))
        })
        .map_err(|error| error.to_string())?;
    for row in rows {
        let (skill_name, managed_path) = row.map_err(|error| error.to_string())?;
        if !managed_path.join("SKILL.md").is_file() {
            push_doctor_issue(
                issues,
                request,
                "skill_index_target_missing",
                DoctorIssueSeverity::Error,
                Some(skill_name),
                Some(managed_path),
                "SQLite skill index points to a missing managed skill.".to_string(),
                true,
                Some("Re-index the managed store after confirming the skill directory state."),
            );
        }
    }
    Ok(())
}

fn check_deployments(
    paths: &ManagedPaths,
    request: &DoctorRequest,
    issues: &mut Vec<DoctorIssue>,
) -> Result<()> {
    for (skill_name, deployments) in load_deployments(&paths.database_path)? {
        let managed_path = match resolve_managed_skill_path(paths, &skill_name) {
            Ok(path) => path,
            Err(error) => {
                for deployment in deployments {
                    match fs::symlink_metadata(&deployment.target_path) {
                        Err(target_error)
                            if target_error.kind() == std::io::ErrorKind::NotFound =>
                        {
                            push_doctor_issue(
                                issues,
                                request,
                                "deployment_record_stale",
                                DoctorIssueSeverity::Warning,
                                Some(skill_name.clone()),
                                Some(deployment.target_path),
                                "Deployment record references a missing managed skill and runtime target."
                                    .to_string(),
                                true,
                                Some("Remove the stale deployment record; the runtime target is already missing."),
                            );
                        }
                        Ok(_) => push_doctor_issue(
                            issues,
                            request,
                            "deployment_managed_skill_missing",
                            DoctorIssueSeverity::Error,
                            Some(skill_name.clone()),
                            Some(deployment.target_path),
                            format!(
                                "{error}; the runtime target still exists and requires review."
                            ),
                            false,
                            None,
                        ),
                        Err(target_error) => return Err(target_error.to_string()),
                    }
                }
                continue;
            }
        };
        for deployment in deployments {
            match fs::symlink_metadata(&deployment.target_path) {
                Ok(metadata) if !metadata.file_type().is_symlink() => push_doctor_issue(
                    issues,
                    request,
                    "deployment_target_not_symlink",
                    DoctorIssueSeverity::Error,
                    Some(skill_name.clone()),
                    Some(deployment.target_path),
                    "Deployment target is no longer a symlink.".to_string(),
                    false,
                    None,
                ),
                Ok(_)
                    if !symlink_points_to_managed_entry(
                        &deployment.target_path,
                        &managed_path,
                    )? =>
                {
                    push_doctor_issue(
                        issues,
                        request,
                        "deployment_target_mismatch",
                        DoctorIssueSeverity::Error,
                        Some(skill_name.clone()),
                        Some(deployment.target_path),
                        "Deployment symlink points outside the expected managed skill.".to_string(),
                        false,
                        None,
                    )
                }
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => push_doctor_issue(
                    issues,
                    request,
                    "deployment_target_missing",
                    DoctorIssueSeverity::Warning,
                    Some(skill_name.clone()),
                    Some(deployment.target_path),
                    "Deployment record points to a missing runtime target.".to_string(),
                    true,
                    Some("Remove the stale deployment record or deploy the skill again."),
                ),
                Err(error) => return Err(error.to_string()),
            }
        }
    }
    Ok(())
}

fn symlink_points_to_managed_entry(symlink: &Path, expected: &Path) -> Result<bool> {
    let target = fs::read_link(symlink).map_err(|error| error.to_string())?;
    let target = if target.is_absolute() {
        target
    } else {
        symlink
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .join(target)
    };
    if target == expected {
        return Ok(true);
    }

    Ok(canonical_entry_path(&target)? == canonical_entry_path(expected)?)
}

fn canonical_entry_path(path: &Path) -> Result<PathBuf> {
    let file_name = path
        .file_name()
        .ok_or_else(|| format!("Path has no final entry: {}", path.display()))?;
    let parent = path
        .parent()
        .ok_or_else(|| format!("Path has no parent: {}", path.display()))?;
    let canonical_parent = fs::canonicalize(parent).map_err(|error| error.to_string())?;
    Ok(canonical_parent.join(file_name))
}

fn check_workspaces(
    paths: &ManagedPaths,
    request: &DoctorRequest,
    issues: &mut Vec<DoctorIssue>,
) -> Result<()> {
    for workspace in load_workspaces(&paths.database_path)? {
        if !workspace.path.is_dir() {
            push_doctor_issue(
                issues,
                request,
                "workspace_missing",
                DoctorIssueSeverity::Warning,
                Some(workspace.display_name),
                Some(workspace.path),
                "Registered workspace directory is missing.".to_string(),
                true,
                Some("Forget the manual workspace or run a workspace scan to prune stale auto entries."),
            );
            continue;
        }
        for entry in readable_directory_entries(&workspace.path)? {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') && name.contains(".delete-check-") && name.ends_with(".tmp") {
                push_doctor_issue(
                    issues,
                    request,
                    "deletion_quarantine_preserved",
                    DoctorIssueSeverity::Error,
                    Some(workspace.display_name.clone()),
                    Some(entry.path()),
                    "A deletion ownership check preserved an unexpected workspace target."
                        .to_string(),
                    false,
                    Some("Review the preserved path manually; SkillBox will not delete unknown content."),
                );
            }
        }
    }
    Ok(())
}

fn check_import_records(
    paths: &ManagedPaths,
    request: &DoctorRequest,
    issues: &mut Vec<DoctorIssue>,
) -> Result<()> {
    for record in load_import_records(&paths.database_path, &ImportRecordFilter::default())? {
        if record.status != ImportRecordStatus::Active {
            continue;
        }
        if !record.backup_path.join("SKILL.md").is_file() {
            push_doctor_issue(
                issues,
                request,
                "import_backup_missing",
                DoctorIssueSeverity::Error,
                Some(record.skill_name.clone()),
                Some(record.backup_path),
                "Active import record has no valid backup.".to_string(),
                false,
                None,
            );
        }
        let source_valid = fs::symlink_metadata(&record.source_path)
            .map(|metadata| metadata.file_type().is_symlink())
            .unwrap_or(false)
            && symlink_points_to_path(&record.source_path, &record.managed_path).unwrap_or(false);
        if !source_valid {
            push_doctor_issue(
                issues,
                request,
                "import_source_mismatch",
                DoctorIssueSeverity::Error,
                Some(record.skill_name),
                Some(record.source_path),
                "Active import source is not linked to its recorded managed skill.".to_string(),
                false,
                None,
            );
        }
    }
    Ok(())
}

fn check_skill_user_metadata(
    paths: &ManagedPaths,
    request: &DoctorRequest,
    issues: &mut Vec<DoctorIssue>,
) -> Result<()> {
    let managed_names = managed_skill_names(paths)?;
    for metadata in list_skill_user_metadata(&paths.root)? {
        if !managed_names.contains(&metadata.skill_name) {
            push_doctor_issue(
                issues,
                request,
                "skill_metadata_stale",
                DoctorIssueSeverity::Warning,
                Some(metadata.skill_name),
                None,
                "Favorites or tags reference a skill that is no longer managed.".to_string(),
                true,
                Some("Remove the stale skill metadata row."),
            );
        }
    }
    Ok(())
}

fn managed_skill_names(paths: &ManagedPaths) -> Result<HashSet<String>> {
    let mut names = HashSet::new();
    for entry in readable_directory_entries(&paths.user_skills_root)? {
        if entry.path().join("SKILL.md").is_file() {
            names.insert(entry.file_name().to_string_lossy().to_string());
        }
    }
    for entry in readable_directory_entries(&paths.remote_skills_root)? {
        if entry.path().join("current/SKILL.md").is_file() {
            names.insert(entry.file_name().to_string_lossy().to_string());
        }
    }
    Ok(names)
}

fn readable_directory_entries(path: &Path) -> Result<Vec<fs::DirEntry>> {
    fs::read_dir(path)
        .map_err(|error| error.to_string())?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

#[allow(clippy::too_many_arguments)]
fn push_doctor_issue(
    issues: &mut Vec<DoctorIssue>,
    request: &DoctorRequest,
    code: &str,
    severity: DoctorIssueSeverity,
    entity_name: Option<String>,
    path: Option<PathBuf>,
    message: String,
    repairable: bool,
    suggested_action: Option<&str>,
) {
    issues.push(DoctorIssue {
        code: code.to_string(),
        severity,
        entity_name,
        path,
        message,
        repairable,
        suggested_action: request
            .repair_preview
            .then(|| suggested_action.map(str::to_string))
            .flatten(),
    });
}

fn doctor_severity_rank(severity: DoctorIssueSeverity) -> u8 {
    match severity {
        DoctorIssueSeverity::Warning => 0,
        DoctorIssueSeverity::Error => 1,
    }
}
