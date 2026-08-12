use crate::*;
use skillbox_git::{GitRepositoryIdentity, GitService};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

const MAX_COLLECTION_CHILDREN: usize = 500;
const MAX_COLLECTION_ERRORS: usize = 32;

#[derive(Debug, Default)]
pub(crate) struct CollectionDiscoveryStats {
    pub unique_repository_count: usize,
    pub repository_inspections: usize,
    pub repository_cache_hits: usize,
    pub snapshot_hash_computations: usize,
    pub snapshot_cache_hits: usize,
}

pub(crate) fn discover_import_collections_with_progress<F>(
    candidates: &[ImportCandidate],
    groups: &[ImportCandidateGroup],
    mut progress: F,
) -> (Vec<ImportCandidateCollection>, CollectionDiscoveryStats)
where
    F: FnMut(usize, usize, usize),
{
    let git = GitService::new();
    let mut builders = BTreeMap::<(PathBuf, PathBuf), CollectionBuilder>::new();
    let mut identity_cache = HashMap::<PathBuf, Option<GitRepositoryIdentity>>::new();
    let mut snapshot_cache = HashMap::<PathBuf, String>::new();
    let mut unique_repository_keys = HashSet::<(PathBuf, PathBuf)>::new();
    let mut stats = CollectionDiscoveryStats::default();

    for (index, candidate) in candidates.iter().enumerate() {
        let identity_cache_key = repository_identity_cache_key(&candidate.real_path);
        let identity = match identity_cache.entry(identity_cache_key) {
            std::collections::hash_map::Entry::Occupied(entry) => {
                stats.repository_cache_hits += 1;
                entry.get().clone()
            }
            std::collections::hash_map::Entry::Vacant(entry) => {
                stats.repository_inspections += 1;
                let identity = git.repository_identity(&candidate.real_path).ok().flatten();
                entry.insert(identity.clone());
                identity
            }
        };
        let identity = match identity {
            Some(identity) => identity,
            _ => {
                progress(index + 1, candidates.len(), builders.len());
                continue;
            }
        };
        unique_repository_keys
            .insert((identity.worktree_root.clone(), identity.common_dir.clone()));
        let unique_repository_count = unique_repository_keys.len();
        let Some(relative_path) =
            safe_collection_relative_path(&identity.worktree_root, &candidate.real_path)
        else {
            progress(index + 1, candidates.len(), unique_repository_count);
            continue;
        };
        if relative_path.as_os_str().is_empty() {
            progress(index + 1, candidates.len(), unique_repository_count);
            continue;
        }
        let Some(group) = groups
            .iter()
            .find(|group| group.name.eq_ignore_ascii_case(&candidate.name))
        else {
            progress(index + 1, candidates.len(), unique_repository_count);
            continue;
        };
        let Some(variant) = group.variants.iter().find(|variant| {
            variant
                .locations
                .iter()
                .any(|location| location.source_path == candidate.source_path)
        }) else {
            progress(index + 1, candidates.len(), unique_repository_count);
            continue;
        };
        let snapshot_hash = if !variant.snapshot_hash.is_empty() {
            variant.snapshot_hash.clone()
        } else {
            match snapshot_cache.entry(candidate.real_path.clone()) {
                std::collections::hash_map::Entry::Occupied(entry) => {
                    stats.snapshot_cache_hits += 1;
                    entry.get().clone()
                }
                std::collections::hash_map::Entry::Vacant(entry) => {
                    stats.snapshot_hash_computations += 1;
                    let hash = match skill_directory_snapshot_hash(&candidate.real_path) {
                        Ok(hash) => hash,
                        Err(_) => {
                            progress(index + 1, candidates.len(), unique_repository_count);
                            continue;
                        }
                    };
                    entry.insert(hash.clone());
                    hash
                }
            }
        };
        let key = (identity.worktree_root.clone(), identity.common_dir.clone());
        let builder = builders.entry(key).or_insert_with(|| CollectionBuilder {
            identity,
            children: BTreeMap::new(),
            errors: Vec::new(),
        });
        let child_key = format!("{relative_path:?}\n{}", variant.id);
        if builder.children.contains_key(&child_key) {
            progress(index + 1, candidates.len(), unique_repository_count);
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
        let actionable = candidate.import_status == ImportCandidateStatus::Importable
            && candidate.conflict.is_none();
        let selected_type = if actionable {
            variant.selected_type
        } else {
            Some(candidate.suggested_type)
        };
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
                diff: String::new(),
                import_status: candidate.import_status,
                conflict: candidate.conflict.clone(),
                usage_count: candidate.usage_count,
                locations,
                unlinked_locations,
                suggested_types: variant.suggested_types.clone(),
                requires_type_review: actionable && variant.requires_type_review,
                selected_type,
                is_selected: variant.candidate.is_selected && selected_type.is_some(),
            },
        );
        progress(index + 1, candidates.len(), unique_repository_count);
    }

    stats.unique_repository_count = unique_repository_keys.len();
    let collections = builders
        .into_values()
        .filter_map(|builder| builder.finish())
        .collect();
    (collections, stats)
}

fn repository_identity_cache_key(path: &Path) -> PathBuf {
    let canonical = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let mut current = canonical.clone();
    loop {
        if fs::symlink_metadata(current.join(".git")).is_ok() {
            return current.join(".git");
        }
        if !current.pop() {
            return canonical;
        }
    }
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
            source_kind: ImportCandidateCollectionSourceKind::GitWorktree,
            canonical_worktree_root: self.identity.worktree_root,
            canonical_repository_id: self.identity.common_dir,
            origin_url,
            branch: self.identity.branch,
            detached,
            reviewed_head_sha: self.identity.head,
            source_url: None,
            requested_reference: None,
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

pub(crate) fn sanitize_origin_url(value: &str) -> String {
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

fn collection_source_kind_string(kind: ImportCandidateCollectionSourceKind) -> &'static str {
    match kind {
        ImportCandidateCollectionSourceKind::GitWorktree => "git_worktree",
        ImportCandidateCollectionSourceKind::InstalledSource => "installed_source",
        ImportCandidateCollectionSourceKind::GithubRemote => "github_remote",
    }
}

fn parse_collection_source_kind(
    value: &str,
) -> rusqlite::Result<ImportCandidateCollectionSourceKind> {
    match value {
        "git_worktree" => Ok(ImportCandidateCollectionSourceKind::GitWorktree),
        "installed_source" => Ok(ImportCandidateCollectionSourceKind::InstalledSource),
        "github_remote" => Ok(ImportCandidateCollectionSourceKind::GithubRemote),
        _ => Err(rusqlite::Error::InvalidColumnType(
            0,
            "source_kind".to_string(),
            rusqlite::types::Type::Text,
        )),
    }
}

pub fn list_skill_collections(managed_root: impl AsRef<Path>) -> Result<Vec<SkillCollection>> {
    let paths = ensure_managed_layout(expand_home(managed_root.as_ref().to_path_buf()))?;
    let connection = open_database(&paths.database_path)?;
    let mut statement = connection
        .prepare(
            "SELECT id, display_name, canonical_worktree_root, canonical_repository_id,
                    origin_url, branch, detached, reviewed_head_sha, source_kind, source_url,
                    requested_reference, available
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
                source_kind: parse_collection_source_kind(&row.get::<_, String>(8)?)?,
                source_url: row.get(9)?,
                requested_reference: row.get(10)?,
                available: row.get::<_, i64>(11)? != 0,
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
        if collection.source_kind == ImportCandidateCollectionSourceKind::GitWorktree {
            collection.available = collection.canonical_worktree_root.is_dir();
        }
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
    if collection.source_kind != ImportCandidateCollectionSourceKind::GitWorktree {
        return Err(
            "Installed source collections use the regular per-skill import flow.".to_string(),
        );
    }
    if collection.preview_id != request.preview_id || collection.canonical_worktree_root != root {
        return Err(
            "Collection preview is stale. Re-open Import Review and try again.".to_string(),
        );
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
        items.push(ImportRequestItem {
            source_path,
            skill_type: selection.skill_type,
            deploy_back_to_source: false,
        });
        selected_children.push(child.clone());
    }
    apply_collection_import_with_audit(
        &paths,
        &collection,
        &selected_children,
        items,
        &request.actor,
    )
}

pub(crate) struct CollectionImportTarget {
    pub(crate) name: String,
    pub(crate) path: PathBuf,
    pub(crate) remote_root: Option<PathBuf>,
    pub(crate) expected_snapshot_hash: String,
}

pub(crate) fn collection_import_target(
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

fn collection_import_targets_from_imported(
    imported: &[ImportedCandidate],
) -> Vec<CollectionImportTarget> {
    imported
        .iter()
        .map(|result| {
            let remote_root = match result.kind {
                SkillKind::User => None,
                SkillKind::Remote => result
                    .managed_path
                    .parent()
                    .and_then(Path::parent)
                    .map(Path::to_path_buf),
            };
            let expected_snapshot_hash =
                skill_directory_snapshot_hash(&result.managed_path).unwrap_or_default();
            CollectionImportTarget {
                name: result.name.clone(),
                path: result.managed_path.clone(),
                remote_root,
                expected_snapshot_hash,
            }
        })
        .collect()
}

pub(crate) fn managed_index_contains(database_path: &Path, skill_name: &str) -> Result<bool> {
    if !database_path.is_file() {
        return Ok(false);
    }
    let connection = rusqlite::Connection::open_with_flags(
        database_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .map_err(|error| error.to_string())?;
    connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM skills WHERE name = ?1)",
            rusqlite::params![skill_name],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())
}

pub(crate) fn rollback_collection_imports(
    paths: &ManagedPaths,
    targets: &[CollectionImportTarget],
) -> Result<()> {
    let mut errors = Vec::new();
    for target in targets {
        let current = target.remote_root.as_ref().map(|root| root.join("current"));
        let current_points_to_target = current.as_ref().is_some_and(|current| {
            matches!(
                (fs::canonicalize(current), fs::canonicalize(&target.path)),
                (Ok(current), Ok(target_path)) if current == target_path
            )
        });
        if current_points_to_target {
            if let Some(current) = &current {
                if let Err(error) = fs::remove_file(current) {
                    errors.push(format!("Unable to remove {}: {error}", current.display()));
                }
            }
        }
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
                    errors.push(format!(
                        "Preserved unexpected remote current entry at {}",
                        current.display()
                    ));
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

pub(crate) fn import_candidates_with_paths(
    paths: &ManagedPaths,
    items: Vec<ImportRequestItem>,
) -> Result<ImportBatchResult> {
    validate_unique_import_request_names(&items)?;
    let mut imported = Vec::new();
    let mut errors = Vec::new();
    for item in items {
        let source_path = item.source_path.clone();
        match import_one_candidate_unlogged(paths, item) {
            Ok(candidate) => imported.push(candidate),
            Err(error) => errors.push(ImportCandidateError { source_path, error }),
        }
    }
    Ok(ImportBatchResult { imported, errors })
}

pub(crate) fn apply_collection_import_with_audit(
    paths: &ManagedPaths,
    collection: &ImportCandidateCollection,
    selected_children: &[ImportCandidateCollectionChild],
    items: Vec<ImportRequestItem>,
    actor: &str,
) -> Result<ImportCollectionApplyResult> {
    let selected_names = selected_children
        .iter()
        .map(|child| child.name.clone())
        .collect::<Vec<_>>();
    let operation = start_operation(
        OperationStart {
            operation_type: "import_collection".to_string(),
            actor: actor.to_string(),
            entity_type: "skill_collection".to_string(),
            entity_name: collection.display_name.clone(),
            summary: format!("Import selected skills from {}", collection.display_name),
            payload: serde_json::json!({
                "collectionId": collection.id,
                "sourceKind": collection.source_kind,
                "sourceUrl": collection.source_url,
                "reviewedHeadSha": collection.reviewed_head_sha,
                "selectedSkillNames": selected_names,
                "phase": "validated"
            }),
        },
        &paths.root,
    )?;

    let fail = |primary: String,
                phase: &str,
                imported: &[ImportedCandidate]|
     -> Result<ImportCollectionApplyResult> {
        let receipt_targets = collection_import_targets_from_imported(imported);
        let rollback = rollback_collection_imports(paths, &receipt_targets);
        let (rollback_outcome, rollback_error) = match rollback {
            Ok(()) => ("succeeded", None),
            Err(error) => ("partial", Some(error)),
        };
        let mut error_message = primary.clone();
        if let Some(error) = rollback_error.as_deref() {
            error_message.push_str(&format!(" Collection rollback was incomplete: {error}"));
        }
        let payload = serde_json::json!({
            "collectionId": collection.id,
            "reviewedHeadSha": collection.reviewed_head_sha,
            "selectedSkillNames": selected_names,
            "phase": phase,
            "rollback": rollback_outcome,
            "partialRecovery": rollback_outcome == "partial"
        });
        match finish_operation(
            OperationFinish {
                id: operation.id.clone(),
                status: OperationStatus::Failed,
                summary: format!("Collection import failed for {}", collection.display_name),
                error: Some(error_message.clone()),
                payload,
            },
            &paths.root,
        ) {
            Ok(_) => Err(error_message),
            Err(log_error) => Err(format!(
                "{error_message} (operation log failed: {log_error})"
            )),
        }
    };

    let batch = match import_candidates_with_paths(paths, items) {
        Ok(batch) => batch,
        Err(error) => return fail(error, "import_validation", &[]),
    };
    if !batch.errors.is_empty() {
        return fail(
            format!(
                "Collection import did not complete: {} skill(s) failed.",
                batch.errors.len()
            ),
            "import",
            &batch.imported,
        );
    }
    let collection_record = match persist_collection(
        &paths.database_path,
        collection,
        selected_children,
        &batch.imported,
    ) {
        Ok(record) => record,
        Err(error) => {
            return fail(
                format!("Collection metadata could not be saved: {error}"),
                "persist",
                &batch.imported,
            )
        }
    };
    let mut warnings = Vec::new();
    if let Err(error) = finish_operation(
        OperationFinish {
            id: operation.id,
            status: OperationStatus::Succeeded,
            summary: format!("Imported selected skills from {}", collection.display_name),
            error: None,
            payload: serde_json::json!({
                "collectionId": collection.id,
                "reviewedHeadSha": collection.reviewed_head_sha,
                "selectedSkillNames": selected_names,
                "importedSkillNames": batch.imported.iter().map(|item| item.name.clone()).collect::<Vec<_>>(),
                "phase": "completed"
            }),
        },
        &paths.root,
    ) {
        warnings.push(format!("Collection import completed, but its operation history could not be finalized: {error}"));
    }
    Ok(ImportCollectionApplyResult {
        collection: collection_record,
        imported: batch.imported,
        errors: batch.errors,
        warnings,
    })
}

pub(crate) fn persist_collection(
    database_path: &Path,
    preview: &ImportCandidateCollection,
    children: &[ImportCandidateCollectionChild],
    imported: &[ImportedCandidate],
) -> Result<SkillCollection> {
    let mut connection = open_database(database_path)?;
    let transaction = connection
        .transaction()
        .map_err(|error| error.to_string())?;
    let existing_reviewed_sha: Option<Option<String>> = transaction
        .query_row(
            "SELECT reviewed_head_sha FROM skill_collections WHERE id = ?1",
            rusqlite::params![preview.id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    if let Some(existing_reviewed_sha) = existing_reviewed_sha {
        if existing_reviewed_sha.as_ref() != preview.reviewed_head_sha.as_ref() {
            return Err(
                "Collection already has a different reviewed SHA; collection updates are not available until Phase D."
                    .to_string(),
            );
        }
    }
    transaction
        .execute(
            "INSERT INTO skill_collections (
                id, display_name, canonical_worktree_root, canonical_repository_id,
                origin_url, branch, detached, reviewed_head_sha, source_kind, source_url,
                requested_reference, available, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 1, ?12)
             ON CONFLICT(id) DO UPDATE SET
                display_name = excluded.display_name,
                canonical_worktree_root = excluded.canonical_worktree_root,
                canonical_repository_id = excluded.canonical_repository_id,
                origin_url = excluded.origin_url,
                branch = excluded.branch,
                detached = excluded.detached,
                reviewed_head_sha = excluded.reviewed_head_sha,
                source_kind = excluded.source_kind,
                source_url = excluded.source_url,
                requested_reference = excluded.requested_reference,
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
                collection_source_kind_string(preview.source_kind),
                preview.source_url,
                preview.requested_reference,
                current_rfc3339_timestamp(),
            ],
        )
        .map_err(|error| error.to_string())?;
    for (child, imported) in children.iter().zip(imported.iter()) {
        transaction
            .execute(
                "INSERT INTO skill_collection_members (
                    collection_id, skill_name, relative_path, reviewed_head_sha,
                    snapshot_hash, content_hash, managed_skill_name
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                 ON CONFLICT(collection_id, relative_path) DO UPDATE SET
                    skill_name = excluded.skill_name,
                    reviewed_head_sha = excluded.reviewed_head_sha,
                    snapshot_hash = excluded.snapshot_hash,
                    content_hash = excluded.content_hash,
                    managed_skill_name = excluded.managed_skill_name",
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
    let members = transaction
        .prepare(
            "SELECT collection_id, skill_name, relative_path, reviewed_head_sha,
                    snapshot_hash, content_hash, managed_skill_name
               FROM skill_collection_members
              WHERE collection_id = ?1
              ORDER BY relative_path",
        )
        .map_err(|error| error.to_string())?
        .query_map(rusqlite::params![preview.id], |row| {
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
        source_kind: preview.source_kind,
        source_url: preview.source_url.clone(),
        requested_reference: preview.requested_reference.clone(),
        available: true,
        members,
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
