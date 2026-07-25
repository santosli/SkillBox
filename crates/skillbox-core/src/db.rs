use crate::*;
use fs2::FileExt;
use std::fs::{File, OpenOptions};

pub(crate) const LATEST_DATABASE_SCHEMA_VERSION: i64 = 5;

pub(crate) fn open_database(database_path: &Path) -> Result<Connection> {
    let connection = Connection::open(database_path).map_err(|error| error.to_string())?;
    connection
        .execute_batch(
            "
            PRAGMA busy_timeout = 5000;
            PRAGMA journal_mode = WAL;
            ",
        )
        .map_err(|error| error.to_string())?;
    Ok(connection)
}

pub(crate) fn init_database(database_path: &Path) -> Result<()> {
    if let Some(parent) = database_path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let _migration_lock = acquire_database_migration_lock(database_path)?;
    let existing_database = fs::metadata(database_path)
        .map(|metadata| metadata.len() > 0)
        .unwrap_or(false);
    let mut connection = open_database(database_path)?;
    let current_version = current_database_schema_version(&connection)?;
    if existing_database && current_version < LATEST_DATABASE_SCHEMA_VERSION {
        backup_database_before_migration(&connection, database_path, current_version)?;
    }

    run_database_migrations(&mut connection)?;
    validate_database_integrity(&connection)
}

fn acquire_database_migration_lock(database_path: &Path) -> Result<File> {
    let file_name = database_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("skillbox.sqlite");
    let lock_path = database_path.with_file_name(format!("{file_name}.migration.lock"));
    let lock = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(&lock_path)
        .map_err(|error| format!("Unable to open database migration lock: {error}"))?;
    lock.lock_exclusive()
        .map_err(|error| format!("Unable to acquire database migration lock: {error}"))?;
    Ok(lock)
}

pub(crate) fn current_database_schema_version(connection: &Connection) -> Result<i64> {
    let migration_table_exists: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'schema_migrations')",
            [],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    if !migration_table_exists {
        return Ok(0);
    }

    connection
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            [],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())
}

pub(crate) fn run_database_migrations(connection: &mut Connection) -> Result<()> {
    connection
        .execute_batch(
            "
            CREATE TABLE IF NOT EXISTS schema_migrations (
              version INTEGER PRIMARY KEY,
              name TEXT NOT NULL,
              applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            ",
        )
        .map_err(|error| error.to_string())?;

    for (version, name) in [
        (1_i64, "baseline"),
        (2_i64, "legacy_compatibility"),
        (3_i64, "skill_user_metadata"),
        (4_i64, "skill_usage_ranking_indexes"),
        (5_i64, "canonical_usage_agent_ids"),
    ] {
        let applied: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version = ?1)",
                params![version],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        if applied {
            continue;
        }

        let transaction = connection
            .transaction()
            .map_err(|error| error.to_string())?;
        match version {
            1 => apply_baseline_migration(&transaction)?,
            2 => apply_legacy_compatibility_migration(&transaction)?,
            3 => apply_skill_user_metadata_migration(&transaction)?,
            4 => apply_skill_usage_ranking_indexes_migration(&transaction)?,
            5 => apply_canonical_usage_agent_ids_migration(&transaction)?,
            _ => return Err(format!("Unknown database migration version: {version}")),
        }
        transaction
            .execute(
                "INSERT INTO schema_migrations (version, name) VALUES (?1, ?2)",
                params![version, name],
            )
            .map_err(|error| error.to_string())?;
        transaction.commit().map_err(|error| error.to_string())?;
    }

    Ok(())
}

fn apply_baseline_migration(connection: &Connection) -> Result<()> {
    connection
        .execute_batch(
            "
            CREATE TABLE IF NOT EXISTS skills (
              name TEXT PRIMARY KEY,
              type TEXT NOT NULL,
              description TEXT NOT NULL DEFAULT '',
              version TEXT NOT NULL DEFAULT '',
              managed_path TEXT NOT NULL,
              status TEXT NOT NULL DEFAULT 'ok',
              content_hash TEXT NOT NULL DEFAULT '',
              updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS deployments (
              skill_name TEXT NOT NULL,
              target_root TEXT NOT NULL,
              target_path TEXT NOT NULL,
              mode TEXT NOT NULL,
              updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
              PRIMARY KEY (skill_name, target_root)
            );

            CREATE TABLE IF NOT EXISTS preferences (
              key TEXT PRIMARY KEY,
              value TEXT NOT NULL,
              updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS workspaces (
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

            CREATE TABLE IF NOT EXISTS operations (
              id TEXT PRIMARY KEY,
              type TEXT NOT NULL,
              status TEXT NOT NULL,
              actor TEXT NOT NULL,
              entity_type TEXT NOT NULL,
              entity_name TEXT NOT NULL,
              started_at TEXT NOT NULL,
              finished_at TEXT,
              summary TEXT NOT NULL,
              error TEXT,
              payload_json TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS skill_usage_events (
              id TEXT PRIMARY KEY,
              event_id TEXT,
              skill_name TEXT NOT NULL,
              agent_id TEXT NOT NULL,
              runtime_root TEXT NOT NULL,
              used_at TEXT NOT NULL,
              recorded_at TEXT NOT NULL,
              prompt_excerpt TEXT,
              metadata_json TEXT NOT NULL DEFAULT '{}'
            );

            CREATE UNIQUE INDEX IF NOT EXISTS skill_usage_events_event_id_unique
            ON skill_usage_events (agent_id, runtime_root, event_id)
            WHERE event_id IS NOT NULL;

            CREATE TABLE IF NOT EXISTS skill_usage_stats (
              skill_name TEXT NOT NULL,
              agent_id TEXT NOT NULL,
              runtime_root TEXT NOT NULL,
              usage_count INTEGER NOT NULL DEFAULT 0,
              last_used_at TEXT NOT NULL,
              updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
              PRIMARY KEY (skill_name, agent_id, runtime_root)
            );

            CREATE TABLE IF NOT EXISTS import_records (
              id TEXT PRIMARY KEY,
              skill_name TEXT NOT NULL,
              type TEXT NOT NULL,
              source_path TEXT NOT NULL,
              source_root TEXT,
              managed_path TEXT NOT NULL,
              content_hash TEXT NOT NULL,
              backup_path TEXT NOT NULL,
              deployed_path TEXT NOT NULL,
              status TEXT NOT NULL,
              legacy INTEGER NOT NULL DEFAULT 0,
              imported_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
              reverted_at TEXT
            );
            ",
        )
        .map_err(|error| error.to_string())
}

fn apply_legacy_compatibility_migration(connection: &Connection) -> Result<()> {
    ensure_database_column(
        connection,
        "workspaces",
        "imported_skill_count",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    ensure_database_column(connection, "skill_usage_events", "prompt_excerpt", "TEXT")?;
    migrate_legacy_node_operations_table(connection)?;
    Ok(())
}

fn apply_skill_user_metadata_migration(connection: &Connection) -> Result<()> {
    connection
        .execute_batch(
            "
            CREATE TABLE IF NOT EXISTS skill_user_metadata (
              skill_name TEXT PRIMARY KEY,
              favorite INTEGER NOT NULL DEFAULT 0,
              tags_json TEXT NOT NULL DEFAULT '[]',
              updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            ",
        )
        .map_err(|error| error.to_string())
}

fn apply_skill_usage_ranking_indexes_migration(connection: &Connection) -> Result<()> {
    connection
        .execute_batch(
            "
            CREATE INDEX IF NOT EXISTS skill_usage_events_rank_time
            ON skill_usage_events (used_at, skill_name);

            CREATE INDEX IF NOT EXISTS skill_usage_events_rank_agent_time
            ON skill_usage_events (agent_id, used_at, skill_name);

            CREATE INDEX IF NOT EXISTS skill_usage_events_rank_runtime_time
            ON skill_usage_events (runtime_root, used_at, skill_name);

            CREATE INDEX IF NOT EXISTS skill_usage_events_rank_agent_runtime_time
            ON skill_usage_events (agent_id, runtime_root, used_at, skill_name);
            ",
        )
        .map_err(|error| error.to_string())
}

fn apply_canonical_usage_agent_ids_migration(connection: &Connection) -> Result<()> {
    canonicalize_stored_usage_agent_id(connection, "agents", "codex")?;
    canonicalize_stored_usage_agent_id(connection, "claude", "claude-code")?;
    Ok(())
}

fn canonicalize_stored_usage_agent_id(
    connection: &Connection,
    legacy_agent_id: &str,
    canonical_agent_id: &str,
) -> Result<()> {
    connection
        .execute(
            "
            DELETE FROM skill_usage_events
            WHERE agent_id = ?1
              AND event_id IS NOT NULL
              AND EXISTS (
                SELECT 1
                FROM skill_usage_events AS canonical
                WHERE canonical.agent_id = ?2
                  AND canonical.runtime_root = skill_usage_events.runtime_root
                  AND canonical.event_id = skill_usage_events.event_id
              )
            ",
            params![legacy_agent_id, canonical_agent_id],
        )
        .map_err(|error| error.to_string())?;
    connection
        .execute(
            "
            UPDATE skill_usage_events
            SET agent_id = ?1
            WHERE agent_id = ?2
            ",
            params![canonical_agent_id, legacy_agent_id],
        )
        .map_err(|error| error.to_string())?;

    connection
        .execute(
            "
            DELETE FROM skill_usage_stats
            WHERE agent_id IN (?1, ?2)
            ",
            params![legacy_agent_id, canonical_agent_id],
        )
        .map_err(|error| error.to_string())?;
    connection
        .execute(
            "
            INSERT INTO skill_usage_stats (
              skill_name,
              agent_id,
              runtime_root,
              usage_count,
              last_used_at
            )
            SELECT
              skill_name,
              agent_id,
              runtime_root,
              COUNT(*),
              MAX(used_at)
            FROM skill_usage_events
            WHERE agent_id = ?1
            GROUP BY skill_name, agent_id, runtime_root
            ",
            params![canonical_agent_id],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

pub(crate) fn backup_database_before_migration(
    connection: &Connection,
    database_path: &Path,
    current_version: i64,
) -> Result<PathBuf> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let file_name = database_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("skillbox.sqlite");
    let backup_path = database_path.with_file_name(format!(
        "{file_name}.pre-migration-v{current_version}-{nanos}.bak"
    ));
    connection
        .execute(
            "VACUUM main INTO ?1",
            params![backup_path.to_string_lossy()],
        )
        .map_err(|error| format!("Unable to back up database before migration: {error}"))?;
    Ok(backup_path)
}

pub(crate) fn validate_database_integrity(connection: &Connection) -> Result<()> {
    let integrity: String = connection
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(|error| error.to_string())?;
    if integrity == "ok" {
        Ok(())
    } else {
        Err(format!("SQLite integrity check failed: {integrity}"))
    }
}

pub(crate) fn migrate_legacy_node_operations_table(connection: &Connection) -> Result<()> {
    let columns = table_column_names(connection, "operations")?;
    let has_legacy_columns = columns.iter().any(|column| column == "skill_name")
        && columns.iter().any(|column| column == "message")
        && columns.iter().any(|column| column == "created_at");
    let has_rust_columns = columns.iter().any(|column| column == "actor")
        && columns.iter().any(|column| column == "entity_type")
        && columns.iter().any(|column| column == "payload_json");
    if !has_legacy_columns || has_rust_columns {
        return Ok(());
    }

    connection
        .execute_batch(
            "
            DROP TABLE IF EXISTS operations_new;

            CREATE TABLE operations_new (
              id TEXT PRIMARY KEY,
              type TEXT NOT NULL,
              status TEXT NOT NULL,
              actor TEXT NOT NULL,
              entity_type TEXT NOT NULL,
              entity_name TEXT NOT NULL,
              started_at TEXT NOT NULL,
              finished_at TEXT,
              summary TEXT NOT NULL,
              error TEXT,
              payload_json TEXT NOT NULL
            );

            INSERT INTO operations_new (
              id, type, status, actor, entity_type, entity_name,
              started_at, finished_at, summary, error, payload_json
            )
            SELECT
              'legacy-node-' || id,
              type,
              CASE status
                WHEN 'ok' THEN 'succeeded'
                WHEN 'succeeded' THEN 'succeeded'
                WHEN 'started' THEN 'started'
                WHEN 'failed' THEN 'failed'
                WHEN 'cancelled' THEN 'cancelled'
                ELSE 'failed'
              END,
              'legacy-node',
              CASE
                WHEN skill_name IS NULL OR skill_name = '' THEN 'operation'
                ELSE 'skill'
              END,
              COALESCE(skill_name, ''),
              created_at,
              CASE
                WHEN status IN ('ok', 'succeeded', 'failed', 'cancelled') THEN created_at
                ELSE NULL
              END,
              CASE
                WHEN message = '' THEN type
                ELSE message
              END,
              CASE
                WHEN status IN ('ok', 'succeeded', 'started', 'failed', 'cancelled') THEN NULL
                ELSE status
              END,
              '{\"legacyNode\":true}'
            FROM operations;

            DROP TABLE operations;
            ALTER TABLE operations_new RENAME TO operations;
            ",
        )
        .map_err(|error| error.to_string())
}

pub(crate) fn table_column_names(connection: &Connection, table: &str) -> Result<Vec<String>> {
    let mut statement = connection
        .prepare(&format!("PRAGMA table_info({table})"))
        .map_err(|error| error.to_string())?;
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|error| error.to_string())?;
    let mut names = Vec::new();
    for column in columns {
        names.push(column.map_err(|error| error.to_string())?);
    }
    Ok(names)
}

pub(crate) fn ensure_database_column(
    connection: &Connection,
    table: &str,
    column: &str,
    definition: &str,
) -> Result<()> {
    for existing in table_column_names(connection, table)? {
        if existing == column {
            return Ok(());
        }
    }

    connection
        .execute(
            &format!("ALTER TABLE {table} ADD COLUMN {column} {definition}"),
            [],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

pub(crate) fn operation_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    format!("op-{nanos}")
}

pub(crate) fn operation_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default();
    seconds.to_string()
}

pub(crate) fn file_modified_timestamp(path: &Path) -> String {
    use std::time::UNIX_EPOCH;

    fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_default()
}

pub(crate) fn usage_event_row_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    format!("usage-{nanos}")
}

pub(crate) fn import_record_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    format!("import-{nanos}")
}

pub(crate) fn current_rfc3339_timestamp() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, false)
}

pub(crate) fn read_bool_preference(database_path: &Path, key: &str) -> Result<Option<bool>> {
    let connection = open_database(database_path).map_err(|error| error.to_string())?;
    let value: Option<String> = connection
        .query_row(
            "SELECT value FROM preferences WHERE key = ?1",
            params![key],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;

    match value.as_deref() {
        None => Ok(None),
        Some("true") => Ok(Some(true)),
        Some("false") => Ok(Some(false)),
        Some(other) => Err(format!("Invalid boolean preference {key}: {other}")),
    }
}

pub(crate) fn write_bool_preference(database_path: &Path, key: &str, value: bool) -> Result<()> {
    let connection = open_database(database_path).map_err(|error| error.to_string())?;
    connection
        .execute(
            "
            INSERT INTO preferences (key, value)
            VALUES (?1, ?2)
            ON CONFLICT(key) DO UPDATE SET
              value = excluded.value,
              updated_at = CURRENT_TIMESTAMP
            ",
            params![key, if value { "true" } else { "false" }],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

pub(crate) fn read_u32_preference(database_path: &Path, key: &str) -> Result<Option<u32>> {
    let connection = open_database(database_path).map_err(|error| error.to_string())?;
    let value: Option<String> = connection
        .query_row(
            "SELECT value FROM preferences WHERE key = ?1",
            params![key],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;

    value
        .map(|raw| {
            raw.parse::<u32>()
                .map_err(|error| format!("Invalid numeric preference {key}: {error}"))
        })
        .transpose()
}

pub(crate) fn write_u32_preference(database_path: &Path, key: &str, value: u32) -> Result<()> {
    let connection = open_database(database_path).map_err(|error| error.to_string())?;
    connection
        .execute(
            "
            INSERT INTO preferences (key, value)
            VALUES (?1, ?2)
            ON CONFLICT(key) DO UPDATE SET
              value = excluded.value,
              updated_at = CURRENT_TIMESTAMP
            ",
            params![key, value.to_string()],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

pub(crate) fn read_remote_update_cache(
    database_path: &Path,
) -> Result<Option<RemoteSkillUpdateCheck>> {
    let connection = open_database(database_path).map_err(|error| error.to_string())?;
    let value: Option<String> = connection
        .query_row(
            "SELECT value FROM preferences WHERE key = ?1",
            params!["remote_skill_update_cache"],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;

    value
        .map(|raw| serde_json::from_str(&raw).map_err(|error| error.to_string()))
        .transpose()
}

pub(crate) fn write_remote_update_cache(
    database_path: &Path,
    result: &RemoteSkillUpdateCheck,
) -> Result<()> {
    let connection = open_database(database_path).map_err(|error| error.to_string())?;
    let value = serde_json::to_string(result).map_err(|error| error.to_string())?;
    connection
        .execute(
            "
            INSERT INTO preferences (key, value)
            VALUES (?1, ?2)
            ON CONFLICT(key) DO UPDATE SET
              value = excluded.value,
              updated_at = CURRENT_TIMESTAMP
            ",
            params!["remote_skill_update_cache", value],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

pub(crate) fn read_app_update_cache(database_path: &Path) -> Result<Option<AppUpdateCheckCache>> {
    let connection = open_database(database_path).map_err(|error| error.to_string())?;
    let value: Option<String> = connection
        .query_row(
            "SELECT value FROM preferences WHERE key = ?1",
            params!["app_update_check_cache"],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;

    value
        .map(|raw| serde_json::from_str(&raw).map_err(|error| error.to_string()))
        .transpose()
}

pub(crate) fn write_app_update_cache(
    database_path: &Path,
    result: &AppUpdateCheckCache,
) -> Result<()> {
    let connection = open_database(database_path).map_err(|error| error.to_string())?;
    let value = serde_json::to_string(result).map_err(|error| error.to_string())?;
    connection
        .execute(
            "
            INSERT INTO preferences (key, value)
            VALUES (?1, ?2)
            ON CONFLICT(key) DO UPDATE SET
              value = excluded.value,
              updated_at = CURRENT_TIMESTAMP
            ",
            params!["app_update_check_cache", value],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

pub(crate) fn index_skill(
    database_path: &Path,
    skill: &Skill,
    kind: SkillKind,
    managed_path: &Path,
) -> Result<()> {
    let connection = open_database(database_path).map_err(|error| error.to_string())?;
    connection
        .execute(
            "
            INSERT INTO skills (
              name, type, description, version, managed_path, status, content_hash, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, 'ok', ?6, CURRENT_TIMESTAMP)
            ON CONFLICT(name) DO UPDATE SET
              type = excluded.type,
              description = excluded.description,
              version = excluded.version,
              managed_path = excluded.managed_path,
              content_hash = excluded.content_hash,
              updated_at = CURRENT_TIMESTAMP
            ",
            params![
                skill.name,
                kind.as_str(),
                skill.description,
                skill.version,
                managed_path.to_string_lossy(),
                skill.content_hash
            ],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

pub(crate) fn index_deployment(
    database_path: &Path,
    skill_name: &str,
    target_root: &Path,
    target_path: &Path,
) -> Result<()> {
    let connection = open_database(database_path).map_err(|error| error.to_string())?;
    connection
        .execute(
            "
            INSERT INTO deployments (skill_name, target_root, target_path, mode, updated_at)
            VALUES (?1, ?2, ?3, 'symlink', CURRENT_TIMESTAMP)
            ON CONFLICT(skill_name, target_root) DO UPDATE SET
              target_path = excluded.target_path,
              mode = excluded.mode,
              updated_at = CURRENT_TIMESTAMP
            ",
            params![
                skill_name,
                target_root.to_string_lossy(),
                target_path.to_string_lossy()
            ],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

pub(crate) fn remove_deployment(
    database_path: &Path,
    skill_name: &str,
    target_root: &Path,
) -> Result<()> {
    let connection = open_database(database_path).map_err(|error| error.to_string())?;
    connection
        .execute(
            "DELETE FROM deployments WHERE skill_name = ?1 AND target_root = ?2",
            params![skill_name, target_root.to_string_lossy()],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

pub(crate) fn load_deployments(
    database_path: &Path,
) -> Result<HashMap<String, Vec<ManagedSkillDeployment>>> {
    let connection = open_database(database_path).map_err(|error| error.to_string())?;
    let mut statement = connection
        .prepare(
            "
            SELECT skill_name, target_root, target_path, mode
            FROM deployments
            ORDER BY skill_name, target_root
            ",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                ManagedSkillDeployment {
                    target_root: PathBuf::from(row.get::<_, String>(1)?),
                    target_path: PathBuf::from(row.get::<_, String>(2)?),
                    mode: row.get::<_, String>(3)?,
                },
            ))
        })
        .map_err(|error| error.to_string())?;
    let mut deployments: HashMap<String, Vec<ManagedSkillDeployment>> = HashMap::new();

    for row in rows {
        let (skill_name, deployment) = row.map_err(|error| error.to_string())?;
        deployments.entry(skill_name).or_default().push(deployment);
    }

    Ok(deployments)
}

pub(crate) fn insert_import_record(database_path: &Path, record: &ImportRecord) -> Result<()> {
    let connection = open_database(database_path).map_err(|error| error.to_string())?;
    connection
        .execute(
            "
            INSERT INTO import_records (
              id, skill_name, type, source_path, source_root, managed_path,
              content_hash, backup_path, deployed_path, status, legacy,
              imported_at, reverted_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
            ",
            params![
                record.id,
                record.skill_name,
                record.kind.as_str(),
                record.source_path.to_string_lossy(),
                record
                    .source_root
                    .as_ref()
                    .map(|path| path.to_string_lossy().to_string()),
                record.managed_path.to_string_lossy(),
                record.content_hash,
                record.backup_path.to_string_lossy(),
                record.deployed_path.to_string_lossy(),
                record.status.as_str(),
                if record.legacy { 1 } else { 0 },
                record.imported_at,
                record.reverted_at
            ],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

pub(crate) fn load_import_records(
    database_path: &Path,
    filter: &ImportRecordFilter,
) -> Result<Vec<ImportRecord>> {
    let connection = open_database(database_path).map_err(|error| error.to_string())?;
    let mut statement = connection
        .prepare(
            "
            SELECT id, skill_name, type, source_path, source_root, managed_path,
                   content_hash, backup_path, deployed_path, status, legacy,
                   imported_at, reverted_at
            FROM import_records
            WHERE (?1 IS NULL OR skill_name = ?1)
            ORDER BY imported_at DESC, id DESC
            ",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map(
            params![filter.skill_name.as_deref()],
            import_record_from_row,
        )
        .map_err(|error| error.to_string())?;
    let mut records = Vec::new();

    for row in rows {
        records.push(row.map_err(|error| error.to_string())?);
    }

    Ok(records)
}

pub(crate) fn load_import_record(database_path: &Path, id: &str) -> Result<ImportRecord> {
    let connection = open_database(database_path).map_err(|error| error.to_string())?;
    connection
        .query_row(
            "
            SELECT id, skill_name, type, source_path, source_root, managed_path,
                   content_hash, backup_path, deployed_path, status, legacy,
                   imported_at, reverted_at
            FROM import_records
            WHERE id = ?1
            ",
            params![id],
            import_record_from_row,
        )
        .map_err(|error| error.to_string())
}

pub(crate) fn mark_import_record_reverted(database_path: &Path, id: &str) -> Result<()> {
    let connection = open_database(database_path).map_err(|error| error.to_string())?;
    connection
        .execute(
            "
            UPDATE import_records
            SET status = ?2,
                reverted_at = CURRENT_TIMESTAMP
            WHERE id = ?1
            ",
            params![id, ImportRecordStatus::Reverted.as_str()],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

pub(crate) fn remove_skill_index(database_path: &Path, skill_name: &str) -> Result<()> {
    let connection = open_database(database_path).map_err(|error| error.to_string())?;
    connection
        .execute("DELETE FROM skills WHERE name = ?1", params![skill_name])
        .map_err(|error| error.to_string())?;
    Ok(())
}

pub(crate) fn remove_deleted_skill_active_records(
    database_path: &Path,
    skill_name: &str,
) -> Result<()> {
    let mut connection = open_database(database_path).map_err(|error| error.to_string())?;
    let transaction = connection
        .transaction()
        .map_err(|error| error.to_string())?;
    transaction
        .execute(
            "DELETE FROM deployments WHERE skill_name = ?1",
            params![skill_name],
        )
        .map_err(|error| error.to_string())?;
    transaction
        .execute("DELETE FROM skills WHERE name = ?1", params![skill_name])
        .map_err(|error| error.to_string())?;
    transaction
        .execute(
            "DELETE FROM skill_user_metadata WHERE skill_name = ?1",
            params![skill_name],
        )
        .map_err(|error| error.to_string())?;

    let cached: Option<String> = transaction
        .query_row(
            "SELECT value FROM preferences WHERE key = ?1",
            params!["remote_skill_update_cache"],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    if let Some(raw) = cached {
        match serde_json::from_str::<RemoteSkillUpdateCheck>(&raw) {
            Ok(mut cache) => {
                cache
                    .statuses
                    .retain(|status| status.skill_name != skill_name);
                transaction
                    .execute(
                        "UPDATE preferences SET value = ?2, updated_at = CURRENT_TIMESTAMP WHERE key = ?1",
                        params![
                            "remote_skill_update_cache",
                            serde_json::to_string(&cache).map_err(|error| error.to_string())?
                        ],
                    )
                    .map_err(|error| error.to_string())?;
            }
            Err(_) => {
                transaction
                    .execute(
                        "DELETE FROM preferences WHERE key = ?1",
                        params!["remote_skill_update_cache"],
                    )
                    .map_err(|error| error.to_string())?;
            }
        }
    }

    transaction.commit().map_err(|error| error.to_string())
}

pub(crate) fn import_record_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ImportRecord> {
    let kind_raw: String = row.get(2)?;
    let status_raw: String = row.get(9)?;
    let source_root: Option<String> = row.get(4)?;
    let legacy_raw: i64 = row.get(10)?;

    Ok(ImportRecord {
        id: row.get(0)?,
        skill_name: row.get(1)?,
        kind: parse_skill_kind(&kind_raw).unwrap_or(SkillKind::User),
        source_path: PathBuf::from(row.get::<_, String>(3)?),
        source_root: source_root.map(PathBuf::from),
        managed_path: PathBuf::from(row.get::<_, String>(5)?),
        content_hash: row.get(6)?,
        backup_path: PathBuf::from(row.get::<_, String>(7)?),
        deployed_path: PathBuf::from(row.get::<_, String>(8)?),
        status: parse_import_record_status(&status_raw).unwrap_or(ImportRecordStatus::Failed),
        legacy: legacy_raw != 0,
        imported_at: row.get(11)?,
        reverted_at: row.get(12)?,
        can_revert: false,
        revert_block_reason: None,
        affected_deployment_count: 0,
    })
}

pub(crate) fn parse_skill_kind(value: &str) -> Option<SkillKind> {
    match value {
        "user" => Some(SkillKind::User),
        "remote" => Some(SkillKind::Remote),
        _ => None,
    }
}

pub(crate) fn parse_import_record_status(value: &str) -> Option<ImportRecordStatus> {
    match value {
        "active" => Some(ImportRecordStatus::Active),
        "reverted" => Some(ImportRecordStatus::Reverted),
        "failed" => Some(ImportRecordStatus::Failed),
        _ => None,
    }
}
