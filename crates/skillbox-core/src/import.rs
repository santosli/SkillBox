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
        );

        candidates.push(ImportCandidate {
            name: skill.name,
            description: skill.description,
            source_path: skill.path,
            source_root: skill.source_root,
            real_path: skill.real_path,
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
) -> usize {
    if import_status == ImportCandidateStatus::Imported {
        return usage_by_skill
            .get(&skill.name)
            .map(|usage| usage.usage_count)
            .unwrap_or_default();
    }

    skill
        .source_root
        .as_ref()
        .and_then(|root| usage_by_skill_runtime.get(&(skill.name.clone(), usage_runtime_key(root))))
        .map(|usage| usage.usage_count)
        .unwrap_or_default()
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
