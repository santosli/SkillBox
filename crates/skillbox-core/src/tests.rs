use super::*;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn parses_basic_skill_frontmatter() {
    let metadata = parse_skill_frontmatter(
        "---
name: demo
version: 0.1.0
description: \"Demo skill\"
---

# Demo
",
    );

    assert_eq!(metadata.name, "demo");
    assert_eq!(metadata.version, "0.1.0");
    assert_eq!(metadata.description, "Demo skill");
}

#[test]
fn parses_folded_skill_description_frontmatter() {
    let metadata = parse_skill_frontmatter(
        "---
name: interview-evaluation
description: >
  Interview evaluation workflow for reviewing
  candidate answers and generating feedback.
version: 0.1.0
---

# Interview evaluation
",
    );

    assert_eq!(metadata.name, "interview-evaluation");
    assert_eq!(
        metadata.description,
        "Interview evaluation workflow for reviewing candidate answers and generating feedback."
    );
    assert_eq!(metadata.version, "0.1.0");
}

#[test]
fn database_initialization_configures_busy_timeout_and_wal() {
    let source = include_str!("db.rs");

    assert!(source.contains("PRAGMA busy_timeout = 5000"));
    assert!(source.contains("PRAGMA journal_mode = WAL"));
}

#[test]
fn legacy_node_sqlite_schema_migrates_operations_and_remains_writable() {
    let root = temp_dir("legacy-node-sqlite");
    let managed_root = root.join("SkillBox");
    let paths = managed_paths(&managed_root);
    fs::create_dir_all(paths.database_path.parent().unwrap()).unwrap();
    let connection = rusqlite::Connection::open(&paths.database_path).unwrap();
    connection
        .execute_batch(
            "
            CREATE TABLE skills (
              name TEXT PRIMARY KEY,
              type TEXT NOT NULL,
              description TEXT NOT NULL DEFAULT '',
              version TEXT NOT NULL DEFAULT '',
              managed_path TEXT NOT NULL,
              status TEXT NOT NULL DEFAULT 'ok',
              content_hash TEXT NOT NULL DEFAULT '',
              source_json TEXT NOT NULL DEFAULT '{}',
              updated_at TEXT NOT NULL
            );

            CREATE TABLE deployments (
              skill_name TEXT NOT NULL,
              target_root TEXT NOT NULL,
              target_path TEXT NOT NULL,
              mode TEXT NOT NULL,
              updated_at TEXT NOT NULL,
              PRIMARY KEY (skill_name, target_root)
            );

            CREATE TABLE operations (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              type TEXT NOT NULL,
              skill_name TEXT,
              status TEXT NOT NULL,
              message TEXT NOT NULL DEFAULT '',
              created_at TEXT NOT NULL
            );

            INSERT INTO operations (type, skill_name, status, message, created_at)
            VALUES ('install', 'demo', 'ok', 'Installed demo', '2026-06-10T00:00:00Z');
            ",
        )
        .unwrap();
    drop(connection);

    ensure_managed_layout(&managed_root).unwrap();
    let operations = list_operations(OperationFilter::default(), &managed_root).unwrap();
    let legacy = operations
        .operations
        .iter()
        .find(|operation| operation.id == "legacy-node-1")
        .unwrap();
    assert_eq!(legacy.operation_type, "install");
    assert_eq!(legacy.status, OperationStatus::Succeeded);
    assert_eq!(legacy.actor, "legacy-node");
    assert_eq!(legacy.entity_type, "skill");
    assert_eq!(legacy.entity_name, "demo");
    assert_eq!(legacy.summary, "Installed demo");

    let source = root.join("source").join("new-skill");
    make_skill(&source, "new-skill", "New skill");
    let imported = import_skill(&source, SkillKind::User, &managed_root).unwrap();
    assert_eq!(imported.name, "new-skill");
    let deployment = deploy_skill("new-skill", &managed_root, root.join("runtime")).unwrap();
    assert!(fs::symlink_metadata(deployment.target_path)
        .unwrap()
        .file_type()
        .is_symlink());

    let operation = start_operation(
        OperationStart {
            operation_type: "test_operation".to_string(),
            actor: "test".to_string(),
            entity_type: "skill".to_string(),
            entity_name: "new-skill".to_string(),
            summary: "Test operation".to_string(),
            payload: serde_json::json!({}),
        },
        &managed_root,
    )
    .unwrap();
    assert!(operation.id.starts_with("op-"));
}

#[test]
fn unique_backup_path_uses_bounded_suffix_search() {
    let source = include_str!("import.rs");
    let start = source.find("fn unique_backup_path").unwrap();
    let end = start + source[start..].find("fn is_under_path").unwrap();
    let function_source = &source[start..end];

    assert!(!function_source.contains("for index in 2.. {"));
    assert!(!function_source.contains("unreachable!(\"backup suffix loop is unbounded\")"));
}

#[test]
fn scans_nested_skill_directories() {
    let root = temp_dir("scan");
    make_skill(&root.join("alpha"), "alpha", "Alpha skill");
    make_skill(&root.join("group").join("beta"), "beta", "Beta skill");

    let scan = scan_skill_roots(std::slice::from_ref(&root)).unwrap();

    assert_eq!(scan.errors.len(), 0);
    let names: Vec<_> = scan
        .skills
        .iter()
        .map(|skill| skill.name.as_str())
        .collect();
    assert_eq!(names, vec!["alpha", "beta"]);
}

#[test]
fn scan_skill_roots_does_not_follow_symlinked_directories() {
    let root = temp_dir("scan-symlink-root");
    let outside = temp_dir("scan-symlink-outside");
    make_skill(&outside.join("leaked"), "leaked", "Leaked skill");
    symlink_dir(&outside, &root.join("linked")).unwrap();

    let scan = scan_skill_roots(&[root]).unwrap();

    assert_eq!(scan.errors.len(), 0);
    assert!(scan.skills.is_empty());
}

#[test]
fn global_runtime_roots_include_project_local_skill_roots() {
    let root = temp_dir("global-runtime-roots");
    let project_agents_root = root
        .join("Library")
        .join("Mobile Documents")
        .join("iCloud~md~obsidian")
        .join("Documents")
        .join("demo-vault")
        .join(".agents")
        .join("skills");
    let project_codex_root = root
        .join("zone")
        .join("project")
        .join(".codex")
        .join("skills");
    let global_claude_root = root.join(".claude").join("skills");
    let project_claude_root = root
        .join("Documents")
        .join("project")
        .join(".claude")
        .join("skills");

    make_skill(
        &project_agents_root.join("demo-local"),
        "demo-local",
        "demo-vault local skill",
    );
    make_skill(
        &project_codex_root.join("project-remote"),
        "project-remote",
        "Project remote skill",
    );
    make_skill(
        &global_claude_root.join("claude-global"),
        "claude-global",
        "Claude global skill",
    );
    make_skill(
        &project_claude_root.join("claude-project"),
        "claude-project",
        "Claude project skill",
    );

    let roots = runtime_roots_under(&root);

    assert!(roots.contains(&root.join(".codex").join("skills")));
    assert!(roots.contains(&root.join(".agents").join("skills")));
    assert!(roots.contains(&global_claude_root));
    assert!(roots.contains(&project_agents_root));
    assert!(roots.contains(&project_codex_root));
    assert!(roots.contains(&project_claude_root));
}

#[test]
fn default_managed_root_uses_hidden_skillbox_directory() {
    let previous = std::env::var_os("SKILLBOX_HOME");
    std::env::remove_var("SKILLBOX_HOME");

    let root = default_managed_root();

    match previous {
        Some(value) => std::env::set_var("SKILLBOX_HOME", value),
        None => std::env::remove_var("SKILLBOX_HOME"),
    }
    assert_eq!(
        root.file_name().and_then(|name| name.to_str()),
        Some(".skillbox")
    );
}

#[test]
fn ensure_managed_layout_writes_default_user_skills_gitignore() {
    let managed_root = temp_dir("managed-layout-gitignore").join("SkillBox");

    let paths = ensure_managed_layout(&managed_root).unwrap();
    let gitignore = fs::read_to_string(paths.user_skills_root.join(".gitignore")).unwrap();

    assert!(gitignore.contains(".DS_Store"));
    assert!(gitignore.contains("__pycache__/"));
    assert!(gitignore.contains("*.py[cod]"));
    assert!(gitignore.contains("node_modules/"));
    assert!(gitignore.contains(".env"));
    assert!(gitignore.contains("!.env.example"));
}

#[test]
fn ensure_managed_layout_preserves_existing_user_skills_gitignore() {
    let managed_root = temp_dir("managed-layout-preserve-gitignore").join("SkillBox");
    let user_skills_root = managed_root.join("user-skills");
    fs::create_dir_all(&user_skills_root).unwrap();
    fs::write(user_skills_root.join(".gitignore"), "custom-ignore\n").unwrap();

    let paths = ensure_managed_layout(&managed_root).unwrap();
    let gitignore = fs::read_to_string(paths.user_skills_root.join(".gitignore")).unwrap();

    assert_eq!(gitignore, "custom-ignore\n");
}

#[test]
fn legacy_managed_root_is_linked_when_hidden_root_is_empty_stub() {
    let root = temp_dir("legacy-managed-root-link");
    let hidden_root = root.join(".skillbox");
    let legacy_root = root.join("SkillBox");
    fs::create_dir_all(hidden_root.join("user-skills")).unwrap();
    fs::create_dir_all(hidden_root.join("remote-skills")).unwrap();
    fs::write(hidden_root.join("skillbox.sqlite"), "").unwrap();
    make_skill(
        &legacy_root.join("user-skills").join("demo"),
        "demo",
        "Legacy demo",
    );

    let migrated = link_legacy_managed_root_if_needed(&hidden_root, &legacy_root).unwrap();
    let paths = ensure_managed_layout(&hidden_root).unwrap();
    let state = managed_state(&hidden_root).unwrap();

    assert!(migrated);
    assert_eq!(paths.root, hidden_root);
    assert!(fs::symlink_metadata(&hidden_root)
        .unwrap()
        .file_type()
        .is_symlink());
    assert_eq!(fs::read_link(&hidden_root).unwrap(), legacy_root);
    assert_eq!(state.skills.len(), 1);
    assert_eq!(state.skills[0].name, "demo");
}

#[test]
fn list_workspaces_initializes_empty_registry() {
    let managed_root = temp_dir("workspace-empty").join("SkillBox");

    let workspaces = list_workspaces(&managed_root).unwrap();

    assert!(workspaces.is_empty());
}

#[test]
fn add_workspace_rejects_missing_directory() {
    let root = temp_dir("workspace-missing");
    let managed_root = root.join("SkillBox");

    let error = add_workspace(
        WorkspaceAddRequest {
            path: root.join("missing").join("skills"),
            kind: WorkspaceKind::User,
        },
        &managed_root,
    )
    .unwrap_err();

    assert!(error.contains("Workspace path does not exist"));
}

#[test]
fn add_workspace_scans_existing_root_and_dedupes_by_canonical_path() {
    let root = temp_dir("workspace-add");
    let managed_root = root.join("SkillBox");
    let workspace_root = root.join("project").join(".agents").join("skills");
    make_skill(&workspace_root.join("alpha"), "alpha", "Alpha skill");

    let first = add_workspace(
        WorkspaceAddRequest {
            path: workspace_root.clone(),
            kind: WorkspaceKind::User,
        },
        &managed_root,
    )
    .unwrap();
    let second = add_workspace(
        WorkspaceAddRequest {
            path: workspace_root.join("."),
            kind: WorkspaceKind::User,
        },
        &managed_root,
    )
    .unwrap();
    let workspaces = list_workspaces(&managed_root).unwrap();

    assert_eq!(first.skill_count, 1);
    assert_eq!(first.last_scan_error_count, 0);
    assert_eq!(first.kind, WorkspaceKind::User);
    assert_eq!(first.source, WorkspaceSource::Manual);
    assert_eq!(first.agent_id.as_deref(), Some("agents"));
    assert_eq!(first.display_name, "project");
    assert_eq!(second.canonical_path, first.canonical_path);
    assert_eq!(workspaces.len(), 1);
}

#[test]
fn add_workspace_counts_imported_skills() {
    let root = temp_dir("workspace-imported-count");
    let managed_root = root.join("SkillBox");
    let workspace_root = root.join("project").join(".agents").join("skills");
    let imported_source = workspace_root.join("alpha");
    make_skill(&imported_source, "alpha", "Alpha skill");
    make_skill(&workspace_root.join("beta"), "beta", "Beta skill");
    import_skill(&imported_source, SkillKind::User, &managed_root).unwrap();

    let workspace = add_workspace(
        WorkspaceAddRequest {
            path: workspace_root,
            kind: WorkspaceKind::User,
        },
        &managed_root,
    )
    .unwrap();

    assert_eq!(workspace.skill_count, 2);
    assert_eq!(workspace.imported_skill_count, 1);
}

#[test]
fn add_workspace_counts_deployed_symlinked_skills() {
    let root = temp_dir("workspace-deployed-count");
    let managed_root = root.join("SkillBox");
    let workspace_root = root.join("project").join(".agents").join("skills");
    let source = workspace_root.join("alpha");
    make_skill(&source, "alpha", "Alpha skill");

    import_candidates(
        vec![ImportRequestItem {
            source_path: source.clone(),
            skill_type: SkillKind::User,
            deploy_back_to_source: true,
        }],
        &managed_root,
    )
    .unwrap();

    let workspace = add_workspace(
        WorkspaceAddRequest {
            path: workspace_root,
            kind: WorkspaceKind::User,
        },
        &managed_root,
    )
    .unwrap();

    assert_eq!(workspace.skill_count, 1);
    assert_eq!(workspace.imported_skill_count, 1);
}

#[test]
fn record_skill_usage_allows_unmanaged_skill_and_dedupes_event_ids() {
    let root = temp_dir("usage-unmanaged-dedupe");
    let managed_root = root.join("SkillBox");
    let runtime_root = root.join("project").join(".codex").join("skills");
    fs::create_dir_all(&runtime_root).unwrap();

    let first = record_skill_usage(
        RecordSkillUsageRequest {
            skill_name: "draft-helper".to_string(),
            agent_id: "codex".to_string(),
            runtime_root: runtime_root.clone(),
            event_id: Some("codex-run-1".to_string()),
            used_at: Some("2026-06-02T10:15:00Z".to_string()),
            prompt_excerpt: None,
            metadata: Some(serde_json::json!({ "source": "codex-app" })),
        },
        &managed_root,
    )
    .unwrap();
    let second = record_skill_usage(
        RecordSkillUsageRequest {
            skill_name: "draft-helper".to_string(),
            agent_id: "codex".to_string(),
            runtime_root: runtime_root.clone(),
            event_id: Some("codex-run-1".to_string()),
            used_at: Some("2026-06-02T10:16:00Z".to_string()),
            prompt_excerpt: Some("Second prompt should backfill the existing event".to_string()),
            metadata: Some(serde_json::json!({ "source": "codex-app" })),
        },
        &managed_root,
    )
    .unwrap();

    assert_eq!(first.usage_count, 1);
    assert!(!first.deduplicated);
    assert_eq!(first.used_at, "2026-06-02T10:15:00+00:00");
    assert_eq!(second.usage_count, 1);
    assert!(second.deduplicated);
    assert_eq!(second.last_used_at, "2026-06-02T10:15:00+00:00");

    let history = list_history(HistoryFilter::default(), &managed_root).unwrap();
    assert_eq!(
        history.entries[0].prompt_excerpt.as_deref(),
        Some("Second prompt should backfill the existing event")
    );
}

#[test]
fn managed_state_includes_skill_usage_summary() {
    let root = temp_dir("usage-managed-state");
    let managed_root = root.join("SkillBox");
    let source = root.join("runtime").join("alpha");
    let codex_runtime = root.join(".codex").join("skills");
    let agents_runtime = root.join(".agents").join("skills");
    make_skill(&source, "alpha", "Alpha skill");
    import_skill(&source, SkillKind::User, &managed_root).unwrap();

    record_skill_usage(
        RecordSkillUsageRequest {
            skill_name: "alpha".to_string(),
            agent_id: "codex".to_string(),
            runtime_root: codex_runtime,
            event_id: None,
            used_at: Some("2026-06-02T09:00:00Z".to_string()),
            prompt_excerpt: None,
            metadata: None,
        },
        &managed_root,
    )
    .unwrap();
    record_skill_usage(
        RecordSkillUsageRequest {
            skill_name: "alpha".to_string(),
            agent_id: "agents".to_string(),
            runtime_root: agents_runtime,
            event_id: None,
            used_at: Some("2026-06-02T11:00:00Z".to_string()),
            prompt_excerpt: None,
            metadata: None,
        },
        &managed_root,
    )
    .unwrap();

    let state = managed_state(&managed_root).unwrap();

    assert_eq!(state.skills[0].name, "alpha");
    assert_eq!(state.skills[0].usage_count, 2);
    assert_eq!(
        state.skills[0].last_used_at.as_deref(),
        Some("2026-06-02T11:00:00+00:00")
    );
}

#[test]
fn workspace_and_import_candidates_include_usage_counts() {
    let root = temp_dir("usage-workspace-candidates");
    let managed_root = root.join("SkillBox");
    let workspace_root = root.join("project").join(".agents").join("skills");
    make_skill(&workspace_root.join("alpha"), "alpha", "Alpha skill");

    record_skill_usage(
        RecordSkillUsageRequest {
            skill_name: "alpha".to_string(),
            agent_id: "agents".to_string(),
            runtime_root: workspace_root.clone(),
            event_id: None,
            used_at: Some("2026-06-02T12:00:00Z".to_string()),
            prompt_excerpt: None,
            metadata: None,
        },
        &managed_root,
    )
    .unwrap();
    record_skill_usage(
        RecordSkillUsageRequest {
            skill_name: "alpha".to_string(),
            agent_id: "agents".to_string(),
            runtime_root: workspace_root.clone(),
            event_id: None,
            used_at: Some("2026-06-02T12:01:00Z".to_string()),
            prompt_excerpt: None,
            metadata: None,
        },
        &managed_root,
    )
    .unwrap();

    let candidates =
        scan_import_candidates(std::slice::from_ref(&workspace_root), &managed_root).unwrap();
    let workspaces = list_workspaces(&managed_root).unwrap();

    assert_eq!(workspace(&workspaces, &workspace_root).usage_count, 2);
    assert_eq!(candidate(&candidates.candidates, "alpha").usage_count, 2);
}

#[test]
fn record_skill_usage_rejects_content_metadata() {
    let root = temp_dir("usage-metadata-content");
    let managed_root = root.join("SkillBox");
    let runtime_root = root.join(".codex").join("skills");

    let error = record_skill_usage(
        RecordSkillUsageRequest {
            skill_name: "alpha".to_string(),
            agent_id: "codex".to_string(),
            runtime_root,
            event_id: None,
            used_at: Some("2026-06-02T12:00:00Z".to_string()),
            prompt_excerpt: None,
            metadata: Some(serde_json::json!({ "prompt": "private request" })),
        },
        &managed_root,
    )
    .unwrap_err();

    assert!(error.contains("metadata"));
    assert!(error.contains("prompt"));
}

#[test]
fn usage_hook_install_injects_codex_and_claude_stop_hooks() {
    let root = temp_dir("usage-hook-install");
    let home = root.join("home");
    fs::create_dir_all(home.join(".codex")).unwrap();
    fs::write(
        home.join(".codex").join("hooks.json"),
        r#"{"hooks":{"Stop":[{"hooks":[{"type":"command","command":"echo existing"}]}]}}"#,
    )
    .unwrap();
    fs::create_dir_all(home.join(".claude")).unwrap();
    fs::write(
        home.join(".claude").join("settings.json"),
        r#"{"permissions":{"allow":["Read"]}}"#,
    )
    .unwrap();

    let codex = install_usage_hook_for_home(UsageHookTarget::CodexApp, &home).unwrap();
    let claude = install_usage_hook_for_home(UsageHookTarget::ClaudeCodeCli, &home).unwrap();

    assert!(codex.installed);
    assert!(claude.installed);
    assert!(codex.backup_path.is_some());
    assert!(claude.backup_path.is_some());

    let codex_config: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(home.join(".codex/hooks.json")).unwrap()).unwrap();
    let claude_config: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(home.join(".claude/settings.json")).unwrap())
            .unwrap();

    assert_eq!(
        codex_config["hooks"]["Stop"][0]["hooks"][0]["command"],
        "echo existing"
    );
    assert!(json_has_hook_command(&codex_config, &codex.status.command));
    assert!(json_has_hook_command(
        &claude_config,
        &claude.status.command
    ));
    assert_eq!(claude_config["permissions"]["allow"][0], "Read");

    let statuses = usage_hook_statuses_for_home(&home).unwrap();
    let codex_app_status = statuses
        .iter()
        .find(|status| status.target == UsageHookTarget::CodexApp)
        .unwrap();
    let codex_cli_status = statuses
        .iter()
        .find(|status| status.target == UsageHookTarget::CodexCli)
        .unwrap();
    let claude_status = statuses
        .iter()
        .find(|status| status.target == UsageHookTarget::ClaudeCodeCli)
        .unwrap();

    assert!(codex_app_status.installed);
    assert!(codex_app_status.trust_required);
    assert!(codex_app_status
        .activation_note
        .as_ref()
        .unwrap()
        .contains("/hooks"));
    assert!(codex_cli_status.installed);
    assert!(codex_cli_status.trust_required);
    assert!(codex_cli_status
        .activation_note
        .as_ref()
        .unwrap()
        .contains("/hooks"));
    assert!(claude_status.installed);
    assert!(!claude_status.trust_required);
    assert!(claude_status.activation_note.is_none());
}

#[test]
fn usage_hook_status_marks_codex_trusted_after_hook_records_usage() {
    let root = temp_dir("usage-hook-trusted-after-record");
    let home = root.join("home");
    fs::create_dir_all(home.join(".codex")).unwrap();
    fs::write(home.join(".codex").join("hooks.json"), r#"{"hooks":{}}"#).unwrap();

    install_usage_hook_for_home(UsageHookTarget::CodexApp, &home).unwrap();
    let statuses = usage_hook_statuses_for_home(&home).unwrap();
    let codex_status = statuses
        .iter()
        .find(|status| status.target == UsageHookTarget::CodexApp)
        .unwrap();
    assert!(codex_status.trust_required);

    record_skill_usage(
        RecordSkillUsageRequest {
            skill_name: "frontend-design".to_string(),
            agent_id: "codex".to_string(),
            runtime_root: home.join(".codex/skills"),
            event_id: Some("hook-event-1".to_string()),
            used_at: Some("2026-06-04T00:00:00Z".to_string()),
            prompt_excerpt: None,
            metadata: Some(serde_json::json!({
                "source": "agent_hook",
                "hook_agent": "codex"
            })),
        },
        home.join(".skillbox"),
    )
    .unwrap();

    let statuses = usage_hook_statuses_for_home(&home).unwrap();
    let codex_status = statuses
        .iter()
        .find(|status| status.target == UsageHookTarget::CodexApp)
        .unwrap();
    assert!(codex_status.installed);
    assert!(!codex_status.trust_required);
    assert!(codex_status.activation_note.is_none());
}

#[test]
fn usage_hook_command_uses_stable_wrapper_path() {
    let root = temp_dir("usage-hook-command-wrapper-path");
    let home = root.join("home");

    assert_eq!(
        usage_hook_command_for_home(UsageHookTarget::CodexApp, &home),
        format!(
            "{} codex",
            shell_quote_path(&home.join(".skillbox/bin/skillbox-usage-hook"))
        )
    );
}

#[test]
fn usage_hook_install_replaces_legacy_bare_command() {
    let root = temp_dir("usage-hook-replace-legacy");
    let home = root.join("home");
    fs::create_dir_all(home.join(".codex")).unwrap();
    fs::write(
            home.join(".codex").join("hooks.json"),
            r#"{"hooks":{"Stop":[{"hooks":[{"type":"command","command":"skillbox usage-hook codex"}]}]}}"#,
        )
        .unwrap();

    let result = install_usage_hook_for_home(UsageHookTarget::CodexApp, &home).unwrap();
    let codex_config: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(home.join(".codex/hooks.json")).unwrap()).unwrap();

    assert!(result.installed);
    assert!(result.backup_path.is_some());
    assert!(home.join(".skillbox/bin/skillbox-usage-hook").is_file());
    assert!(home
        .join(".skillbox/bin/skillbox-usage-hook-runner")
        .is_file());
    assert!(!result.status.command.contains("target/debug"));
    assert!(!json_has_hook_command(
        &codex_config,
        "skillbox usage-hook codex"
    ));
    assert!(json_has_hook_command(&codex_config, &result.status.command));
}

#[test]
fn usage_hook_install_replaces_development_absolute_command() {
    let root = temp_dir("usage-hook-replace-dev-command");
    let home = root.join("home");
    let old_command = "'/Users/example/zone/skill-box/target/debug/skillbox-cli' usage-hook codex";
    fs::create_dir_all(home.join(".codex")).unwrap();
    fs::write(
        home.join(".codex").join("hooks.json"),
        serde_json::json!({
            "hooks": {
                "Stop": [{
                    "hooks": [{
                        "type": "command",
                        "command": old_command
                    }]
                }]
            }
        })
        .to_string(),
    )
    .unwrap();

    let result = install_usage_hook_for_home(UsageHookTarget::CodexApp, &home).unwrap();
    let wrapper = fs::read_to_string(home.join(".skillbox/bin/skillbox-usage-hook")).unwrap();
    let codex_config: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(home.join(".codex/hooks.json")).unwrap()).unwrap();

    assert!(result.installed);
    assert!(!result.status.command.contains("target/debug"));
    assert!(!wrapper.contains("target/debug"));
    assert!(!json_has_hook_command(&codex_config, old_command));
    assert!(json_has_hook_command(&codex_config, &result.status.command));
}

#[test]
fn usage_hook_records_skill_blocks_from_codex_transcript() {
    let root = temp_dir("usage-hook-codex-record");
    let managed_root = root.join("SkillBox");
    let runtime_root = root.join("project").join(".agents").join("skills");
    let skill_root = runtime_root.join("probe");
    fs::create_dir_all(&skill_root).unwrap();
    fs::write(
        skill_root.join("SKILL.md"),
        "---\nname: probe\ndescription: Probe\n---\n",
    )
    .unwrap();
    let transcript = root.join("codex.jsonl");
    fs::write(
            &transcript,
            format!(
                "{}\n{}\n{}\n",
                serde_json::json!({
                    "type": "turn_context",
                    "payload": { "turn_id": "turn-1" }
                }),
                serde_json::json!({
                    "type": "response_item",
                    "payload": {
                        "type": "message",
                        "role": "user",
                        "content": [{
                            "type": "input_text",
                            "text": format!(
                                "Please use probe to review the draft plan.\n<skill>\n<name>probe</name>\n<path>{}</path>\n---\nname: probe\n---\n</skill>",
                                skill_root.join("SKILL.md").display()
                            )
                        }]
                    }
                }),
                serde_json::json!({
                    "type": "turn_context",
                    "payload": { "turn_id": "turn-2" }
                })
            ),
        )
        .unwrap();
    let hook_input = serde_json::json!({
        "session_id": "session-1",
        "turn_id": "turn-1",
        "transcript_path": transcript,
        "cwd": root.join("project"),
        "hook_event_name": "Stop",
        "model": "gpt-test"
    })
    .to_string();

    let first = record_skill_usage_from_hook("codex", &hook_input, &managed_root).unwrap();
    let second = record_skill_usage_from_hook("codex", &hook_input, &managed_root).unwrap();

    assert_eq!(first.recorded.len(), 1);
    assert_eq!(first.recorded[0].skill_name, "probe");
    assert_eq!(first.recorded[0].agent_id, "agents");
    assert_eq!(
        first.recorded[0].runtime_root,
        fs::canonicalize(runtime_root).unwrap()
    );
    assert!(!first.recorded[0].deduplicated);
    assert_eq!(second.recorded.len(), 1);
    assert!(second.recorded[0].deduplicated);
}

#[test]
fn usage_hook_records_codex_desktop_task_complete_turns() {
    let root = temp_dir("usage-hook-codex-desktop-record");
    let managed_root = root.join("SkillBox");
    let runtime_root = root.join("project").join(".codex").join("skills");
    let skill_root = runtime_root.join("probe");
    fs::create_dir_all(&skill_root).unwrap();
    fs::write(
        skill_root.join("SKILL.md"),
        "---\nname: probe\ndescription: Probe\n---\n",
    )
    .unwrap();
    let transcript = root.join("codex-desktop.jsonl");
    fs::write(
            &transcript,
            format!(
                "{}\n{}\n{}\n{}\n{}\n{}\n{}\n",
                serde_json::json!({
                    "type": "session_meta",
                    "payload": { "id": "session-1" }
                }),
                serde_json::json!({
                    "type": "response_item",
                    "payload": {
                        "type": "message",
                        "role": "user",
                        "content": [{ "type": "input_text", "text": "first turn" }]
                    }
                }),
                serde_json::json!({
                    "type": "event_msg",
                    "payload": {
                        "type": "task_complete",
                        "turn_id": "turn-1"
                    }
                }),
                serde_json::json!({
                    "type": "event_msg",
                    "payload": {
                        "type": "user_message",
                        "message": format!(
                            "[$probe]({}) Review this plan",
                            skill_root.join("SKILL.md").display()
                        )
                    }
                }),
                serde_json::json!({
                    "type": "response_item",
                    "payload": {
                        "type": "message",
                        "role": "user",
                        "content": [{
                            "type": "input_text",
                            "text": format!(
                                "<skill>\n<name>probe</name>\n<path>{}</path>\n---\nname: probe\n---\n</skill>",
                                skill_root.join("SKILL.md").display()
                            )
                        }]
                    }
                }),
                serde_json::json!({
                    "type": "response_item",
                    "payload": {
                        "type": "message",
                        "role": "assistant",
                        "content": [{ "type": "output_text", "text": "used probe" }]
                    }
                }),
                serde_json::json!({
                    "type": "event_msg",
                    "payload": {
                        "type": "task_complete",
                        "turn_id": "turn-2"
                    }
                })
            ),
        )
        .unwrap();
    let hook_input = serde_json::json!({
        "session_id": "session-1",
        "turn_id": "turn-2",
        "transcript_path": transcript,
        "hook_event_name": "Stop",
        "model": "gpt-test"
    })
    .to_string();

    let result = record_skill_usage_from_hook("codex", &hook_input, &managed_root).unwrap();

    assert_eq!(result.recorded.len(), 1);
    assert_eq!(result.recorded[0].skill_name, "probe");
    assert_eq!(result.recorded[0].agent_id, "codex");
    assert_eq!(
        result.recorded[0].runtime_root,
        fs::canonicalize(runtime_root).unwrap()
    );

    let history = list_history(HistoryFilter::default(), &managed_root).unwrap();
    assert_eq!(
        history.entries[0].prompt_excerpt.as_deref(),
        Some("Review this plan")
    );
}

#[test]
fn scan_workspaces_discovers_global_and_user_roots() {
    let root = temp_dir("workspace-scan");
    let managed_root = root.join("SkillBox");
    let global_codex_root = root.join(".codex").join("skills");
    let global_claude_root = root.join(".claude").join("skills");
    let project_agents_root = root
        .join("Library")
        .join("Mobile Documents")
        .join("iCloud~md~obsidian")
        .join("Documents")
        .join("demo-vault")
        .join(".agents")
        .join("skills");
    make_skill(
        &global_codex_root.join("find-skills"),
        "find-skills",
        "Find skills",
    );
    make_skill(
        &global_claude_root.join("claude-helper"),
        "claude-helper",
        "Claude helper",
    );
    make_skill(
        &project_agents_root.join("demo-local"),
        "demo-local",
        "demo-vault local skill",
    );

    let result = scan_workspaces_under(&root, &managed_root).unwrap();
    let workspaces = list_workspaces(&managed_root).unwrap();
    let global_codex = workspace(&workspaces, &global_codex_root);
    let global_claude = workspace(&workspaces, &global_claude_root);
    let project_agents = workspace(&workspaces, &project_agents_root);

    assert_eq!(result.scanned_count, 3);
    assert_eq!(global_codex.kind, WorkspaceKind::Global);
    assert_eq!(global_codex.agent_id.as_deref(), Some("codex"));
    assert_eq!(global_codex.display_name, "Codex");
    assert_eq!(global_claude.kind, WorkspaceKind::Global);
    assert_eq!(global_claude.agent_id.as_deref(), Some("claude"));
    assert_eq!(global_claude.display_name, "Claude");
    assert_eq!(project_agents.kind, WorkspaceKind::User);
    assert_eq!(project_agents.agent_id.as_deref(), Some("agents"));
    assert_eq!(project_agents.display_name, "demo-vault");
}

#[test]
fn scan_workspaces_prunes_auto_roots_missing_from_latest_scan() {
    let root = temp_dir("workspace-scan-prune");
    let managed_root = root.join("SkillBox");
    let old_project_root = root.join("zone").join("audio-dialogue-web");
    let old_workspace_root = old_project_root.join(".codex").join("skills");
    let new_workspace_root = root
        .join("zone")
        .join("play")
        .join("audio-dialogue-web")
        .join(".codex")
        .join("skills");
    make_skill(&old_workspace_root.join("local"), "local", "Local skill");

    scan_workspaces_under(&root, &managed_root).unwrap();
    let old_canonical_path = fs::canonicalize(&old_workspace_root).unwrap();
    fs::remove_dir_all(&old_project_root).unwrap();
    make_skill(&new_workspace_root.join("local"), "local", "Local skill");

    let result = scan_workspaces_under(&root, &managed_root).unwrap();
    let workspaces = list_workspaces(&managed_root).unwrap();

    assert_eq!(result.scanned_count, 1);
    assert_eq!(workspace(&workspaces, &new_workspace_root).skill_count, 1);
    assert!(!workspaces
        .iter()
        .any(|workspace| workspace.canonical_path == old_canonical_path));
}

#[test]
fn scan_workspaces_keeps_manual_roots_missing_from_latest_scan() {
    let root = temp_dir("workspace-scan-keeps-manual");
    let managed_root = root.join("SkillBox");
    let manual_workspace_root = root.join(".external").join(".codex").join("skills");
    let auto_workspace_root = root
        .join("zone")
        .join("project")
        .join(".codex")
        .join("skills");
    make_skill(
        &manual_workspace_root.join("manual"),
        "manual",
        "Manual skill",
    );
    make_skill(&auto_workspace_root.join("auto"), "auto", "Auto skill");

    add_workspace(
        WorkspaceAddRequest {
            path: manual_workspace_root.clone(),
            kind: WorkspaceKind::User,
        },
        &managed_root,
    )
    .unwrap();
    let result = scan_workspaces_under(&root, &managed_root).unwrap();
    let workspaces = list_workspaces(&managed_root).unwrap();
    let manual_workspace = workspace(&workspaces, &manual_workspace_root);

    assert_eq!(result.scanned_count, 1);
    assert_eq!(manual_workspace.source, WorkspaceSource::Manual);
    assert_eq!(manual_workspace.skill_count, 1);
    assert_eq!(
        workspace(&workspaces, &auto_workspace_root).source,
        WorkspaceSource::Auto
    );
}

#[test]
fn scan_import_candidates_records_scanned_workspaces() {
    let root = temp_dir("workspace-import-candidates");
    let managed_root = root.join("SkillBox");
    let workspace_root = root.join("project").join(".agents").join("skills");
    make_skill(
        &workspace_root.join("demo-local"),
        "demo-local",
        "demo-vault local skill",
    );

    let candidates =
        scan_import_candidates(std::slice::from_ref(&workspace_root), &managed_root).unwrap();
    let workspaces = list_workspaces(&managed_root).unwrap();
    let recorded = workspace(&workspaces, &workspace_root);

    assert_eq!(candidates.candidates.len(), 1);
    assert_eq!(recorded.kind, WorkspaceKind::User);
    assert_eq!(recorded.source, WorkspaceSource::Auto);
    assert_eq!(recorded.display_name, "project");
    assert_eq!(recorded.skill_count, 1);
}

#[test]
fn scan_import_candidates_uses_discovered_project_local_roots() {
    let root = temp_dir("candidate-project-roots");
    let project_agents_root = root
        .join("Library")
        .join("Mobile Documents")
        .join("iCloud~md~obsidian")
        .join("Documents")
        .join("demo-vault")
        .join(".agents")
        .join("skills");
    let managed_root = root.join("SkillBox");

    make_skill(
        &project_agents_root.join("demo-local"),
        "demo-local",
        "demo-vault local skill",
    );

    let roots = runtime_roots_under(&root);
    let candidates = scan_import_candidates(&roots, &managed_root).unwrap();
    let candidate = candidate(&candidates.candidates, "demo-local");

    assert_eq!(candidate.suggested_type, SkillKind::User);
    assert_eq!(candidate.source_root, Some(project_agents_root));
    assert!(candidate.is_selected);
}

#[test]
fn imports_user_skill_and_deploys_symlink() {
    let root = temp_dir("import-deploy");
    let source = root.join("source").join("demo");
    let managed_root = root.join("SkillBox");
    let target_root = root.join("runtime");
    make_skill(&source, "demo", "Demo skill");

    let imported = import_skill(&source, SkillKind::User, &managed_root).unwrap();
    let deployment = deploy_skill("demo", &managed_root, &target_root).unwrap();

    assert_eq!(read_skill(&imported.managed_path).unwrap().name, "demo");
    assert!(fs::symlink_metadata(&deployment.target_path)
        .unwrap()
        .file_type()
        .is_symlink());
    assert_eq!(
        fs::canonicalize(&deployment.target_path).unwrap(),
        fs::canonicalize(&imported.managed_path).unwrap()
    );

    let state = managed_state(&managed_root).unwrap();
    assert_eq!(state.skills.len(), 1);
    assert_eq!(state.skills[0].deployments.len(), 1);
    assert_eq!(state.skills[0].deployments[0].target_root, target_root);
    assert_eq!(
        state.skills[0].deployments[0].target_path,
        deployment.target_path
    );
    assert_eq!(state.skills[0].deployments[0].mode, "symlink");
}

#[test]
fn deploys_remote_skill_to_current_symlink() {
    let root = temp_dir("remote-deploy-current");
    let source = root.join("source").join("remote-demo");
    let managed_root = root.join("SkillBox");
    let target_root = root.join("runtime");
    make_skill(&source, "remote-demo", "Remote demo skill");
    import_skill(&source, SkillKind::Remote, &managed_root).unwrap();

    let deployment = deploy_skill("remote-demo", &managed_root, &target_root).unwrap();
    let current = managed_root
        .join("remote-skills")
        .join("remote-demo")
        .join("current");

    assert!(fs::symlink_metadata(&deployment.target_path)
        .unwrap()
        .file_type()
        .is_symlink());
    assert_eq!(fs::read_link(&deployment.target_path).unwrap(), current);
}

#[test]
fn redeploys_remote_skill_version_symlink_to_current() {
    let root = temp_dir("remote-redeploy-current");
    let source = root.join("source").join("remote-demo");
    let managed_root = root.join("SkillBox");
    let target_root = root.join("runtime");
    let target_path = target_root.join("remote-demo");
    make_skill(&source, "remote-demo", "Remote demo skill");
    let imported = import_skill(&source, SkillKind::Remote, &managed_root).unwrap();
    fs::create_dir_all(&target_root).unwrap();
    symlink_dir(&imported.managed_path, &target_path).unwrap();

    deploy_skill("remote-demo", &managed_root, &target_root).unwrap();
    let current = managed_root
        .join("remote-skills")
        .join("remote-demo")
        .join("current");

    assert_eq!(fs::read_link(&target_path).unwrap(), current);
}

#[test]
fn refuses_to_overwrite_existing_non_symlink_deployment_target() {
    let root = temp_dir("deploy-conflict");
    let source = root.join("source").join("demo");
    let managed_root = root.join("SkillBox");
    let target_root = root.join("runtime");
    make_skill(&source, "demo", "Demo skill");
    import_skill(&source, SkillKind::User, &managed_root).unwrap();
    fs::create_dir_all(target_root.join("demo")).unwrap();

    let error = deploy_skill("demo", &managed_root, &target_root).unwrap_err();

    assert!(error.contains("Refusing to overwrite existing non-symlink target"));
}

#[test]
fn undeploys_managed_symlink_and_removes_deployment_index() {
    let root = temp_dir("undeploy-managed-link");
    let source = root.join("source").join("demo");
    let managed_root = root.join("SkillBox");
    let target_root = root.join("runtime");
    make_skill(&source, "demo", "Demo skill");
    import_skill(&source, SkillKind::User, &managed_root).unwrap();
    let deployment = deploy_skill("demo", &managed_root, &target_root).unwrap();

    let undeployment = undeploy_skill("demo", &managed_root, &target_root).unwrap();

    assert_eq!(undeployment.skill_name, "demo");
    assert_eq!(undeployment.target_root, target_root);
    assert_eq!(undeployment.target_path, deployment.target_path);
    assert!(!undeployment.target_path.exists());
    let state = managed_state(&managed_root).unwrap();
    assert_eq!(state.skills[0].deployments.len(), 0);
}

#[test]
fn undeploy_missing_target_removes_stale_deployment_index() {
    let root = temp_dir("undeploy-missing-target");
    let source = root.join("source").join("demo");
    let managed_root = root.join("SkillBox");
    let target_root = root.join("runtime");
    make_skill(&source, "demo", "Demo skill");
    import_skill(&source, SkillKind::User, &managed_root).unwrap();
    let deployment = deploy_skill("demo", &managed_root, &target_root).unwrap();
    fs::remove_file(&deployment.target_path).unwrap();

    let undeployment = undeploy_skill("demo", &managed_root, &target_root).unwrap();

    assert_eq!(undeployment.target_path, deployment.target_path);
    let state = managed_state(&managed_root).unwrap();
    assert_eq!(state.skills[0].deployments.len(), 0);
}

#[test]
fn undeploy_removes_workspace_alias_symlink() {
    let root = temp_dir("undeploy-alias-link");
    let source = root.join("source").join("dida-task-sync");
    let managed_root = root.join("SkillBox");
    let target_root = root.join("demo-vault").join(".agents").join("skills");
    make_skill(&source, "dida-task-sync", "Dida sync skill");
    let imported = import_skill(&source, SkillKind::User, &managed_root).unwrap();
    fs::create_dir_all(&target_root).unwrap();
    add_workspace(
        WorkspaceAddRequest {
            path: target_root.clone(),
            kind: WorkspaceKind::User,
        },
        &managed_root,
    )
    .unwrap();
    let alias_path = target_root.join("dida-task-sync 2");
    symlink_dir(&imported.managed_path, &alias_path).unwrap();

    let state = managed_state(&managed_root).unwrap();
    assert_eq!(state.skills[0].deployments.len(), 1);

    let undeployment = undeploy_skill("dida-task-sync", &managed_root, &target_root).unwrap();

    assert_eq!(undeployment.target_path, alias_path);
    assert!(!undeployment.target_path.exists());
    let state = managed_state(&managed_root).unwrap();
    assert_eq!(state.skills[0].deployments.len(), 0);
}

#[test]
fn undeploy_refuses_non_symlink_target() {
    let root = temp_dir("undeploy-non-symlink");
    let source = root.join("source").join("demo");
    let managed_root = root.join("SkillBox");
    let target_root = root.join("runtime");
    make_skill(&source, "demo", "Demo skill");
    import_skill(&source, SkillKind::User, &managed_root).unwrap();
    fs::create_dir_all(target_root.join("demo")).unwrap();

    let error = undeploy_skill("demo", &managed_root, &target_root).unwrap_err();

    assert!(error.contains("Refusing to remove existing non-symlink target"));
    assert!(target_root.join("demo").exists());
}

#[test]
fn undeploy_refuses_symlink_pointing_elsewhere() {
    let root = temp_dir("undeploy-foreign-link");
    let source = root.join("source").join("demo");
    let managed_root = root.join("SkillBox");
    let target_root = root.join("runtime");
    let other_target = root.join("other").join("demo");
    make_skill(&source, "demo", "Demo skill");
    make_skill(&other_target, "demo", "Other demo skill");
    import_skill(&source, SkillKind::User, &managed_root).unwrap();
    fs::create_dir_all(&target_root).unwrap();
    symlink_dir(&other_target, &target_root.join("demo")).unwrap();

    let error = undeploy_skill("demo", &managed_root, &target_root).unwrap_err();

    assert!(error.contains("Refusing to remove symlink pointing elsewhere"));
    assert!(fs::symlink_metadata(target_root.join("demo"))
        .unwrap()
        .file_type()
        .is_symlink());
}

#[test]
fn managed_state_is_first_use_when_managed_store_has_no_skills() {
    let root = temp_dir("managed-state-empty");
    let state = managed_state(root.join("SkillBox")).unwrap();

    assert!(state.is_first_use);
    assert_eq!(state.skills.len(), 0);
}

#[test]
fn managed_state_lists_remote_skill_current_once() {
    let root = temp_dir("managed-state-remote-once");
    let source = root.join("runtime").join("find-skills");
    let managed_root = root.join("SkillBox");
    make_skill(&source, "find-skills", "Find skills");
    import_skill(&source, SkillKind::Remote, &managed_root).unwrap();

    let state = managed_state(&managed_root).unwrap();

    assert_eq!(state.skills.len(), 1);
    assert_eq!(state.skills[0].name, "find-skills");
    assert_eq!(state.skills[0].kind, SkillKind::Remote);
    assert!(state.skills[0].path.ends_with("current"));
}

#[test]
fn managed_state_infers_workspace_symlink_deployments_without_index() {
    let root = temp_dir("managed-state-inferred-deployment");
    let source = root.join("source").join("ui-ux-pro-max");
    let managed_root = root.join("SkillBox");
    let workspace_root = root.join("demo-app").join(".codex").join("skills");
    make_skill(&source, "ui-ux-pro-max", "UI UX skill");
    import_skill(&source, SkillKind::Remote, &managed_root).unwrap();
    fs::create_dir_all(&workspace_root).unwrap();
    add_workspace(
        WorkspaceAddRequest {
            path: workspace_root.clone(),
            kind: WorkspaceKind::User,
        },
        &managed_root,
    )
    .unwrap();
    let managed_current = managed_root
        .join("remote-skills")
        .join("ui-ux-pro-max")
        .join("current");
    symlink_dir(&managed_current, &workspace_root.join("ui-ux-pro-max")).unwrap();

    let state = managed_state(&managed_root).unwrap();

    assert_eq!(state.skills.len(), 1);
    assert_eq!(state.skills[0].deployments.len(), 1);
    assert_eq!(state.skills[0].deployments[0].target_root, workspace_root);
    assert_eq!(
        state.skills[0].deployments[0].target_path,
        state.skills[0].deployments[0]
            .target_root
            .join("ui-ux-pro-max")
    );
    assert_eq!(state.skills[0].deployments[0].mode, "symlink");
}

#[test]
fn managed_state_detects_workspace_alias_symlink_deployment() {
    let root = temp_dir("managed-state-alias-deployment");
    let source = root.join("source").join("dida-task-sync");
    let managed_root = root.join("SkillBox");
    let workspace_root = root.join("demo-vault").join(".agents").join("skills");
    make_skill(&source, "dida-task-sync", "Dida sync skill");
    let imported = import_skill(&source, SkillKind::User, &managed_root).unwrap();
    fs::create_dir_all(&workspace_root).unwrap();
    add_workspace(
        WorkspaceAddRequest {
            path: workspace_root.clone(),
            kind: WorkspaceKind::User,
        },
        &managed_root,
    )
    .unwrap();
    let alias_path = workspace_root.join("dida-task-sync 2");
    symlink_dir(&imported.managed_path, &alias_path).unwrap();

    let state = managed_state(&managed_root).unwrap();

    assert_eq!(state.skills.len(), 1);
    assert_eq!(state.skills[0].deployments.len(), 1);
    assert_eq!(state.skills[0].deployments[0].target_root, workspace_root);
    assert_eq!(state.skills[0].deployments[0].target_path, alias_path);
    assert_eq!(state.skills[0].deployments[0].mode, "symlink");
}

#[test]
fn managed_preferences_default_to_showing_local_import_confirmation() {
    let root = temp_dir("preferences-default");
    let preferences = managed_preferences(root.join("SkillBox")).unwrap();

    assert!(!preferences.skip_local_import_confirmation);
    assert_eq!(preferences.status_refresh_interval_minutes, 5);
    assert_eq!(preferences.remote_update_timeout_seconds, 30);
}

#[test]
fn managed_preferences_persist_skip_local_import_confirmation() {
    let root = temp_dir("preferences-persist");
    let managed_root = root.join("SkillBox");

    set_skip_local_import_confirmation(&managed_root, true).unwrap();
    let preferences = managed_preferences(&managed_root).unwrap();

    assert!(preferences.skip_local_import_confirmation);
    assert_eq!(preferences.status_refresh_interval_minutes, 5);
    assert_eq!(preferences.remote_update_timeout_seconds, 30);
}

#[test]
fn managed_preferences_persist_status_refresh_interval() {
    let root = temp_dir("preferences-refresh-interval");
    let managed_root = root.join("SkillBox");

    let preferences = set_status_refresh_interval_minutes(&managed_root, 10).unwrap();

    assert_eq!(preferences.status_refresh_interval_minutes, 10);
    assert_eq!(
        managed_preferences(&managed_root)
            .unwrap()
            .status_refresh_interval_minutes,
        10
    );
}

#[test]
fn managed_preferences_reject_invalid_status_refresh_interval() {
    let root = temp_dir("preferences-invalid-refresh-interval");
    let managed_root = root.join("SkillBox");

    let error = set_status_refresh_interval_minutes(&managed_root, 0).unwrap_err();

    assert!(error.contains("between 1 and 1440"));
}

#[test]
fn managed_preferences_persist_remote_update_timeout() {
    let root = temp_dir("preferences-remote-timeout");
    let managed_root = root.join("SkillBox");

    let preferences = set_remote_update_timeout_seconds(&managed_root, 45).unwrap();

    assert_eq!(preferences.remote_update_timeout_seconds, 45);
    assert_eq!(
        managed_preferences(&managed_root)
            .unwrap()
            .remote_update_timeout_seconds,
        45
    );
}

#[test]
fn managed_preferences_reject_invalid_remote_update_timeout() {
    let root = temp_dir("preferences-invalid-remote-timeout");
    let managed_root = root.join("SkillBox");

    let error = set_remote_update_timeout_seconds(&managed_root, 4).unwrap_err();

    assert!(error.contains("between 5 and 300"));
}

#[test]
fn operation_log_records_success_failure_and_cancellation() {
    let managed_root = temp_dir("operation-log-statuses").join("SkillBox");
    ensure_managed_layout(&managed_root).unwrap();

    let started = start_operation(
        OperationStart {
            operation_type: "bind_remote_source".to_string(),
            actor: "cli".to_string(),
            entity_type: "skill".to_string(),
            entity_name: "find-skills".to_string(),
            summary: "Bind find-skills to GitHub source".to_string(),
            payload: serde_json::json!({
                "sourceUrl": "https://github.com/acme/skills/tree/main/find-skills"
            }),
        },
        &managed_root,
    )
    .unwrap();
    assert_eq!(started.status, OperationStatus::Started);

    let succeeded = finish_operation(
        OperationFinish {
            id: started.id.clone(),
            status: OperationStatus::Succeeded,
            summary: "Bound find-skills to GitHub source".to_string(),
            error: None,
            payload: serde_json::json!({"validation": "same_skill_changed"}),
        },
        &managed_root,
    )
    .unwrap();
    assert_eq!(succeeded.status, OperationStatus::Succeeded);

    let failed = start_operation(
        OperationStart {
            operation_type: "update_remote_skill".to_string(),
            actor: "desktop".to_string(),
            entity_type: "skill".to_string(),
            entity_name: "find-skills".to_string(),
            summary: "Update find-skills".to_string(),
            payload: serde_json::json!({
                "fromVersion": "manual-abc",
                "toVersion": "123"
            }),
        },
        &managed_root,
    )
    .unwrap();
    let failed = finish_operation(
        OperationFinish {
            id: failed.id,
            status: OperationStatus::Failed,
            summary: "Update find-skills failed".to_string(),
            error: Some("Missing SKILL.md".to_string()),
            payload: serde_json::json!({"restoredCurrent": true}),
        },
        &managed_root,
    )
    .unwrap();
    assert_eq!(failed.status, OperationStatus::Failed);
    assert_eq!(failed.error.as_deref(), Some("Missing SKILL.md"));

    let cancelled = start_operation(
        OperationStart {
            operation_type: "preview_version_change".to_string(),
            actor: "desktop".to_string(),
            entity_type: "skill".to_string(),
            entity_name: "find-skills".to_string(),
            summary: "Preview rollback for find-skills".to_string(),
            payload: serde_json::json!({"action": "rollback"}),
        },
        &managed_root,
    )
    .unwrap();
    let cancelled = finish_operation(
        OperationFinish {
            id: cancelled.id,
            status: OperationStatus::Cancelled,
            summary: "Rollback preview cancelled".to_string(),
            error: None,
            payload: serde_json::json!({"cancelledBy": "user"}),
        },
        &managed_root,
    )
    .unwrap();
    assert_eq!(cancelled.status, OperationStatus::Cancelled);

    let list = list_operations(OperationFilter::default(), &managed_root).unwrap();
    assert_eq!(list.operations.len(), 3);
    assert_eq!(list.operations[0].status, OperationStatus::Cancelled);
    assert_eq!(list.operations[1].status, OperationStatus::Failed);
    assert_eq!(list.operations[2].status, OperationStatus::Succeeded);
}

#[test]
fn operation_log_filters_by_entity_and_status() {
    let managed_root = temp_dir("operation-log-filters").join("SkillBox");
    ensure_managed_layout(&managed_root).unwrap();

    let alpha = start_operation(
        OperationStart {
            operation_type: "deploy_skill".to_string(),
            actor: "cli".to_string(),
            entity_type: "skill".to_string(),
            entity_name: "alpha".to_string(),
            summary: "Deploy alpha".to_string(),
            payload: serde_json::json!({}),
        },
        &managed_root,
    )
    .unwrap();
    finish_operation(
        OperationFinish {
            id: alpha.id,
            status: OperationStatus::Succeeded,
            summary: "Deployed alpha".to_string(),
            error: None,
            payload: serde_json::json!({}),
        },
        &managed_root,
    )
    .unwrap();

    let beta = start_operation(
        OperationStart {
            operation_type: "deploy_skill".to_string(),
            actor: "cli".to_string(),
            entity_type: "skill".to_string(),
            entity_name: "beta".to_string(),
            summary: "Deploy beta".to_string(),
            payload: serde_json::json!({}),
        },
        &managed_root,
    )
    .unwrap();
    finish_operation(
        OperationFinish {
            id: beta.id,
            status: OperationStatus::Failed,
            summary: "Deploy beta failed".to_string(),
            error: Some("target exists".to_string()),
            payload: serde_json::json!({}),
        },
        &managed_root,
    )
    .unwrap();

    let filtered = list_operations(
        OperationFilter {
            entity_type: Some("skill".to_string()),
            entity_name: Some("beta".to_string()),
            status: Some(OperationStatus::Failed),
            limit: Some(20),
        },
        &managed_root,
    )
    .unwrap();

    assert_eq!(filtered.operations.len(), 1);
    assert_eq!(filtered.operations[0].entity_name, "beta");
    assert_eq!(filtered.operations[0].status, OperationStatus::Failed);
}

#[test]
fn history_lists_skill_usage_and_operations_together() {
    let managed_root = temp_dir("history-combined").join("SkillBox");
    let runtime_root = temp_dir("history-runtime").join(".codex").join("skills");
    fs::create_dir_all(&runtime_root).unwrap();
    record_skill_usage(
        RecordSkillUsageRequest {
            skill_name: "grill-me".to_string(),
            agent_id: "codex".to_string(),
            runtime_root: runtime_root.clone(),
            event_id: Some("event-1".to_string()),
            used_at: Some("2026-06-03T10:00:00Z".to_string()),
            prompt_excerpt: None,
            metadata: Some(serde_json::json!({"source": "test"})),
        },
        &managed_root,
    )
    .unwrap();
    let operation = start_operation(
        OperationStart {
            operation_type: "deploy_skill".to_string(),
            actor: "desktop".to_string(),
            entity_type: "skill".to_string(),
            entity_name: "grill-me".to_string(),
            summary: "Deploy grill-me".to_string(),
            payload: serde_json::json!({}),
        },
        &managed_root,
    )
    .unwrap();
    finish_operation(
        OperationFinish {
            id: operation.id,
            status: OperationStatus::Succeeded,
            summary: "Deployed grill-me".to_string(),
            error: None,
            payload: serde_json::json!({}),
        },
        &managed_root,
    )
    .unwrap();

    let history = list_history(HistoryFilter::default(), &managed_root).unwrap();
    let usage_only = list_history(
        HistoryFilter {
            kind: Some(HistoryEntryKind::SkillUsage),
            limit: Some(20),
        },
        &managed_root,
    )
    .unwrap();

    assert_eq!(history.skill_usage_count, 1);
    assert_eq!(history.operation_count, 1);
    assert_eq!(history.entries.len(), 2);
    assert!(history
        .entries
        .iter()
        .any(|entry| entry.kind == HistoryEntryKind::SkillUsage
            && entry.skill_name.as_deref() == Some("grill-me")
            && entry.agent_id.as_deref() == Some("codex")));
    assert!(history
        .entries
        .iter()
        .any(|entry| entry.kind == HistoryEntryKind::Operation
            && entry.status == Some(OperationStatus::Succeeded)));
    assert_eq!(usage_only.entries.len(), 1);
    assert_eq!(usage_only.entries[0].kind, HistoryEntryKind::SkillUsage);
}

#[test]
fn history_abbreviates_full_sha_values_in_operation_titles() {
    let managed_root = temp_dir("history-short-sha").join("SkillBox");
    let from_sha = "690f15cac7b4c055c5ab109c79ed9259934081";
    let to_sha = "da20c92503b2e8ff1cf28ca81a0df4673debdbf7";
    let full_summary = format!("Changed frontend-design from {from_sha} to {to_sha}");
    let operation = start_operation(
        OperationStart {
            operation_type: "update_remote_skill".to_string(),
            actor: "desktop".to_string(),
            entity_type: "skill".to_string(),
            entity_name: "frontend-design".to_string(),
            summary: "Apply update for frontend-design".to_string(),
            payload: serde_json::json!({}),
        },
        &managed_root,
    )
    .unwrap();
    finish_operation(
        OperationFinish {
            id: operation.id,
            status: OperationStatus::Succeeded,
            summary: full_summary.clone(),
            error: None,
            payload: serde_json::json!({}),
        },
        &managed_root,
    )
    .unwrap();

    let history = list_history(HistoryFilter::default(), &managed_root).unwrap();
    let operations = list_operations(OperationFilter::default(), &managed_root).unwrap();
    let title = &history.entries[0].title;

    assert_eq!(
        title,
        "Changed frontend-design from 690f15cac7b4 to da20c92503b2"
    );
    assert!(!title.contains(from_sha));
    assert!(!title.contains(to_sha));
    assert_eq!(operations.operations[0].summary, full_summary);
}

#[test]
fn user_skills_git_status_is_not_configured_without_origin() {
    let managed_root = temp_dir("user-skills-status").join("SkillBox");
    let status = user_skills_git_status(&managed_root).unwrap();

    assert_eq!(status.state, UserSkillsGitState::NotConfigured);
    assert!(!status.initialized);
    assert!(status.remote_url.is_none());
}

#[test]
fn set_user_skills_git_remote_initializes_repo_and_sets_origin() {
    let managed_root = temp_dir("user-skills-remote-settings").join("SkillBox");
    let remote = bare_remote("user-skills-remote-settings-origin");
    let remote_url = remote.to_string_lossy().to_string();

    let status = set_user_skills_git_remote(
        UserSkillsGitRemoteRequest {
            remote_url: remote_url.clone(),
        },
        &managed_root,
    )
    .unwrap();

    assert!(status.initialized);
    assert_eq!(status.state, UserSkillsGitState::Dirty);
    assert_eq!(status.changed_paths, vec![".gitignore".to_string()]);
    assert_eq!(status.remote_url.as_deref(), Some(remote_url.as_str()));
}

#[test]
fn sync_user_skills_initializes_shared_repo_and_commits_all_skills() {
    let root = temp_dir("user-skills-sync");
    let managed_root = root.join("SkillBox");
    let paths = ensure_managed_layout(&managed_root).unwrap();
    make_skill(
        &paths.user_skills_root.join("alpha"),
        "alpha",
        "Alpha skill",
    );
    make_skill(&paths.user_skills_root.join("beta"), "beta", "Beta skill");
    let remote = bare_remote("user-skills-sync-remote");

    let result = sync_user_skills_git(
        UserSkillsSyncRequest {
            remote_url: Some(remote.to_string_lossy().to_string()),
            commit_message: Some("Sync user skills".to_string()),
            push: true,
            selected_paths: None,
        },
        &managed_root,
    )
    .unwrap();

    assert!(result.initialized);
    assert!(result.remote_updated);
    assert!(result.committed);
    assert!(result.pushed);
    assert_eq!(result.state, UserSkillsGitState::Clean);
}

#[test]
fn sync_user_skills_reports_push_failed_without_losing_commit() {
    let root = temp_dir("user-skills-push-fail");
    let managed_root = root.join("SkillBox");
    let paths = ensure_managed_layout(&managed_root).unwrap();
    make_skill(
        &paths.user_skills_root.join("alpha"),
        "alpha",
        "Alpha skill",
    );

    let result = sync_user_skills_git(
        UserSkillsSyncRequest {
            remote_url: Some("/no/such/remote.git".to_string()),
            commit_message: Some("Sync user skills".to_string()),
            push: true,
            selected_paths: None,
        },
        &managed_root,
    )
    .unwrap();

    assert!(result.committed);
    assert!(!result.pushed);
    assert_eq!(result.state, UserSkillsGitState::PushFailed);
    assert!(result.message.contains("push"));
}

#[test]
fn user_skills_git_changes_include_files_and_diff() {
    let root = temp_dir("user-skills-changes");
    let managed_root = root.join("SkillBox");
    let paths = ensure_managed_layout(&managed_root).unwrap();
    make_skill(
        &paths.user_skills_root.join("alpha"),
        "alpha",
        "Alpha skill",
    );
    sync_user_skills_git(
        UserSkillsSyncRequest {
            remote_url: None,
            commit_message: Some("Initial user skills".to_string()),
            push: false,
            selected_paths: None,
        },
        &managed_root,
    )
    .unwrap();
    fs::write(
        paths.user_skills_root.join("alpha").join("SKILL.md"),
        "---\nname: alpha\ndescription: Updated alpha skill\n---\n",
    )
    .unwrap();
    make_skill(&paths.user_skills_root.join("beta"), "beta", "Beta skill");

    let changes = user_skills_git_changes(&managed_root).unwrap();

    let paths: Vec<_> = changes
        .files
        .iter()
        .map(|file| file.path.as_str())
        .collect();
    assert!(paths.contains(&"alpha/SKILL.md"));
    assert!(paths.contains(&"beta/SKILL.md"));
    assert!(changes
        .files
        .iter()
        .any(|file| file.path == "alpha/SKILL.md" && file.diff.contains("Updated alpha")));
    assert!(changes
        .files
        .iter()
        .any(|file| file.path == "beta/SKILL.md" && file.diff.contains("Beta skill")));
}

#[test]
fn user_skills_git_status_reports_changed_paths() {
    let root = temp_dir("user-skills-status-changed-paths");
    let managed_root = root.join("SkillBox");
    let paths = ensure_managed_layout(&managed_root).unwrap();
    make_skill(
        &paths.user_skills_root.join("alpha"),
        "alpha",
        "Alpha skill",
    );
    make_skill(&paths.user_skills_root.join("beta"), "beta", "Beta skill");
    let remote = bare_remote("user-skills-status-changed-paths-origin");
    sync_user_skills_git(
        UserSkillsSyncRequest {
            remote_url: Some(remote.to_string_lossy().to_string()),
            commit_message: Some("Initial user skills".to_string()),
            push: false,
            selected_paths: None,
        },
        &managed_root,
    )
    .unwrap();
    fs::write(
        paths.user_skills_root.join("alpha").join("SKILL.md"),
        "---\nname: alpha\ndescription: Updated alpha skill\n---\n",
    )
    .unwrap();

    let status = user_skills_git_status(&managed_root).unwrap();

    assert_eq!(status.state, UserSkillsGitState::Dirty);
    assert_eq!(status.changed_paths, vec!["alpha/SKILL.md".to_string()]);
}

#[test]
fn sync_user_skills_commits_only_selected_paths() {
    let root = temp_dir("user-skills-selected-sync");
    let managed_root = root.join("SkillBox");
    let paths = ensure_managed_layout(&managed_root).unwrap();
    make_skill(
        &paths.user_skills_root.join("alpha"),
        "alpha",
        "Alpha skill",
    );
    make_skill(&paths.user_skills_root.join("beta"), "beta", "Beta skill");
    let remote = bare_remote("user-skills-selected-sync-remote");
    sync_user_skills_git(
        UserSkillsSyncRequest {
            remote_url: Some(remote.to_string_lossy().to_string()),
            commit_message: Some("Initial user skills".to_string()),
            push: false,
            selected_paths: None,
        },
        &managed_root,
    )
    .unwrap();
    fs::write(
        paths.user_skills_root.join("alpha").join("SKILL.md"),
        "---\nname: alpha\ndescription: Updated alpha skill\n---\n",
    )
    .unwrap();
    fs::write(
        paths.user_skills_root.join("beta").join("SKILL.md"),
        "---\nname: beta\ndescription: Updated beta skill\n---\n",
    )
    .unwrap();

    let result = sync_user_skills_git(
        UserSkillsSyncRequest {
            remote_url: None,
            commit_message: Some("Sync selected user skill".to_string()),
            push: false,
            selected_paths: Some(vec!["alpha/SKILL.md".to_string()]),
        },
        &managed_root,
    )
    .unwrap();

    assert!(result.committed);
    assert_eq!(result.state, UserSkillsGitState::Dirty);
    assert!(result.raw_status.contains("beta/SKILL.md"));
    assert!(!result.raw_status.contains("alpha/SKILL.md"));
}

#[test]
fn user_skill_versions_include_current_worktree_and_git_history() {
    let root = temp_dir("user-skill-versions");
    let managed_root = root.join("SkillBox");
    let paths = ensure_managed_layout(&managed_root).unwrap();
    make_skill_with_body(
        &paths.user_skills_root.join("alpha"),
        "alpha",
        "Alpha skill",
        "version one\n",
    );
    make_skill_with_body(
        &paths.user_skills_root.join("beta"),
        "beta",
        "Beta skill",
        "beta version\n",
    );
    sync_user_skills_git(
        UserSkillsSyncRequest {
            remote_url: None,
            commit_message: Some("Initial user skills".to_string()),
            push: false,
            selected_paths: None,
        },
        &managed_root,
    )
    .unwrap();
    make_skill_with_body(
        &paths.user_skills_root.join("alpha"),
        "alpha",
        "Alpha skill",
        "version two\n",
    );
    sync_user_skills_git(
        UserSkillsSyncRequest {
            remote_url: None,
            commit_message: Some("Update alpha skill".to_string()),
            push: false,
            selected_paths: Some(vec!["alpha/SKILL.md".to_string()]),
        },
        &managed_root,
    )
    .unwrap();
    make_skill_with_body(
        &paths.user_skills_root.join("alpha"),
        "alpha",
        "Alpha skill",
        "work in progress\n",
    );

    let versions = list_user_skill_versions("alpha", &managed_root).unwrap();

    assert_eq!(versions.skill_name, "alpha");
    assert_eq!(versions.versions.len(), 3);
    assert!(versions.versions[0].is_current);
    assert_eq!(versions.versions[0].kind, "working");
    assert_eq!(versions.current_version, versions.versions[0].version);
    assert_eq!(versions.versions[1].kind, "git");
    assert_eq!(
        versions.versions[1].message.as_deref(),
        Some("Update alpha skill")
    );
    assert_eq!(
        versions.versions[2].message.as_deref(),
        Some("Initial user skills")
    );
    assert!(!versions
        .versions
        .iter()
        .any(|version| version.message.as_deref() == Some("Beta skill")));
}

#[test]
fn check_remote_skill_updates_reports_update_available_and_up_to_date() {
    let root = temp_dir("remote-updates");
    let managed_root = root.join("SkillBox");
    let paths = ensure_managed_layout(&managed_root).unwrap();
    let remote = bare_remote_with_main("remote-updates-origin");
    let latest_sha = remote_head(&remote);

    write_remote_source(
        &paths.remote_skills_root.join("fresh"),
        &remote,
        &latest_sha,
    );
    write_remote_source(
        &paths.remote_skills_root.join("stale"),
        &remote,
        "0000000000000000000000000000000000000000",
    );

    let result = check_remote_skill_updates(&managed_root).unwrap();
    let fresh = remote_status(&result.statuses, "fresh");
    let stale = remote_status(&result.statuses, "stale");

    assert_eq!(fresh.state, RemoteSkillUpdateState::UpToDate);
    assert!(!fresh.update_available);
    assert_eq!(fresh.latest_sha.as_deref(), Some(latest_sha.as_str()));
    assert_eq!(stale.state, RemoteSkillUpdateState::UpdateAvailable);
    assert!(stale.update_available);
    assert_eq!(stale.latest_sha.as_deref(), Some(latest_sha.as_str()));
}

#[test]
fn install_github_remote_skill_writes_version_current_metadata_and_index() {
    let root = temp_dir("install-github-remote");
    let managed_root = root.join("SkillBox");
    let remote = bare_remote_with_skill_content(
        "install-github-remote-origin",
        "find-skills",
        "Find skills",
        "Remote body\n",
    );
    let installed_sha = remote_head(&remote);
    let _rewrite = github_repo_rewrite("acme", "install-github-remote", &remote);

    let result = install_github_remote_skill(
        InstallGithubRemoteSkillRequest {
            source_url: github_source_url("acme", "install-github-remote", "find-skills"),
            target_root: None,
            actor: "cli".to_string(),
        },
        &managed_root,
    )
    .unwrap();

    let paths = managed_paths(&managed_root);
    let remote_root = paths.remote_skills_root.join("find-skills");
    let version_path = remote_root.join("versions").join(&installed_sha);
    assert_eq!(result.skill_name, "find-skills");
    assert_eq!(result.installed_sha, installed_sha);
    assert_eq!(result.version_path, version_path);
    assert_eq!(
        fs::canonicalize(remote_root.join("current")).unwrap(),
        fs::canonicalize(&version_path).unwrap()
    );
    assert!(version_path.join("SKILL.md").exists());

    let source_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(remote_root.join("source.json")).unwrap())
            .unwrap();
    assert_eq!(source_json["type"], "github");
    assert_eq!(source_json["owner"], "acme");
    assert_eq!(source_json["repo"], "install-github-remote");
    assert_eq!(source_json["path"], "skills/find-skills");
    assert_eq!(source_json["ref"], "main");
    assert_eq!(source_json["currentVersion"], installed_sha);
    assert_eq!(source_json["installedSha"], installed_sha);
    assert_eq!(source_json["latestSha"], installed_sha);
    assert_eq!(source_json["tracking"], true);

    let connection = open_database(&paths.database_path).unwrap();
    let (kind, indexed_path): (String, String) = connection
        .query_row(
            "SELECT type, managed_path FROM skills WHERE name = 'find-skills'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(kind, "remote");
    assert_eq!(indexed_path, version_path.to_string_lossy().to_string());
}

#[test]
fn install_github_remote_skill_deploys_to_target_root() {
    let root = temp_dir("install-github-deploy");
    let managed_root = root.join("SkillBox");
    let target_root = root.join("runtime");
    let remote = bare_remote_with_skill_content(
        "install-github-deploy-origin",
        "find-skills",
        "Find skills",
        "",
    );
    let _rewrite = github_repo_rewrite("acme", "install-github-deploy", &remote);

    let result = install_github_remote_skill(
        InstallGithubRemoteSkillRequest {
            source_url: github_source_url("acme", "install-github-deploy", "find-skills"),
            target_root: Some(target_root.clone()),
            actor: "cli".to_string(),
        },
        &managed_root,
    )
    .unwrap();

    let deployment = result.deployment.unwrap();
    assert_eq!(deployment.target_root, target_root);
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

    let first = install_github_remote_skill(
        InstallGithubRemoteSkillRequest {
            source_url: github_source_url("acme", "install-github-reuse-version", "find-skills"),
            target_root: None,
            actor: "cli".to_string(),
        },
        &managed_root,
    )
    .unwrap();
    let marker = first.version_path.join("marker.txt");
    fs::write(&marker, "kept").unwrap();

    let second = install_github_remote_skill(
        InstallGithubRemoteSkillRequest {
            source_url: github_source_url("acme", "install-github-reuse-version", "find-skills"),
            target_root: None,
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
    let version_path = managed_root
        .join("remote-skills")
        .join("find-skills")
        .join("versions")
        .join(&installed_sha);
    fs::create_dir_all(version_path.parent().unwrap()).unwrap();
    fs::write(&version_path, "not a directory").unwrap();

    let error = install_github_remote_skill(
        InstallGithubRemoteSkillRequest {
            source_url: github_source_url("acme", "install-github-copy-failure", "find-skills"),
            target_root: None,
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

#[test]
fn install_github_remote_skill_rejects_traversal_url_without_creating_store() {
    let root = temp_dir("install-github-traversal");
    let managed_root = root.join("SkillBox");

    let error = install_github_remote_skill(
        InstallGithubRemoteSkillRequest {
            source_url: "https://github.com/acme/repo/tree/main/skills/../../secret".to_string(),
            target_root: None,
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
    let remote_root = managed_root.join("remote-skills").join("find-skills");
    let current_path = remote_root.join("current");
    fs::create_dir_all(&remote_root).unwrap();
    fs::write(&current_path, "not a symlink").unwrap();

    let error = install_github_remote_skill(
        InstallGithubRemoteSkillRequest {
            source_url: github_source_url("acme", "install-github-current-conflict", "find-skills"),
            target_root: None,
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
fn scan_import_candidates_excludes_already_imported_skills_by_hash() {
    let root = temp_dir("candidate-excludes-imported");
    let source = root.join("runtime").join("demo");
    let managed_root = root.join("SkillBox");
    make_skill(&source, "demo", "Demo skill");
    import_skill(&source, SkillKind::User, &managed_root).unwrap();

    let candidates = scan_import_candidates(&[root.join("runtime")], &managed_root).unwrap();

    assert_eq!(candidates.candidates.len(), 1);
    let demo = candidate(&candidates.candidates, "demo");
    assert_eq!(demo.import_status, ImportCandidateStatus::Imported);
    assert!(!demo.is_selected);
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

    record_skill_usage(
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
    record_skill_usage(
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

fn temp_dir(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("skillbox-{label}-{nanos}"));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn make_skill(path: &std::path::Path, name: &str, description: &str) {
    make_skill_with_body(path, name, description, "");
}

fn make_skill_with_body(path: &std::path::Path, name: &str, description: &str, extra_body: &str) {
    fs::create_dir_all(path).unwrap();
    fs::write(
        path.join("SKILL.md"),
        format!(
            "---
name: {name}
description: \"{description}\"
---

# {name}
{extra_body}
"
        ),
    )
    .unwrap();
}

fn candidate<'a>(candidates: &'a [ImportCandidate], name: &str) -> &'a ImportCandidate {
    candidates
        .iter()
        .find(|candidate| candidate.name == name)
        .unwrap_or_else(|| panic!("candidate not found: {name}"))
}

fn remote_status<'a>(
    statuses: &'a [RemoteSkillUpdateStatus],
    skill_name: &str,
) -> &'a RemoteSkillUpdateStatus {
    statuses
        .iter()
        .find(|status| status.skill_name == skill_name)
        .unwrap_or_else(|| panic!("remote status not found: {skill_name}"))
}

fn workspace<'a>(workspaces: &'a [Workspace], path: &std::path::Path) -> &'a Workspace {
    let canonical = fs::canonicalize(path).unwrap();
    workspaces
        .iter()
        .find(|workspace| workspace.canonical_path == canonical)
        .unwrap_or_else(|| panic!("workspace not found: {}", path.display()))
}

fn write_remote_source(
    remote_root: &std::path::Path,
    repo_url: &std::path::Path,
    installed_sha: &str,
) {
    fs::create_dir_all(remote_root).unwrap();
    fs::write(
        remote_root.join("source.json"),
        format!(
            r#"{{
  "type": "github",
  "repoUrl": "{}",
  "ref": "main",
  "installedSha": "{}"
}}"#,
            repo_url.display(),
            installed_sha
        ),
    )
    .unwrap();
}

fn write_remote_source_with_json(remote_root: &std::path::Path, json: &str) {
    fs::create_dir_all(remote_root).unwrap();
    fs::write(remote_root.join("source.json"), json).unwrap();
}

fn bare_remote(label: &str) -> PathBuf {
    let remote = temp_dir(label).join("remote.git");
    let output = std::process::Command::new("git")
        .arg("init")
        .arg("--bare")
        .arg(&remote)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    remote
}

fn bare_remote_with_main(label: &str) -> PathBuf {
    let remote = bare_remote(label);
    let work = temp_dir(&format!("{label}-work"));
    run_git(&work, &["init", "-b", "main"]);
    fs::write(work.join("README.md"), "remote").unwrap();
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
            "Initial",
        ],
    );
    run_git(
        &work,
        &["remote", "add", "origin", remote.to_str().unwrap()],
    );
    run_git(&work, &["push", "origin", "main"]);
    remote
}

fn bare_remote_with_skill_content(
    label: &str,
    skill_name: &str,
    description: &str,
    body: &str,
) -> PathBuf {
    let remote = bare_remote(label);
    let work = temp_dir(&format!("{label}-work"));
    run_git(&work, &["init", "-b", "main"]);
    let skill_dir = work.join("skills").join(skill_name);
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(
        skill_dir.join("SKILL.md"),
        format!(
            "---
name: {skill_name}
description: \"{description}\"
---

# {skill_name}
{body}
"
        ),
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
            "Add skill",
        ],
    );
    run_git(
        &work,
        &["remote", "add", "origin", remote.to_str().unwrap()],
    );
    run_git(&work, &["push", "-u", "origin", "main"]);
    remote
}

fn github_source_url(owner: &str, repo: &str, skill_name: &str) -> String {
    format!("https://github.com/{owner}/{repo}/tree/main/skills/{skill_name}")
}

static GIT_CONFIG_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct GitConfigRewriteGuard {
    _lock: std::sync::MutexGuard<'static, ()>,
    previous: Vec<(&'static str, Option<std::ffi::OsString>)>,
}

impl Drop for GitConfigRewriteGuard {
    fn drop(&mut self) {
        for (key, value) in self.previous.drain(..) {
            match value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
    }
}

fn github_repo_rewrite(owner: &str, repo: &str, remote: &std::path::Path) -> GitConfigRewriteGuard {
    let lock = GIT_CONFIG_LOCK.lock().unwrap();
    let keys = ["GIT_CONFIG_COUNT", "GIT_CONFIG_KEY_0", "GIT_CONFIG_VALUE_0"];
    let previous = keys
        .into_iter()
        .map(|key| (key, std::env::var_os(key)))
        .collect::<Vec<_>>();

    std::env::set_var("GIT_CONFIG_COUNT", "1");
    std::env::set_var(
        "GIT_CONFIG_KEY_0",
        format!("url.file://{}.insteadOf", remote.display()),
    );
    std::env::set_var(
        "GIT_CONFIG_VALUE_0",
        format!("https://github.com/{owner}/{repo}.git"),
    );

    GitConfigRewriteGuard {
        _lock: lock,
        previous,
    }
}

fn remote_head(remote: &std::path::Path) -> String {
    let output = std::process::Command::new("git")
        .arg("ls-remote")
        .arg(remote)
        .arg("main")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout)
        .split_whitespace()
        .next()
        .unwrap()
        .to_string()
}

fn run_git(cwd: &std::path::Path, args: &[&str]) {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
