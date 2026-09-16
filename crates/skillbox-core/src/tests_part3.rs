use super::*;
use crate::test_support::*;
use std::fs;

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
fn change_skill_kind_moves_user_skill_to_remote_and_retargets_deployments() {
    let root = temp_dir("change-kind-user-to-remote");
    let source = root.join("source").join("agently-mail");
    let managed_root = root.join("SkillBox");
    let target_root = root.join("runtime");
    make_skill(&source, "agently-mail", "Mail skill");
    import_skill(&source, SkillKind::User, &managed_root).unwrap();
    let deployment = deploy_skill("agently-mail", &managed_root, &target_root).unwrap();

    let changed = change_skill_kind("agently-mail", SkillKind::Remote, &managed_root).unwrap();
    let current = fs::canonicalize(&managed_root)
        .unwrap()
        .join("remote-skills")
        .join("agently-mail")
        .join("current");
    let state = managed_state(&managed_root).unwrap();

    assert_eq!(changed.kind, SkillKind::Remote);
    assert!(!managed_root
        .join("user-skills")
        .join("agently-mail")
        .exists());
    assert!(changed.managed_path.parent().unwrap().ends_with("versions"));
    assert_eq!(fs::read_link(&current).unwrap(), changed.managed_path);
    assert_eq!(fs::read_link(&deployment.target_path).unwrap(), current);
    assert_eq!(state.skills.len(), 1);
    assert_eq!(state.skills[0].kind, SkillKind::Remote);
    assert!(state.skills[0].path.ends_with("current"));
}

#[test]
fn change_skill_kind_moves_remote_skill_to_user_and_retargets_deployments() {
    let root = temp_dir("change-kind-remote-to-user");
    let source = root.join("source").join("json-canvas");
    let managed_root = root.join("SkillBox");
    let target_root = root.join("runtime");
    make_skill(&source, "json-canvas", "Canvas skill");
    import_skill(&source, SkillKind::Remote, &managed_root).unwrap();
    let deployment = deploy_skill("json-canvas", &managed_root, &target_root).unwrap();

    let changed = change_skill_kind("json-canvas", SkillKind::User, &managed_root).unwrap();
    let canonical_managed_root = fs::canonicalize(&managed_root).unwrap();
    let user_path = canonical_managed_root
        .join("user-skills")
        .join("json-canvas");
    let current = canonical_managed_root
        .join("remote-skills")
        .join("json-canvas")
        .join("current");
    let state = managed_state(&managed_root).unwrap();

    assert_eq!(changed.kind, SkillKind::User);
    assert_eq!(changed.managed_path, user_path);
    assert!(changed.managed_path.join("SKILL.md").exists());
    assert!(fs::symlink_metadata(&current).is_err());
    assert_eq!(
        fs::read_link(&deployment.target_path).unwrap(),
        changed.managed_path
    );
    assert_eq!(state.skills.len(), 1);
    assert_eq!(state.skills[0].kind, SkillKind::User);
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
fn managed_preferences_persist_commit_summary_cli() {
    let root = temp_dir("preferences-commit-summary-cli");
    let managed_root = root.join("SkillBox");
    let cli = root.join("summarize");
    write_test_executable(
        &cli,
        "#!/bin/sh\ncat >/dev/null\necho 'feat(github): update alpha skill'\n",
    );

    let preferences =
        set_commit_summary_cli(&managed_root, cli.to_string_lossy().as_ref()).unwrap();

    assert_eq!(
        preferences.commit_summary_cli,
        cli.to_string_lossy().as_ref()
    );
    assert_eq!(
        preferences.resolved_commit_summary_cli,
        cli.to_string_lossy().as_ref()
    );

    let cleared = set_commit_summary_cli(&managed_root, "  ").unwrap();
    assert_eq!(cleared.commit_summary_cli, "");
    assert_eq!(cleared.resolved_commit_summary_cli, "");
}

#[test]
fn set_commit_summary_cli_rejects_command_lines() {
    let managed_root = temp_dir("preferences-commit-summary-cli-reject").join("SkillBox");
    let error = set_commit_summary_cli(&managed_root, "agent -p --mode ask").unwrap_err();
    assert!(error.contains("executable path only"));
}

#[test]
fn suggest_user_skills_commit_message_uses_heuristic_without_cli() {
    let managed_root = temp_dir("commit-summary-heuristic").join("SkillBox");
    let paths = ensure_managed_layout(&managed_root).unwrap();
    make_skill(
        &paths.user_skills_root.join("alpha"),
        "alpha",
        "Alpha skill",
    );

    let suggested = suggest_user_skills_commit_message(
        SuggestUserSkillsCommitRequest {
            selected_paths: Some(vec!["alpha/SKILL.md".to_string()]),
        },
        &managed_root,
    )
    .unwrap();

    assert_eq!(suggested.source, "heuristic");
    assert_eq!(suggested.message, "feat(github): add alpha skill");
    assert!(suggested.cli_path.is_none());
}

#[test]
fn suggest_user_skills_commit_message_runs_generic_cli_with_stdin() {
    let root = temp_dir("commit-summary-generic-cli");
    let managed_root = root.join("SkillBox");
    let paths = ensure_managed_layout(&managed_root).unwrap();
    make_skill(
        &paths.user_skills_root.join("alpha"),
        "alpha",
        "Alpha skill",
    );
    let cli = root.join("summarize");
    write_test_executable(
        &cli,
        "#!/bin/sh\ncat > \"$(dirname \"$0\")/stdin.txt\"\necho 'feat(github): refresh alpha prompts'\n",
    );
    set_commit_summary_cli(&managed_root, cli.to_string_lossy().as_ref()).unwrap();

    let suggested = suggest_user_skills_commit_message(
        SuggestUserSkillsCommitRequest {
            selected_paths: Some(vec!["alpha/SKILL.md".to_string()]),
        },
        &managed_root,
    )
    .unwrap();

    assert_eq!(suggested.source, "cli");
    assert_eq!(suggested.message, "feat(github): refresh alpha prompts");
    assert_eq!(
        suggested.cli_path.as_deref(),
        Some(cli.to_string_lossy().as_ref())
    );
    let stdin = fs::read_to_string(root.join("stdin.txt")).unwrap();
    assert!(stdin.contains("alpha/SKILL.md"));
    assert!(stdin.contains("Conventional Commit"));
    assert!(stdin.contains("Never write"));
    assert!(stdin.contains("Skill purpose:"));
}

#[test]
fn suggest_user_skills_commit_message_invokes_cursor_agent_with_read_only_flags() {
    let root = temp_dir("commit-summary-cursor-agent");
    let managed_root = root.join("SkillBox");
    let paths = ensure_managed_layout(&managed_root).unwrap();
    make_skill(
        &paths.user_skills_root.join("alpha"),
        "alpha",
        "Alpha skill",
    );
    let cli = root.join("agent");
    write_test_executable(
        &cli,
        r#"#!/bin/sh
print=0
mode=""
model=""
prev=""
for arg in "$@"; do
  if [ "$prev" = "--mode" ]; then
    mode="$arg"
  fi
  if [ "$prev" = "--model" ]; then
    model="$arg"
  fi
  if [ "$arg" = "--print" ] || [ "$arg" = "-p" ]; then
    print=1
  fi
  prev="$arg"
done
printf '%s\n' "$@" > "$(dirname "$0")/args.txt"
if [ "$print" != 1 ] || [ "$mode" != "ask" ] || [ "$model" != "cursor-grok-4.6-high-fast" ]; then
  echo "missing read-only flags" >&2
  exit 1
fi
echo 'feat(github): add alpha for local prompt fixtures'
"#,
    );
    set_commit_summary_cli(&managed_root, cli.to_string_lossy().as_ref()).unwrap();

    let suggested = suggest_user_skills_commit_message(
        SuggestUserSkillsCommitRequest {
            selected_paths: Some(vec!["alpha/SKILL.md".to_string()]),
        },
        &managed_root,
    )
    .unwrap();

    assert_eq!(suggested.source, "cli");
    assert_eq!(
        suggested.message,
        "feat(github): add alpha for local prompt fixtures"
    );
    let args = fs::read_to_string(root.join("args.txt")).unwrap();
    assert!(args.contains("--print"));
    assert!(args.contains("--mode\nask") || args.contains("--mode"));
    assert!(args.contains("ask"));
    assert!(args.contains("--sandbox"));
    assert!(args.contains("enabled"));
    assert!(args.contains("--model"));
    assert!(args.contains("cursor-grok-4.6-high-fast"));
    assert!(!args.contains("--force"));
    assert!(!args.contains("--yolo"));
}

#[test]
fn suggest_user_skills_commit_message_replaces_generic_cli_output() {
    let root = temp_dir("commit-summary-generic-rewrite");
    let managed_root = root.join("SkillBox");
    let paths = ensure_managed_layout(&managed_root).unwrap();
    make_skill(
        &paths.user_skills_root.join("content-title-optimizer"),
        "content-title-optimizer",
        "Generate, compare, and revise evidence-grounded titles from a topic",
    );
    let cli = root.join("summarize");
    write_test_executable(
        &cli,
        "#!/bin/sh\necho 'feat(github): add content-title-optimizer skill'\n",
    );
    set_commit_summary_cli(&managed_root, cli.to_string_lossy().as_ref()).unwrap();

    let suggested = suggest_user_skills_commit_message(
        SuggestUserSkillsCommitRequest {
            selected_paths: Some(vec!["content-title-optimizer/SKILL.md".to_string()]),
        },
        &managed_root,
    )
    .unwrap();

    assert_eq!(suggested.source, "cli");
    assert_eq!(
        suggested.message,
        "feat(github): add content-title-optimizer to generate, compare, and revise evidence-grounded titles from a topic"
    );
}

#[test]
fn suggest_user_skills_commit_message_times_out_slow_cli() {
    let root = temp_dir("commit-summary-timeout");
    let managed_root = root.join("SkillBox");
    let paths = ensure_managed_layout(&managed_root).unwrap();
    make_skill(
        &paths.user_skills_root.join("alpha"),
        "alpha",
        "Alpha skill",
    );
    let cli = root.join("slow");
    write_test_executable(&cli, "#!/bin/sh\nsleep 8\n");
    set_commit_summary_cli(&managed_root, cli.to_string_lossy().as_ref()).unwrap();

    let error = suggest_user_skills_commit_message(
        SuggestUserSkillsCommitRequest {
            selected_paths: Some(vec!["alpha/SKILL.md".to_string()]),
        },
        &managed_root,
    )
    .unwrap_err();

    assert!(error.contains("timed out"));
}

#[test]
fn app_update_check_cache_round_trips_through_preferences() {
    let root = temp_dir("app-update-cache");
    let managed_root = root.join("SkillBox");
    let cache = AppUpdateCheckCache {
        current_version: "0.4.5".to_string(),
        available: true,
        version: "0.5.0".to_string(),
        date: "2026-07-25T10:00:00Z".to_string(),
        body: "Daily update reminders.".to_string(),
        checked_at: "1784954400".to_string(),
        message: "App update available.".to_string(),
    };

    assert_eq!(cached_app_update_check(&managed_root).unwrap(), None);
    cache_app_update_check(&managed_root, &cache).unwrap();

    assert_eq!(cached_app_update_check(&managed_root).unwrap(), Some(cache));
}

#[test]
fn app_update_check_cache_rejects_corrupt_json() {
    let root = temp_dir("app-update-cache-corrupt");
    let managed_root = root.join("SkillBox");
    let paths = ensure_managed_layout(&managed_root).unwrap();
    let connection = open_database(&paths.database_path).unwrap();
    connection
        .execute(
            "INSERT INTO preferences (key, value) VALUES ('app_update_check_cache', '{broken')",
            [],
        )
        .unwrap();

    assert!(cached_app_update_check(&managed_root).is_err());
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
    record_test_call(
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
fn history_kind_query_finds_older_references_beyond_mixed_page_limit() {
    let root = temp_dir("history-kind-query-limit");
    let managed_root = root.join("SkillBox");
    let runtime_root = root.join("project").join(".codex").join("skills");
    fs::create_dir_all(&runtime_root).unwrap();

    for index in 0..205 {
        record_test_call(
            RecordSkillUsageRequest {
                skill_name: format!("recent-call-{index}"),
                agent_id: "codex".to_string(),
                runtime_root: runtime_root.clone(),
                event_id: Some(format!("recent-call-event-{index}")),
                used_at: Some("2026-07-31T23:59:00Z".to_string()),
                prompt_excerpt: None,
                metadata: Some(serde_json::json!({"source": "agent_hook"})),
            },
            &managed_root,
        )
        .unwrap();
    }
    record_skill_usage(
        RecordSkillUsageRequest {
            skill_name: "older-reference".to_string(),
            agent_id: "codex".to_string(),
            runtime_root: runtime_root.clone(),
            event_id: Some("older-reference-event".to_string()),
            used_at: Some("2026-07-01T00:00:00Z".to_string()),
            prompt_excerpt: None,
            metadata: None,
        },
        &managed_root,
    )
    .unwrap();

    let all_history = list_history(
        HistoryFilter {
            limit: Some(200),
            ..HistoryFilter::default()
        },
        &managed_root,
    )
    .unwrap();
    let references = list_history(
        HistoryFilter {
            kind: Some(HistoryEntryKind::UsageReference),
            limit: Some(200),
        },
        &managed_root,
    )
    .unwrap();

    assert_eq!(all_history.skill_usage_count, 205);
    assert_eq!(all_history.skill_reference_count, 1);
    assert_eq!(all_history.entries.len(), 200);
    assert!(all_history
        .entries
        .iter()
        .all(|entry| entry.kind == HistoryEntryKind::SkillUsage));
    assert_eq!(references.skill_usage_count, 205);
    assert_eq!(references.skill_reference_count, 1);
    assert_eq!(references.entries.len(), 1);
    assert_eq!(
        references.entries[0].skill_name.as_deref(),
        Some("older-reference")
    );
    assert_eq!(references.entries[0].kind, HistoryEntryKind::UsageReference);
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

    let operations = list_operations(
        OperationFilter {
            entity_name: Some("user-skills".to_string()),
            ..OperationFilter::default()
        },
        &managed_root,
    )
    .unwrap();
    let sync_operation = operations
        .operations
        .iter()
        .find(|operation| operation.operation_type == "sync_user_skills_git")
        .unwrap();
    assert_eq!(sync_operation.status, OperationStatus::Failed);
    assert!(sync_operation
        .error
        .as_deref()
        .is_some_and(|error| error.contains("push")));
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
fn user_skill_new_file_diff_inlines_text_under_one_megabyte() {
    let root = temp_dir("user-skill-large-text-diff");
    fs::create_dir_all(&root).unwrap();
    let content = "large text line\n".repeat(9_000);
    fs::write(root.join("large.txt"), &content).unwrap();

    let diff = new_file_diff(&root, "large.txt").unwrap();

    assert!(!diff.contains("Diff omitted"));
    assert!(diff.contains("+large text line"));
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
    let source_url = github_source_url("acme", "install-github-remote", "find-skills");
    let preview = github_install_preview(&source_url, None, &managed_root);

    assert_eq!(preview.skill_name, "find-skills");
    assert_eq!(preview.installed_sha, installed_sha);
    assert!(preview.files.iter().any(|file| file.path == "SKILL.md"));
    assert!(!managed_root.exists());
    assert!(!managed_root
        .join("remote-skills")
        .join("find-skills")
        .exists());

    let result = install_github_remote_skill(
        InstallGithubRemoteSkillRequest {
            source_url,
            target_root: None,
            preview_id: Some(preview.preview_id),
            confirm_warnings: false,
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
fn install_github_root_skill_previews_installs_indexes_and_deploys_sanitized_worktree() {
    let root = temp_dir("install-github-root-skill");
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
    let (remote, _work) = bare_remote_with_root_skill_content(
        "install-github-root-skill-origin",
        "humanizer-zh",
        "Humanizer zh",
        "Original body\n",
    );
    let installed_sha = remote_head(&remote);
    let _rewrite = github_repo_rewrite("acme", "install-github-root-skill", &remote);
    let source_url =
        "https://github.com/acme/install-github-root-skill/blob/main/SKILL.md".to_string();
    let preview = github_install_preview(&source_url, Some(target_root.clone()), &managed_root);

    assert_eq!(preview.skill_name, "humanizer-zh");
    assert!(preview.root);
    assert_eq!(preview.path, "");
    assert_eq!(
        preview.source_url,
        "https://github.com/acme/install-github-root-skill/tree/main"
    );
    for expected in ["SKILL.md", "README.md", "assets/prompt.txt"] {
        assert!(preview.files.iter().any(|file| file.path == expected));
    }
    assert!(!preview
        .files
        .iter()
        .any(|file| file.path == ".git" || file.path.starts_with(".git/")));
    assert_eq!(
        preview.compatibility.as_ref().unwrap().status,
        CompatibilityStatus::Compatible
    );
    assert!(!managed_root.join("remote-skills/humanizer-zh").exists());
    assert!(!target_root.join("humanizer-zh").exists());

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

    let paths = managed_paths(&managed_root);
    let remote_root = paths.remote_skills_root.join("humanizer-zh");
    let version_path = remote_root.join("versions").join(&installed_sha);
    assert!(result.root);
    assert_eq!(result.path, "");
    assert_eq!(result.version_path, version_path);
    assert!(version_path.join("SKILL.md").exists());
    assert!(version_path.join("README.md").exists());
    assert!(version_path.join("assets/prompt.txt").exists());
    assert!(!version_path.join(".git").exists());
    assert_eq!(
        fs::canonicalize(remote_root.join("current")).unwrap(),
        fs::canonicalize(&version_path).unwrap()
    );
    let deployment = result.deployment.unwrap();
    assert_eq!(
        deployment.target_root,
        fs::canonicalize(&target_root).unwrap()
    );
    assert_eq!(
        fs::canonicalize(deployment.target_path).unwrap(),
        fs::canonicalize(remote_root.join("current")).unwrap()
    );

    let source_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(remote_root.join("source.json")).unwrap())
            .unwrap();
    assert_eq!(source_json["root"], true);
    assert_eq!(source_json["path"], "");
    assert_eq!(source_json["currentVersion"], installed_sha);
    let round_trip = read_remote_source(&remote_root).unwrap();
    assert!(round_trip.root);
    assert_eq!(round_trip.path.as_deref(), Some(""));

    let connection = open_database(&paths.database_path).unwrap();
    let indexed_path: String = connection
        .query_row(
            "SELECT managed_path FROM skills WHERE name = 'humanizer-zh'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(indexed_path, version_path.to_string_lossy());
}

#[test]
fn install_github_root_skill_rejects_preview_after_branch_advances() {
    let root = temp_dir("install-github-root-skill-stale");
    let managed_root = root.join("SkillBox");
    let (remote, work) = bare_remote_with_root_skill_content(
        "install-github-root-skill-stale-origin",
        "humanizer-zh",
        "Humanizer zh",
        "Original body\n",
    );
    let _rewrite = github_repo_rewrite("acme", "install-github-root-skill-stale", &remote);
    let source_url =
        "https://github.com/acme/install-github-root-skill-stale/blob/main/SKILL.md".to_string();
    let preview = github_install_preview(&source_url, None, &managed_root);

    make_skill_with_body(&work, "humanizer-zh", "Humanizer zh", "Advanced body\n");
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
            "Advance root skill",
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
    let remote_root = paths.remote_skills_root.join("humanizer-zh");
    assert!(error.contains("Remote install preview is stale"));
    assert!(!remote_root.join("versions").exists());
    assert!(!remote_root.join("current").exists());
    assert!(!remote_root.join("source.json").exists());
    let connection = open_database(&paths.database_path).unwrap();
    let indexed = connection
        .query_row(
            "SELECT name FROM skills WHERE name = 'humanizer-zh'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .unwrap();
    assert_eq!(indexed, None);
}

#[test]
fn github_collection_preview_and_apply_selects_children_at_one_reviewed_sha() {
    let root = temp_dir("github-collection-apply");
    let managed_root = root.join("SkillBox");
    let (remote, _work) = bare_remote_with_multiple_skill_content(
        "github-collection-apply-origin",
        &["alpha", "beta"],
    );
    let _rewrite = github_repo_rewrite("acme", "github-collection-apply", &remote);
    let source_url = "https://github.com/acme/github-collection-apply/tree/main";
    let preview = preview_github_skill_collection(
        PreviewGithubSkillCollectionRequest {
            source_url: source_url.to_string(),
        },
        &managed_root,
    )
    .unwrap();
    assert_eq!(preview.diagnostics.fetch_count, 1);
    assert_eq!(preview.collection.children.len(), 2);
    let reviewed_sha = preview.collection.reviewed_head_sha.clone().unwrap();
    let selections = preview
        .collection
        .children
        .iter()
        .map(|child| ImportCollectionChildSelection {
            relative_path: child.relative_path.clone(),
            group_id: child.group_id.clone(),
            variant_id: child.variant_id.clone(),
            skill_type: SkillKind::User,
        })
        .collect();

    let result = apply_github_skill_collection(
        GithubSkillCollectionApplyRequest {
            source_url: source_url.to_string(),
            collection_id: preview.collection.id,
            preview_id: preview.collection.preview_id,
            selections,
            actor: "test".to_string(),
        },
        &managed_root,
    )
    .unwrap();

    assert_eq!(result.imported.len(), 2);
    assert_eq!(
        result.collection.source_kind,
        ImportCandidateCollectionSourceKind::GithubRemote
    );
    assert_eq!(
        result.collection.reviewed_head_sha.as_deref(),
        Some(reviewed_sha.as_str())
    );
    let paths = managed_paths(&managed_root);
    assert!(paths.user_skills_root.join("alpha/SKILL.md").exists());
    assert!(paths.user_skills_root.join("beta/SKILL.md").exists());
    let connection = open_database(&paths.database_path).unwrap();
    let (source_kind, source_url, member_count): (String, String, i64) = connection
        .query_row(
            "SELECT c.source_kind, c.source_url, (SELECT COUNT(*) FROM skill_collection_members m WHERE m.collection_id = c.id) FROM skill_collections c WHERE c.id = ?1",
            [&result.collection.id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(source_kind, "github_remote");
    assert_eq!(
        source_url,
        "https://github.com/acme/github-collection-apply/tree/main"
    );
    assert_eq!(member_count, 2);
}

#[test]
fn github_collection_incremental_apply_preserves_members_at_one_reviewed_sha() {
    let root = temp_dir("github-collection-incremental-apply");
    let managed_root = root.join("SkillBox");
    let (remote, work) = bare_remote_with_multiple_skill_content(
        "github-collection-incremental-apply-origin",
        &["alpha", "beta"],
    );
    let _rewrite = github_repo_rewrite("acme", "github-collection-incremental-apply", &remote);
    let source_url = "https://github.com/acme/github-collection-incremental-apply/tree/main";

    let first = preview_github_skill_collection(
        PreviewGithubSkillCollectionRequest {
            source_url: source_url.to_string(),
        },
        &managed_root,
    )
    .unwrap();
    let reviewed_sha = first.collection.reviewed_head_sha.clone();
    let alpha = first
        .collection
        .children
        .iter()
        .find(|child| child.name == "alpha")
        .unwrap()
        .clone();
    let first_result = apply_github_skill_collection(
        GithubSkillCollectionApplyRequest {
            source_url: source_url.to_string(),
            collection_id: first.collection.id.clone(),
            preview_id: first.collection.preview_id,
            selections: vec![ImportCollectionChildSelection {
                relative_path: alpha.relative_path.clone(),
                group_id: alpha.group_id,
                variant_id: alpha.variant_id,
                skill_type: SkillKind::User,
            }],
            actor: "test".to_string(),
        },
        &managed_root,
    )
    .unwrap();
    assert_eq!(first_result.collection.members.len(), 1);

    let second = preview_github_skill_collection(
        PreviewGithubSkillCollectionRequest {
            source_url: source_url.to_string(),
        },
        &managed_root,
    )
    .unwrap();
    assert_eq!(second.collection.reviewed_head_sha, reviewed_sha);
    let beta = second
        .collection
        .children
        .iter()
        .find(|child| child.name == "beta")
        .unwrap()
        .clone();
    let second_result = apply_github_skill_collection(
        GithubSkillCollectionApplyRequest {
            source_url: source_url.to_string(),
            collection_id: second.collection.id.clone(),
            preview_id: second.collection.preview_id,
            selections: vec![ImportCollectionChildSelection {
                relative_path: beta.relative_path,
                group_id: beta.group_id,
                variant_id: beta.variant_id,
                skill_type: SkillKind::User,
            }],
            actor: "test".to_string(),
        },
        &managed_root,
    )
    .unwrap();

    assert_eq!(second_result.collection.members.len(), 2);
    assert_eq!(
        list_skill_collections(&managed_root).unwrap()[0]
            .members
            .len(),
        2
    );

    fs::write(
        work.join("skills/beta/SKILL.md"),
        "---\nname: beta\ndescription: Changed\n---\n\n# Changed\n",
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
            "Advance collection",
        ],
    );
    run_git(&work, &["push", "origin", "main"]);
    let third = preview_github_skill_collection(
        PreviewGithubSkillCollectionRequest {
            source_url: source_url.to_string(),
        },
        &managed_root,
    )
    .unwrap();
    let changed_beta = third
        .collection
        .children
        .iter()
        .find(|child| child.name == "beta")
        .unwrap()
        .clone();
    let error = apply_github_skill_collection(
        GithubSkillCollectionApplyRequest {
            source_url: source_url.to_string(),
            collection_id: third.collection.id,
            preview_id: third.collection.preview_id,
            selections: vec![ImportCollectionChildSelection {
                relative_path: changed_beta.relative_path,
                group_id: changed_beta.group_id,
                variant_id: changed_beta.variant_id,
                skill_type: SkillKind::User,
            }],
            actor: "test".to_string(),
        },
        &managed_root,
    )
    .unwrap_err();
    assert!(
        error.contains("different reviewed SHA") || error.contains("Managed target"),
        "{error}"
    );
    assert_eq!(
        list_skill_collections(&managed_root).unwrap()[0]
            .members
            .len(),
        2
    );
}

#[test]
fn github_collection_identity_is_stable_across_commits_on_one_ref() {
    let root = temp_dir("github-collection-stable-identity");
    let managed_root = root.join("SkillBox");
    let (remote, work) = bare_remote_with_multiple_skill_content(
        "github-collection-stable-identity-origin",
        &["alpha", "beta"],
    );
    let _rewrite = github_repo_rewrite("acme", "github-collection-stable-identity", &remote);
    let source_url =
        "https://github.com/acme/github-collection-stable-identity/tree/main".to_string();

    let first = preview_github_skill_collection(
        PreviewGithubSkillCollectionRequest {
            source_url: source_url.clone(),
        },
        &managed_root,
    )
    .unwrap();
    fs::write(
        work.join("skills/alpha/SKILL.md"),
        "---\nname: alpha\ndescription: Changed\n---\n\n# Changed\n",
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
            "Change alpha",
        ],
    );
    run_git(&work, &["push", "origin", "main"]);
    let second = preview_github_skill_collection(
        PreviewGithubSkillCollectionRequest { source_url },
        &managed_root,
    )
    .unwrap();

    assert_eq!(first.collection.id, second.collection.id);
    assert_ne!(first.collection.preview_id, second.collection.preview_id);
    assert_ne!(
        first.collection.reviewed_head_sha,
        second.collection.reviewed_head_sha
    );
}

#[test]
fn collection_batch_failure_rolls_back_remote_current_index_and_audits_one_operation() {
    let root = temp_dir("collection-batch-failure-audit");
    let repository = root.join("collection");
    let managed_root = root.join("SkillBox");
    fs::create_dir_all(&repository).unwrap();
    run_git(&repository, &["init", "-b", "main"]);
    make_skill(&repository.join("skills/alpha"), "alpha", "Alpha");
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
            "Collection",
        ],
    );

    let scan = scan_import_candidates(std::slice::from_ref(&repository), &managed_root).unwrap();
    let mut collection = scan.collections.into_iter().next().unwrap();
    let child = collection.children[0].clone();
    collection.source_kind = ImportCandidateCollectionSourceKind::GithubRemote;
    collection.source_url = Some("https://github.com/acme/collection/tree/main".to_string());
    collection.requested_reference = Some("main".to_string());
    let paths = ensure_managed_layout(managed_root.clone()).unwrap();
    let missing = repository.join("skills/missing");
    let external_missing_root = paths.remote_skills_root.join("missing");
    make_skill(
        &external_missing_root.join("versions/external"),
        "missing",
        "External content that rollback must preserve",
    );
    let error = apply_collection_import_with_audit(
        &paths,
        &collection,
        std::slice::from_ref(&child),
        vec![
            ImportRequestItem {
                source_path: child.source_path.clone(),
                skill_type: SkillKind::Remote,
                deploy_back_to_source: false,
            },
            ImportRequestItem {
                source_path: missing,
                skill_type: SkillKind::Remote,
                deploy_back_to_source: false,
            },
        ],
        "test",
    )
    .unwrap_err();

    assert!(error.contains("Collection import did not complete"));
    assert!(!paths.remote_skills_root.join("alpha").exists());
    assert!(external_missing_root
        .join("versions/external/SKILL.md")
        .is_file());
    let operations = list_operations(OperationFilter::default(), &managed_root)
        .unwrap()
        .operations;
    let collection_operations = operations
        .iter()
        .filter(|operation| operation.operation_type == "import_collection")
        .collect::<Vec<_>>();
    assert_eq!(collection_operations.len(), 1);
    assert!(operations
        .iter()
        .all(|operation| operation.operation_type != "import_candidate"));
    assert_eq!(collection_operations[0].status, OperationStatus::Failed);
    assert_eq!(
        collection_operations[0]
            .payload
            .get("partialRecovery")
            .and_then(serde_json::Value::as_bool),
        Some(false)
    );
    assert!(list_skill_collections(&managed_root).unwrap().is_empty());
}

#[cfg(unix)]
#[test]
fn collection_batch_rollback_preserves_same_content_identity_replacement() {
    let root = temp_dir("collection-batch-identity-replacement");
    let repository = root.join("collection");
    let managed_root = root.join("SkillBox");
    fs::create_dir_all(&repository).unwrap();
    run_git(&repository, &["init", "-b", "main"]);
    make_skill(&repository.join("skills/alpha"), "alpha", "Alpha");
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
            "Collection",
        ],
    );

    let scan = scan_import_candidates(std::slice::from_ref(&repository), &managed_root).unwrap();
    let mut collection = scan.collections.into_iter().next().unwrap();
    let child = collection.children[0].clone();
    collection.source_kind = ImportCandidateCollectionSourceKind::GithubRemote;
    collection.source_url = Some("https://github.com/acme/collection/tree/main".to_string());
    collection.requested_reference = Some("main".to_string());
    let paths = ensure_managed_layout(managed_root.clone()).unwrap();
    let missing = repository.join("skills/missing");
    let _hook = install_collection_import_test_hook(|imported| {
        assert_eq!(imported.len(), 1);
        let imported_path = &imported[0].managed_path;
        let replacement_path = imported_path.with_file_name("external-replacement");
        let skill_md = fs::read(imported_path.join("SKILL.md")).unwrap();
        fs::rename(imported_path, &replacement_path).unwrap();
        fs::create_dir_all(imported_path).unwrap();
        fs::write(imported_path.join("SKILL.md"), skill_md).unwrap();

        let current = imported_path
            .parent()
            .and_then(Path::parent)
            .unwrap()
            .join("current");
        fs::remove_file(&current).unwrap();
        std::os::unix::fs::symlink(&replacement_path, &current).unwrap();
    });

    let error = apply_collection_import_with_audit(
        &paths,
        &collection,
        std::slice::from_ref(&child),
        vec![
            ImportRequestItem {
                source_path: child.source_path.clone(),
                skill_type: SkillKind::Remote,
                deploy_back_to_source: false,
            },
            ImportRequestItem {
                source_path: missing,
                skill_type: SkillKind::Remote,
                deploy_back_to_source: false,
            },
        ],
        "test",
    )
    .unwrap_err();

    assert!(error.contains("filesystem identity changed"), "{error}");
    let replacement_path = paths
        .remote_skills_root
        .join("alpha/versions/external-replacement");
    assert!(replacement_path.join("SKILL.md").is_file());
    let current = paths.remote_skills_root.join("alpha/current");
    assert_eq!(
        fs::canonicalize(current).unwrap(),
        fs::canonicalize(replacement_path).unwrap()
    );
    let operations = list_operations(OperationFilter::default(), &managed_root)
        .unwrap()
        .operations;
    let collection_operation = operations
        .iter()
        .find(|operation| operation.operation_type == "import_collection")
        .unwrap();
    assert_eq!(collection_operation.status, OperationStatus::Failed);
    assert_eq!(
        collection_operation
            .payload
            .get("partialRecovery")
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert!(list_skill_collections(&managed_root).unwrap().is_empty());
}

#[test]
fn github_collection_apply_rejects_new_sha_before_new_child_writes() {
    let root = temp_dir("github-collection-new-child-sha");
    let managed_root = root.join("SkillBox");
    let (remote, work) = bare_remote_with_multiple_skill_content(
        "github-collection-new-child-sha-origin",
        &["alpha", "beta"],
    );
    let _rewrite = github_repo_rewrite("acme", "github-collection-new-child-sha", &remote);
    let source_url = "https://github.com/acme/github-collection-new-child-sha/tree/main";

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
    let first_result = apply_github_skill_collection(
        GithubSkillCollectionApplyRequest {
            source_url: source_url.to_string(),
            collection_id: first.collection.id.clone(),
            preview_id: first.collection.preview_id,
            selections: vec![ImportCollectionChildSelection {
                relative_path: alpha.relative_path,
                group_id: alpha.group_id,
                variant_id: alpha.variant_id,
                skill_type: SkillKind::User,
            }],
            actor: "test".to_string(),
        },
        &managed_root,
    )
    .unwrap();
    let operations_before = list_operations(OperationFilter::default(), &managed_root)
        .unwrap()
        .operations
        .len();

    make_skill(&work.join("skills/charlie"), "charlie", "Charlie");
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
            "Add Charlie",
        ],
    );
    run_git(&work, &["push", "origin", "main"]);

    let second = preview_github_skill_collection(
        PreviewGithubSkillCollectionRequest {
            source_url: source_url.to_string(),
        },
        &managed_root,
    )
    .unwrap();
    assert_ne!(
        first_result.collection.reviewed_head_sha,
        second.collection.reviewed_head_sha
    );
    let charlie = second
        .collection
        .children
        .iter()
        .find(|child| child.name == "charlie")
        .unwrap()
        .clone();
    let error = apply_github_skill_collection(
        GithubSkillCollectionApplyRequest {
            source_url: source_url.to_string(),
            collection_id: second.collection.id,
            preview_id: second.collection.preview_id,
            selections: vec![ImportCollectionChildSelection {
                relative_path: charlie.relative_path,
                group_id: charlie.group_id,
                variant_id: charlie.variant_id,
                skill_type: SkillKind::User,
            }],
            actor: "test".to_string(),
        },
        &managed_root,
    )
    .unwrap_err();

    assert!(error.contains("different reviewed SHA"), "{error}");
    assert!(!managed_root.join("user-skills/charlie").exists());
    let paths = managed_paths(&managed_root);
    assert!(!managed_index_contains(&paths.database_path, "charlie").unwrap());
    assert_eq!(
        list_skill_collections(&managed_root).unwrap()[0]
            .members
            .iter()
            .map(|member| member.managed_skill_name.as_str())
            .collect::<Vec<_>>(),
        vec!["alpha"]
    );
    assert_eq!(
        list_operations(OperationFilter::default(), &managed_root)
            .unwrap()
            .operations
            .len(),
        operations_before
    );
}

#[test]
fn github_collection_apply_rejects_new_remote_sha_before_managed_writes() {
    let root = temp_dir("github-collection-stale");
    let managed_root = root.join("SkillBox");
    let (remote, work) = bare_remote_with_multiple_skill_content(
        "github-collection-stale-origin",
        &["alpha", "beta"],
    );
    let _rewrite = github_repo_rewrite("acme", "github-collection-stale", &remote);
    let source_url = "https://github.com/acme/github-collection-stale/tree/main";
    let preview = preview_github_skill_collection(
        PreviewGithubSkillCollectionRequest {
            source_url: source_url.to_string(),
        },
        &managed_root,
    )
    .unwrap();
    fs::write(
        work.join("skills/alpha/SKILL.md"),
        "---\nname: alpha\ndescription: Changed\n---\n",
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
            "Advance collection",
        ],
    );
    run_git(&work, &["push", "origin", "main"]);
    let selections = preview
        .collection
        .children
        .iter()
        .map(|child| ImportCollectionChildSelection {
            relative_path: child.relative_path.clone(),
            group_id: child.group_id.clone(),
            variant_id: child.variant_id.clone(),
            skill_type: SkillKind::User,
        })
        .collect();

    let error = apply_github_skill_collection(
        GithubSkillCollectionApplyRequest {
            source_url: source_url.to_string(),
            collection_id: preview.collection.id,
            preview_id: preview.collection.preview_id,
            selections,
            actor: "test".to_string(),
        },
        &managed_root,
    )
    .unwrap_err();

    assert!(error.contains("stale"));
    assert!(!managed_root.join("user-skills/alpha").exists());
    assert!(!managed_root.join("user-skills/beta").exists());
    assert!(!managed_root.join("skillbox.sqlite").exists());
}

#[test]
fn github_collection_update_preview_classifies_unchanged_updated_added_and_removed() {
    let root = temp_dir("github-collection-update-classify");
    let managed_root = root.join("SkillBox");
    let (remote, work) = bare_remote_with_multiple_skill_content(
        "github-collection-update-classify-origin",
        &["alpha", "beta"],
    );
    let _rewrite = github_repo_rewrite("acme", "github-collection-update-classify", &remote);
    let source_url = "https://github.com/acme/github-collection-update-classify/tree/main";
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

    fs::write(
        work.join("skills/alpha/SKILL.md"),
        "---\nname: alpha\ndescription: Updated alpha\n---\n\n# Alpha updated\n",
    )
    .unwrap();
    fs::remove_dir_all(work.join("skills/beta")).unwrap();
    make_skill(&work.join("skills/gamma"), "gamma", "Gamma");
    commit_and_push_collection(&work, "Change collection tree");

    let preview = github_update_preview(source_url, &managed_root);
    let kinds: Vec<_> = preview
        .changes
        .iter()
        .map(|change| (change.name.as_str(), change.change))
        .collect();
    assert!(kinds.contains(&("alpha", GithubCollectionChildChangeKind::Updated)));
    assert!(kinds.contains(&("beta", GithubCollectionChildChangeKind::Removed)));
    assert!(kinds.contains(&("gamma", GithubCollectionChildChangeKind::Added)));
    assert!(
        preview
            .changes
            .iter()
            .find(|change| change.name == "alpha")
            .unwrap()
            .required
    );
    assert!(preview.collection.children.iter().any(|child| {
        child.name == "beta"
            && child
                .conflict
                .as_deref()
                .is_some_and(|text| text.contains("not deleted"))
    }));
    let updated_alpha = preview
        .collection
        .children
        .iter()
        .find(|child| child.name == "alpha")
        .unwrap()
        .clone();
    let deselect_error = apply_github_skill_collection_update(
        GithubSkillCollectionApplyRequest {
            source_url: source_url.to_string(),
            collection_id: preview.collection_id,
            preview_id: preview.preview_id,
            selections: vec![],
            actor: "test".to_string(),
        },
        &managed_root,
    )
    .unwrap_err();
    assert!(
        deselect_error.contains("Select every updated collection member"),
        "{deselect_error}"
    );
    let _ = updated_alpha;
}
