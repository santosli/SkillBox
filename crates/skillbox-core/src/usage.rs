use crate::*;

pub fn record_skill_usage(
    request: RecordSkillUsageRequest,
    managed_root: impl AsRef<Path>,
) -> Result<SkillUsageRecordResult> {
    let paths = ensure_managed_layout(managed_root.as_ref().to_path_buf())?;
    let skill_name = request.skill_name.trim().to_string();
    validate_skill_name(&skill_name)?;
    let agent_id = normalize_usage_agent_id(&request.agent_id)?;
    let runtime_root = normalize_usage_runtime_root(request.runtime_root)?;
    let runtime_root_value = runtime_root.to_string_lossy().to_string();
    let event_id = normalize_usage_event_id(request.event_id)?;
    let used_at = normalize_usage_timestamp(request.used_at.as_deref())?;
    let recorded_at = current_rfc3339_timestamp();
    let prompt_excerpt = normalize_usage_prompt_excerpt(request.prompt_excerpt.as_deref());
    let metadata_json = normalize_usage_metadata(request.metadata)?;
    let connection = open_database(&paths.database_path).map_err(|error| error.to_string())?;

    if let Some(event_id_value) = event_id.as_deref() {
        let existing = connection
            .query_row(
                "
                SELECT used_at, recorded_at, prompt_excerpt
                FROM skill_usage_events
                WHERE agent_id = ?1 AND runtime_root = ?2 AND event_id = ?3
                ",
                params![&agent_id, &runtime_root_value, event_id_value],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                    ))
                },
            )
            .optional()
            .map_err(|error| error.to_string())?;

        if let Some((existing_used_at, existing_recorded_at, existing_prompt_excerpt)) = existing {
            if existing_prompt_excerpt.is_none() {
                if let Some(prompt_excerpt_value) = prompt_excerpt.as_deref() {
                    connection
                        .execute(
                            "
                            UPDATE skill_usage_events
                            SET prompt_excerpt = ?1
                            WHERE agent_id = ?2
                              AND runtime_root = ?3
                              AND event_id = ?4
                              AND prompt_excerpt IS NULL
                            ",
                            params![
                                prompt_excerpt_value,
                                &agent_id,
                                &runtime_root_value,
                                event_id_value,
                            ],
                        )
                        .map_err(|error| error.to_string())?;
                }
            }
            let usage =
                load_usage_stat_for_key(&connection, &skill_name, &agent_id, &runtime_root_value)?;
            return Ok(SkillUsageRecordResult {
                skill_name,
                agent_id,
                runtime_root,
                event_id,
                used_at: existing_used_at.clone(),
                recorded_at: existing_recorded_at,
                usage_count: usage.usage_count,
                last_used_at: usage.last_used_at.unwrap_or(existing_used_at),
                deduplicated: true,
            });
        }
    }

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
              metadata_json
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            ",
            params![
                usage_event_row_id(),
                event_id.as_deref(),
                &skill_name,
                &agent_id,
                &runtime_root_value,
                &used_at,
                &recorded_at,
                prompt_excerpt.as_deref(),
                &metadata_json,
            ],
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
            VALUES (?1, ?2, ?3, 1, ?4)
            ON CONFLICT(skill_name, agent_id, runtime_root) DO UPDATE SET
              usage_count = skill_usage_stats.usage_count + 1,
              last_used_at = CASE
                WHEN excluded.last_used_at > skill_usage_stats.last_used_at
                THEN excluded.last_used_at
                ELSE skill_usage_stats.last_used_at
              END,
              updated_at = CURRENT_TIMESTAMP
            ",
            params![&skill_name, &agent_id, &runtime_root_value, &used_at],
        )
        .map_err(|error| error.to_string())?;

    let usage = load_usage_stat_for_key(&connection, &skill_name, &agent_id, &runtime_root_value)?;
    Ok(SkillUsageRecordResult {
        skill_name,
        agent_id,
        runtime_root,
        event_id,
        used_at,
        recorded_at,
        usage_count: usage.usage_count,
        last_used_at: usage.last_used_at.unwrap_or_default(),
        deduplicated: false,
    })
}

pub(crate) fn normalize_usage_timestamp(value: Option<&str>) -> Result<String> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(current_rfc3339_timestamp());
    };
    DateTime::parse_from_rfc3339(value)
        .map(|timestamp| {
            timestamp
                .with_timezone(&Utc)
                .to_rfc3339_opts(SecondsFormat::Secs, false)
        })
        .map_err(|error| format!("Invalid usage timestamp: {error}"))
}

pub(crate) fn normalize_usage_agent_id(value: &str) -> Result<String> {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.is_empty()
        || normalized
            .chars()
            .any(|character| !matches!(character, 'a'..='z' | '0'..='9' | '-' | '_'))
    {
        return Err(format!("Invalid usage agent id: {value}"));
    }
    Ok(normalized)
}

pub(crate) fn normalize_usage_runtime_root(path: PathBuf) -> Result<PathBuf> {
    let expanded = expand_home(path);
    if !expanded.is_absolute() {
        return Err("Usage runtime root must be an absolute path.".to_string());
    }
    Ok(fs::canonicalize(&expanded).unwrap_or(expanded))
}

pub(crate) fn normalize_usage_event_id(value: Option<String>) -> Result<Option<String>> {
    value
        .map(|event_id| {
            let event_id = event_id.trim().to_string();
            if event_id.is_empty() {
                Err("Usage event id cannot be empty.".to_string())
            } else {
                Ok(event_id)
            }
        })
        .transpose()
}

pub(crate) fn normalize_usage_metadata(value: Option<serde_json::Value>) -> Result<String> {
    let metadata = value.unwrap_or_else(|| serde_json::Value::Object(serde_json::Map::new()));
    if !metadata.is_object() {
        return Err("Usage metadata must be a JSON object.".to_string());
    }
    if let Some(key) = usage_metadata_content_key(&metadata) {
        return Err(format!(
            "Usage metadata cannot include content field: {key}"
        ));
    }

    let metadata_json = serde_json::to_string(&metadata).map_err(|error| error.to_string())?;
    if metadata_json.len() > MAX_USAGE_METADATA_JSON_BYTES {
        return Err(format!(
            "Usage metadata must be at most {MAX_USAGE_METADATA_JSON_BYTES} bytes."
        ));
    }
    Ok(metadata_json)
}

pub(crate) fn normalize_usage_prompt_excerpt(value: Option<&str>) -> Option<String> {
    let stripped = strip_skill_blocks(value?);
    let stripped = strip_skill_markdown_links(&stripped);
    let compact = stripped.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.is_empty() {
        return None;
    }

    let mut chars = compact.chars();
    let mut excerpt = chars
        .by_ref()
        .take(MAX_USAGE_PROMPT_EXCERPT_CHARS)
        .collect::<String>();
    if chars.next().is_some() {
        excerpt.push_str("...");
    }
    Some(excerpt)
}

pub(crate) fn strip_skill_blocks(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut remaining = input;

    while let Some(start) = remaining.find("<skill>") {
        output.push_str(&remaining[..start]);
        let after_start = &remaining[start + "<skill>".len()..];
        let Some(end) = after_start.find("</skill>") else {
            return output;
        };
        remaining = &after_start[end + "</skill>".len()..];
    }

    output.push_str(remaining);
    output
}

pub(crate) fn strip_skill_markdown_links(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut remaining = input;

    while let Some(start) = remaining.find("[$") {
        output.push_str(&remaining[..start]);
        let after_start = &remaining[start..];
        let Some(label_end) = after_start.find("](") else {
            output.push_str(after_start);
            return output;
        };
        let Some(link_end) = after_start[label_end + 2..].find(')') else {
            output.push_str(after_start);
            return output;
        };
        remaining = &after_start[label_end + 2 + link_end + 1..];
    }

    output.push_str(remaining);
    output
}

pub(crate) fn usage_metadata_content_key(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::Object(object) => {
            for (key, nested) in object {
                let normalized_key = key.to_ascii_lowercase();
                if USAGE_METADATA_CONTENT_KEYS
                    .iter()
                    .any(|content_key| content_key == &normalized_key)
                {
                    return Some(key.clone());
                }
                if let Some(content_key) = usage_metadata_content_key(nested) {
                    return Some(content_key);
                }
            }
            None
        }
        serde_json::Value::Array(values) => values.iter().find_map(usage_metadata_content_key),
        _ => None,
    }
}

pub(crate) fn usage_runtime_key(path: &Path) -> String {
    let expanded = expand_home(path.to_path_buf());
    fs::canonicalize(&expanded)
        .unwrap_or(expanded)
        .to_string_lossy()
        .to_string()
}

pub(crate) fn load_usage_by_skill(database_path: &Path) -> Result<HashMap<String, UsageSummary>> {
    let connection = open_database(database_path).map_err(|error| error.to_string())?;
    let mut statement = connection
        .prepare(
            "
            SELECT skill_name, SUM(usage_count), MAX(last_used_at)
            FROM skill_usage_stats
            GROUP BY skill_name
            ",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| {
            let usage_count: i64 = row.get(1)?;
            Ok((
                row.get::<_, String>(0)?,
                UsageSummary {
                    usage_count: usize::try_from(usage_count.max(0)).unwrap_or_default(),
                    last_used_at: row.get(2)?,
                },
            ))
        })
        .map_err(|error| error.to_string())?;
    let mut usage = HashMap::new();
    for row in rows {
        let (skill_name, summary) = row.map_err(|error| error.to_string())?;
        usage.insert(skill_name, summary);
    }
    Ok(usage)
}

pub(crate) fn load_usage_by_runtime(database_path: &Path) -> Result<HashMap<String, UsageSummary>> {
    let connection = open_database(database_path).map_err(|error| error.to_string())?;
    let mut statement = connection
        .prepare(
            "
            SELECT runtime_root, SUM(usage_count), MAX(last_used_at)
            FROM skill_usage_stats
            GROUP BY runtime_root
            ",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| {
            let usage_count: i64 = row.get(1)?;
            Ok((
                row.get::<_, String>(0)?,
                UsageSummary {
                    usage_count: usize::try_from(usage_count.max(0)).unwrap_or_default(),
                    last_used_at: row.get(2)?,
                },
            ))
        })
        .map_err(|error| error.to_string())?;
    let mut usage = HashMap::new();
    for row in rows {
        let (runtime_root, summary) = row.map_err(|error| error.to_string())?;
        usage.insert(runtime_root, summary);
    }
    Ok(usage)
}

pub(crate) fn load_usage_by_skill_runtime(
    database_path: &Path,
) -> Result<HashMap<(String, String), UsageSummary>> {
    let connection = open_database(database_path).map_err(|error| error.to_string())?;
    let mut statement = connection
        .prepare(
            "
            SELECT skill_name, runtime_root, usage_count, last_used_at
            FROM skill_usage_stats
            ",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| {
            let usage_count: i64 = row.get(2)?;
            Ok((
                (row.get::<_, String>(0)?, row.get::<_, String>(1)?),
                UsageSummary {
                    usage_count: usize::try_from(usage_count.max(0)).unwrap_or_default(),
                    last_used_at: row.get(3)?,
                },
            ))
        })
        .map_err(|error| error.to_string())?;
    let mut usage = HashMap::new();
    for row in rows {
        let (key, summary) = row.map_err(|error| error.to_string())?;
        usage.insert(key, summary);
    }
    Ok(usage)
}

pub(crate) fn load_usage_stat_for_key(
    connection: &Connection,
    skill_name: &str,
    agent_id: &str,
    runtime_root: &str,
) -> Result<UsageSummary> {
    connection
        .query_row(
            "
            SELECT usage_count, last_used_at
            FROM skill_usage_stats
            WHERE skill_name = ?1 AND agent_id = ?2 AND runtime_root = ?3
            ",
            params![skill_name, agent_id, runtime_root],
            |row| {
                let usage_count: i64 = row.get(0)?;
                Ok(UsageSummary {
                    usage_count: usize::try_from(usage_count.max(0)).unwrap_or_default(),
                    last_used_at: Some(row.get(1)?),
                })
            },
        )
        .optional()
        .map(|usage| usage.unwrap_or_default())
        .map_err(|error| error.to_string())
}
