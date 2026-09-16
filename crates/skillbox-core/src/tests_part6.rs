use super::*;
use crate::test_support::*;
use std::fs;

#[test]
fn scan_import_candidates_keeps_distinct_imported_targets_separate() {
    let root = temp_dir("candidate-imported-distinct-targets");
    let agents_root = root.join("global").join(".agents").join("skills");
    let codex_root = root.join("project").join(".codex").join("skills");
    let managed_root = root.join("SkillBox");
    let user_managed = managed_root.join("user-skills/demo");
    let remote_version = managed_root.join("remote-skills/demo/versions/manual-test");
    let remote_current = managed_root.join("remote-skills/demo/current");
    make_skill(&user_managed, "demo", "Demo skill");
    make_skill(&remote_version, "demo", "Demo skill");
    fs::write(user_managed.join("prompt.md"), "user\n").unwrap();
    fs::write(remote_version.join("prompt.md"), "remote\n").unwrap();
    symlink_dir(&remote_version, &remote_current).unwrap();
    fs::create_dir_all(&agents_root).unwrap();
    fs::create_dir_all(&codex_root).unwrap();
    symlink_dir(&user_managed, &agents_root.join("demo")).unwrap();
    symlink_dir(&remote_current, &codex_root.join("demo")).unwrap();

    let candidates = scan_import_candidates(&[agents_root, codex_root], &managed_root).unwrap();

    assert_eq!(candidates.candidates.len(), 2);
    assert!(candidates
        .candidates
        .iter()
        .all(|candidate| candidate.import_status == ImportCandidateStatus::Imported));
    assert_ne!(
        candidates.candidates[0].real_path,
        candidates.candidates[1].real_path
    );
    assert_eq!(candidates.groups.len(), 1);
    assert_eq!(candidates.groups[0].variants.len(), 2);
    assert!(candidates.groups[0].selected_variant_id.is_none());
    let user_candidate = candidates
        .candidates
        .iter()
        .find(|candidate| is_under_path(&candidate.real_path, &user_managed))
        .unwrap();
    let remote_candidate = candidates
        .candidates
        .iter()
        .find(|candidate| is_under_path(&candidate.real_path, &remote_version))
        .unwrap();
    assert_eq!(user_candidate.suggested_type, SkillKind::User);
    assert_eq!(remote_candidate.suggested_type, SkillKind::Remote);
}

#[test]
fn scan_import_candidates_uses_total_usage_for_imported_skills() {
    let root = temp_dir("candidate-imported-usage");
    let first_root = root.join("global").join(".codex").join("skills");
    let second_root = root.join("project").join(".codex").join("skills");
    let first_source = first_root.join("demo");
    let second_source = second_root.join("demo");
    let managed_root = root.join("SkillBox");
    make_skill(&first_source, "demo", "Demo skill");

    let result = import_candidates(
        vec![ImportRequestItem {
            source_path: first_source.clone(),
            skill_type: SkillKind::User,
            deploy_back_to_source: true,
        }],
        &managed_root,
    )
    .unwrap();
    fs::create_dir_all(&second_root).unwrap();
    symlink_dir(&result.imported[0].managed_path, &second_source).unwrap();

    record_test_call(
        RecordSkillUsageRequest {
            skill_name: "demo".to_string(),
            agent_id: "codex".to_string(),
            runtime_root: second_root.clone(),
            event_id: None,
            used_at: Some("2026-06-02T12:00:00Z".to_string()),
            prompt_excerpt: None,
            metadata: None,
        },
        &managed_root,
    )
    .unwrap();
    record_test_call(
        RecordSkillUsageRequest {
            skill_name: "demo".to_string(),
            agent_id: "codex".to_string(),
            runtime_root: second_root.clone(),
            event_id: None,
            used_at: Some("2026-06-02T12:01:00Z".to_string()),
            prompt_excerpt: None,
            metadata: None,
        },
        &managed_root,
    )
    .unwrap();

    let candidates =
        scan_import_candidates(&[first_root.clone(), second_root.clone()], &managed_root).unwrap();

    assert_eq!(candidates.candidates.len(), 1);
    let demo = candidate(&candidates.candidates, "demo");
    assert_eq!(demo.import_status, ImportCandidateStatus::Imported);
    assert_eq!(demo.usage_count, 2);
    assert_eq!(candidates.groups[0].usage_count, 2);
}

#[test]
fn scan_import_candidates_skips_unmanaged_symlinked_sources() {
    let root = temp_dir("candidate-unmanaged-symlink");
    let runtime_root = root.join("runtime");
    let outside = temp_dir("candidate-unmanaged-symlink-outside");
    let managed_root = root.join("SkillBox");
    make_skill(&outside.join("demo"), "demo", "Demo skill");
    fs::create_dir_all(&runtime_root).unwrap();
    symlink_dir(&outside.join("demo"), &runtime_root.join("demo")).unwrap();

    let candidates = scan_import_candidates(&[runtime_root], &managed_root).unwrap();

    assert!(candidates.candidates.is_empty());
}

#[test]
fn import_candidates_copies_remote_skill_updates_current_and_symlinks_source_to_current() {
    let root = temp_dir("candidate-import-remote");
    let source = root.join("runtime").join("remote-demo");
    let managed_root = root.join("SkillBox");
    make_skill(&source, "remote-demo", "Remote demo skill");

    let result = import_candidates(
        vec![ImportRequestItem {
            source_path: source.clone(),
            skill_type: SkillKind::Remote,
            deploy_back_to_source: true,
        }],
        &managed_root,
    )
    .unwrap();

    assert_eq!(result.errors.len(), 0);
    assert_eq!(result.imported.len(), 1);
    let current = managed_root
        .join("remote-skills")
        .join("remote-demo")
        .join("current");
    assert!(fs::symlink_metadata(&current)
        .unwrap()
        .file_type()
        .is_symlink());
    assert!(current.join("SKILL.md").exists());
    assert!(fs::symlink_metadata(&source)
        .unwrap()
        .file_type()
        .is_symlink());
    assert_eq!(
        fs::canonicalize(&source).unwrap(),
        fs::canonicalize(&current).unwrap()
    );
}

#[test]
fn remote_import_rejects_same_skill_md_with_different_assets() {
    let root = temp_dir("candidate-remote-asset-conflict");
    let first_source = root.join("first/remote-demo");
    let second_source = root.join("second/remote-demo");
    let managed_root = root.join("SkillBox");
    make_skill(&first_source, "remote-demo", "Remote demo skill");
    make_skill(&second_source, "remote-demo", "Remote demo skill");
    fs::write(first_source.join("prompt.md"), "first\n").unwrap();
    fs::write(second_source.join("prompt.md"), "second\n").unwrap();

    let first_result = import_candidates(
        vec![ImportRequestItem {
            source_path: first_source.clone(),
            skill_type: SkillKind::Remote,
            deploy_back_to_source: true,
        }],
        &managed_root,
    )
    .unwrap();
    let result = import_candidates(
        vec![ImportRequestItem {
            source_path: second_source.clone(),
            skill_type: SkillKind::Remote,
            deploy_back_to_source: true,
        }],
        &managed_root,
    )
    .unwrap();

    assert_eq!(first_result.imported.len(), 1);
    assert!(first_result.errors.is_empty());
    assert!(result.imported.is_empty());
    assert_eq!(result.errors.len(), 1);
    assert!(fs::symlink_metadata(&first_source)
        .unwrap()
        .file_type()
        .is_symlink());
    assert!(second_source.is_dir());
    assert!(!fs::symlink_metadata(&second_source)
        .unwrap()
        .file_type()
        .is_symlink());
    assert_eq!(
        fs::read_to_string(
            managed_root
                .join("remote-skills/remote-demo/current")
                .join("prompt.md")
        )
        .unwrap(),
        "first\n"
    );
}

#[test]
fn scan_import_candidates_reports_conflicting_managed_target() {
    let root = temp_dir("candidate-conflict");
    let source = root.join("runtime").join("demo");
    let managed_root = root.join("SkillBox");
    make_skill(&source, "demo", "Runtime version");
    make_skill(
        &managed_root.join("user-skills").join("demo"),
        "demo",
        "Managed version",
    );

    let candidates = scan_import_candidates(&[root.join("runtime")], &managed_root).unwrap();

    let demo = candidate(&candidates.candidates, "demo");
    assert!(demo
        .conflict
        .as_ref()
        .unwrap()
        .contains("Managed target exists"));
    assert!(!demo.is_selected);
}

#[test]
fn same_skill_md_with_different_assets_cannot_reuse_user_target() {
    let root = temp_dir("candidate-asset-conflict");
    let source = root.join("runtime").join("demo");
    let managed_root = root.join("SkillBox");
    let managed_source = managed_root.join("user-skills").join("demo");
    make_skill(&source, "demo", "Demo skill");
    make_skill(&managed_source, "demo", "Demo skill");
    fs::write(source.join("prompt.md"), "runtime\n").unwrap();
    fs::write(managed_source.join("prompt.md"), "managed\n").unwrap();

    let candidates = scan_import_candidates(&[root.join("runtime")], &managed_root).unwrap();
    let demo = candidate(&candidates.candidates, "demo");
    assert!(demo
        .conflict
        .as_ref()
        .unwrap()
        .contains("Managed target exists"));
    assert!(!demo.is_selected);

    let result = import_candidates(
        vec![ImportRequestItem {
            source_path: source.clone(),
            skill_type: SkillKind::User,
            deploy_back_to_source: true,
        }],
        &managed_root,
    )
    .unwrap();

    assert!(result.imported.is_empty());
    assert_eq!(result.errors.len(), 1);
    assert!(source.is_dir());
    assert!(!fs::symlink_metadata(&source)
        .unwrap()
        .file_type()
        .is_symlink());
    assert_eq!(
        fs::read_to_string(managed_source.join("prompt.md")).unwrap(),
        "managed\n"
    );
}
