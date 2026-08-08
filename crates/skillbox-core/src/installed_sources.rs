use crate::*;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

const MAX_LOCKFILE_BYTES: u64 = 5 * 1024 * 1024;
const MAX_LOCKFILE_ENTRIES: usize = 10_000;
const MAX_LOCKFILE_STRING_BYTES: usize = 4 * 1024;
const SUPPORTED_LOCKFILE_RUNTIME_DIRS: &[&str] = &[".agents", ".claude", ".codex", ".cursor"];

#[derive(Debug, Default)]
pub(crate) struct InstalledSourceDiscoveryStats {
    pub lockfiles_scanned: usize,
    pub lockfile_entries: usize,
    pub lockfile_matches: usize,
    pub invalid_lockfile_entries: usize,
    pub lockfile_errors: usize,
    pub installed_source_collections: usize,
}

#[derive(Debug, Clone)]
struct LockfileEntry {
    root: PathBuf,
    skill_name: String,
    source_url: String,
    skill_path: String,
}

pub(crate) fn discover_installed_source_collections(
    roots: &[PathBuf],
    candidates: &[ImportCandidate],
    groups: &[ImportCandidateGroup],
    live_collection_group_ids: &HashSet<String>,
) -> (
    Vec<ImportCandidateCollection>,
    InstalledSourceDiscoveryStats,
) {
    let mut stats = InstalledSourceDiscoveryStats::default();
    let mut entries = Vec::new();

    for lockfile in configured_lockfiles(roots) {
        match load_lockfile(&lockfile) {
            Ok((mut loaded, invalid_entries)) => {
                stats.lockfiles_scanned += 1;
                stats.lockfile_entries += loaded.len();
                stats.invalid_lockfile_entries += invalid_entries;
                entries.append(&mut loaded);
            }
            Err(LockfileLoadError::InvalidEntryCount) => {
                stats.lockfiles_scanned += 1;
                stats.lockfile_errors += 1;
            }
            Err(LockfileLoadError::Unreadable) => {
                stats.lockfiles_scanned += 1;
                stats.lockfile_errors += 1;
            }
        }
    }

    let mut entries_by_name = HashMap::<String, Vec<LockfileEntry>>::new();
    for entry in entries {
        entries_by_name
            .entry(entry.skill_name.to_ascii_lowercase())
            .or_default()
            .push(entry);
    }

    let groups_by_name = groups
        .iter()
        .map(|group| (group.name.to_ascii_lowercase(), group))
        .collect::<HashMap<_, _>>();
    let mut collection_children =
        BTreeMap::<String, BTreeMap<String, ImportCandidateCollectionChild>>::new();
    let mut seen = HashSet::<(String, String, String)>::new();

    for candidate in candidates {
        let name = candidate.name.to_ascii_lowercase();
        let Some(lock_entries) = entries_by_name.get(&name) else {
            continue;
        };
        let Some(group) = groups_by_name.get(&name) else {
            continue;
        };
        if live_collection_group_ids.contains(&group.id) {
            continue;
        }

        for entry in lock_entries {
            if !candidate_matches_lock_entry(candidate, entry) {
                continue;
            }
            let Some(variant) = group.variants.iter().find(|variant| {
                variant
                    .locations
                    .iter()
                    .any(|location| location.source_path == candidate.source_path)
            }) else {
                continue;
            };
            let key = (
                entry.source_url.clone(),
                group.id.clone(),
                variant.id.clone(),
            );
            if !seen.insert(key) {
                continue;
            }
            stats.lockfile_matches += 1;
            let child = installed_source_child(entry, group, variant);
            collection_children
                .entry(entry.source_url.clone())
                .or_default()
                .insert(child.id.clone(), child);
        }
    }

    let collections = collection_children
        .into_iter()
        .filter_map(|(source_url, children)| {
            let children = children.into_values().collect::<Vec<_>>();
            if children.is_empty() {
                return None;
            }
            let child_seed = children
                .iter()
                .map(|child| {
                    format!(
                        "{}\n{}\n{}\n{}\n{:?}\n{:?}",
                        child.relative_path,
                        child.group_id,
                        child.variant_id,
                        child.snapshot_hash,
                        child.import_status,
                        child.conflict
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            let identity = format!("installed-source-v1\n{source_url}\n{child_seed}");
            let display_name = source_url
                .strip_prefix("https://github.com/")
                .unwrap_or(&source_url)
                .to_string();
            stats.installed_source_collections += 1;
            Some(ImportCandidateCollection {
                id: format!("installed-source-{}", &sha256(&identity)[..16]),
                preview_id: sha256(&identity),
                display_name,
                source_kind: ImportCandidateCollectionSourceKind::InstalledSource,
                canonical_worktree_root: PathBuf::new(),
                canonical_repository_id: PathBuf::new(),
                origin_url: Some(source_url),
                branch: None,
                detached: false,
                reviewed_head_sha: None,
                children,
                errors: Vec::new(),
            })
        })
        .collect::<Vec<_>>();
    (collections, stats)
}

#[derive(Debug, Copy, Clone)]
enum LockfileLoadError {
    Unreadable,
    InvalidEntryCount,
}

fn configured_lockfiles(roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let mut seen = HashSet::new();
    for root in roots {
        let root = expand_home(root.clone());
        let Some(runtime_dir) = root.parent() else {
            continue;
        };
        let Some(runtime_name) = runtime_dir.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !SUPPORTED_LOCKFILE_RUNTIME_DIRS.contains(&runtime_name) {
            continue;
        }
        let lockfile = runtime_dir.join(".skill-lock.json");
        if fs::symlink_metadata(&lockfile).is_err() {
            continue;
        }
        let identity = fs::canonicalize(&lockfile).unwrap_or_else(|_| lockfile.clone());
        if seen.insert(identity) {
            paths.push(lockfile);
        }
    }
    paths.sort();
    paths
}

fn load_lockfile(
    path: &Path,
) -> std::result::Result<(Vec<LockfileEntry>, usize), LockfileLoadError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| LockfileLoadError::Unreadable)?;
    if !metadata.file_type().is_file() || metadata.len() > MAX_LOCKFILE_BYTES {
        return Err(LockfileLoadError::Unreadable);
    }
    let bytes = fs::read(path).map_err(|_| LockfileLoadError::Unreadable)?;
    if bytes.len() as u64 > MAX_LOCKFILE_BYTES {
        return Err(LockfileLoadError::Unreadable);
    }
    let value =
        serde_json::from_slice::<Value>(&bytes).map_err(|_| LockfileLoadError::Unreadable)?;
    if value.get("version").and_then(Value::as_u64) != Some(3) {
        return Err(LockfileLoadError::Unreadable);
    }
    let Some(skills) = value.get("skills").and_then(Value::as_object) else {
        return Err(LockfileLoadError::Unreadable);
    };
    if skills.len() > MAX_LOCKFILE_ENTRIES {
        return Err(LockfileLoadError::InvalidEntryCount);
    }

    let root = path
        .parent()
        .map(|parent| parent.join("skills"))
        .ok_or(LockfileLoadError::Unreadable)?;
    let mut entries = Vec::new();
    let mut invalid_entries = 0;
    for (name, value) in skills {
        let Some(entry) = parse_lockfile_entry(name, value, &root) else {
            invalid_entries += 1;
            continue;
        };
        entries.push(entry);
    }
    Ok((entries, invalid_entries))
}

fn parse_lockfile_entry(name: &str, value: &Value, root: &Path) -> Option<LockfileEntry> {
    if name.is_empty() || name.len() > MAX_LOCKFILE_STRING_BYTES {
        return None;
    }
    let object = value.as_object()?;
    let source_type = bounded_string(object.get("sourceType")?)?;
    if source_type != "github" {
        return None;
    }
    let source_url = bounded_string(object.get("sourceUrl")?)?;
    let source_url = skillbox_github::normalize_github_repo_url(&source_url).ok()?;
    let skill_path = bounded_string(object.get("skillPath")?)?;
    validate_lockfile_skill_path(&skill_path, name)?;
    let plugin_name = object
        .get("pluginName")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    if plugin_name.len() > MAX_LOCKFILE_STRING_BYTES {
        return None;
    }
    if !plugin_name.is_empty()
        && !plugin_name.eq_ignore_ascii_case(name)
        && !plugin_name.eq_ignore_ascii_case(lockfile_skill_name(&skill_path)?)
    {
        return None;
    }
    Some(LockfileEntry {
        root: root.to_path_buf(),
        skill_name: name.to_string(),
        source_url,
        skill_path,
    })
}

fn bounded_string(value: &Value) -> Option<String> {
    let value = value.as_str()?;
    (value.len() <= MAX_LOCKFILE_STRING_BYTES).then(|| value.to_string())
}

fn validate_lockfile_skill_path(value: &str, entry_name: &str) -> Option<()> {
    skillbox_github::validate_repo_relative_path(value).ok()?;
    if !value.ends_with("/SKILL.md") || value.contains('\\') {
        return None;
    }
    let parent_name = lockfile_skill_name(value)?;
    if !parent_name.eq_ignore_ascii_case(entry_name) {
        return None;
    }
    Some(())
}

fn lockfile_skill_name(value: &str) -> Option<&str> {
    let mut parts = value.rsplit('/');
    if parts.next()? != "SKILL.md" {
        return None;
    }
    let name = parts.next()?;
    (!name.is_empty()).then_some(name)
}

fn candidate_matches_lock_entry(candidate: &ImportCandidate, entry: &LockfileEntry) -> bool {
    if !same_path(candidate.source_root.as_deref(), Some(&entry.root)) {
        return false;
    }
    if !candidate.name.eq_ignore_ascii_case(&entry.skill_name) {
        return false;
    }
    let Some(relative) = candidate.source_path.strip_prefix(&entry.root).ok() else {
        return false;
    };
    let mut components = relative.components();
    matches!(
        (components.next(), components.next()),
        (Some(Component::Normal(name)), None)
            if name.to_string_lossy().eq_ignore_ascii_case(&entry.skill_name)
    )
}

fn same_path(left: Option<&Path>, right: Option<&Path>) -> bool {
    let (Some(left), Some(right)) = (left, right) else {
        return false;
    };
    let left = fs::canonicalize(left).unwrap_or_else(|_| left.to_path_buf());
    let right = fs::canonicalize(right).unwrap_or_else(|_| right.to_path_buf());
    left == right
}

fn installed_source_child(
    entry: &LockfileEntry,
    group: &ImportCandidateGroup,
    variant: &ImportCandidateVariant,
) -> ImportCandidateCollectionChild {
    let candidate = &variant.candidate;
    let actionable = candidate.import_status == ImportCandidateStatus::Importable
        && candidate.conflict.is_none();
    let selected_type = if actionable {
        variant.selected_type
    } else {
        Some(candidate.suggested_type)
    };
    let relative_path = entry
        .skill_path
        .strip_suffix("/SKILL.md")
        .unwrap_or(&entry.skill_path)
        .to_string();
    let identity = format!(
        "{}\n{}\n{}\n{}",
        entry.source_url, group.id, variant.id, relative_path
    );
    ImportCandidateCollectionChild {
        id: format!("child-{}", &sha256(&identity)[..16]),
        group_id: group.id.clone(),
        variant_id: variant.id.clone(),
        name: candidate.name.clone(),
        relative_path,
        source_path: candidate.source_path.clone(),
        real_path: candidate.real_path.clone(),
        content_hash: candidate.content_hash.clone(),
        snapshot_hash: variant.snapshot_hash.clone(),
        import_status: candidate.import_status,
        conflict: candidate.conflict.clone(),
        usage_count: candidate.usage_count,
        locations: variant.locations.clone(),
        unlinked_locations: Vec::new(),
        suggested_types: variant.suggested_types.clone(),
        requires_type_review: actionable && variant.requires_type_review,
        selected_type,
        is_selected: candidate.is_selected && selected_type.is_some(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lockfile_skill_paths_require_a_safe_nested_skill_md() {
        assert!(validate_lockfile_skill_path("skills/demo/SKILL.md", "demo").is_some());
        assert!(validate_lockfile_skill_path("skills/demo/skill.md", "demo").is_none());
        assert!(validate_lockfile_skill_path("skills/../demo/SKILL.md", "demo").is_none());
        assert!(validate_lockfile_skill_path("/tmp/demo/SKILL.md", "demo").is_none());
        assert!(validate_lockfile_skill_path("skills/other/SKILL.md", "demo").is_none());
    }
}
