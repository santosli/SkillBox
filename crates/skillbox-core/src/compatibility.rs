use crate::*;
use rusqlite::OpenFlags;
use std::os::unix::fs::MetadataExt;

const DEPLOYMENT_MODE_SYMLINK: &str = "symlink";

pub fn preview_skill_deployment(
    request: DeploymentCompatibilityPreviewRequest,
    managed_root: impl AsRef<Path>,
) -> Result<CompatibilityReport> {
    validate_skill_name(&request.skill_name)?;
    let paths = managed_paths(managed_root.as_ref().to_path_buf());
    let managed_path = resolve_managed_skill_path(&paths, &request.skill_name)?;
    build_skill_deployment_compatibility(
        &request.skill_name,
        &managed_path,
        &managed_path,
        &request.target_root,
        &paths.database_path,
    )
}

pub(crate) fn preview_skill_path_deployment(
    skill_name: &str,
    skill_path: &Path,
    expected_deployment_path: &Path,
    target_root: &Path,
    managed_root: &Path,
) -> Result<CompatibilityReport> {
    let paths = managed_paths(managed_root.to_path_buf());
    build_skill_deployment_compatibility(
        skill_name,
        skill_path,
        expected_deployment_path,
        target_root,
        &paths.database_path,
    )
}

fn build_skill_deployment_compatibility(
    skill_name: &str,
    skill_path: &Path,
    expected_deployment_path: &Path,
    requested_target_root: &Path,
    database_path: &Path,
) -> Result<CompatibilityReport> {
    let target_root = canonical_existing_directory(requested_target_root)?;
    let (profile_id, root_key, format) =
        load_registered_workspace_runtime(database_path, &target_root)?;
    let profile = runtime_profile(&profile_id)
        .ok_or_else(|| format!("Unsupported runtime profile: {profile_id}"))?;
    let mut issues = Vec::new();
    match profile.roots.iter().find(|root| root.key == root_key) {
        None => issues.push(blocked_issue(
            "unsupported_runtime_root",
            None,
            format!(
                "Workspace root key '{root_key}' is not defined by the {} profile.",
                profile.display_name
            ),
            "Re-scan or re-add this workspace before deploying.",
        )),
        Some(root)
            if root.scope == RuntimeRootScope::Project
                && !path_ends_with(&target_root, Path::new(&root.relative_path)) =>
        {
            issues.push(blocked_issue(
                "runtime_root_mismatch",
                None,
                format!(
                    "Workspace path does not match the {} profile root {}.",
                    profile.display_name, root.relative_path
                ),
                "Re-scan or re-add this workspace before deploying.",
            ));
        }
        Some(_) => {}
    }

    if profile.format != format {
        issues.push(blocked_issue(
            "format_mismatch",
            None,
            format!(
                "Workspace format {} does not match profile format {}.",
                format.as_str(),
                profile.format.as_str()
            ),
            "Re-scan or re-add this workspace before deploying.",
        ));
    }
    if !profile
        .deployment_modes
        .iter()
        .any(|mode| mode == DEPLOYMENT_MODE_SYMLINK)
    {
        issues.push(blocked_issue(
            "unsupported_deployment_mode",
            None,
            "This runtime profile does not support symlink deployment.".to_string(),
            "Choose a runtime profile that supports symlink deployment.",
        ));
    }

    let skill_md = fs::read_to_string(skill_path.join("SKILL.md"))
        .map_err(|error| format!("Unable to read managed SKILL.md: {error}"))?;
    match parse_skill_frontmatter_document(&skill_md) {
        Ok(document) => {
            if !document.metadata.name.is_empty() && document.metadata.name != skill_name {
                issues.push(blocked_issue(
                    "skill_name_mismatch",
                    Some("name"),
                    format!(
                        "Frontmatter name '{}' does not match managed skill '{}'.",
                        document.metadata.name, skill_name
                    ),
                    "Make the frontmatter name match the managed skill directory before deploying.",
                ));
            }
            for field in document.unknown_fields {
                issues.push(CompatibilityIssue {
                    code: "unknown_optional_frontmatter".to_string(),
                    severity: CompatibilityIssueSeverity::Warning,
                    field: Some(field.clone()),
                    message: format!(
                        "Frontmatter field '{field}' is not defined by the {} profile and will be preserved unchanged.",
                        profile.display_name
                    ),
                    suggested_action: Some(
                        "Confirm that the target runtime understands this optional field."
                            .to_string(),
                    ),
                });
            }
            for field in &profile.frontmatter_policy.required_fields {
                if !document.fields.contains_key(field) {
                    issues.push(blocked_issue(
                        "required_frontmatter_missing",
                        Some(field),
                        format!(
                            "Required frontmatter field '{field}' is missing for the {} profile.",
                            profile.display_name
                        ),
                        "Add the required field to SKILL.md and preview again.",
                    ));
                }
            }
        }
        Err(error) => issues.push(blocked_issue(
            "invalid_frontmatter",
            None,
            error,
            "Fix the SKILL.md frontmatter and preview again.",
        )),
    }

    let target_path = target_root.join(skill_name);
    let target_state =
        deployment_target_state(&target_path, expected_deployment_path, &mut issues)?;
    let skill_snapshot = skill_directory_snapshot_hash(skill_path)?;
    let status = compatibility_status(&issues);
    let preview_id = content_hash_text(&format!(
        "deployment-compatibility-v1\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}",
        skill_name,
        skill_snapshot,
        target_root.display(),
        profile.id,
        profile.registry_version,
        root_key,
        format.as_str(),
        target_state
    ));

    Ok(CompatibilityReport {
        preview_id,
        skill_name: skill_name.to_string(),
        target_root,
        profile,
        root_key,
        format,
        deployment_mode: DEPLOYMENT_MODE_SYMLINK.to_string(),
        status,
        issues,
    })
}

pub fn apply_skill_deployment(
    request: DeploymentCompatibilityApplyRequest,
    managed_root: impl AsRef<Path>,
) -> Result<Deployment> {
    let mutation_lock = acquire_user_skills_mutation_lock(managed_root.as_ref())?;
    let managed_root = mutation_lock.truth_root().to_path_buf();
    let preview = preview_skill_deployment(
        DeploymentCompatibilityPreviewRequest {
            skill_name: request.skill_name.clone(),
            target_root: request.target_root.clone(),
        },
        &managed_root,
    )?;
    if preview.preview_id != request.preview_id {
        return Err(
            "Deployment compatibility preview is stale. Preview the target again before deploying."
                .to_string(),
        );
    }
    match preview.status {
        CompatibilityStatus::Blocked => {
            return Err(
                "Deployment is blocked by runtime compatibility issues. Review the preview."
                    .to_string(),
            )
        }
        CompatibilityStatus::Warnings if !request.confirm_warnings => {
            return Err(
                "Deployment has compatibility warnings. Confirm the warnings before deploying."
                    .to_string(),
            )
        }
        CompatibilityStatus::Compatible | CompatibilityStatus::Warnings => {}
    }
    deploy_skill_with_lock_held(&request.skill_name, &managed_root, preview.target_root)
}

fn load_registered_workspace_runtime(
    database_path: &Path,
    canonical_path: &Path,
) -> Result<(String, String, RuntimeFormat)> {
    if !database_path.exists() {
        return Err(format!(
            "Deployment target is not a registered workspace: {}",
            canonical_path.display()
        ));
    }
    let connection = Connection::open_with_flags(
        database_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|error| error.to_string())?;
    connection
        .query_row(
            "
            SELECT profile_id, root_key, format
            FROM workspaces
            WHERE canonical_path = ?1
            ",
            params![canonical_path.to_string_lossy()],
            |row| {
                let profile_id: String = row.get(0)?;
                let root_key: String = row.get(1)?;
                let format: String = row.get(2)?;
                Ok((profile_id, root_key, format))
            },
        )
        .optional()
        .map_err(|error| error.to_string())?
        .ok_or_else(|| {
            format!(
                "Deployment target is not a registered workspace: {}",
                canonical_path.display()
            )
        })
        .map(|(profile_id, root_key, format)| {
            (profile_id, root_key, runtime_format_from_str(&format))
        })
}

fn canonical_existing_directory(path: &Path) -> Result<PathBuf> {
    let path = expand_home(path.to_path_buf());
    let metadata = fs::symlink_metadata(&path).map_err(|error| {
        format!(
            "Deployment target cannot be read: {} ({error})",
            path.display()
        )
    })?;
    if !metadata.is_dir() && !metadata.file_type().is_symlink() {
        return Err(format!(
            "Deployment target is not a directory: {}",
            path.display()
        ));
    }
    let canonical = fs::canonicalize(&path).map_err(|error| error.to_string())?;
    if !fs::metadata(&canonical)
        .map_err(|error| error.to_string())?
        .is_dir()
    {
        return Err(format!(
            "Deployment target is not a directory: {}",
            canonical.display()
        ));
    }
    Ok(canonical)
}

fn deployment_target_state(
    target_path: &Path,
    managed_path: &Path,
    issues: &mut Vec<CompatibilityIssue>,
) -> Result<String> {
    match fs::symlink_metadata(target_path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok("missing".to_string()),
        Err(error) => Err(error.to_string()),
        Ok(metadata) if !metadata.file_type().is_symlink() => {
            issues.push(blocked_issue(
                "existing_non_symlink_target",
                None,
                format!(
                    "Refusing to overwrite existing non-symlink target: {}",
                    target_path.display()
                ),
                "Move or import the existing target before deploying.",
            ));
            Ok(format!(
                "non-symlink:{}:{}:{}",
                metadata.dev(),
                metadata.ino(),
                metadata.len()
            ))
        }
        Ok(metadata) => {
            let link = fs::read_link(target_path).map_err(|error| error.to_string())?;
            let resolved = fs::canonicalize(target_path).map_err(|error| error.to_string())?;
            let expected = fs::canonicalize(managed_path).map_err(|error| error.to_string())?;
            if resolved != expected {
                issues.push(blocked_issue(
                    "existing_foreign_symlink",
                    None,
                    format!(
                        "Refusing to replace symlink pointing elsewhere: {}",
                        target_path.display()
                    ),
                    "Remove or reconcile the existing symlink before deploying.",
                ));
            }
            Ok(format!(
                "symlink:{}:{}:{}",
                metadata.dev(),
                metadata.ino(),
                link.display()
            ))
        }
    }
}

fn compatibility_status(issues: &[CompatibilityIssue]) -> CompatibilityStatus {
    if issues
        .iter()
        .any(|issue| issue.severity == CompatibilityIssueSeverity::Blocked)
    {
        CompatibilityStatus::Blocked
    } else if issues.is_empty() {
        CompatibilityStatus::Compatible
    } else {
        CompatibilityStatus::Warnings
    }
}

fn blocked_issue(
    code: &str,
    field: Option<&str>,
    message: String,
    suggested_action: &str,
) -> CompatibilityIssue {
    CompatibilityIssue {
        code: code.to_string(),
        severity: CompatibilityIssueSeverity::Blocked,
        field: field.map(str::to_string),
        message,
        suggested_action: Some(suggested_action.to_string()),
    }
}
