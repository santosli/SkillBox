use super::*;
use crate::test_support::*;
use std::fs;

#[test]
fn scan_import_candidates_does_not_merge_github_and_well_known_provenance() {
    let root = temp_dir("candidate-installed-source-kind-separation");
    let agents_root = root.join(".agents/skills");
    let managed_root = root.join("SkillBox");
    let github_names = ["github-alpha", "github-beta"];
    let well_known_names = ["known-alpha", "known-beta"];
    let shared_url = "https://github.com/acme/skills";

    for name in github_names.into_iter().chain(well_known_names) {
        make_skill(&agents_root.join(name), name, "Installed source skill");
    }
    fs::create_dir_all(root.join(".agents")).unwrap();
    fs::write(
        root.join(".agents/.skill-lock.json"),
        serde_json::to_vec(&serde_json::json!({
            "version": 3,
            "skills": {
                "github-alpha": {
                    "sourceType": "github",
                    "sourceUrl": format!("{shared_url}.git"),
                    "skillPath": "skills/github-alpha/SKILL.md"
                },
                "github-beta": {
                    "sourceType": "github",
                    "sourceUrl": format!("{shared_url}.git"),
                    "skillPath": "skills/github-beta/SKILL.md"
                },
                "known-alpha": {
                    "sourceType": "well-known",
                    "sourceBaseUrl": shared_url,
                    "sourceUrl": format!("{shared_url}/.well-known/skills/known-alpha/SKILL.md"),
                    "wellKnownDigest": format!("sha256:{}", "e".repeat(64))
                },
                "known-beta": {
                    "sourceType": "well-known",
                    "sourceBaseUrl": shared_url,
                    "sourceUrl": format!("{shared_url}/.well-known/skills/known-beta/SKILL.md"),
                    "wellKnownDigest": format!("sha256:{}", "f".repeat(64))
                }
            }
        }))
        .unwrap(),
    )
    .unwrap();

    let scan = scan_import_candidates(std::slice::from_ref(&agents_root), &managed_root).unwrap();

    assert_eq!(scan.collections.len(), 2);
    assert!(scan
        .collections
        .iter()
        .all(|collection| collection.children.len() == 2));
    assert_eq!(scan.diagnostics.installed_source_lockfile_matches, 4);
    assert_eq!(scan.diagnostics.installed_source_collections, 2);
}

#[test]
fn scan_import_candidates_rejects_unsafe_or_mismatched_well_known_provenance() {
    let root = temp_dir("candidate-installed-source-well-known-safety");
    let agents_root = root.join(".agents/skills");
    let managed_root = root.join("SkillBox");
    let names = [
        "lark-query",
        "lark-mismatch",
        "lark-bad-digest",
        "lark-wrong-type",
    ];
    for name in names {
        make_skill(&agents_root.join(name), name, "Unsafe provenance skill");
    }
    fs::create_dir_all(root.join(".agents")).unwrap();
    fs::write(
        root.join(".agents/.skill-lock.json"),
        serde_json::to_vec(&serde_json::json!({
            "version": 3,
            "skills": {
                "lark-query": {
                    "sourceType": "well-known",
                    "sourceBaseUrl": "https://open.feishu.cn?token=secret",
                    "sourceUrl": "https://open.feishu.cn/.well-known/skills/lark-query/SKILL.md",
                    "wellKnownDigest": format!("sha256:{}", "c".repeat(64))
                },
                "lark-mismatch": {
                    "sourceType": "well-known",
                    "sourceBaseUrl": "https://open.feishu.cn",
                    "sourceUrl": "https://open.feishu.cn/.well-known/skills/other/SKILL.md",
                    "wellKnownDigest": format!("sha256:{}", "d".repeat(64))
                },
                "lark-bad-digest": {
                    "sourceType": "well-known",
                    "sourceBaseUrl": "https://open.feishu.cn",
                    "sourceUrl": "https://open.feishu.cn/.well-known/skills/lark-bad-digest/SKILL.md",
                    "wellKnownDigest": "sha256:not-a-digest"
                },
                "lark-wrong-type": {
                    "sourceType": "custom",
                    "sourceBaseUrl": "https://open.feishu.cn",
                    "sourceUrl": "https://open.feishu.cn/.well-known/skills/lark-wrong-type/SKILL.md",
                    "wellKnownDigest": format!("sha256:{}", "e".repeat(64))
                }
            }
        }))
        .unwrap(),
    )
    .unwrap();

    let scan = scan_import_candidates(std::slice::from_ref(&agents_root), &managed_root).unwrap();

    assert!(scan.collections.is_empty());
    assert_eq!(scan.standalone_groups.len(), names.len());
    assert_eq!(scan.diagnostics.installed_source_lockfile_matches, 0);
    assert_eq!(
        scan.diagnostics.installed_source_invalid_entries,
        names.len()
    );
    assert_eq!(scan.diagnostics.installed_source_collections, 0);
}

#[test]
fn scan_import_candidates_keeps_single_installed_source_match_standalone() {
    let root = temp_dir("candidate-installed-source-lockfile-singleton");
    let agents_root = root.join(".agents/skills");
    let managed_root = root.join("SkillBox");
    make_skill(
        &agents_root.join("standalone-source-skill"),
        "standalone-source-skill",
        "Single installed source skill",
    );
    fs::create_dir_all(root.join(".agents")).unwrap();
    fs::write(
        root.join(".agents/.skill-lock.json"),
        serde_json::to_vec(&serde_json::json!({
            "version": 3,
            "skills": {
                "standalone-source-skill": {
                    "sourceType": "github",
                    "sourceUrl": "https://github.com/acme/skills.git",
                    "skillPath": "skills/standalone-source-skill/SKILL.md",
                    "skillFolderHash": "stale"
                }
            }
        }))
        .unwrap(),
    )
    .unwrap();

    let scan = scan_import_candidates(std::slice::from_ref(&agents_root), &managed_root).unwrap();

    assert!(scan.collections.is_empty());
    assert_eq!(scan.standalone_groups.len(), 1);
    assert_eq!(scan.diagnostics.installed_source_lockfile_matches, 1);
    assert_eq!(scan.diagnostics.installed_source_collections, 0);
}

#[test]
fn scan_import_candidates_keeps_unsafe_lockfile_entries_standalone_and_live_git_wins() {
    let root = temp_dir("candidate-installed-source-safety");
    let agents_root = root.join(".agents/skills");
    let managed_root = root.join("SkillBox");
    make_skill(&agents_root.join("safe-skill"), "safe-skill", "Safe skill");
    make_skill(
        &agents_root.join("unsafe-skill"),
        "unsafe-skill",
        "Unsafe skill",
    );
    fs::create_dir_all(root.join(".agents")).unwrap();
    fs::write(
        root.join(".agents/.skill-lock.json"),
        serde_json::to_vec(&serde_json::json!({
            "version": 3,
            "skills": {
                "safe-skill": {
                    "sourceType": "github",
                    "sourceUrl": "https://github.com/acme/safe.git?token=secret",
                    "skillPath": "skills/safe-skill/SKILL.md",
                    "skillFolderHash": "stale"
                },
                "unsafe-skill": {
                    "sourceType": "github",
                    "sourceUrl": "https://github.com/acme/safe.git",
                    "skillPath": "skills/other/SKILL.md",
                    "skillFolderHash": "stale"
                }
            }
        }))
        .unwrap(),
    )
    .unwrap();

    let scan = scan_import_candidates(std::slice::from_ref(&agents_root), &managed_root).unwrap();
    assert!(scan.collections.is_empty());
    assert_eq!(scan.standalone_groups.len(), 2);
    assert_eq!(scan.diagnostics.installed_source_lockfile_matches, 0);
    assert_eq!(scan.diagnostics.installed_source_invalid_entries, 2);

    let repository = root.join("live-repository");
    let live_root = repository.join(".agents/skills");
    make_skill(
        &live_root.join("live-skill"),
        "live-skill",
        "Live Git skill",
    );
    run_git(&repository, &["init", "-b", "main"]);
    run_git(&repository, &["add", "."]);
    run_git(
        &repository,
        &[
            "-c",
            "user.name=SkillBox",
            "-c",
            "user.email=skillbox@example.invalid",
            "commit",
            "-m",
            "Live collection",
        ],
    );
    fs::create_dir_all(repository.join(".agents")).unwrap();
    fs::write(
        repository.join(".agents/.skill-lock.json"),
        serde_json::to_vec(&serde_json::json!({
            "version": 3,
            "skills": {
                "live-skill": {
                    "sourceType": "github",
                    "sourceUrl": "https://github.com/acme/live.git",
                    "skillPath": "skills/live-skill/SKILL.md"
                }
            }
        }))
        .unwrap(),
    )
    .unwrap();

    let scan = scan_import_candidates(std::slice::from_ref(&live_root), &managed_root).unwrap();
    assert_eq!(scan.collections.len(), 1);
    assert_eq!(
        scan.collections[0].source_kind,
        ImportCandidateCollectionSourceKind::GitWorktree
    );
    assert_eq!(scan.diagnostics.installed_source_collections, 0);
}

#[test]
fn scan_import_candidates_ignores_malformed_or_oversized_lockfiles_without_hiding_candidates() {
    let root = temp_dir("candidate-installed-source-lockfile-bounds");
    let agents_root = root.join(".agents/skills");
    let managed_root = root.join("SkillBox");
    make_skill(
        &agents_root.join("standalone"),
        "standalone",
        "Standalone skill",
    );
    fs::create_dir_all(root.join(".agents")).unwrap();

    fs::write(root.join(".agents/.skill-lock.json"), b"{not-json").unwrap();
    let malformed =
        scan_import_candidates(std::slice::from_ref(&agents_root), &managed_root).unwrap();
    assert!(malformed.collections.is_empty());
    assert_eq!(malformed.standalone_groups.len(), 1);
    assert_eq!(malformed.diagnostics.installed_source_lockfiles_scanned, 1);
    assert_eq!(malformed.diagnostics.installed_source_lockfile_errors, 1);

    fs::write(
        root.join(".agents/.skill-lock.json"),
        vec![b' '; 5 * 1024 * 1024 + 1],
    )
    .unwrap();
    let oversized =
        scan_import_candidates(std::slice::from_ref(&agents_root), &managed_root).unwrap();
    assert!(oversized.collections.is_empty());
    assert_eq!(oversized.standalone_groups.len(), 1);
    assert_eq!(oversized.diagnostics.installed_source_lockfiles_scanned, 1);
    assert_eq!(oversized.diagnostics.installed_source_lockfile_errors, 1);
}

#[test]
fn git_collection_apply_persists_selected_children_and_rejects_stale_head_before_writes() {
    let root = temp_dir("git-collection-apply");
    let repository = root.join("skill-collection");
    let managed_root = root.join("SkillBox");
    fs::create_dir_all(&repository).unwrap();
    run_git(&repository, &["init", "-b", "main"]);
    make_skill(&repository.join("skills/alpha"), "alpha", "Alpha skill");
    run_git(&repository, &["add", "."]);
    run_git(
        &repository,
        &[
            "-c",
            "user.name=SkillBox",
            "-c",
            "user.email=skillbox@example.invalid",
            "commit",
            "-m",
            "Initial collection",
        ],
    );

    let scan = scan_import_candidates(std::slice::from_ref(&repository), &managed_root).unwrap();
    let preview = scan.collections.into_iter().next().unwrap();
    let child = preview.children[0].clone();
    let request = ImportCollectionApplyRequest {
        collection_id: preview.id.clone(),
        worktree_root: repository.clone(),
        preview_id: preview.preview_id.clone(),
        selections: vec![ImportCollectionChildSelection {
            relative_path: child.relative_path.clone(),
            group_id: child.group_id.clone(),
            variant_id: child.variant_id.clone(),
            skill_type: SkillKind::User,
        }],
        actor: "test".to_string(),
    };

    let applied = apply_import_collection(request.clone(), &managed_root).unwrap();
    assert_eq!(applied.imported.len(), 1);
    assert!(managed_root.join("user-skills/alpha/SKILL.md").is_file());
    assert!(repository.join("skills/alpha").is_dir());
    let stored = list_skill_collections(&managed_root).unwrap();
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].members.len(), 1);
    assert_eq!(stored[0].members[0].relative_path, "skills/alpha");

    fs::write(
        repository.join("skills/alpha/SKILL.md"),
        "---\nname: alpha\ndescription: Changed\n---\n\n# Changed\n",
    )
    .unwrap();
    run_git(&repository, &["add", "."]);
    run_git(
        &repository,
        &[
            "-c",
            "user.name=SkillBox",
            "-c",
            "user.email=skillbox@example.invalid",
            "commit",
            "-m",
            "Change alpha",
        ],
    );

    let error = apply_import_collection(request, &managed_root).unwrap_err();
    assert!(error.contains("stale"));
    assert_eq!(
        fs::read_to_string(managed_root.join("user-skills/alpha/SKILL.md")).unwrap(),
        "---\nname: alpha\ndescription: \"Alpha skill\"\n---\n\n# alpha\n\n"
    );
    assert_eq!(
        list_skill_collections(&managed_root).unwrap()[0]
            .members
            .len(),
        1
    );
}

#[test]
fn scan_import_candidates_keeps_different_executable_bits_separate() {
    let root = temp_dir("candidate-different-modes");
    let first_root = root.join("first").join(".agents").join("skills");
    let second_root = root.join("second").join(".agents").join("skills");
    let first_source = first_root.join("demo");
    let second_source = second_root.join("demo");
    let managed_root = root.join("SkillBox");
    make_skill(&first_source, "demo", "Demo skill");
    make_skill(&second_source, "demo", "Demo skill");
    fs::write(first_source.join("run.sh"), "echo demo\n").unwrap();
    fs::write(second_source.join("run.sh"), "echo demo\n").unwrap();
    fs::set_permissions(
        first_source.join("run.sh"),
        fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    fs::set_permissions(
        second_source.join("run.sh"),
        fs::Permissions::from_mode(0o644),
    )
    .unwrap();

    let candidates = scan_import_candidates(&[first_root, second_root], &managed_root).unwrap();

    assert_eq!(candidates.candidates.len(), 2);
}

#[test]
fn scan_import_candidates_keeps_nested_git_contents_in_snapshot() {
    let root = temp_dir("candidate-nested-git");
    let first_root = root.join("first").join(".agents").join("skills");
    let second_root = root.join("second").join(".agents").join("skills");
    let first_source = first_root.join("demo");
    let second_source = second_root.join("demo");
    let managed_root = root.join("SkillBox");
    make_skill(&first_source, "demo", "Demo skill");
    make_skill(&second_source, "demo", "Demo skill");
    fs::create_dir_all(first_source.join("nested/.git")).unwrap();
    fs::create_dir_all(second_source.join("nested/.git")).unwrap();
    fs::write(first_source.join("nested/.git/config"), "first\n").unwrap();
    fs::write(second_source.join("nested/.git/config"), "second\n").unwrap();

    let candidates = scan_import_candidates(&[first_root, second_root], &managed_root).unwrap();

    assert_eq!(candidates.candidates.len(), 2);
}

#[test]
fn import_candidates_reuses_identical_user_target_without_deployments() {
    let root = temp_dir("candidate-import-identical-copies");
    let first_source = root.join("first").join("demo");
    let second_source = root.join("second").join("demo");
    let managed_root = root.join("SkillBox");
    make_skill(&first_source, "demo", "Demo skill");
    make_skill(&second_source, "demo", "Demo skill");

    let first_result = import_candidates(
        vec![ImportRequestItem {
            source_path: first_source.clone(),
            skill_type: SkillKind::User,
            deploy_back_to_source: false,
        }],
        &managed_root,
    )
    .unwrap();
    let second_result = import_candidates(
        vec![ImportRequestItem {
            source_path: second_source.clone(),
            skill_type: SkillKind::User,
            deploy_back_to_source: false,
        }],
        &managed_root,
    )
    .unwrap();

    assert!(first_result.errors.is_empty());
    assert!(second_result.errors.is_empty());
    assert_eq!(first_result.imported.len(), 1);
    assert_eq!(second_result.imported.len(), 1);
    assert!(first_source.is_dir());
    assert!(second_source.is_dir());
    let records = list_import_records(
        ImportRecordFilter {
            skill_name: Some("demo".to_string()),
        },
        &managed_root,
    )
    .unwrap();
    assert!(records.records.is_empty());
}

#[test]
fn import_candidates_rejects_multiple_sources_for_one_skill_before_writing() {
    let root = temp_dir("candidate-one-variant-only");
    let first_source = root.join("first/demo");
    let second_source = root.join("second/demo");
    let managed_root = root.join("SkillBox");
    make_skill(&first_source, "demo", "First demo");
    make_skill(&second_source, "demo", "Second demo");
    fs::write(first_source.join("prompt.md"), "first\n").unwrap();
    fs::write(second_source.join("prompt.md"), "second\n").unwrap();

    let error = import_candidates(
        vec![
            ImportRequestItem {
                source_path: first_source.clone(),
                skill_type: SkillKind::User,
                deploy_back_to_source: true,
            },
            ImportRequestItem {
                source_path: second_source.clone(),
                skill_type: SkillKind::Remote,
                deploy_back_to_source: true,
            },
        ],
        &managed_root,
    )
    .unwrap_err();

    assert!(error.contains("only one source variant for skill demo"));
    assert!(first_source.is_dir());
    assert!(second_source.is_dir());
    assert!(!managed_root.join("user-skills/demo").exists());
    assert!(!managed_root.join("remote-skills/demo").exists());
}

#[test]
fn import_candidates_copies_user_skill_backs_up_original_and_symlinks_source() {
    let root = temp_dir("candidate-import-user");
    let source = root.join("runtime").join("demo");
    let managed_root = root.join("SkillBox");
    make_skill(&source, "demo", "Demo skill");

    let result = import_candidates(
        vec![ImportRequestItem {
            source_path: source.clone(),
            skill_type: SkillKind::User,
            deploy_back_to_source: true,
        }],
        &managed_root,
    )
    .unwrap();

    assert_eq!(result.errors.len(), 0);
    assert_eq!(result.imported.len(), 1);
    let imported = &result.imported[0];
    assert_eq!(imported.name, "demo");
    assert!(imported
        .backup_path
        .as_ref()
        .unwrap()
        .join("SKILL.md")
        .exists());
    assert!(fs::symlink_metadata(&source)
        .unwrap()
        .file_type()
        .is_symlink());
    assert_eq!(
        fs::canonicalize(&source).unwrap(),
        fs::canonicalize(managed_root.join("user-skills").join("demo")).unwrap()
    );
}

#[test]
fn import_candidates_records_deploy_back_imports_per_skill() {
    let root = temp_dir("candidate-import-record");
    let source = root.join("runtime").join("demo");
    let managed_root = root.join("SkillBox");
    make_skill(&source, "demo", "Demo skill");

    import_candidates(
        vec![ImportRequestItem {
            source_path: source.clone(),
            skill_type: SkillKind::User,
            deploy_back_to_source: true,
        }],
        &managed_root,
    )
    .unwrap();

    let records = list_import_records(
        ImportRecordFilter {
            skill_name: Some("demo".to_string()),
        },
        &managed_root,
    )
    .unwrap();

    assert_eq!(records.records.len(), 1);
    let record = &records.records[0];
    assert_eq!(record.skill_name, "demo");
    assert_eq!(record.kind, SkillKind::User);
    assert_eq!(record.source_path, source);
    assert_eq!(
        record.managed_path,
        fs::canonicalize(&managed_root)
            .unwrap()
            .join("user-skills")
            .join("demo")
    );
    assert_eq!(record.status, ImportRecordStatus::Active);
    assert!(!record.legacy);
    assert!(record.can_revert);
    assert_eq!(record.affected_deployment_count, 1);
    assert!(record.backup_path.join("SKILL.md").exists());
}

#[test]
fn revert_remote_import_restores_backup_and_keeps_remote_versions() {
    let root = temp_dir("revert-remote-import");
    let runtime_root = root.join("runtime");
    let source = runtime_root.join("remote-demo");
    let managed_root = root.join("SkillBox");
    make_skill(&source, "remote-demo", "Remote demo skill");

    import_candidates(
        vec![ImportRequestItem {
            source_path: source.clone(),
            skill_type: SkillKind::Remote,
            deploy_back_to_source: true,
        }],
        &managed_root,
    )
    .unwrap();
    let record = list_import_records(
        ImportRecordFilter {
            skill_name: Some("remote-demo".to_string()),
        },
        &managed_root,
    )
    .unwrap()
    .records
    .remove(0);
    let remote_root = managed_root.join("remote-skills").join("remote-demo");
    let current = remote_root.join("current");
    let current_target = fs::canonicalize(&current).unwrap();

    let result = revert_import(
        RevertImportRequest {
            import_record_id: record.id.clone(),
            actor: "test".to_string(),
        },
        &managed_root,
    )
    .unwrap();

    assert_eq!(result.record.status, ImportRecordStatus::Reverted);
    assert!(source.join("SKILL.md").exists());
    assert!(!fs::symlink_metadata(&source)
        .unwrap()
        .file_type()
        .is_symlink());
    assert!(!record.backup_path.exists());
    assert!(current.join("SKILL.md").exists());
    assert_eq!(fs::canonicalize(&current).unwrap(), current_target);
    assert!(current_target.exists());

    let state = managed_state(&managed_root).unwrap();
    let skill = state
        .skills
        .iter()
        .find(|skill| skill.name == "remote-demo")
        .unwrap();
    assert!(skill.deployments.is_empty());
}

#[test]
fn remote_import_can_be_reverted_again_after_reimporting_same_version() {
    let root = temp_dir("revert-remote-reimport");
    let source = root.join("runtime").join("remote-demo");
    let managed_root = root.join("SkillBox");
    make_skill(&source, "remote-demo", "Remote demo skill");

    import_candidates(
        vec![ImportRequestItem {
            source_path: source.clone(),
            skill_type: SkillKind::Remote,
            deploy_back_to_source: true,
        }],
        &managed_root,
    )
    .unwrap();
    let first_record = list_import_records(
        ImportRecordFilter {
            skill_name: Some("remote-demo".to_string()),
        },
        &managed_root,
    )
    .unwrap()
    .records
    .remove(0);
    revert_import(
        RevertImportRequest {
            import_record_id: first_record.id.clone(),
            actor: "test".to_string(),
        },
        &managed_root,
    )
    .unwrap();

    import_candidates(
        vec![ImportRequestItem {
            source_path: source.clone(),
            skill_type: SkillKind::Remote,
            deploy_back_to_source: true,
        }],
        &managed_root,
    )
    .unwrap();
    let records = list_import_records(
        ImportRecordFilter {
            skill_name: Some("remote-demo".to_string()),
        },
        &managed_root,
    )
    .unwrap();
    let active_records: Vec<_> = records
        .records
        .iter()
        .filter(|record| record.status == ImportRecordStatus::Active)
        .collect();
    let reverted_records: Vec<_> = records
        .records
        .iter()
        .filter(|record| record.status == ImportRecordStatus::Reverted)
        .collect();

    assert_eq!(records.records.len(), 2);
    assert_eq!(active_records.len(), 1);
    assert_eq!(reverted_records.len(), 1);
    assert_ne!(active_records[0].id, first_record.id);
    assert!(active_records[0].can_revert);
    assert!(fs::symlink_metadata(&source)
        .unwrap()
        .file_type()
        .is_symlink());
}

#[test]
fn revert_user_import_restores_backup_and_removes_unreferenced_managed_copy() {
    let root = temp_dir("revert-user-import");
    let source = root.join("runtime").join("demo");
    let managed_root = root.join("SkillBox");
    make_skill(&source, "demo", "Demo skill");

    import_candidates(
        vec![ImportRequestItem {
            source_path: source.clone(),
            skill_type: SkillKind::User,
            deploy_back_to_source: true,
        }],
        &managed_root,
    )
    .unwrap();
    let record = list_import_records(
        ImportRecordFilter {
            skill_name: Some("demo".to_string()),
        },
        &managed_root,
    )
    .unwrap()
    .records
    .remove(0);

    revert_import(
        RevertImportRequest {
            import_record_id: record.id,
            actor: "test".to_string(),
        },
        &managed_root,
    )
    .unwrap();

    assert!(source.join("SKILL.md").exists());
    assert!(!fs::symlink_metadata(&source)
        .unwrap()
        .file_type()
        .is_symlink());
    assert!(!managed_root.join("user-skills").join("demo").exists());
    assert!(managed_state(&managed_root).unwrap().skills.is_empty());
}

#[test]
fn revert_import_rejects_multiple_workspace_deployments() {
    let root = temp_dir("revert-import-multiple-deployments");
    let source = root.join("runtime").join("remote-demo");
    let second_runtime = root.join("other-runtime");
    let managed_root = root.join("SkillBox");
    make_skill(&source, "remote-demo", "Remote demo skill");

    import_candidates(
        vec![ImportRequestItem {
            source_path: source.clone(),
            skill_type: SkillKind::Remote,
            deploy_back_to_source: true,
        }],
        &managed_root,
    )
    .unwrap();
    deploy_skill("remote-demo", &managed_root, &second_runtime).unwrap();
    let record = list_import_records(
        ImportRecordFilter {
            skill_name: Some("remote-demo".to_string()),
        },
        &managed_root,
    )
    .unwrap()
    .records
    .remove(0);

    assert!(!record.can_revert);
    assert!(record
        .revert_block_reason
        .as_ref()
        .unwrap()
        .contains("multiple workspaces"));

    let error = revert_import(
        RevertImportRequest {
            import_record_id: record.id,
            actor: "test".to_string(),
        },
        &managed_root,
    )
    .unwrap_err();
    assert!(error.contains("multiple workspaces"));
}

#[test]
fn revert_import_rejects_non_symlink_source() {
    let root = temp_dir("revert-import-non-symlink-source");
    let source = root.join("runtime").join("demo");
    let managed_root = root.join("SkillBox");
    make_skill(&source, "demo", "Demo skill");

    import_candidates(
        vec![ImportRequestItem {
            source_path: source.clone(),
            skill_type: SkillKind::Remote,
            deploy_back_to_source: true,
        }],
        &managed_root,
    )
    .unwrap();
    fs::remove_file(&source).unwrap();
    make_skill(&source, "demo", "User edited source");
    let record = list_import_records(
        ImportRecordFilter {
            skill_name: Some("demo".to_string()),
        },
        &managed_root,
    )
    .unwrap()
    .records
    .remove(0);

    assert!(!record.can_revert);
    assert!(record
        .revert_block_reason
        .as_ref()
        .unwrap()
        .contains("non-symlink source"));

    let error = revert_import(
        RevertImportRequest {
            import_record_id: record.id,
            actor: "test".to_string(),
        },
        &managed_root,
    )
    .unwrap_err();
    assert!(error.contains("non-symlink source"));
    assert!(source.join("SKILL.md").exists());
}

#[test]
fn revert_import_rejects_symlink_pointing_elsewhere() {
    let root = temp_dir("revert-import-foreign-symlink");
    let source = root.join("runtime").join("demo");
    let other = root.join("other").join("demo");
    let managed_root = root.join("SkillBox");
    make_skill(&source, "demo", "Demo skill");
    make_skill(&other, "demo", "Other demo skill");

    import_candidates(
        vec![ImportRequestItem {
            source_path: source.clone(),
            skill_type: SkillKind::Remote,
            deploy_back_to_source: true,
        }],
        &managed_root,
    )
    .unwrap();
    fs::remove_file(&source).unwrap();
    symlink_dir(&other, &source).unwrap();
    let record = list_import_records(
        ImportRecordFilter {
            skill_name: Some("demo".to_string()),
        },
        &managed_root,
    )
    .unwrap()
    .records
    .remove(0);

    assert!(!record.can_revert);
    assert!(record
        .revert_block_reason
        .as_ref()
        .unwrap()
        .contains("pointing elsewhere"));

    let error = revert_import(
        RevertImportRequest {
            import_record_id: record.id,
            actor: "test".to_string(),
        },
        &managed_root,
    )
    .unwrap_err();
    assert!(error.contains("pointing elsewhere"));
    assert_eq!(fs::read_link(&source).unwrap(), other);
}

#[test]
fn revert_import_rejects_missing_backup() {
    let root = temp_dir("revert-import-missing-backup");
    let source = root.join("runtime").join("demo");
    let managed_root = root.join("SkillBox");
    make_skill(&source, "demo", "Demo skill");

    import_candidates(
        vec![ImportRequestItem {
            source_path: source.clone(),
            skill_type: SkillKind::Remote,
            deploy_back_to_source: true,
        }],
        &managed_root,
    )
    .unwrap();
    let record = list_import_records(
        ImportRecordFilter {
            skill_name: Some("demo".to_string()),
        },
        &managed_root,
    )
    .unwrap()
    .records
    .remove(0);
    fs::remove_dir_all(&record.backup_path).unwrap();
    let record = list_import_records(
        ImportRecordFilter {
            skill_name: Some("demo".to_string()),
        },
        &managed_root,
    )
    .unwrap()
    .records
    .remove(0);

    assert!(!record.can_revert);
    assert!(record
        .revert_block_reason
        .as_ref()
        .unwrap()
        .contains("SKILL.md not found"));

    let error = revert_import(
        RevertImportRequest {
            import_record_id: record.id,
            actor: "test".to_string(),
        },
        &managed_root,
    )
    .unwrap_err();
    assert!(error.contains("SKILL.md not found"));
}

#[test]
fn revert_import_rejects_backup_name_mismatch() {
    let root = temp_dir("revert-import-backup-name-mismatch");
    let source = root.join("runtime").join("demo");
    let managed_root = root.join("SkillBox");
    make_skill(&source, "demo", "Demo skill");

    import_candidates(
        vec![ImportRequestItem {
            source_path: source.clone(),
            skill_type: SkillKind::Remote,
            deploy_back_to_source: true,
        }],
        &managed_root,
    )
    .unwrap();
    let record = list_import_records(
        ImportRecordFilter {
            skill_name: Some("demo".to_string()),
        },
        &managed_root,
    )
    .unwrap()
    .records
    .remove(0);
    make_skill(&record.backup_path, "other-demo", "Demo skill");
    let record = list_import_records(
        ImportRecordFilter {
            skill_name: Some("demo".to_string()),
        },
        &managed_root,
    )
    .unwrap()
    .records
    .remove(0);

    assert!(!record.can_revert);
    assert!(record
        .revert_block_reason
        .as_ref()
        .unwrap()
        .contains("Backup skill name does not match"));
}

#[test]
fn revert_import_rejects_backup_hash_mismatch() {
    let root = temp_dir("revert-import-backup-hash-mismatch");
    let source = root.join("runtime").join("demo");
    let managed_root = root.join("SkillBox");
    make_skill(&source, "demo", "Demo skill");

    import_candidates(
        vec![ImportRequestItem {
            source_path: source.clone(),
            skill_type: SkillKind::Remote,
            deploy_back_to_source: true,
        }],
        &managed_root,
    )
    .unwrap();
    let record = list_import_records(
        ImportRecordFilter {
            skill_name: Some("demo".to_string()),
        },
        &managed_root,
    )
    .unwrap()
    .records
    .remove(0);
    make_skill_with_body(
        &record.backup_path,
        "demo",
        "Demo skill",
        "\nChanged backup\n",
    );
    let record = list_import_records(
        ImportRecordFilter {
            skill_name: Some("demo".to_string()),
        },
        &managed_root,
    )
    .unwrap()
    .records
    .remove(0);

    assert!(!record.can_revert);
    assert!(record
        .revert_block_reason
        .as_ref()
        .unwrap()
        .contains("Backup content hash does not match"));
}

#[test]
fn revert_import_rejects_duplicate_revert() {
    let root = temp_dir("revert-import-duplicate");
    let source = root.join("runtime").join("demo");
    let managed_root = root.join("SkillBox");
    make_skill(&source, "demo", "Demo skill");

    import_candidates(
        vec![ImportRequestItem {
            source_path: source.clone(),
            skill_type: SkillKind::Remote,
            deploy_back_to_source: true,
        }],
        &managed_root,
    )
    .unwrap();
    let record = list_import_records(
        ImportRecordFilter {
            skill_name: Some("demo".to_string()),
        },
        &managed_root,
    )
    .unwrap()
    .records
    .remove(0);
    revert_import(
        RevertImportRequest {
            import_record_id: record.id.clone(),
            actor: "test".to_string(),
        },
        &managed_root,
    )
    .unwrap();

    let error = revert_import(
        RevertImportRequest {
            import_record_id: record.id,
            actor: "test".to_string(),
        },
        &managed_root,
    )
    .unwrap_err();

    assert!(error.contains("not active"));
}

#[test]
fn legacy_import_records_are_reconciled_when_evidence_is_unique() {
    let root = temp_dir("legacy-import-record");
    let runtime_root = root.join("runtime");
    let source = runtime_root.join("demo");
    let managed_root = root.join("SkillBox");
    make_skill(&source, "demo", "Demo skill");
    let paths = ensure_managed_layout(managed_root.clone()).unwrap();
    let imported = import_skill(&source, SkillKind::User, &managed_root).unwrap();
    let backup_path = replace_source_with_symlink(
        &source,
        &imported.managed_path,
        &paths,
        &imported.name,
        &imported.content_hash,
    )
    .unwrap()
    .unwrap();
    index_deployment(&paths.database_path, "demo", &runtime_root, &source).unwrap();

    let records = list_import_records(
        ImportRecordFilter {
            skill_name: Some("demo".to_string()),
        },
        &managed_root,
    )
    .unwrap();

    assert_eq!(records.records.len(), 1);
    let record = &records.records[0];
    assert!(record.legacy);
    assert_eq!(record.backup_path, backup_path);
    assert!(record.can_revert);
}

#[test]
fn legacy_import_records_are_not_reconciled_when_backup_is_ambiguous() {
    let root = temp_dir("legacy-import-record-ambiguous");
    let runtime_root = root.join("runtime");
    let source = runtime_root.join("demo");
    let managed_root = root.join("SkillBox");
    make_skill(&source, "demo", "Demo skill");
    let paths = ensure_managed_layout(managed_root.clone()).unwrap();
    let imported = import_skill(&source, SkillKind::User, &managed_root).unwrap();
    let backup_path = replace_source_with_symlink(
        &source,
        &imported.managed_path,
        &paths,
        &imported.name,
        &imported.content_hash,
    )
    .unwrap()
    .unwrap();
    let ambiguous_backup = paths
        .root
        .join("backups")
        .join("imports")
        .join(format!("demo-{}-ambiguous", &imported.content_hash[..12]));
    copy_skill_dir(&backup_path, &ambiguous_backup).unwrap();
    index_deployment(&paths.database_path, "demo", &runtime_root, &source).unwrap();

    let records = list_import_records(
        ImportRecordFilter {
            skill_name: Some("demo".to_string()),
        },
        &managed_root,
    )
    .unwrap();

    assert!(records.records.is_empty());
}

#[test]
fn scan_import_candidates_shows_managed_symlinked_sources_as_imported() {
    let root = temp_dir("candidate-imported-symlink");
    let runtime_root = root.join("runtime");
    let source = runtime_root.join("demo");
    let managed_root = root.join("SkillBox");
    make_skill(&source, "demo", "Demo skill");

    import_candidates(
        vec![ImportRequestItem {
            source_path: source.clone(),
            skill_type: SkillKind::User,
            deploy_back_to_source: true,
        }],
        &managed_root,
    )
    .unwrap();

    let candidates = scan_import_candidates(&[runtime_root], &managed_root).unwrap();

    assert_eq!(candidates.candidates.len(), 1);
    let demo = candidate(&candidates.candidates, "demo");
    assert_eq!(demo.import_status, ImportCandidateStatus::Imported);
    assert!(!demo.is_selected);
    assert!(demo.source_path.ends_with("runtime/demo"));
    assert!(is_under_path(&demo.real_path, &managed_root));
}

#[test]
fn scan_import_candidates_dedupes_imported_skill_across_runtime_roots() {
    let root = temp_dir("candidate-imported-dedupe");
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

    let candidates =
        scan_import_candidates(&[first_root.clone(), second_root.clone()], &managed_root).unwrap();

    assert_eq!(candidates.candidates.len(), 1);
    let demo = candidate(&candidates.candidates, "demo");
    assert_eq!(demo.import_status, ImportCandidateStatus::Imported);
    assert_eq!(demo.content_hash, result.imported[0].content_hash);
}
