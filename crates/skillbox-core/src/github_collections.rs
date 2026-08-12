use crate::*;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::Instant;

const MAX_REMOTE_COLLECTION_CHILDREN: usize = 500;
const MAX_REMOTE_COLLECTION_ENTRIES: usize = skillbox_git::MAX_STRICT_TREE_ENTRIES;
const MAX_REMOTE_COLLECTION_FILE_BYTES: u64 = skillbox_git::MAX_STRICT_TREE_FILE_BYTES;
const MAX_REMOTE_COLLECTION_TOTAL_BYTES: u64 = skillbox_git::MAX_STRICT_TREE_TOTAL_BYTES;
const MAX_REMOTE_COLLECTION_DEPTH: usize = 12;

pub fn preview_github_skill_collection(
    request: PreviewGithubSkillCollectionRequest,
    managed_root: impl AsRef<Path>,
) -> Result<GithubSkillCollectionPreview> {
    let source = skillbox_github::parse_github_skill_url(&request.source_url)?;
    if !source.is_root {
        return Err(
            "This URL points to one skill. Use the single-skill GitHub install preview for it."
                .to_string(),
        );
    }
    if !source.reference_explicit {
        return Err(
            "GitHub repository URLs need an explicit ref for collection preview. Use /tree/<ref> so the reviewed commit is unambiguous."
                .to_string(),
        );
    }

    let started = Instant::now();
    let temp = temporary_work_dir("github-collection-preview");
    let result = (|| {
        let checkout = temp.join("checkout");
        let git = skillbox_git::GitService::new();
        let fetch =
            git.fetch_ref_tree_with_diagnostics(&source.repo_url, &source.reference, &checkout)?;
        let paths = managed_paths(managed_root.as_ref().to_path_buf());
        build_github_skill_collection_preview(
            &source,
            &fetch.resolved_sha,
            &checkout,
            &paths,
            fetch.fetch_count,
            started,
        )
    })();
    let _ = fs::remove_dir_all(&temp);
    result
}

pub fn preview_github_skill_collection_result(
    request: PreviewGithubSkillCollectionRequest,
    managed_root: impl AsRef<Path>,
) -> Result<GithubSkillCollectionPreviewResult> {
    let source = skillbox_github::parse_github_skill_url(&request.source_url)?;
    if !source.is_root {
        return Ok(GithubSkillCollectionPreviewResult::SingleSkill {
            message:
                "This URL points to one skill. Use the single-skill GitHub install preview for it."
                    .to_string(),
        });
    }
    if !source.reference_explicit {
        return Ok(GithubSkillCollectionPreviewResult::ExplicitReferenceRequired {
            message: "GitHub repository URLs need an explicit ref for collection preview. Use /tree/<ref> so the reviewed commit is unambiguous."
                .to_string(),
        });
    }
    Ok(GithubSkillCollectionPreviewResult::Collection {
        preview: Box::new(preview_github_skill_collection(request, managed_root)?),
    })
}

pub fn apply_github_skill_collection(
    request: GithubSkillCollectionApplyRequest,
    managed_root: impl AsRef<Path>,
) -> Result<ImportCollectionApplyResult> {
    if request.selections.is_empty() {
        return Err("Select at least one skill from the collection.".to_string());
    }

    let mutation_lock = acquire_user_skills_mutation_lock(managed_root.as_ref())?;
    let truth_root = mutation_lock.truth_root().to_path_buf();
    let paths = managed_paths(truth_root.clone());
    let source = skillbox_github::parse_github_skill_url(&request.source_url)?;
    if !source.is_root {
        return Err(
            "This URL points to one skill. Use the single-skill GitHub install flow for it."
                .to_string(),
        );
    }
    if !source.reference_explicit {
        return Err(
            "GitHub repository URLs need an explicit ref for collection apply. Re-open preview with /tree/<ref>."
                .to_string(),
        );
    }

    let temp = temporary_work_dir("github-collection-apply");
    let result = (|| {
        let checkout = temp.join("checkout");
        let git = skillbox_git::GitService::new();
        let fetch =
            git.fetch_ref_tree_with_diagnostics(&source.repo_url, &source.reference, &checkout)?;
        let preview = build_github_skill_collection_preview(
            &source,
            &fetch.resolved_sha,
            &checkout,
            &paths,
            fetch.fetch_count,
            Instant::now(),
        )?;
        let collection = &preview.collection;
        if collection.id != request.collection_id
            || collection.preview_id != request.preview_id
            || collection.source_url.as_deref() != Some(source.url.as_str())
        {
            return Err(
                "GitHub collection preview is stale. Re-open the preview and try again."
                    .to_string(),
            );
        }
        if let Some(existing_reviewed_sha) =
            persisted_collection_reviewed_sha(&paths.database_path, &collection.id)?
        {
            if existing_reviewed_sha.as_ref() != collection.reviewed_head_sha.as_ref() {
                return Err(
                    "Collection already has a different reviewed SHA; collection updates are not available until Phase D."
                        .to_string(),
                );
            }
        }

        let mut selected_names = HashSet::new();
        let mut items = Vec::new();
        let mut selected_children = Vec::new();
        for selection in &request.selections {
            let child = collection
                .children
                .iter()
                .find(|child| child.relative_path == selection.relative_path)
                .ok_or_else(|| {
                    "Selected GitHub collection child is not part of the reviewed preview."
                        .to_string()
                })?;
            if child.group_id != selection.group_id
                || child.variant_id != selection.variant_id
                || child.import_status != ImportCandidateStatus::Importable
                || child.conflict.is_some()
            {
                return Err(format!(
                    "Selected GitHub collection child {} is stale or not importable.",
                    child.name
                ));
            }
            if !selected_names.insert(child.name.to_ascii_lowercase()) {
                return Err(format!(
                    "Import review may select only one source variant for skill {}.",
                    child.name
                ));
            }
            let source_path = checkout.join(&child.relative_path);
            let skill = read_skill(&source_path)
                .map_err(|error| format!("GitHub collection preview is stale: {error}"))?;
            validate_skill_name(&skill.name)?;
            if skill.name != child.name
                || skill.content_hash != child.content_hash
                || skill_directory_snapshot_hash(&source_path)? != child.snapshot_hash
            {
                return Err(
                    "GitHub collection preview is stale. Re-open the preview and try again."
                        .to_string(),
                );
            }
            let target = collection_import_target(&paths, &skill, selection.skill_type);
            if managed_index_contains(&paths.database_path, &skill.name)?
                || fs::symlink_metadata(&target.path).is_ok()
                || target
                    .remote_root
                    .as_ref()
                    .is_some_and(|path| fs::symlink_metadata(path).is_ok())
            {
                return Err(format!(
                    "Managed target for {} changed after preview. Review the collection again.",
                    skill.name
                ));
            }
            items.push(ImportRequestItem {
                source_path,
                skill_type: selection.skill_type,
                deploy_back_to_source: false,
            });
            selected_children.push(child.clone());
        }

        // Keep layout/database creation after source, selection, snapshot, and
        // managed-target validation so stale previews do not initialize state.
        let paths = ensure_managed_layout(truth_root.clone())?;
        apply_collection_import_with_audit(
            &paths,
            collection,
            &selected_children,
            items,
            &request.actor,
        )
    })();
    let _ = fs::remove_dir_all(&temp);
    result
}

fn build_github_skill_collection_preview(
    source: &skillbox_github::GitHubSkillSource,
    resolved_sha: &str,
    checkout: &Path,
    paths: &ManagedPaths,
    fetch_count: usize,
    started: Instant,
) -> Result<GithubSkillCollectionPreview> {
    let checkout = fs::canonicalize(checkout).map_err(|error| error.to_string())?;
    let mut skill_dirs = Vec::new();
    let mut errors = Vec::new();
    let mut entry_count = 0;
    let mut total_bytes = 0;
    if is_regular_file(&checkout.join("SKILL.md")) {
        skill_dirs.push(checkout.to_path_buf());
    }
    walk_remote_tree(
        &checkout,
        &checkout,
        0,
        &mut entry_count,
        &mut total_bytes,
        &mut skill_dirs,
        &mut errors,
    )?;
    skill_dirs.sort();
    skill_dirs.dedup();
    if skill_dirs.iter().enumerate().any(|(index, parent)| {
        skill_dirs[index + 1..]
            .iter()
            .any(|child| child.starts_with(parent))
    }) {
        return Err(
            "GitHub collection contains overlapping skill directories. Keep SKILL.md roots disjoint."
                .to_string(),
        );
    }
    if skill_dirs.is_empty() {
        return Err("The repository contains no valid SKILL.md directories.".to_string());
    }
    if skill_dirs.len() > MAX_REMOTE_COLLECTION_CHILDREN {
        return Err(format!(
            "GitHub collection contains {} skill directories, exceeding the safety limit of {}. Narrow the repository or select a smaller source.",
            skill_dirs.len(),
            MAX_REMOTE_COLLECTION_CHILDREN
        ));
    }

    let usage_by_skill = if paths.database_path.is_file() {
        load_usage_by_skill(&paths.database_path).unwrap_or_default()
    } else {
        HashMap::new()
    };
    let mut candidates = Vec::new();
    for skill_dir in &skill_dirs {
        match read_remote_skill(skill_dir) {
            Ok(skill) => {
                let conflict = managed_target_conflict(paths, &skill, SkillKind::Remote)?;
                let imported = managed_skill_matches(paths, &skill);
                let status = if imported {
                    ImportCandidateStatus::Imported
                } else {
                    ImportCandidateStatus::Importable
                };
                candidates.push(ImportCandidate {
                    name: skill.name,
                    description: skill.description,
                    source_path: skill.path.clone(),
                    source_root: Some(checkout.to_path_buf()),
                    real_path: skill.real_path,
                    is_symlink: false,
                    symlink_target_path: None,
                    content_hash: skill.content_hash,
                    additional_source_paths: Vec::new(),
                    suggested_type: SkillKind::Remote,
                    suggestion_reason: "GitHub repository source".to_string(),
                    import_status: status,
                    is_selected: status == ImportCandidateStatus::Importable && conflict.is_none(),
                    conflict,
                    usage_count: 0,
                });
            }
            Err(error) => errors.push(ImportCandidateError {
                source_path: safe_relative_display(&checkout, skill_dir),
                error,
            }),
        }
    }
    if candidates.is_empty() {
        return Err("The repository contains no valid SKILL.md directories.".to_string());
    }

    let (mut groups, _) = group_import_candidates(&candidates, &usage_by_skill);
    let mut children = Vec::new();
    let mut relative_paths_by_name = HashMap::<String, usize>::new();
    for group in &groups {
        for variant in &group.variants {
            for location in &variant.locations {
                let relative_path = safe_collection_relative_path(&checkout, &location.real_path)
                    .ok_or_else(|| {
                    "Remote collection contained an unsafe child path.".to_string()
                })?;
                let relative_path = relative_path.to_string_lossy().to_string();
                *relative_paths_by_name
                    .entry(variant.candidate.name.to_ascii_lowercase())
                    .or_default() += 1;
                let snapshot_hash = if variant.snapshot_hash.is_empty() {
                    skill_directory_snapshot_hash(&location.real_path)?
                } else {
                    variant.snapshot_hash.clone()
                };
                let diff_path = if relative_path.is_empty() {
                    "SKILL.md".to_string()
                } else {
                    format!("{relative_path}/SKILL.md")
                };
                let diff = new_file_diff(&checkout, &diff_path).unwrap_or_default();
                children.push(ImportCandidateCollectionChild {
                    id: format!(
                        "child-{}",
                        &sha256(&format!("{}\n{}", group.id, relative_path))[..16]
                    ),
                    group_id: group.id.clone(),
                    variant_id: variant.id.clone(),
                    name: variant.candidate.name.clone(),
                    relative_path,
                    source_path: location.source_path.clone(),
                    real_path: location.real_path.clone(),
                    content_hash: variant.candidate.content_hash.clone(),
                    snapshot_hash,
                    diff,
                    import_status: variant.candidate.import_status,
                    conflict: variant.candidate.conflict.clone(),
                    usage_count: group.usage_count,
                    locations: vec![location.clone()],
                    unlinked_locations: Vec::new(),
                    suggested_types: variant.suggested_types.clone(),
                    requires_type_review: variant.requires_type_review,
                    selected_type: variant.selected_type,
                    is_selected: variant.candidate.is_selected,
                });
            }
        }
    }
    for child in &mut children {
        if relative_paths_by_name
            .get(&child.name.to_ascii_lowercase())
            .copied()
            .unwrap_or_default()
            > 1
        {
            child.conflict = Some("Duplicate skill name in repository".to_string());
            child.is_selected = false;
        }
    }
    sanitize_remote_group_paths(&mut groups, &checkout);
    sanitize_remote_collection_children(&mut children, &checkout);
    children.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    let duplicate_name_count = relative_paths_by_name
        .values()
        .filter(|count| **count > 1)
        .count();
    let child_seed = children
        .iter()
        .map(|child| {
            format!(
                "{}\n{}\n{}\n{:?}\n{}",
                child.relative_path,
                child.name,
                child.snapshot_hash,
                child.import_status,
                child.conflict.as_deref().unwrap_or_default()
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let source_url = sanitize_origin_url(&source.url);
    let collection_identity = format!(
        "skillbox-github-collection-identity-v1\n{}\n{}\n{}",
        "github_remote", source_url, source.reference
    );
    let collection_id = format!("github-collection-{}", &sha256(&collection_identity)[..16]);
    let preview_identity = format!(
        "skillbox-github-collection-preview-v1\n{}\n{}\n{}\n{}",
        source_url, source.reference, resolved_sha, child_seed
    );
    let preview_id = format!("github-collection-preview-{}", sha256(&preview_identity));
    let collection = ImportCandidateCollection {
        id: collection_id,
        preview_id,
        display_name: source.repo.clone(),
        source_kind: ImportCandidateCollectionSourceKind::GithubRemote,
        canonical_worktree_root: PathBuf::new(),
        canonical_repository_id: PathBuf::from(source.repo_url.clone()),
        origin_url: Some(source_url.clone()),
        branch: Some(source.reference.clone()),
        detached: false,
        reviewed_head_sha: Some(resolved_sha.to_string()),
        source_url: Some(source_url),
        requested_reference: Some(source.reference.clone()),
        children,
        errors,
    };
    let diagnostics = GithubSkillCollectionDiagnostics {
        fetch_count,
        child_count: skill_dirs.len(),
        valid_child_count: candidates.len(),
        invalid_child_count: collection.errors.len(),
        duplicate_name_count,
        elapsed_ms: started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
    };
    let preview_errors = collection.errors.clone();
    Ok(GithubSkillCollectionPreview {
        collection,
        groups,
        errors: preview_errors,
        diagnostics,
    })
}

fn sanitize_remote_group_paths(groups: &mut [ImportCandidateGroup], checkout: &Path) {
    for group in groups {
        for variant in &mut group.variants {
            let relative_candidate_path =
                safe_relative_display(checkout, &variant.candidate.real_path);
            variant.candidate.source_path = relative_candidate_path.clone();
            variant.candidate.source_root = Some(PathBuf::from("."));
            variant.candidate.real_path = relative_candidate_path.clone();
            variant.candidate.additional_source_paths = variant
                .candidate
                .additional_source_paths
                .iter()
                .map(|path| safe_relative_display(checkout, path))
                .collect();
            for location in &mut variant.locations {
                let relative = safe_relative_display(checkout, &location.real_path);
                location.source_path = relative.clone();
                location.source_root = Some(PathBuf::from("."));
                location.real_path = relative;
                location.symlink_target_path = None;
            }
        }
    }
}

fn sanitize_remote_collection_children(
    children: &mut [ImportCandidateCollectionChild],
    checkout: &Path,
) {
    for child in children {
        child.source_path = safe_relative_display(checkout, &child.real_path);
        child.real_path = child.source_path.clone();
        for location in &mut child.locations {
            let relative = safe_relative_display(checkout, &location.real_path);
            location.source_path = relative.clone();
            location.source_root = Some(PathBuf::from("."));
            location.real_path = relative;
            location.symlink_target_path = None;
        }
        for location in &mut child.unlinked_locations {
            let relative = safe_relative_display(checkout, &location.real_path);
            location.source_path = relative.clone();
            location.source_root = Some(PathBuf::from("."));
            location.real_path = relative;
            location.symlink_target_path = None;
        }
    }
}

fn walk_remote_tree(
    root: &Path,
    current: &Path,
    depth: usize,
    entry_count: &mut usize,
    total_bytes: &mut u64,
    skill_dirs: &mut Vec<PathBuf>,
    errors: &mut Vec<ImportCandidateError>,
) -> Result<()> {
    if depth > MAX_REMOTE_COLLECTION_DEPTH {
        return Err("GitHub collection tree exceeds the directory depth safety limit.".to_string());
    }
    let entries = fs::read_dir(current).map_err(|error| error.to_string())?;
    for entry in entries {
        *entry_count += 1;
        if *entry_count > MAX_REMOTE_COLLECTION_ENTRIES {
            return Err("GitHub collection tree exceeds the entry safety limit.".to_string());
        }
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        if safe_collection_relative_path(root, &path).is_none()
            || path
                .strip_prefix(root)
                .ok()
                .map(|relative| {
                    relative.components().any(|component| {
                        matches!(component, Component::Normal(value) if value == ".git" || value.to_string_lossy().contains(':'))
                    })
                })
                .unwrap_or(true)
        {
            return Err("GitHub collection tree contains an unsafe path.".to_string());
        }
        let file_type = entry.file_type().map_err(|error| error.to_string())?;
        if file_type.is_symlink() {
            errors.push(ImportCandidateError {
                source_path: safe_relative_display(root, &path),
                error: "Symlinks are not supported in GitHub skill collections.".to_string(),
            });
            continue;
        }
        if file_type.is_dir() {
            if entry.file_name() == ".git" {
                return Err("Git metadata is not allowed in a GitHub collection tree.".to_string());
            }
            if is_regular_file(&path.join("SKILL.md")) {
                skill_dirs.push(path.clone());
            }
            walk_remote_tree(
                root,
                &path,
                depth + 1,
                entry_count,
                total_bytes,
                skill_dirs,
                errors,
            )?;
        } else if file_type.is_file() {
            if entry.file_name() == ".git" {
                return Err("Git metadata is not allowed in a GitHub collection tree.".to_string());
            }
            let length = entry.metadata().map_err(|error| error.to_string())?.len();
            if length > MAX_REMOTE_COLLECTION_FILE_BYTES {
                return Err("GitHub collection contains a file over the safety limit.".to_string());
            }
            *total_bytes = total_bytes.saturating_add(length);
            if *total_bytes > MAX_REMOTE_COLLECTION_TOTAL_BYTES {
                return Err("GitHub collection exceeds the total byte safety limit.".to_string());
            }
        } else {
            return Err("GitHub collection contains an unsupported file type.".to_string());
        }
    }
    Ok(())
}

fn read_remote_skill(path: &Path) -> Result<Skill> {
    let skill_md = path.join("SKILL.md");
    if !is_regular_file(&skill_md) {
        return Err("SKILL.md must be a regular non-symlink file.".to_string());
    }
    validate_remote_skill_tree(path)?;
    let content = fs::read_to_string(&skill_md).map_err(|error| error.to_string())?;
    parse_skill_frontmatter_document(&content)?;
    let skill = read_skill(path)?;
    validate_skill_name(&skill.name)?;
    Ok(skill)
}

fn validate_remote_skill_tree(root: &Path) -> Result<()> {
    let mut entries = vec![root.to_path_buf()];
    while let Some(current) = entries.pop() {
        for entry in fs::read_dir(&current).map_err(|error| error.to_string())? {
            let entry = entry.map_err(|error| error.to_string())?;
            let file_type = entry.file_type().map_err(|error| error.to_string())?;
            if file_type.is_symlink() {
                return Err("Skill directories may not contain symlinks.".to_string());
            }
            if file_type.is_dir() {
                entries.push(entry.path());
            } else if !file_type.is_file() {
                return Err("Skill directories may not contain unsupported file types.".to_string());
            }
        }
    }
    Ok(())
}

fn is_regular_file(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_file())
        .unwrap_or(false)
}

fn managed_skill_matches(paths: &ManagedPaths, skill: &Skill) -> bool {
    [
        paths.user_skills_root.join(&skill.name),
        paths.remote_skills_root.join(&skill.name).join("current"),
    ]
    .iter()
    .any(|path| {
        read_skill(path)
            .ok()
            .is_some_and(|managed| managed.content_hash == skill.content_hash)
    })
}

fn safe_collection_relative_path(root: &Path, path: &Path) -> Option<PathBuf> {
    let relative = path.strip_prefix(root).ok()?;
    for component in relative.components() {
        match component {
            Component::Normal(name) => {
                let name = name.to_string_lossy();
                if name == ".git"
                    || name.contains(':')
                    || name.contains('\\')
                    || name.chars().any(char::is_control)
                {
                    return None;
                }
            }
            _ => return None,
        }
    }
    Some(relative.to_path_buf())
}

fn safe_relative_display(root: &Path, path: &Path) -> PathBuf {
    safe_collection_relative_path(root, path).unwrap_or_else(|| PathBuf::from("<invalid>"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source() -> skillbox_github::GitHubSkillSource {
        skillbox_github::GitHubSkillSource {
            owner: "acme".to_string(),
            repo: "skills".to_string(),
            reference: "main".to_string(),
            reference_explicit: true,
            path: String::new(),
            is_root: true,
            url: "https://github.com/acme/skills".to_string(),
            repo_url: "https://github.com/acme/skills.git".to_string(),
            kind: "github".to_string(),
        }
    }

    fn write_skill(root: &Path, name: &str) {
        let path = root.join("skills").join(name);
        fs::create_dir_all(&path).unwrap();
        fs::write(
            path.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: Test skill\n---\n"),
        )
        .unwrap();
    }

    fn write_root_skill(root: &Path, name: &str) {
        fs::create_dir_all(root).unwrap();
        fs::write(
            root.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: Root skill\n---\n"),
        )
        .unwrap();
    }

    #[test]
    fn preview_groups_many_children_without_managed_writes() {
        let temp = temporary_work_dir("github-collection-test");
        let checkout = temp.join("checkout");
        write_skill(&checkout, "alpha");
        write_skill(&checkout, "beta");
        let paths = managed_paths(temp.join("managed"));

        let preview = build_github_skill_collection_preview(
            &source(),
            "0123456789012345678901234567890123456789",
            &checkout,
            &paths,
            1,
            Instant::now(),
        )
        .unwrap();

        assert_eq!(
            preview.collection.source_kind,
            ImportCandidateCollectionSourceKind::GithubRemote
        );
        assert_eq!(preview.collection.children.len(), 2);
        assert_eq!(preview.diagnostics.fetch_count, 1);
        assert!(preview
            .collection
            .children
            .iter()
            .all(|child| child.diff.contains("SKILL.md")));
        assert!(!paths.database_path.exists());
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn preview_scales_to_one_hundred_children_with_one_fetch_diagnostic() {
        let temp = temporary_work_dir("github-collection-large-fixture");
        let checkout = temp.join("checkout");
        for index in 0..100 {
            write_skill(&checkout, &format!("skill-{index:03}"));
        }
        let paths = managed_paths(temp.join("managed"));

        let preview = build_github_skill_collection_preview(
            &source(),
            "0123456789012345678901234567890123456789",
            &checkout,
            &paths,
            1,
            Instant::now(),
        )
        .unwrap();

        assert_eq!(preview.collection.children.len(), 100);
        assert_eq!(preview.diagnostics.fetch_count, 1);
        assert_eq!(preview.diagnostics.child_count, 100);
        assert_eq!(preview.diagnostics.valid_child_count, 100);
        assert!(preview.diagnostics.elapsed_ms < 30_000);
        assert!(!paths.database_path.exists());
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn preview_rejects_more_than_five_hundred_children_without_partial_collection() {
        let temp = temporary_work_dir("github-collection-child-limit");
        let checkout = temp.join("checkout");
        for index in 0..=MAX_REMOTE_COLLECTION_CHILDREN {
            write_skill(&checkout, &format!("skill-{index:03}"));
        }
        let paths = managed_paths(temp.join("managed"));

        let error = build_github_skill_collection_preview(
            &source(),
            "0123456789012345678901234567890123456789",
            &checkout,
            &paths,
            1,
            Instant::now(),
        )
        .unwrap_err();

        assert!(error.contains("exceeding the safety limit"), "{error}");
        assert!(!paths.database_path.exists());
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn preview_result_routes_single_skill_and_bare_repository_structurally() {
        let single = preview_github_skill_collection_result(
            PreviewGithubSkillCollectionRequest {
                source_url: "https://github.com/acme/repo/tree/main/skills/demo".to_string(),
            },
            temporary_work_dir("github-collection-route-single"),
        )
        .unwrap();
        let single_json = serde_json::to_value(single).unwrap();
        assert_eq!(single_json["kind"], "single_skill");
        assert!(single_json["message"]
            .as_str()
            .unwrap()
            .contains("single-skill"));

        let bare = preview_github_skill_collection_result(
            PreviewGithubSkillCollectionRequest {
                source_url: "https://github.com/acme/repo".to_string(),
            },
            temporary_work_dir("github-collection-route-bare"),
        )
        .unwrap();
        let bare_json = serde_json::to_value(bare).unwrap();
        assert_eq!(bare_json["kind"], "explicit_reference_required");
        assert!(bare_json["message"]
            .as_str()
            .unwrap()
            .contains("/tree/<ref>"));
    }

    #[test]
    fn preview_keeps_valid_children_when_one_child_is_malformed() {
        let temp = temporary_work_dir("github-collection-invalid-child-test");
        let checkout = temp.join("checkout");
        write_skill(&checkout, "valid");
        let invalid = checkout.join("skills/invalid");
        fs::create_dir_all(&invalid).unwrap();
        fs::write(invalid.join("SKILL.md"), "---\nname: invalid\n").unwrap();
        let paths = managed_paths(temp.join("managed"));

        let preview = build_github_skill_collection_preview(
            &source(),
            "0123456789012345678901234567890123456789",
            &checkout,
            &paths,
            1,
            Instant::now(),
        )
        .unwrap();

        assert_eq!(preview.collection.children.len(), 1);
        assert_eq!(preview.collection.children[0].name, "valid");
        assert_eq!(preview.diagnostics.invalid_child_count, 1);
        assert!(preview
            .errors
            .iter()
            .any(|error| error.source_path == Path::new("skills/invalid")));
        assert!(!paths.database_path.exists());
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn preview_rejects_empty_repository_without_managed_writes() {
        let temp = temporary_work_dir("github-collection-empty-test");
        let checkout = temp.join("checkout");
        fs::create_dir_all(&checkout).unwrap();
        fs::write(checkout.join("README.md"), "no skills\n").unwrap();
        let paths = managed_paths(temp.join("managed"));

        let error = build_github_skill_collection_preview(
            &source(),
            "0123456789012345678901234567890123456789",
            &checkout,
            &paths,
            1,
            Instant::now(),
        )
        .unwrap_err();

        assert!(error.contains("no valid SKILL.md"));
        assert!(!paths.database_path.exists());
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn preview_supports_repository_root_skill_without_creating_state() {
        let temp = temporary_work_dir("github-collection-root-skill-test");
        let checkout = temp.join("checkout");
        write_root_skill(&checkout, "root-skill");
        let paths = managed_paths(temp.join("managed"));

        let preview = build_github_skill_collection_preview(
            &source(),
            "0123456789012345678901234567890123456789",
            &checkout,
            &paths,
            1,
            Instant::now(),
        )
        .unwrap();

        assert_eq!(preview.collection.children.len(), 1);
        assert_eq!(preview.collection.children[0].relative_path, "");
        assert!(preview.collection.children[0].diff.contains("SKILL.md"));
        assert!(!paths.database_path.exists());
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn preview_rejects_root_and_nested_skill_overlap() {
        let temp = temporary_work_dir("github-collection-root-overlap-test");
        let checkout = temp.join("checkout");
        write_root_skill(&checkout, "root-skill");
        write_skill(&checkout, "nested-skill");
        let paths = managed_paths(temp.join("managed"));

        let error = build_github_skill_collection_preview(
            &source(),
            "0123456789012345678901234567890123456789",
            &checkout,
            &paths,
            1,
            Instant::now(),
        )
        .unwrap_err();

        assert!(error.contains("overlapping skill directories"));
        assert!(!paths.database_path.exists());
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn preview_rejects_parent_and_nested_skill_overlap() {
        let temp = temporary_work_dir("github-collection-parent-overlap-test");
        let checkout = temp.join("checkout");
        write_skill(&checkout, "parent");
        write_skill(&checkout.join("skills/parent"), "nested");
        let paths = managed_paths(temp.join("managed"));

        let error = build_github_skill_collection_preview(
            &source(),
            "0123456789012345678901234567890123456789",
            &checkout,
            &paths,
            1,
            Instant::now(),
        )
        .unwrap_err();

        assert!(error.contains("overlapping skill directories"));
        assert!(!paths.database_path.exists());
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn preview_marks_duplicate_child_names_as_unselectable() {
        let temp = temporary_work_dir("github-collection-duplicate-name-test");
        let checkout = temp.join("checkout");
        write_skill(&checkout, "alpha");
        fs::create_dir_all(checkout.join("other/alpha")).unwrap();
        fs::write(
            checkout.join("other/alpha/SKILL.md"),
            "---\nname: alpha\ndescription: Duplicate\n---\n",
        )
        .unwrap();
        let paths = managed_paths(temp.join("managed"));

        let preview = build_github_skill_collection_preview(
            &source(),
            "0123456789012345678901234567890123456789",
            &checkout,
            &paths,
            1,
            Instant::now(),
        )
        .unwrap();

        assert_eq!(preview.collection.children.len(), 2);
        assert!(preview
            .collection
            .children
            .iter()
            .all(|child| child.conflict.as_deref() == Some("Duplicate skill name in repository")));
        assert_eq!(preview.diagnostics.duplicate_name_count, 1);
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn preview_keeps_valid_children_when_tree_contains_a_symlink_diagnostic() {
        let temp = temporary_work_dir("github-collection-symlink-test");
        let checkout = temp.join("checkout");
        write_skill(&checkout, "alpha");
        fs::write(checkout.join("README.md"), "readme").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink("outside", checkout.join("unsafe-link")).unwrap();
        let paths = managed_paths(temp.join("managed"));
        let preview = build_github_skill_collection_preview(
            &source(),
            "0123456789012345678901234567890123456789",
            &checkout,
            &paths,
            1,
            Instant::now(),
        )
        .unwrap();
        assert_eq!(preview.collection.children.len(), 1);
        assert_eq!(preview.collection.errors.len(), 1);
        let _ = fs::remove_dir_all(temp);
    }

    #[cfg(unix)]
    #[test]
    fn preview_rejects_symlink_inside_skill_but_keeps_other_valid_children() {
        let temp = temporary_work_dir("github-collection-skill-symlink-test");
        let checkout = temp.join("checkout");
        write_skill(&checkout, "alpha");
        write_skill(&checkout, "beta");
        std::os::unix::fs::symlink("outside", checkout.join("skills/alpha/reference.txt")).unwrap();
        let paths = managed_paths(temp.join("managed"));

        let preview = build_github_skill_collection_preview(
            &source(),
            "0123456789012345678901234567890123456789",
            &checkout,
            &paths,
            1,
            Instant::now(),
        )
        .unwrap();

        assert_eq!(preview.collection.children.len(), 1);
        assert_eq!(preview.collection.children[0].name, "beta");
        assert!(preview
            .errors
            .iter()
            .any(|error| error.error.contains("symlinks")));
        let _ = fs::remove_dir_all(temp);
    }
}
