use super::*;
use rusqlite::OptionalExtension;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::sync::{Arc, Barrier};
use std::time::{SystemTime, UNIX_EPOCH};

fn record_test_call(
    mut request: RecordSkillUsageRequest,
    managed_root: impl AsRef<Path>,
) -> Result<SkillUsageRecordResult> {
    let source = request
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.get("source"))
        .and_then(|value| value.as_str());
    let source = source.map(str::to_string);
    if !matches!(
        source.as_deref(),
        Some(
            "agent_hook"
                | "codex_session_backfill"
                | "claude_code_session_backfill"
                | "cursor_agent_transcript_read"
        )
    ) {
        let metadata = request
            .metadata
            .get_or_insert_with(|| serde_json::json!({}));
        if let Some(object) = metadata.as_object_mut() {
            object.insert(
                "source".to_string(),
                serde_json::Value::String("agent_hook".to_string()),
            );
        }
    }
    if source.as_deref() == Some("claude_code_session_backfill") {
        if let Some(object) = request
            .metadata
            .as_mut()
            .and_then(|value| value.as_object_mut())
        {
            object.insert(
                "evidence_signal".to_string(),
                serde_json::Value::String("native_skill_tool".to_string()),
            );
        }
    }
    record_trusted_generated_skill_usage(request, managed_root)
}

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
fn structured_frontmatter_preserves_unknown_optional_fields() {
    let document = parse_skill_frontmatter_document(
        "---
name: demo
description: Demo
allowed-tools:
  - Bash
metadata:
  owner: example
---
",
    )
    .expect("frontmatter should parse");

    assert_eq!(document.metadata.name, "demo");
    assert_eq!(
        document.unknown_fields,
        vec!["allowed-tools".to_string(), "metadata".to_string()]
    );
    assert!(document.fields.contains_key("allowed-tools"));
    assert!(document.fields.contains_key("metadata"));
}

#[test]
fn structured_frontmatter_rejects_malformed_or_typed_known_fields() {
    assert!(parse_skill_frontmatter_document(
        "---
name: [demo
---
"
    )
    .unwrap_err()
    .starts_with("Invalid SKILL.md frontmatter:"));
    assert_eq!(
        parse_skill_frontmatter_document(
            "---
name:
  - demo
---
"
        )
        .unwrap_err(),
        "SKILL.md frontmatter field 'name' must be a string."
    );
}

#[test]
fn database_initialization_configures_busy_timeout_and_wal() {
    let source = include_str!("db.rs");

    assert!(source.contains("PRAGMA busy_timeout = 5000"));
    assert!(source.contains("PRAGMA journal_mode = WAL"));
}

#[test]
fn database_initialization_records_ordered_schema_migrations() {
    let root = temp_dir("database-schema-migrations");
    let paths = ensure_managed_layout(root.join("SkillBox")).unwrap();
    let connection = rusqlite::Connection::open(&paths.database_path).unwrap();
    let versions = connection
        .prepare("SELECT version, name FROM schema_migrations ORDER BY version")
        .unwrap()
        .query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })
        .unwrap()
        .collect::<std::result::Result<Vec<_>, _>>()
        .unwrap();

    assert_eq!(
        versions,
        vec![
            (1, "baseline".to_string()),
            (2, "legacy_compatibility".to_string()),
            (3, "skill_user_metadata".to_string()),
            (4, "skill_usage_ranking_indexes".to_string()),
            (5, "canonical_usage_agent_ids".to_string()),
            (6, "runtime_profiles".to_string()),
            (7, "usage_evidence_classification".to_string())
        ]
    );
    assert_eq!(
        current_database_schema_version(&connection).unwrap(),
        LATEST_DATABASE_SCHEMA_VERSION
    );
    assert!(table_column_names(&connection, "skill_user_metadata")
        .unwrap()
        .contains(&"tags_json".to_string()));
    for column in ["profile_id", "root_key", "format"] {
        assert!(table_column_names(&connection, "workspaces")
            .unwrap()
            .contains(&column.to_string()));
    }
    for index in [
        "skill_usage_events_rank_time",
        "skill_usage_events_rank_agent_time",
        "skill_usage_events_rank_runtime_time",
        "skill_usage_events_rank_agent_runtime_time",
    ] {
        let exists: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'index' AND name = ?1)",
                [index],
                |row| row.get(0),
            )
            .unwrap();
        assert!(exists, "missing ranking index {index}");
    }
}

#[test]
fn usage_evidence_migration_preserves_events_and_rebuilds_call_stats() {
    let root = temp_dir("usage-evidence-v7-migration");
    let managed_root = root.join("SkillBox");
    let paths = ensure_managed_layout(&managed_root).unwrap();
    let connection = rusqlite::Connection::open(&paths.database_path).unwrap();
    connection
        .execute_batch(
            "
            DELETE FROM skill_usage_stats;
            DELETE FROM skill_usage_events;
            DELETE FROM schema_migrations WHERE version = 7;
            DROP INDEX IF EXISTS skill_usage_events_evidence_time;
            DROP INDEX IF EXISTS skill_usage_events_evidence_agent_time;
            DROP INDEX IF EXISTS skill_usage_events_evidence_runtime_time;
            ALTER TABLE skill_usage_events DROP COLUMN evidence_sources_json;
            ALTER TABLE skill_usage_events DROP COLUMN evidence_class;
            INSERT INTO skill_usage_events (
              id, event_id, skill_name, agent_id, runtime_root,
              used_at, recorded_at, metadata_json
            ) VALUES
              ('hook', 'hook', 'demo', 'codex', '/tmp/runtime',
               '2026-07-01T00:00:00+00:00', '2026-07-01T00:00:01+00:00',
               '{\"source\":\"agent_hook\"}'),
              ('codex', 'codex', 'demo', 'codex', '/tmp/runtime',
               '2026-07-02T00:00:00+00:00', '2026-07-02T00:00:01+00:00',
               '{\"source\":\"codex_session_backfill\"}'),
              ('claude', 'claude', 'demo', 'claude-code', '/tmp/runtime',
               '2026-07-03T00:00:00+00:00', '2026-07-03T00:00:01+00:00',
               '{\"source\":\"claude_code_session_backfill\"}'),
              ('cursor', 'cursor', 'demo', 'cursor', '/tmp/runtime',
               '2026-07-04T00:00:00+00:00', '2026-07-04T00:00:01+00:00',
               '{\"source\":\"cursor_session_backfill\"}'),
              ('cursor-transcript', 'cursor-transcript', 'demo', 'cursor', '/tmp/runtime',
               '2026-07-04T01:00:00+00:00', '2026-07-04T01:00:01+00:00',
               '{\"source\":\"cursor_agent_transcript_read\"}'),
              ('manual', 'manual', 'demo', 'codex', '/tmp/runtime',
               '2026-07-05T00:00:00+00:00', '2026-07-05T00:00:01+00:00',
               '{\"source\":\"/Users/alice/private-client\"}');
            INSERT INTO skill_usage_stats (
              skill_name, agent_id, runtime_root, usage_count, last_used_at
            ) VALUES ('demo', 'codex', '/tmp/runtime', 5, '2026-07-05T00:00:00+00:00');
            ",
        )
        .unwrap();
    drop(connection);

    ensure_managed_layout(&managed_root).unwrap();
    let connection = rusqlite::Connection::open(&paths.database_path).unwrap();
    let classes = connection
        .prepare("SELECT id, evidence_class FROM skill_usage_events ORDER BY id")
        .unwrap()
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .unwrap()
        .collect::<std::result::Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(
        classes,
        vec![
            ("claude".to_string(), "inferred".to_string()),
            ("codex".to_string(), "inferred".to_string()),
            ("cursor".to_string(), "reference".to_string()),
            ("cursor-transcript".to_string(), "inferred".to_string()),
            ("hook".to_string(), "confirmed".to_string()),
            ("manual".to_string(), "reference".to_string()),
        ]
    );
    let raw_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM skill_usage_events", [], |row| {
            row.get(0)
        })
        .unwrap();
    let call_count: i64 = connection
        .query_row(
            "SELECT SUM(usage_count) FROM skill_usage_stats",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(raw_count, 6);
    assert_eq!(call_count, 4);
    let evidence_sources_json: String = connection
        .query_row(
            "SELECT evidence_sources_json FROM skill_usage_events WHERE id = 'codex'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&evidence_sources_json).unwrap(),
        serde_json::json!([{
            "source": "codex_session_backfill",
            "evidence_class": "inferred"
        }])
    );
    drop(connection);

    let replayed_claude = record_trusted_generated_skill_usage(
        RecordSkillUsageRequest {
            skill_name: "demo".to_string(),
            agent_id: "claude-code".to_string(),
            runtime_root: PathBuf::from("/tmp/runtime"),
            event_id: Some("claude".to_string()),
            used_at: Some("2026-07-03T00:00:00+00:00".to_string()),
            prompt_excerpt: None,
            metadata: Some(serde_json::json!({
                "source": "claude_code_session_backfill",
                "evidence_signal": "native_skill_tool"
            })),
        },
        &managed_root,
    )
    .unwrap();
    assert!(replayed_claude.deduplicated);
    assert!(replayed_claude.upgraded);
    assert_eq!(
        replayed_claude.evidence_class,
        SkillUsageEvidenceClass::Confirmed
    );
    assert_eq!(replayed_claude.usage_count, 1);

    ensure_managed_layout(&managed_root).unwrap();
    let connection = rusqlite::Connection::open(&paths.database_path).unwrap();
    let call_count: i64 = connection
        .query_row(
            "SELECT SUM(usage_count) FROM skill_usage_stats",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(call_count, 4);
    let (raw_count, claude_class): (i64, String) = (
        connection
            .query_row("SELECT COUNT(*) FROM skill_usage_events", [], |row| {
                row.get(0)
            })
            .unwrap(),
        connection
            .query_row(
                "SELECT evidence_class FROM skill_usage_events WHERE id = 'claude'",
                [],
                |row| row.get(0),
            )
            .unwrap(),
    );
    assert_eq!(raw_count, 6);
    assert_eq!(claude_class, "confirmed");
    drop(connection);
    let audit_json = serde_json::to_string(&usage_audit(&managed_root).unwrap()).unwrap();
    assert!(!audit_json.contains("/Users/alice/private-client"));
    assert!(audit_json.contains("\"source\":\"manual\""));
}

#[test]
fn usage_evidence_repair_recovers_pre_release_v7_rows_with_missing_provenance() {
    let root = temp_dir("usage-evidence-v7-repair");
    let managed_root = root.join("SkillBox");
    let paths = ensure_managed_layout(&managed_root).unwrap();
    let connection = rusqlite::Connection::open(&paths.database_path).unwrap();
    connection
        .execute_batch(
            "
            DELETE FROM skill_usage_stats;
            DELETE FROM skill_usage_events;
            INSERT INTO skill_usage_events (
              id, event_id, skill_name, agent_id, runtime_root,
              used_at, recorded_at, metadata_json, evidence_class, evidence_sources_json
            ) VALUES
              ('hook', 'hook', 'demo', 'codex', '/tmp/runtime',
               '2026-07-01T00:00:00+00:00', '2026-07-01T00:00:01+00:00',
               '{\"source\":\"agent_hook\"}', 'reference', '[\"agent_hook\"]'),
              ('codex', 'codex', 'demo', 'codex', '/tmp/runtime',
               '2026-07-02T00:00:00+00:00', '2026-07-02T00:00:01+00:00',
               '{\"source\":\"codex_session_backfill\"}', 'reference', '[\"codex_session_backfill\"]'),
              ('cursor', 'cursor', 'demo', 'cursor', '/tmp/runtime',
               '2026-07-03T00:00:00+00:00', '2026-07-03T00:00:01+00:00',
               '{\"source\":\"cursor_session_backfill\"}', 'reference', '[]'),
              ('cursor-transcript', 'cursor-transcript', 'demo', 'cursor', '/tmp/runtime',
               '2026-07-04T00:00:00+00:00', '2026-07-04T00:00:01+00:00',
               '{\"source\":\"cursor_agent_transcript_read\"}', 'confirmed',
               '[{\"source\":\"cursor_agent_transcript_read\",\"evidence_class\":\"confirmed\"}]');
            INSERT INTO skill_usage_stats (
              skill_name, agent_id, runtime_root, usage_count, last_used_at
            ) VALUES ('demo', 'codex', '/tmp/runtime', 1, '2026-07-01T00:00:00+00:00');
            ",
        )
        .unwrap();
    drop(connection);

    ensure_managed_layout(&managed_root).unwrap();
    let audit = usage_audit(&managed_root).unwrap();
    assert_eq!(audit.total_calls, 3);
    assert_eq!(audit.confirmed_calls, 1);
    assert_eq!(audit.inferred_calls, 2);
    assert_eq!(audit.history_references, 1);
    let connection = rusqlite::Connection::open(&paths.database_path).unwrap();
    let call_count: i64 = connection
        .query_row(
            "SELECT SUM(usage_count) FROM skill_usage_stats",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(call_count, 3);
    let cursor_transcript_class: String = connection
        .query_row(
            "SELECT evidence_class FROM skill_usage_events WHERE id = 'cursor-transcript'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(cursor_transcript_class, "inferred");
    let missing_provenance: i64 = connection
        .query_row(
            "
            SELECT COUNT(*)
            FROM skill_usage_events
            WHERE evidence_sources_json = '[]'
               OR evidence_sources_json LIKE '[\"%'
            ",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(missing_provenance, 0);

    drop(connection);
    ensure_managed_layout(&managed_root).unwrap();
    let audit = usage_audit(&managed_root).unwrap();
    assert_eq!(audit.total_calls, 3);
    assert_eq!(audit.history_references, 1);
}

#[test]
fn runtime_profile_migration_backfills_canonical_and_custom_workspace_roots() {
    let root = temp_dir("runtime-profile-backfill");
    let managed_root = root.join("SkillBox");
    fs::create_dir_all(&managed_root).unwrap();
    let database_path = managed_root.join("skillbox.sqlite");
    let connection = rusqlite::Connection::open(&database_path).unwrap();
    connection
        .execute_batch(
            "
            CREATE TABLE schema_migrations (
              version INTEGER PRIMARY KEY,
              name TEXT NOT NULL,
              applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            INSERT INTO schema_migrations (version, name) VALUES
              (1, 'baseline'),
              (2, 'legacy_compatibility'),
              (3, 'skill_user_metadata'),
              (4, 'skill_usage_ranking_indexes'),
              (5, 'canonical_usage_agent_ids');
            CREATE TABLE workspaces (
              canonical_path TEXT PRIMARY KEY,
              path TEXT NOT NULL,
              kind TEXT NOT NULL,
              source TEXT NOT NULL,
              agent_id TEXT,
              display_name TEXT NOT NULL,
              skill_count INTEGER NOT NULL DEFAULT 0,
              imported_skill_count INTEGER NOT NULL DEFAULT 0,
              last_scan_error_count INTEGER NOT NULL DEFAULT 0,
              last_scan_error TEXT,
              last_scanned_at TEXT,
              created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
              updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            INSERT INTO workspaces (
              canonical_path, path, kind, source, agent_id, display_name
            ) VALUES
              ('/tmp/project/.agents/skills', '/tmp/project/.agents/skills', 'user', 'manual', 'agents', 'Agents'),
              ('/tmp/project/.codex/skills', '/tmp/project/.codex/skills', 'user', 'manual', 'codex', 'Codex'),
              ('/tmp/project/.claude/skills', '/tmp/project/.claude/skills', 'user', 'manual', 'claude', 'Claude'),
              ('/tmp/project/.cursor/skills', '/tmp/project/.cursor/skills', 'user', 'manual', 'cursor', 'Cursor'),
              ('/tmp/shared/skills', '/tmp/project/.agents/skills', 'user', 'manual', 'agents', 'Agents alias'),
              ('/tmp/custom-skills', '/tmp/custom-skills', 'user', 'manual', NULL, 'Custom');
            ",
        )
        .unwrap();
    drop(connection);

    let mut connection = rusqlite::Connection::open(&database_path).unwrap();
    run_database_migrations(&mut connection).unwrap();
    let rows = connection
        .prepare(
            "SELECT path, profile_id, root_key, format FROM workspaces ORDER BY path, canonical_path",
        )
        .unwrap()
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })
        .unwrap()
        .collect::<std::result::Result<Vec<_>, _>>()
        .unwrap();

    assert_eq!(
        rows,
        vec![
            (
                "/tmp/custom-skills".to_string(),
                "custom-skill-md".to_string(),
                "exact".to_string(),
                "skill_md".to_string(),
            ),
            (
                "/tmp/project/.agents/skills".to_string(),
                "agents".to_string(),
                "skills".to_string(),
                "skill_md".to_string(),
            ),
            (
                "/tmp/project/.agents/skills".to_string(),
                "custom-skill-md".to_string(),
                "exact".to_string(),
                "skill_md".to_string(),
            ),
            (
                "/tmp/project/.claude/skills".to_string(),
                "claude-code".to_string(),
                "skills".to_string(),
                "skill_md".to_string(),
            ),
            (
                "/tmp/project/.codex/skills".to_string(),
                "codex".to_string(),
                "skills".to_string(),
                "skill_md".to_string(),
            ),
            (
                "/tmp/project/.cursor/skills".to_string(),
                "cursor".to_string(),
                "skills".to_string(),
                "skill_md".to_string(),
            ),
        ]
    );
}

#[test]
fn v5_symlink_alias_migrates_as_custom_and_remains_deployable() {
    let root = temp_dir("runtime-profile-v5-symlink-alias");
    let managed_root = root.join("SkillBox");
    let source = root.join("source/demo");
    let actual_root = root.join("shared/skills");
    let linked_root = root.join("project/.agents/skills");
    make_skill(&source, "demo", "Demo skill");
    fs::create_dir_all(&actual_root).unwrap();
    fs::create_dir_all(linked_root.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(&actual_root, &linked_root).unwrap();
    import_skill(&source, SkillKind::User, &managed_root).unwrap();

    let database_path = managed_paths(&managed_root).database_path;
    let connection = rusqlite::Connection::open(&database_path).unwrap();
    connection
        .execute_batch(
            "
            DELETE FROM workspaces;
            DELETE FROM schema_migrations WHERE version = 6;
            ALTER TABLE workspaces DROP COLUMN profile_id;
            ALTER TABLE workspaces DROP COLUMN root_key;
            ALTER TABLE workspaces DROP COLUMN format;
            ",
        )
        .unwrap();
    connection
        .execute(
            "
            INSERT INTO workspaces (
              canonical_path, path, kind, source, agent_id, display_name
            ) VALUES (?1, ?2, 'user', 'manual', 'agents', 'Agents alias')
            ",
            rusqlite::params![
                fs::canonicalize(&actual_root)
                    .unwrap()
                    .to_string_lossy()
                    .to_string(),
                linked_root.to_string_lossy().to_string()
            ],
        )
        .unwrap();
    drop(connection);

    let mut connection = rusqlite::Connection::open(&database_path).unwrap();
    run_database_migrations(&mut connection).unwrap();
    let (profile_id, root_key): (String, String) = connection
        .query_row(
            "SELECT profile_id, root_key FROM workspaces WHERE canonical_path = ?1",
            [fs::canonicalize(&actual_root)
                .unwrap()
                .to_string_lossy()
                .to_string()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(profile_id, "custom-skill-md");
    assert_eq!(root_key, "exact");
    drop(connection);

    let preview = preview_skill_deployment(
        DeploymentCompatibilityPreviewRequest {
            skill_name: "demo".to_string(),
            target_root: linked_root.clone(),
        },
        &managed_root,
    )
    .unwrap();
    assert_eq!(preview.profile.id, "custom-skill-md");
    assert_eq!(preview.root_key, "exact");
    assert_eq!(preview.status, CompatibilityStatus::Compatible);

    let deployment = apply_skill_deployment(
        DeploymentCompatibilityApplyRequest {
            skill_name: "demo".to_string(),
            target_root: linked_root,
            preview_id: preview.preview_id,
            confirm_warnings: false,
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
fn existing_database_is_backed_up_once_before_migration() {
    let root = temp_dir("database-migration-backup");
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
              updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            INSERT INTO skills (name, type, managed_path) VALUES ('demo', 'user', '/tmp/demo');
            ",
        )
        .unwrap();
    drop(connection);

    ensure_managed_layout(&managed_root).unwrap();
    let backups = database_migration_backups(&paths.database_path);
    assert_eq!(backups.len(), 1);
    let backup = rusqlite::Connection::open(&backups[0]).unwrap();
    let name: String = backup
        .query_row("SELECT name FROM skills", [], |row| row.get(0))
        .unwrap();
    assert_eq!(name, "demo");
    drop(backup);

    ensure_managed_layout(&managed_root).unwrap();
    assert_eq!(database_migration_backups(&paths.database_path), backups);
}

#[test]
fn schema_v4_ranking_index_migration_preserves_usage_events() {
    let root = temp_dir("database-ranking-index-migration");
    let managed_root = root.join("SkillBox");
    let paths = ensure_managed_layout(&managed_root).unwrap();
    let connection = rusqlite::Connection::open(&paths.database_path).unwrap();
    connection
        .execute_batch(
            "
            DELETE FROM schema_migrations WHERE version >= 4;
            DROP INDEX IF EXISTS skill_usage_events_rank_time;
            DROP INDEX IF EXISTS skill_usage_events_rank_agent_time;
            DROP INDEX IF EXISTS skill_usage_events_rank_runtime_time;
            DROP INDEX IF EXISTS skill_usage_events_rank_agent_runtime_time;
            INSERT INTO skill_usage_events (
              id, skill_name, agent_id, runtime_root, used_at, recorded_at, metadata_json
            ) VALUES (
              'usage-before-v4', 'demo', 'codex', '/tmp/runtime',
              '2026-06-01T00:00:00+00:00', '2026-06-01T00:00:01+00:00', '{}'
            );
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
            "SELECT COUNT(*) FROM skill_usage_events WHERE id = 'usage-before-v4'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(event_count, 1);
    assert_eq!(database_migration_backups(&paths.database_path).len(), 1);
}

#[test]
fn concurrent_database_initialization_serializes_backup_and_migrations() {
    let root = temp_dir("database-concurrent-migrations");
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
              updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            INSERT INTO skills (name, type, managed_path) VALUES ('demo', 'user', '/tmp/demo');
            ",
        )
        .unwrap();
    drop(connection);

    let worker_count = 24;
    let barrier = Arc::new(Barrier::new(worker_count));
    let workers = (0..worker_count)
        .map(|_| {
            let barrier = Arc::clone(&barrier);
            let managed_root = managed_root.clone();
            std::thread::spawn(move || {
                barrier.wait();
                ensure_managed_layout(managed_root)
            })
        })
        .collect::<Vec<_>>();
    let results = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .collect::<Vec<_>>();

    assert!(
        results.iter().all(Result::is_ok),
        "concurrent initialization errors: {:?}",
        results
            .iter()
            .filter_map(|result| result.as_ref().err())
            .collect::<Vec<_>>()
    );
    assert_eq!(database_migration_backups(&paths.database_path).len(), 1);
    let connection = rusqlite::Connection::open(&paths.database_path).unwrap();
    assert_eq!(
        current_database_schema_version(&connection).unwrap(),
        LATEST_DATABASE_SCHEMA_VERSION
    );
}

#[test]
fn skill_user_metadata_persists_favorites_and_normalized_tags() {
    let root = temp_dir("skill-user-metadata");
    let managed_root = root.join("SkillBox");

    let metadata = set_skill_user_metadata(
        SkillUserMetadataUpdate {
            skill_name: "demo".to_string(),
            favorite: true,
            tags: vec![
                " Research Notes ".to_string(),
                "research-notes".to_string(),
                "Rust!".to_string(),
            ],
        },
        &managed_root,
    )
    .unwrap();

    assert!(metadata.favorite);
    assert_eq!(metadata.tags, vec!["research-notes", "rust"]);
    assert_eq!(
        list_skill_user_metadata(&managed_root).unwrap(),
        vec![metadata]
    );
}

#[test]
fn legacy_skill_user_metadata_does_not_overwrite_database_values() {
    let root = temp_dir("legacy-skill-user-metadata");
    let managed_root = root.join("SkillBox");
    set_skill_user_metadata(
        SkillUserMetadataUpdate {
            skill_name: "demo".to_string(),
            favorite: false,
            tags: vec!["database".to_string()],
        },
        &managed_root,
    )
    .unwrap();

    let metadata = migrate_legacy_skill_user_metadata(
        vec![SkillUserMetadataUpdate {
            skill_name: "demo".to_string(),
            favorite: true,
            tags: vec!["local-storage".to_string()],
        }],
        &managed_root,
    )
    .unwrap();

    assert_eq!(
        metadata,
        vec![SkillUserMetadata {
            skill_name: "demo".to_string(),
            favorite: false,
            tags: vec!["database".to_string()]
        }]
    );
}

#[test]
fn doctor_reports_healthy_fresh_managed_store() {
    let root = temp_dir("doctor-healthy");
    let managed_root = root.join("SkillBox");

    let report = run_doctor(DoctorRequest::default(), &managed_root).unwrap();

    assert!(report.healthy);
    assert_eq!(report.schema_version, LATEST_DATABASE_SCHEMA_VERSION);
    assert!(report.issues.is_empty());
}

#[test]
fn doctor_detects_missing_deployment_and_previews_repair() {
    let root = temp_dir("doctor-missing-deployment");
    let managed_root = root.join("SkillBox");
    let source = root.join("source").join("demo");
    make_skill(&source, "demo", "Demo skill");
    import_skill(&source, SkillKind::User, &managed_root).unwrap();
    let deployment = deploy_skill("demo", &managed_root, root.join("runtime")).unwrap();
    fs::remove_file(&deployment.target_path).unwrap();

    let report = run_doctor(
        DoctorRequest {
            repair_preview: true,
        },
        &managed_root,
    )
    .unwrap();
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.code == "deployment_target_missing")
        .unwrap();

    assert!(!report.healthy);
    assert_eq!(issue.severity, DoctorIssueSeverity::Warning);
    assert!(issue.repairable);
    assert!(issue.suggested_action.is_some());
}

#[test]
fn doctor_detects_remote_skill_without_current_version() {
    let root = temp_dir("doctor-remote-current");
    let managed_root = root.join("SkillBox");
    let paths = ensure_managed_layout(&managed_root).unwrap();
    make_skill(
        &paths
            .remote_skills_root
            .join("demo")
            .join("versions")
            .join("manual-demo"),
        "demo",
        "Demo skill",
    );

    let report = run_doctor(DoctorRequest::default(), &managed_root).unwrap();

    assert!(report
        .issues
        .iter()
        .any(|issue| issue.code == "remote_current_missing"));
    assert_eq!(report.error_count, 1);
}

#[test]
fn doctor_reports_preserved_deletion_quarantine_for_manual_review() {
    let root = temp_dir("doctor-deletion-quarantine");
    let managed_root = root.join("SkillBox");
    let runtime = root.join("runtime");
    fs::create_dir_all(&runtime).unwrap();
    add_workspace(
        WorkspaceAddRequest {
            path: runtime.clone(),
            kind: WorkspaceKind::User,
        },
        &managed_root,
    )
    .unwrap();
    let preserved = runtime.join(".demo.delete-check-123.tmp");
    fs::write(&preserved, "unexpected user content").unwrap();

    let report = run_doctor(DoctorRequest::default(), &managed_root).unwrap();
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.code == "deletion_quarantine_preserved")
        .unwrap();

    assert_eq!(issue.path.as_deref(), Some(preserved.as_path()));
    assert_eq!(issue.severity, DoctorIssueSeverity::Error);
    assert!(!issue.repairable);
}

#[test]
fn doctor_accepts_deployment_through_managed_root_alias() {
    let root = temp_dir("doctor-managed-root-alias");
    let managed_root = root.join("SkillBox");
    let managed_alias = root.join(".skillbox");
    let source = root.join("source").join("demo");
    let runtime = root.join("runtime");
    make_skill(&source, "demo", "Demo skill");
    import_skill(&source, SkillKind::Remote, &managed_root).unwrap();
    deploy_skill("demo", &managed_root, &runtime).unwrap();
    symlink_dir(&managed_root, &managed_alias).unwrap();

    let report = run_doctor(DoctorRequest::default(), &managed_alias).unwrap();

    assert!(report.healthy, "unexpected issues: {:?}", report.issues);
}

#[test]
fn doctor_rejects_deployment_that_bypasses_remote_current() {
    let root = temp_dir("doctor-remote-version-deployment");
    let managed_root = root.join("SkillBox");
    let source = root.join("source").join("demo");
    let runtime = root.join("runtime");
    make_skill(&source, "demo", "Demo skill");
    let imported = import_skill(&source, SkillKind::Remote, &managed_root).unwrap();
    let deployment = deploy_skill("demo", &managed_root, &runtime).unwrap();
    let version_path = fs::canonicalize(&imported.managed_path).unwrap();
    fs::remove_file(&deployment.target_path).unwrap();
    symlink_dir(&version_path, &deployment.target_path).unwrap();

    let report = run_doctor(DoctorRequest::default(), &managed_root).unwrap();

    assert!(report
        .issues
        .iter()
        .any(|issue| issue.code == "deployment_target_mismatch"));
}

#[test]
fn doctor_distinguishes_stale_deployment_record_from_existing_runtime_target() {
    let root = temp_dir("doctor-stale-deployment-record");
    let managed_root = root.join("SkillBox");
    let paths = ensure_managed_layout(&managed_root).unwrap();
    let missing_runtime = root.join("missing-runtime");
    let existing_runtime = root.join("existing-runtime");
    let existing_target = existing_runtime.join("existing");
    fs::create_dir_all(&existing_target).unwrap();
    index_deployment(
        &paths.database_path,
        "stale",
        &missing_runtime,
        &missing_runtime.join("stale"),
    )
    .unwrap();
    index_deployment(
        &paths.database_path,
        "existing",
        &existing_runtime,
        &existing_target,
    )
    .unwrap();

    let report = run_doctor(DoctorRequest::default(), &managed_root).unwrap();
    let stale = report
        .issues
        .iter()
        .find(|issue| issue.entity_name.as_deref() == Some("stale"))
        .unwrap();
    let existing = report
        .issues
        .iter()
        .find(|issue| issue.entity_name.as_deref() == Some("existing"))
        .unwrap();

    assert_eq!(stale.code, "deployment_record_stale");
    assert_eq!(stale.severity, DoctorIssueSeverity::Warning);
    assert_eq!(
        stale.path.as_deref(),
        Some(missing_runtime.join("stale").as_path())
    );
    assert!(stale.repairable);
    assert_eq!(existing.code, "deployment_managed_skill_missing");
    assert_eq!(existing.severity, DoctorIssueSeverity::Error);
    assert_eq!(existing.path.as_deref(), Some(existing_target.as_path()));
    assert!(!existing.repairable);

    let repair = repair_stale_deployment_records(&managed_root).unwrap();
    assert_eq!(repair.removed_deployment_records, 1);
    let deployments = load_deployments(&paths.database_path).unwrap();
    assert!(!deployments.contains_key("stale"));
    assert!(deployments.contains_key("existing"));
    assert!(existing_target.is_dir());

    let operations = list_operations(
        OperationFilter {
            entity_name: Some("deployments".to_string()),
            ..OperationFilter::default()
        },
        &managed_root,
    )
    .unwrap();
    assert_eq!(operations.operations.len(), 1);
    assert_eq!(
        operations.operations[0].operation_type,
        "repair_stale_deployments"
    );
    assert_eq!(operations.operations[0].status, OperationStatus::Succeeded);
}

#[test]
fn major_managed_store_mutations_are_audited() {
    let root = temp_dir("major-mutation-audit");
    let managed_root = root.join("SkillBox");
    let source = root.join("source").join("demo");
    let runtime = root.join("runtime");
    let workspace = root.join("workspace").join(".agents").join("skills");
    make_skill(&source, "demo", "Demo skill");
    fs::create_dir_all(&workspace).unwrap();

    import_skill(&source, SkillKind::User, &managed_root).unwrap();
    deploy_skill("demo", &managed_root, &runtime).unwrap();
    undeploy_skill("demo", &managed_root, &runtime).unwrap();
    change_skill_kind("demo", SkillKind::Remote, &managed_root).unwrap();
    add_workspace(
        WorkspaceAddRequest {
            path: workspace.clone(),
            kind: WorkspaceKind::User,
        },
        &managed_root,
    )
    .unwrap();
    forget_workspace(&workspace, &managed_root).unwrap();

    let operations = list_operations(
        OperationFilter {
            limit: Some(100),
            ..OperationFilter::default()
        },
        &managed_root,
    )
    .unwrap();
    let types = operations
        .operations
        .iter()
        .map(|operation| operation.operation_type.as_str())
        .collect::<HashSet<_>>();

    for expected in [
        "import_skill",
        "deploy_skill",
        "undeploy_skill",
        "change_skill_kind",
        "add_workspace",
        "forget_workspace",
    ] {
        assert!(types.contains(expected), "missing {expected} audit record");
    }
    assert!(operations
        .operations
        .iter()
        .all(|operation| operation.status == OperationStatus::Succeeded));
}

#[test]
fn failed_managed_store_mutation_is_audited() {
    let root = temp_dir("failed-mutation-audit");
    let managed_root = root.join("SkillBox");
    let source = root.join("source").join("demo");
    let runtime = root.join("runtime");
    make_skill(&source, "demo", "Demo skill");
    import_skill(&source, SkillKind::User, &managed_root).unwrap();
    make_skill(&runtime.join("demo"), "demo", "Existing unmanaged skill");

    assert!(deploy_skill("demo", &managed_root, &runtime).is_err());
    let operations = list_operations(
        OperationFilter {
            entity_name: Some("demo".to_string()),
            ..OperationFilter::default()
        },
        &managed_root,
    )
    .unwrap();
    let failed = operations
        .operations
        .iter()
        .find(|operation| operation.operation_type == "deploy_skill")
        .unwrap();

    assert_eq!(failed.status, OperationStatus::Failed);
    assert!(failed
        .error
        .as_deref()
        .unwrap()
        .contains("Refusing to overwrite"));
}

#[test]
fn usage_hook_install_is_audited_without_exposing_config_contents() {
    let root = temp_dir("usage-hook-audit");
    let home = root.join("home");
    let managed_root = home.join(".skillbox");

    install_usage_hook_for_home_with_audit(UsageHookTarget::ClaudeCodeCli, &home, &managed_root)
        .unwrap();
    let operations = list_operations(
        OperationFilter {
            entity_type: Some("agent_config".to_string()),
            ..OperationFilter::default()
        },
        &managed_root,
    )
    .unwrap();

    assert_eq!(operations.operations.len(), 1);
    assert_eq!(
        operations.operations[0].operation_type,
        "install_usage_hook"
    );
    assert_eq!(operations.operations[0].status, OperationStatus::Succeeded);
    assert!(operations.operations[0].payload.get("configPath").is_some());
    assert!(operations.operations[0].payload.get("config").is_none());
}

fn database_migration_backups(database_path: &Path) -> Vec<PathBuf> {
    let prefix = format!(
        "{}.pre-migration-",
        database_path.file_name().unwrap().to_string_lossy()
    );
    let mut backups = fs::read_dir(database_path.parent().unwrap())
        .unwrap()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(&prefix) && name.ends_with(".bak"))
        })
        .collect::<Vec<_>>();
    backups.sort();
    backups
}

#[test]
fn sha256_outputs_lowercase_hex_digest() {
    assert_eq!(
        sha256("abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
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
fn workspace_setup_existing_skills_root_previews_and_applies_without_creating_paths() {
    let root = temp_dir("workspace-setup-existing");
    let managed_root = root.join("SkillBox");
    let workspace_root = root.join("project").join(".agents").join("skills");
    make_skill(&workspace_root.join("alpha"), "alpha", "Alpha skill");

    let preview = preview_workspace_setup(
        WorkspaceSetupPreviewRequest {
            selected_path: workspace_root.clone(),
            kind: WorkspaceKind::User,
        },
        &managed_root,
    )
    .unwrap();

    assert_eq!(preview.mode, WorkspaceSetupMode::ExistingRoot);
    assert_eq!(preview.roots.len(), 1);
    assert!(preview.roots[0].exists);
    assert!(!managed_root.exists());

    let result = apply_workspace_setup(
        WorkspaceSetupApplyRequest {
            selected_path: workspace_root.clone(),
            kind: WorkspaceKind::User,
            selected_root: preview.roots[0].path.clone(),
            create_missing: false,
            preview_id: preview.preview_id,
        },
        &managed_root,
    )
    .unwrap();

    assert_eq!(
        result.workspace.path,
        fs::canonicalize(workspace_root).unwrap()
    );
    assert!(result.created_path.is_none());
}

#[test]
fn workspace_setup_preserves_exact_symlinked_skills_root_registration() {
    let root = temp_dir("workspace-setup-existing-symlink");
    let managed_root = root.join("SkillBox");
    let actual_root = root.join("shared").join("skills");
    let linked_root = root.join("project").join(".agents").join("skills");
    fs::create_dir_all(&actual_root).unwrap();
    fs::create_dir_all(linked_root.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(&actual_root, &linked_root).unwrap();

    let preview = preview_workspace_setup(
        WorkspaceSetupPreviewRequest {
            selected_path: linked_root.clone(),
            kind: WorkspaceKind::User,
        },
        &managed_root,
    )
    .unwrap();

    assert_eq!(preview.mode, WorkspaceSetupMode::ExistingRoot);
    assert_eq!(
        preview.roots[0].path,
        fs::canonicalize(actual_root).unwrap()
    );

    let result = apply_workspace_setup(
        WorkspaceSetupApplyRequest {
            selected_path: linked_root,
            kind: WorkspaceKind::User,
            selected_root: preview.roots[0].path.clone(),
            create_missing: false,
            preview_id: preview.preview_id,
        },
        &managed_root,
    )
    .unwrap();

    assert_eq!(result.workspace.profile_id, "custom-skill-md");
    assert_eq!(result.workspace.root_key, "exact");
}

#[test]
fn workspace_setup_project_preview_discovers_one_and_multiple_roots() {
    let root = temp_dir("workspace-setup-discover");
    let project = root.join("project");
    fs::create_dir_all(project.join(".codex/skills")).unwrap();

    let one = preview_workspace_setup(
        WorkspaceSetupPreviewRequest {
            selected_path: project.clone(),
            kind: WorkspaceKind::User,
        },
        root.join("SkillBox"),
    )
    .unwrap();
    assert_eq!(one.mode, WorkspaceSetupMode::ProjectWithRoots);
    assert_eq!(one.roots.iter().filter(|root| root.exists).count(), 1);
    assert_eq!(
        one.roots
            .iter()
            .find(|root| root.exists)
            .unwrap()
            .relative_path,
        ".codex/skills"
    );

    fs::create_dir_all(project.join(".claude/skills")).unwrap();
    let multiple = preview_workspace_setup(
        WorkspaceSetupPreviewRequest {
            selected_path: project,
            kind: WorkspaceKind::User,
        },
        root.join("SkillBox"),
    )
    .unwrap();
    assert_eq!(multiple.roots.iter().filter(|root| root.exists).count(), 2);
}

#[test]
fn workspace_setup_missing_project_preview_is_read_only_and_defaults_to_agents() {
    let root = temp_dir("workspace-setup-missing-preview");
    let project = root.join("project");
    fs::create_dir_all(&project).unwrap();

    let preview = preview_workspace_setup(
        WorkspaceSetupPreviewRequest {
            selected_path: project.clone(),
            kind: WorkspaceKind::User,
        },
        root.join("SkillBox"),
    )
    .unwrap();

    assert_eq!(preview.mode, WorkspaceSetupMode::ProjectWithoutRoots);
    assert_eq!(preview.roots.len(), 4);
    assert!(preview.roots.iter().all(|root| !root.exists));
    assert_eq!(
        preview
            .roots
            .iter()
            .find(|root| root.recommended)
            .unwrap()
            .relative_path,
        ".agents/skills"
    );
    assert!(!project.join(".agents").exists());
    assert!(!project.join(".codex").exists());
    assert!(!project.join(".claude").exists());
    assert!(!project.join(".cursor").exists());
}

#[test]
fn workspace_setup_missing_project_uses_existing_runtime_marker_as_recommendation() {
    let root = temp_dir("workspace-setup-marker");
    let project = root.join("project");
    fs::create_dir_all(project.join(".claude")).unwrap();

    let preview = preview_workspace_setup(
        WorkspaceSetupPreviewRequest {
            selected_path: project,
            kind: WorkspaceKind::User,
        },
        root.join("SkillBox"),
    )
    .unwrap();

    assert_eq!(
        preview
            .roots
            .iter()
            .find(|root| root.recommended)
            .unwrap()
            .relative_path,
        ".claude/skills"
    );
}

#[test]
fn workspace_setup_rejects_home_root_and_managed_store_as_projects() {
    let root = temp_dir("workspace-setup-broad-root");
    let managed_root = root.join("SkillBox");
    fs::create_dir_all(&managed_root).unwrap();

    let managed_error = preview_workspace_setup(
        WorkspaceSetupPreviewRequest {
            selected_path: managed_root.clone(),
            kind: WorkspaceKind::User,
        },
        &managed_root,
    )
    .unwrap_err();
    assert!(managed_error.contains("managed store"));

    let home = home_dir();
    let home_error = preview_workspace_setup(
        WorkspaceSetupPreviewRequest {
            selected_path: home,
            kind: WorkspaceKind::User,
        },
        &managed_root,
    )
    .unwrap_err();
    assert!(home_error.contains("home directory"));
}

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
    let current = managed_root
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
    let user_path = managed_root.join("user-skills").join("json-canvas");
    let current = managed_root
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

    let result = import_candidates(
        vec![
            ImportRequestItem {
                source_path: first_source.clone(),
                skill_type: SkillKind::User,
                deploy_back_to_source: false,
            },
            ImportRequestItem {
                source_path: second_source.clone(),
                skill_type: SkillKind::User,
                deploy_back_to_source: false,
            },
        ],
        &managed_root,
    )
    .unwrap();

    assert!(result.errors.is_empty());
    assert_eq!(result.imported.len(), 2);
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
        managed_root.join("user-skills").join("demo")
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

    let result = import_candidates(
        vec![
            ImportRequestItem {
                source_path: first_source.clone(),
                skill_type: SkillKind::Remote,
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
    .unwrap();

    assert_eq!(result.imported.len(), 1);
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

fn insert_legacy_usage_event(
    database_path: &std::path::Path,
    skill_name: &str,
    agent_id: &str,
    runtime_root: &std::path::Path,
    used_at: &str,
    event_id: &str,
) {
    let connection = open_database(database_path).unwrap();
    let runtime_root =
        fs::canonicalize(runtime_root).unwrap_or_else(|_| runtime_root.to_path_buf());
    let runtime_root_value = runtime_root.to_string_lossy().to_string();
    connection
        .execute(
            "
            INSERT INTO skill_usage_events (
              id,
              event_id,
              skill_name,
              agent_id,
              runtime_root,
              used_at,
              recorded_at,
              prompt_excerpt,
              metadata_json,
              evidence_class,
              evidence_sources_json
            )
            VALUES (
              ?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL,
              '{\"source\":\"agent_hook\"}',
              'confirmed',
              '[{\"source\":\"agent_hook\",\"evidence_class\":\"confirmed\"}]'
            )
            ",
            rusqlite::params![
                format!("legacy-{event_id}"),
                event_id,
                skill_name,
                agent_id,
                runtime_root_value,
                used_at,
                used_at,
            ],
        )
        .unwrap();
    connection
        .execute(
            "
            INSERT INTO skill_usage_stats (
              skill_name, agent_id, runtime_root, usage_count, last_used_at
            )
            VALUES (?1, ?2, ?3, 1, ?4)
            ON CONFLICT(skill_name, agent_id, runtime_root) DO UPDATE SET
              usage_count = skill_usage_stats.usage_count + 1,
              last_used_at = MAX(skill_usage_stats.last_used_at, excluded.last_used_at)
            ",
            rusqlite::params![skill_name, agent_id, runtime_root_value, used_at],
        )
        .unwrap();
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

fn bare_remote_with_root_skill_content(
    label: &str,
    skill_name: &str,
    description: &str,
    body: &str,
) -> (PathBuf, PathBuf) {
    let remote = bare_remote(label);
    let work = temp_dir(&format!("{label}-work"));
    run_git(&work, &["init", "-b", "main"]);
    make_skill_with_body(&work, skill_name, description, body);
    fs::write(work.join("README.md"), format!("# {skill_name}\n")).unwrap();
    fs::write(work.join(".gitignore"), "*.tmp\n").unwrap();
    fs::create_dir_all(work.join("assets")).unwrap();
    fs::write(work.join("assets/prompt.txt"), "prompt\n").unwrap();
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
            "Add root skill",
        ],
    );
    run_git(
        &work,
        &["remote", "add", "origin", remote.to_str().unwrap()],
    );
    run_git(&work, &["push", "-u", "origin", "main"]);
    (remote, work)
}

fn github_source_url(owner: &str, repo: &str, skill_name: &str) -> String {
    format!("https://github.com/{owner}/{repo}/tree/main/skills/{skill_name}")
}

fn github_install_preview(
    source_url: &str,
    target_root: Option<PathBuf>,
    managed_root: &std::path::Path,
) -> GithubRemoteSkillInstallPreview {
    preview_github_remote_skill_install(
        PreviewGithubRemoteSkillInstallRequest {
            source_url: source_url.to_string(),
            target_root,
        },
        managed_root,
    )
    .unwrap()
}

static GIT_CONFIG_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct GitConfigRewriteGuard {
    _lock: std::sync::MutexGuard<'static, ()>,
    previous: Vec<(&'static str, Option<std::ffi::OsString>)>,
}

impl Drop for GitConfigRewriteGuard {
    fn drop(&mut self) {
        let previous_count = self
            .previous
            .iter()
            .find_map(|(key, value)| (*key == "GIT_CONFIG_COUNT").then(|| value.clone()))
            .flatten();
        std::env::remove_var("GIT_CONFIG_COUNT");
        for (key, value) in self
            .previous
            .drain(..)
            .filter(|(key, _)| *key != "GIT_CONFIG_COUNT")
        {
            match value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
        if let Some(value) = previous_count {
            std::env::set_var("GIT_CONFIG_COUNT", value);
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

    std::env::set_var(
        "GIT_CONFIG_KEY_0",
        format!("url.file://{}.insteadOf", remote.display()),
    );
    std::env::set_var(
        "GIT_CONFIG_VALUE_0",
        format!("https://github.com/{owner}/{repo}.git"),
    );
    std::env::set_var("GIT_CONFIG_COUNT", "1");

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
