use super::*;
use crate::test_support::*;
use std::fs;

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
    assert_eq!(first.recorded[0].agent_id, "codex");
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
fn usage_backfill_imports_codex_session_skills_with_dedupe() {
    let root = temp_dir("usage-backfill-codex-sessions");
    let home = root.join("home");
    let managed_root = root.join("SkillBox");
    let runtime_root = home.join(".codex").join("skills");
    let skill_root = runtime_root.join("probe");
    fs::create_dir_all(&skill_root).unwrap();
    fs::write(
        skill_root.join("SKILL.md"),
        "---\nname: probe\ndescription: Probe\n---\n",
    )
    .unwrap();

    let sessions_root = home
        .join(".codex")
        .join("sessions")
        .join("2026")
        .join("07")
        .join("23");
    fs::create_dir_all(&sessions_root).unwrap();
    let session_path = sessions_root
        .join("rollout-2026-07-23T10-00-00-019f8ce8-837c-7fc3-a20c-415aa87e6856.jsonl");
    let skill_path = skill_root.join("SKILL.md");
    fs::write(
        &session_path,
        format!(
            "{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n",
            serde_json::json!({
                "timestamp": "2026-07-23T02:00:00.000Z",
                "type": "session_meta",
                "payload": {
                    "session_id": "session-thread-1",
                    "id": "019f8ce8-837c-7fc3-a20c-415aa87e6856"
                }
            }),
            serde_json::json!({
                "timestamp": "2026-07-23T02:00:01.000Z",
                "type": "turn_context",
                "payload": { "turn_id": "turn-1" }
            }),
            serde_json::json!({
                "timestamp": "2026-07-23T02:00:02.000Z",
                "type": "response_item",
                "payload": {
                    "type": "message",
                    "role": "user",
                    "content": [{
                        "type": "input_text",
                        "text": format!(
                            "Use [$probe]({0}).\n<skill>\n<name>probe</name>\n<path>{0}</path>\n</skill>",
                            skill_path.display()
                        )
                    }]
                }
            }),
            serde_json::json!({
                "timestamp": "2026-07-23T02:00:03.000Z",
                "type": "response_item",
                "payload": {
                    "type": "message",
                    "role": "assistant",
                    "content": [{
                        "type": "output_text",
                        "text": format!(
                            "Echo only: <skill><name>not-invoked</name><path>{}</path></skill>",
                            skill_path.display()
                        )
                    }]
                }
            }),
            serde_json::json!({
                "timestamp": "2026-07-23T02:10:00.000Z",
                "type": "turn_context",
                "payload": { "turn_id": "turn-2" }
            }),
            serde_json::json!({
                "timestamp": "2026-07-23T02:10:01.000Z",
                "type": "response_item",
                "payload": {
                    "type": "message",
                    "role": "user",
                    "content": [{
                        "type": "input_text",
                        "text": format!(
                            "Use probe again.\n<skill>\n<name>probe</name>\n<path>{}</path>\n</skill>",
                            skill_path.display()
                        )
                    }]
                }
            }),
            serde_json::json!({
                "timestamp": "2026-07-23T02:20:00.000Z",
                "type": "turn_context",
                "payload": { "turn_id": "turn-3" }
            }),
            serde_json::json!({
                "timestamp": "2026-07-23T02:20:01.000Z",
                "type": "event_msg",
                "payload": {
                    "type": "user_message",
                    "message": format!("Explicit only: [$probe]({})", skill_path.display())
                }
            }),
            serde_json::json!({
                "timestamp": "2026-07-23T02:30:00.000Z",
                "type": "turn_context",
                "payload": { "turn_id": "turn-code-example" }
            }),
            serde_json::json!({
                "timestamp": "2026-07-23T02:30:01.000Z",
                "type": "event_msg",
                "payload": {
                    "type": "user_message",
                    "message": "Code examples are not calls: <skill><name>probe</name><path>{}</path></skill> and [$probe]({})"
                }
            })
        ),
    )
    .unwrap();

    let first = backfill_codex_session_usage_for_home(
        BackfillCodexSessionUsageRequest {
            include_archived: false,
            sessions_root: Some(home.join(".codex").join("sessions")),
            archived_sessions_root: None,
        },
        &home,
        &managed_root,
    )
    .unwrap();
    assert_eq!(first.scanned_files, 1);
    assert_eq!(first.discovered, 3);
    assert_eq!(first.recorded, 3);
    assert_eq!(first.deduplicated, 0);

    let second = backfill_codex_session_usage_for_home(
        BackfillCodexSessionUsageRequest {
            include_archived: false,
            sessions_root: Some(home.join(".codex").join("sessions")),
            archived_sessions_root: None,
        },
        &home,
        &managed_root,
    )
    .unwrap();
    assert_eq!(second.discovered, 3);
    assert_eq!(second.recorded, 0);
    assert_eq!(second.deduplicated, 3);

    let rankings = list_skill_usage_rankings_at(
        SkillUsageRankingRequest {
            range: SkillUsageRankingRange::AllTime,
            include_unmanaged: true,
            ..SkillUsageRankingRequest::default()
        },
        &managed_root,
        DateTime::parse_from_rfc3339("2026-07-24T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc),
    )
    .unwrap();
    let probe = rankings
        .rows
        .iter()
        .find(|row| row.skill_name == "probe")
        .expect("probe row");
    assert_eq!(probe.usage_count, 3);
    assert_eq!(probe.confirmed_count, 0);
    assert_eq!(probe.inferred_count, 3);
    assert_eq!(probe.reference_count, 0);
    assert_eq!(rankings.total_calls, 3);
    assert_eq!(rankings.total_confirmed_calls, 0);
    assert_eq!(rankings.total_inferred_calls, 3);
    assert_eq!(rankings.total_history_references, 0);
    assert_eq!(rankings.coverage.agent_hook_calls, 0);
    assert_eq!(rankings.coverage.codex_session_backfill_calls, 3);
    assert_eq!(rankings.coverage.other_observed_calls, 0);
    assert_eq!(rankings.coverage.scanned_codex_session_files, 1);
    assert_eq!(
        rankings.coverage.earliest_event_at.as_deref(),
        Some("2026-07-23T02:00:01+00:00")
    );
    assert_eq!(
        rankings.coverage.latest_event_at.as_deref(),
        Some("2026-07-23T02:20:00+00:00")
    );
    assert_eq!(
        probe.last_used_at.as_deref(),
        Some("2026-07-23T02:20:00+00:00")
    );
}

#[test]
fn usage_backfill_uses_session_cwd_for_managed_workspace_identity() {
    let root = temp_dir("usage-backfill-managed-workspace");
    let home = root.join("home");
    let managed_root = home.join("SkillBox");
    let paths = ensure_managed_layout(&managed_root).unwrap();
    let managed_skill = paths.user_skills_root.join("probe");
    make_skill(&managed_skill, "probe", "Managed probe");
    let first_project = home.join("Projects").join("first");
    let second_project = home.join("Projects").join("second");
    let first_runtime = first_project.join(".codex").join("skills");
    let second_runtime = second_project.join(".codex").join("skills");
    fs::create_dir_all(&first_runtime).unwrap();
    fs::create_dir_all(&second_runtime).unwrap();
    symlink_dir(&managed_skill, &first_runtime.join("probe")).unwrap();
    symlink_dir(&managed_skill, &second_runtime.join("probe")).unwrap();

    let sessions_root = home.join(".codex").join("sessions");
    fs::create_dir_all(&sessions_root).unwrap();
    let session_path = sessions_root.join("rollout-managed-workspace.jsonl");
    let skill_path = managed_skill.join("SKILL.md");
    fs::write(
        &session_path,
        format!(
            "{}\n{}\n{}\n",
            serde_json::json!({
                "timestamp": "2026-07-23T02:00:00.000Z",
                "type": "session_meta",
                "payload": {
                    "id": "session-managed-workspace",
                    "cwd": second_project
                }
            }),
            serde_json::json!({
                "timestamp": "2026-07-23T02:00:01.000Z",
                "type": "turn_context",
                "payload": { "turn_id": "turn-1" }
            }),
            serde_json::json!({
                "timestamp": "2026-07-23T02:00:02.000Z",
                "type": "response_item",
                "payload": {
                    "type": "message",
                    "role": "user",
                    "content": [{
                        "type": "input_text",
                        "text": format!(
                            "<skill>\n<name>probe</name>\n<path>{}</path>\n</skill>",
                            skill_path.display()
                        )
                    }]
                }
            })
        ),
    )
    .unwrap();

    let backfill = backfill_codex_session_usage_for_home(
        BackfillCodexSessionUsageRequest {
            include_archived: false,
            sessions_root: Some(sessions_root),
            archived_sessions_root: None,
        },
        &home,
        &managed_root,
    )
    .unwrap();
    assert_eq!(backfill.recorded, 1);

    let runtime_roots = runtime_roots_under(&home);
    let hook_request = usage_request_from_skill_ref_with_roots(UsageRequestFromSkillRef {
        skill_ref: &HookSkillRef {
            name: "probe".to_string(),
            path: skill_path,
            prompt_excerpt: None,
        },
        hook_agent: "codex",
        session_id: "session-managed-workspace",
        turn_id: Some("turn-1"),
        index: 0,
        hook_event: "Stop",
        model: "gpt-test",
        runtime_roots: Some(&runtime_roots),
        preferred_runtime_context: Some(&second_project),
    })
    .unwrap();
    let hook_record = record_trusted_generated_skill_usage(hook_request, &managed_root).unwrap();

    assert!(hook_record.deduplicated);
    assert!(hook_record.upgraded);
    assert_eq!(
        hook_record.evidence_class,
        SkillUsageEvidenceClass::Confirmed
    );
    assert_eq!(
        hook_record.runtime_root,
        fs::canonicalize(&second_runtime).unwrap()
    );
    let usage = load_usage_by_runtime(&paths.database_path).unwrap();
    assert_eq!(usage.len(), 1);
    assert_eq!(
        usage
            .get(
                &fs::canonicalize(&second_runtime)
                    .unwrap()
                    .to_string_lossy()
                    .to_string()
            )
            .unwrap()
            .usage_count,
        1
    );
    let audit = usage_audit(&managed_root).unwrap();
    assert_eq!(audit.total_calls, 1);
    assert_eq!(audit.confirmed_calls, 1);
    assert_eq!(audit.inferred_calls, 0);
    assert_eq!(audit.history_references, 0);
    assert!(audit.source_counts.iter().any(|source| {
        source.source == "codex_session_backfill"
            && source.evidence_class == SkillUsageEvidenceClass::Inferred
            && source.count == 1
    }));
    assert!(audit.source_counts.iter().any(|source| {
        source.source == "agent_hook"
            && source.evidence_class == SkillUsageEvidenceClass::Confirmed
            && source.count == 1
    }));
}

#[test]
fn usage_backfill_counts_invalid_json_lines_as_skipped_errors() {
    let root = temp_dir("usage-backfill-invalid-json");
    let home = root.join("home");
    let managed_root = root.join("SkillBox");
    let runtime_root = home.join(".codex").join("skills");
    let skill_root = runtime_root.join("probe");
    fs::create_dir_all(&skill_root).unwrap();
    fs::write(
        skill_root.join("SKILL.md"),
        "---\nname: probe\ndescription: Probe\n---\n",
    )
    .unwrap();

    let sessions_root = home.join(".codex").join("sessions");
    fs::create_dir_all(&sessions_root).unwrap();
    let session_path = sessions_root.join("rollout-broken.jsonl");
    let skill_path = skill_root.join("SKILL.md");
    fs::write(
        &session_path,
        format!(
            "{}\nnot-json\n{}\n{}\n",
            serde_json::json!({
                "timestamp": "2026-07-23T02:00:00.000Z",
                "type": "session_meta",
                "payload": { "id": "session-broken" }
            }),
            serde_json::json!({
                "timestamp": "2026-07-23T02:00:01.000Z",
                "type": "turn_context",
                "payload": { "turn_id": "turn-1" }
            }),
            serde_json::json!({
                "timestamp": "2026-07-23T02:00:02.000Z",
                "type": "response_item",
                "payload": {
                    "type": "message",
                    "role": "user",
                    "content": [{
                        "type": "input_text",
                        "text": format!(
                            "Use probe.\n<skill>\n<name>probe</name>\n<path>{}</path>\n</skill>",
                            skill_path.display()
                        )
                    }]
                }
            })
        ),
    )
    .unwrap();

    let result = backfill_codex_session_usage_for_home(
        BackfillCodexSessionUsageRequest {
            include_archived: false,
            sessions_root: Some(sessions_root),
            archived_sessions_root: None,
        },
        &home,
        &managed_root,
    )
    .unwrap();

    assert_eq!(result.scanned_files, 1);
    assert_eq!(result.discovered, 1);
    assert_eq!(result.recorded, 1);
    assert_eq!(result.skipped, 1);
    assert!(result
        .errors
        .iter()
        .any(|error| error.contains("invalid JSON line")));
}

#[test]
fn usage_backfill_ignores_non_rollouts_and_symlinked_entries() {
    let root = temp_dir("usage-backfill-file-boundary");
    let home = root.join("home");
    let managed_root = root.join("SkillBox");
    let sessions_root = home.join(".codex").join("sessions");
    let nested = sessions_root.join("nested");
    let outside = root.join("outside");
    fs::create_dir_all(&nested).unwrap();
    fs::create_dir_all(&outside).unwrap();
    fs::write(nested.join("rollout-valid.jsonl"), "{}\n").unwrap();
    fs::write(nested.join("notes.jsonl"), "{}\n").unwrap();
    fs::write(outside.join("rollout-outside.jsonl"), "{}\n").unwrap();
    std::os::unix::fs::symlink(
        outside.join("rollout-outside.jsonl"),
        nested.join("rollout-linked.jsonl"),
    )
    .unwrap();
    std::os::unix::fs::symlink(&outside, sessions_root.join("linked-directory")).unwrap();

    let result = backfill_codex_session_usage_for_home(
        BackfillCodexSessionUsageRequest {
            include_archived: false,
            sessions_root: Some(sessions_root),
            archived_sessions_root: None,
        },
        &home,
        &managed_root,
    )
    .unwrap();

    assert_eq!(result.scanned_files, 1);
    assert_eq!(result.discovered, 0);
    assert_eq!(result.recorded, 0);
}

#[test]
fn usage_preview_import_resolves_unmanaged_skill_from_runtime_root() {
    let root = temp_dir("usage-preview-import");
    let managed_root = root.join("SkillBox");
    let runtime_root = root.join(".codex").join("skills");
    let skill_root = runtime_root.join("probe");
    make_skill(&skill_root, "probe", "Probe skill");

    record_test_call(
        RecordSkillUsageRequest {
            skill_name: "probe".to_string(),
            agent_id: "codex".to_string(),
            runtime_root: runtime_root.clone(),
            event_id: Some("preview-import-1".to_string()),
            used_at: Some("2026-07-23T02:00:00Z".to_string()),
            prompt_excerpt: None,
            metadata: None,
        },
        &managed_root,
    )
    .unwrap();

    let candidate = preview_usage_skill_import("probe", &managed_root).unwrap();
    assert_eq!(candidate.name, "probe");
    assert_eq!(candidate.import_status, ImportCandidateStatus::Importable);
    assert!(candidate.is_selected);
    assert_eq!(
        candidate.source_path,
        fs::canonicalize(&skill_root).unwrap()
    );

    import_skill(&skill_root, SkillKind::User, &managed_root).unwrap();
    let error = preview_usage_skill_import("probe", &managed_root).unwrap_err();
    assert!(error.contains("already imported"));
}

#[test]
fn usage_preview_import_recovers_from_deletion_backup_when_runtime_root_is_gone() {
    let root = temp_dir("usage-preview-import-backup");
    let managed_root = root.join("SkillBox");
    let paths = ensure_managed_layout(&managed_root).unwrap();
    let missing_runtime = paths.remote_skills_root.join("probe").join("versions");
    let backup_root = paths
        .root
        .join("backups")
        .join("deletions")
        .join("probe-100");
    let version_dir = backup_root.join("versions").join("manual-abc");
    make_skill(&version_dir, "probe", "Recovered probe");

    record_test_call(
        RecordSkillUsageRequest {
            skill_name: "probe".to_string(),
            agent_id: "codex".to_string(),
            runtime_root: missing_runtime,
            event_id: Some("preview-import-backup-1".to_string()),
            used_at: Some("2026-07-23T02:00:00Z".to_string()),
            prompt_excerpt: None,
            metadata: None,
        },
        &managed_root,
    )
    .unwrap();

    let candidate = preview_usage_skill_import("probe", &managed_root).unwrap();
    assert_eq!(candidate.name, "probe");
    assert_eq!(candidate.import_status, ImportCandidateStatus::Importable);
    assert_eq!(
        fs::canonicalize(&candidate.source_path).unwrap(),
        fs::canonicalize(&version_dir).unwrap()
    );
}

#[test]
fn usage_preview_import_prefers_deletion_backup_current_over_other_versions() {
    let root = temp_dir("usage-preview-import-backup-current");
    let managed_root = root.join("SkillBox");
    let paths = ensure_managed_layout(&managed_root).unwrap();
    let missing_runtime = paths.remote_skills_root.join("probe").join("versions");
    let backup_root = paths
        .root
        .join("backups")
        .join("deletions")
        .join("probe-200");
    let stale_version = backup_root.join("versions").join("aaaa-stale");
    let current = backup_root.join("current");
    make_skill(&stale_version, "probe", "Stale probe");
    make_skill(&current, "probe", "Current probe");

    record_test_call(
        RecordSkillUsageRequest {
            skill_name: "probe".to_string(),
            agent_id: "codex".to_string(),
            runtime_root: missing_runtime,
            event_id: Some("preview-import-backup-current-1".to_string()),
            used_at: Some("2026-07-23T02:00:00Z".to_string()),
            prompt_excerpt: None,
            metadata: None,
        },
        &managed_root,
    )
    .unwrap();

    let candidate = preview_usage_skill_import("probe", &managed_root).unwrap();
    assert_eq!(candidate.name, "probe");
    assert_eq!(
        fs::canonicalize(&candidate.source_path).unwrap(),
        fs::canonicalize(&current).unwrap()
    );
    assert!(fs::read_to_string(candidate.source_path.join("SKILL.md"))
        .unwrap()
        .contains("Current probe"));
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
    assert_eq!(global_claude.display_name, "Claude Code");
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
fn scan_import_candidates_includes_symlinks_to_discovered_runtime_roots() {
    let root = temp_dir("candidate-runtime-symlink");
    let agents_root = root.join(".agents").join("skills");
    let claude_root = root.join(".claude").join("skills");
    let managed_root = root.join("SkillBox");
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

    let candidates =
        scan_import_candidates(std::slice::from_ref(&claude_root), &managed_root).unwrap();
    let candidate = candidate(&candidates.candidates, "lark-mail");

    assert_eq!(candidate.source_root, Some(claude_root));
    assert_eq!(candidate.source_path, claude_skill);
    assert_eq!(candidate.real_path, fs::canonicalize(agents_skill).unwrap());
    assert!(candidate.is_symlink);
    assert_eq!(
        candidate.symlink_target_path,
        Some(candidate.real_path.clone())
    );
    assert_eq!(candidate.usage_count, 1);
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
fn deployment_compatibility_preview_is_read_only_and_apply_requires_fresh_confirmation() {
    let root = temp_dir("deployment-compatibility");
    let source = root.join("source").join("demo");
    let managed_root = root.join("SkillBox");
    let target_root = root.join("project/.codex/skills");
    fs::create_dir_all(&target_root).unwrap();
    fs::create_dir_all(&source).unwrap();
    fs::write(
        source.join("SKILL.md"),
        "---
name: demo
description: Demo skill
optional-runtime-field: preserved
---
# Demo
",
    )
    .unwrap();
    fs::write(source.join("asset.txt"), "before").unwrap();
    import_skill(&source, SkillKind::User, &managed_root).unwrap();
    let workspace = add_workspace(
        WorkspaceAddRequest {
            path: target_root.clone(),
            kind: WorkspaceKind::User,
        },
        &managed_root,
    )
    .unwrap();
    assert_eq!(workspace.profile_id, "codex");

    let preview = preview_skill_deployment(
        DeploymentCompatibilityPreviewRequest {
            skill_name: "demo".to_string(),
            target_root: target_root.clone(),
        },
        &managed_root,
    )
    .unwrap();
    assert_eq!(preview.status, CompatibilityStatus::Warnings);
    assert_eq!(preview.profile.id, "codex");
    assert_eq!(preview.root_key, "skills");
    assert_eq!(preview.issues.len(), 1);
    assert_eq!(preview.issues[0].code, "unknown_optional_frontmatter");
    assert!(!target_root.join("demo").exists());

    let warning_error = apply_skill_deployment(
        DeploymentCompatibilityApplyRequest {
            skill_name: "demo".to_string(),
            target_root: target_root.clone(),
            preview_id: preview.preview_id.clone(),
            confirm_warnings: false,
        },
        &managed_root,
    )
    .unwrap_err();
    assert!(warning_error.contains("Confirm the warnings"));
    assert!(!target_root.join("demo").exists());

    fs::write(
        managed_root.join("user-skills/demo/asset.txt"),
        "changed after preview",
    )
    .unwrap();
    let stale_error = apply_skill_deployment(
        DeploymentCompatibilityApplyRequest {
            skill_name: "demo".to_string(),
            target_root: target_root.clone(),
            preview_id: preview.preview_id,
            confirm_warnings: true,
        },
        &managed_root,
    )
    .unwrap_err();
    assert!(stale_error.contains("preview is stale"));
    assert!(!target_root.join("demo").exists());

    let fresh = preview_skill_deployment(
        DeploymentCompatibilityPreviewRequest {
            skill_name: "demo".to_string(),
            target_root: target_root.clone(),
        },
        &managed_root,
    )
    .unwrap();
    let deployment = apply_skill_deployment(
        DeploymentCompatibilityApplyRequest {
            skill_name: "demo".to_string(),
            target_root,
            preview_id: fresh.preview_id,
            confirm_warnings: true,
        },
        &managed_root,
    )
    .unwrap();
    assert!(fs::symlink_metadata(deployment.target_path)
        .unwrap()
        .file_type()
        .is_symlink());
}

#[test]
fn deployment_compatibility_apply_holds_the_shared_mutation_lock_during_revalidation() {
    let root = temp_dir("deployment-compatibility-shared-lock");
    let source = root.join("source/demo");
    let managed_root = root.join("SkillBox");
    let target_root = root.join("project/.codex/skills");
    fs::create_dir_all(&target_root).unwrap();
    make_skill(&source, "demo", "Demo skill");
    import_skill(&source, SkillKind::User, &managed_root).unwrap();
    add_workspace(
        WorkspaceAddRequest {
            path: target_root.clone(),
            kind: WorkspaceKind::User,
        },
        &managed_root,
    )
    .unwrap();
    let preview = preview_skill_deployment(
        DeploymentCompatibilityPreviewRequest {
            skill_name: "demo".to_string(),
            target_root: target_root.clone(),
        },
        &managed_root,
    )
    .unwrap();
    let _lock = acquire_user_skills_mutation_lock(&managed_root).unwrap();

    let error = apply_skill_deployment(
        DeploymentCompatibilityApplyRequest {
            skill_name: "demo".to_string(),
            target_root: target_root.clone(),
            preview_id: preview.preview_id,
            confirm_warnings: false,
        },
        &managed_root,
    )
    .unwrap_err();

    assert!(error.contains("Another user-skills mutation"));
    assert!(!target_root.join("demo").exists());
}

#[test]
fn deployment_compatibility_blocks_invalid_frontmatter_and_existing_content() {
    let root = temp_dir("deployment-compatibility-blocked");
    let source = root.join("source").join("demo");
    let managed_root = root.join("SkillBox");
    let target_root = root.join("project/.agents/skills");
    make_skill(&source, "demo", "Demo skill");
    import_skill(&source, SkillKind::User, &managed_root).unwrap();
    fs::create_dir_all(target_root.join("demo")).unwrap();
    add_workspace(
        WorkspaceAddRequest {
            path: target_root.clone(),
            kind: WorkspaceKind::User,
        },
        &managed_root,
    )
    .unwrap();
    fs::write(
        managed_root.join("user-skills/demo/SKILL.md"),
        "---
name: [demo
---
",
    )
    .unwrap();

    let preview = preview_skill_deployment(
        DeploymentCompatibilityPreviewRequest {
            skill_name: "demo".to_string(),
            target_root,
        },
        &managed_root,
    )
    .unwrap();
    assert_eq!(preview.status, CompatibilityStatus::Blocked);
    assert!(preview
        .issues
        .iter()
        .any(|issue| issue.code == "invalid_frontmatter"));
    assert!(preview
        .issues
        .iter()
        .any(|issue| issue.code == "existing_non_symlink_target"));
    assert!(apply_skill_deployment(
        DeploymentCompatibilityApplyRequest {
            skill_name: "demo".to_string(),
            target_root: preview.target_root.clone(),
            preview_id: preview.preview_id,
            confirm_warnings: true,
        },
        &managed_root,
    )
    .unwrap_err()
    .contains("blocked"));
}

#[test]
fn every_runtime_profile_has_valid_warning_blocked_and_malformed_fixtures() {
    let profiles = [
        ("agents", ".agents/skills"),
        ("codex", ".codex/skills"),
        ("claude-code", ".claude/skills"),
        ("cursor", ".cursor/skills"),
        ("custom-skill-md", "custom-skills"),
    ];
    let fixtures = [
        (
            "valid",
            "---\nname: demo\ndescription: Demo skill\n---\n# Demo\n",
            CompatibilityStatus::Compatible,
            None,
        ),
        (
            "warning",
            "---\nname: demo\ndescription: Demo skill\ntools:\n  - shell\n---\n# Demo\n",
            CompatibilityStatus::Warnings,
            Some("unknown_optional_frontmatter"),
        ),
        (
            "blocked",
            "---\nname: another-skill\ndescription: Demo skill\n---\n# Demo\n",
            CompatibilityStatus::Blocked,
            Some("skill_name_mismatch"),
        ),
        (
            "malformed",
            "---\nname: [demo\n---\n# Demo\n",
            CompatibilityStatus::Blocked,
            Some("invalid_frontmatter"),
        ),
    ];

    for (profile_id, relative_root) in profiles {
        for (fixture, content, expected_status, expected_issue) in fixtures {
            let root = temp_dir(&format!("compatibility-{profile_id}-{fixture}"));
            let managed_root = root.join("SkillBox");
            let source = root.join("source/demo");
            let target_root = root.join("project").join(relative_root);
            fs::create_dir_all(&source).unwrap();
            fs::create_dir_all(&target_root).unwrap();
            fs::write(
                source.join("SKILL.md"),
                "---\nname: demo\ndescription: Demo skill\n---\n# Demo\n",
            )
            .unwrap();
            import_skill(&source, SkillKind::User, &managed_root).unwrap();
            fs::write(managed_root.join("user-skills/demo/SKILL.md"), content).unwrap();
            let workspace = add_workspace(
                WorkspaceAddRequest {
                    path: target_root.clone(),
                    kind: WorkspaceKind::User,
                },
                &managed_root,
            )
            .unwrap();
            assert_eq!(workspace.profile_id, profile_id);
            if profile_id == "custom-skill-md" {
                assert_eq!(workspace.root_key, "exact");
            }

            let report = preview_skill_deployment(
                DeploymentCompatibilityPreviewRequest {
                    skill_name: "demo".to_string(),
                    target_root,
                },
                &managed_root,
            )
            .unwrap();
            if profile_id == "custom-skill-md" {
                assert_eq!(report.profile.id, "custom-skill-md");
                assert_eq!(report.root_key, "exact");
            }
            assert_eq!(
                report.status, expected_status,
                "{profile_id} {fixture} status"
            );
            if let Some(expected_issue) = expected_issue {
                assert!(
                    report
                        .issues
                        .iter()
                        .any(|issue| issue.code == expected_issue),
                    "{profile_id} {fixture} should report {expected_issue}"
                );
            }
        }
    }
}

#[test]
fn deployment_compatibility_preview_rejects_target_state_changes() {
    let root = temp_dir("deployment-target-state-stale");
    let source = root.join("source/demo");
    let managed_root = root.join("SkillBox");
    let target_root = root.join("project/.cursor/skills");
    make_skill(&source, "demo", "Demo skill");
    fs::create_dir_all(&target_root).unwrap();
    import_skill(&source, SkillKind::User, &managed_root).unwrap();
    add_workspace(
        WorkspaceAddRequest {
            path: target_root.clone(),
            kind: WorkspaceKind::User,
        },
        &managed_root,
    )
    .unwrap();
    let preview = preview_skill_deployment(
        DeploymentCompatibilityPreviewRequest {
            skill_name: "demo".to_string(),
            target_root: target_root.clone(),
        },
        &managed_root,
    )
    .unwrap();

    fs::create_dir_all(target_root.join("demo")).unwrap();
    let error = apply_skill_deployment(
        DeploymentCompatibilityApplyRequest {
            skill_name: "demo".to_string(),
            target_root,
            preview_id: preview.preview_id,
            confirm_warnings: false,
        },
        &managed_root,
    )
    .unwrap_err();
    assert!(error.contains("preview is stale"));
}

#[test]
fn deployment_compatibility_preview_rejects_workspace_profile_changes() {
    let root = temp_dir("deployment-profile-stale");
    let source = root.join("source/demo");
    let managed_root = root.join("SkillBox");
    let target_root = root.join("project/.codex/skills");
    make_skill(&source, "demo", "Demo skill");
    fs::create_dir_all(&target_root).unwrap();
    import_skill(&source, SkillKind::User, &managed_root).unwrap();
    add_workspace(
        WorkspaceAddRequest {
            path: target_root.clone(),
            kind: WorkspaceKind::User,
        },
        &managed_root,
    )
    .unwrap();
    let preview = preview_skill_deployment(
        DeploymentCompatibilityPreviewRequest {
            skill_name: "demo".to_string(),
            target_root: target_root.clone(),
        },
        &managed_root,
    )
    .unwrap();
    let database_path = managed_paths(managed_root.clone()).database_path;
    rusqlite::Connection::open(database_path)
        .unwrap()
        .execute(
            "UPDATE workspaces SET profile_id = 'agents' WHERE canonical_path = ?1",
            [fs::canonicalize(&target_root)
                .unwrap()
                .to_string_lossy()
                .to_string()],
        )
        .unwrap();

    let error = apply_skill_deployment(
        DeploymentCompatibilityApplyRequest {
            skill_name: "demo".to_string(),
            target_root,
            preview_id: preview.preview_id,
            confirm_warnings: false,
        },
        &managed_root,
    )
    .unwrap_err();
    assert!(error.contains("preview is stale"));
}

#[test]
fn deployment_compatibility_blocks_unsupported_persisted_format() {
    let root = temp_dir("deployment-format-blocked");
    let source = root.join("source/demo");
    let managed_root = root.join("SkillBox");
    let target_root = root.join("project/.claude/skills");
    make_skill(&source, "demo", "Demo skill");
    fs::create_dir_all(&target_root).unwrap();
    import_skill(&source, SkillKind::User, &managed_root).unwrap();
    add_workspace(
        WorkspaceAddRequest {
            path: target_root.clone(),
            kind: WorkspaceKind::User,
        },
        &managed_root,
    )
    .unwrap();
    rusqlite::Connection::open(managed_paths(managed_root.clone()).database_path)
        .unwrap()
        .execute(
            "UPDATE workspaces SET format = 'native_rules' WHERE canonical_path = ?1",
            [fs::canonicalize(&target_root)
                .unwrap()
                .to_string_lossy()
                .to_string()],
        )
        .unwrap();

    let preview = preview_skill_deployment(
        DeploymentCompatibilityPreviewRequest {
            skill_name: "demo".to_string(),
            target_root,
        },
        &managed_root,
    )
    .unwrap();
    assert_eq!(preview.status, CompatibilityStatus::Blocked);
    assert_eq!(preview.format, RuntimeFormat::Unsupported);
    assert!(preview
        .issues
        .iter()
        .any(|issue| issue.code == "format_mismatch"));
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
    let current = fs::canonicalize(&managed_root)
        .unwrap()
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
    let current = fs::canonicalize(&managed_root)
        .unwrap()
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
fn undeploy_refuses_active_import_source_workspace() {
    let root = temp_dir("undeploy-active-import-source");
    let source = root.join("source").join("demo");
    let managed_root = root.join("SkillBox");
    let runtime = root.join("runtime");
    make_skill(&source, "demo", "Demo skill");
    let imported = import_skill(&source, SkillKind::User, &managed_root).unwrap();
    let deployment = deploy_skill("demo", &managed_root, &runtime).unwrap();
    let paths = ensure_managed_layout(&managed_root).unwrap();
    let connection = open_database(&paths.database_path).unwrap();
    connection
        .execute(
            "INSERT INTO import_records (
                id, skill_name, type, source_path, source_root, managed_path,
                content_hash, backup_path, deployed_path, status, legacy
             ) VALUES ('active-undeploy-test', 'demo', 'user', ?1, ?2, ?3, ?4, ?5, ?1, 'active', 0)",
            params![
                deployment.target_path.to_string_lossy(),
                runtime.to_string_lossy(),
                imported.managed_path.to_string_lossy(),
                imported.content_hash,
                root.join("backup").to_string_lossy()
            ],
        )
        .unwrap();

    let error = undeploy_skill("demo", &managed_root, &runtime).unwrap_err();

    assert!(error.contains("Revert the import first"));
    assert!(fs::symlink_metadata(deployment.target_path)
        .unwrap()
        .file_type()
        .is_symlink());
}

#[test]
fn undeploy_refuses_active_import_source_through_symlinked_workspace_path() {
    let root = temp_dir("undeploy-active-import-alias");
    let source = root.join("source").join("demo");
    let managed_root = root.join("SkillBox");
    let runtime = root.join("runtime");
    let runtime_alias = root.join("runtime-alias");
    make_skill(&source, "demo", "Demo skill");
    let imported = import_skill(&source, SkillKind::User, &managed_root).unwrap();
    let deployment = deploy_skill("demo", &managed_root, &runtime).unwrap();
    symlink_dir(&runtime, &runtime_alias).unwrap();
    let paths = ensure_managed_layout(&managed_root).unwrap();
    let connection = open_database(&paths.database_path).unwrap();
    connection
        .execute(
            "INSERT INTO import_records (
                id, skill_name, type, source_path, source_root, managed_path,
                content_hash, backup_path, deployed_path, status, legacy
             ) VALUES ('active-undeploy-alias', 'demo', 'user', ?1, ?2, ?3, ?4, ?5, ?1, 'active', 0)",
            params![
                deployment.target_path.to_string_lossy(),
                runtime.to_string_lossy(),
                imported.managed_path.to_string_lossy(),
                imported.content_hash,
                root.join("backup").to_string_lossy()
            ],
        )
        .unwrap();

    let error = undeploy_skill("demo", &managed_root, &runtime_alias).unwrap_err();

    assert!(error.contains("Revert the import first"));
    assert!(fs::symlink_metadata(deployment.target_path)
        .unwrap()
        .file_type()
        .is_symlink());
}

#[test]
fn deletes_user_skill_from_managed_store_and_all_workspaces() {
    let root = temp_dir("delete-user-skill");
    let source = root.join("source").join("demo");
    let managed_root = root.join("SkillBox");
    let first_runtime = root.join("runtime-one");
    let second_runtime = root.join("runtime-two");
    make_skill(&source, "demo", "Demo skill");
    let imported = import_skill(&source, SkillKind::User, &managed_root).unwrap();
    let first = deploy_skill("demo", &managed_root, &first_runtime).unwrap();
    let second = deploy_skill("demo", &managed_root, &second_runtime).unwrap();
    set_skill_user_metadata(
        SkillUserMetadataUpdate {
            skill_name: "demo".to_string(),
            favorite: true,
            tags: vec!["test".to_string()],
        },
        &managed_root,
    )
    .unwrap();

    let preview = preview_delete_skill("demo", &managed_root).unwrap();
    assert!(preview.can_delete);
    assert_eq!(preview.deployments.len(), 2);
    let result = delete_skill(
        DeleteSkillRequest {
            skill_name: "demo".to_string(),
            preview_id: preview.preview_id,
            confirmed_skill_name: "demo".to_string(),
            actor: "test".to_string(),
        },
        &managed_root,
    )
    .unwrap();

    assert_eq!(result.kind, SkillKind::User);
    assert_eq!(result.removed_deployments.len(), 2);
    assert!(!imported.managed_path.exists());
    assert!(result.backup_path.join("SKILL.md").exists());
    assert!(fs::symlink_metadata(first.target_path).is_err());
    assert!(fs::symlink_metadata(second.target_path).is_err());
    assert!(managed_state(&managed_root).unwrap().skills.is_empty());
    assert!(list_skill_user_metadata(&managed_root).unwrap().is_empty());
}

#[test]
fn delete_skill_preview_blocks_active_import_without_mutating_files() {
    let root = temp_dir("delete-active-import");
    let source = root.join("source").join("demo");
    let managed_root = root.join("SkillBox");
    make_skill(&source, "demo", "Demo skill");
    let imported = import_skill(&source, SkillKind::User, &managed_root).unwrap();
    let paths = ensure_managed_layout(&managed_root).unwrap();
    let connection = open_database(&paths.database_path).unwrap();
    connection
        .execute(
            "INSERT INTO import_records (
                id, skill_name, type, source_path, managed_path, content_hash,
                backup_path, deployed_path, status, legacy
             ) VALUES ('active-delete-test', 'demo', 'user', ?1, ?2, ?3, ?4, ?5, 'active', 0)",
            params![
                source.to_string_lossy(),
                imported.managed_path.to_string_lossy(),
                imported.content_hash,
                root.join("backup").to_string_lossy(),
                source.to_string_lossy()
            ],
        )
        .unwrap();

    let preview = preview_delete_skill("demo", &managed_root).unwrap();

    assert!(!preview.can_delete);
    assert!(preview.blockers[0].contains("active import record"));
    assert!(imported.managed_path.exists());
}

#[test]
fn delete_skill_preview_blocks_foreign_indexed_deployment() {
    let root = temp_dir("delete-foreign-deployment");
    let source = root.join("source").join("demo");
    let other = root.join("other").join("demo");
    let managed_root = root.join("SkillBox");
    let runtime = root.join("runtime");
    make_skill(&source, "demo", "Demo skill");
    make_skill(&other, "demo", "Other skill");
    let imported = import_skill(&source, SkillKind::User, &managed_root).unwrap();
    fs::create_dir_all(&runtime).unwrap();
    let target_path = runtime.join("demo");
    symlink_dir(&other, &target_path).unwrap();
    let paths = ensure_managed_layout(&managed_root).unwrap();
    index_deployment(&paths.database_path, "demo", &runtime, &target_path).unwrap();

    let preview = preview_delete_skill("demo", &managed_root).unwrap();

    assert!(!preview.can_delete);
    assert!(preview.blockers[0].contains("pointing elsewhere"));
    assert!(imported.managed_path.exists());
    assert!(fs::symlink_metadata(target_path)
        .unwrap()
        .file_type()
        .is_symlink());
}

#[test]
fn deletes_complete_remote_skill_root_to_recovery_backup() {
    let root = temp_dir("delete-remote-skill");
    let source = root.join("source").join("demo");
    let managed_root = root.join("SkillBox");
    make_skill(&source, "demo", "Remote demo skill");
    import_skill(&source, SkillKind::Remote, &managed_root).unwrap();
    let paths = ensure_managed_layout(&managed_root).unwrap();
    let remote_root = paths.remote_skills_root.join("demo");
    fs::write(remote_root.join("source.json"), "{}").unwrap();

    let preview = preview_delete_skill("demo", &managed_root).unwrap();
    let result = delete_skill(
        DeleteSkillRequest {
            skill_name: "demo".to_string(),
            preview_id: preview.preview_id,
            confirmed_skill_name: "demo".to_string(),
            actor: "test".to_string(),
        },
        &managed_root,
    )
    .unwrap();

    assert_eq!(result.kind, SkillKind::Remote);
    assert!(!remote_root.exists());
    assert!(result.backup_path.join("source.json").exists());
    assert!(result.backup_path.join("versions").is_dir());
    assert!(fs::symlink_metadata(result.backup_path.join("current"))
        .unwrap()
        .file_type()
        .is_symlink());
}

#[test]
fn remote_delete_preview_becomes_stale_when_source_metadata_changes() {
    let root = temp_dir("delete-remote-stale-source");
    let source = root.join("source").join("demo");
    let managed_root = root.join("SkillBox");
    make_skill(&source, "demo", "Remote demo skill");
    import_skill(&source, SkillKind::Remote, &managed_root).unwrap();
    let paths = ensure_managed_layout(&managed_root).unwrap();
    let remote_root = paths.remote_skills_root.join("demo");
    fs::write(remote_root.join("source.json"), "{\"ref\":\"main\"}").unwrap();
    let preview = preview_delete_skill("demo", &managed_root).unwrap();
    fs::write(remote_root.join("source.json"), "{\"ref\":\"other\"}").unwrap();

    let error = delete_skill(
        DeleteSkillRequest {
            skill_name: "demo".to_string(),
            preview_id: preview.preview_id,
            confirmed_skill_name: "demo".to_string(),
            actor: "test".to_string(),
        },
        &managed_root,
    )
    .unwrap_err();

    assert!(error.contains("state changed"));
    assert!(remote_root.exists());
}

#[test]
fn deletes_broken_remote_skill_with_missing_current_link() {
    let root = temp_dir("delete-broken-remote");
    let source = root.join("source").join("demo");
    let managed_root = root.join("SkillBox");
    make_skill(&source, "demo", "Remote demo skill");
    import_skill(&source, SkillKind::Remote, &managed_root).unwrap();
    let paths = ensure_managed_layout(&managed_root).unwrap();
    let remote_root = paths.remote_skills_root.join("demo");
    fs::remove_file(remote_root.join("current")).unwrap();

    let preview = preview_delete_skill("demo", &managed_root).unwrap();
    let result = delete_skill(
        DeleteSkillRequest {
            skill_name: "demo".to_string(),
            preview_id: preview.preview_id,
            confirmed_skill_name: "demo".to_string(),
            actor: "test".to_string(),
        },
        &managed_root,
    )
    .unwrap();

    assert!(!remote_root.exists());
    assert!(result.backup_path.join("versions").exists());
    assert!(!result.backup_path.join("current").exists());
}

#[test]
fn deletes_broken_remote_with_legacy_direct_version_deployment() {
    let root = temp_dir("delete-broken-remote-direct-version");
    let source = root.join("source").join("demo");
    let managed_root = root.join("SkillBox");
    let runtime = root.join("runtime");
    make_skill(&source, "demo", "Remote demo skill");
    let imported = import_skill(&source, SkillKind::Remote, &managed_root).unwrap();
    let deployment = deploy_skill("demo", &managed_root, &runtime).unwrap();
    fs::remove_file(&deployment.target_path).unwrap();
    symlink_dir(&imported.managed_path, &deployment.target_path).unwrap();
    let paths = ensure_managed_layout(&managed_root).unwrap();
    let remote_root = paths.remote_skills_root.join("demo");
    fs::remove_file(remote_root.join("current")).unwrap();

    let preview = preview_delete_skill("demo", &managed_root).unwrap();
    assert!(
        preview.can_delete,
        "unexpected blockers: {:?}",
        preview.blockers
    );
    let result = delete_skill(
        DeleteSkillRequest {
            skill_name: "demo".to_string(),
            preview_id: preview.preview_id,
            confirmed_skill_name: "demo".to_string(),
            actor: "test".to_string(),
        },
        &managed_root,
    )
    .unwrap();

    assert!(!remote_root.exists());
    assert!(fs::symlink_metadata(deployment.target_path).is_err());
    assert!(result.backup_path.join("versions").exists());
}

#[test]
fn delete_skill_removes_only_its_remote_update_cache_status() {
    let root = temp_dir("delete-remote-cache-status");
    let source = root.join("source").join("demo");
    let managed_root = root.join("SkillBox");
    make_skill(&source, "demo", "Demo skill");
    import_skill(&source, SkillKind::User, &managed_root).unwrap();
    let paths = ensure_managed_layout(&managed_root).unwrap();
    write_remote_update_cache(
        &paths.database_path,
        &RemoteSkillUpdateCheck {
            checked_at: Some("2026-07-12T00:00:00Z".to_string()),
            statuses: vec![
                no_source_remote_update_status("demo"),
                no_source_remote_update_status("other"),
            ],
        },
    )
    .unwrap();
    let preview = preview_delete_skill("demo", &managed_root).unwrap();

    delete_skill(
        DeleteSkillRequest {
            skill_name: "demo".to_string(),
            preview_id: preview.preview_id,
            confirmed_skill_name: "demo".to_string(),
            actor: "test".to_string(),
        },
        &managed_root,
    )
    .unwrap();

    let cache = read_remote_update_cache(&paths.database_path)
        .unwrap()
        .unwrap();
    assert_eq!(cache.statuses.len(), 1);
    assert_eq!(cache.statuses[0].skill_name, "other");
}

#[test]
fn corrupted_remote_update_cache_does_not_block_skill_deletion() {
    let root = temp_dir("delete-corrupted-remote-cache");
    let source = root.join("source").join("demo");
    let managed_root = root.join("SkillBox");
    make_skill(&source, "demo", "Demo skill");
    import_skill(&source, SkillKind::User, &managed_root).unwrap();
    let paths = ensure_managed_layout(&managed_root).unwrap();
    let connection = open_database(&paths.database_path).unwrap();
    connection
        .execute(
            "INSERT INTO preferences (key, value) VALUES ('remote_skill_update_cache', '{broken')",
            [],
        )
        .unwrap();
    let preview = preview_delete_skill("demo", &managed_root).unwrap();

    delete_skill(
        DeleteSkillRequest {
            skill_name: "demo".to_string(),
            preview_id: preview.preview_id,
            confirmed_skill_name: "demo".to_string(),
            actor: "test".to_string(),
        },
        &managed_root,
    )
    .unwrap();

    let cached: Option<String> = connection
        .query_row(
            "SELECT value FROM preferences WHERE key = 'remote_skill_update_cache'",
            [],
            |row| row.get(0),
        )
        .optional()
        .unwrap();
    assert!(cached.is_none());
}

#[test]
fn database_cleanup_failure_restores_managed_skill_and_deployments() {
    let root = temp_dir("delete-db-rollback");
    let source = root.join("source").join("demo");
    let managed_root = root.join("SkillBox");
    let first_runtime = root.join("runtime-one");
    let second_runtime = root.join("runtime-two");
    make_skill(&source, "demo", "Demo skill");
    let imported = import_skill(&source, SkillKind::User, &managed_root).unwrap();
    let first = deploy_skill("demo", &managed_root, &first_runtime).unwrap();
    let second = deploy_skill("demo", &managed_root, &second_runtime).unwrap();
    let paths = ensure_managed_layout(&managed_root).unwrap();
    let connection = open_database(&paths.database_path).unwrap();
    connection
        .execute_batch(
            "CREATE TRIGGER fail_delete_deployment
             BEFORE DELETE ON deployments
             WHEN OLD.skill_name = 'demo'
             BEGIN
               SELECT RAISE(ABORT, 'forced deployment cleanup failure');
             END;",
        )
        .unwrap();
    let preview = preview_delete_skill("demo", &managed_root).unwrap();

    let error = delete_skill(
        DeleteSkillRequest {
            skill_name: "demo".to_string(),
            preview_id: preview.preview_id,
            confirmed_skill_name: "demo".to_string(),
            actor: "test".to_string(),
        },
        &managed_root,
    )
    .unwrap_err();

    assert!(error.contains("forced deployment cleanup failure"));
    assert!(imported.managed_path.exists());
    for target in [first.target_path, second.target_path] {
        assert!(fs::symlink_metadata(target)
            .unwrap()
            .file_type()
            .is_symlink());
    }
    assert_eq!(
        load_deployments(&paths.database_path)
            .unwrap()
            .get("demo")
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn midflight_deployment_removal_failure_restores_prior_symlinks() {
    let root = temp_dir("delete-midflight-rollback");
    let managed = root.join("managed").join("demo");
    let first_target = root.join("runtime-one").join("demo");
    let second_target = root.join("runtime-two").join("demo");
    fs::create_dir_all(&managed).unwrap();
    fs::create_dir_all(first_target.parent().unwrap()).unwrap();
    fs::create_dir_all(second_target.parent().unwrap()).unwrap();
    symlink_dir(&managed, &first_target).unwrap();
    symlink_dir(&managed, &second_target).unwrap();
    let deployments = vec![
        ManagedSkillDeployment {
            target_root: first_target.parent().unwrap().to_path_buf(),
            target_path: first_target.clone(),
            mode: "symlink".to_string(),
        },
        ManagedSkillDeployment {
            target_root: second_target.parent().unwrap().to_path_buf(),
            target_path: second_target.clone(),
            mode: "symlink".to_string(),
        },
    ];
    let mut call_count = 0;

    let error = remove_skill_deployment_symlinks_with(
        &deployments,
        std::slice::from_ref(&managed),
        &root.join("backups/deletion-conflicts"),
        &managed,
        |target, references, conflict_root| {
            call_count += 1;
            if call_count == 2 {
                Err("forced second deployment failure".to_string())
            } else {
                remove_owned_skill_symlink(target, references, conflict_root)
            }
        },
    )
    .unwrap_err();

    assert!(error.contains("forced second deployment failure"));
    for target in [first_target, second_target] {
        assert!(fs::symlink_metadata(target)
            .unwrap()
            .file_type()
            .is_symlink());
    }
}

#[test]
fn delete_skill_rejects_preview_after_managed_content_changes() {
    let root = temp_dir("delete-stale-preview");
    let source = root.join("source").join("demo");
    let managed_root = root.join("SkillBox");
    make_skill(&source, "demo", "Demo skill");
    let imported = import_skill(&source, SkillKind::User, &managed_root).unwrap();
    let preview = preview_delete_skill("demo", &managed_root).unwrap();
    fs::create_dir_all(imported.managed_path.join("scripts")).unwrap();
    fs::write(imported.managed_path.join("scripts/tool.sh"), "changed\n").unwrap();

    let error = delete_skill(
        DeleteSkillRequest {
            skill_name: "demo".to_string(),
            preview_id: preview.preview_id,
            confirmed_skill_name: "demo".to_string(),
            actor: "test".to_string(),
        },
        &managed_root,
    )
    .unwrap_err();

    assert!(error.contains("state changed"));
    assert!(imported.managed_path.exists());
}

#[cfg(unix)]
#[test]
fn delete_skill_preview_tracks_special_directory_entries() {
    let root = temp_dir("delete-stale-special-entry");
    let source = root.join("source").join("demo");
    let managed_root = root.join("SkillBox");
    make_skill(&source, "demo", "Demo skill");
    let imported = import_skill(&source, SkillKind::User, &managed_root).unwrap();
    let preview = preview_delete_skill("demo", &managed_root).unwrap();
    let fifo_path = imported.managed_path.join("events.pipe");
    let status = std::process::Command::new("mkfifo")
        .arg(&fifo_path)
        .status()
        .unwrap();
    assert!(status.success());

    let error = delete_skill(
        DeleteSkillRequest {
            skill_name: "demo".to_string(),
            preview_id: preview.preview_id,
            confirmed_skill_name: "demo".to_string(),
            actor: "test".to_string(),
        },
        &managed_root,
    )
    .unwrap_err();

    assert!(error.contains("state changed"));
    assert!(fifo_path.exists());
}

#[test]
fn quarantined_deployment_check_preserves_non_symlink_target() {
    let root = temp_dir("delete-quarantine-non-symlink");
    let target = root.join("runtime").join("demo");
    let managed = root.join("managed").join("demo");
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    fs::create_dir_all(&managed).unwrap();
    fs::write(&target, "user content").unwrap();

    let error = remove_owned_skill_symlink(
        &target,
        &[managed],
        &root.join("SkillBox/backups/deletion-conflicts"),
    )
    .unwrap_err();

    assert!(error.contains("state changed"));
    assert_eq!(fs::read_to_string(target).unwrap(), "user content");
}

#[test]
fn managed_state_is_first_use_when_managed_store_has_no_skills() {
    let root = temp_dir("managed-state-empty");
    let state = managed_state(root.join("SkillBox")).unwrap();

    assert!(state.is_first_use);
    assert_eq!(state.skills.len(), 0);
}
