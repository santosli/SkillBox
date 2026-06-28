use crate::*;

pub fn scan_import_candidates(
    roots: &[PathBuf],
    managed_root: impl AsRef<Path>,
) -> Result<ImportCandidateScan> {
    let paths = ensure_managed_layout(managed_root.as_ref().to_path_buf())?;
    let imported_hashes = imported_skill_hashes(&paths)?;
    let usage_by_skill = load_usage_by_skill(&paths.database_path)?;
    let usage_by_skill_runtime = load_usage_by_skill_runtime(&paths.database_path)?;
    let scan = scan_skill_roots_for_import(roots, &paths)?;
    record_scanned_workspaces(&paths, &scan.roots)?;
    let mut candidates = Vec::new();

    for skill in scan.skills {
        let is_system = is_system_skill(&skill);
        let is_imported = skill_is_imported(&skill, &imported_hashes, &paths);
        let (suggested_type, suggestion_reason, default_selected) =
            infer_import_candidate_type(&skill, &paths);
        let (suggestion_reason, import_status, is_selected, conflict) = if is_system {
            (
                suggestion_reason,
                ImportCandidateStatus::System,
                false,
                None,
            )
        } else if is_imported {
            (
                imported_candidate_reason(&skill, &paths),
                ImportCandidateStatus::Imported,
                false,
                None,
            )
        } else {
            let conflict = managed_target_conflict(&paths, &skill, suggested_type)?;
            let is_selected = default_selected && conflict.is_none();
            (
                suggestion_reason,
                ImportCandidateStatus::Importable,
                is_selected,
                conflict,
            )
        };

        let usage_count = import_candidate_usage_count(
            &skill,
            import_status,
            &usage_by_skill,
            &usage_by_skill_runtime,
            &paths,
        );
        let symlink_target_path = skill_symlink_target_path(&skill);

        candidates.push(ImportCandidate {
            name: skill.name,
            description: skill.description,
            source_path: skill.path,
            source_root: skill.source_root,
            real_path: skill.real_path,
            is_symlink: skill.is_symlink,
            symlink_target_path,
            content_hash: skill.content_hash,
            suggested_type,
            suggestion_reason,
            import_status,
            is_selected,
            conflict,
            usage_count,
        });
    }

    candidates.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| left.source_path.cmp(&right.source_path))
    });
    dedupe_imported_candidates(&mut candidates);
    Ok(ImportCandidateScan {
        roots: scan.roots,
        candidates,
        errors: scan.errors,
    })
}

pub(crate) fn dedupe_imported_candidates(candidates: &mut Vec<ImportCandidate>) {
    let mut imported_keys = HashSet::new();
    candidates.retain(|candidate| {
        if candidate.import_status != ImportCandidateStatus::Imported {
            return true;
        }

        imported_keys.insert((candidate.name.clone(), candidate.content_hash.clone()))
    });
}

pub(crate) fn import_candidate_usage_count(
    skill: &Skill,
    import_status: ImportCandidateStatus,
    usage_by_skill: &HashMap<String, UsageSummary>,
    usage_by_skill_runtime: &HashMap<(String, String), UsageSummary>,
    paths: &ManagedPaths,
) -> usize {
    if import_status == ImportCandidateStatus::Imported {
        return usage_by_skill
            .get(&skill.name)
            .map(|usage| usage.usage_count)
            .unwrap_or_default();
    }

    skill_usage_runtime_keys(skill, paths)
        .into_iter()
        .filter_map(|runtime_key| usage_by_skill_runtime.get(&(skill.name.clone(), runtime_key)))
        .map(|usage| usage.usage_count)
        .sum()
}

pub fn import_candidates(
    items: Vec<ImportRequestItem>,
    managed_root: impl AsRef<Path>,
) -> Result<ImportBatchResult> {
    let paths = ensure_managed_layout(managed_root.as_ref().to_path_buf())?;
    let mut imported = Vec::new();
    let mut errors = Vec::new();

    for item in items {
        let source_path = item.source_path.clone();
        match import_one_candidate(&paths, item) {
            Ok(candidate) => imported.push(candidate),
            Err(error) => errors.push(ImportCandidateError { source_path, error }),
        }
    }

    Ok(ImportBatchResult { imported, errors })
}

pub(crate) fn import_one_candidate(
    paths: &ManagedPaths,
    item: ImportRequestItem,
) -> Result<ImportedCandidate> {
    let source_path = expand_home(item.source_path);
    let imported = import_skill(&source_path, item.skill_type, &paths.root)?;
    let deployment_target = match item.skill_type {
        SkillKind::User => imported.managed_path.clone(),
        SkillKind::Remote => paths
            .remote_skills_root
            .join(&imported.name)
            .join("current"),
    };
    let (backup_path, deployed_path) = if item.deploy_back_to_source {
        let backup_path = replace_source_with_symlink(
            &source_path,
            &deployment_target,
            paths,
            &imported.name,
            &imported.content_hash,
        )?;
        if let Some(backup_path) = backup_path.as_ref() {
            let source_root = source_path
                .parent()
                .ok_or_else(|| format!("Source path has no parent: {}", source_path.display()))?
                .to_path_buf();
            index_deployment(
                &paths.database_path,
                &imported.name,
                &source_root,
                &source_path,
            )?;
            insert_import_record(
                &paths.database_path,
                &new_import_record(
                    &imported.name,
                    imported.kind,
                    &source_path,
                    Some(&source_root),
                    &deployment_target,
                    &imported.content_hash,
                    backup_path,
                    &source_path,
                    false,
                ),
            )?;
        }
        (backup_path, Some(source_path.clone()))
    } else {
        (None, None)
    };

    Ok(ImportedCandidate {
        name: imported.name,
        kind: imported.kind,
        source_path,
        managed_path: imported.managed_path,
        content_hash: imported.content_hash,
        backup_path,
        deployed_path,
    })
}

pub fn list_import_records(
    filter: ImportRecordFilter,
    managed_root: impl AsRef<Path>,
) -> Result<ImportRecordList> {
    let paths = ensure_managed_layout(managed_root.as_ref().to_path_buf())?;
    reconcile_legacy_import_records(&paths, &filter)?;
    let mut records = load_import_records(&paths.database_path, &filter)?
        .into_iter()
        .map(|record| hydrate_import_record(&paths, record))
        .collect::<Result<Vec<_>>>()?;

    records.sort_by(|left, right| {
        right
            .imported_at
            .cmp(&left.imported_at)
            .then_with(|| right.id.cmp(&left.id))
    });
    Ok(ImportRecordList { records })
}

pub fn revert_import(
    request: RevertImportRequest,
    managed_root: impl AsRef<Path>,
) -> Result<RevertImportResult> {
    let managed_root = managed_root.as_ref().to_path_buf();
    let paths = ensure_managed_layout(managed_root.clone())?;
    let record = hydrate_import_record(
        &paths,
        load_import_record(&paths.database_path, &request.import_record_id)?,
    )?;
    let operation = start_operation(
        OperationStart {
            operation_type: "revert_import".to_string(),
            actor: request.actor.clone(),
            entity_type: "import_record".to_string(),
            entity_name: record.id.clone(),
            summary: format!("Revert import for {}", record.skill_name),
            payload: serde_json::json!({
                "skillName": record.skill_name,
                "sourcePath": record.source_path,
                "backupPath": record.backup_path
            }),
        },
        &managed_root,
    )?;

    match revert_import_inner(&paths, &record, operation.id.clone()) {
        Ok(result) => {
            finish_operation(
                OperationFinish {
                    id: operation.id,
                    status: OperationStatus::Succeeded,
                    summary: format!("Reverted import for {}", result.record.skill_name),
                    error: None,
                    payload: serde_json::json!({
                        "recordId": result.record.id,
                        "restoredPath": result.restored_path,
                        "removedManagedPath": result.removed_managed_path
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
                    summary: format!("Revert import failed for {}", record.skill_name),
                    error: Some(error.clone()),
                    payload: serde_json::json!({"recordId": record.id}),
                },
                &managed_root,
            );
            Err(error)
        }
    }
}

pub(crate) fn new_import_record(
    skill_name: &str,
    kind: SkillKind,
    source_path: &Path,
    source_root: Option<&Path>,
    managed_path: &Path,
    content_hash: &str,
    backup_path: &Path,
    deployed_path: &Path,
    legacy: bool,
) -> ImportRecord {
    ImportRecord {
        id: import_record_id(),
        skill_name: skill_name.to_string(),
        kind,
        source_path: source_path.to_path_buf(),
        source_root: source_root.map(Path::to_path_buf),
        managed_path: managed_path.to_path_buf(),
        content_hash: content_hash.to_string(),
        backup_path: backup_path.to_path_buf(),
        deployed_path: deployed_path.to_path_buf(),
        status: ImportRecordStatus::Active,
        legacy,
        imported_at: current_rfc3339_timestamp(),
        reverted_at: None,
        can_revert: false,
        revert_block_reason: None,
        affected_deployment_count: 0,
    }
}

fn hydrate_import_record(paths: &ManagedPaths, mut record: ImportRecord) -> Result<ImportRecord> {
    let (affected_deployment_count, block_reason) = import_record_revert_status(paths, &record)?;
    record.affected_deployment_count = affected_deployment_count;
    record.can_revert = block_reason.is_none();
    record.revert_block_reason = block_reason;
    Ok(record)
}

fn import_record_revert_status(
    paths: &ManagedPaths,
    record: &ImportRecord,
) -> Result<(usize, Option<String>)> {
    let deployment_count = load_deployments(&paths.database_path)?
        .get(&record.skill_name)
        .map(Vec::len)
        .unwrap_or_default();
    let active_record_count = load_import_records(
        &paths.database_path,
        &ImportRecordFilter {
            skill_name: Some(record.skill_name.clone()),
        },
    )?
    .into_iter()
    .filter(|candidate| candidate.status == ImportRecordStatus::Active)
    .count();
    let affected_count = deployment_count
        .max(active_record_count)
        .max(usize::from(record.status == ImportRecordStatus::Active));

    if record.status != ImportRecordStatus::Active {
        return Ok((
            affected_count,
            Some("Import record is not active.".to_string()),
        ));
    }
    if deployment_count > 1 || active_record_count > 1 {
        return Ok((
            affected_count,
            Some("Cannot revert while this skill is deployed to multiple workspaces.".to_string()),
        ));
    }
    if let Err(error) = validate_import_backup(record) {
        return Ok((affected_count, Some(error)));
    }
    if let Err(error) = validate_import_source_path(record) {
        return Ok((affected_count, Some(error)));
    }

    Ok((affected_count, None))
}

fn revert_import_inner(
    paths: &ManagedPaths,
    record: &ImportRecord,
    operation_id: String,
) -> Result<RevertImportResult> {
    if let Some(reason) = record.revert_block_reason.clone() {
        return Err(reason);
    }

    let source_existed = match fs::symlink_metadata(&record.source_path) {
        Ok(metadata) if metadata.file_type().is_symlink() => true,
        Ok(_) => {
            return Err(format!(
                "Refusing to replace existing non-symlink source: {}",
                record.source_path.display()
            ));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(error) => return Err(error.to_string()),
    };
    if source_existed {
        fs::remove_file(&record.source_path).map_err(|error| error.to_string())?;
    }

    if let Err(error) = fs::rename(&record.backup_path, &record.source_path) {
        if source_existed {
            let _ = symlink_dir(&record.managed_path, &record.source_path);
        }
        return Err(error.to_string());
    }

    if let Some(source_root) = record.source_path.parent() {
        remove_deployment(&paths.database_path, &record.skill_name, source_root)?;
    }
    let removed_managed_path = if record.kind == SkillKind::User {
        remove_reverted_user_managed_copy(paths, record)?
    } else {
        None
    };
    mark_import_record_reverted(&paths.database_path, &record.id)?;
    let record =
        hydrate_import_record(paths, load_import_record(&paths.database_path, &record.id)?)?;

    Ok(RevertImportResult {
        restored_path: record.source_path.clone(),
        record,
        removed_managed_path,
        operation_id,
    })
}

fn remove_reverted_user_managed_copy(
    paths: &ManagedPaths,
    record: &ImportRecord,
) -> Result<Option<PathBuf>> {
    let managed_path = normalize_lexical_path(&record.managed_path);
    let user_root = normalize_lexical_path(&paths.user_skills_root);
    if !managed_path.starts_with(&user_root) {
        return Err(format!(
            "Refusing to remove managed path outside user skills root: {}",
            record.managed_path.display()
        ));
    }

    match fs::symlink_metadata(&record.managed_path) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {
            fs::remove_dir_all(&record.managed_path).map_err(|error| error.to_string())?;
            remove_skill_index(&paths.database_path, &record.skill_name)?;
            Ok(Some(record.managed_path.clone()))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            remove_skill_index(&paths.database_path, &record.skill_name)?;
            Ok(None)
        }
        Ok(_) => Err(format!(
            "Refusing to remove non-directory managed user skill: {}",
            record.managed_path.display()
        )),
        Err(error) => Err(error.to_string()),
    }
}

fn validate_import_backup(record: &ImportRecord) -> Result<()> {
    let backup_skill = read_skill(&record.backup_path)?;
    if backup_skill.name != record.skill_name {
        return Err(format!(
            "Backup skill name does not match {}",
            record.skill_name
        ));
    }
    if backup_skill.content_hash != record.content_hash {
        return Err("Backup content hash does not match import record.".to_string());
    }
    Ok(())
}

fn validate_import_source_path(record: &ImportRecord) -> Result<()> {
    match fs::symlink_metadata(&record.source_path) {
        Ok(metadata) => {
            if !metadata.file_type().is_symlink() {
                return Err(format!(
                    "Refusing to replace existing non-symlink source: {}",
                    record.source_path.display()
                ));
            }
            if !symlink_points_to_import_target(&record.source_path, record)? {
                return Err(format!(
                    "Refusing to remove symlink pointing elsewhere: {}",
                    record.source_path.display()
                ));
            }
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

fn symlink_points_to_import_target(symlink: &Path, record: &ImportRecord) -> Result<bool> {
    let target = fs::read_link(symlink).map_err(|error| error.to_string())?;
    let absolute_target = if target.is_absolute() {
        target
    } else {
        symlink
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .join(target)
    };
    let normalized_target = normalize_lexical_path(&absolute_target);
    let normalized_expected = normalize_lexical_path(&record.managed_path);
    if normalized_target == normalized_expected {
        return Ok(true);
    }

    match (
        fs::canonicalize(&absolute_target),
        fs::canonicalize(&record.managed_path),
    ) {
        (Ok(target), Ok(expected)) => Ok(target == expected),
        _ => Ok(false),
    }
}

fn reconcile_legacy_import_records(
    paths: &ManagedPaths,
    filter: &ImportRecordFilter,
) -> Result<()> {
    let deployments = load_deployments(&paths.database_path)?;
    let existing_records = load_import_records(&paths.database_path, filter)?;
    let existing_sources: HashSet<PathBuf> = existing_records
        .iter()
        .map(|record| record.source_path.clone())
        .collect();

    for (skill_name, skill_deployments) in deployments {
        if filter
            .skill_name
            .as_ref()
            .map(|filter_name| filter_name != &skill_name)
            .unwrap_or(false)
        {
            continue;
        }
        if skill_deployments.len() != 1 {
            continue;
        }
        let deployment = &skill_deployments[0];
        if existing_sources.contains(&deployment.target_path) {
            continue;
        }
        let Some((kind, managed_path, skill)) = managed_import_record_skill(paths, &skill_name)?
        else {
            continue;
        };
        let backup_candidates = matching_legacy_import_backups(paths, &skill)?;
        if backup_candidates.len() != 1 {
            continue;
        }
        let record = new_import_record(
            &skill.name,
            kind,
            &deployment.target_path,
            Some(&deployment.target_root),
            &managed_path,
            &skill.content_hash,
            &backup_candidates[0],
            &deployment.target_path,
            true,
        );
        if validate_import_source_path(&record).is_ok() {
            insert_import_record(&paths.database_path, &record)?;
        }
    }

    Ok(())
}

fn managed_import_record_skill(
    paths: &ManagedPaths,
    skill_name: &str,
) -> Result<Option<(SkillKind, PathBuf, Skill)>> {
    let user_path = paths.user_skills_root.join(skill_name);
    if user_path.join("SKILL.md").exists() {
        let skill = read_skill(&user_path)?;
        return Ok(Some((SkillKind::User, user_path, skill)));
    }

    let remote_current = paths.remote_skills_root.join(skill_name).join("current");
    if remote_current.join("SKILL.md").exists() {
        let skill = read_skill(&remote_current)?;
        return Ok(Some((SkillKind::Remote, remote_current, skill)));
    }

    Ok(None)
}

fn matching_legacy_import_backups(paths: &ManagedPaths, skill: &Skill) -> Result<Vec<PathBuf>> {
    let backups_root = paths.root.join("backups").join("imports");
    let mut backups = Vec::new();
    let Ok(entries) = fs::read_dir(&backups_root) else {
        return Ok(backups);
    };

    for entry in entries {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Ok(backup_skill) = read_skill(&path) else {
            continue;
        };
        if backup_skill.name == skill.name && backup_skill.content_hash == skill.content_hash {
            backups.push(path);
        }
    }
    backups.sort();
    Ok(backups)
}

pub(crate) fn infer_import_candidate_type(
    skill: &Skill,
    paths: &ManagedPaths,
) -> (SkillKind, String, bool) {
    let path = skill.path.to_string_lossy();

    if path.contains("/.codex/skills/.system/") || path.ends_with("/.codex/skills/.system") {
        return (
            SkillKind::Remote,
            "inside ~/.codex/skills/.system".to_string(),
            false,
        );
    }

    if path.contains("/.agents/skills/") || path.ends_with("/.agents/skills") {
        return (SkillKind::User, "inside ~/.agents/skills".to_string(), true);
    }

    if path.contains("/.codex/skills/") || path.ends_with("/.codex/skills") {
        return (
            SkillKind::Remote,
            "inside ~/.codex/skills".to_string(),
            true,
        );
    }

    if skill_declares_github_source(&skill.skill_md_path) {
        return (
            SkillKind::Remote,
            "GitHub source metadata found".to_string(),
            true,
        );
    }

    if is_under_path(&skill.real_path, &paths.user_skills_root) {
        return (SkillKind::User, "inside user skill root".to_string(), true);
    }

    (SkillKind::User, "Needs confirm".to_string(), true)
}

pub(crate) fn is_system_skill(skill: &Skill) -> bool {
    let path = skill.path.to_string_lossy();
    path.contains("/.codex/skills/.system/") || path.ends_with("/.codex/skills/.system")
}

pub(crate) fn imported_candidate_reason(skill: &Skill, paths: &ManagedPaths) -> String {
    if skill.is_symlink && is_under_path(&skill.real_path, &paths.root) {
        return "Imported; source links to SkillBox".to_string();
    }

    "Already imported in SkillBox".to_string()
}

pub(crate) fn skill_declares_github_source(skill_md_path: &Path) -> bool {
    fs::read_to_string(skill_md_path)
        .map(|content| content.to_lowercase().contains("github.com/"))
        .unwrap_or(false)
}

pub(crate) fn managed_target_conflict(
    paths: &ManagedPaths,
    skill: &Skill,
    kind: SkillKind,
) -> Result<Option<String>> {
    match kind {
        SkillKind::User => {
            let target = paths.user_skills_root.join(&skill.name);
            if !target.exists() {
                return Ok(None);
            }
            if read_skill(&target)
                .map(|existing| existing.content_hash == skill.content_hash)
                .unwrap_or(false)
            {
                return Ok(None);
            }
            Ok(Some(format!("Managed target exists: {}", target.display())))
        }
        SkillKind::Remote => {
            let remote_root = paths.remote_skills_root.join(&skill.name);
            let version_target = remote_root
                .join("versions")
                .join(format!("manual-{}", &skill.content_hash[..12]));
            if version_target.exists() {
                return Ok(None);
            }
            if remote_root.exists() && !remote_root.is_dir() {
                return Ok(Some(format!(
                    "Managed target exists: {}",
                    remote_root.display()
                )));
            }
            Ok(None)
        }
    }
}

pub(crate) fn replace_source_with_symlink(
    source_path: &Path,
    target_path: &Path,
    paths: &ManagedPaths,
    skill_name: &str,
    content_hash: &str,
) -> Result<Option<PathBuf>> {
    let metadata = fs::symlink_metadata(source_path).map_err(|error| error.to_string())?;
    if metadata.file_type().is_symlink() {
        let linked = fs::canonicalize(source_path).map_err(|error| error.to_string())?;
        let expected = fs::canonicalize(target_path).map_err(|error| error.to_string())?;
        if linked == expected {
            return Ok(None);
        }
        return Err(format!(
            "Refusing to replace symlink pointing elsewhere: {}",
            source_path.display()
        ));
    }

    let backup_path = unique_backup_path(paths, skill_name, content_hash);
    if let Some(parent) = backup_path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }

    fs::rename(source_path, &backup_path).map_err(|error| error.to_string())?;
    if let Err(error) = symlink_dir(target_path, source_path) {
        let _ = fs::rename(&backup_path, source_path);
        return Err(error);
    }

    Ok(Some(backup_path))
}

pub(crate) fn unique_backup_path(
    paths: &ManagedPaths,
    skill_name: &str,
    content_hash: &str,
) -> PathBuf {
    let hash = &content_hash[..12];
    let base = paths
        .root
        .join("backups")
        .join("imports")
        .join(format!("{skill_name}-{hash}"));
    if !base.exists() {
        return base;
    }

    for index in 2..=10_000 {
        let candidate = paths
            .root
            .join("backups")
            .join("imports")
            .join(format!("{skill_name}-{hash}-{index}"));
        if !candidate.exists() {
            return candidate;
        }
    }

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    paths
        .root
        .join("backups")
        .join("imports")
        .join(format!("{skill_name}-{hash}-{nanos}"))
}

pub(crate) fn is_under_path(path: &Path, root: &Path) -> bool {
    let path = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let root = fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    path.starts_with(root)
}
