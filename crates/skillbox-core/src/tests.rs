use super::*;
use crate::test_support::*;
use std::fs;
use std::process::Command;
use std::sync::{Arc, Barrier};

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
            (7, "usage_evidence_classification".to_string()),
            (8, "skill_collections".to_string()),
            (9, "github_skill_collections".to_string()),
            (10, "skill_collection_revisions".to_string())
        ]
    );
    assert_eq!(
        current_database_schema_version(&connection).unwrap(),
        LATEST_DATABASE_SCHEMA_VERSION
    );
    assert!(table_column_names(&connection, "skill_user_metadata")
        .unwrap()
        .contains(&"tags_json".to_string()));
    assert!(table_column_names(&connection, "skill_collections")
        .unwrap()
        .contains(&"canonical_worktree_root".to_string()));
    assert!(table_column_names(&connection, "skill_collection_members")
        .unwrap()
        .contains(&"relative_path".to_string()));
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
fn schema_v9_github_collection_migration_is_idempotent_for_existing_database() {
    let root = temp_dir("skill-collections-v8-migration");
    let managed_root = root.join("SkillBox");
    let paths = ensure_managed_layout(&managed_root).unwrap();
    {
        let connection = rusqlite::Connection::open(&paths.database_path).unwrap();
        connection
            .execute_batch(
                "
                DROP TABLE skill_collection_members;
                DROP TABLE skill_collections;
                DELETE FROM schema_migrations WHERE version >= 8;
                ",
            )
            .unwrap();
    }

    ensure_managed_layout(&managed_root).unwrap();
    ensure_managed_layout(&managed_root).unwrap();
    let connection = rusqlite::Connection::open(&paths.database_path).unwrap();
    assert_eq!(
        current_database_schema_version(&connection).unwrap(),
        LATEST_DATABASE_SCHEMA_VERSION
    );
    assert!(table_column_names(&connection, "skill_collections")
        .unwrap()
        .contains(&"reviewed_head_sha".to_string()));
    for column in ["source_kind", "source_url", "requested_reference"] {
        assert!(
            table_column_names(&connection, "skill_collections")
                .unwrap()
                .contains(&column.to_string()),
            "missing {column}"
        );
    }
    assert!(table_column_names(&connection, "skill_collection_members")
        .unwrap()
        .contains(&"managed_skill_name".to_string()));
}

#[test]
fn schema_v10_collection_revision_migration_is_idempotent_for_existing_database() {
    let root = temp_dir("skill-collections-v10-migration");
    let managed_root = root.join("SkillBox");
    let paths = ensure_managed_layout(&managed_root).unwrap();
    {
        let connection = rusqlite::Connection::open(&paths.database_path).unwrap();
        connection
            .execute_batch(
                "
                DROP TABLE IF EXISTS skill_collection_revision_members;
                DROP TABLE IF EXISTS skill_collection_revisions;
                DELETE FROM schema_migrations WHERE version = 10;
                ",
            )
            .unwrap();
    }

    ensure_managed_layout(&managed_root).unwrap();
    ensure_managed_layout(&managed_root).unwrap();
    let connection = rusqlite::Connection::open(&paths.database_path).unwrap();
    assert_eq!(
        current_database_schema_version(&connection).unwrap(),
        LATEST_DATABASE_SCHEMA_VERSION
    );
    assert!(table_column_names(&connection, "skill_collections")
        .unwrap()
        .contains(&"previous_reviewed_head_sha".to_string()));
    for table in [
        "skill_collection_revisions",
        "skill_collection_revision_members",
    ] {
        let exists: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
                [table],
                |row| row.get(0),
            )
            .unwrap();
        assert!(exists, "missing {table}");
    }
    assert!(
        table_column_names(&connection, "skill_collection_revision_members")
            .unwrap()
            .contains(&"backup_path".to_string())
    );
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
fn explicit_git_setup_writes_default_user_skills_gitignore() {
    let managed_root = temp_dir("managed-layout-gitignore").join("SkillBox");

    let paths = ensure_managed_layout(&managed_root).unwrap();
    ensure_default_user_skills_gitignore(&paths.user_skills_root).unwrap();
    let gitignore = fs::read_to_string(paths.user_skills_root.join(".gitignore")).unwrap();

    assert!(gitignore.contains(".DS_Store"));
    assert!(gitignore.contains("__pycache__/"));
    assert!(gitignore.contains("*.py[cod]"));
    assert!(gitignore.contains("node_modules/"));
    assert!(gitignore.contains(".env"));
    assert!(gitignore.contains("!.env.example"));
}

#[test]
fn managed_layout_never_writes_git_defaults() {
    let managed_root = temp_dir("managed-layout-mutation-lock");
    let paths = managed_paths(&managed_root);
    fs::create_dir_all(&paths.user_skills_root).unwrap();

    ensure_managed_layout(&managed_root).unwrap();
    assert!(!paths.user_skills_root.join(".gitignore").exists());
}

#[test]
fn managed_layout_read_does_not_race_git_defaults_during_mutation() {
    let managed_root = temp_dir("managed-layout-read-barrier");
    let paths = managed_paths(&managed_root);
    fs::create_dir_all(&paths.user_skills_root).unwrap();
    let entered = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));
    let worker_root = managed_root.clone();
    let worker_entered = entered.clone();
    let worker_release = release.clone();
    let worker = std::thread::spawn(move || {
        let _lock = acquire_user_skills_mutation_lock(&worker_root).unwrap();
        worker_entered.wait();
        worker_release.wait();
    });

    entered.wait();
    ensure_managed_layout(&managed_root).unwrap();
    assert!(!paths.user_skills_root.join(".gitignore").exists());
    assert!(!paths.user_skills_root.join(".git/info/exclude").exists());
    release.wait();
    worker.join().unwrap();
}

#[test]
fn ensure_managed_layout_preserves_existing_user_skills_gitignore() {
    let managed_root = temp_dir("managed-layout-preserve-gitignore").join("SkillBox");
    let user_skills_root = managed_root.join("user-skills");
    fs::create_dir_all(&user_skills_root).unwrap();
    fs::write(user_skills_root.join(".gitignore"), "custom-ignore\n").unwrap();

    let paths = ensure_managed_layout(&managed_root).unwrap();
    ensure_default_user_skills_gitignore(&paths.user_skills_root).unwrap();
    let gitignore = fs::read_to_string(paths.user_skills_root.join(".gitignore")).unwrap();

    assert_eq!(gitignore, "custom-ignore\n");
}

#[test]
fn ensure_managed_layout_keeps_existing_user_skills_repo_clean() {
    let managed_root = temp_dir("managed-layout-clean-git-repo").join("SkillBox");
    let paths = ensure_managed_layout(&managed_root).unwrap();
    let repo = paths.user_skills_root;
    ensure_default_user_skills_gitignore(&repo).unwrap();
    skillbox_git::GitService::new().init_main(&repo).unwrap();
    run_git(&repo, &["add", ".gitignore"]);
    run_git(
        &repo,
        &[
            "-c",
            "user.name=SkillBox",
            "-c",
            "user.email=skillbox@example.invalid",
            "commit",
            "-m",
            "Track defaults",
        ],
    );
    run_git(&repo, &["rm", ".gitignore"]);
    run_git(
        &repo,
        &[
            "-c",
            "user.name=SkillBox",
            "-c",
            "user.email=skillbox@example.invalid",
            "commit",
            "-m",
            "Remove tracked defaults",
        ],
    );

    ensure_default_user_skills_gitignore(&repo).unwrap();

    assert!(!repo.join(".gitignore").exists());
    assert!(fs::read_to_string(repo.join(".git/info/exclude"))
        .unwrap()
        .contains("# SkillBox managed defaults"));
    assert!(!skillbox_git::GitService::new().status(&repo).unwrap().dirty);
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
fn mutation_lock_resolves_legacy_only_home_before_creating_lock_file() {
    const CHILD_HOME: &str = "SKILLBOX_LEGACY_LOCK_TEST_HOME";
    if let Some(home) = std::env::var_os(CHILD_HOME) {
        let home = PathBuf::from(home);
        let hidden_root = home.join(".skillbox");
        let legacy_root = home.join("SkillBox");
        let error = check_user_skills_inbound(&hidden_root).unwrap_err();
        assert!(error.contains("not initialized"));
        assert!(fs::symlink_metadata(&hidden_root)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(
            fs::canonicalize(&hidden_root).unwrap(),
            fs::canonicalize(&legacy_root).unwrap()
        );
        assert!(legacy_root.join(".user-skills-mutation.lock").is_file());
        assert!(!home.join(".skillbox.empty-backup-0").exists());
        return;
    }

    let home = temp_dir("legacy-lock-real-home");
    let legacy_root = home.join("SkillBox");
    make_skill(
        &legacy_root.join("user-skills/demo"),
        "demo",
        "Legacy-only demo",
    );
    let output = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "tests::mutation_lock_resolves_legacy_only_home_before_creating_lock_file",
            "--nocapture",
        ])
        .env("HOME", &home)
        .env_remove("SKILLBOX_HOME")
        .env(CHILD_HOME, &home)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(fs::symlink_metadata(home.join(".skillbox"))
        .unwrap()
        .file_type()
        .is_symlink());
    assert!(legacy_root.join(".user-skills-mutation.lock").is_file());
}

#[test]
fn mutation_lock_expands_tilde_before_locking_across_working_directories() {
    const ROLE: &str = "SKILLBOX_TILDE_LOCK_ROLE";
    const READY: &str = "SKILLBOX_TILDE_LOCK_READY";
    const RELEASE: &str = "SKILLBOX_TILDE_LOCK_RELEASE";
    if let Some(role) = std::env::var_os(ROLE) {
        let role = role.to_string_lossy();
        let managed_root = if role.starts_with("env-") {
            default_managed_root()
        } else {
            PathBuf::from("~/.skillbox")
        };
        match acquire_user_skills_mutation_lock(&managed_root) {
            Ok(_lock) if role.ends_with("holder") => {
                fs::write(std::env::var_os(READY).unwrap(), b"ready").unwrap();
                let release = PathBuf::from(std::env::var_os(RELEASE).unwrap());
                while !release.exists() {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
            }
            Err(error) if role == "contender" => {
                assert!(error.contains("Another user-skills mutation"));
            }
            Ok(_) => panic!("contender unexpectedly acquired the tilde mutation lock"),
            Err(error) => panic!("lock holder failed unexpectedly: {error}"),
        }
        return;
    }

    for (holder_role, configured_home) in [
        ("env-tilde-holder", "~/.skillbox"),
        ("env-normalized-holder", "~/alias/../.skillbox"),
        ("cli-holder", "~/.skillbox"),
    ] {
        let home = temp_dir(&format!("tilde-lock-home-{holder_role}"));
        fs::create_dir_all(home.join("alias")).unwrap();
        make_skill(
            &home.join("SkillBox/user-skills/legacy-demo"),
            "legacy-demo",
            "Legacy demo",
        );
        let holder_cwd = temp_dir(&format!("tilde-lock-holder-cwd-{holder_role}"));
        let contender_cwd = temp_dir(&format!("tilde-lock-contender-cwd-{holder_role}"));
        let barrier = temp_dir(&format!("tilde-lock-barrier-{holder_role}"));
        let ready = barrier.join("ready");
        let release = barrier.join("release");
        let executable = std::env::current_exe().unwrap();
        let mut holder = Command::new(&executable)
            .args([
                "--exact",
                "tests::mutation_lock_expands_tilde_before_locking_across_working_directories",
                "--nocapture",
            ])
            .current_dir(&holder_cwd)
            .env("HOME", &home)
            .env("SKILLBOX_HOME", configured_home)
            .env(ROLE, holder_role)
            .env(READY, &ready)
            .env(RELEASE, &release)
            .spawn()
            .unwrap();
        while !ready.exists() {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let contender = Command::new(&executable)
            .args([
                "--exact",
                "tests::mutation_lock_expands_tilde_before_locking_across_working_directories",
                "--nocapture",
            ])
            .current_dir(&contender_cwd)
            .env("HOME", &home)
            .env_remove("SKILLBOX_HOME")
            .env(ROLE, "contender")
            .env(READY, &ready)
            .env(RELEASE, &release)
            .output()
            .unwrap();
        assert!(
            contender.status.success(),
            "{}",
            String::from_utf8_lossy(&contender.stderr)
        );
        fs::write(&release, b"release").unwrap();
        assert!(holder.wait().unwrap().success());
        assert!(fs::symlink_metadata(home.join(".skillbox"))
            .unwrap()
            .file_type()
            .is_symlink());
        assert!(home.join("SkillBox/.user-skills-mutation.lock").is_file());
        assert!(!holder_cwd.join("~/.skillbox").exists());
        assert!(!contender_cwd.join("~/.skillbox").exists());
    }
}

#[test]
fn mutation_lock_canonicalizes_symlink_parent_before_legacy_root_selection() {
    const ROLE: &str = "SKILLBOX_SYMLINK_PARENT_LOCK_ROLE";
    const MANAGED_ROOT: &str = "SKILLBOX_SYMLINK_PARENT_MANAGED_ROOT";
    const READY: &str = "SKILLBOX_SYMLINK_PARENT_LOCK_READY";
    const RELEASE: &str = "SKILLBOX_SYMLINK_PARENT_LOCK_RELEASE";
    if let Some(role) = std::env::var_os(ROLE) {
        let root = PathBuf::from(std::env::var_os(MANAGED_ROOT).unwrap());
        match acquire_user_skills_mutation_lock(&root) {
            Ok(_lock) if role == "holder" => {
                fs::write(std::env::var_os(READY).unwrap(), b"ready").unwrap();
                let release = PathBuf::from(std::env::var_os(RELEASE).unwrap());
                while !release.exists() {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
            }
            Err(error) if role == "contender" => {
                assert!(error.contains("Another user-skills mutation"), "{error}");
            }
            Ok(_) => panic!("contender unexpectedly acquired the legacy mutation lock"),
            Err(error) => panic!("lock holder failed unexpectedly: {error}"),
        }
        return;
    }

    let root = temp_dir("symlink-parent-lock");
    let home = root.join("home");
    let home_alias = root.join("home-link");
    fs::create_dir_all(&home).unwrap();
    std::os::unix::fs::symlink(&home, &home_alias).unwrap();
    let legacy_root = home.join("SkillBox");
    make_skill(
        &legacy_root.join("user-skills/legacy-demo"),
        "legacy-demo",
        "Legacy demo",
    );
    let barrier = temp_dir("symlink-parent-lock-barrier");
    let ready = barrier.join("ready");
    let release = barrier.join("release");
    let holder_cwd = temp_dir("symlink-parent-holder-cwd");
    let contender_cwd = temp_dir("symlink-parent-contender-cwd");
    let executable = std::env::current_exe().unwrap();
    let mut holder = Command::new(&executable)
        .args([
            "--exact",
            "tests::mutation_lock_canonicalizes_symlink_parent_before_legacy_root_selection",
            "--nocapture",
        ])
        .current_dir(&holder_cwd)
        .env("HOME", &home)
        .env("SKILLBOX_HOME", home_alias.join(".skillbox"))
        .env(ROLE, "holder")
        .env(MANAGED_ROOT, home_alias.join(".skillbox"))
        .env(READY, &ready)
        .env(RELEASE, &release)
        .spawn()
        .unwrap();
    while !ready.exists() {
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let contender = Command::new(&executable)
        .args([
            "--exact",
            "tests::mutation_lock_canonicalizes_symlink_parent_before_legacy_root_selection",
            "--nocapture",
        ])
        .current_dir(&contender_cwd)
        .env("HOME", &home)
        .env_remove("SKILLBOX_HOME")
        .env(ROLE, "contender")
        .env(MANAGED_ROOT, &legacy_root)
        .env(READY, &ready)
        .env(RELEASE, &release)
        .output()
        .unwrap();
    assert!(
        contender.status.success(),
        "{}",
        String::from_utf8_lossy(&contender.stderr)
    );
    fs::write(&release, b"release").unwrap();
    assert!(holder.wait().unwrap().success());
    let hidden_root = home.join(".skillbox");
    assert!(fs::symlink_metadata(&hidden_root)
        .unwrap()
        .file_type()
        .is_symlink());
    assert_eq!(
        fs::canonicalize(home_alias.join(".skillbox")).unwrap(),
        fs::canonicalize(&legacy_root).unwrap()
    );
    assert!(legacy_root.join(".user-skills-mutation.lock").is_file());
    assert!(!holder_cwd.join("home-link/.skillbox").exists());
    assert!(!contender_cwd.join("home-link/.skillbox").exists());
}

#[test]
fn mutation_lock_preserves_parent_and_symlink_resolution_before_creation() {
    const ROLE: &str = "SKILLBOX_PARENT_SEGMENT_LOCK_ROLE";
    const ROOT_ARG: &str = "SKILLBOX_PARENT_SEGMENT_LOCK_ROOT";
    const EXPECTED: &str = "SKILLBOX_PARENT_SEGMENT_EXPECTED_ROOT";
    const READY: &str = "SKILLBOX_PARENT_SEGMENT_LOCK_READY";
    const RELEASE: &str = "SKILLBOX_PARENT_SEGMENT_LOCK_RELEASE";
    if let Some(role) = std::env::var_os(ROLE) {
        let root = PathBuf::from(std::env::var_os(ROOT_ARG).unwrap());
        let expected = PathBuf::from(std::env::var_os(EXPECTED).unwrap());
        match acquire_user_skills_mutation_lock(&root) {
            Ok(lock) if role == "holder" => {
                assert_eq!(lock.truth_root(), fs::canonicalize(&expected).unwrap());
                fs::write(std::env::var_os(READY).unwrap(), b"ready").unwrap();
                let release = PathBuf::from(std::env::var_os(RELEASE).unwrap());
                while !release.exists() {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
            }
            Err(error) if role == "contender" => {
                assert!(error.contains("Another user-skills mutation"), "{error}");
            }
            Ok(_) => panic!("contender unexpectedly acquired the mutation lock"),
            Err(error) => panic!("lock holder failed unexpectedly: {error}"),
        }
        return;
    }

    let executable = std::env::current_exe().unwrap();
    for case in ["relative-parent", "symlink-parent"] {
        let root = temp_dir(&format!("managed-root-resolution-{case}"));
        let (holder_cwd, contender_cwd, holder_arg, contender_arg, expected) =
            if case == "relative-parent" {
                let holder_cwd = root.join("holder");
                let contender_cwd = root.join("contender");
                fs::create_dir_all(&holder_cwd).unwrap();
                fs::create_dir_all(&contender_cwd).unwrap();
                (
                    holder_cwd,
                    contender_cwd,
                    PathBuf::from("../SkillBox"),
                    PathBuf::from("../SkillBox"),
                    root.join("SkillBox"),
                )
            } else {
                let real_parent = root.join("real");
                let holder_cwd = root.join("holder");
                let contender_cwd = root.join("contender");
                fs::create_dir_all(real_parent.join("child")).unwrap();
                fs::create_dir_all(&holder_cwd).unwrap();
                fs::create_dir_all(&contender_cwd).unwrap();
                std::os::unix::fs::symlink(real_parent.join("child"), root.join("alias")).unwrap();
                (
                    holder_cwd,
                    contender_cwd,
                    root.join("alias/../SkillBox"),
                    real_parent.join("SkillBox"),
                    real_parent.join("SkillBox"),
                )
            };
        let barrier = temp_dir(&format!("managed-root-resolution-barrier-{case}"));
        let ready = barrier.join("ready");
        let release = barrier.join("release");
        let mut holder = Command::new(&executable)
            .args([
                "--exact",
                "tests::mutation_lock_preserves_parent_and_symlink_resolution_before_creation",
                "--nocapture",
            ])
            .current_dir(&holder_cwd)
            .env(ROLE, "holder")
            .env(ROOT_ARG, &holder_arg)
            .env(EXPECTED, &expected)
            .env(READY, &ready)
            .env(RELEASE, &release)
            .spawn()
            .unwrap();
        while !ready.exists() {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let contender = Command::new(&executable)
            .args([
                "--exact",
                "tests::mutation_lock_preserves_parent_and_symlink_resolution_before_creation",
                "--nocapture",
            ])
            .current_dir(&contender_cwd)
            .env(ROLE, "contender")
            .env(ROOT_ARG, &contender_arg)
            .env(EXPECTED, &expected)
            .env(READY, &ready)
            .env(RELEASE, &release)
            .output()
            .unwrap();
        assert!(
            contender.status.success(),
            "{}",
            String::from_utf8_lossy(&contender.stderr)
        );
        fs::write(&release, b"release").unwrap();
        assert!(holder.wait().unwrap().success());
        assert!(expected.join(".user-skills-mutation.lock").is_file());
        assert!(!holder_cwd.join("SkillBox").exists());
        assert!(!contender_cwd.join("SkillBox").exists());
    }
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
