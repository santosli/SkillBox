use super::*;
use crate::test_support::*;
use std::fs;

#[test]
fn github_collection_update_apply_advances_sha_and_rollback_restores_previous() {
    let root = temp_dir("github-collection-update-apply");
    let managed_root = root.join("SkillBox");
    let (remote, work) = bare_remote_with_multiple_skill_content(
        "github-collection-update-apply-origin",
        &["alpha", "beta"],
    );
    let _rewrite = github_repo_rewrite("acme", "github-collection-update-apply", &remote);
    let source_url = "https://github.com/acme/github-collection-update-apply/tree/main";
    let first = preview_github_skill_collection(
        PreviewGithubSkillCollectionRequest {
            source_url: source_url.to_string(),
        },
        &managed_root,
    )
    .unwrap();
    let from_sha = first.collection.reviewed_head_sha.clone().unwrap();
    let alpha = first
        .collection
        .children
        .iter()
        .find(|child| child.name == "alpha")
        .unwrap()
        .clone();
    let beta = first
        .collection
        .children
        .iter()
        .find(|child| child.name == "beta")
        .unwrap()
        .clone();
    apply_github_skill_collection(
        GithubSkillCollectionApplyRequest {
            source_url: source_url.to_string(),
            collection_id: first.collection.id.clone(),
            preview_id: first.collection.preview_id,
            selections: vec![
                github_collection_child_selection(&alpha, SkillKind::User),
                github_collection_child_selection(&beta, SkillKind::Remote),
            ],
            actor: "test".to_string(),
        },
        &managed_root,
    )
    .unwrap();

    fs::write(
        work.join("skills/alpha/SKILL.md"),
        "---\nname: alpha\ndescription: Updated alpha\n---\n\n# Alpha updated\n",
    )
    .unwrap();
    fs::remove_dir_all(work.join("skills/beta")).unwrap();
    make_skill(&work.join("skills/gamma"), "gamma", "Gamma");
    commit_and_push_collection(&work, "Advance collection");

    let preview = github_update_preview(source_url, &managed_root);
    let to_sha = preview.to_sha.clone();
    let updated_alpha = preview
        .collection
        .children
        .iter()
        .find(|child| child.name == "alpha")
        .unwrap()
        .clone();
    let added_gamma = preview
        .collection
        .children
        .iter()
        .find(|child| child.name == "gamma")
        .unwrap()
        .clone();
    let result = apply_github_skill_collection_update(
        GithubSkillCollectionApplyRequest {
            source_url: source_url.to_string(),
            collection_id: preview.collection_id.clone(),
            preview_id: preview.preview_id.clone(),
            selections: vec![
                github_collection_child_selection(&updated_alpha, SkillKind::User),
                github_collection_child_selection(&added_gamma, SkillKind::User),
            ],
            actor: "test".to_string(),
        },
        &managed_root,
    )
    .unwrap();

    assert_eq!(
        result.collection.reviewed_head_sha.as_deref(),
        Some(to_sha.as_str())
    );
    assert_eq!(
        result.collection.previous_reviewed_head_sha.as_deref(),
        Some(from_sha.as_str())
    );
    let names: Vec<_> = result
        .collection
        .members
        .iter()
        .map(|member| member.managed_skill_name.as_str())
        .collect();
    assert!(names.contains(&"alpha"));
    assert!(names.contains(&"gamma"));
    assert!(!names.contains(&"beta"));
    let paths = managed_paths(&managed_root);
    let alpha_body = fs::read_to_string(paths.user_skills_root.join("alpha/SKILL.md")).unwrap();
    assert!(alpha_body.contains("Alpha updated"));
    assert!(paths
        .remote_skills_root
        .join("beta/current/SKILL.md")
        .exists());
    assert!(paths.user_skills_root.join("gamma/SKILL.md").exists());
    let connection = open_database(&paths.database_path).unwrap();
    let deployment_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM deployments", [], |row| row.get(0))
        .unwrap();
    assert_eq!(deployment_count, 0);

    let rollback_preview = preview_github_skill_collection_rollback(
        GithubCollectionRollbackRequest {
            collection_id: result.collection.id.clone(),
            preview_id: String::new(),
            actor: "test".to_string(),
        },
        &managed_root,
    )
    .unwrap();
    assert_eq!(rollback_preview.to_sha, from_sha);
    assert!(rollback_preview
        .leaving_skill_names
        .contains(&"gamma".to_string()));
    let rolled = apply_github_skill_collection_rollback(
        GithubCollectionRollbackRequest {
            collection_id: rollback_preview.collection_id,
            preview_id: rollback_preview.preview_id,
            actor: "test".to_string(),
        },
        &managed_root,
    )
    .unwrap();
    assert_eq!(
        rolled.collection.reviewed_head_sha.as_deref(),
        Some(from_sha.as_str())
    );
    assert_eq!(rolled.collection.previous_reviewed_head_sha, None);
    let restored_alpha = fs::read_to_string(paths.user_skills_root.join("alpha/SKILL.md")).unwrap();
    assert!(restored_alpha.contains("alpha skill"));
    let restored_names: Vec<_> = rolled
        .collection
        .members
        .iter()
        .map(|member| member.managed_skill_name.as_str())
        .collect();
    assert!(restored_names.contains(&"alpha"));
    assert!(restored_names.contains(&"beta"));
    assert!(!restored_names.contains(&"gamma"));
    assert!(paths.user_skills_root.join("gamma/SKILL.md").exists());
}

#[test]
fn github_collection_update_rejects_dirty_member_and_missing_previous_revision() {
    let root = temp_dir("github-collection-update-dirty");
    let managed_root = root.join("SkillBox");
    let (remote, work) = bare_remote_with_multiple_skill_content(
        "github-collection-update-dirty-origin",
        &["alpha"],
    );
    let _rewrite = github_repo_rewrite("acme", "github-collection-update-dirty", &remote);
    let source_url = "https://github.com/acme/github-collection-update-dirty/tree/main";
    let first = preview_github_skill_collection(
        PreviewGithubSkillCollectionRequest {
            source_url: source_url.to_string(),
        },
        &managed_root,
    )
    .unwrap();
    let alpha = first.collection.children[0].clone();
    apply_github_skill_collection(
        GithubSkillCollectionApplyRequest {
            source_url: source_url.to_string(),
            collection_id: first.collection.id.clone(),
            preview_id: first.collection.preview_id,
            selections: vec![github_collection_child_selection(&alpha, SkillKind::User)],
            actor: "test".to_string(),
        },
        &managed_root,
    )
    .unwrap();

    let rollback_error = preview_github_skill_collection_rollback(
        GithubCollectionRollbackRequest {
            collection_id: first.collection.id.clone(),
            preview_id: String::new(),
            actor: "test".to_string(),
        },
        &managed_root,
    )
    .unwrap_err();
    assert!(
        rollback_error.contains("no previous reviewed SHA"),
        "{rollback_error}"
    );

    fs::write(
        managed_root.join("user-skills/alpha/SKILL.md"),
        "---\nname: alpha\ndescription: Local edit\n---\n\n# Dirty\n",
    )
    .unwrap();
    fs::write(
        work.join("skills/alpha/SKILL.md"),
        "---\nname: alpha\ndescription: Remote edit\n---\n\n# Remote\n",
    )
    .unwrap();
    commit_and_push_collection(&work, "Remote edit");

    let preview = github_update_preview(source_url, &managed_root);
    let blocked = preview
        .changes
        .iter()
        .find(|change| change.name == "alpha")
        .unwrap();
    assert_eq!(blocked.change, GithubCollectionChildChangeKind::Blocked);
    assert!(!blocked.eligible);
    let updated = preview
        .collection
        .children
        .iter()
        .find(|child| child.name == "alpha")
        .unwrap();
    let apply_error = apply_github_skill_collection_update(
        GithubSkillCollectionApplyRequest {
            source_url: source_url.to_string(),
            collection_id: preview.collection_id,
            preview_id: preview.preview_id,
            selections: vec![github_collection_child_selection(updated, SkillKind::User)],
            actor: "test".to_string(),
        },
        &managed_root,
    )
    .unwrap_err();
    assert!(
        apply_error.contains("not eligible") || apply_error.contains("cannot be selected"),
        "{apply_error}"
    );
}

#[test]
fn github_collection_update_compensates_user_write_when_later_child_fails() {
    let root = temp_dir("github-collection-update-compensate");
    let managed_root = root.join("SkillBox");
    let (remote, work) = bare_remote_with_multiple_skill_content(
        "github-collection-update-compensate-origin",
        &["alpha", "beta"],
    );
    let _rewrite = github_repo_rewrite("acme", "github-collection-update-compensate", &remote);
    let source_url = "https://github.com/acme/github-collection-update-compensate/tree/main";
    let first = preview_github_skill_collection(
        PreviewGithubSkillCollectionRequest {
            source_url: source_url.to_string(),
        },
        &managed_root,
    )
    .unwrap();
    let alpha = first
        .collection
        .children
        .iter()
        .find(|child| child.name == "alpha")
        .unwrap()
        .clone();
    let beta = first
        .collection
        .children
        .iter()
        .find(|child| child.name == "beta")
        .unwrap()
        .clone();
    apply_github_skill_collection(
        GithubSkillCollectionApplyRequest {
            source_url: source_url.to_string(),
            collection_id: first.collection.id.clone(),
            preview_id: first.collection.preview_id,
            selections: vec![
                github_collection_child_selection(&alpha, SkillKind::User),
                github_collection_child_selection(&beta, SkillKind::Remote),
            ],
            actor: "test".to_string(),
        },
        &managed_root,
    )
    .unwrap();
    let original_alpha =
        fs::read_to_string(managed_root.join("user-skills/alpha/SKILL.md")).unwrap();

    fs::write(
        work.join("skills/alpha/SKILL.md"),
        "---\nname: alpha\ndescription: Updated alpha\n---\n\n# Alpha updated\n",
    )
    .unwrap();
    fs::write(
        work.join("skills/beta/SKILL.md"),
        "---\nname: beta\ndescription: Updated beta\n---\n\n# Beta updated\n",
    )
    .unwrap();
    commit_and_push_collection(&work, "Update both skills");

    let preview = github_update_preview(source_url, &managed_root);
    let updated_beta = preview
        .collection
        .children
        .iter()
        .find(|child| child.name == "beta")
        .unwrap();
    let version_name = format!("manual-{}", &updated_beta.content_hash[..12]);
    let conflicting = managed_root
        .join("remote-skills/beta/versions")
        .join(&version_name);
    fs::create_dir_all(&conflicting).unwrap();
    fs::write(
        conflicting.join("SKILL.md"),
        "---\nname: beta\ndescription: Conflicting version\n---\n\n# Conflict\n",
    )
    .unwrap();

    let updated_alpha = preview
        .collection
        .children
        .iter()
        .find(|child| child.name == "alpha")
        .unwrap()
        .clone();
    let updated_beta = updated_beta.clone();
    let error = apply_github_skill_collection_update(
        GithubSkillCollectionApplyRequest {
            source_url: source_url.to_string(),
            collection_id: preview.collection_id,
            preview_id: preview.preview_id,
            selections: vec![
                github_collection_child_selection(&updated_alpha, SkillKind::User),
                github_collection_child_selection(&updated_beta, SkillKind::Remote),
            ],
            actor: "test".to_string(),
        },
        &managed_root,
    )
    .unwrap_err();
    assert!(
        error.contains("different contents") || error.contains("incomplete"),
        "{error}"
    );
    let restored_alpha =
        fs::read_to_string(managed_root.join("user-skills/alpha/SKILL.md")).unwrap();
    assert_eq!(restored_alpha, original_alpha);
    let operations = list_operations(OperationFilter::default(), &managed_root)
        .unwrap()
        .operations;
    assert!(operations.iter().any(|operation| {
        operation.operation_type == "update_github_collection"
            && operation.status == OperationStatus::Failed
    }));
}

#[test]
fn github_collection_update_preview_reports_up_to_date_at_the_same_sha() {
    let root = temp_dir("github-collection-update-uptodate");
    let managed_root = root.join("SkillBox");
    let (remote, _work) = bare_remote_with_multiple_skill_content(
        "github-collection-update-uptodate-origin",
        &["alpha"],
    );
    let _rewrite = github_repo_rewrite("acme", "github-collection-update-uptodate", &remote);
    let source_url = "https://github.com/acme/github-collection-update-uptodate/tree/main";
    let first = preview_github_skill_collection(
        PreviewGithubSkillCollectionRequest {
            source_url: source_url.to_string(),
        },
        &managed_root,
    )
    .unwrap();
    apply_github_skill_collection(
        GithubSkillCollectionApplyRequest {
            source_url: source_url.to_string(),
            collection_id: first.collection.id.clone(),
            preview_id: first.collection.preview_id,
            selections: vec![github_collection_child_selection(
                &first.collection.children[0],
                SkillKind::User,
            )],
            actor: "test".to_string(),
        },
        &managed_root,
    )
    .unwrap();

    match preview_github_skill_collection_update(
        PreviewGithubSkillCollectionRequest {
            source_url: source_url.to_string(),
        },
        &managed_root,
    )
    .unwrap()
    {
        GithubCollectionUpdatePreviewResult::UpToDate { message, .. } => {
            assert!(message.contains("already at this reviewed SHA"));
        }
        other => panic!("expected up to date, got {other:?}"),
    }
}

#[test]
fn install_github_warning_target_requires_confirmation_before_any_install_state() {
    let root = temp_dir("install-github-warning-confirmation");
    let managed_root = root.join("SkillBox");
    let target_root = root.join("project/.agents/skills");
    fs::create_dir_all(&target_root).unwrap();
    add_workspace(
        WorkspaceAddRequest {
            path: target_root.clone(),
            kind: WorkspaceKind::User,
        },
        &managed_root,
    )
    .unwrap();
    let (remote, work) = bare_remote_with_root_skill_content(
        "install-github-warning-confirmation-origin",
        "warning-skill",
        "Warning skill",
        "Original body\n",
    );
    fs::write(
        work.join("SKILL.md"),
        "---
name: warning-skill
description: Warning skill
tools:
  - shell
---
# Warning skill
",
    )
    .unwrap();
    run_git(&work, &["add", "SKILL.md"]);
    run_git(
        &work,
        &[
            "-c",
            "user.name=SkillBox",
            "-c",
            "user.email=skillbox@example.invalid",
            "commit",
            "-m",
            "Add optional frontmatter",
        ],
    );
    run_git(&work, &["push", "origin", "main"]);
    let _rewrite = github_repo_rewrite("acme", "install-github-warning-confirmation", &remote);
    let source_url =
        "https://github.com/acme/install-github-warning-confirmation/blob/main/SKILL.md"
            .to_string();
    let preview = github_install_preview(&source_url, Some(target_root.clone()), &managed_root);
    assert_eq!(
        preview.compatibility.as_ref().unwrap().status,
        CompatibilityStatus::Warnings
    );
    let legacy_request: InstallGithubRemoteSkillRequest =
        serde_json::from_value(serde_json::json!({
            "source_url": source_url.clone(),
            "target_root": target_root.clone(),
            "preview_id": preview.preview_id.clone(),
            "actor": "desktop"
        }))
        .unwrap();
    assert!(!legacy_request.confirm_warnings);

    let assert_no_install_state = || {
        let paths = managed_paths(&managed_root);
        let remote_root = paths.remote_skills_root.join("warning-skill");
        assert!(!remote_root.join("versions").exists());
        assert!(!remote_root.join("current").exists());
        assert!(!remote_root.join("source.json").exists());
        assert!(!target_root.join("warning-skill").exists());
        let connection = open_database(&paths.database_path).unwrap();
        let indexed = connection
            .query_row(
                "SELECT name FROM skills WHERE name = 'warning-skill'",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .unwrap();
        assert_eq!(indexed, None);
    };

    let stale_error = install_github_remote_skill(
        InstallGithubRemoteSkillRequest {
            source_url: source_url.clone(),
            target_root: Some(target_root.clone()),
            preview_id: Some(format!("{}-stale", preview.preview_id)),
            confirm_warnings: true,
            actor: "cli".to_string(),
        },
        &managed_root,
    )
    .unwrap_err();
    assert!(stale_error.contains("Remote install preview is stale"));
    assert_no_install_state();

    let confirmation_error = install_github_remote_skill(
        InstallGithubRemoteSkillRequest {
            source_url: source_url.clone(),
            target_root: Some(target_root.clone()),
            preview_id: Some(preview.preview_id.clone()),
            confirm_warnings: false,
            actor: "cli".to_string(),
        },
        &managed_root,
    )
    .unwrap_err();
    assert!(confirmation_error.contains("explicitly confirm warnings"));
    assert_no_install_state();

    let result = install_github_remote_skill(
        InstallGithubRemoteSkillRequest {
            source_url,
            target_root: Some(target_root.clone()),
            preview_id: Some(preview.preview_id),
            confirm_warnings: true,
            actor: "cli".to_string(),
        },
        &managed_root,
    )
    .unwrap();
    assert_eq!(
        result.deployment.as_ref().unwrap().target_root,
        fs::canonicalize(&target_root).unwrap()
    );
    assert!(result.version_path.join("SKILL.md").exists());
    assert!(fs::symlink_metadata(result.deployment.unwrap().target_path)
        .unwrap()
        .file_type()
        .is_symlink());
}

#[test]
fn install_github_alias_target_deploys_and_indexes_only_canonical_root() {
    let root = temp_dir("install-github-canonical-alias-target");
    let managed_root = root.join("SkillBox");
    let canonical_root = root.join("shared/skills");
    let alias_root = root.join("project/.agents/skills");
    fs::create_dir_all(&canonical_root).unwrap();
    fs::create_dir_all(alias_root.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(&canonical_root, &alias_root).unwrap();
    let workspace = add_workspace(
        WorkspaceAddRequest {
            path: alias_root.clone(),
            kind: WorkspaceKind::User,
        },
        &managed_root,
    )
    .unwrap();
    let canonical_root = fs::canonicalize(canonical_root).unwrap();
    assert_eq!(workspace.canonical_path, canonical_root);
    assert_eq!(workspace.profile_id, "custom-skill-md");
    assert_eq!(workspace.root_key, "exact");

    let remote = bare_remote_with_skill_content(
        "install-github-canonical-alias-target-origin",
        "demo",
        "Demo skill",
        "",
    );
    let _rewrite = github_repo_rewrite("acme", "install-github-canonical-alias-target", &remote);
    let source_url = github_source_url("acme", "install-github-canonical-alias-target", "demo");
    let preview = github_install_preview(&source_url, Some(alias_root.clone()), &managed_root);
    let compatibility = preview.compatibility.as_ref().unwrap();
    assert_eq!(compatibility.profile.id, "custom-skill-md");
    assert_eq!(compatibility.root_key, "exact");
    assert_eq!(compatibility.target_root, canonical_root);

    let installed = install_github_remote_skill(
        InstallGithubRemoteSkillRequest {
            source_url,
            target_root: Some(alias_root),
            preview_id: Some(preview.preview_id),
            confirm_warnings: false,
            actor: "cli".to_string(),
        },
        &managed_root,
    )
    .unwrap();
    let deployment = installed.deployment.unwrap();
    assert_eq!(deployment.target_root, canonical_root);
    assert_eq!(deployment.target_path, canonical_root.join("demo"));

    let connection = open_database(&managed_paths(&managed_root).database_path).unwrap();
    let (indexed_root, indexed_path): (String, String) = connection
        .query_row(
            "
            SELECT target_root, target_path
            FROM deployments
            WHERE skill_name = 'demo'
            ",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(indexed_root, canonical_root.to_string_lossy());
    assert_eq!(indexed_path, canonical_root.join("demo").to_string_lossy());
}

#[test]
fn install_github_alias_retarget_rejects_stale_preview_without_install_state() {
    let root = temp_dir("install-github-alias-retarget");
    let managed_root = root.join("SkillBox");
    let first_root = root.join("shared-a/skills");
    let second_root = root.join("shared-b/skills");
    let alias_root = root.join("project/.agents/skills");
    fs::create_dir_all(&first_root).unwrap();
    fs::create_dir_all(&second_root).unwrap();
    fs::create_dir_all(alias_root.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(&first_root, &alias_root).unwrap();
    for target_root in [&first_root, &second_root] {
        let workspace = add_workspace(
            WorkspaceAddRequest {
                path: target_root.clone(),
                kind: WorkspaceKind::User,
            },
            &managed_root,
        )
        .unwrap();
        assert_eq!(workspace.profile_id, "custom-skill-md");
        assert_eq!(workspace.root_key, "exact");
    }

    let remote = bare_remote_with_skill_content(
        "install-github-alias-retarget-origin",
        "demo",
        "Demo skill",
        "",
    );
    let _rewrite = github_repo_rewrite("acme", "install-github-alias-retarget", &remote);
    let source_url = github_source_url("acme", "install-github-alias-retarget", "demo");
    let preview = github_install_preview(&source_url, Some(alias_root.clone()), &managed_root);
    assert_eq!(
        preview.compatibility.as_ref().unwrap().target_root,
        fs::canonicalize(&first_root).unwrap()
    );

    fs::remove_file(&alias_root).unwrap();
    std::os::unix::fs::symlink(&second_root, &alias_root).unwrap();
    let error = install_github_remote_skill(
        InstallGithubRemoteSkillRequest {
            source_url,
            target_root: Some(alias_root),
            preview_id: Some(preview.preview_id),
            confirm_warnings: false,
            actor: "cli".to_string(),
        },
        &managed_root,
    )
    .unwrap_err();
    assert!(error.contains("Remote install preview is stale"));

    let paths = managed_paths(&managed_root);
    let remote_root = paths.remote_skills_root.join("demo");
    assert!(!remote_root.join("versions").exists());
    assert!(!remote_root.join("current").exists());
    assert!(!remote_root.join("source.json").exists());
    assert!(!first_root.join("demo").exists());
    assert!(!second_root.join("demo").exists());
    let connection = open_database(&paths.database_path).unwrap();
    let indexed_skill = connection
        .query_row("SELECT name FROM skills WHERE name = 'demo'", [], |row| {
            row.get::<_, String>(0)
        })
        .optional()
        .unwrap();
    let indexed_deployment = connection
        .query_row(
            "SELECT skill_name FROM deployments WHERE skill_name = 'demo'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .unwrap();
    assert_eq!(indexed_skill, None);
    assert_eq!(indexed_deployment, None);
}

#[test]
fn preview_github_root_skill_rejects_symlink_escape_without_managed_state() {
    let root = temp_dir("preview-github-root-symlink-escape");
    let managed_root = root.join("SkillBox");
    let outside = root.join("outside.txt");
    fs::write(&outside, "secret").unwrap();
    let (remote, work) = bare_remote_with_root_skill_content(
        "preview-github-root-symlink-escape-origin",
        "humanizer-zh",
        "Humanizer zh",
        "Original body\n",
    );
    symlink_any(&outside, &work.join("outside-link")).unwrap();
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
            "Add escaping symlink",
        ],
    );
    run_git(&work, &["push", "origin", "main"]);
    let _rewrite = github_repo_rewrite("acme", "preview-github-root-symlink-escape", &remote);

    let error = preview_github_remote_skill_install(
        PreviewGithubRemoteSkillInstallRequest {
            source_url:
                "https://github.com/acme/preview-github-root-symlink-escape/blob/main/SKILL.md"
                    .to_string(),
            target_root: None,
        },
        &managed_root,
    )
    .unwrap_err();

    assert!(error.contains("Refusing to copy symlink outside source root"));
    assert!(!managed_root.exists());
}

#[test]
fn github_root_skill_update_check_preview_and_apply_preserve_root_metadata() {
    let root = temp_dir("github-root-skill-update");
    let managed_root = root.join("SkillBox");
    let (remote, work) = bare_remote_with_root_skill_content(
        "github-root-skill-update-origin",
        "humanizer-zh",
        "Humanizer zh",
        "Original body\n",
    );
    let _rewrite = github_repo_rewrite("acme", "github-root-skill-update", &remote);
    let source_url =
        "https://github.com/acme/github-root-skill-update/blob/main/SKILL.md".to_string();
    let install_preview = github_install_preview(&source_url, None, &managed_root);
    let installed = install_github_remote_skill(
        InstallGithubRemoteSkillRequest {
            source_url,
            target_root: None,
            preview_id: Some(install_preview.preview_id),
            confirm_warnings: false,
            actor: "cli".to_string(),
        },
        &managed_root,
    )
    .unwrap();

    make_skill_with_body(&work, "humanizer-zh", "Humanizer zh", "Updated body\n");
    fs::write(work.join("assets/prompt.txt"), "updated prompt\n").unwrap();
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
            "Update root skill",
        ],
    );
    run_git(&work, &["push", "origin", "main"]);
    let latest_sha = remote_head(&remote);

    let checked = check_remote_skill_update(&managed_root, "humanizer-zh").unwrap();
    let status = remote_status(&checked.statuses, "humanizer-zh");
    assert_eq!(status.state, RemoteSkillUpdateState::UpdateAvailable);
    assert_eq!(status.latest_sha.as_deref(), Some(latest_sha.as_str()));

    let preview = preview_remote_version_change(
        RemoteVersionChangeRequest {
            skill_name: "humanizer-zh".to_string(),
            action: RemoteVersionChangeAction::Update,
            target_version: Some(latest_sha.clone()),
            actor: "cli".to_string(),
        },
        &managed_root,
    )
    .unwrap();
    assert_eq!(preview.from_version, installed.installed_sha);
    assert_eq!(preview.to_version, latest_sha);
    assert!(preview.files.iter().any(|file| file.path == "SKILL.md"));
    assert!(preview
        .files
        .iter()
        .any(|file| file.path == "assets/prompt.txt"));
    assert!(!preview
        .files
        .iter()
        .any(|file| file.path == ".git" || file.path.starts_with(".git/")));

    let applied = apply_remote_version_change(
        RemoteVersionChangeApplyRequest {
            skill_name: "humanizer-zh".to_string(),
            action: RemoteVersionChangeAction::Update,
            target_version: preview.to_version.clone(),
            preview_id: Some(preview.preview_id),
            actor: "cli".to_string(),
        },
        &managed_root,
    )
    .unwrap();

    let paths = managed_paths(&managed_root);
    let remote_root = paths.remote_skills_root.join("humanizer-zh");
    let updated_version = remote_root.join("versions").join(&latest_sha);
    assert_eq!(applied.to_version, latest_sha);
    assert_eq!(
        current_remote_version(&paths, "humanizer-zh").unwrap(),
        latest_sha
    );
    assert_eq!(
        fs::read_to_string(updated_version.join("assets/prompt.txt")).unwrap(),
        "updated prompt\n"
    );
    assert!(!updated_version.join(".git").exists());
    let source = read_remote_source(&remote_root).unwrap();
    assert!(source.root);
    assert_eq!(source.path.as_deref(), Some(""));
    assert_eq!(source.current_version.as_deref(), Some(latest_sha.as_str()));
}

#[test]
fn install_github_remote_skill_rejects_missing_preview_id() {
    let root = temp_dir("install-github-missing-preview");
    let managed_root = root.join("SkillBox");

    let error = install_github_remote_skill(
        InstallGithubRemoteSkillRequest {
            source_url: "https://github.com/acme/repo/tree/main/skills/demo".to_string(),
            target_root: None,
            preview_id: None,
            confirm_warnings: false,
            actor: "cli".to_string(),
        },
        &managed_root,
    )
    .unwrap_err();

    assert!(error.contains("Remote install preview is required"));
    assert!(!managed_root.exists());
}

#[test]
fn install_github_remote_skill_rejects_stale_preview_id() {
    let root = temp_dir("install-github-stale-preview");
    let managed_root = root.join("SkillBox");
    let remote = bare_remote_with_skill_content(
        "install-github-stale-preview-origin",
        "find-skills",
        "Find skills",
        "",
    );
    let _rewrite = github_repo_rewrite("acme", "install-github-stale-preview", &remote);
    let source_url = github_source_url("acme", "install-github-stale-preview", "find-skills");
    let preview = github_install_preview(&source_url, None, &managed_root);

    let error = install_github_remote_skill(
        InstallGithubRemoteSkillRequest {
            source_url,
            target_root: None,
            preview_id: Some(format!("{}-stale", preview.preview_id)),
            confirm_warnings: false,
            actor: "cli".to_string(),
        },
        &managed_root,
    )
    .unwrap_err();

    assert!(error.contains("Remote install preview is stale"));
    assert!(!managed_root
        .join("remote-skills")
        .join("find-skills")
        .join("current")
        .exists());
}

#[test]
fn install_github_remote_skill_rejects_preview_after_branch_advances() {
    let root = temp_dir("install-github-branch-advanced");
    let managed_root = root.join("SkillBox");
    let remote = bare_remote("install-github-branch-advanced-origin");
    let work = temp_dir("install-github-branch-advanced-work");
    run_git(&work, &["init", "-b", "main"]);
    let skill_dir = work.join("skills").join("find-skills");
    make_skill_with_body(&skill_dir, "find-skills", "Find skills", "Original body\n");
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
            "Add skill",
        ],
    );
    run_git(
        &work,
        &["remote", "add", "origin", remote.to_str().unwrap()],
    );
    run_git(&work, &["push", "-u", "origin", "main"]);
    let _rewrite = github_repo_rewrite("acme", "install-github-branch-advanced", &remote);
    let source_url = github_source_url("acme", "install-github-branch-advanced", "find-skills");
    let preview = github_install_preview(&source_url, None, &managed_root);

    make_skill_with_body(&skill_dir, "find-skills", "Find skills", "Advanced body\n");
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
            "Advance skill",
        ],
    );
    run_git(&work, &["push", "origin", "main"]);

    let error = install_github_remote_skill(
        InstallGithubRemoteSkillRequest {
            source_url,
            target_root: None,
            preview_id: Some(preview.preview_id),
            confirm_warnings: false,
            actor: "cli".to_string(),
        },
        &managed_root,
    )
    .unwrap_err();

    let paths = managed_paths(&managed_root);
    let remote_root = paths.remote_skills_root.join("find-skills");
    assert!(error.contains("Remote install preview is stale"));
    assert!(!remote_root.join("versions").exists());
    assert!(!remote_root.join("current").exists());
    assert!(!remote_root.join("source.json").exists());
    let connection = open_database(&paths.database_path).unwrap();
    let indexed = connection
        .query_row(
            "SELECT name FROM skills WHERE name = 'find-skills'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .unwrap();
    assert_eq!(indexed, None);
}

#[test]
fn install_github_remote_skill_deploys_to_target_root() {
    let root = temp_dir("install-github-deploy");
    let managed_root = root.join("SkillBox");
    let target_root = root.join("project/.claude/skills");
    fs::create_dir_all(&target_root).unwrap();
    add_workspace(
        WorkspaceAddRequest {
            path: target_root.clone(),
            kind: WorkspaceKind::User,
        },
        &managed_root,
    )
    .unwrap();
    let remote = bare_remote_with_skill_content(
        "install-github-deploy-origin",
        "find-skills",
        "Find skills",
        "",
    );
    let _rewrite = github_repo_rewrite("acme", "install-github-deploy", &remote);
    let source_url = github_source_url("acme", "install-github-deploy", "find-skills");
    let preview = github_install_preview(&source_url, Some(target_root.clone()), &managed_root);

    assert_eq!(
        preview.compatibility.as_ref().unwrap().status,
        CompatibilityStatus::Compatible
    );
    assert!(!target_root.join("find-skills").exists());

    let result = install_github_remote_skill(
        InstallGithubRemoteSkillRequest {
            source_url,
            target_root: Some(target_root.clone()),
            preview_id: Some(preview.preview_id),
            confirm_warnings: false,
            actor: "cli".to_string(),
        },
        &managed_root,
    )
    .unwrap();

    let deployment = result.deployment.unwrap();
    assert_eq!(
        deployment.target_root,
        fs::canonicalize(&target_root).unwrap()
    );
    assert!(fs::symlink_metadata(&deployment.target_path)
        .unwrap()
        .file_type()
        .is_symlink());
    assert_eq!(
        fs::canonicalize(&deployment.target_path).unwrap(),
        fs::canonicalize(result.current_path).unwrap()
    );
}

#[test]
fn install_github_remote_skill_reuses_existing_version_snapshot() {
    let root = temp_dir("install-github-reuse-version");
    let managed_root = root.join("SkillBox");
    let remote = bare_remote_with_skill_content(
        "install-github-reuse-version-origin",
        "find-skills",
        "Find skills",
        "",
    );
    let _rewrite = github_repo_rewrite("acme", "install-github-reuse-version", &remote);
    let source_url = github_source_url("acme", "install-github-reuse-version", "find-skills");
    let first_preview = github_install_preview(&source_url, None, &managed_root);

    let first = install_github_remote_skill(
        InstallGithubRemoteSkillRequest {
            source_url: source_url.clone(),
            target_root: None,
            preview_id: Some(first_preview.preview_id),
            confirm_warnings: false,
            actor: "cli".to_string(),
        },
        &managed_root,
    )
    .unwrap();
    let marker = first.version_path.join("marker.txt");
    fs::write(&marker, "kept").unwrap();
    let second_preview = github_install_preview(&source_url, None, &managed_root);

    let second = install_github_remote_skill(
        InstallGithubRemoteSkillRequest {
            source_url,
            target_root: None,
            preview_id: Some(second_preview.preview_id),
            confirm_warnings: false,
            actor: "cli".to_string(),
        },
        &managed_root,
    )
    .unwrap();

    assert_eq!(second.version_path, first.version_path);
    assert_eq!(fs::read_to_string(marker).unwrap(), "kept");
}

#[test]
fn install_github_remote_skill_cleans_partial_version_on_copy_failure() {
    let root = temp_dir("install-github-copy-failure");
    let managed_root = root.join("SkillBox");
    let remote = bare_remote_with_skill_content(
        "install-github-copy-failure-origin",
        "find-skills",
        "Find skills",
        "",
    );
    let installed_sha = remote_head(&remote);
    let _rewrite = github_repo_rewrite("acme", "install-github-copy-failure", &remote);
    let source_url = github_source_url("acme", "install-github-copy-failure", "find-skills");
    let preview = github_install_preview(&source_url, None, &managed_root);
    let version_path = managed_root
        .join("remote-skills")
        .join("find-skills")
        .join("versions")
        .join(&installed_sha);
    fs::create_dir_all(version_path.parent().unwrap()).unwrap();
    fs::write(&version_path, "not a directory").unwrap();

    let error = install_github_remote_skill(
        InstallGithubRemoteSkillRequest {
            source_url,
            target_root: None,
            preview_id: Some(preview.preview_id),
            confirm_warnings: false,
            actor: "cli".to_string(),
        },
        &managed_root,
    )
    .unwrap_err();

    assert!(error.contains("Destination already exists"));
    assert!(!version_path.exists());
    assert!(!managed_root
        .join("remote-skills")
        .join("find-skills")
        .join("current")
        .exists());
}
