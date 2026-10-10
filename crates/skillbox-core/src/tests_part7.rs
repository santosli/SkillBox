use super::*;
use crate::test_support::*;
use std::fs;
use std::sync::{Arc, Barrier};

#[test]
fn install_github_remote_skill_rejects_traversal_url_without_creating_store() {
    let root = temp_dir("install-github-traversal");
    let managed_root = root.join("SkillBox");

    let error = install_github_remote_skill(
        InstallGithubRemoteSkillRequest {
            source_url: "https://github.com/acme/repo/tree/main/skills/../../secret".to_string(),
            target_root: None,
            preview_id: Some("stale".to_string()),
            confirm_warnings: false,
            actor: "cli".to_string(),
        },
        &managed_root,
    )
    .unwrap_err();

    assert!(error.contains("path must stay inside the repository"));
    assert!(!managed_root.exists());
}

#[test]
fn install_github_remote_skill_rejects_non_github_url_without_creating_store() {
    let root = temp_dir("install-github-non-github");
    let managed_root = root.join("SkillBox");

    let error = install_github_remote_skill(
        InstallGithubRemoteSkillRequest {
            source_url: "https://example.com/acme/repo/tree/main/skills/demo".to_string(),
            target_root: None,
            preview_id: Some("stale".to_string()),
            confirm_warnings: false,
            actor: "cli".to_string(),
        },
        &managed_root,
    )
    .unwrap_err();

    assert!(error.contains("Only GitHub URLs are supported"));
    assert!(!managed_root.exists());
}

#[test]
fn install_github_remote_skill_rejects_invalid_ref_without_creating_store() {
    let root = temp_dir("install-github-invalid-ref");
    let managed_root = root.join("SkillBox");

    let error = install_github_remote_skill(
        InstallGithubRemoteSkillRequest {
            source_url: "https://github.com/acme/repo/tree/-bad/skills/demo".to_string(),
            target_root: None,
            preview_id: Some("stale".to_string()),
            confirm_warnings: false,
            actor: "cli".to_string(),
        },
        &managed_root,
    )
    .unwrap_err();

    assert!(error.contains("Git reference must not start with '-'"));
    assert!(!managed_root.exists());
}

#[test]
fn install_github_remote_skill_refuses_non_symlink_current_and_removes_new_version() {
    let root = temp_dir("install-github-current-conflict");
    let managed_root = root.join("SkillBox");
    let remote = bare_remote_with_skill_content(
        "install-github-current-conflict-origin",
        "find-skills",
        "Find skills",
        "",
    );
    let installed_sha = remote_head(&remote);
    let _rewrite = github_repo_rewrite("acme", "install-github-current-conflict", &remote);
    let source_url = github_source_url("acme", "install-github-current-conflict", "find-skills");
    let preview = github_install_preview(&source_url, None, &managed_root);
    let remote_root = managed_root.join("remote-skills").join("find-skills");
    let current_path = remote_root.join("current");
    fs::create_dir_all(&remote_root).unwrap();
    fs::write(&current_path, "not a symlink").unwrap();

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

    assert!(error.contains("Refusing to replace existing non-symlink current"));
    assert_eq!(fs::read_to_string(&current_path).unwrap(), "not a symlink");
    assert!(!remote_root.join("versions").join(installed_sha).exists());
}

#[test]
fn check_remote_skill_updates_ignores_commits_outside_skill_path() {
    let root = temp_dir("remote-update-same-skill-path");
    let managed_root = root.join("SkillBox");
    let paths = ensure_managed_layout(&managed_root).unwrap();
    let remote = bare_remote("remote-update-same-skill-path-origin");
    let work = temp_dir("remote-update-same-skill-path-work");
    run_git(&work, &["init", "-b", "main"]);
    make_skill(
        &work.join("skills").join("find-skills"),
        "find-skills",
        "Find skills",
    );
    make_skill(&work.join("skills").join("other"), "other", "Other skill");
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
            "Add skills",
        ],
    );
    run_git(
        &work,
        &["remote", "add", "origin", remote.to_str().unwrap()],
    );
    run_git(&work, &["push", "-u", "origin", "main"]);
    let installed_sha = remote_head(&remote);
    let find_skills_version = paths
        .remote_skills_root
        .join("find-skills")
        .join("versions")
        .join(&installed_sha);
    copy_skill_dir(
        &work.join("skills").join("find-skills"),
        &find_skills_version,
    )
    .unwrap();
    update_current_symlink(
        &paths.remote_skills_root.join("find-skills"),
        &find_skills_version,
    )
    .unwrap();
    let other_version = paths
        .remote_skills_root
        .join("other")
        .join("versions")
        .join(&installed_sha);
    copy_skill_dir(&work.join("skills").join("other"), &other_version).unwrap();
    update_current_symlink(&paths.remote_skills_root.join("other"), &other_version).unwrap();
    fs::write(
        work.join("skills").join("other").join("notes.md"),
        "other skill docs\n",
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
            "Update other skill",
        ],
    );
    run_git(&work, &["push", "origin", "main"]);
    let latest_sha = remote_head(&remote);

    write_remote_source_with_json(
        &paths.remote_skills_root.join("find-skills"),
        &format!(
            r#"{{
                  "type":"github",
                  "repoUrl":"{}",
                  "path":"skills/find-skills",
                  "ref":"main",
                  "refKind":"branch",
                  "tracking":true,
                  "currentVersion":"{}",
                  "installedSha":"{}"
                }}"#,
            remote.to_string_lossy(),
            installed_sha,
            installed_sha
        ),
    );
    write_remote_source_with_json(
        &paths.remote_skills_root.join("other"),
        &format!(
            r#"{{
                  "type":"github",
                  "repoUrl":"{}",
                  "path":"skills/other",
                  "ref":"main",
                  "refKind":"branch",
                  "tracking":true,
                  "currentVersion":"{}",
                  "installedSha":"{}"
                }}"#,
            remote.to_string_lossy(),
            installed_sha,
            installed_sha
        ),
    );

    let result = check_remote_skill_updates(&managed_root).unwrap();
    let find_skills = remote_status(&result.statuses, "find-skills");
    let other = remote_status(&result.statuses, "other");

    assert_eq!(find_skills.state, RemoteSkillUpdateState::UpToDate);
    assert!(!find_skills.update_available);
    assert_eq!(find_skills.latest_sha.as_deref(), Some(latest_sha.as_str()));
    assert_eq!(other.state, RemoteSkillUpdateState::UpdateAvailable);
    assert!(other.update_available);
    assert_eq!(other.latest_sha.as_deref(), Some(latest_sha.as_str()));
}

#[test]
fn temporary_work_dirs_are_unique_across_concurrent_checks() {
    let barrier = Arc::new(Barrier::new(32));
    let handles = (0..32)
        .map(|_| {
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                temporary_work_dir("concurrent-check")
            })
        })
        .collect::<Vec<_>>();
    let mut paths = handles
        .into_iter()
        .map(|handle| handle.join().unwrap())
        .collect::<Vec<_>>();
    paths.sort();
    paths.dedup();

    assert_eq!(paths.len(), 32);
}

#[test]
fn check_remote_skill_updates_marks_missing_source_separately_from_not_checkable() {
    let root = temp_dir("remote-not-checkable");
    let managed_root = root.join("SkillBox");
    let paths = ensure_managed_layout(&managed_root).unwrap();
    fs::create_dir_all(paths.remote_skills_root.join("missing-source")).unwrap();
    fs::create_dir_all(paths.remote_skills_root.join("manual-source")).unwrap();
    fs::write(
        paths
            .remote_skills_root
            .join("manual-source")
            .join("source.json"),
        r#"{"type":"manual","installedSha":"manual-abc123"}"#,
    )
    .unwrap();

    let result = check_remote_skill_updates(&managed_root).unwrap();
    let missing = remote_status(&result.statuses, "missing-source");
    let manual = remote_status(&result.statuses, "manual-source");

    assert_eq!(missing.state, RemoteSkillUpdateState::NoSource);
    assert_eq!(manual.state, RemoteSkillUpdateState::NotCheckable);
    assert!(!missing.update_available);
    assert!(!manual.update_available);
}

#[test]
fn cached_remote_skill_updates_reuses_last_check_and_marks_missing_sources() {
    let root = temp_dir("remote-update-cache");
    let managed_root = root.join("SkillBox");
    let paths = ensure_managed_layout(&managed_root).unwrap();
    let remote = bare_remote_with_main("remote-update-cache-origin");
    let latest_sha = remote_head(&remote);

    write_remote_source(
        &paths.remote_skills_root.join("fresh"),
        &remote,
        &latest_sha,
    );
    fs::create_dir_all(paths.remote_skills_root.join("missing-source")).unwrap();

    let checked = check_remote_skill_updates(&managed_root).unwrap();
    let cached = cached_remote_skill_updates(&managed_root).unwrap();
    let fresh = remote_status(&cached.statuses, "fresh");
    let missing = remote_status(&cached.statuses, "missing-source");

    assert_eq!(cached.checked_at, checked.checked_at);
    assert_eq!(fresh.state, RemoteSkillUpdateState::UpToDate);
    assert_eq!(fresh.latest_sha.as_deref(), Some(latest_sha.as_str()));
    assert_eq!(missing.state, RemoteSkillUpdateState::NoSource);
}

#[test]
fn check_remote_skill_updates_records_git_failures_per_skill() {
    let root = temp_dir("remote-check-failed");
    let managed_root = root.join("SkillBox");
    let paths = ensure_managed_layout(&managed_root).unwrap();
    write_remote_source(
        &paths.remote_skills_root.join("broken"),
        &root.join("missing.git"),
        "0000000000000000000000000000000000000000",
    );

    let result = check_remote_skill_updates(&managed_root).unwrap();
    let broken = remote_status(&result.statuses, "broken");

    assert_eq!(broken.state, RemoteSkillUpdateState::CheckFailed);
    assert!(!broken.update_available);
    assert!(broken.message.as_deref().unwrap_or("").contains("Git"));
}

#[test]
fn check_remote_skill_update_preserves_cached_success_on_failure() {
    let root = temp_dir("remote-check-preserve-cache");
    let managed_root = root.join("SkillBox");
    let paths = ensure_managed_layout(&managed_root).unwrap();
    let remote = bare_remote_with_main("remote-check-preserve-cache-origin");
    let latest_sha = remote_head(&remote);
    let skill_root = paths.remote_skills_root.join("fresh");
    write_remote_source(&skill_root, &remote, &latest_sha);

    let checked = check_remote_skill_updates(&managed_root).unwrap();
    assert_eq!(
        remote_status(&checked.statuses, "fresh").state,
        RemoteSkillUpdateState::UpToDate
    );
    write_remote_source(&skill_root, &root.join("missing.git"), &latest_sha);

    let failed = check_remote_skill_updates(&managed_root).unwrap();
    let fresh = remote_status(&failed.statuses, "fresh");

    assert_eq!(fresh.state, RemoteSkillUpdateState::UpToDate);
    assert_eq!(fresh.latest_sha.as_deref(), Some(latest_sha.as_str()));
    assert!(fresh
        .message
        .as_deref()
        .unwrap_or("")
        .starts_with("Last check failed: Git update check failed:"));
}

#[test]
fn check_single_remote_skill_update_only_refreshes_requested_skill() {
    let root = temp_dir("remote-check-one");
    let managed_root = root.join("SkillBox");
    let paths = ensure_managed_layout(&managed_root).unwrap();
    let remote = bare_remote_with_main("remote-check-one-origin");
    let latest_sha = remote_head(&remote);
    write_remote_source(
        &paths.remote_skills_root.join("target"),
        &remote,
        "0000000000000000000000000000000000000000",
    );
    write_remote_source(
        &paths.remote_skills_root.join("other"),
        &remote,
        &latest_sha,
    );
    check_remote_skill_updates(&managed_root).unwrap();
    write_remote_source(
        &paths.remote_skills_root.join("other"),
        &root.join("missing.git"),
        &latest_sha,
    );

    let result = check_remote_skill_update(&managed_root, "target").unwrap();
    let target = remote_status(&result.statuses, "target");
    let other = remote_status(&result.statuses, "other");

    assert_eq!(target.state, RemoteSkillUpdateState::UpdateAvailable);
    assert_eq!(other.state, RemoteSkillUpdateState::UpToDate);
    assert_eq!(other.message, None);
}

#[test]
fn check_remote_skill_updates_uses_limited_concurrency() {
    let source = include_str!("remote.rs");
    let check_start = source.find("pub fn check_remote_skill_updates").unwrap();
    let cached_start = source.find("pub fn cached_remote_skill_updates").unwrap();
    let check_source = &source[check_start..cached_start];

    assert!(include_str!("lib.rs").contains("const REMOTE_UPDATE_CHECK_CONCURRENCY: usize = 3;"));
    assert!(check_source.contains("check_remote_skill_update_batch"));
    assert!(check_source.contains("REMOTE_UPDATE_CHECK_CONCURRENCY"));
}

#[test]
fn check_remote_skill_updates_marks_pinned_sources() {
    let root = temp_dir("remote-pinned-sources");
    let managed_root = root.join("SkillBox");
    let paths = ensure_managed_layout(&managed_root).unwrap();

    write_remote_source_with_json(
        &paths.remote_skills_root.join("tagged"),
        r#"{
              "type":"github",
              "url":"https://github.com/acme/skills/tree/v1.0.0/skills/tagged",
              "repoUrl":"https://github.com/acme/skills.git",
              "ref":"v1.0.0",
              "refKind":"tag",
              "tracking":true,
              "currentVersion":"0123456789abcdef0123456789abcdef01234567",
              "installedSha":"0123456789abcdef0123456789abcdef01234567"
            }"#,
    );
    write_remote_source_with_json(
        &paths.remote_skills_root.join("commit"),
        r#"{
              "type":"github",
              "repoUrl":"https://github.com/acme/skills.git",
              "ref":"0123456789abcdef0123456789abcdef01234567",
              "currentVersion":"0123456789abcdef0123456789abcdef01234567",
              "installedSha":"0123456789abcdef0123456789abcdef01234567"
            }"#,
    );

    let result = check_remote_skill_updates(&managed_root).unwrap();
    let tagged = remote_status(&result.statuses, "tagged");
    assert_eq!(tagged.state, RemoteSkillUpdateState::Pinned);
    assert!(!tagged.update_available);
    assert_eq!(
        tagged.source_url.as_deref(),
        Some("https://github.com/acme/skills/tree/v1.0.0/skills/tagged")
    );
    assert_eq!(tagged.message.as_deref(), Some("Pinned GitHub source."));
    assert!(!tagged.tracking);

    let commit = remote_status(&result.statuses, "commit");
    assert_eq!(commit.state, RemoteSkillUpdateState::Pinned);
    assert_eq!(commit.ref_kind.as_deref(), Some("commit"));
    assert!(!commit.tracking);
}

#[test]
fn check_remote_skill_updates_compares_latest_sha_to_current_version_for_manual_binding() {
    let root = temp_dir("remote-manual-bound-update");
    let managed_root = root.join("SkillBox");
    let paths = ensure_managed_layout(&managed_root).unwrap();
    let remote = bare_remote_with_main("remote-manual-bound-update-origin");
    let latest_sha = remote_head(&remote);

    write_remote_source_with_json(
        &paths.remote_skills_root.join("bound"),
        &format!(
            r#"{{
                  "type":"github",
                  "repoUrl":"{}",
                  "ref":"main",
                  "refKind":"branch",
                  "tracking":true,
                  "currentVersion":"manual-abc123def456",
                  "installedSha":null,
                  "latestSha":"{}"
                }}"#,
            remote.to_string_lossy(),
            latest_sha
        ),
    );

    let result = check_remote_skill_updates(&managed_root).unwrap();
    let bound = remote_status(&result.statuses, "bound");
    assert_eq!(bound.state, RemoteSkillUpdateState::UpdateAvailable);
    assert_eq!(bound.latest_sha.as_deref(), Some(latest_sha.as_str()));
    assert_eq!(
        bound.current_version.as_deref(),
        Some("manual-abc123def456")
    );
    assert_eq!(bound.installed_sha, None);
}

#[test]
fn source_binding_preview_detects_exact_match() {
    let root = temp_dir("source-binding-exact");
    let managed_root = root.join("SkillBox");
    let source = root.join("local").join("demo");
    make_skill(&source, "demo", "Demo skill");
    import_skill(&source, SkillKind::Remote, &managed_root).unwrap();
    let remote =
        bare_remote_with_skill_content("source-binding-exact-origin", "demo", "Demo skill", "");
    let _rewrite = github_repo_rewrite("acme", "source-binding-exact", &remote);

    let preview = preview_remote_source_binding(
        RemoteSourceBindingRequest {
            skill_name: "demo".to_string(),
            source_url: github_source_url("acme", "source-binding-exact", "demo"),
            actor: "cli".to_string(),
        },
        &managed_root,
    )
    .unwrap();

    assert_eq!(preview.validation, SourceBindingValidation::ExactMatch);
    assert_eq!(preview.skill_name, "demo");
    assert_eq!(preview.ref_kind.as_deref(), Some("branch"));
    assert!(preview.tracking);
}

#[test]
fn source_binding_supports_repository_root_skill_metadata() {
    let root = temp_dir("source-binding-root-skill");
    let managed_root = root.join("SkillBox");
    let source = root.join("local").join("humanizer-zh");
    make_skill(&source, "humanizer-zh", "Humanizer zh");
    import_skill(&source, SkillKind::Remote, &managed_root).unwrap();
    let (remote, _work) = bare_remote_with_root_skill_content(
        "source-binding-root-skill-origin",
        "humanizer-zh",
        "Humanizer zh",
        "",
    );
    let _rewrite = github_repo_rewrite("acme", "source-binding-root-skill", &remote);
    let source_url = "https://github.com/acme/source-binding-root-skill".to_string();

    let preview = preview_remote_source_binding(
        RemoteSourceBindingRequest {
            skill_name: "humanizer-zh".to_string(),
            source_url: source_url.clone(),
            actor: "cli".to_string(),
        },
        &managed_root,
    )
    .unwrap();

    assert!(preview.root);
    assert_eq!(preview.path, "");
    assert_eq!(
        preview.source_url,
        "https://github.com/acme/source-binding-root-skill/tree/main"
    );
    assert_eq!(preview.validation, SourceBindingValidation::ExactMatch);

    bind_remote_source(
        BindRemoteSourceRequest {
            skill_name: "humanizer-zh".to_string(),
            source_url,
            actor: "cli".to_string(),
        },
        &managed_root,
    )
    .unwrap();

    let paths = managed_paths(&managed_root);
    let metadata = read_remote_source(&paths.remote_skills_root.join("humanizer-zh")).unwrap();
    assert!(metadata.root);
    assert_eq!(metadata.path.as_deref(), Some(""));
    assert_eq!(
        remote_source_browser_url(&metadata).as_deref(),
        Some("https://github.com/acme/source-binding-root-skill/tree/main")
    );
}

#[test]
fn source_binding_preview_resolves_marketplace_skill_path() {
    let root = temp_dir("source-binding-marketplace-path");
    let managed_root = root.join("SkillBox");
    let source = root.join("local").join("find-skills");
    make_skill(&source, "find-skills", "Find skills");
    import_skill(&source, SkillKind::Remote, &managed_root).unwrap();
    let remote = bare_remote_with_skill_content(
        "source-binding-marketplace-path-origin",
        "find-skills",
        "Find skills",
        "",
    );
    let _rewrite = github_repo_rewrite("acme", "source-binding-marketplace-path", &remote);

    let preview = preview_remote_source_binding(
        RemoteSourceBindingRequest {
            skill_name: "find-skills".to_string(),
            source_url:
                "https://github.com/acme/source-binding-marketplace-path/tree/main/find-skills"
                    .to_string(),
            actor: "cli".to_string(),
        },
        &managed_root,
    )
    .unwrap();

    assert_eq!(preview.path, "skills/find-skills");
    assert_eq!(
        preview.source_url,
        "https://github.com/acme/source-binding-marketplace-path/tree/main/skills/find-skills"
    );
    assert_eq!(preview.validation, SourceBindingValidation::ExactMatch);
}

#[test]
fn source_binding_changed_source_does_not_switch_current() {
    let root = temp_dir("source-binding-changed");
    let managed_root = root.join("SkillBox");
    let paths = ensure_managed_layout(&managed_root).unwrap();
    let source = root.join("local").join("find-skills");
    make_skill(&source, "find-skills", "Find skills");
    let imported = import_skill(&source, SkillKind::Remote, &managed_root).unwrap();
    let before_current =
        fs::read_link(paths.remote_skills_root.join("find-skills").join("current")).unwrap();
    let remote = bare_remote_with_skill_content(
        "source-binding-changed-origin",
        "find-skills",
        "Find skills",
        "Updated body\n",
    );
    let _rewrite = github_repo_rewrite("acme", "source-binding-changed", &remote);
    let source_url = github_source_url("acme", "source-binding-changed", "find-skills");
    let preview = preview_remote_source_binding(
        RemoteSourceBindingRequest {
            skill_name: "find-skills".to_string(),
            source_url: source_url.clone(),
            actor: "desktop".to_string(),
        },
        &managed_root,
    )
    .unwrap();

    assert_eq!(
        preview.validation,
        SourceBindingValidation::SameSkillChanged
    );
    let result = bind_remote_source(
        BindRemoteSourceRequest {
            skill_name: "find-skills".to_string(),
            source_url,
            actor: "desktop".to_string(),
        },
        &managed_root,
    )
    .unwrap();

    let after_current =
        fs::read_link(paths.remote_skills_root.join("find-skills").join("current")).unwrap();
    assert_eq!(after_current, before_current);
    assert_eq!(result.validation, SourceBindingValidation::SameSkillChanged);
    assert!(result.source_path.exists());
    let source_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&result.source_path).unwrap()).unwrap();
    assert_eq!(source_json["type"], "github");
    assert_eq!(source_json["refKind"], "branch");
    assert_eq!(source_json["tracking"], true);
    assert_eq!(
        source_json["currentVersion"],
        before_current
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap()
    );
    let latest_sha = result.latest_sha.clone().unwrap();
    assert!(!paths
        .remote_skills_root
        .join("find-skills")
        .join("versions")
        .join(latest_sha)
        .exists());
    assert!(imported.managed_path.exists());
    let operations = list_operations(OperationFilter::default(), &managed_root).unwrap();
    assert!(operations
        .operations
        .iter()
        .any(|operation| operation.operation_type == "bind_remote_source"
            && operation.status == OperationStatus::Succeeded));
}

#[test]
fn source_binding_preview_rejects_name_mismatch() {
    let root = temp_dir("source-binding-mismatch");
    let managed_root = root.join("SkillBox");
    let source = root.join("local").join("alpha");
    make_skill(&source, "alpha", "Alpha skill");
    import_skill(&source, SkillKind::Remote, &managed_root).unwrap();
    let remote =
        bare_remote_with_skill_content("source-binding-mismatch-origin", "beta", "Beta skill", "");
    let _rewrite = github_repo_rewrite("acme", "source-binding-mismatch", &remote);

    let preview = preview_remote_source_binding(
        RemoteSourceBindingRequest {
            skill_name: "alpha".to_string(),
            source_url: github_source_url("acme", "source-binding-mismatch", "beta"),
            actor: "cli".to_string(),
        },
        &managed_root,
    )
    .unwrap();

    assert_eq!(preview.validation, SourceBindingValidation::Mismatch);
    assert!(preview
        .message
        .contains("Remote skill name beta does not match alpha"));

    let error = bind_remote_source(
        BindRemoteSourceRequest {
            skill_name: "alpha".to_string(),
            source_url: github_source_url("acme", "source-binding-mismatch", "beta"),
            actor: "cli".to_string(),
        },
        &managed_root,
    )
    .unwrap_err();
    assert!(error.contains("Remote skill name beta does not match alpha"));
    let operations = list_operations(OperationFilter::default(), &managed_root).unwrap();
    assert!(operations
        .operations
        .iter()
        .any(|operation| operation.operation_type == "bind_remote_source"
            && operation.status == OperationStatus::Failed));
}

#[test]
fn remote_version_list_marks_current() {
    let root = temp_dir("remote-version-list");
    let managed_root = root.join("SkillBox");
    let source = root.join("local").join("demo");
    make_skill(&source, "demo", "Demo skill");
    import_skill(&source, SkillKind::Remote, &managed_root).unwrap();

    let versions = list_remote_skill_versions("demo", &managed_root).unwrap();

    assert_eq!(versions.skill_name, "demo");
    assert_eq!(versions.versions.len(), 1);
    assert!(versions.versions[0].is_current);
    assert!(versions.versions[0].version.starts_with("manual-"));
    assert!(!versions.versions[0].updated_at.is_empty());
    assert!(versions.versions[0]
        .updated_at
        .chars()
        .all(|character| character.is_ascii_digit()));
}

#[test]
fn remote_version_preview_rollback_lists_every_changed_file() {
    let root = temp_dir("remote-preview-rollback");
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
    fs::write(v2_path.join("extra.txt"), "extra\n").unwrap();
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

    assert_eq!(preview.from_version, v2);
    assert_eq!(preview.to_version, v1);
    assert!(preview.files.iter().any(|file| file.path == "SKILL.md"));
    assert!(preview.files.iter().any(|file| file.path == "extra.txt"));
    assert!(preview
        .files
        .iter()
        .any(|file| file.path == "extra.txt" && file.diff.contains("-extra")));
}

#[test]
fn read_remote_source_rejects_untrusted_github_metadata() {
    let root = temp_dir("remote-source-validation");
    let remote_root = root.join("remote-skills").join("demo");

    write_remote_source_with_json(
        &remote_root,
        r#"{
              "type":"github",
              "repoUrl":"file:///tmp/repo.git",
              "ref":"main",
              "path":"skills/demo"
            }"#,
    );

    let error = read_remote_source(&remote_root).unwrap_err();
    assert!(error.contains("Only https://github.com remote URLs are supported"));

    write_remote_source_with_json(
        &remote_root,
        r#"{
              "type":"github",
              "repoUrl":"https://github.com/acme/repo.git",
              "ref":"main",
              "path":"skills/../../secret"
            }"#,
    );

    let error = read_remote_source(&remote_root).unwrap_err();
    assert!(error.contains("path must stay inside the repository"));

    write_remote_source_with_json(
        &remote_root,
        r#"{
              "type":"github",
              "repoUrl":"https://github.com/acme/repo.git",
              "ref":"main",
              "path":"skills/demo",
              "root":true
            }"#,
    );

    let error = read_remote_source(&remote_root).unwrap_err();
    assert!(error.contains("root source must not include a repository path"));
}

#[test]
fn read_remote_source_keeps_path_only_metadata_backward_compatible() {
    let root = temp_dir("remote-source-path-only-compatibility");
    let remote_root = root.join("remote-skills").join("demo");
    write_remote_source_with_json(
        &remote_root,
        r#"{
              "type":"github",
              "repoUrl":"https://github.com/acme/repo.git",
              "ref":"main",
              "path":"skills/demo"
            }"#,
    );

    let source = read_remote_source(&remote_root).unwrap();
    assert!(!source.root);
    assert_eq!(source.path.as_deref(), Some("skills/demo"));
}

#[test]
fn update_current_symlink_refuses_existing_non_symlink() {
    let root = temp_dir("current-non-symlink");
    let remote_root = root.join("remote");
    let version = remote_root.join("versions").join("v1");
    fs::create_dir_all(&version).unwrap();
    fs::create_dir_all(&remote_root).unwrap();
    fs::write(remote_root.join("current"), "not a symlink").unwrap();

    let error = update_current_symlink(&remote_root, &version).unwrap_err();

    assert!(error.contains("Refusing to replace existing non-symlink current"));
    assert_eq!(
        fs::read_to_string(remote_root.join("current")).unwrap(),
        "not a symlink"
    );
}

#[test]
fn copy_skill_dir_rejects_symlinks_that_escape_source_root() {
    let root = temp_dir("copy-symlink-escape");
    let source = root.join("source");
    let outside = root.join("outside");
    let destination = root.join("destination");
    make_skill(&source, "demo", "Demo skill");
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("secret.txt"), "secret").unwrap();
    symlink_any(&outside.join("secret.txt"), &source.join("secret-link")).unwrap();

    let error = copy_skill_dir(&source, &destination).unwrap_err();

    assert!(error.contains("Refusing to copy symlink outside source root"));
    assert!(!destination.exists());
}

#[test]
fn copy_skill_dir_preserves_internal_broken_symlink() {
    let root = temp_dir("copy-broken-symlink");
    let source = root.join("source");
    let destination = root.join("destination");
    make_skill(&source, "demo", "Demo skill");
    symlink_any(Path::new("missing.txt"), &source.join("missing-link")).unwrap();

    copy_skill_dir(&source, &destination).unwrap();

    assert!(fs::symlink_metadata(destination.join("missing-link"))
        .unwrap()
        .file_type()
        .is_symlink());
    assert_eq!(
        fs::read_link(destination.join("missing-link")).unwrap(),
        PathBuf::from("missing.txt")
    );
}

#[test]
fn remote_version_preview_keeps_binary_file_metadata() {
    let root = temp_dir("remote-preview-binary");
    let managed_root = root.join("SkillBox");
    let paths = ensure_managed_layout(&managed_root).unwrap();
    let source_v1 = root.join("local-v1").join("demo");
    make_skill(&source_v1, "demo", "Demo skill");
    import_skill(&source_v1, SkillKind::Remote, &managed_root).unwrap();
    let v1 = current_remote_version(&paths, "demo").unwrap();
    let remote_root = paths.remote_skills_root.join("demo");
    let v2 = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let v2_path = remote_root.join("versions").join(v2);
    copy_skill_dir(&source_v1, &v2_path).unwrap();
    fs::write(v2_path.join("asset.bin"), [0xff, 0x00, 0x10]).unwrap();
    update_current_symlink(&remote_root, &v2_path).unwrap();

    let preview = preview_remote_version_change(
        RemoteVersionChangeRequest {
            skill_name: "demo".to_string(),
            action: RemoteVersionChangeAction::Rollback,
            target_version: Some(v1),
            actor: "cli".to_string(),
        },
        &managed_root,
    )
    .unwrap();

    let binary = preview
        .files
        .iter()
        .find(|file| file.path == "asset.bin")
        .unwrap();
    assert!(binary.binary);
    assert_eq!(binary.old_size, Some(3));
    assert!(binary.old_hash.is_some());
    assert_eq!(binary.diff, "");
}

#[test]
fn remote_diff_file_inlines_text_under_one_megabyte() {
    let root = temp_dir("remote-diff-large-text");
    let old_root = root.join("old");
    let new_root = root.join("new");
    fs::create_dir_all(&old_root).unwrap();
    fs::create_dir_all(&new_root).unwrap();
    let content = "large text line\n".repeat(9_000);
    fs::write(new_root.join("SKILL.md"), &content).unwrap();

    let diff_file = remote_diff_file(
        &old_root,
        &new_root,
        skillbox_git::GitDiffFile {
            path: "SKILL.md".to_string(),
            old_path: None,
            status: "A".to_string(),
            diff: "@@\n+large text line\n".to_string(),
        },
    )
    .unwrap();

    assert!(!diff_file.too_large);
    assert_eq!(diff_file.diff, "@@\n+large text line\n");
    assert_eq!(diff_file.new_size, Some(content.len() as u64));
}

#[test]
fn remote_diff_file_handles_directory_paths_without_file_metadata() {
    let root = temp_dir("remote-diff-directory");
    let old_root = root.join("old");
    let new_root = root.join("new");
    fs::create_dir_all(old_root.join("assets")).unwrap();
    fs::create_dir_all(&new_root).unwrap();

    let diff_file = remote_diff_file(
        &old_root,
        &new_root,
        skillbox_git::GitDiffFile {
            path: "assets".to_string(),
            old_path: None,
            status: "D".to_string(),
            diff: String::new(),
        },
    )
    .unwrap();

    assert_eq!(diff_file.path, "assets");
    assert_eq!(diff_file.label, "Deleted");
    assert_eq!(diff_file.old_hash, None);
    assert_eq!(diff_file.new_hash, None);
    assert_eq!(diff_file.old_size, None);
    assert_eq!(diff_file.new_size, None);
    assert!(!diff_file.binary);
    assert!(!diff_file.too_large);
}

#[test]
fn remote_version_preview_update_uses_temp_snapshot_without_installing_version() {
    let root = temp_dir("remote-preview-update");
    let managed_root = root.join("SkillBox");
    let paths = ensure_managed_layout(&managed_root).unwrap();
    let source = root.join("local").join("find-skills");
    make_skill(&source, "find-skills", "Find skills");
    import_skill(&source, SkillKind::Remote, &managed_root).unwrap();
    let remote = bare_remote_with_skill_content(
        "remote-preview-update-origin",
        "find-skills",
        "Find skills",
        "Updated remote body\n",
    );
    let _rewrite = github_repo_rewrite("acme", "remote-preview-update", &remote);
    bind_remote_source(
        BindRemoteSourceRequest {
            skill_name: "find-skills".to_string(),
            source_url: github_source_url("acme", "remote-preview-update", "find-skills"),
            actor: "cli".to_string(),
        },
        &managed_root,
    )
    .unwrap();
    let latest_sha = read_remote_source(&paths.remote_skills_root.join("find-skills"))
        .unwrap()
        .latest_sha
        .unwrap();

    let preview = preview_remote_version_change(
        RemoteVersionChangeRequest {
            skill_name: "find-skills".to_string(),
            action: RemoteVersionChangeAction::Update,
            target_version: None,
            actor: "cli".to_string(),
        },
        &managed_root,
    )
    .unwrap();

    assert_eq!(preview.to_version, latest_sha);
    assert!(preview.files.iter().any(|file| file.path == "SKILL.md"));
    assert!(!paths
        .remote_skills_root
        .join("find-skills")
        .join("versions")
        .join(&preview.to_version)
        .exists());
}

#[test]
fn remote_version_preview_update_honors_explicit_target_version() {
    let root = temp_dir("remote-preview-update-explicit-target");
    let managed_root = root.join("SkillBox");
    let paths = ensure_managed_layout(&managed_root).unwrap();
    let source = root.join("local").join("find-skills");
    make_skill(&source, "find-skills", "Find skills");
    import_skill(&source, SkillKind::Remote, &managed_root).unwrap();
    let current_version = current_remote_version(&paths, "find-skills").unwrap();
    let remote_root = paths.remote_skills_root.join("find-skills");
    let target_version = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    let target_path = remote_root.join("versions").join(target_version);
    copy_skill_dir(&source, &target_path).unwrap();
    fs::write(
        target_path.join("SKILL.md"),
        "---\nname: find-skills\ndescription: Find skills\n---\nUpdated body\n",
    )
    .unwrap();
    write_remote_source_with_json(
        &remote_root,
        &format!(
            r#"{{
                  "type":"github",
                  "currentVersion":"{current_version}",
                  "latestSha":"{current_version}"
                }}"#
        ),
    );

    let preview = preview_remote_version_change(
        RemoteVersionChangeRequest {
            skill_name: "find-skills".to_string(),
            action: RemoteVersionChangeAction::Update,
            target_version: Some(target_version.to_string()),
            actor: "cli".to_string(),
        },
        &managed_root,
    )
    .unwrap();

    assert_eq!(preview.from_version, current_version);
    assert_eq!(preview.to_version, target_version);
    assert!(preview.files.iter().any(|file| file.path == "SKILL.md"));
}
