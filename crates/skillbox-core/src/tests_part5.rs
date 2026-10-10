use super::*;
use crate::test_support::*;
use std::fs;

#[test]
fn apply_rollback_switches_current_and_records_operation() {
    let root = temp_dir("apply-rollback");
    let managed_root = root.join("SkillBox");
    let paths = ensure_managed_layout(&managed_root).unwrap();
    let source_v1 = root.join("local-v1").join("demo");
    make_skill(&source_v1, "demo", "Demo skill");
    import_skill(&source_v1, SkillKind::Remote, &managed_root).unwrap();
    let v1 = current_remote_version(&paths, "demo").unwrap();
    let remote_root = paths.remote_skills_root.join("demo");
    let v2 = "0123456789abcdef0123456789abcdef01234567";
    let v2_path = remote_root.join("versions").join(v2);
    copy_skill_dir(&source_v1, &v2_path).unwrap();
    fs::write(
        v2_path.join("SKILL.md"),
        "---\nname: demo\ndescription: Demo skill\n---\nupdated\n",
    )
    .unwrap();
    update_current_symlink(&remote_root, &v2_path).unwrap();

    let result = apply_remote_version_change(
        RemoteVersionChangeApplyRequest {
            skill_name: "demo".to_string(),
            action: RemoteVersionChangeAction::Rollback,
            target_version: v1.clone(),
            preview_id: None,
            actor: "cli".to_string(),
        },
        &managed_root,
    )
    .unwrap();

    assert_eq!(result.from_version, v2);
    assert_eq!(result.to_version, v1);
    assert_eq!(
        current_remote_version(&paths, "demo").unwrap(),
        result.to_version
    );
    let operations = list_operations(OperationFilter::default(), &managed_root).unwrap();
    assert!(operations
        .operations
        .iter()
        .any(
            |operation| operation.operation_type == "rollback_remote_skill"
                && operation.status == OperationStatus::Succeeded
        ));
}

#[test]
fn apply_remote_version_change_rejects_stale_preview_id() {
    let root = temp_dir("apply-stale-preview");
    let managed_root = root.join("SkillBox");
    let paths = ensure_managed_layout(&managed_root).unwrap();
    let source_v1 = root.join("local-v1").join("demo");
    make_skill(&source_v1, "demo", "Demo skill");
    import_skill(&source_v1, SkillKind::Remote, &managed_root).unwrap();
    let v1 = current_remote_version(&paths, "demo").unwrap();
    let remote_root = paths.remote_skills_root.join("demo");
    let v2 = "0123456789abcdef0123456789abcdef01234567";
    let v2_path = remote_root.join("versions").join(v2);
    copy_skill_dir(&source_v1, &v2_path).unwrap();
    fs::write(
        v2_path.join("SKILL.md"),
        "---\nname: demo\ndescription: Demo skill\n---\nupdated\n",
    )
    .unwrap();
    update_current_symlink(&remote_root, &v2_path).unwrap();

    let preview = preview_remote_version_change(
        RemoteVersionChangeRequest {
            skill_name: "demo".to_string(),
            action: RemoteVersionChangeAction::Rollback,
            target_version: Some(v1.clone()),
            actor: "cli".to_string(),
        },
        &managed_root,
    )
    .unwrap();

    let error = apply_remote_version_change(
        RemoteVersionChangeApplyRequest {
            skill_name: "demo".to_string(),
            action: RemoteVersionChangeAction::Rollback,
            target_version: v1,
            preview_id: Some(format!("{}-stale", preview.preview_id)),
            actor: "cli".to_string(),
        },
        &managed_root,
    )
    .unwrap_err();

    assert!(error.contains("Remote version preview is stale"));
    assert_eq!(current_remote_version(&paths, "demo").unwrap(), v2);
}

#[test]
fn apply_update_writes_latest_version_and_preserves_old_version() {
    let root = temp_dir("apply-update");
    let managed_root = root.join("SkillBox");
    let paths = ensure_managed_layout(&managed_root).unwrap();
    let source = root.join("local").join("find-skills");
    make_skill(&source, "find-skills", "Find skills");
    import_skill(&source, SkillKind::Remote, &managed_root).unwrap();
    let old_version = current_remote_version(&paths, "find-skills").unwrap();
    let remote = bare_remote_with_skill_content(
        "apply-update-origin",
        "find-skills",
        "Find skills",
        "Updated remote body\n",
    );
    let _rewrite = github_repo_rewrite("acme", "apply-update", &remote);
    let source_url = github_source_url("acme", "apply-update", "find-skills");
    bind_remote_source(
        BindRemoteSourceRequest {
            skill_name: "find-skills".to_string(),
            source_url,
            actor: "cli".to_string(),
        },
        &managed_root,
    )
    .unwrap();
    let latest_sha = read_remote_source(&paths.remote_skills_root.join("find-skills"))
        .unwrap()
        .latest_sha
        .unwrap();

    let result = apply_remote_version_change(
        RemoteVersionChangeApplyRequest {
            skill_name: "find-skills".to_string(),
            action: RemoteVersionChangeAction::Update,
            target_version: latest_sha.clone(),
            preview_id: None,
            actor: "cli".to_string(),
        },
        &managed_root,
    )
    .unwrap();

    assert_eq!(result.to_version, latest_sha);
    assert!(paths
        .remote_skills_root
        .join("find-skills")
        .join("versions")
        .join(&old_version)
        .exists());
    assert!(paths
        .remote_skills_root
        .join("find-skills")
        .join("versions")
        .join(&result.to_version)
        .exists());
    assert_eq!(
        current_remote_version(&paths, "find-skills").unwrap(),
        result.to_version
    );
    let source = read_remote_source(&paths.remote_skills_root.join("find-skills")).unwrap();
    assert_eq!(
        source.current_version.as_deref(),
        Some(result.to_version.as_str())
    );
    assert_eq!(
        source.installed_sha.as_deref(),
        Some(result.to_version.as_str())
    );
    let operations = list_operations(OperationFilter::default(), &managed_root).unwrap();
    assert!(operations
        .operations
        .iter()
        .any(
            |operation| operation.operation_type == "update_remote_skill"
                && operation.status == OperationStatus::Succeeded
        ));
}

#[test]
fn apply_update_snapshots_same_repo_symlinked_directories() {
    let root = temp_dir("apply-update-repo-symlink");
    let managed_root = root.join("SkillBox");
    let paths = ensure_managed_layout(&managed_root).unwrap();
    let source = root.join("local").join("find-skills");
    make_skill(&source, "find-skills", "Find skills");
    import_skill(&source, SkillKind::Remote, &managed_root).unwrap();

    let remote = bare_remote("apply-update-repo-symlink-origin");
    let work = temp_dir("apply-update-repo-symlink-work");
    run_git(&work, &["init", "-b", "main"]);
    let skill_dir = work.join("skills").join("find-skills");
    make_skill(&skill_dir, "find-skills", "Find skills");
    fs::write(
        skill_dir.join("SKILL.md"),
        "---\nname: find-skills\ndescription: Find skills\n---\nupdated\n",
    )
    .unwrap();
    fs::create_dir_all(work.join("shared-scripts")).unwrap();
    fs::write(
        work.join("shared-scripts").join("design_system.py"),
        "print('shared')\n",
    )
    .unwrap();
    symlink_dir(
        Path::new("../../shared-scripts"),
        &skill_dir.join("scripts"),
    )
    .unwrap();
    run_git(&work, &["add", "."]);
    run_git(
        &work,
        &[
            "-c",
            "user.name=SkillBox",
            "-c",
            "user.email=skillbox@example.invalid",
            "commit",
            "-m",
            "Add skill with shared scripts",
        ],
    );
    run_git(
        &work,
        &["remote", "add", "origin", remote.to_str().unwrap()],
    );
    run_git(&work, &["push", "origin", "main"]);
    let _rewrite = github_repo_rewrite("acme", "apply-update-repo-symlink", &remote);
    let source_url = github_source_url("acme", "apply-update-repo-symlink", "find-skills");
    bind_remote_source(
        BindRemoteSourceRequest {
            skill_name: "find-skills".to_string(),
            source_url,
            actor: "cli".to_string(),
        },
        &managed_root,
    )
    .unwrap();
    let latest_sha = read_remote_source(&paths.remote_skills_root.join("find-skills"))
        .unwrap()
        .latest_sha
        .unwrap();

    let result = apply_remote_version_change(
        RemoteVersionChangeApplyRequest {
            skill_name: "find-skills".to_string(),
            action: RemoteVersionChangeAction::Update,
            target_version: latest_sha,
            preview_id: None,
            actor: "cli".to_string(),
        },
        &managed_root,
    )
    .unwrap();

    let version_path = paths
        .remote_skills_root
        .join("find-skills")
        .join("versions")
        .join(result.to_version);
    let scripts_path = version_path.join("scripts");
    assert!(!fs::symlink_metadata(&scripts_path)
        .unwrap()
        .file_type()
        .is_symlink());
    assert_eq!(
        fs::read_to_string(scripts_path.join("design_system.py")).unwrap(),
        "print('shared')\n"
    );
}

#[test]
fn source_candidates_rank_by_name_path_trust_and_popularity() {
    let candidates = rank_remote_source_candidates(
        "find-skills",
        vec![
            RemoteSourceCandidate {
                owner: "small".to_string(),
                repo: "misc".to_string(),
                path: "tools/other".to_string(),
                reference: "main".to_string(),
                source_url: "https://github.com/small/misc/tree/main/tools/other".to_string(),
                repo_url: "https://github.com/small/misc.git".to_string(),
                name: Some("other".to_string()),
                description: Some("Other".to_string()),
                stars: 1000,
                archived: false,
                fork: false,
                updated_at: "2026-01-01T00:00:00Z".to_string(),
                match_reasons: vec![],
                score: 0,
            },
            RemoteSourceCandidate {
                owner: "acme".to_string(),
                repo: "skills".to_string(),
                path: "skills/find-skills".to_string(),
                reference: "main".to_string(),
                source_url: "https://github.com/acme/skills/tree/main/skills/find-skills"
                    .to_string(),
                repo_url: "https://github.com/acme/skills.git".to_string(),
                name: Some("find-skills".to_string()),
                description: Some("Find skills".to_string()),
                stars: 10,
                archived: false,
                fork: false,
                updated_at: "2025-01-01T00:00:00Z".to_string(),
                match_reasons: vec![],
                score: 0,
            },
        ],
    );

    assert_eq!(candidates[0].path, "skills/find-skills");
    assert!(candidates[0]
        .match_reasons
        .contains(&"Exact skill name match".to_string()));
}

#[test]
fn parses_claude_marketplace_skill_candidates_with_exact_name_priority() {
    let response = r#"[
          {
            "id": "vercel-labs/skills/find-skills",
            "name": "find-skills",
            "description": "Discover and install specialized agent skills.",
            "repo": "vercel-labs/skills",
            "path": "find-skills",
            "stars": 18600,
            "installs": 1500000,
            "installCommand": "npx skills add https://github.com/vercel-labs/skills --skill find-skills",
            "lastUpdated": "2026-05-16T17:00:48.907+00:00",
            "listingStatus": "listed"
          },
          {
            "id": "example/misc/find-skills-helper",
            "name": "find-skills-helper",
            "description": "Helper",
            "repo": "example/misc",
            "path": ".claude/skills/find-skills-helper/SKILL.md",
            "stars": 1,
            "installs": 1,
            "listingStatus": "listed"
          }
        ]"#;

    let candidates = parse_claude_marketplace_skill_candidates("find-skills", response).unwrap();

    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].owner, "vercel-labs");
    assert_eq!(candidates[0].repo, "skills");
    assert_eq!(candidates[0].path, "find-skills");
    assert_eq!(
        candidates[0].source_url,
        "https://github.com/vercel-labs/skills/tree/main/find-skills"
    );
    assert!(candidates[0]
        .match_reasons
        .contains(&"Claude Marketplace listed skill".to_string()));
}

#[test]
fn claude_marketplace_api_curl_args_target_skills_api() {
    let args = claude_marketplace_api_curl_args();

    assert_eq!(
        args.last().map(String::as_str),
        Some(CLAUDE_MARKETPLACE_SKILLS_API)
    );
    assert!(args.iter().any(|arg| arg == "Accept: application/json"));
}

#[test]
fn scan_import_candidates_infers_type_from_path_and_metadata() {
    let root = temp_dir("candidate-type");
    let agents_root = root.join(".agents").join("skills");
    let codex_root = root.join(".codex").join("skills");
    let system_root = codex_root.join(".system");
    let misc_root = root.join("Downloads").join("skills");
    let managed_root = root.join("SkillBox");

    make_skill(&agents_root.join("local"), "local", "Local skill");
    make_skill(&codex_root.join("remote"), "remote", "Remote skill");
    make_skill(&system_root.join("system"), "system", "System skill");
    make_skill_with_body(
        &misc_root.join("github-skill"),
        "github-skill",
        "GitHub skill",
        "source: https://github.com/acme/skills/tree/main/github-skill",
    );
    make_skill(&misc_root.join("unknown"), "unknown", "Unknown skill");

    let candidates =
        scan_import_candidates(&[agents_root, codex_root, misc_root], &managed_root).unwrap();

    let local = candidate(&candidates.candidates, "local");
    assert_eq!(local.suggested_type, SkillKind::User);
    assert_eq!(local.suggestion_reason, "inside ~/.agents/skills");
    assert!(local.is_selected);

    let remote = candidate(&candidates.candidates, "remote");
    assert_eq!(remote.suggested_type, SkillKind::Remote);
    assert_eq!(remote.suggestion_reason, "inside ~/.codex/skills");
    assert!(remote.is_selected);

    let system = candidate(&candidates.candidates, "system");
    assert_eq!(system.suggested_type, SkillKind::Remote);
    assert_eq!(system.suggestion_reason, "inside ~/.codex/skills/.system");
    assert_eq!(system.import_status, ImportCandidateStatus::System);
    assert!(!system.is_selected);

    let github = candidate(&candidates.candidates, "github-skill");
    assert_eq!(github.suggested_type, SkillKind::Remote);
    assert_eq!(github.suggestion_reason, "GitHub source metadata found");
    assert!(github.is_selected);

    let unknown = candidate(&candidates.candidates, "unknown");
    assert_eq!(unknown.suggested_type, SkillKind::User);
    assert_eq!(unknown.suggestion_reason, "Needs confirm");
    assert!(unknown.is_selected);
}

#[test]
fn scan_import_candidates_does_not_mark_copied_only_hash_matches_as_imported() {
    let root = temp_dir("candidate-copied-only-hash-match");
    let source = root.join("runtime").join("demo");
    let managed_root = root.join("SkillBox");
    make_skill(&source, "demo", "Demo skill");
    import_skill(&source, SkillKind::User, &managed_root).unwrap();

    let candidates = scan_import_candidates(&[root.join("runtime")], &managed_root).unwrap();

    assert_eq!(candidates.candidates.len(), 1);
    let demo = candidate(&candidates.candidates, "demo");
    assert_eq!(demo.import_status, ImportCandidateStatus::Importable);
    assert!(demo.is_selected);
}

#[test]
fn scan_import_candidates_groups_identical_copied_skills_across_roots() {
    let root = temp_dir("candidate-identical-copies");
    let global_root = root.join("global").join(".agents").join("skills");
    let project_root = root.join("project").join(".agents").join("skills");
    let global_source = global_root.join("demo");
    let project_source = project_root.join("demo");
    let managed_root = root.join("SkillBox");
    make_skill(&global_source, "demo", "Demo skill");
    make_skill(&project_source, "demo", "Demo skill");
    fs::create_dir_all(global_source.join("scripts")).unwrap();
    fs::create_dir_all(project_source.join("scripts")).unwrap();
    fs::write(global_source.join("scripts/run.sh"), "echo demo\n").unwrap();
    fs::write(project_source.join("scripts/run.sh"), "echo demo\n").unwrap();
    fs::create_dir_all(global_source.join(".git")).unwrap();
    fs::create_dir_all(project_source.join(".git")).unwrap();
    fs::write(global_source.join(".git/config"), "global\n").unwrap();
    fs::write(project_source.join(".git/config"), "project\n").unwrap();

    let candidates =
        scan_import_candidates(&[global_root.clone(), project_root.clone()], &managed_root)
            .unwrap();

    assert_eq!(candidates.candidates.len(), 1);
    let demo = candidate(&candidates.candidates, "demo");
    assert_eq!(demo.source_path, global_source);
    assert_eq!(demo.additional_source_paths, vec![project_source.clone()]);
    assert_eq!(demo.import_status, ImportCandidateStatus::Importable);
    assert!(demo.is_selected);
    assert_eq!(candidates.groups.len(), 1);
    let group = &candidates.groups[0];
    assert_eq!(group.name, "demo");
    assert_eq!(group.variants.len(), 1);
    assert_eq!(group.variants[0].locations.len(), 2);
    assert_eq!(
        group.selected_variant_id.as_deref(),
        Some(group.variants[0].id.as_str())
    );
    assert!(!group.requires_review);

    let reversed = scan_import_candidates(&[project_root, global_root], &managed_root).unwrap();
    let reversed_demo = candidate(&reversed.candidates, "demo");
    assert_eq!(reversed_demo.source_path, project_source);
    assert_eq!(reversed_demo.additional_source_paths, vec![global_source]);
}

#[test]
fn scan_import_candidates_keeps_same_skill_md_with_different_assets_separate() {
    let root = temp_dir("candidate-different-assets");
    let first_root = root.join("first").join(".agents").join("skills");
    let second_root = root.join("second").join(".agents").join("skills");
    let first_source = first_root.join("demo");
    let second_source = second_root.join("demo");
    let managed_root = root.join("SkillBox");
    make_skill(&first_source, "demo", "Demo skill");
    make_skill(&second_source, "demo", "Demo skill");
    fs::write(first_source.join("prompt.md"), "first\n").unwrap();
    fs::write(second_source.join("prompt.md"), "second\n").unwrap();

    let candidates = scan_import_candidates(&[first_root, second_root], &managed_root).unwrap();

    assert_eq!(candidates.candidates.len(), 2);
    assert!(candidates
        .candidates
        .iter()
        .all(|candidate| candidate.additional_source_paths.is_empty()));
    assert_eq!(candidates.groups.len(), 1);
    assert_eq!(candidates.groups[0].variants.len(), 2);
    assert!(candidates.groups[0].requires_review);
    assert!(candidates.groups[0].selected_variant_id.is_none());
    assert!(candidates.groups[0]
        .variants
        .iter()
        .all(|variant| !variant.candidate.is_selected));
}

#[test]
fn scan_import_candidates_groups_mixed_type_suggestions_without_splitting_content() {
    let root = temp_dir("candidate-mixed-type-suggestion-group");
    let agents_root = root.join("home/.agents/skills");
    let claude_root = root.join("home/.claude/skills");
    let codex_root = root.join("home/.codex/skills");
    let cursor_root = root.join("home/.cursor/skills");
    let agents_source = agents_root.join("general-video");
    let claude_source = claude_root.join("general-video");
    let managed_root = root.join("SkillBox");
    make_skill(&agents_source, "general-video", "Create product videos");
    make_skill(&claude_source, "general-video", "Create product videos");
    fs::create_dir_all(&codex_root).unwrap();
    fs::create_dir_all(&cursor_root).unwrap();
    symlink_dir(&claude_source, &codex_root.join("general-video")).unwrap();
    symlink_dir(&claude_source, &cursor_root.join("general-video")).unwrap();

    let scan = scan_import_candidates(
        &[agents_root, claude_root, cursor_root, codex_root],
        &managed_root,
    )
    .unwrap();

    assert_eq!(scan.groups.len(), 1);
    let group = &scan.groups[0];
    assert_eq!(group.name, "general-video");
    assert_eq!(group.variants.len(), 1);
    assert_eq!(group.variants[0].locations.len(), 4);
    assert_eq!(group.variants[0].candidate.source_path, agents_source);
    assert!(!group.requires_review);
    assert_eq!(
        group.selected_variant_id.as_deref(),
        Some(group.variants[0].id.as_str())
    );
    assert!(group.variants[0].requires_type_review);
    assert_eq!(
        group.variants[0].suggested_types,
        vec![SkillKind::User, SkillKind::Remote]
    );
    assert_eq!(group.variants[0].selected_type, None);
    assert!(!group.variants[0].candidate.is_selected);
    assert_eq!(
        group.variants[0]
            .locations
            .iter()
            .filter(|location| location.is_symlink)
            .count(),
        2
    );
    assert_eq!(
        group.variants[0]
            .locations
            .iter()
            .filter(|location| location.suggested_type == SkillKind::Remote)
            .count(),
        1
    );
    assert_eq!(
        group.variants[0]
            .locations
            .iter()
            .map(|location| location.source_path.clone())
            .collect::<Vec<_>>(),
        vec![
            root.join("home/.agents/skills/general-video"),
            root.join("home/.claude/skills/general-video"),
            root.join("home/.cursor/skills/general-video"),
            root.join("home/.codex/skills/general-video"),
        ]
    );
}

#[test]
fn scan_import_candidates_groups_git_repository_children_and_keeps_external_copies_unlinked() {
    let root = temp_dir("candidate-git-collection");
    let repository = root.join("skill-collection");
    let project = root.join("project");
    let outside_root = root.join("outside/.agents/skills");
    let managed_root = root.join("SkillBox");
    fs::create_dir_all(&repository).unwrap();
    run_git(&repository, &["init", "-b", "main"]);
    make_skill(&repository.join("skills/alpha"), "alpha", "Alpha skill");
    make_skill(&repository.join("skills/beta"), "beta", "Beta skill");
    let nested_repository = repository.join("nested-repository");
    fs::create_dir_all(&nested_repository).unwrap();
    run_git(&nested_repository, &["init", "-b", "main"]);
    make_skill(
        &nested_repository.join("skills/nested"),
        "nested",
        "Nested skill",
    );
    run_git(&nested_repository, &["add", "."]);
    run_git(
        &nested_repository,
        &[
            "-c",
            "user.name=SkillBox",
            "-c",
            "user.email=skillbox@example.invalid",
            "commit",
            "-m",
            "Add nested skill",
        ],
    );
    fs::write(repository.join("README.md"), "Skill collection\n").unwrap();
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
            "Add collection skills",
        ],
    );

    make_skill(&outside_root.join("alpha"), "alpha", "Alpha skill");
    let project_runtime_root = project.join(".agents/skills");
    fs::create_dir_all(&project_runtime_root).unwrap();
    symlink_dir(
        &repository.join("skills/alpha"),
        &project_runtime_root.join("alpha"),
    )
    .unwrap();

    let scan = scan_import_candidates(
        &[
            repository.clone(),
            project_runtime_root,
            outside_root.clone(),
        ],
        &managed_root,
    )
    .unwrap();
    assert_eq!(scan.collections.len(), 2);
    let collection = scan
        .collections
        .iter()
        .find(|collection| {
            collection.canonical_worktree_root == fs::canonicalize(&repository).unwrap()
        })
        .unwrap();
    assert_eq!(
        collection.canonical_worktree_root,
        fs::canonicalize(&repository).unwrap()
    );
    assert_eq!(collection.children.len(), 2);

    let alpha = collection
        .children
        .iter()
        .find(|child| child.name == "alpha")
        .unwrap();
    assert_eq!(alpha.relative_path, "skills/alpha");
    assert_eq!(alpha.locations.len(), 2);
    assert_eq!(alpha.unlinked_locations.len(), 1);
    assert_eq!(
        alpha.unlinked_locations[0].source_path,
        outside_root.join("alpha")
    );
    assert!(collection
        .children
        .iter()
        .all(|child| child.relative_path.starts_with("skills/")));
    let nested = scan
        .collections
        .iter()
        .find(|collection| collection.display_name == "nested-repository")
        .unwrap();
    assert_eq!(nested.children.len(), 1);
    assert_eq!(nested.children[0].relative_path, "skills/nested");
}

#[test]
fn scan_import_candidates_reuses_repository_and_snapshot_work_for_duplicate_locations() {
    let root = temp_dir("candidate-git-collection-cache");
    let repository = root.join("skill-collection");
    let runtime_root = root.join("project/.agents/skills");
    let managed_root = root.join("SkillBox");
    let skill_count = 48;
    fs::create_dir_all(&repository).unwrap();
    run_git(&repository, &["init", "-b", "main"]);
    for index in 0..skill_count {
        let name = format!("skill-{index:02}");
        make_skill(
            &repository.join("skills").join(&name),
            &name,
            "Cached collection skill",
        );
    }
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
            "Add cached collection skills",
        ],
    );
    fs::create_dir_all(&runtime_root).unwrap();
    for index in 0..skill_count {
        let name = format!("skill-{index:02}");
        symlink_dir(
            &repository.join("skills").join(&name),
            &runtime_root.join(&name),
        )
        .unwrap();
    }

    let scan = scan_import_candidates(&[repository, runtime_root], &managed_root).unwrap();
    assert_eq!(scan.collections.len(), 1);
    assert_eq!(scan.diagnostics.unique_repository_count, 1);
    assert_eq!(scan.diagnostics.repository_inspections, 1);
    assert!(scan.diagnostics.repository_cache_hits >= skill_count);
    assert!(scan.diagnostics.snapshot_hash_computations <= skill_count);
    assert!(scan.diagnostics.snapshot_cache_hits >= skill_count);
}

#[test]
fn scan_import_candidates_groups_validated_installed_source_lockfile_entries() {
    let root = temp_dir("candidate-installed-source-lockfile");
    let agents_root = root.join(".agents/skills");
    let claude_root = root.join(".claude/skills");
    let managed_root = root.join("SkillBox");
    let source_url = "https://github.com/dontbesilent2025/dbskill.git";
    let names = (0..24)
        .map(|index| format!("dbs-skill-{index:02}"))
        .collect::<Vec<_>>();

    for name in &names {
        make_skill(&agents_root.join(name), name, "Installed source skill");
        fs::create_dir_all(&claude_root).unwrap();
        symlink_dir(&agents_root.join(name), &claude_root.join(name)).unwrap();
    }
    let skills = names
        .iter()
        .map(|name| {
            (
                name.clone(),
                serde_json::json!({
                    "source": "dontbesilent2025/dbskill",
                    "sourceType": "github",
                    "sourceUrl": source_url,
                    "skillPath": format!("skills/{name}/SKILL.md"),
                    "skillFolderHash": "stale-lock-hash"
                }),
            )
        })
        .collect::<serde_json::Map<_, _>>();
    fs::create_dir_all(root.join(".agents")).unwrap();
    fs::write(
        root.join(".agents/.skill-lock.json"),
        serde_json::to_vec(&serde_json::json!({ "version": 3, "skills": skills })).unwrap(),
    )
    .unwrap();

    let scan = scan_import_candidates(&[agents_root, claude_root], &managed_root).unwrap();
    assert_eq!(scan.collections.len(), 1);
    let collection = &scan.collections[0];
    assert_eq!(
        collection.source_kind,
        ImportCandidateCollectionSourceKind::InstalledSource
    );
    assert_eq!(collection.display_name, "dontbesilent2025/dbskill");
    assert_eq!(
        collection.origin_url.as_deref(),
        Some("https://github.com/dontbesilent2025/dbskill")
    );
    assert!(collection.canonical_worktree_root.as_os_str().is_empty());
    assert!(collection.reviewed_head_sha.is_none());
    assert_eq!(collection.children.len(), names.len());
    assert_eq!(scan.standalone_groups.len(), 0);
    assert_eq!(scan.diagnostics.installed_source_lockfiles_scanned, 1);
    assert_eq!(
        scan.diagnostics.installed_source_lockfile_entries,
        names.len()
    );
    assert_eq!(
        scan.diagnostics.installed_source_lockfile_matches,
        names.len()
    );
    assert_eq!(scan.diagnostics.installed_source_collections, 1);
    assert!(collection
        .children
        .iter()
        .all(|child| !child.snapshot_hash.is_empty()));
    assert!(collection
        .children
        .iter()
        .all(|child| child.locations.len() == 2));
}

#[test]
fn scan_import_candidates_groups_installer_plugin_children_without_using_plugin_name_as_identity() {
    let root = temp_dir("candidate-installed-source-hyperframes-plugin");
    let agents_root = root.join(".agents/skills");
    let claude_root = root.join(".claude/skills");
    let codex_root = root.join(".codex/skills");
    let cursor_root = root.join(".cursor/skills");
    let managed_root = root.join("SkillBox");
    let source_url = "https://github.com/heygen-com/hyperframes.git";
    let names = [
        "hyperframes",
        "hyperframes-animation",
        "hyperframes-cli",
        "hyperframes-core",
    ];

    for name in names {
        make_skill(
            &agents_root.join(name),
            name,
            "HyperFrames installer-provenance skill",
        );
        for runtime_root in [&claude_root, &codex_root, &cursor_root] {
            fs::create_dir_all(runtime_root).unwrap();
            symlink_dir(&agents_root.join(name), &runtime_root.join(name)).unwrap();
        }
    }
    let skills = names
        .iter()
        .map(|name| {
            (
                (*name).to_string(),
                serde_json::json!({
                    "sourceType": "github",
                    "sourceUrl": source_url,
                    "skillPath": format!("skills/{name}/SKILL.md"),
                    "pluginName": "core-skills",
                    "skillFolderHash": "stale-lock-hash"
                }),
            )
        })
        .collect::<serde_json::Map<_, _>>();
    fs::create_dir_all(root.join(".agents")).unwrap();
    fs::write(
        root.join(".agents/.skill-lock.json"),
        serde_json::to_vec(&serde_json::json!({ "version": 3, "skills": skills })).unwrap(),
    )
    .unwrap();

    let scan = scan_import_candidates(
        &[agents_root, claude_root, codex_root, cursor_root],
        &managed_root,
    )
    .unwrap();

    let collections = scan
        .collections
        .iter()
        .filter(|collection| {
            collection.source_kind == ImportCandidateCollectionSourceKind::InstalledSource
                && collection.origin_url.as_deref()
                    == Some("https://github.com/heygen-com/hyperframes")
        })
        .collect::<Vec<_>>();
    assert_eq!(collections.len(), 1);
    let collection = collections[0];
    assert_eq!(collection.children.len(), names.len());
    assert!(collection
        .children
        .iter()
        .all(|child| { names.contains(&child.name.as_str()) && child.locations.len() == 4 }));
    assert!(scan
        .standalone_groups
        .iter()
        .all(|group| !names.contains(&group.name.as_str())));
    assert_eq!(
        scan.diagnostics.installed_source_lockfile_entries,
        names.len()
    );
    assert_eq!(
        scan.diagnostics.installed_source_lockfile_matches,
        names.len()
    );
    assert_eq!(scan.diagnostics.installed_source_collections, 1);
}

#[test]
fn scan_import_candidates_groups_verified_well_known_children_by_source_base() {
    let root = temp_dir("candidate-installed-source-well-known");
    let agents_root = root.join(".agents/skills");
    let claude_root = root.join(".claude/skills");
    let managed_root = root.join("SkillBox");
    let source_base_url = "https://open.feishu.cn";
    let names = [
        "lark-approval",
        "lark-apps",
        "lark-attendance",
        "lark-base",
        "lark-doc",
        "lark-im",
    ];

    for name in names {
        make_skill(
            &agents_root.join(name),
            name,
            "Verified well-known source skill",
        );
        fs::create_dir_all(&claude_root).unwrap();
        symlink_dir(&agents_root.join(name), &claude_root.join(name)).unwrap();
    }
    let skills = names
        .iter()
        .map(|name| {
            (
                (*name).to_string(),
                serde_json::json!({
                    "sourceType": "well-known",
                    "sourceBaseUrl": source_base_url,
                    "sourceUrl": format!("{source_base_url}/.well-known/skills/{name}/SKILL.md"),
                    "skillFolderHash": "",
                    "wellKnownDigest": format!("sha256:{}", "a".repeat(64))
                }),
            )
        })
        .collect::<serde_json::Map<_, _>>();
    fs::create_dir_all(root.join(".agents")).unwrap();
    fs::write(
        root.join(".agents/.skill-lock.json"),
        serde_json::to_vec(&serde_json::json!({ "version": 3, "skills": skills })).unwrap(),
    )
    .unwrap();

    let scan = scan_import_candidates(&[agents_root, claude_root], &managed_root).unwrap();

    assert_eq!(scan.collections.len(), 1);
    let collection = &scan.collections[0];
    assert_eq!(
        collection.source_kind,
        ImportCandidateCollectionSourceKind::InstalledSource
    );
    assert_eq!(collection.display_name, "open.feishu.cn");
    assert_eq!(collection.origin_url.as_deref(), Some(source_base_url));
    assert_eq!(collection.children.len(), names.len());
    assert_eq!(scan.standalone_groups.len(), 0);
    assert_eq!(
        scan.diagnostics.installed_source_lockfile_entries,
        names.len()
    );
    assert_eq!(
        scan.diagnostics.installed_source_lockfile_matches,
        names.len()
    );
    assert_eq!(scan.diagnostics.installed_source_invalid_entries, 0);
    assert_eq!(scan.diagnostics.installed_source_collections, 1);
    let child_names = collection
        .children
        .iter()
        .map(|child| child.name.as_str())
        .collect::<HashSet<_>>();
    assert_eq!(child_names.len(), names.len());
    assert!(collection
        .children
        .iter()
        .all(|child| child.locations.len() == 2 && !child.snapshot_hash.is_empty()));
}

#[test]
fn scan_import_candidates_keeps_well_known_sources_separate_and_singletons_standalone() {
    let root = temp_dir("candidate-installed-source-well-known-separation");
    let agents_root = root.join(".agents/skills");
    let managed_root = root.join("SkillBox");
    let sources = [
        ("lark-alpha", "https://open.feishu.cn"),
        ("lark-beta", "https://open.feishu.cn"),
        ("lark-gamma", "https://skills.example.com/team"),
        ("lark-delta", "https://skills.example.com/team"),
        ("lark-single", "https://single.example.com"),
    ];

    for (name, _) in sources {
        make_skill(&agents_root.join(name), name, "Well-known source skill");
    }
    let skills = sources
        .iter()
        .map(|(name, source_base_url)| {
            (
                (*name).to_string(),
                serde_json::json!({
                    "sourceType": "well-known",
                    "sourceBaseUrl": source_base_url,
                    "sourceUrl": format!("{source_base_url}/.well-known/skills/{name}/SKILL.md"),
                    "wellKnownDigest": format!("sha256:{}", "b".repeat(64))
                }),
            )
        })
        .collect::<serde_json::Map<_, _>>();
    fs::create_dir_all(root.join(".agents")).unwrap();
    fs::write(
        root.join(".agents/.skill-lock.json"),
        serde_json::to_vec(&serde_json::json!({ "version": 3, "skills": skills })).unwrap(),
    )
    .unwrap();

    let scan = scan_import_candidates(std::slice::from_ref(&agents_root), &managed_root).unwrap();

    assert_eq!(scan.collections.len(), 2);
    assert_eq!(scan.collections[0].children.len(), 2);
    assert_eq!(scan.collections[1].children.len(), 2);
    assert_ne!(
        scan.collections[0].origin_url,
        scan.collections[1].origin_url
    );
    assert_eq!(scan.standalone_groups.len(), 1);
    assert_eq!(scan.standalone_groups[0].name, "lark-single");
    assert_eq!(
        scan.diagnostics.installed_source_lockfile_matches,
        sources.len()
    );
    assert_eq!(scan.diagnostics.installed_source_collections, 2);
}
