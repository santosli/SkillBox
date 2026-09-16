use super::*;
use crate::test_support::*;
use std::fs;

#[test]
fn workspace_setup_creates_only_selected_root_and_registers_it() {
    let root = temp_dir("workspace-setup-create");
    let project = root.join("project");
    let managed_root = root.join("SkillBox");
    fs::create_dir_all(&project).unwrap();
    let preview = preview_workspace_setup(
        WorkspaceSetupPreviewRequest {
            selected_path: project.clone(),
            kind: WorkspaceKind::User,
        },
        &managed_root,
    )
    .unwrap();
    let selected = preview
        .roots
        .iter()
        .find(|root| root.relative_path == ".codex/skills")
        .unwrap();

    let result = apply_workspace_setup(
        WorkspaceSetupApplyRequest {
            selected_path: project.clone(),
            kind: WorkspaceKind::User,
            selected_root: selected.path.clone(),
            create_missing: true,
            preview_id: preview.preview_id,
        },
        &managed_root,
    )
    .unwrap();

    assert_eq!(
        result.created_path.as_deref(),
        Some(selected.path.as_path())
    );
    assert!(project.join(".codex/skills").is_dir());
    assert!(!project.join(".agents").exists());
    assert!(!project.join(".claude").exists());
    assert_eq!(result.workspace.agent_id.as_deref(), Some("codex"));
    assert_eq!(result.workspace.kind, WorkspaceKind::User);
    let operations = list_operations(OperationFilter::default(), &managed_root).unwrap();
    assert!(operations.operations.iter().any(|operation| {
        operation.operation_type == "add_workspace"
            && operation.status == OperationStatus::Succeeded
    }));
}

#[test]
fn workspace_setup_rejects_stale_and_tampered_preview_selections() {
    let root = temp_dir("workspace-setup-stale");
    let project = root.join("project");
    let managed_root = root.join("SkillBox");
    fs::create_dir_all(&project).unwrap();
    let preview = preview_workspace_setup(
        WorkspaceSetupPreviewRequest {
            selected_path: project.clone(),
            kind: WorkspaceKind::User,
        },
        &managed_root,
    )
    .unwrap();

    let stale = apply_workspace_setup(
        WorkspaceSetupApplyRequest {
            selected_path: project.clone(),
            kind: WorkspaceKind::User,
            selected_root: preview.roots[0].path.clone(),
            create_missing: true,
            preview_id: format!("{}-stale", preview.preview_id),
        },
        &managed_root,
    )
    .unwrap_err();
    assert!(stale.contains("preview is stale"));

    let tampered = apply_workspace_setup(
        WorkspaceSetupApplyRequest {
            selected_path: project.clone(),
            kind: WorkspaceKind::User,
            selected_root: project.join("../outside/skills"),
            create_missing: true,
            preview_id: preview.preview_id,
        },
        &managed_root,
    )
    .unwrap_err();
    assert!(tampered.contains("not part of this workspace preview"));
    assert!(!project.join(".agents").exists());
}

#[test]
fn workspace_setup_rejects_preview_after_project_directory_is_replaced() {
    let root = temp_dir("workspace-setup-replaced-project");
    let project = root.join("project");
    let managed_root = root.join("SkillBox");
    fs::create_dir_all(&project).unwrap();
    let preview = preview_workspace_setup(
        WorkspaceSetupPreviewRequest {
            selected_path: project.clone(),
            kind: WorkspaceKind::User,
        },
        &managed_root,
    )
    .unwrap();
    let selected = preview.roots[0].clone();

    fs::rename(&project, root.join("old-project")).unwrap();
    fs::create_dir_all(&project).unwrap();
    let error = apply_workspace_setup(
        WorkspaceSetupApplyRequest {
            selected_path: project.clone(),
            kind: WorkspaceKind::User,
            selected_root: selected.path,
            create_missing: true,
            preview_id: preview.preview_id,
        },
        &managed_root,
    )
    .unwrap_err();

    assert!(error.contains("preview is stale"));
    assert!(!project.join(".agents").exists());
}

#[test]
fn workspace_setup_rejects_symlink_escape_and_non_directory_target() {
    let root = temp_dir("workspace-setup-unsafe");
    let project = root.join("project");
    let outside = root.join("outside");
    fs::create_dir_all(&project).unwrap();
    fs::create_dir_all(&outside).unwrap();
    std::os::unix::fs::symlink(&outside, project.join(".agents")).unwrap();

    let symlink_error = preview_workspace_setup(
        WorkspaceSetupPreviewRequest {
            selected_path: project.clone(),
            kind: WorkspaceKind::User,
        },
        root.join("SkillBox"),
    )
    .unwrap_err();
    assert!(symlink_error.contains("cannot be a symlink"));

    fs::remove_file(project.join(".agents")).unwrap();
    fs::create_dir_all(project.join(".codex")).unwrap();
    fs::write(project.join(".codex/skills"), "not a directory").unwrap();
    let file_error = preview_workspace_setup(
        WorkspaceSetupPreviewRequest {
            selected_path: project,
            kind: WorkspaceKind::User,
        },
        root.join("SkillBox"),
    )
    .unwrap_err();
    assert!(file_error.contains("not a directory"));
}

#[test]
fn workspace_setup_rejects_unreadable_project_directory() {
    let root = temp_dir("workspace-setup-unreadable");
    let project = root.join("project");
    fs::create_dir_all(&project).unwrap();
    let mut permissions = fs::metadata(&project).unwrap().permissions();
    permissions.set_mode(0o000);
    fs::set_permissions(&project, permissions).unwrap();

    let error = preview_workspace_setup(
        WorkspaceSetupPreviewRequest {
            selected_path: project.clone(),
            kind: WorkspaceKind::User,
        },
        root.join("SkillBox"),
    )
    .unwrap_err();

    let mut restore = fs::metadata(&project).unwrap().permissions();
    restore.set_mode(0o700);
    fs::set_permissions(&project, restore).unwrap();
    assert!(error.contains("not readable"));
}

#[test]
fn workspace_setup_registration_failure_removes_only_new_empty_directories() {
    let root = temp_dir("workspace-setup-cleanup");
    let project = root.join("project");
    let marker = project.join(".agents");
    fs::create_dir_all(&marker).unwrap();
    fs::write(marker.join("keep.txt"), "keep").unwrap();
    let preview = preview_workspace_setup(
        WorkspaceSetupPreviewRequest {
            selected_path: project.clone(),
            kind: WorkspaceKind::User,
        },
        root.join("SkillBox"),
    )
    .unwrap();
    let selected = preview
        .roots
        .iter()
        .find(|root| root.relative_path == ".agents/skills")
        .unwrap();

    let error = apply_workspace_setup_with_register(
        WorkspaceSetupApplyRequest {
            selected_path: project.clone(),
            kind: WorkspaceKind::User,
            selected_root: selected.path.clone(),
            create_missing: true,
            preview_id: preview.preview_id,
        },
        &root.join("SkillBox"),
        |_| Err("registration failed".to_string()),
    )
    .unwrap_err();

    assert_eq!(error, "registration failed");
    assert!(!project.join(".agents/skills").exists());
    assert_eq!(fs::read_to_string(marker.join("keep.txt")).unwrap(), "keep");
}

#[test]
fn workspace_setup_global_scope_requires_an_existing_exact_root() {
    let root = temp_dir("workspace-setup-global");
    let global_root = root.join("custom-global-skills");
    let managed_root = root.join("SkillBox");
    fs::create_dir_all(&global_root).unwrap();
    let preview = preview_workspace_setup(
        WorkspaceSetupPreviewRequest {
            selected_path: global_root.clone(),
            kind: WorkspaceKind::Global,
        },
        &managed_root,
    )
    .unwrap();

    assert_eq!(preview.mode, WorkspaceSetupMode::ExistingRoot);
    let error = apply_workspace_setup(
        WorkspaceSetupApplyRequest {
            selected_path: global_root,
            kind: WorkspaceKind::Global,
            selected_root: preview.roots[0].path.clone(),
            create_missing: true,
            preview_id: preview.preview_id,
        },
        &managed_root,
    )
    .unwrap_err();
    assert!(error.contains("selection changed"));
}

#[test]
fn add_workspace_does_not_count_copied_only_skills_as_imported() {
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
    assert_eq!(workspace.imported_skill_count, 0);
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

    assert_eq!(first.usage_count, 0);
    assert_eq!(first.evidence_class, SkillUsageEvidenceClass::Reference);
    assert!(!first.deduplicated);
    assert_eq!(first.used_at, "2026-06-02T10:15:00+00:00");
    assert_eq!(second.usage_count, 0);
    assert!(second.deduplicated);
    assert_eq!(second.last_used_at, "2026-06-02T10:15:00+00:00");

    let history = list_history(HistoryFilter::default(), &managed_root).unwrap();
    assert_eq!(history.skill_usage_count, 0);
    assert_eq!(history.skill_reference_count, 1);
    assert_eq!(
        history.entries[0].prompt_excerpt.as_deref(),
        Some("Second prompt should backfill the existing event")
    );
}

#[test]
fn usage_audit_is_aggregate_only_and_keeps_event_content_private() {
    let root = temp_dir("usage-audit-private");
    let managed_root = root.join("SkillBox");
    let runtime_root = root.join(".codex").join("skills");
    let event_id = "private-event-id";
    let prompt = "private prompt content";

    record_skill_usage(
        RecordSkillUsageRequest {
            skill_name: "private-skill".to_string(),
            agent_id: "codex".to_string(),
            runtime_root,
            event_id: Some(event_id.to_string()),
            used_at: Some("2026-06-02T10:15:00Z".to_string()),
            prompt_excerpt: Some(prompt.to_string()),
            metadata: Some(serde_json::json!({ "source": "manual" })),
        },
        &managed_root,
    )
    .unwrap();

    let audit = usage_audit(&managed_root).unwrap();
    assert_eq!(audit.total_calls, 0);
    assert_eq!(audit.confirmed_calls, 0);
    assert_eq!(audit.inferred_calls, 0);
    assert_eq!(audit.history_references, 1);
    assert_eq!(audit.codex_provider_reported_total, None);
    assert_eq!(audit.codex_remaining_gap, None);
    assert_eq!(audit.known_limitations.len(), 1);
    assert!(audit.known_limitations[0].contains("may still undercount Codex usage"));
    let json = serde_json::to_string(&audit).unwrap();
    assert!(!json.contains("private-skill"));
    assert!(!json.contains(event_id));
    assert!(!json.contains(prompt));
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

    record_test_call(
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
    record_test_call(
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
fn usage_rankings_include_managed_zero_rows_and_apply_time_range_ordering() {
    let root = temp_dir("usage-rankings-range");
    let managed_root = root.join("SkillBox");
    let source_root = root.join("sources");
    let workspace = root.join("project").join(".codex").join("skills");
    fs::create_dir_all(&workspace).unwrap();
    for name in ["alpha", "beta", "gamma"] {
        let source = source_root.join(name);
        make_skill(&source, name, "Ranking skill");
        import_skill(&source, SkillKind::User, &managed_root).unwrap();
    }

    for (skill_name, used_at) in [
        ("alpha", "2026-06-29T12:00:00Z"),
        ("alpha", "2026-06-28T12:00:00Z"),
        ("beta", "2026-06-23T12:00:00Z"),
        ("beta", "2026-06-20T12:00:00Z"),
        ("draft-helper", "2026-06-29T12:00:00Z"),
        ("alpha", "2026-07-01T12:00:00Z"),
    ] {
        let request = RecordSkillUsageRequest {
            skill_name: skill_name.to_string(),
            agent_id: "codex".to_string(),
            runtime_root: workspace.clone(),
            event_id: None,
            used_at: Some(used_at.to_string()),
            prompt_excerpt: None,
            metadata: (skill_name == "alpha" && used_at == "2026-06-29T12:00:00Z")
                .then(|| serde_json::json!({ "source": "agent_hook" })),
        };
        if request.metadata.is_some() {
            record_trusted_generated_skill_usage(request, &managed_root).unwrap();
        } else {
            record_test_call(request, &managed_root).unwrap();
        }
    }

    let as_of = DateTime::parse_from_rfc3339("2026-06-30T12:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let last_seven = list_skill_usage_rankings_at(
        SkillUsageRankingRequest {
            range: SkillUsageRankingRange::Last7Days,
            ..SkillUsageRankingRequest::default()
        },
        &managed_root,
        as_of,
    )
    .unwrap();

    assert_eq!(
        last_seven.range_start.as_deref(),
        Some("2026-06-23T12:00:00+00:00")
    );
    assert_eq!(last_seven.range_end, "2026-06-30T12:00:00+00:00");
    assert_eq!(last_seven.total_observed_calls, 3);
    assert_eq!(
        last_seven.coverage.earliest_event_at.as_deref(),
        Some("2026-06-23T12:00:00+00:00")
    );
    assert_eq!(
        last_seven.coverage.latest_event_at.as_deref(),
        Some("2026-06-29T12:00:00+00:00")
    );
    assert_eq!(last_seven.coverage.agent_hook_calls, 3);
    assert_eq!(last_seven.coverage.codex_session_backfill_calls, 0);
    assert_eq!(last_seven.coverage.other_observed_calls, 0);
    assert_eq!(last_seven.coverage.scanned_codex_session_files, 0);
    assert_eq!(
        last_seven
            .rows
            .iter()
            .map(|row| (row.rank, row.skill_name.as_str(), row.usage_count))
            .collect::<Vec<_>>(),
        vec![(1, "alpha", 2), (2, "beta", 1), (3, "gamma", 0)]
    );
    assert!(last_seven.rows.iter().all(|row| row.managed));
    assert!(last_seven
        .rows
        .iter()
        .all(|row| row.kind == Some(SkillKind::User)));

    let last_thirty =
        list_skill_usage_rankings_at(SkillUsageRankingRequest::default(), &managed_root, as_of)
            .unwrap();
    assert_eq!(last_thirty.rows[0].skill_name, "alpha");
    assert_eq!(last_thirty.rows[0].usage_count, 2);
    assert_eq!(last_thirty.rows[1].skill_name, "beta");
    assert_eq!(last_thirty.rows[1].usage_count, 2);
    assert_eq!(
        last_thirty.rows[0].last_used_at.as_deref(),
        Some("2026-06-29T12:00:00+00:00")
    );

    let all_time = list_skill_usage_rankings_at(
        SkillUsageRankingRequest {
            range: SkillUsageRankingRange::AllTime,
            ..SkillUsageRankingRequest::default()
        },
        &managed_root,
        as_of,
    )
    .unwrap();
    assert_eq!(all_time.range_start, None);
    assert_eq!(all_time.total_observed_calls, 4);
    assert_eq!(all_time.rows[0].usage_count, 2);
}

#[test]
fn usage_rankings_filter_agent_and_workspace_and_optionally_include_unmanaged() {
    let root = temp_dir("usage-rankings-filters");
    let managed_root = root.join("SkillBox");
    let source = root.join("source").join("alpha");
    let first_workspace = root.join("one").join(".codex").join("skills");
    let second_workspace = root.join("two").join(".agents").join("skills");
    make_skill(&source, "alpha", "Alpha skill");
    import_skill(&source, SkillKind::User, &managed_root).unwrap();
    fs::create_dir_all(&first_workspace).unwrap();
    fs::create_dir_all(&second_workspace).unwrap();

    for (skill_name, agent_id, runtime_root, used_at) in [
        ("alpha", "codex", &first_workspace, "2026-06-29T10:00:00Z"),
        (
            "alpha",
            "claude-code",
            &first_workspace,
            "2026-06-29T11:00:00Z",
        ),
        ("alpha", "codex", &second_workspace, "2026-06-29T12:00:00Z"),
        (
            "draft-helper",
            "codex",
            &first_workspace,
            "2026-06-29T09:00:00Z",
        ),
    ] {
        record_test_call(
            RecordSkillUsageRequest {
                skill_name: skill_name.to_string(),
                agent_id: agent_id.to_string(),
                runtime_root: runtime_root.clone(),
                event_id: None,
                used_at: Some(used_at.to_string()),
                prompt_excerpt: Some("private excerpt".to_string()),
                metadata: Some(serde_json::json!({ "source": "test" })),
            },
            &managed_root,
        )
        .unwrap();
    }

    let as_of = DateTime::parse_from_rfc3339("2026-06-30T12:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let agent_only = list_skill_usage_rankings_at(
        SkillUsageRankingRequest {
            range: SkillUsageRankingRange::Last7Days,
            agent_id: Some("codex".to_string()),
            ..SkillUsageRankingRequest::default()
        },
        &managed_root,
        as_of,
    )
    .unwrap();
    assert_eq!(agent_only.rows.len(), 1);
    assert_eq!(agent_only.rows[0].usage_count, 2);

    let workspace_only = list_skill_usage_rankings_at(
        SkillUsageRankingRequest {
            range: SkillUsageRankingRange::Last7Days,
            workspace_root: Some(first_workspace.clone()),
            ..SkillUsageRankingRequest::default()
        },
        &managed_root,
        as_of,
    )
    .unwrap();
    assert_eq!(workspace_only.rows.len(), 1);
    assert_eq!(workspace_only.rows[0].usage_count, 2);

    let result = list_skill_usage_rankings_at(
        SkillUsageRankingRequest {
            range: SkillUsageRankingRange::Last7Days,
            skill_type: None,
            agent_id: Some("CODEX".to_string()),
            workspace_root: Some(first_workspace.clone()),
            include_unmanaged: true,
        },
        &managed_root,
        as_of,
    )
    .unwrap();

    assert_eq!(result.agent_id.as_deref(), Some("codex"));
    assert_eq!(
        result.workspace_root,
        Some(fs::canonicalize(first_workspace).unwrap())
    );
    assert_eq!(result.total_observed_calls, 2);
    assert_eq!(result.rows.len(), 2);
    assert_eq!(result.rows[0].skill_name, "alpha");
    assert_eq!(result.rows[0].usage_count, 1);
    assert!(result.rows[0].managed);
    assert_eq!(result.rows[1].skill_name, "draft-helper");
    assert_eq!(result.rows[1].usage_count, 1);
    assert!(!result.rows[1].managed);
    assert!(!result.rows[1].system);
    assert_eq!(result.rows[1].kind, None);

    let json = serde_json::to_string(&result).unwrap();
    assert!(!json.contains("private excerpt"));
    assert!(!json.contains("metadata"));
}

#[test]
fn usage_rankings_filter_user_remote_and_system_with_scoped_coverage() {
    let root = temp_dir("usage-rankings-skill-types");
    let managed_root = root.join("SkillBox");
    let source_root = root.join("sources");
    let runtime_root = root.join(".codex").join("skills");

    for (name, kind) in [
        ("user-alpha", SkillKind::User),
        ("user-zero", SkillKind::User),
        ("remote-beta", SkillKind::Remote),
    ] {
        let source = source_root.join(name);
        make_skill(&source, name, "Type-filtered ranking skill");
        import_skill(&source, kind, &managed_root).unwrap();
    }
    make_skill(
        &runtime_root.join(".system").join("system-gamma"),
        "system-gamma",
        "System ranking skill",
    );

    for (skill_name, used_at, source, source_kind) in [
        (
            "user-alpha",
            "2026-06-29T10:00:00Z",
            "agent_hook",
            "regular",
        ),
        (
            "user-alpha",
            "2026-06-29T11:00:00Z",
            "manual_test",
            "regular",
        ),
        (
            "remote-beta",
            "2026-06-29T12:00:00Z",
            "codex_session_backfill",
            "regular",
        ),
        (
            "system-gamma",
            "2026-06-29T13:00:00Z",
            "agent_hook",
            "system",
        ),
        (
            "system-gamma",
            "2026-06-29T14:00:00Z",
            "codex_session_backfill",
            "system",
        ),
    ] {
        let request = RecordSkillUsageRequest {
            skill_name: skill_name.to_string(),
            agent_id: "codex".to_string(),
            runtime_root: runtime_root.clone(),
            event_id: Some(format!("type-filter-{skill_name}-{used_at}")),
            used_at: Some(used_at.to_string()),
            prompt_excerpt: None,
            metadata: Some(serde_json::json!({
                "source": source,
                "skill_source_kind": source_kind
            })),
        };
        if matches!(source, "agent_hook" | "codex_session_backfill") {
            record_trusted_generated_skill_usage(request, &managed_root).unwrap();
        } else {
            record_test_call(request, &managed_root).unwrap();
        }
    }

    let as_of = DateTime::parse_from_rfc3339("2026-06-30T12:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let user = list_skill_usage_rankings_at(
        SkillUsageRankingRequest {
            range: SkillUsageRankingRange::Last7Days,
            skill_type: Some(SkillUsageRankingSkillType::User),
            include_unmanaged: true,
            ..SkillUsageRankingRequest::default()
        },
        &managed_root,
        as_of,
    )
    .unwrap();
    assert_eq!(user.skill_type, Some(SkillUsageRankingSkillType::User));
    assert_eq!(user.total_observed_calls, 2);
    assert_eq!(
        user.rows
            .iter()
            .map(|row| (row.rank, row.skill_name.as_str(), row.usage_count))
            .collect::<Vec<_>>(),
        vec![(1, "user-alpha", 2), (2, "user-zero", 0)]
    );
    assert_eq!(
        user.coverage.earliest_event_at.as_deref(),
        Some("2026-06-29T10:00:00+00:00")
    );
    assert_eq!(
        user.coverage.latest_event_at.as_deref(),
        Some("2026-06-29T11:00:00+00:00")
    );
    assert_eq!(user.coverage.agent_hook_calls, 2);
    assert_eq!(user.coverage.codex_session_backfill_calls, 0);
    assert_eq!(user.coverage.other_observed_calls, 0);

    let remote = list_skill_usage_rankings_at(
        SkillUsageRankingRequest {
            range: SkillUsageRankingRange::Last7Days,
            skill_type: Some(SkillUsageRankingSkillType::Remote),
            include_unmanaged: true,
            ..SkillUsageRankingRequest::default()
        },
        &managed_root,
        as_of,
    )
    .unwrap();
    assert_eq!(remote.skill_type, Some(SkillUsageRankingSkillType::Remote));
    assert_eq!(remote.total_observed_calls, 1);
    assert_eq!(
        remote
            .rows
            .iter()
            .map(|row| (row.rank, row.skill_name.as_str(), row.usage_count))
            .collect::<Vec<_>>(),
        vec![(1, "remote-beta", 1)]
    );
    assert_eq!(
        remote.coverage.earliest_event_at.as_deref(),
        Some("2026-06-29T12:00:00+00:00")
    );
    assert_eq!(
        remote.coverage.latest_event_at.as_deref(),
        Some("2026-06-29T12:00:00+00:00")
    );
    assert_eq!(remote.coverage.agent_hook_calls, 0);
    assert_eq!(remote.coverage.codex_session_backfill_calls, 1);
    assert_eq!(remote.coverage.other_observed_calls, 0);

    let system = list_skill_usage_rankings_at(
        SkillUsageRankingRequest {
            range: SkillUsageRankingRange::Last7Days,
            skill_type: Some(SkillUsageRankingSkillType::System),
            include_unmanaged: true,
            ..SkillUsageRankingRequest::default()
        },
        &managed_root,
        as_of,
    )
    .unwrap();
    assert_eq!(system.skill_type, Some(SkillUsageRankingSkillType::System));
    assert_eq!(system.total_observed_calls, 2);
    assert_eq!(
        system
            .rows
            .iter()
            .map(|row| {
                (
                    row.rank,
                    row.skill_name.as_str(),
                    row.usage_count,
                    row.system,
                )
            })
            .collect::<Vec<_>>(),
        vec![(1, "system-gamma", 2, true)]
    );
    assert_eq!(
        system.coverage.earliest_event_at.as_deref(),
        Some("2026-06-29T13:00:00+00:00")
    );
    assert_eq!(
        system.coverage.latest_event_at.as_deref(),
        Some("2026-06-29T14:00:00+00:00")
    );
    assert_eq!(system.coverage.agent_hook_calls, 1);
    assert_eq!(system.coverage.codex_session_backfill_calls, 1);
    assert_eq!(system.coverage.other_observed_calls, 0);
}

#[test]
fn usage_rankings_mark_codex_system_skills_as_non_importable() {
    let root = temp_dir("usage-rankings-system");
    let managed_root = root.join("SkillBox");
    let runtime_root = root.join(".codex").join("skills");
    let system_skill = runtime_root.join(".system").join("skill-creator");
    let unmanaged_skill = runtime_root.join("draft-helper");
    make_skill(&system_skill, "skill-creator", "System skill");
    make_skill(&unmanaged_skill, "draft-helper", "Draft helper");

    for (skill_name, used_at) in [
        ("skill-creator", "2026-06-29T10:00:00Z"),
        ("draft-helper", "2026-06-29T09:00:00Z"),
    ] {
        record_test_call(
            RecordSkillUsageRequest {
                skill_name: skill_name.to_string(),
                agent_id: "codex".to_string(),
                runtime_root: runtime_root.clone(),
                event_id: None,
                used_at: Some(used_at.to_string()),
                prompt_excerpt: None,
                metadata: None,
            },
            &managed_root,
        )
        .unwrap();
    }

    let as_of = DateTime::parse_from_rfc3339("2026-06-30T12:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let result = list_skill_usage_rankings_at(
        SkillUsageRankingRequest {
            range: SkillUsageRankingRange::Last7Days,
            include_unmanaged: true,
            ..SkillUsageRankingRequest::default()
        },
        &managed_root,
        as_of,
    )
    .unwrap();

    let system = result
        .rows
        .iter()
        .find(|row| row.skill_name == "skill-creator")
        .expect("system skill row");
    assert!(!system.managed);
    assert!(system.system);
    assert!(!system.source_missing);
    assert_eq!(system.kind, None);

    let unmanaged = result
        .rows
        .iter()
        .find(|row| row.skill_name == "draft-helper")
        .expect("unmanaged skill row");
    assert!(!unmanaged.managed);
    assert!(!unmanaged.system);
    assert!(!unmanaged.source_missing);

    let import_error = preview_usage_skill_import("skill-creator", &managed_root).unwrap_err();
    assert!(import_error.contains("not importable"));
}

#[test]
fn usage_rankings_mark_missing_unmanaged_sources_as_deleted() {
    let root = temp_dir("usage-rankings-deleted");
    let managed_root = root.join("SkillBox");
    let runtime_root = root.join(".codex").join("skills");
    fs::create_dir_all(&runtime_root).unwrap();
    let broken = runtime_root.join("ghost-skill");
    symlink_dir(
        &managed_root
            .join("remote-skills")
            .join("ghost-skill")
            .join("current"),
        &broken,
    )
    .unwrap();

    record_test_call(
        RecordSkillUsageRequest {
            skill_name: "ghost-skill".to_string(),
            agent_id: "codex".to_string(),
            runtime_root: runtime_root.clone(),
            event_id: None,
            used_at: Some("2026-06-29T10:00:00Z".to_string()),
            prompt_excerpt: None,
            metadata: None,
        },
        &managed_root,
    )
    .unwrap();

    let as_of = DateTime::parse_from_rfc3339("2026-06-30T12:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let result = list_skill_usage_rankings_at(
        SkillUsageRankingRequest {
            range: SkillUsageRankingRange::Last7Days,
            include_unmanaged: true,
            ..SkillUsageRankingRequest::default()
        },
        &managed_root,
        as_of,
    )
    .unwrap();

    let ghost = result
        .rows
        .iter()
        .find(|row| row.skill_name == "ghost-skill")
        .expect("deleted skill row");
    assert!(!ghost.managed);
    assert!(!ghost.system);
    assert!(ghost.source_missing);
}

#[test]
fn usage_rankings_agent_filter_matches_legacy_path_based_agent_ids() {
    let root = temp_dir("usage-rankings-agent-aliases");
    let managed_root = root.join("SkillBox");
    let paths = ensure_managed_layout(&managed_root).unwrap();
    let agents_root = root.join("project").join(".agents").join("skills");
    let claude_root = root.join("project").join(".claude").join("skills");
    fs::create_dir_all(&agents_root).unwrap();
    fs::create_dir_all(&claude_root).unwrap();

    insert_legacy_usage_event(
        &paths.database_path,
        "draft-helper",
        "agents",
        &agents_root,
        "2026-06-29T10:00:00Z",
        "legacy-agents-1",
    );
    insert_legacy_usage_event(
        &paths.database_path,
        "claude-helper",
        "claude",
        &claude_root,
        "2026-06-29T11:00:00Z",
        "legacy-claude-1",
    );

    let as_of = DateTime::parse_from_rfc3339("2026-06-30T12:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let codex_filter = list_skill_usage_rankings_at(
        SkillUsageRankingRequest {
            range: SkillUsageRankingRange::Last7Days,
            agent_id: Some("codex".to_string()),
            include_unmanaged: true,
            ..SkillUsageRankingRequest::default()
        },
        &managed_root,
        as_of,
    )
    .unwrap();
    assert_eq!(codex_filter.rows.len(), 1);
    assert_eq!(codex_filter.rows[0].skill_name, "draft-helper");
    assert_eq!(codex_filter.total_observed_calls, 1);

    let claude_filter = list_skill_usage_rankings_at(
        SkillUsageRankingRequest {
            range: SkillUsageRankingRange::Last7Days,
            agent_id: Some("claude-code".to_string()),
            include_unmanaged: true,
            ..SkillUsageRankingRequest::default()
        },
        &managed_root,
        as_of,
    )
    .unwrap();
    assert_eq!(claude_filter.rows.len(), 1);
    assert_eq!(claude_filter.rows[0].skill_name, "claude-helper");
    assert_eq!(claude_filter.total_observed_calls, 1);
}

#[test]
fn usage_rankings_system_and_deleted_flags_stay_scoped_to_observed_roots() {
    let root = temp_dir("usage-rankings-system-scope");
    let managed_root = root.join("SkillBox");
    let observed_root = root.join("observed").join(".codex").join("skills");
    let other_root = root.join("other").join(".codex").join("skills");
    make_skill(&observed_root.join("shared-skill"), "shared-skill", "Local");
    make_skill(
        &other_root.join(".system").join("shared-skill"),
        "shared-skill",
        "System copy",
    );
    make_skill(
        &other_root.join("ghost"),
        "ghost",
        "Still present elsewhere",
    );

    record_test_call(
        RecordSkillUsageRequest {
            skill_name: "shared-skill".to_string(),
            agent_id: "codex".to_string(),
            runtime_root: observed_root.clone(),
            event_id: Some("system-scope-1".to_string()),
            used_at: Some("2026-06-29T10:00:00Z".to_string()),
            prompt_excerpt: None,
            metadata: None,
        },
        &managed_root,
    )
    .unwrap();
    record_test_call(
        RecordSkillUsageRequest {
            skill_name: "ghost".to_string(),
            agent_id: "codex".to_string(),
            runtime_root: observed_root.clone(),
            event_id: Some("system-scope-2".to_string()),
            used_at: Some("2026-06-29T11:00:00Z".to_string()),
            prompt_excerpt: None,
            metadata: None,
        },
        &managed_root,
    )
    .unwrap();

    let as_of = DateTime::parse_from_rfc3339("2026-06-30T12:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let result = list_skill_usage_rankings_at(
        SkillUsageRankingRequest {
            range: SkillUsageRankingRange::Last7Days,
            include_unmanaged: true,
            ..SkillUsageRankingRequest::default()
        },
        &managed_root,
        as_of,
    )
    .unwrap();

    let shared = result
        .rows
        .iter()
        .find(|row| row.skill_name == "shared-skill")
        .expect("shared skill");
    assert!(!shared.system);
    assert!(!shared.source_missing);

    let ghost = result
        .rows
        .iter()
        .find(|row| row.skill_name == "ghost")
        .expect("ghost skill");
    assert!(!ghost.system);
    assert!(ghost.source_missing);
}

#[test]
fn usage_rankings_split_regular_and_system_rows_with_same_skill_name() {
    let root = temp_dir("usage-rankings-system-split");
    let managed_root = root.join("SkillBox");
    let regular_root = root.join("regular").join(".codex").join("skills");
    let system_root = root.join("system").join(".codex").join("skills");
    make_skill(
        &regular_root.join("shared-skill"),
        "shared-skill",
        "Regular",
    );
    make_skill(
        &system_root.join(".system").join("shared-skill"),
        "shared-skill",
        "System",
    );

    record_test_call(
        RecordSkillUsageRequest {
            skill_name: "shared-skill".to_string(),
            agent_id: "codex".to_string(),
            runtime_root: regular_root.clone(),
            event_id: Some("split-regular-1".to_string()),
            used_at: Some("2026-06-29T10:00:00Z".to_string()),
            prompt_excerpt: None,
            metadata: None,
        },
        &managed_root,
    )
    .unwrap();
    record_test_call(
        RecordSkillUsageRequest {
            skill_name: "shared-skill".to_string(),
            agent_id: "codex".to_string(),
            runtime_root: system_root.clone(),
            event_id: Some("split-system-1".to_string()),
            used_at: Some("2026-06-29T11:00:00Z".to_string()),
            prompt_excerpt: None,
            metadata: None,
        },
        &managed_root,
    )
    .unwrap();

    let as_of = DateTime::parse_from_rfc3339("2026-06-30T12:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let result = list_skill_usage_rankings_at(
        SkillUsageRankingRequest {
            range: SkillUsageRankingRange::Last7Days,
            include_unmanaged: true,
            ..SkillUsageRankingRequest::default()
        },
        &managed_root,
        as_of,
    )
    .unwrap();

    let regular = result
        .rows
        .iter()
        .find(|row| row.skill_name == "shared-skill" && !row.system)
        .expect("regular row");
    let system = result
        .rows
        .iter()
        .find(|row| row.skill_name == "shared-skill" && row.system)
        .expect("system row");
    assert_eq!(regular.usage_count, 1);
    assert!(!regular.source_missing);
    assert_eq!(system.usage_count, 1);
    assert!(!system.source_missing);
}

#[test]
fn usage_rankings_keep_managed_and_system_calls_separate_in_one_runtime() {
    let root = temp_dir("usage-rankings-managed-system-split");
    let managed_root = root.join("SkillBox");
    let paths = ensure_managed_layout(&managed_root).unwrap();
    let runtime_root = root.join("project").join(".codex").join("skills");
    make_skill(
        &paths.user_skills_root.join("shared-skill"),
        "shared-skill",
        "Managed regular",
    );
    make_skill(
        &runtime_root.join("shared-skill"),
        "shared-skill",
        "Runtime regular",
    );
    make_skill(
        &runtime_root.join(".system").join("shared-skill"),
        "shared-skill",
        "Runtime system",
    );

    for (event_id, source_kind, used_at) in [
        ("managed-system-regular", "regular", "2026-06-29T10:00:00Z"),
        ("managed-system-system", "system", "2026-06-29T11:00:00Z"),
    ] {
        record_test_call(
            RecordSkillUsageRequest {
                skill_name: "shared-skill".to_string(),
                agent_id: "codex".to_string(),
                runtime_root: runtime_root.clone(),
                event_id: Some(event_id.to_string()),
                used_at: Some(used_at.to_string()),
                prompt_excerpt: None,
                metadata: Some(serde_json::json!({
                    "skill_source_kind": source_kind
                })),
            },
            &managed_root,
        )
        .unwrap();
    }

    let result = list_skill_usage_rankings_at(
        SkillUsageRankingRequest {
            range: SkillUsageRankingRange::Last7Days,
            include_unmanaged: true,
            ..SkillUsageRankingRequest::default()
        },
        &managed_root,
        DateTime::parse_from_rfc3339("2026-06-30T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc),
    )
    .unwrap();
    let matching = result
        .rows
        .iter()
        .filter(|row| row.skill_name == "shared-skill")
        .collect::<Vec<_>>();

    assert_eq!(matching.len(), 2);
    let regular = matching.iter().find(|row| !row.system).unwrap();
    let system = matching.iter().find(|row| row.system).unwrap();
    assert!(regular.managed);
    assert_eq!(regular.usage_count, 1);
    assert!(!system.managed);
    assert_eq!(system.usage_count, 1);
    assert_ne!(regular.source_id, system.source_id);
}

#[test]
fn usage_rankings_do_not_guess_ambiguous_legacy_sources() {
    let root = temp_dir("usage-rankings-unknown-source");
    let managed_root = root.join("SkillBox");
    let runtime_root = root.join("project").join(".codex").join("skills");
    make_skill(
        &runtime_root.join("shared-skill"),
        "shared-skill",
        "Regular",
    );
    make_skill(
        &runtime_root.join(".system").join("shared-skill"),
        "shared-skill",
        "System",
    );
    record_test_call(
        RecordSkillUsageRequest {
            skill_name: "shared-skill".to_string(),
            agent_id: "codex".to_string(),
            runtime_root,
            event_id: Some("ambiguous-legacy-source".to_string()),
            used_at: Some("2026-06-29T10:00:00Z".to_string()),
            prompt_excerpt: None,
            metadata: None,
        },
        &managed_root,
    )
    .unwrap();

    let result = list_skill_usage_rankings_at(
        SkillUsageRankingRequest {
            range: SkillUsageRankingRange::Last7Days,
            include_unmanaged: true,
            ..SkillUsageRankingRequest::default()
        },
        &managed_root,
        DateTime::parse_from_rfc3339("2026-06-30T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc),
    )
    .unwrap();
    let row = result
        .rows
        .iter()
        .find(|row| row.skill_name == "shared-skill")
        .unwrap();

    assert_eq!(row.source_kind, SkillUsageSourceKind::Unknown);
    assert_eq!(row.usage_count, 1);
    assert!(!row.managed);
    assert!(!row.system);
}

#[test]
fn usage_preview_import_selects_the_requested_regular_source() {
    let root = temp_dir("usage-preview-source-aware");
    let managed_root = root.join("SkillBox");
    let missing_regular_root = root.join("a-missing").join(".codex").join("skills");
    let first_root = root.join("first").join(".codex").join("skills");
    let second_root = root.join("second").join(".codex").join("skills");
    fs::create_dir_all(&missing_regular_root).unwrap();
    make_skill(
        &first_root.join(".system").join("shared-skill"),
        "shared-skill",
        "System",
    );
    make_skill(&second_root.join("shared-skill"), "shared-skill", "Regular");
    for (runtime_root, event_id, source_kind) in [
        (
            &missing_regular_root,
            "source-aware-missing-regular",
            "regular",
        ),
        (&first_root, "source-aware-system", "system"),
        (&second_root, "source-aware-regular", "regular"),
    ] {
        record_test_call(
            RecordSkillUsageRequest {
                skill_name: "shared-skill".to_string(),
                agent_id: "codex".to_string(),
                runtime_root: runtime_root.clone(),
                event_id: Some(event_id.to_string()),
                used_at: Some("2026-06-29T10:00:00Z".to_string()),
                prompt_excerpt: None,
                metadata: Some(serde_json::json!({
                    "skill_source_kind": source_kind
                })),
            },
            &managed_root,
        )
        .unwrap();
    }

    let rankings = list_skill_usage_rankings_at(
        SkillUsageRankingRequest {
            range: SkillUsageRankingRange::AllTime,
            include_unmanaged: true,
            ..SkillUsageRankingRequest::default()
        },
        &managed_root,
        DateTime::parse_from_rfc3339("2026-06-30T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc),
    )
    .unwrap();
    let regular = rankings
        .rows
        .iter()
        .find(|row| {
            row.skill_name == "shared-skill" && row.source_kind == SkillUsageSourceKind::Regular
        })
        .unwrap();
    assert_eq!(regular.source_runtime_roots.len(), 2);

    let candidate = preview_usage_skill_import_for_source(
        PreviewUsageSkillImportRequest {
            skill_name: "shared-skill".to_string(),
            source_kind: Some(SkillUsageSourceKind::Regular),
            source_id: Some(regular.source_id.clone()),
            source_runtime_roots: regular.source_runtime_roots.clone(),
            ranking_request: Some(SkillUsageRankingRequest {
                range: SkillUsageRankingRange::AllTime,
                include_unmanaged: true,
                ..SkillUsageRankingRequest::default()
            }),
            ranking_generated_at: Some(rankings.generated_at.clone()),
            runtime_root: None,
        },
        &managed_root,
    )
    .unwrap();

    assert_eq!(
        fs::canonicalize(candidate.source_path).unwrap(),
        fs::canonicalize(second_root.join("shared-skill")).unwrap()
    );
    let stale_error = preview_usage_skill_import_for_source(
        PreviewUsageSkillImportRequest {
            skill_name: "shared-skill".to_string(),
            source_kind: Some(SkillUsageSourceKind::Regular),
            source_id: Some(regular.source_id.clone()),
            source_runtime_roots: vec![root.join("unrecorded").join(".codex").join("skills")],
            ranking_request: Some(SkillUsageRankingRequest {
                range: SkillUsageRankingRange::AllTime,
                include_unmanaged: true,
                ..SkillUsageRankingRequest::default()
            }),
            ranking_generated_at: Some(rankings.generated_at.clone()),
            runtime_root: None,
        },
        &managed_root,
    )
    .unwrap_err();
    assert!(stale_error.contains("displayed row"));
    let missing_identity_error = preview_usage_skill_import_for_source(
        PreviewUsageSkillImportRequest {
            skill_name: "shared-skill".to_string(),
            source_kind: Some(SkillUsageSourceKind::Regular),
            source_id: None,
            source_runtime_roots: regular.source_runtime_roots.clone(),
            ranking_request: None,
            ranking_generated_at: None,
            runtime_root: None,
        },
        &managed_root,
    )
    .unwrap_err();
    assert!(missing_identity_error.contains("identity is required"));
    let subset_error = preview_usage_skill_import_for_source(
        PreviewUsageSkillImportRequest {
            skill_name: "shared-skill".to_string(),
            source_kind: Some(SkillUsageSourceKind::Regular),
            source_id: Some(regular.source_id.clone()),
            source_runtime_roots: vec![second_root.clone()],
            ranking_request: Some(SkillUsageRankingRequest {
                range: SkillUsageRankingRange::AllTime,
                include_unmanaged: true,
                ..SkillUsageRankingRequest::default()
            }),
            ranking_generated_at: Some(rankings.generated_at.clone()),
            runtime_root: None,
        },
        &managed_root,
    )
    .unwrap_err();
    assert!(subset_error.contains("displayed row"));

    let system_error = preview_usage_skill_import_for_source(
        PreviewUsageSkillImportRequest {
            skill_name: "shared-skill".to_string(),
            source_kind: Some(SkillUsageSourceKind::System),
            source_id: None,
            source_runtime_roots: Vec::new(),
            ranking_request: None,
            ranking_generated_at: None,
            runtime_root: Some(first_root),
        },
        &managed_root,
    )
    .unwrap_err();
    assert!(system_error.contains("cannot be imported"));
}

#[test]
fn usage_record_dedupes_legacy_agent_ids_against_canonical_writes() {
    let root = temp_dir("usage-legacy-agent-dedupe");
    let managed_root = root.join("SkillBox");
    let paths = ensure_managed_layout(&managed_root).unwrap();
    let runtime_root = root.join("project").join(".agents").join("skills");
    fs::create_dir_all(&runtime_root).unwrap();
    insert_legacy_usage_event(
        &paths.database_path,
        "probe",
        "agents",
        &runtime_root,
        "2026-06-29T10:00:00Z",
        "legacy-event-1",
    );

    let first = record_test_call(
        RecordSkillUsageRequest {
            skill_name: "probe".to_string(),
            agent_id: "codex".to_string(),
            runtime_root: runtime_root.clone(),
            event_id: Some("legacy-event-1".to_string()),
            used_at: Some("2026-06-29T10:00:00Z".to_string()),
            prompt_excerpt: None,
            metadata: None,
        },
        &managed_root,
    )
    .unwrap();
    assert!(first.deduplicated);
    assert_eq!(first.agent_id, "codex");

    let connection = open_database(&paths.database_path).unwrap();
    let event_count: i64 = connection
        .query_row(
            "
            SELECT COUNT(*)
            FROM skill_usage_events
            WHERE event_id = 'legacy-event-1'
            ",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(event_count, 1);
    let stored_agent: String = connection
        .query_row(
            "
            SELECT agent_id
            FROM skill_usage_events
            WHERE event_id = 'legacy-event-1'
            ",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(stored_agent, "codex");
    let stored_count: i64 = connection
        .query_row(
            "
            SELECT usage_count
            FROM skill_usage_stats
            WHERE skill_name = 'probe' AND agent_id = 'codex'
            ",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(stored_count, event_count);
}

#[test]
fn usage_record_enriches_duplicate_event_source_identity_without_incrementing() {
    let root = temp_dir("usage-event-source-enrichment");
    let managed_root = root.join("SkillBox");
    let paths = ensure_managed_layout(&managed_root).unwrap();
    let runtime_root = root.join("project").join(".codex").join("skills");
    fs::create_dir_all(&runtime_root).unwrap();
    let base = RecordSkillUsageRequest {
        skill_name: "probe".to_string(),
        agent_id: "codex".to_string(),
        runtime_root,
        event_id: Some("source-enrichment-1".to_string()),
        used_at: Some("2026-06-29T10:00:00Z".to_string()),
        prompt_excerpt: None,
        metadata: Some(serde_json::json!({ "source": "agent_hook" })),
    };
    let first = record_trusted_generated_skill_usage(base.clone(), &managed_root).unwrap();
    let second = record_trusted_generated_skill_usage(
        RecordSkillUsageRequest {
            metadata: Some(serde_json::json!({
                "source": "codex_session_backfill",
                "skill_source_kind": "system"
            })),
            ..base
        },
        &managed_root,
    )
    .unwrap();

    assert!(!first.deduplicated);
    assert!(second.deduplicated);
    assert_eq!(second.usage_count, 1);
    let connection = open_database(&paths.database_path).unwrap();
    let metadata_json: String = connection
        .query_row(
            "
            SELECT metadata_json
            FROM skill_usage_events
            WHERE event_id = 'source-enrichment-1'
            ",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let metadata: serde_json::Value = serde_json::from_str(&metadata_json).unwrap();
    assert_eq!(
        metadata
            .get("skill_source_kind")
            .and_then(|value| value.as_str()),
        Some("system")
    );
}

#[test]
fn usage_evidence_upgrades_reference_to_inferred_to_confirmed_without_double_counting() {
    let root = temp_dir("usage-evidence-upgrade");
    let managed_root = root.join("SkillBox");
    let paths = ensure_managed_layout(&managed_root).unwrap();
    let runtime_root = root.join("project/.codex/skills");
    fs::create_dir_all(&runtime_root).unwrap();
    let base = RecordSkillUsageRequest {
        skill_name: "probe".to_string(),
        agent_id: "codex".to_string(),
        runtime_root,
        event_id: Some("shared-invocation".to_string()),
        used_at: Some("2026-07-01T10:00:00Z".to_string()),
        prompt_excerpt: None,
        metadata: None,
    };

    let reference = record_skill_usage(base.clone(), &managed_root).unwrap();
    assert_eq!(reference.evidence_class, SkillUsageEvidenceClass::Reference);
    assert_eq!(reference.usage_count, 0);

    let inferred = record_trusted_generated_skill_usage(
        RecordSkillUsageRequest {
            metadata: Some(serde_json::json!({ "source": "codex_session_backfill" })),
            ..base.clone()
        },
        &managed_root,
    )
    .unwrap();
    assert!(inferred.upgraded);
    assert_eq!(inferred.evidence_class, SkillUsageEvidenceClass::Inferred);
    assert_eq!(inferred.usage_count, 1);

    let confirmed = record_trusted_generated_skill_usage(
        RecordSkillUsageRequest {
            metadata: Some(serde_json::json!({ "source": "agent_hook" })),
            ..base
        },
        &managed_root,
    )
    .unwrap();
    assert!(confirmed.upgraded);
    assert_eq!(confirmed.evidence_class, SkillUsageEvidenceClass::Confirmed);
    assert_eq!(confirmed.usage_count, 1);

    let connection = open_database(&paths.database_path).unwrap();
    let (event_count, call_count): (i64, i64) = (
        connection
            .query_row("SELECT COUNT(*) FROM skill_usage_events", [], |row| {
                row.get(0)
            })
            .unwrap(),
        connection
            .query_row(
                "SELECT SUM(usage_count) FROM skill_usage_stats",
                [],
                |row| row.get(0),
            )
            .unwrap(),
    );
    assert_eq!(event_count, 1);
    assert_eq!(call_count, 1);
    let sources_json: String = connection
        .query_row(
            "SELECT evidence_sources_json FROM skill_usage_events WHERE event_id = 'shared-invocation'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&sources_json).unwrap(),
        serde_json::json!([
            { "source": "manual", "evidence_class": "reference" },
            { "source": "codex_session_backfill", "evidence_class": "inferred" },
            { "source": "agent_hook", "evidence_class": "confirmed" }
        ])
    );
    let rankings = list_skill_usage_rankings_at(
        SkillUsageRankingRequest {
            range: SkillUsageRankingRange::AllTime,
            include_unmanaged: true,
            ..SkillUsageRankingRequest::default()
        },
        &managed_root,
        DateTime::parse_from_rfc3339("2026-07-02T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc),
    )
    .unwrap();
    assert_eq!(rankings.total_calls, 1);
    assert_eq!(rankings.total_confirmed_calls, 1);
    assert_eq!(rankings.total_inferred_calls, 0);
    assert_eq!(rankings.total_history_references, 0);
    assert_eq!(rankings.coverage.agent_hook_calls, 1);
    assert_eq!(rankings.coverage.codex_session_backfill_calls, 0);
    assert_eq!(rankings.coverage.other_observed_calls, 0);
    assert_eq!(rankings.coverage.source_counts.len(), 3);
}

#[test]
fn usage_evidence_never_downgrades_confirmed_events() {
    let root = temp_dir("usage-evidence-no-downgrade");
    let managed_root = root.join("SkillBox");
    let runtime_root = root.join("project/.codex/skills");
    fs::create_dir_all(&runtime_root).unwrap();
    let base = RecordSkillUsageRequest {
        skill_name: "probe".to_string(),
        agent_id: "codex".to_string(),
        runtime_root,
        event_id: Some("confirmed-first".to_string()),
        used_at: Some("2026-07-01T10:00:00Z".to_string()),
        prompt_excerpt: None,
        metadata: Some(serde_json::json!({ "source": "agent_hook" })),
    };
    let first = record_trusted_generated_skill_usage(base.clone(), &managed_root).unwrap();
    let second = record_trusted_generated_skill_usage(
        RecordSkillUsageRequest {
            metadata: Some(serde_json::json!({ "source": "codex_session_backfill" })),
            ..base
        },
        &managed_root,
    )
    .unwrap();
    assert_eq!(first.evidence_class, SkillUsageEvidenceClass::Confirmed);
    assert_eq!(second.evidence_class, SkillUsageEvidenceClass::Confirmed);
    assert!(!second.upgraded);
    assert_eq!(second.usage_count, 1);
}

#[test]
fn usage_record_dedupes_generated_event_after_runtime_attribution_changes() {
    let root = temp_dir("usage-event-runtime-change");
    let managed_root = root.join("SkillBox");
    let first_runtime = root.join("first").join(".codex").join("skills");
    let second_runtime = root.join("second").join(".codex").join("skills");
    fs::create_dir_all(&first_runtime).unwrap();
    fs::create_dir_all(&second_runtime).unwrap();
    let base = RecordSkillUsageRequest {
        skill_name: "probe".to_string(),
        agent_id: "codex".to_string(),
        runtime_root: first_runtime.clone(),
        event_id: Some("codex:session:turn:0:probe:pathhash".to_string()),
        used_at: Some("2026-06-29T10:00:00Z".to_string()),
        prompt_excerpt: None,
        metadata: Some(serde_json::json!({
            "source": "agent_hook",
            "skill_source_kind": "regular"
        })),
    };
    let first = record_trusted_generated_skill_usage(base.clone(), &managed_root).unwrap();
    let second = record_trusted_generated_skill_usage(
        RecordSkillUsageRequest {
            runtime_root: second_runtime,
            metadata: Some(serde_json::json!({
                "source": "codex_session_backfill",
                "skill_source_kind": "regular"
            })),
            ..base
        },
        &managed_root,
    )
    .unwrap();

    assert!(!first.deduplicated);
    assert!(second.deduplicated);
    assert_eq!(
        second.runtime_root,
        fs::canonicalize(first_runtime).unwrap()
    );
    assert_eq!(second.usage_count, 1);
}

#[test]
fn usage_record_does_not_trust_generated_metadata_from_public_requests() {
    let root = temp_dir("usage-event-untrusted-generated-metadata");
    let managed_root = root.join("SkillBox");
    let first_runtime = root.join("first").join(".codex").join("skills");
    let trusted_runtime = root.join("trusted").join(".codex").join("skills");
    fs::create_dir_all(&first_runtime).unwrap();
    fs::create_dir_all(&trusted_runtime).unwrap();
    let request = RecordSkillUsageRequest {
        skill_name: "probe".to_string(),
        agent_id: "codex".to_string(),
        runtime_root: first_runtime,
        event_id: Some("codex:session:turn:0:probe:pathhash".to_string()),
        used_at: Some("2026-06-29T10:00:00Z".to_string()),
        prompt_excerpt: None,
        metadata: Some(serde_json::json!({
            "source": "agent_hook",
            "skill_source_kind": "regular"
        })),
    };
    let error = record_skill_usage(request.clone(), &managed_root).unwrap_err();

    assert!(error.contains("reserved"));
    let trusted = record_trusted_generated_skill_usage(
        RecordSkillUsageRequest {
            runtime_root: trusted_runtime.clone(),
            ..request
        },
        &managed_root,
    )
    .unwrap();
    assert!(!trusted.deduplicated);
    assert_eq!(
        trusted.runtime_root,
        fs::canonicalize(trusted_runtime).unwrap()
    );
}

#[test]
fn usage_record_rolls_back_event_when_stats_write_fails() {
    let root = temp_dir("usage-record-atomic");
    let managed_root = root.join("SkillBox");
    let paths = ensure_managed_layout(&managed_root).unwrap();
    let connection = open_database(&paths.database_path).unwrap();
    connection
        .execute_batch(
            "
            CREATE TRIGGER reject_usage_stats_insert
            BEFORE INSERT ON skill_usage_stats
            BEGIN
              SELECT RAISE(FAIL, 'stats write rejected');
            END;
            ",
        )
        .unwrap();
    drop(connection);
    let runtime_root = root.join("project").join(".codex").join("skills");
    fs::create_dir_all(&runtime_root).unwrap();

    let error = record_test_call(
        RecordSkillUsageRequest {
            skill_name: "probe".to_string(),
            agent_id: "codex".to_string(),
            runtime_root,
            event_id: Some("atomic-event-1".to_string()),
            used_at: Some("2026-06-29T10:00:00Z".to_string()),
            prompt_excerpt: None,
            metadata: None,
        },
        &managed_root,
    )
    .unwrap_err();
    assert!(error.contains("stats write rejected"));

    let connection = open_database(&paths.database_path).unwrap();
    let event_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM skill_usage_events WHERE event_id = 'atomic-event-1'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(event_count, 0);
}

#[test]
fn schema_v5_canonicalizes_legacy_usage_agent_ids() {
    let root = temp_dir("database-canonical-usage-agent-ids");
    let managed_root = root.join("SkillBox");
    let paths = ensure_managed_layout(&managed_root).unwrap();
    let connection = rusqlite::Connection::open(&paths.database_path).unwrap();
    connection
        .execute_batch(
            "
            DELETE FROM schema_migrations WHERE version = 5;
            INSERT INTO skill_usage_events (
              id, event_id, skill_name, agent_id, runtime_root, used_at, recorded_at, metadata_json
            ) VALUES
              ('legacy-agents', 'evt-1', 'probe', 'agents', '/tmp/runtime',
               '2026-06-01T00:00:00+00:00', '2026-06-01T00:00:01+00:00',
               '{\"source\":\"agent_hook\"}'),
              ('canonical-codex', 'evt-1', 'probe', 'codex', '/tmp/runtime',
               '2026-06-01T00:00:00+00:00', '2026-06-01T00:00:01+00:00',
               '{\"source\":\"agent_hook\"}'),
              ('legacy-agents-unique', 'evt-unique', 'probe', 'agents', '/tmp/runtime',
               '2026-06-01T02:00:00+00:00', '2026-06-01T02:00:01+00:00',
               '{\"source\":\"agent_hook\"}'),
              ('legacy-agents-null', NULL, 'probe', 'agents', '/tmp/runtime',
               '2026-06-01T03:00:00+00:00', '2026-06-01T03:00:01+00:00',
               '{\"source\":\"agent_hook\"}'),
              ('canonical-codex-null', NULL, 'probe', 'codex', '/tmp/runtime',
               '2026-06-01T04:00:00+00:00', '2026-06-01T04:00:01+00:00',
               '{\"source\":\"agent_hook\"}'),
              ('legacy-claude', 'evt-2', 'helper', 'claude', '/tmp/claude',
               '2026-06-01T00:00:00+00:00', '2026-06-01T00:00:01+00:00',
               '{\"source\":\"agent_hook\"}');
            INSERT INTO skill_usage_stats (
              skill_name, agent_id, runtime_root, usage_count, last_used_at
            ) VALUES
              ('probe', 'agents', '/tmp/runtime', 2, '2026-06-01T00:00:00+00:00'),
              ('probe', 'codex', '/tmp/runtime', 3, '2026-06-01T01:00:00+00:00'),
              ('helper', 'claude', '/tmp/claude', 1, '2026-06-01T00:00:00+00:00');
            ",
        )
        .unwrap();
    drop(connection);

    ensure_managed_layout(&managed_root).unwrap();

    let connection = rusqlite::Connection::open(&paths.database_path).unwrap();
    assert_eq!(
        current_database_schema_version(&connection).unwrap(),
        LATEST_DATABASE_SCHEMA_VERSION
    );
    let event_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM skill_usage_events WHERE event_id = 'evt-1'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(event_count, 1);
    let helper_agent: String = connection
        .query_row(
            "SELECT agent_id FROM skill_usage_events WHERE event_id = 'evt-2'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(helper_agent, "claude-code");
    let probe_count: i64 = connection
        .query_row(
            "
            SELECT usage_count
            FROM skill_usage_stats
            WHERE skill_name = 'probe' AND agent_id = 'codex' AND runtime_root = '/tmp/runtime'
            ",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(probe_count, 4);
    let probe_event_count: i64 = connection
        .query_row(
            "
            SELECT COUNT(*)
            FROM skill_usage_events
            WHERE skill_name = 'probe' AND agent_id = 'codex' AND runtime_root = '/tmp/runtime'
            ",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(probe_event_count, probe_count);
    let probe_last_used_at: String = connection
        .query_row(
            "
            SELECT last_used_at
            FROM skill_usage_stats
            WHERE skill_name = 'probe' AND agent_id = 'codex' AND runtime_root = '/tmp/runtime'
            ",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(probe_last_used_at, "2026-06-01T04:00:00+00:00");
    let legacy_stats: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM skill_usage_stats WHERE agent_id IN ('agents', 'claude')",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(legacy_stats, 0);
}

#[test]
fn usage_rankings_reject_invalid_filters() {
    let managed_root = temp_dir("usage-rankings-invalid").join("SkillBox");
    let as_of = Utc::now();

    let agent_error = list_skill_usage_rankings_at(
        SkillUsageRankingRequest {
            agent_id: Some("bad agent".to_string()),
            ..SkillUsageRankingRequest::default()
        },
        &managed_root,
        as_of,
    )
    .unwrap_err();
    assert!(agent_error.contains("Invalid usage agent id"));

    let workspace_error = list_skill_usage_rankings_at(
        SkillUsageRankingRequest {
            workspace_root: Some(PathBuf::from("relative/skills")),
            ..SkillUsageRankingRequest::default()
        },
        &managed_root,
        as_of,
    )
    .unwrap_err();
    assert!(workspace_error.contains("absolute path"));
}

#[test]
fn usage_ranking_request_accepts_desktop_camel_case_fields() {
    let request: SkillUsageRankingRequest = serde_json::from_value(serde_json::json!({
        "range": "last_7_days",
        "skillType": "system",
        "agentId": "codex",
        "workspaceRoot": "/Users/example/.codex/skills",
        "includeUnmanaged": true
    }))
    .unwrap();

    assert_eq!(request.range, SkillUsageRankingRange::Last7Days);
    assert_eq!(request.skill_type, Some(SkillUsageRankingSkillType::System));
    assert_eq!(request.agent_id.as_deref(), Some("codex"));
    assert_eq!(
        request.workspace_root,
        Some(PathBuf::from("/Users/example/.codex/skills"))
    );
    assert!(request.include_unmanaged);
}

#[test]
fn workspace_and_import_candidates_include_usage_counts() {
    let root = temp_dir("usage-workspace-candidates");
    let managed_root = root.join("SkillBox");
    let workspace_root = root.join("project").join(".agents").join("skills");
    make_skill(&workspace_root.join("alpha"), "alpha", "Alpha skill");

    record_test_call(
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
    record_test_call(
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
    record_skill_usage(
        RecordSkillUsageRequest {
            skill_name: "alpha".to_string(),
            agent_id: "agents".to_string(),
            runtime_root: workspace_root.clone(),
            event_id: Some("history-reference-alpha".to_string()),
            used_at: Some("2026-06-02T12:02:00Z".to_string()),
            prompt_excerpt: None,
            metadata: Some(serde_json::json!({ "source": "manual" })),
        },
        &managed_root,
    )
    .unwrap();

    let candidates =
        scan_import_candidates(std::slice::from_ref(&workspace_root), &managed_root).unwrap();
    let workspaces = list_workspaces(&managed_root).unwrap();

    assert_eq!(workspace(&workspaces, &workspace_root).usage_count, 2);
    assert_eq!(workspace(&workspaces, &workspace_root).reference_count, 1);
    assert_eq!(candidate(&candidates.candidates, "alpha").usage_count, 2);
}

#[test]
fn workspace_usage_counts_symlinked_runtime_skill_calls() {
    let root = temp_dir("usage-workspace-runtime-symlink");
    let managed_root = root.join("SkillBox");
    let agents_root = root.join(".agents").join("skills");
    let claude_root = root.join(".claude").join("skills");
    let agents_skill = agents_root.join("lark-mail");
    let claude_skill = claude_root.join("lark-mail");

    make_skill(&agents_skill, "lark-mail", "Lark mail skill");
    fs::create_dir_all(&claude_root).unwrap();
    symlink_dir(&agents_skill, &claude_skill).unwrap();
    record_test_call(
        RecordSkillUsageRequest {
            skill_name: "lark-mail".to_string(),
            agent_id: "agents".to_string(),
            runtime_root: agents_root.clone(),
            event_id: None,
            used_at: Some("2026-06-02T12:00:00Z".to_string()),
            prompt_excerpt: None,
            metadata: None,
        },
        &managed_root,
    )
    .unwrap();

    scan_workspaces_under(&root, &managed_root).unwrap();
    let workspaces = list_workspaces(&managed_root).unwrap();

    assert_eq!(workspace(&workspaces, &claude_root).usage_count, 1);
}

#[test]
fn record_skill_usage_rejects_content_metadata() {
    let root = temp_dir("usage-metadata-content");
    let managed_root = root.join("SkillBox");
    let runtime_root = root.join(".codex").join("skills");

    let error = record_test_call(
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

    record_trusted_generated_skill_usage(
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
