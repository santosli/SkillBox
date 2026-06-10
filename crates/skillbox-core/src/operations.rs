use crate::*;

pub fn start_operation(
    request: OperationStart,
    managed_root: impl AsRef<Path>,
) -> Result<OperationRecord> {
    let paths = ensure_managed_layout(managed_root.as_ref().to_path_buf())?;
    let id = operation_id();
    let started_at = operation_timestamp();
    let payload_json =
        serde_json::to_string(&request.payload).map_err(|error| error.to_string())?;
    let connection = open_database(&paths.database_path).map_err(|error| error.to_string())?;

    connection
        .execute(
            "
            INSERT INTO operations (
              id, type, status, actor, entity_type, entity_name,
              started_at, finished_at, summary, error, payload_json
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, ?8, NULL, ?9)
            ",
            params![
                id,
                request.operation_type,
                OperationStatus::Started.as_str(),
                request.actor,
                request.entity_type,
                request.entity_name,
                started_at,
                request.summary,
                payload_json
            ],
        )
        .map_err(|error| error.to_string())?;

    load_operation(&connection, &id)
}

pub fn finish_operation(
    request: OperationFinish,
    managed_root: impl AsRef<Path>,
) -> Result<OperationRecord> {
    let paths = ensure_managed_layout(managed_root.as_ref().to_path_buf())?;
    let id = request.id.clone();
    let finished_at = operation_timestamp();
    let payload_json =
        serde_json::to_string(&request.payload).map_err(|error| error.to_string())?;
    let connection = open_database(&paths.database_path).map_err(|error| error.to_string())?;

    connection
        .execute(
            "
            UPDATE operations
            SET status = ?2,
                finished_at = ?3,
                summary = ?4,
                error = ?5,
                payload_json = ?6
            WHERE id = ?1
            ",
            params![
                id,
                request.status.as_str(),
                finished_at,
                request.summary,
                request.error,
                payload_json
            ],
        )
        .map_err(|error| error.to_string())?;

    load_operation(&connection, &id)
}

pub fn list_operations(
    filter: OperationFilter,
    managed_root: impl AsRef<Path>,
) -> Result<OperationList> {
    let paths = ensure_managed_layout(managed_root.as_ref().to_path_buf())?;
    let connection = open_database(&paths.database_path).map_err(|error| error.to_string())?;
    let limit = i64::from(filter.limit.unwrap_or(50).clamp(1, 500));
    let status = filter.status.map(OperationStatus::as_str);
    let mut statement = connection
        .prepare(
            "
            SELECT id, type, status, actor, entity_type, entity_name,
                   started_at, finished_at, summary, error, payload_json
            FROM operations
            WHERE (?1 IS NULL OR entity_type = ?1)
              AND (?2 IS NULL OR entity_name = ?2)
              AND (?3 IS NULL OR status = ?3)
            ORDER BY started_at DESC, id DESC
            LIMIT ?4
            ",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map(
            params![filter.entity_type, filter.entity_name, status, limit],
            operation_from_row,
        )
        .map_err(|error| error.to_string())?;
    let mut operations = Vec::new();

    for row in rows {
        operations.push(row.map_err(|error| error.to_string())?);
    }

    Ok(OperationList { operations })
}

pub fn list_history(filter: HistoryFilter, managed_root: impl AsRef<Path>) -> Result<HistoryList> {
    let paths = ensure_managed_layout(managed_root.as_ref().to_path_buf())?;
    let connection = open_database(&paths.database_path).map_err(|error| error.to_string())?;
    let limit = usize::try_from(filter.limit.unwrap_or(100).clamp(1, 500)).unwrap_or(100);
    let skill_usage_count = history_table_count(&connection, "skill_usage_events")?;
    let operation_count = history_table_count(&connection, "operations")?;
    let mut entries = Vec::new();

    if filter.kind.is_none() || filter.kind == Some(HistoryEntryKind::SkillUsage) {
        entries.extend(load_skill_usage_history_entries(&connection)?);
    }
    if filter.kind.is_none() || filter.kind == Some(HistoryEntryKind::Operation) {
        entries.extend(load_operation_history_entries(&connection)?);
    }

    entries.sort_by(|left, right| {
        history_sort_key(&right.timestamp)
            .cmp(&history_sort_key(&left.timestamp))
            .then_with(|| right.id.cmp(&left.id))
    });
    entries.truncate(limit);

    Ok(HistoryList {
        entries,
        skill_usage_count,
        operation_count,
    })
}

pub(crate) fn load_operation(connection: &Connection, id: &str) -> Result<OperationRecord> {
    connection
        .query_row(
            "
            SELECT id, type, status, actor, entity_type, entity_name,
                   started_at, finished_at, summary, error, payload_json
            FROM operations
            WHERE id = ?1
            ",
            params![id],
            operation_from_row,
        )
        .map_err(|error| error.to_string())
}

pub(crate) fn operation_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<OperationRecord> {
    let status: String = row.get(2)?;
    let payload_json: String = row.get(10)?;

    Ok(OperationRecord {
        id: row.get(0)?,
        operation_type: row.get(1)?,
        status: parse_operation_status(&status).unwrap_or(OperationStatus::Failed),
        actor: row.get(3)?,
        entity_type: row.get(4)?,
        entity_name: row.get(5)?,
        started_at: row.get(6)?,
        finished_at: row.get(7)?,
        summary: row.get(8)?,
        error: row.get(9)?,
        payload: serde_json::from_str(&payload_json).unwrap_or_else(|_| serde_json::json!({})),
    })
}

pub(crate) fn history_table_count(connection: &Connection, table: &str) -> Result<usize> {
    let table = match table {
        "skill_usage_events" => "skill_usage_events",
        "operations" => "operations",
        _ => return Err(format!("Unknown history table: {table}")),
    };
    let count: i64 = connection
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .map_err(|error| error.to_string())?;
    Ok(usize::try_from(count.max(0)).unwrap_or_default())
}

pub(crate) fn load_skill_usage_history_entries(
    connection: &Connection,
) -> Result<Vec<HistoryEntry>> {
    let mut statement = connection
        .prepare(
            "
            SELECT id, skill_name, agent_id, runtime_root, used_at, prompt_excerpt
            FROM skill_usage_events
            ORDER BY used_at DESC, id DESC
            LIMIT 500
            ",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| {
            let skill_name: String = row.get(1)?;
            let agent_id: String = row.get(2)?;
            let runtime_root: String = row.get(3)?;
            Ok(HistoryEntry {
                id: row.get(0)?,
                kind: HistoryEntryKind::SkillUsage,
                timestamp: row.get(4)?,
                title: format!("Skill call: {skill_name}"),
                subtitle: format!(
                    "{agent_id} in {}",
                    compact_home_path(&PathBuf::from(&runtime_root))
                ),
                prompt_excerpt: row.get(5)?,
                status: None,
                skill_name: Some(skill_name),
                agent_id: Some(agent_id),
                runtime_root: Some(PathBuf::from(runtime_root)),
                operation_type: None,
                actor: None,
                entity_type: None,
                entity_name: None,
                error: None,
            })
        })
        .map_err(|error| error.to_string())?;
    let mut entries = Vec::new();

    for row in rows {
        entries.push(row.map_err(|error| error.to_string())?);
    }

    Ok(entries)
}

pub(crate) fn load_operation_history_entries(connection: &Connection) -> Result<Vec<HistoryEntry>> {
    let mut statement = connection
        .prepare(
            "
            SELECT id, type, status, actor, entity_type, entity_name,
                   started_at, finished_at, summary, error
            FROM operations
            ORDER BY COALESCE(finished_at, started_at) DESC, id DESC
            LIMIT 500
            ",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| {
            let status_raw: String = row.get(2)?;
            let status = parse_operation_status(&status_raw).unwrap_or(OperationStatus::Failed);
            let operation_type: String = row.get(1)?;
            let actor: String = row.get(3)?;
            let entity_type: String = row.get(4)?;
            let entity_name: String = row.get(5)?;
            let started_at: String = row.get(6)?;
            let finished_at: Option<String> = row.get(7)?;
            let summary: String = row.get(8)?;
            let error: Option<String> = row.get(9)?;

            Ok(HistoryEntry {
                id: row.get(0)?,
                kind: HistoryEntryKind::Operation,
                timestamp: finished_at.unwrap_or(started_at),
                title: abbreviate_history_sha_values(&summary),
                subtitle: format!("{operation_type} by {actor}"),
                prompt_excerpt: None,
                status: Some(status),
                skill_name: (entity_type == "skill").then_some(entity_name.clone()),
                agent_id: None,
                runtime_root: None,
                operation_type: Some(operation_type),
                actor: Some(actor),
                entity_type: Some(entity_type),
                entity_name: Some(entity_name),
                error,
            })
        })
        .map_err(|error| error.to_string())?;
    let mut entries = Vec::new();

    for row in rows {
        entries.push(row.map_err(|error| error.to_string())?);
    }

    Ok(entries)
}

pub(crate) fn history_sort_key(timestamp: &str) -> i128 {
    let value = timestamp.trim();
    if let Ok(seconds) = value.parse::<i128>() {
        return seconds;
    }
    DateTime::parse_from_rfc3339(value)
        .map(|date| i128::from(date.timestamp()))
        .unwrap_or_default()
}

pub(crate) fn compact_home_path(path: &Path) -> String {
    let home = home_dir();
    path.strip_prefix(&home)
        .map(|relative| format!("~/{}", relative.to_string_lossy()))
        .unwrap_or_else(|_| path.to_string_lossy().to_string())
}

pub(crate) fn abbreviate_history_sha_values(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut buffer = String::new();

    for character in input.chars() {
        if character.is_ascii_hexdigit() {
            buffer.push(character);
            continue;
        }

        push_abbreviated_sha_buffer(&mut output, &buffer);
        buffer.clear();
        output.push(character);
    }

    push_abbreviated_sha_buffer(&mut output, &buffer);
    output
}

pub(crate) fn push_abbreviated_sha_buffer(output: &mut String, buffer: &str) {
    if buffer.len() >= 16 {
        output.push_str(&buffer[..12]);
    } else {
        output.push_str(buffer);
    }
}

pub(crate) fn parse_operation_status(value: &str) -> Option<OperationStatus> {
    match value {
        "started" => Some(OperationStatus::Started),
        "succeeded" => Some(OperationStatus::Succeeded),
        "failed" => Some(OperationStatus::Failed),
        "cancelled" => Some(OperationStatus::Cancelled),
        _ => None,
    }
}
