use crate::*;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

pub fn preview_github_skill_collection_update(
    request: PreviewGithubSkillCollectionRequest,
    managed_root: impl AsRef<Path>,
) -> Result<GithubCollectionUpdatePreviewResult> {
    let source = skillbox_github::parse_github_skill_url(&request.source_url)?;
    if !source.is_root {
        return Ok(GithubCollectionUpdatePreviewResult::SingleSkill {
            message:
                "This URL points to one skill. Use the single-skill GitHub install preview for it."
                    .to_string(),
        });
    }
    if !source.reference_explicit {
        return Ok(
            GithubCollectionUpdatePreviewResult::ExplicitReferenceRequired {
                message: "GitHub repository URLs need an explicit ref for collection preview. Use /tree/<ref> so the reviewed commit is unambiguous."
                    .to_string(),
            },
        );
    }

    let collection_id = github_collection_id(&source);
    let paths = managed_paths(expand_home(managed_root.as_ref().to_path_buf()));
    let persisted = list_skill_collections(&paths.root)?
        .into_iter()
        .find(|collection| collection.id == collection_id);
    let Some(persisted) = persisted else {
        return Ok(GithubCollectionUpdatePreviewResult::NotImported {
            message: "This GitHub collection is not imported yet. Use github-collection-preview to review a first install."
                .to_string(),
        });
    };
    if persisted.source_kind != ImportCandidateCollectionSourceKind::GithubRemote {
        return Err(
            "Only GitHub collections support collection-level update. Local worktree collections stay on collection-apply."
                .to_string(),
        );
    }

    let tree = fetch_github_collection_tree(&source, &paths)?;
    if persisted.reviewed_head_sha.as_ref() == tree.collection.reviewed_head_sha.as_ref() {
        return Ok(GithubCollectionUpdatePreviewResult::UpToDate {
            preview: Box::new(tree),
            message: "Collection is already at this reviewed SHA. Use github-collection-apply to import additional children."
                .to_string(),
        });
    }

    let preview = build_github_collection_update_preview(&paths, persisted, tree)?;
    Ok(GithubCollectionUpdatePreviewResult::Update {
        preview: Box::new(preview),
    })
}

pub fn apply_github_skill_collection_update(
    request: GithubSkillCollectionApplyRequest,
    managed_root: impl AsRef<Path>,
) -> Result<ImportCollectionApplyResult> {
    let mutation_lock = acquire_user_skills_mutation_lock(managed_root.as_ref())?;
    let truth_root = mutation_lock.truth_root().to_path_buf();
    let paths = ensure_managed_layout(truth_root.clone())?;
    let source = skillbox_github::parse_github_skill_url(&request.source_url)?;
    if !source.is_root {
        return Err(
            "This URL points to one skill. Use the single-skill GitHub install flow for it."
                .to_string(),
        );
    }
    if !source.reference_explicit {
        return Err(
            "GitHub repository URLs need an explicit ref for collection update. Re-open preview with /tree/<ref>."
                .to_string(),
        );
    }

    let temp = temporary_work_dir("github-collection-update");
    let result = (|| {
        let checkout = temp.join("checkout");
        let git = skillbox_git::GitService::new();
        let fetch =
            git.fetch_ref_tree_with_diagnostics(&source.repo_url, &source.reference, &checkout)?;
        let tree = build_github_skill_collection_preview(
            &source,
            &fetch.resolved_sha,
            &checkout,
            &paths,
            fetch.fetch_count,
            Instant::now(),
        )?;
        let persisted = list_skill_collections(&paths.root)?
            .into_iter()
            .find(|collection| collection.id == request.collection_id)
            .ok_or_else(|| {
                "GitHub collection is not imported. Use github-collection-apply for the first install."
                    .to_string()
            })?;
        if persisted.id != tree.collection.id {
            return Err(
                "GitHub collection preview is stale. Re-open the preview and try again."
                    .to_string(),
            );
        }
        let preview = build_github_collection_update_preview(&paths, persisted, tree)?;
        if preview.preview_id != request.preview_id
            || preview.collection_id != request.collection_id
            || preview.source_url != sanitize_origin_url(&source.url)
        {
            return Err(
                "GitHub collection update preview is stale. Re-open the preview and try again."
                    .to_string(),
            );
        }
        apply_github_collection_update_with_audit(&paths, &checkout, &preview, &request)
    })();
    let _ = fs::remove_dir_all(&temp);
    result
}

pub fn preview_github_skill_collection_rollback(
    request: GithubCollectionRollbackRequest,
    managed_root: impl AsRef<Path>,
) -> Result<GithubCollectionRollbackPreview> {
    let paths = managed_paths(expand_home(managed_root.as_ref().to_path_buf()));
    build_github_collection_rollback_preview(&paths, &request.collection_id)
}

pub fn apply_github_skill_collection_rollback(
    request: GithubCollectionRollbackRequest,
    managed_root: impl AsRef<Path>,
) -> Result<GithubCollectionRollbackResult> {
    if request.preview_id.trim().is_empty() {
        return Err(
            "Collection rollback preview is required. Run github-collection-rollback-preview first."
                .to_string(),
        );
    }
    let mutation_lock = acquire_user_skills_mutation_lock(managed_root.as_ref())?;
    let truth_root = mutation_lock.truth_root().to_path_buf();
    let paths = ensure_managed_layout(truth_root)?;
    let preview = build_github_collection_rollback_preview(&paths, &request.collection_id)?;
    if preview.preview_id != request.preview_id || preview.collection_id != request.collection_id {
        return Err(
            "Collection rollback preview is stale. Re-open the preview and try again.".to_string(),
        );
    }
    if !preview.errors.is_empty() {
        return Err(preview.errors.join(" "));
    }
    apply_github_collection_rollback_with_audit(&paths, &preview, &request.actor)
}

fn fetch_github_collection_tree(
    source: &skillbox_github::GitHubSkillSource,
    paths: &ManagedPaths,
) -> Result<GithubSkillCollectionPreview> {
    let started = Instant::now();
    let temp = temporary_work_dir("github-collection-update-preview");
    let result = (|| {
        let checkout = temp.join("checkout");
        let git = skillbox_git::GitService::new();
        let fetch =
            git.fetch_ref_tree_with_diagnostics(&source.repo_url, &source.reference, &checkout)?;
        build_github_skill_collection_preview(
            source,
            &fetch.resolved_sha,
            &checkout,
            paths,
            fetch.fetch_count,
            started,
        )
    })();
    let _ = fs::remove_dir_all(&temp);
    result
}

fn build_github_collection_update_preview(
    paths: &ManagedPaths,
    persisted: SkillCollection,
    mut tree: GithubSkillCollectionPreview,
) -> Result<GithubCollectionUpdatePreview> {
    let from_sha = persisted.reviewed_head_sha.clone().ok_or_else(|| {
        "Imported GitHub collection is missing a reviewed SHA and cannot be updated.".to_string()
    })?;
    let to_sha = tree
        .collection
        .reviewed_head_sha
        .clone()
        .ok_or_else(|| "GitHub collection preview did not resolve a commit SHA.".to_string())?;
    if from_sha == to_sha {
        return Err("Collection is already at this reviewed SHA.".to_string());
    }

    let changes = classify_github_collection_changes(paths, &persisted, &tree.collection)?;
    append_removed_collection_children(&mut tree, &changes);
    apply_update_selection_defaults(&mut tree, &changes);
    let change_seed = changes
        .iter()
        .map(|change| {
            format!(
                "{}\n{:?}\n{}\n{}\n{}\n{}",
                change.relative_path,
                change.change,
                change.from_snapshot_hash.as_deref().unwrap_or_default(),
                change.to_snapshot_hash.as_deref().unwrap_or_default(),
                change.conflict.as_deref().unwrap_or_default(),
                change.required
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let source_url = tree
        .collection
        .source_url
        .clone()
        .unwrap_or_else(|| persisted.source_url.clone().unwrap_or_default());
    let requested_reference = tree
        .collection
        .requested_reference
        .clone()
        .unwrap_or_else(|| persisted.requested_reference.clone().unwrap_or_default());
    let preview_id = format!(
        "github-collection-update-{}",
        sha256(&format!(
            "skillbox-github-collection-update-preview-v1\n{}\n{}\n{}\n{}\n{}\n{}",
            persisted.id, from_sha, to_sha, source_url, requested_reference, change_seed
        ))
    );
    tree.collection.preview_id = preview_id.clone();

    let mut affected = Vec::new();
    for change in &changes {
        if matches!(
            change.change,
            GithubCollectionChildChangeKind::Updated | GithubCollectionChildChangeKind::Blocked
        ) {
            if let (Some(name), Some(kind)) = (&change.managed_skill_name, change.skill_kind) {
                affected.extend(collection_member_deployments(paths, name, kind)?);
            }
        }
    }

    Ok(GithubCollectionUpdatePreview {
        preview_id,
        collection_id: persisted.id,
        source_url,
        requested_reference,
        from_sha,
        to_sha,
        collection: tree.collection,
        groups: tree.groups,
        changes,
        errors: tree.errors,
        diagnostics: tree.diagnostics,
        affected_deployments: affected,
    })
}

fn classify_github_collection_changes(
    paths: &ManagedPaths,
    persisted: &SkillCollection,
    preview: &ImportCandidateCollection,
) -> Result<Vec<GithubCollectionChildChange>> {
    let mut changes = Vec::new();
    let mut seen_members = HashSet::new();
    let members: HashMap<_, _> = persisted
        .members
        .iter()
        .map(|member| (member.relative_path.clone(), member))
        .collect();

    for child in &preview.children {
        if let Some(member) = members.get(&child.relative_path) {
            seen_members.insert(member.relative_path.clone());
            let kind = skill_kind_from_index(&paths.database_path, &member.managed_skill_name)?;
            let Some(kind) = kind else {
                changes.push(blocked_change(
                    child,
                    Some(*member),
                    "Managed skill is missing from the SkillBox index.",
                ));
                continue;
            };
            let managed_path = managed_skill_path(paths, &member.managed_skill_name, kind);
            let current_snapshot = skill_directory_snapshot_hash(&managed_path).ok();
            if current_snapshot.as_ref() != Some(&member.snapshot_hash) {
                changes.push(blocked_change(
                    child,
                    Some(*member),
                    "Managed copy changed after the last collection review. Update that skill individually or restore the reviewed snapshot first.",
                ));
                continue;
            }
            if child.conflict.as_deref() == Some("Duplicate skill name in repository") {
                changes.push(blocked_change(
                    child,
                    Some(*member),
                    child.conflict.clone().unwrap_or_default(),
                ));
                continue;
            }
            if child.snapshot_hash == member.snapshot_hash {
                changes.push(GithubCollectionChildChange {
                    relative_path: child.relative_path.clone(),
                    name: child.name.clone(),
                    change: GithubCollectionChildChangeKind::Unchanged,
                    from_snapshot_hash: Some(member.snapshot_hash.clone()),
                    to_snapshot_hash: Some(child.snapshot_hash.clone()),
                    managed_skill_name: Some(member.managed_skill_name.clone()),
                    skill_kind: Some(kind),
                    group_id: child.group_id.clone(),
                    variant_id: child.variant_id.clone(),
                    conflict: None,
                    eligible: false,
                    required: false,
                    default_selected: false,
                });
            } else {
                changes.push(GithubCollectionChildChange {
                    relative_path: child.relative_path.clone(),
                    name: child.name.clone(),
                    change: GithubCollectionChildChangeKind::Updated,
                    from_snapshot_hash: Some(member.snapshot_hash.clone()),
                    to_snapshot_hash: Some(child.snapshot_hash.clone()),
                    managed_skill_name: Some(member.managed_skill_name.clone()),
                    skill_kind: Some(kind),
                    group_id: child.group_id.clone(),
                    variant_id: child.variant_id.clone(),
                    conflict: None,
                    eligible: true,
                    required: true,
                    default_selected: true,
                });
            }
        } else {
            let blocked = child.conflict.is_some()
                || child.import_status != ImportCandidateStatus::Importable;
            changes.push(GithubCollectionChildChange {
                relative_path: child.relative_path.clone(),
                name: child.name.clone(),
                change: if blocked {
                    GithubCollectionChildChangeKind::Blocked
                } else {
                    GithubCollectionChildChangeKind::Added
                },
                from_snapshot_hash: None,
                to_snapshot_hash: Some(child.snapshot_hash.clone()),
                managed_skill_name: None,
                skill_kind: None,
                group_id: child.group_id.clone(),
                variant_id: child.variant_id.clone(),
                conflict: child.conflict.clone(),
                eligible: !blocked,
                required: false,
                default_selected: false,
            });
        }
    }

    for member in &persisted.members {
        if seen_members.contains(&member.relative_path) {
            continue;
        }
        let kind = skill_kind_from_index(&paths.database_path, &member.managed_skill_name)?;
        changes.push(GithubCollectionChildChange {
            relative_path: member.relative_path.clone(),
            name: member.skill_name.clone(),
            change: GithubCollectionChildChangeKind::Removed,
            from_snapshot_hash: Some(member.snapshot_hash.clone()),
            to_snapshot_hash: None,
            managed_skill_name: Some(member.managed_skill_name.clone()),
            skill_kind: kind,
            group_id: String::new(),
            variant_id: String::new(),
            conflict: None,
            eligible: false,
            required: false,
            default_selected: false,
        });
    }

    changes.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    Ok(changes)
}

fn blocked_change(
    child: &ImportCandidateCollectionChild,
    member: Option<&SkillCollectionMember>,
    conflict: impl Into<String>,
) -> GithubCollectionChildChange {
    GithubCollectionChildChange {
        relative_path: child.relative_path.clone(),
        name: child.name.clone(),
        change: GithubCollectionChildChangeKind::Blocked,
        from_snapshot_hash: member.map(|item| item.snapshot_hash.clone()),
        to_snapshot_hash: Some(child.snapshot_hash.clone()),
        managed_skill_name: member.map(|item| item.managed_skill_name.clone()),
        skill_kind: None,
        group_id: child.group_id.clone(),
        variant_id: child.variant_id.clone(),
        conflict: Some(conflict.into()),
        eligible: false,
        required: false,
        default_selected: false,
    }
}

fn apply_update_selection_defaults(
    tree: &mut GithubSkillCollectionPreview,
    changes: &[GithubCollectionChildChange],
) {
    let by_path = changes
        .iter()
        .map(|change| (change.relative_path.as_str(), change))
        .collect::<HashMap<_, _>>();
    for child in &mut tree.collection.children {
        let Some(change) = by_path.get(child.relative_path.as_str()) else {
            continue;
        };
        match change.change {
            GithubCollectionChildChangeKind::Updated => {
                child.import_status = ImportCandidateStatus::Importable;
                child.conflict = None;
                child.requires_type_review = false;
                child.selected_type = change.skill_kind;
                child.is_selected = true;
            }
            GithubCollectionChildChangeKind::Added => {
                child.is_selected = false;
            }
            GithubCollectionChildChangeKind::Unchanged => {
                child.import_status = ImportCandidateStatus::Imported;
                child.is_selected = false;
            }
            GithubCollectionChildChangeKind::Blocked => {
                child.conflict = change.conflict.clone();
                child.is_selected = false;
            }
            GithubCollectionChildChangeKind::Removed => {}
        }
    }
    for group in &mut tree.groups {
        if let Some(child) = tree
            .collection
            .children
            .iter()
            .find(|child| child.group_id == group.id)
        {
            if let Some(variant) = group
                .variants
                .iter_mut()
                .find(|variant| variant.id == child.variant_id)
            {
                variant.selected_type = child.selected_type;
                variant.requires_type_review = child.requires_type_review;
                variant.candidate.is_selected = child.is_selected;
                variant.candidate.import_status = child.import_status;
                variant.candidate.conflict = child.conflict.clone();
                if let Some(kind) = child.selected_type {
                    variant.suggested_types = vec![kind];
                    variant.candidate.suggested_type = kind;
                }
            }
        }
    }
}

fn append_removed_collection_children(
    tree: &mut GithubSkillCollectionPreview,
    changes: &[GithubCollectionChildChange],
) {
    for change in changes {
        if change.change != GithubCollectionChildChangeKind::Removed {
            continue;
        }
        if tree
            .collection
            .children
            .iter()
            .any(|child| child.relative_path == change.relative_path)
        {
            continue;
        }
        let group_id = format!(
            "removed-{}",
            &sha256(&format!("{}\n{}", tree.collection.id, change.relative_path))[..16]
        );
        let variant_id = format!("{group_id}-variant");
        let conflict = Some(
            "Removed from the repository. Collection membership will be dropped; the skill is not deleted."
                .to_string(),
        );
        let kind = change.skill_kind.unwrap_or(SkillKind::User);
        let candidate = ImportCandidate {
            name: change.name.clone(),
            description: String::new(),
            source_path: PathBuf::from(&change.relative_path),
            source_root: None,
            real_path: PathBuf::from(&change.relative_path),
            is_symlink: false,
            symlink_target_path: None,
            content_hash: change.from_snapshot_hash.clone().unwrap_or_default(),
            additional_source_paths: Vec::new(),
            suggested_type: kind,
            suggestion_reason: "Already managed".to_string(),
            import_status: ImportCandidateStatus::Imported,
            is_selected: false,
            conflict: conflict.clone(),
            usage_count: 0,
        };
        tree.groups.push(ImportCandidateGroup {
            id: group_id.clone(),
            name: change.name.clone(),
            description: String::new(),
            usage_count: 0,
            requires_review: false,
            selected_variant_id: Some(variant_id.clone()),
            variants: vec![ImportCandidateVariant {
                id: variant_id.clone(),
                candidate: candidate.clone(),
                snapshot_hash: change.from_snapshot_hash.clone().unwrap_or_default(),
                locations: Vec::new(),
                suggested_types: vec![kind],
                requires_type_review: false,
                selected_type: Some(kind),
            }],
        });
        tree.collection
            .children
            .push(ImportCandidateCollectionChild {
                id: format!("child-{group_id}"),
                group_id,
                variant_id,
                name: change.name.clone(),
                relative_path: change.relative_path.clone(),
                source_path: PathBuf::from(&change.relative_path),
                real_path: PathBuf::from(&change.relative_path),
                content_hash: change.from_snapshot_hash.clone().unwrap_or_default(),
                snapshot_hash: change.from_snapshot_hash.clone().unwrap_or_default(),
                diff: String::new(),
                import_status: ImportCandidateStatus::Imported,
                conflict,
                usage_count: 0,
                locations: Vec::new(),
                unlinked_locations: Vec::new(),
                suggested_types: vec![kind],
                requires_type_review: false,
                selected_type: Some(kind),
                is_selected: false,
            });
    }
}

fn apply_github_collection_update_with_audit(
    paths: &ManagedPaths,
    checkout: &Path,
    preview: &GithubCollectionUpdatePreview,
    request: &GithubSkillCollectionApplyRequest,
) -> Result<ImportCollectionApplyResult> {
    let required: HashSet<_> = preview
        .changes
        .iter()
        .filter(|change| change.required)
        .map(|change| change.relative_path.clone())
        .collect();
    let selected: HashSet<_> = request
        .selections
        .iter()
        .map(|selection| selection.relative_path.clone())
        .collect();
    if !required.is_subset(&selected) {
        return Err(
            "Select every updated collection member so the collection can move to one reviewed SHA."
                .to_string(),
        );
    }

    let mut selected_names = HashSet::new();
    let mut added_items = Vec::new();
    let mut added_children = Vec::new();
    let mut updated_children = Vec::new();
    for selection in &request.selections {
        let change = preview
            .changes
            .iter()
            .find(|item| item.relative_path == selection.relative_path)
            .ok_or_else(|| {
                "Selected GitHub collection child is not part of the reviewed update.".to_string()
            })?;
        match change.change {
            GithubCollectionChildChangeKind::Updated => {
                if !change.eligible {
                    return Err(format!(
                        "Selected GitHub collection child {} is not eligible to update.",
                        change.name
                    ));
                }
                if change.skill_kind != Some(selection.skill_type) {
                    return Err(format!(
                        "Collection update must keep {} as a {} skill.",
                        change.name,
                        change.skill_kind.unwrap_or(SkillKind::User).as_str()
                    ));
                }
                let child = preview
                    .collection
                    .children
                    .iter()
                    .find(|child| child.relative_path == selection.relative_path)
                    .ok_or_else(|| {
                        "Selected GitHub collection child is not part of the reviewed preview."
                            .to_string()
                    })?;
                updated_children.push((change.clone(), child.clone()));
            }
            GithubCollectionChildChangeKind::Added => {
                if !change.eligible {
                    return Err(format!(
                        "Selected GitHub collection child {} is not importable.",
                        change.name
                    ));
                }
                let child = preview
                    .collection
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
                let target = collection_import_target(paths, &skill, selection.skill_type);
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
                added_items.push(ImportRequestItem {
                    source_path,
                    skill_type: selection.skill_type,
                    deploy_back_to_source: false,
                });
                added_children.push(child.clone());
            }
            _ => {
                return Err(format!(
                    "Collection child {} cannot be selected for this update.",
                    change.name
                ));
            }
        }
    }

    let persisted = list_skill_collections(&paths.root)?
        .into_iter()
        .find(|collection| collection.id == preview.collection_id)
        .ok_or_else(|| "GitHub collection disappeared before apply.".to_string())?;
    if persisted.reviewed_head_sha.as_deref() != Some(preview.from_sha.as_str()) {
        return Err(
            "Collection already moved to a different reviewed SHA. Re-open the update preview."
                .to_string(),
        );
    }

    let operation = start_operation(
        OperationStart {
            operation_type: "update_github_collection".to_string(),
            actor: request.actor.clone(),
            entity_type: "skill_collection".to_string(),
            entity_name: preview.collection.display_name.clone(),
            summary: format!(
                "Update {} from {} to {}",
                preview.collection.display_name,
                short_sha(&preview.from_sha),
                short_sha(&preview.to_sha)
            ),
            payload: serde_json::json!({
                "collectionId": preview.collection_id,
                "fromSha": preview.from_sha,
                "toSha": preview.to_sha,
                "previewId": preview.preview_id,
                "phase": "validated"
            }),
        },
        &paths.root,
    )?;

    let outcome = apply_github_collection_update_inner(
        paths,
        checkout,
        preview,
        &persisted,
        &updated_children,
        added_children,
        added_items,
    );
    match outcome {
        Ok(result) => {
            let mut warnings = result.warnings.clone();
            if let Err(error) = finish_operation(
                OperationFinish {
                    id: operation.id,
                    status: OperationStatus::Succeeded,
                    summary: format!(
                        "Updated {} to {}",
                        preview.collection.display_name,
                        short_sha(&preview.to_sha)
                    ),
                    error: None,
                    payload: serde_json::json!({
                        "collectionId": preview.collection_id,
                        "fromSha": preview.from_sha,
                        "toSha": preview.to_sha,
                        "importedSkillNames": result.imported.iter().map(|item| item.name.clone()).collect::<Vec<_>>(),
                        "phase": "completed"
                    }),
                },
                &paths.root,
            ) {
                warnings.push(format!(
                    "Collection update completed, but its operation history could not be finalized: {error}"
                ));
            }
            Ok(ImportCollectionApplyResult { warnings, ..result })
        }
        Err(error) => {
            let _ = finish_operation(
                OperationFinish {
                    id: operation.id,
                    status: OperationStatus::Failed,
                    summary: format!(
                        "Collection update failed for {}",
                        preview.collection.display_name
                    ),
                    error: Some(error.clone()),
                    payload: serde_json::json!({
                        "collectionId": preview.collection_id,
                        "fromSha": preview.from_sha,
                        "toSha": preview.to_sha,
                        "phase": "failed"
                    }),
                },
                &paths.root,
            );
            Err(error)
        }
    }
}

fn apply_github_collection_update_inner(
    paths: &ManagedPaths,
    checkout: &Path,
    preview: &GithubCollectionUpdatePreview,
    persisted: &SkillCollection,
    updated_children: &[(GithubCollectionChildChange, ImportCandidateCollectionChild)],
    added_children: Vec<ImportCandidateCollectionChild>,
    added_items: Vec<ImportRequestItem>,
) -> Result<ImportCollectionApplyResult> {
    let revision_members = snapshot_collection_revision(paths, persisted)?;
    record_collection_revision(
        &paths.database_path,
        persisted,
        &preview.from_sha,
        &revision_members,
    )?;

    let mut restores = Vec::new();
    let mut imported = Vec::new();
    let apply_result = (|| {
        for (change, child) in updated_children {
            let kind = change.skill_kind.ok_or_else(|| {
                format!("Collection member {} is missing a skill type.", change.name)
            })?;
            let managed_name = change
                .managed_skill_name
                .clone()
                .unwrap_or_else(|| child.name.clone());
            let source_path = checkout.join(&child.relative_path);
            let skill = read_skill(&source_path)?;
            if skill.name != child.name
                || skill_directory_snapshot_hash(&source_path)? != child.snapshot_hash
            {
                return Err(
                    "GitHub collection preview is stale. Re-open the preview and try again."
                        .to_string(),
                );
            }
            match kind {
                SkillKind::User => {
                    let target = paths.user_skills_root.join(&managed_name);
                    replace_skill_directory(&source_path, &target)?;
                    restores.push(RestoreAction::UserFromBackup {
                        target: target.clone(),
                        backup: revision_backup_path(
                            paths,
                            &preview.collection_id,
                            &preview.from_sha,
                            &managed_name,
                        ),
                    });
                    index_skill(&paths.database_path, &skill, SkillKind::User, &target)?;
                    imported.push(ImportedCandidate {
                        name: skill.name,
                        kind: SkillKind::User,
                        source_path,
                        managed_path: target,
                        content_hash: skill.content_hash,
                        backup_path: None,
                        deployed_path: None,
                    });
                }
                SkillKind::Remote => {
                    let remote_root = paths.remote_skills_root.join(&managed_name);
                    let version_name = format!("manual-{}", &skill.content_hash[..12]);
                    let version_path = remote_root.join("versions").join(&version_name);
                    let created = !version_path.exists();
                    if created {
                        if let Some(parent) = version_path.parent() {
                            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
                        }
                        copy_skill_dir(&source_path, &version_path)?;
                    } else if skill_directory_snapshot_hash(&version_path)? != child.snapshot_hash {
                        return Err(format!(
                            "Managed version exists with different contents: {}",
                            version_path.display()
                        ));
                    }
                    let current = remote_root.join("current");
                    let old_current = fs::read_link(&current).ok();
                    update_current_symlink(&remote_root, &version_path)?;
                    restores.push(RestoreAction::RemoteCurrent {
                        remote_root: remote_root.clone(),
                        old_current,
                        new_version: if created {
                            Some(version_path.clone())
                        } else {
                            None
                        },
                    });
                    index_skill(
                        &paths.database_path,
                        &skill,
                        SkillKind::Remote,
                        &version_path,
                    )?;
                    imported.push(ImportedCandidate {
                        name: skill.name,
                        kind: SkillKind::Remote,
                        source_path,
                        managed_path: version_path,
                        content_hash: skill.content_hash,
                        backup_path: None,
                        deployed_path: None,
                    });
                }
            }
        }

        if !added_items.is_empty() {
            let batch = import_candidates_with_paths(paths, added_items)?;
            if !batch.errors.is_empty() {
                let (targets, _) = collection_import_targets_from_imported(&batch.imported);
                restores.push(RestoreAction::Imported(targets));
                return Err(batch
                    .errors
                    .into_iter()
                    .map(|item| item.error)
                    .collect::<Vec<_>>()
                    .join(" "));
            }
            let (targets, _) = collection_import_targets_from_imported(&batch.imported);
            restores.push(RestoreAction::Imported(targets));
            imported.extend(batch.imported);
        }

        let mut member_writes = Vec::new();
        for change in &preview.changes {
            match change.change {
                GithubCollectionChildChangeKind::Unchanged => {
                    member_writes.push(CollectionMemberWrite {
                        relative_path: change.relative_path.clone(),
                        skill_name: change.name.clone(),
                        snapshot_hash: change.to_snapshot_hash.clone().unwrap_or_else(|| {
                            change.from_snapshot_hash.clone().unwrap_or_default()
                        }),
                        content_hash: preview
                            .collection
                            .children
                            .iter()
                            .find(|child| child.relative_path == change.relative_path)
                            .map(|child| child.content_hash.clone())
                            .unwrap_or_default(),
                        managed_skill_name: change
                            .managed_skill_name
                            .clone()
                            .unwrap_or_else(|| change.name.clone()),
                        reviewed_head_sha: Some(preview.to_sha.clone()),
                    });
                }
                GithubCollectionChildChangeKind::Updated => {
                    let child = preview
                        .collection
                        .children
                        .iter()
                        .find(|child| child.relative_path == change.relative_path)
                        .ok_or_else(|| {
                            format!("Updated child {} missing from preview.", change.name)
                        })?;
                    member_writes.push(CollectionMemberWrite {
                        relative_path: change.relative_path.clone(),
                        skill_name: child.name.clone(),
                        snapshot_hash: child.snapshot_hash.clone(),
                        content_hash: child.content_hash.clone(),
                        managed_skill_name: change
                            .managed_skill_name
                            .clone()
                            .unwrap_or_else(|| child.name.clone()),
                        reviewed_head_sha: Some(preview.to_sha.clone()),
                    });
                }
                GithubCollectionChildChangeKind::Added => {
                    if let Some(child) = added_children
                        .iter()
                        .find(|child| child.relative_path == change.relative_path)
                    {
                        let imported_name = imported
                            .iter()
                            .find(|item| item.name == child.name)
                            .map(|item| item.name.clone())
                            .unwrap_or_else(|| child.name.clone());
                        member_writes.push(CollectionMemberWrite {
                            relative_path: child.relative_path.clone(),
                            skill_name: child.name.clone(),
                            snapshot_hash: child.snapshot_hash.clone(),
                            content_hash: child.content_hash.clone(),
                            managed_skill_name: imported_name,
                            reviewed_head_sha: Some(preview.to_sha.clone()),
                        });
                    }
                }
                GithubCollectionChildChangeKind::Removed
                | GithubCollectionChildChangeKind::Blocked => {}
            }
        }
        let drop_paths = preview
            .changes
            .iter()
            .filter(|change| change.change == GithubCollectionChildChangeKind::Removed)
            .map(|change| change.relative_path.clone())
            .collect::<Vec<_>>();
        let mut collection_preview = preview.collection.clone();
        collection_preview.reviewed_head_sha = Some(preview.to_sha.clone());
        let collection = persist_github_collection_update(
            &paths.database_path,
            &collection_preview,
            Some(&preview.from_sha),
            &member_writes,
            &drop_paths,
        )?;
        Ok(ImportCollectionApplyResult {
            collection,
            imported,
            errors: Vec::new(),
            warnings: Vec::new(),
        })
    })();

    match apply_result {
        Ok(result) => Ok(result),
        Err(error) => {
            let mut restore_errors = Vec::new();
            for action in restores.into_iter().rev() {
                if let Err(restore_error) = restore_collection_update_action(paths, action) {
                    restore_errors.push(restore_error);
                }
            }
            if restore_errors.is_empty() {
                Err(error)
            } else {
                Err(format!(
                    "{error} Collection update rollback was incomplete: {}",
                    restore_errors.join(" ")
                ))
            }
        }
    }
}

fn snapshot_collection_revision(
    paths: &ManagedPaths,
    collection: &SkillCollection,
) -> Result<Vec<CollectionRevisionMemberRecord>> {
    let sha = collection.reviewed_head_sha.clone().ok_or_else(|| {
        "Collection is missing a reviewed SHA and cannot snapshot a revision.".to_string()
    })?;
    if collection_revision_exists(&paths.database_path, &collection.id, &sha)? {
        return load_collection_revision_members(&paths.database_path, &collection.id, &sha);
    }
    let mut records = Vec::new();
    for member in &collection.members {
        let kind = skill_kind_from_index(&paths.database_path, &member.managed_skill_name)?
            .ok_or_else(|| {
                format!(
                    "Collection member {} is missing from the SkillBox index.",
                    member.managed_skill_name
                )
            })?;
        let source = managed_skill_path(paths, &member.managed_skill_name, kind);
        let backup = revision_backup_path(paths, &collection.id, &sha, &member.managed_skill_name);
        if let Some(parent) = backup.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        if !backup.exists() {
            copy_skill_dir(&source, &backup)?;
        }
        records.push(CollectionRevisionMemberRecord {
            relative_path: member.relative_path.clone(),
            skill_name: member.skill_name.clone(),
            snapshot_hash: member.snapshot_hash.clone(),
            content_hash: member.content_hash.clone(),
            managed_skill_name: member.managed_skill_name.clone(),
            skill_kind: kind,
            backup_path: Some(backup),
        });
    }
    Ok(records)
}

fn build_github_collection_rollback_preview(
    paths: &ManagedPaths,
    collection_id: &str,
) -> Result<GithubCollectionRollbackPreview> {
    let collection = list_skill_collections(&paths.root)?
        .into_iter()
        .find(|item| item.id == collection_id)
        .ok_or_else(|| "Collection was not found.".to_string())?;
    if collection.source_kind != ImportCandidateCollectionSourceKind::GithubRemote {
        return Err("Only GitHub collections support collection-level rollback.".to_string());
    }
    let from_sha = collection.reviewed_head_sha.clone().ok_or_else(|| {
        "Collection is missing a reviewed SHA and cannot be rolled back.".to_string()
    })?;
    let to_sha = collection.previous_reviewed_head_sha.clone().ok_or_else(|| {
        "This collection has no previous reviewed SHA to restore. Apply a collection update first."
            .to_string()
    })?;
    if !collection_revision_exists(&paths.database_path, &collection.id, &to_sha)? {
        return Err(
            "No collection revision backup exists for the previous SHA. Rollback is unavailable."
                .to_string(),
        );
    }
    let revision_members =
        load_collection_revision_members(&paths.database_path, &collection.id, &to_sha)?;
    let mut errors = Vec::new();
    let mut members = Vec::new();
    let mut affected = Vec::new();
    for member in &revision_members {
        let mut restorable = true;
        let mut message = format!(
            "Restore {} to the snapshot from {}.",
            member.managed_skill_name,
            short_sha(&to_sha)
        );
        let Some(backup) = member.backup_path.as_ref() else {
            restorable = false;
            message = "Revision backup path is missing.".to_string();
            errors.push(format!(
                "{} cannot be restored because its revision backup is missing.",
                member.managed_skill_name
            ));
            members.push(GithubCollectionRollbackMember {
                relative_path: member.relative_path.clone(),
                skill_name: member.skill_name.clone(),
                managed_skill_name: member.managed_skill_name.clone(),
                skill_kind: member.skill_kind,
                restorable,
                message,
            });
            continue;
        };
        if !backup.join("SKILL.md").exists() {
            restorable = false;
            message = "Revision backup is missing SKILL.md.".to_string();
            errors.push(format!(
                "{} cannot be restored because its revision backup is incomplete.",
                member.managed_skill_name
            ));
        } else if let Some(kind) =
            skill_kind_from_index(&paths.database_path, &member.managed_skill_name)?
        {
            if kind != member.skill_kind {
                restorable = false;
                message = "Managed skill type changed after the collection update.".to_string();
                errors.push(format!(
                    "{} changed type and cannot be restored by collection rollback.",
                    member.managed_skill_name
                ));
            } else {
                let current = managed_skill_path(paths, &member.managed_skill_name, kind);
                if let Ok(snapshot) = skill_directory_snapshot_hash(&current) {
                    let current_member = collection
                        .members
                        .iter()
                        .find(|item| item.relative_path == member.relative_path);
                    if let Some(current_member) = current_member {
                        if snapshot != current_member.snapshot_hash {
                            restorable = false;
                            message =
                                "Managed copy changed after the collection update.".to_string();
                            errors.push(format!(
                                "{} changed after the collection update and will not be overwritten.",
                                member.managed_skill_name
                            ));
                        }
                    }
                }
                affected.extend(collection_member_deployments(
                    paths,
                    &member.managed_skill_name,
                    kind,
                )?);
            }
        } else {
            restorable = false;
            message = "Managed skill is missing.".to_string();
            errors.push(format!(
                "{} is missing and cannot be restored by collection rollback.",
                member.managed_skill_name
            ));
        }
        members.push(GithubCollectionRollbackMember {
            relative_path: member.relative_path.clone(),
            skill_name: member.skill_name.clone(),
            managed_skill_name: member.managed_skill_name.clone(),
            skill_kind: member.skill_kind,
            restorable,
            message,
        });
    }
    let revision_names: HashSet<_> = revision_members
        .iter()
        .map(|member| member.managed_skill_name.clone())
        .collect();
    let leaving_skill_names = collection
        .members
        .iter()
        .filter(|member| !revision_names.contains(&member.managed_skill_name))
        .map(|member| member.managed_skill_name.clone())
        .collect::<Vec<_>>();
    let seed = members
        .iter()
        .map(|member| {
            format!(
                "{}\n{}\n{}",
                member.relative_path, member.restorable, member.message
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let preview_id = format!(
        "github-collection-rollback-{}",
        sha256(&format!(
            "skillbox-github-collection-rollback-preview-v1\n{}\n{}\n{}\n{}\n{}",
            collection.id,
            from_sha,
            to_sha,
            leaving_skill_names.join(","),
            seed
        ))
    );
    Ok(GithubCollectionRollbackPreview {
        preview_id,
        collection_id: collection.id,
        display_name: collection.display_name,
        source_url: collection.source_url.unwrap_or_default(),
        from_sha,
        to_sha,
        members,
        leaving_skill_names,
        affected_deployments: affected,
        errors,
    })
}

fn apply_github_collection_rollback_with_audit(
    paths: &ManagedPaths,
    preview: &GithubCollectionRollbackPreview,
    actor: &str,
) -> Result<GithubCollectionRollbackResult> {
    let operation = start_operation(
        OperationStart {
            operation_type: "rollback_github_collection".to_string(),
            actor: actor.to_string(),
            entity_type: "skill_collection".to_string(),
            entity_name: preview.display_name.clone(),
            summary: format!(
                "Roll back {} from {} to {}",
                preview.display_name,
                short_sha(&preview.from_sha),
                short_sha(&preview.to_sha)
            ),
            payload: serde_json::json!({
                "collectionId": preview.collection_id,
                "fromSha": preview.from_sha,
                "toSha": preview.to_sha,
                "previewId": preview.preview_id,
                "phase": "validated"
            }),
        },
        &paths.root,
    )?;
    match apply_github_collection_rollback_inner(paths, preview) {
        Ok(result) => {
            let mut warnings = result.warnings.clone();
            if let Err(error) = finish_operation(
                OperationFinish {
                    id: operation.id,
                    status: OperationStatus::Succeeded,
                    summary: format!(
                        "Rolled back {} to {}",
                        preview.display_name,
                        short_sha(&preview.to_sha)
                    ),
                    error: None,
                    payload: serde_json::json!({
                        "collectionId": preview.collection_id,
                        "fromSha": preview.from_sha,
                        "toSha": preview.to_sha,
                        "restored": result.restored,
                        "phase": "completed"
                    }),
                },
                &paths.root,
            ) {
                warnings.push(format!(
                    "Collection rollback completed, but its operation history could not be finalized: {error}"
                ));
            }
            Ok(GithubCollectionRollbackResult { warnings, ..result })
        }
        Err(error) => {
            let _ = finish_operation(
                OperationFinish {
                    id: operation.id,
                    status: OperationStatus::Failed,
                    summary: format!("Collection rollback failed for {}", preview.display_name),
                    error: Some(error.clone()),
                    payload: serde_json::json!({
                        "collectionId": preview.collection_id,
                        "fromSha": preview.from_sha,
                        "toSha": preview.to_sha,
                        "phase": "failed"
                    }),
                },
                &paths.root,
            );
            Err(error)
        }
    }
}

fn apply_github_collection_rollback_inner(
    paths: &ManagedPaths,
    preview: &GithubCollectionRollbackPreview,
) -> Result<GithubCollectionRollbackResult> {
    let collection = list_skill_collections(&paths.root)?
        .into_iter()
        .find(|item| item.id == preview.collection_id)
        .ok_or_else(|| "Collection was not found.".to_string())?;
    if collection.reviewed_head_sha.as_deref() != Some(preview.from_sha.as_str())
        || collection.previous_reviewed_head_sha.as_deref() != Some(preview.to_sha.as_str())
    {
        return Err(
            "Collection rollback preview is stale. Re-open the preview and try again.".to_string(),
        );
    }
    let revision_members = load_collection_revision_members(
        &paths.database_path,
        &preview.collection_id,
        &preview.to_sha,
    )?;
    let mut restored = Vec::new();
    for member in &revision_members {
        let backup = member.backup_path.as_ref().ok_or_else(|| {
            format!(
                "{} is missing a revision backup.",
                member.managed_skill_name
            )
        })?;
        let target = managed_skill_path(paths, &member.managed_skill_name, member.skill_kind);
        match member.skill_kind {
            SkillKind::User => {
                replace_skill_directory(backup, &target)?;
                let skill = read_skill(&target)?;
                index_skill(&paths.database_path, &skill, SkillKind::User, &target)?;
            }
            SkillKind::Remote => {
                let remote_root = paths.remote_skills_root.join(&member.managed_skill_name);
                let version_name = format!(
                    "manual-{}",
                    &member.content_hash[..12.min(member.content_hash.len())]
                );
                let version_path = remote_root.join("versions").join(&version_name);
                if !version_path.exists() {
                    if let Some(parent) = version_path.parent() {
                        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
                    }
                    copy_skill_dir(backup, &version_path)?;
                }
                update_current_symlink(&remote_root, &version_path)?;
                let skill = read_skill(&version_path)?;
                index_skill(
                    &paths.database_path,
                    &skill,
                    SkillKind::Remote,
                    &version_path,
                )?;
            }
        }
        restored.push(member.managed_skill_name.clone());
    }

    let preview_collection = ImportCandidateCollection {
        id: collection.id.clone(),
        preview_id: preview.preview_id.clone(),
        display_name: collection.display_name.clone(),
        source_kind: collection.source_kind,
        canonical_worktree_root: collection.canonical_worktree_root.clone(),
        canonical_repository_id: collection.canonical_repository_id.clone(),
        origin_url: collection.origin_url.clone(),
        branch: collection.branch.clone(),
        detached: collection.detached,
        reviewed_head_sha: Some(preview.to_sha.clone()),
        source_url: collection.source_url.clone(),
        requested_reference: collection.requested_reference.clone(),
        children: Vec::new(),
        errors: Vec::new(),
    };
    let member_writes = revision_members
        .iter()
        .map(|member| CollectionMemberWrite {
            relative_path: member.relative_path.clone(),
            skill_name: member.skill_name.clone(),
            snapshot_hash: member.snapshot_hash.clone(),
            content_hash: member.content_hash.clone(),
            managed_skill_name: member.managed_skill_name.clone(),
            reviewed_head_sha: Some(preview.to_sha.clone()),
        })
        .collect::<Vec<_>>();
    let drop_paths = collection
        .members
        .iter()
        .filter(|member| {
            !revision_members
                .iter()
                .any(|item| item.relative_path == member.relative_path)
        })
        .map(|member| member.relative_path.clone())
        .collect::<Vec<_>>();
    let stored = persist_github_collection_update(
        &paths.database_path,
        &preview_collection,
        None,
        &member_writes,
        &drop_paths,
    )?;
    Ok(GithubCollectionRollbackResult {
        collection: stored,
        restored,
        leaving_skill_names: preview.leaving_skill_names.clone(),
        errors: Vec::new(),
        warnings: Vec::new(),
    })
}

enum RestoreAction {
    UserFromBackup {
        target: PathBuf,
        backup: PathBuf,
    },
    RemoteCurrent {
        remote_root: PathBuf,
        old_current: Option<PathBuf>,
        new_version: Option<PathBuf>,
    },
    Imported(Vec<CollectionImportTarget>),
}

fn restore_collection_update_action(paths: &ManagedPaths, action: RestoreAction) -> Result<()> {
    match action {
        RestoreAction::UserFromBackup { target, backup } => {
            replace_skill_directory(&backup, &target)
        }
        RestoreAction::RemoteCurrent {
            remote_root,
            old_current,
            new_version,
        } => {
            if let Some(old_current) = old_current {
                update_current_symlink(&remote_root, &old_current)?;
            }
            if let Some(new_version) = new_version {
                if new_version.starts_with(&remote_root) {
                    let _ = fs::remove_dir_all(new_version);
                }
            }
            Ok(())
        }
        RestoreAction::Imported(targets) => rollback_collection_imports(paths, &targets),
    }
}

fn managed_skill_path(paths: &ManagedPaths, skill_name: &str, kind: SkillKind) -> PathBuf {
    match kind {
        SkillKind::User => paths.user_skills_root.join(skill_name),
        SkillKind::Remote => paths.remote_skills_root.join(skill_name).join("current"),
    }
}

fn revision_backup_path(
    paths: &ManagedPaths,
    collection_id: &str,
    sha: &str,
    skill_name: &str,
) -> PathBuf {
    paths
        .root
        .join("backups")
        .join("collection-revisions")
        .join(collection_id)
        .join(sha)
        .join(skill_name)
}

fn collection_member_deployments(
    paths: &ManagedPaths,
    skill_name: &str,
    kind: SkillKind,
) -> Result<Vec<AffectedDeployment>> {
    if kind == SkillKind::Remote {
        return classify_affected_deployments(paths, skill_name);
    }
    if !paths.database_path.is_file() {
        return Ok(Vec::new());
    }
    let deployments = load_deployments(&paths.database_path)?;
    let managed = paths.user_skills_root.join(skill_name);
    Ok(deployments
        .get(skill_name)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|deployment| {
            let follows = fs::read_link(&deployment.target_path)
                .ok()
                .is_some_and(|target| target == managed);
            AffectedDeployment {
                target_root: deployment.target_root,
                target_path: deployment.target_path,
                mode: deployment.mode,
                state: if follows {
                    "follows_managed".to_string()
                } else {
                    "unmanaged".to_string()
                },
                message: if follows {
                    "Deployment follows the managed user skill and will observe the new files."
                        .to_string()
                } else {
                    "Deployment target is not this managed user skill.".to_string()
                },
            }
        })
        .collect())
}

fn short_sha(value: &str) -> String {
    value.chars().take(8).collect()
}
