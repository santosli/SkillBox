use crate::*;
use skillbox_git::{GitRepositoryIdentity, GitService};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

const MAX_COLLECTION_CHILDREN: usize = 500;
const MAX_COLLECTION_ERRORS: usize = 32;

pub(crate) fn discover_import_collections(
    candidates: &[ImportCandidate],
    groups: &[ImportCandidateGroup],
) -> Vec<ImportCandidateCollection> {
    let git = GitService::new();
    let mut builders = BTreeMap::<(PathBuf, PathBuf), CollectionBuilder>::new();

    for candidate in candidates {
        let identity = match git.repository_identity(&candidate.real_path) {
            Ok(Some(identity)) => identity,
            Ok(None) | Err(_) => continue,
        };
        let Some(relative_path) =
            safe_collection_relative_path(&identity.worktree_root, &candidate.real_path)
        else {
            continue;
        };
        if relative_path.as_os_str().is_empty() {
            continue;
        }
        let Some(group) = groups
            .iter()
            .find(|group| group.name.eq_ignore_ascii_case(&candidate.name))
        else {
            continue;
        };
        let Some(variant) = group.variants.iter().find(|variant| {
            variant
                .locations
                .iter()
                .any(|location| location.source_path == candidate.source_path)
        }) else {
            continue;
        };
        let snapshot_hash = match skill_directory_snapshot_hash(&candidate.real_path) {
            Ok(hash) => hash,
            Err(_) => continue,
        };
        let key = (identity.worktree_root.clone(), identity.common_dir.clone());
        let builder = builders.entry(key).or_insert_with(|| CollectionBuilder {
            identity,
            children: BTreeMap::new(),
            errors: Vec::new(),
        });
        let child_key = format!("{relative_path:?}\n{}", variant.id);
        if builder.children.contains_key(&child_key) {
            continue;
        }
        let locations = variant
            .locations
            .iter()
            .filter(|location| {
                safe_collection_relative_path(&builder.identity.worktree_root, &location.real_path)
                    .is_some_and(|path| path == relative_path)
            })
            .cloned()
            .collect::<Vec<_>>();
        let locations = if locations.is_empty() {
            vec![import_candidate_location_for_collection(candidate)]
        } else {
            locations
        };
        let unlinked_locations = variant
            .locations
            .iter()
            .filter(|location| {
                safe_collection_relative_path(&builder.identity.worktree_root, &location.real_path)
                    .is_none()
            })
            .cloned()
            .collect::<Vec<_>>();
        builder.children.insert(
            child_key,
            ImportCandidateCollectionChild {
                id: format!(
                    "child-{}",
                    &sha256(&format!(
                        "{}\n{}\n{}",
                        group.id,
                        variant.id,
                        relative_path.display()
                    ))[..16]
                ),
                group_id: group.id.clone(),
                variant_id: variant.id.clone(),
                name: candidate.name.clone(),
                relative_path: relative_path.to_string_lossy().to_string(),
                source_path: candidate.source_path.clone(),
                real_path: candidate.real_path.clone(),
                content_hash: candidate.content_hash.clone(),
                snapshot_hash,
                import_status: candidate.import_status,
                conflict: candidate.conflict.clone(),
                usage_count: candidate.usage_count,
                locations,
                unlinked_locations,
                suggested_types: variant.suggested_types.clone(),
                requires_type_review: variant.requires_type_review,
                selected_type: variant.selected_type,
                is_selected: variant.candidate.is_selected,
            },
        );
    }

    builders
        .into_values()
        .filter_map(|builder| builder.finish())
        .collect()
}

struct CollectionBuilder {
    identity: GitRepositoryIdentity,
    children: BTreeMap<String, ImportCandidateCollectionChild>,
    errors: Vec<ImportCandidateError>,
}

impl CollectionBuilder {
    fn finish(self) -> Option<ImportCandidateCollection> {
        if self.children.is_empty() {
            return None;
        }
        let children = self
            .children
            .into_values()
            .take(MAX_COLLECTION_CHILDREN)
            .collect::<Vec<_>>();
        let identity_seed = children
            .iter()
            .map(|child| {
                format!(
                    "{}\n{}\n{}\n{:?}\n{}\n{}\n{:?}\n{}\n{:?}",
                    child.relative_path,
                    child.snapshot_hash,
                    child.content_hash,
                    child.import_status,
                    child.conflict.as_deref().unwrap_or_default(),
                    child.variant_id,
                    format!("{:?}", child.suggested_types),
                    child.requires_type_review,
                    format!("{:?}", child.selected_type)
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        let repository_seed = format!(
            "{}\n{}\n{}",
            self.identity.worktree_root.display(),
            self.identity.common_dir.display(),
            self.identity.head.as_deref().unwrap_or_default()
        );
        let id = format!("collection-{}", &sha256(&repository_seed)[..16]);
        let origin_url = self.identity.origin_url.as_deref().map(sanitize_origin_url);
        let preview_id = sha256(&format!(
            "skillbox-collection-preview-v1\n{}\n{}\n{}",
            repository_seed,
            origin_url.as_deref().unwrap_or_default(),
            identity_seed
        ));
        let display_name = self
            .identity
            .worktree_root
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| !name.is_empty())
            .unwrap_or("Git repository")
            .to_string();
        let detached = self.identity.branch.is_none() && self.identity.head.is_some();
        Some(ImportCandidateCollection {
            id,
            preview_id,
            display_name,
            canonical_worktree_root: self.identity.worktree_root,
            canonical_repository_id: self.identity.common_dir,
            origin_url,
            branch: self.identity.branch,
            detached,
            reviewed_head_sha: self.identity.head,
            children,
            errors: self
                .errors
                .into_iter()
                .take(MAX_COLLECTION_ERRORS)
                .collect(),
        })
    }
}

fn import_candidate_location_for_collection(
    candidate: &ImportCandidate,
) -> ImportCandidateLocation {
    ImportCandidateLocation {
        source_path: candidate.source_path.clone(),
        source_root: candidate.source_root.clone(),
        real_path: candidate.real_path.clone(),
        is_symlink: candidate.is_symlink,
        symlink_target_path: candidate.symlink_target_path.clone(),
        suggested_type: candidate.suggested_type,
        suggestion_reason: candidate.suggestion_reason.clone(),
    }
}

fn safe_collection_relative_path(root: &Path, path: &Path) -> Option<PathBuf> {
    let relative = path.strip_prefix(root).ok()?;
    if relative.as_os_str().is_empty() {
        return None;
    }
    for component in relative.components() {
        match component {
            Component::Normal(name) if name != ".git" => {}
            _ => return None,
        }
    }
    Some(relative.to_path_buf())
}

fn sanitize_origin_url(value: &str) -> String {
    let mut value = value.trim().to_string();
    if let Some(index) = value.find(['?', '#']) {
        value.truncate(index);
    }
    if let Some(scheme_end) = value.find("://") {
        let authority_start = scheme_end + 3;
        if let Some(at) = value[authority_start..].find('@') {
            value.replace_range(authority_start..authority_start + at + 1, "");
        }
    } else if let Some(at) = value.find('@') {
        value.replace_range(..at + 1, "");
    }
    value
}

pub fn list_skill_collections(managed_root: impl AsRef<Path>) -> Result<Vec<SkillCollection>> {
    let paths = ensure_managed_layout(expand_home(managed_root.as_ref().to_path_buf()))?;
    let connection = open_database(&paths.database_path)?;
    let mut statement = connection
        .prepare(
            "SELECT id, display_name, canonical_worktree_root, canonical_repository_id,
                    origin_url, branch, detached, reviewed_head_sha, available
               FROM skill_collections
              ORDER BY display_name COLLATE NOCASE, id",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| {
            Ok(SkillCollection {
                id: row.get(0)?,
                display_name: row.get(1)?,
                canonical_worktree_root: PathBuf::from(row.get::<_, String>(2)?),
                canonical_repository_id: PathBuf::from(row.get::<_, String>(3)?),
                origin_url: row.get(4)?,
                branch: row.get(5)?,
                detached: row.get::<_, i64>(6)? != 0,
                reviewed_head_sha: row.get(7)?,
                available: row.get::<_, i64>(8)? != 0,
                members: Vec::new(),
            })
        })
        .map_err(|error| error.to_string())?;
    let mut collections = rows
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    for collection in &mut collections {
        let mut members = connection
            .prepare(
                "SELECT collection_id, skill_name, relative_path, reviewed_head_sha,
                        snapshot_hash, content_hash, managed_skill_name
                   FROM skill_collection_members
                  WHERE collection_id = ?1
                  ORDER BY relative_path",
            )
            .map_err(|error| error.to_string())?;
        collection.members = members
            .query_map([&collection.id], |row| {
                Ok(SkillCollectionMember {
                    collection_id: row.get(0)?,
                    skill_name: row.get(1)?,
                    relative_path: row.get(2)?,
                    reviewed_head_sha: row.get(3)?,
                    snapshot_hash: row.get(4)?,
                    content_hash: row.get(5)?,
                    managed_skill_name: row.get(6)?,
                })
            })
            .map_err(|error| error.to_string())?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;
        collection.available = collection.canonical_worktree_root.is_dir();
    }
    Ok(collections)
}

pub fn apply_import_collection(
    request: ImportCollectionApplyRequest,
    managed_root: impl AsRef<Path>,
) -> Result<ImportCollectionApplyResult> {
    if request.selections.is_empty() {
        return Err("Select at least one skill from the collection.".to_string());
    }
    let mutation_lock = acquire_user_skills_mutation_lock(managed_root.as_ref())?;
    let paths = ensure_managed_layout(mutation_lock.truth_root().to_path_buf())?;
    let root = fs::canonicalize(expand_home(request.worktree_root.clone()))
        .map_err(|error| format!("Unable to resolve collection worktree: {error}"))?;
    let scan = scan_import_candidates(std::slice::from_ref(&root), &paths.root)?;
    let collection = scan
        .collections
        .iter()
        .find(|collection| collection.id == request.collection_id)
        .cloned()
        .ok_or_else(|| {
            "Collection preview is stale. Re-open Import Review and try again.".to_string()
        })?;
    if collection.preview_id != request.preview_id || collection.canonical_worktree_root != root {
        return Err(
            "Collection preview is stale. Re-open Import Review and try again.".to_string(),
        );
    }
    let mut selected_names = HashSet::new();
    let mut items = Vec::new();
    let mut selected_children = Vec::new();
    let mut targets = Vec::new();
    for selection in &request.selections {
        let child = collection
            .children
            .iter()
            .find(|child| child.relative_path == selection.relative_path)
            .ok_or_else(|| {
                "Selected collection child is not part of the reviewed preview.".to_string()
            })?;
        if child.group_id != selection.group_id || child.variant_id != selection.variant_id {
            return Err("Selected collection child variant is stale.".to_string());
        }
        if child.import_status != ImportCandidateStatus::Importable || child.conflict.is_some() {
            return Err(format!(
                "Collection child {} is not importable.",
                child.name
            ));
        }
        if !selected_names.insert(child.name.to_ascii_lowercase()) {
            return Err(format!(
                "Import review may select only one source variant for skill {}.",
                child.name
            ));
        }
        let source_path = expand_home(child.source_path.clone());
        let skill = read_skill(&source_path)
            .map_err(|error| format!("Collection preview is stale: {error}"))?;
        validate_skill_name(&skill.name)?;
        let source_real_path = fs::canonicalize(&source_path)
            .map_err(|error| format!("Collection preview is stale: {error}"))?;
        let Some(source_relative_path) = safe_collection_relative_path(&root, &source_real_path)
        else {
            return Err("Collection source is outside the reviewed Git worktree.".to_string());
        };
        if source_relative_path.to_string_lossy() != child.relative_path
            || skill.name != child.name
            || skill.content_hash != child.content_hash
            || skill_directory_snapshot_hash(&source_real_path)? != child.snapshot_hash
        {
            return Err(
                "Collection preview is stale. Re-open Import Review and try again.".to_string(),
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
        targets.push(target);
        items.push(ImportRequestItem {
            source_path,
            skill_type: selection.skill_type,
            deploy_back_to_source: false,
        });
        selected_children.push(child.clone());
    }
    let batch = import_candidates_with_paths(&paths, items)?;
    if !batch.errors.is_empty() {
        return Err(collection_apply_error(
            format!(
                "Collection import did not complete: {} skill(s) failed.",
                batch.errors.len()
            ),
            rollback_collection_imports(&paths, &targets),
        ));
    }
    let collection_record = match persist_collection(
        &paths.database_path,
        &collection,
        &selected_children,
        &batch.imported,
    ) {
        Ok(record) => record,
        Err(error) => {
            return Err(collection_apply_error(
                format!("Collection metadata could not be saved: {error}"),
                rollback_collection_imports(&paths, &targets),
            ));
        }
    };
    Ok(ImportCollectionApplyResult {
        collection: collection_record,
        imported: batch.imported,
        errors: batch.errors,
    })
}

struct CollectionImportTarget {
    name: String,
    path: PathBuf,
    remote_root: Option<PathBuf>,
    expected_snapshot_hash: String,
}

fn collection_import_target(
    paths: &ManagedPaths,
    skill: &Skill,
    kind: SkillKind,
) -> CollectionImportTarget {
    match kind {
        SkillKind::User => CollectionImportTarget {
            name: skill.name.clone(),
            path: paths.user_skills_root.join(&skill.name),
            remote_root: None,
            expected_snapshot_hash: skill_directory_snapshot_hash(&skill.real_path)
                .unwrap_or_default(),
        },
        SkillKind::Remote => CollectionImportTarget {
            name: skill.name.clone(),
            path: paths
                .remote_skills_root
                .join(&skill.name)
                .join("versions")
                .join(format!("manual-{}", &skill.content_hash[..12])),
            remote_root: Some(paths.remote_skills_root.join(&skill.name)),
            expected_snapshot_hash: skill_directory_snapshot_hash(&skill.real_path)
                .unwrap_or_default(),
        },
    }
}

fn managed_index_contains(database_path: &Path, skill_name: &str) -> Result<bool> {
    let connection = open_database(database_path)?;
    connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM skills WHERE name = ?1)",
            rusqlite::params![skill_name],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())
}

fn rollback_collection_imports(
    paths: &ManagedPaths,
    targets: &[CollectionImportTarget],
) -> Result<()> {
    let mut errors = Vec::new();
    for target in targets {
        let exists = match fs::symlink_metadata(&target.path) {
            Ok(metadata) if metadata.file_type().is_dir() => true,
            Ok(metadata) if metadata.file_type().is_symlink() => {
                errors.push(format!(
                    "Refusing to remove unexpected symlink at {}",
                    target.path.display()
                ));
                false
            }
            Ok(_) => {
                errors.push(format!(
                    "Refusing to remove unexpected non-directory at {}",
                    target.path.display()
                ));
                false
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
            Err(error) => {
                errors.push(format!(
                    "Unable to inspect {}: {error}",
                    target.path.display()
                ));
                false
            }
        };
        if exists {
            match skill_directory_snapshot_hash(&target.path) {
                Ok(snapshot) if snapshot == target.expected_snapshot_hash => {
                    if let Err(error) = fs::remove_dir_all(&target.path) {
                        errors.push(format!(
                            "Unable to remove {}: {error}",
                            target.path.display()
                        ));
                    } else if let Err(error) =
                        remove_skill_index(&paths.database_path, &target.name)
                    {
                        errors.push(format!(
                            "Unable to restore index for {}: {error}",
                            target.name
                        ));
                    }
                }
                Ok(_) => errors.push(format!(
                    "Preserved {} because its contents changed during collection import.",
                    target.path.display()
                )),
                Err(error) => errors.push(format!(
                    "Preserved {} because its contents could not be verified: {error}",
                    target.path.display()
                )),
            }
        }
        if let Some(remote_root) = &target.remote_root {
            let current = remote_root.join("current");
            match fs::symlink_metadata(&current) {
                Ok(metadata) if metadata.file_type().is_symlink() => {
                    let points_to_target = fs::canonicalize(&current)
                        .ok()
                        .zip(fs::canonicalize(&target.path).ok())
                        .is_some_and(|(current, target)| current == target);
                    if points_to_target {
                        if let Err(error) = fs::remove_file(&current) {
                            errors.push(format!("Unable to remove {}: {error}", current.display()));
                        }
                    }
                }
                Ok(_) => errors.push(format!(
                    "Preserved unexpected remote current entry at {}",
                    current.display()
                )),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    errors.push(format!("Unable to inspect {}: {error}", current.display()))
                }
            }
            let _ = fs::remove_dir(remote_root.join("versions"));
            let _ = fs::remove_dir(remote_root);
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join(" "))
    }
}

fn collection_apply_error(primary: String, rollback: Result<()>) -> String {
    match rollback {
        Ok(()) => primary,
        Err(error) => format!("{primary} Collection rollback was incomplete: {error}"),
    }
}

fn import_candidates_with_paths(
    paths: &ManagedPaths,
    items: Vec<ImportRequestItem>,
) -> Result<ImportBatchResult> {
    validate_unique_import_request_names(&items)?;
    let mut imported = Vec::new();
    let mut errors = Vec::new();
    for item in items {
        let source_path = item.source_path.clone();
        match import_one_candidate(paths, item) {
            Ok(candidate) => imported.push(candidate),
            Err(error) => errors.push(ImportCandidateError { source_path, error }),
        }
    }
    Ok(ImportBatchResult { imported, errors })
}

fn persist_collection(
    database_path: &Path,
    preview: &ImportCandidateCollection,
    children: &[ImportCandidateCollectionChild],
    imported: &[ImportedCandidate],
) -> Result<SkillCollection> {
    let mut connection = open_database(database_path)?;
    let transaction = connection
        .transaction()
        .map_err(|error| error.to_string())?;
    transaction
        .execute(
            "INSERT INTO skill_collections (
                id, display_name, canonical_worktree_root, canonical_repository_id,
                origin_url, branch, detached, reviewed_head_sha, available, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1, ?9)
             ON CONFLICT(id) DO UPDATE SET
                display_name = excluded.display_name,
                canonical_worktree_root = excluded.canonical_worktree_root,
                canonical_repository_id = excluded.canonical_repository_id,
                origin_url = excluded.origin_url,
                branch = excluded.branch,
                detached = excluded.detached,
                reviewed_head_sha = excluded.reviewed_head_sha,
                available = 1,
                updated_at = excluded.updated_at",
            rusqlite::params![
                preview.id,
                preview.display_name,
                preview.canonical_worktree_root.to_string_lossy(),
                preview.canonical_repository_id.to_string_lossy(),
                preview.origin_url,
                preview.branch,
                i64::from(preview.detached),
                preview.reviewed_head_sha,
                current_rfc3339_timestamp(),
            ],
        )
        .map_err(|error| error.to_string())?;
    transaction
        .execute(
            "DELETE FROM skill_collection_members WHERE collection_id = ?1",
            rusqlite::params![preview.id],
        )
        .map_err(|error| error.to_string())?;
    for (child, imported) in children.iter().zip(imported.iter()) {
        transaction
            .execute(
                "INSERT INTO skill_collection_members (
                    collection_id, skill_name, relative_path, reviewed_head_sha,
                    snapshot_hash, content_hash, managed_skill_name
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                rusqlite::params![
                    preview.id,
                    child.name,
                    child.relative_path,
                    preview.reviewed_head_sha,
                    child.snapshot_hash,
                    child.content_hash,
                    imported.name,
                ],
            )
            .map_err(|error| error.to_string())?;
    }
    transaction.commit().map_err(|error| error.to_string())?;
    Ok(SkillCollection {
        id: preview.id.clone(),
        display_name: preview.display_name.clone(),
        canonical_worktree_root: preview.canonical_worktree_root.clone(),
        canonical_repository_id: preview.canonical_repository_id.clone(),
        origin_url: preview.origin_url.clone(),
        branch: preview.branch.clone(),
        detached: preview.detached,
        reviewed_head_sha: preview.reviewed_head_sha.clone(),
        available: true,
        members: children
            .iter()
            .zip(imported)
            .map(|(child, imported)| SkillCollectionMember {
                collection_id: preview.id.clone(),
                skill_name: child.name.clone(),
                relative_path: child.relative_path.clone(),
                reviewed_head_sha: preview.reviewed_head_sha.clone(),
                snapshot_hash: child.snapshot_hash.clone(),
                content_hash: child.content_hash.clone(),
                managed_skill_name: imported.name.clone(),
            })
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origin_sanitization_removes_credentials_and_query() {
        assert_eq!(
            sanitize_origin_url("https://user:secret@example.com/skills.git?token=1#fragment"),
            "https://example.com/skills.git"
        );
        assert_eq!(
            sanitize_origin_url("git@example.com:team/skills.git"),
            "example.com:team/skills.git"
        );
    }

    #[test]
    fn relative_collection_paths_reject_git_and_parent_components() {
        let root = Path::new("/tmp/repo");
        assert_eq!(
            safe_collection_relative_path(root, Path::new("/tmp/repo/skills/demo")),
            Some(PathBuf::from("skills/demo"))
        );
        assert_eq!(
            safe_collection_relative_path(root, Path::new("/tmp/repo/.git")),
            None
        );
    }
}
