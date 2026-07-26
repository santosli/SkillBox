use crate::*;
use rusqlite::OpenFlags;

const MAX_CURSOR_BACKFILL_ERRORS: usize = 20;
const MAX_CURSOR_IDENTIFIER_CHARS: usize = 256;

#[derive(Debug)]
struct CursorComposerSession {
    composer_id: String,
    workspace: Option<PathBuf>,
}

pub fn backfill_cursor_session_usage(
    request: BackfillCursorSessionUsageRequest,
    managed_root: impl AsRef<Path>,
) -> Result<BackfillCodexSessionUsageResult> {
    backfill_cursor_session_usage_for_home(request, home_dir(), managed_root)
}

pub(crate) fn backfill_cursor_session_usage_for_home(
    request: BackfillCursorSessionUsageRequest,
    home: impl AsRef<Path>,
    managed_root: impl AsRef<Path>,
) -> Result<BackfillCodexSessionUsageResult> {
    let home = home.as_ref();
    let managed_root = managed_root.as_ref();
    let paths = ensure_managed_layout(managed_root.to_path_buf())?;
    let database_path_was_explicit = request.database_path.is_some();
    let database_path = request.database_path.unwrap_or_else(|| {
        home.join("Library")
            .join("Application Support")
            .join("Cursor")
            .join("User")
            .join("globalStorage")
            .join("state.vscdb")
    });
    let projects_root_was_explicit = request.projects_root.is_some();
    let projects_root = expand_home(
        request
            .projects_root
            .unwrap_or_else(|| home.join(".cursor/projects")),
    );
    let mut result = BackfillCodexSessionUsageResult::default();
    let mut managed_database =
        open_database(&paths.database_path).map_err(|error| error.to_string())?;
    let mut workspace_roots = Vec::new();
    let state_database_available = database_path.is_file();
    if state_database_available {
        let cursor_database = open_cursor_database_read_only(&database_path)?;
        validate_cursor_database_schema(&cursor_database)?;
        let sessions = load_cursor_composer_sessions(&cursor_database, &mut result)?;
        let mut sessions_by_id = HashMap::new();
        for session in sessions {
            sessions_by_id.insert(session.composer_id, session.workspace);
        }
        workspace_roots.extend(sessions_by_id.values().filter_map(Option::as_ref).cloned());
        let runtime_roots = cursor_runtime_roots(
            home,
            &paths,
            sessions_by_id.values().filter_map(Option::as_deref),
        );
        let mut seen_candidates = HashSet::new();
        stream_cursor_skill_rules(
            &cursor_database,
            &sessions_by_id,
            home,
            &paths,
            &runtime_roots,
            &mut managed_database,
            &mut seen_candidates,
            &mut result,
        )?;
    } else if database_path_was_explicit {
        return Err(format!(
            "Cursor history database was not found: {}",
            database_path.display()
        ));
    }
    result.scanned_cursor_state_sessions = result.scanned_files;
    result.cursor_state_references = result
        .recorded
        .saturating_add(result.deduplicated)
        .saturating_add(result.upgraded);
    let state_audit_result = state_database_available.then(|| result.clone());

    let transcript_root_available = projects_root.is_dir();
    let mut transcript_audit_result = None;
    if transcript_root_available {
        let runtime_roots =
            cursor_runtime_roots(home, &paths, workspace_roots.iter().map(PathBuf::as_path));
        let transcript_result = backfill_cursor_agent_transcript_usage(
            &projects_root,
            home,
            &runtime_roots,
            &mut managed_database,
        )?;
        transcript_audit_result = Some(transcript_result.clone());
        merge_cursor_agent_transcript_backfill_result(&mut result, transcript_result);
    } else if projects_root_was_explicit {
        return Err(format!(
            "Cursor projects root was not found: {}",
            projects_root.display()
        ));
    }
    if !state_database_available && !transcript_root_available {
        return Err(format!(
            "Cursor history sources were not found at {} or {}.",
            database_path.display(),
            projects_root.display()
        ));
    }

    let scanned_sessions = u32::try_from(result.scanned_cursor_state_sessions).unwrap_or(u32::MAX);
    if let Err(error) = write_u32_preference(
        &paths.database_path,
        "cursor_usage_backfill_scanned_sessions",
        scanned_sessions,
    ) {
        push_cursor_backfill_error(
            &mut result.errors,
            format!("Unable to persist Cursor scan coverage: {error}"),
        );
    }
    let scanned_transcripts =
        u32::try_from(result.scanned_cursor_transcript_files).unwrap_or(u32::MAX);
    if let Err(error) = write_u32_preference(
        &paths.database_path,
        "cursor_usage_backfill_scanned_transcript_files",
        scanned_transcripts,
    ) {
        push_cursor_backfill_error(
            &mut result.errors,
            format!("Unable to persist Cursor transcript coverage: {error}"),
        );
    }
    if let Some(state_audit_result) = state_audit_result {
        if let Err(error) = persist_usage_backfill_audit(
            &paths.database_path,
            "cursor_session_backfill",
            result.scanned_cursor_state_sessions,
            &state_audit_result,
        ) {
            push_cursor_backfill_error(
                &mut result.errors,
                format!("Unable to persist Cursor usage audit: {error}"),
            );
        }
    }
    if let Some(transcript_audit_result) = transcript_audit_result {
        if let Err(error) = persist_usage_backfill_audit(
            &paths.database_path,
            "cursor_agent_transcript_read",
            result.scanned_cursor_transcript_files,
            &transcript_audit_result,
        ) {
            push_cursor_backfill_error(
                &mut result.errors,
                format!("Unable to persist Cursor transcript audit: {error}"),
            );
        }
    }

    Ok(result)
}

fn open_cursor_database_read_only(path: &Path) -> Result<Connection> {
    if !path.is_file() {
        return Err(format!(
            "Cursor history database was not found: {}",
            path.display()
        ));
    }
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|error| format!("Unable to open Cursor history database read-only: {error}"))?;
    connection
        .busy_timeout(Duration::from_secs(2))
        .map_err(|error| format!("Unable to configure Cursor history database timeout: {error}"))?;
    connection
        .pragma_update(None, "query_only", true)
        .map_err(|error| format!("Unable to enforce read-only Cursor history access: {error}"))?;
    Ok(connection)
}

fn validate_cursor_database_schema(connection: &Connection) -> Result<()> {
    validate_cursor_table_columns(
        connection,
        "cursorDiskKV",
        &[("key", "TEXT"), ("value", "BLOB")],
    )?;
    validate_cursor_table_columns(
        connection,
        "composerHeaders",
        &[
            ("composerId", "TEXT"),
            ("isSubagent", "INTEGER"),
            ("value", "TEXT"),
        ],
    )
}

fn validate_cursor_table_columns(
    connection: &Connection,
    table: &str,
    required: &[(&str, &str)],
) -> Result<()> {
    let mut statement = connection
        .prepare(&format!("PRAGMA table_info({table})"))
        .map_err(|error| format!("Unable to inspect Cursor database schema: {error}"))?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(1)?, row.get::<_, String>(2)?))
        })
        .map_err(|error| format!("Unable to inspect Cursor database schema: {error}"))?;
    let columns = rows
        .collect::<std::result::Result<HashMap<_, _>, _>>()
        .map_err(|error| format!("Unable to inspect Cursor database schema: {error}"))?;

    for (column, expected_type) in required {
        let Some(actual_type) = columns.get(*column) else {
            return Err(format!(
                "Unsupported Cursor database schema: {table}.{column} is missing."
            ));
        };
        if !actual_type.eq_ignore_ascii_case(expected_type) {
            return Err(format!(
                "Unsupported Cursor database schema: {table}.{column} must be {expected_type}."
            ));
        }
    }
    Ok(())
}

fn load_cursor_composer_sessions(
    connection: &Connection,
    result: &mut BackfillCodexSessionUsageResult,
) -> Result<Vec<CursorComposerSession>> {
    let mut statement = connection
        .prepare(
            "
            SELECT
              composerId,
              CASE
                WHEN json_valid(value)
                THEN CAST(json_extract(value, '$.workspaceIdentifier.uri.fsPath') AS TEXT)
                ELSE NULL
              END,
              json_valid(value)
            FROM composerHeaders
            WHERE COALESCE(isSubagent, 0) = 0
            ORDER BY composerId
            ",
        )
        .map_err(|error| format!("Unable to scan Cursor composer sessions: {error}"))?;
    let mut rows = statement
        .query([])
        .map_err(|error| format!("Unable to scan Cursor composer sessions: {error}"))?;
    let mut sessions = Vec::new();

    loop {
        let row = match rows.next() {
            Ok(Some(row)) => row,
            Ok(None) => break,
            Err(error) => {
                result.skipped = result.skipped.saturating_add(1);
                push_cursor_backfill_error(
                    &mut result.errors,
                    format!("Unable to read a Cursor composer header: {error}"),
                );
                continue;
            }
        };
        result.scanned_files = result.scanned_files.saturating_add(1);
        let composer_id = match row.get::<_, String>(0) {
            Ok(value) if valid_cursor_identifier(&value) => value,
            _ => {
                result.skipped = result.skipped.saturating_add(1);
                push_cursor_backfill_error(
                    &mut result.errors,
                    "Skipped a Cursor composer with an invalid identifier.".to_string(),
                );
                continue;
            }
        };
        let json_valid = row.get::<_, i64>(2).unwrap_or_default() != 0;
        if !json_valid {
            result.skipped = result.skipped.saturating_add(1);
            push_cursor_backfill_error(
                &mut result.errors,
                format!("Skipped Cursor composer {composer_id}: invalid header JSON."),
            );
            continue;
        }
        let workspace = row
            .get::<_, Option<String>>(1)
            .ok()
            .flatten()
            .map(PathBuf::from)
            .filter(|path| path.is_absolute());
        sessions.push(CursorComposerSession {
            composer_id,
            workspace,
        });
    }

    Ok(sessions)
}

#[allow(clippy::too_many_arguments)]
fn stream_cursor_skill_rules(
    cursor_database: &Connection,
    sessions: &HashMap<String, Option<PathBuf>>,
    home: &Path,
    paths: &ManagedPaths,
    runtime_roots: &[PathBuf],
    managed_database: &mut Connection,
    seen_candidates: &mut HashSet<(String, String, PathBuf)>,
    result: &mut BackfillCodexSessionUsageResult,
) -> Result<()> {
    let mut statement = cursor_database
        .prepare(
            "
            SELECT
              bubbles.key,
              CAST(json_extract(bubbles.value_json, '$.createdAt') AS TEXT),
              CAST(json_extract(rule.value, '$.filename') AS TEXT)
            FROM (
              SELECT
                key,
                CASE
                  WHEN json_valid(value) THEN CAST(value AS TEXT)
                  ELSE '{}'
                END AS value_json
              FROM cursorDiskKV
              WHERE key LIKE 'bubbleId:%'
            ) AS bubbles
            JOIN json_each(
              CASE
                WHEN json_type(bubbles.value_json, '$.context.cursorRules') = 'array'
                THEN json_extract(bubbles.value_json, '$.context.cursorRules')
                ELSE '[]'
              END
            ) AS rule
            WHERE CAST(json_extract(bubbles.value_json, '$.type') AS INTEGER) = 1
              AND json_type(rule.value, '$.filename') = 'text'
              AND CAST(json_extract(rule.value, '$.addedWithoutMention') AS INTEGER) = 0
            ORDER BY bubbles.key, rule.key
            ",
        )
        .map_err(|error| format!("Unable to scan Cursor human bubbles: {error}"))?;
    let mut rows = statement
        .query([])
        .map_err(|error| format!("Unable to scan Cursor human bubbles: {error}"))?;

    loop {
        let row = match rows.next() {
            Ok(Some(row)) => row,
            Ok(None) => break,
            Err(error) => {
                result.skipped = result.skipped.saturating_add(1);
                push_cursor_backfill_error(
                    &mut result.errors,
                    format!("Unable to read a Cursor bubble rule reference: {error}"),
                );
                continue;
            }
        };
        let key = match row.get::<_, String>(0) {
            Ok(key) => key,
            Err(error) => {
                result.skipped = result.skipped.saturating_add(1);
                push_cursor_backfill_error(
                    &mut result.errors,
                    format!("Skipped a Cursor bubble with an invalid key: {error}"),
                );
                continue;
            }
        };
        let Some((composer_id, bubble_id)) = parse_cursor_bubble_key(&key) else {
            result.skipped = result.skipped.saturating_add(1);
            push_cursor_backfill_error(
                &mut result.errors,
                "Skipped a Cursor bubble with an invalid identifier.".to_string(),
            );
            continue;
        };
        let Some(workspace) = sessions.get(composer_id) else {
            continue;
        };
        let filename = match row.get::<_, String>(2) {
            Ok(filename) => filename,
            Err(error) => {
                result.skipped = result.skipped.saturating_add(1);
                push_cursor_backfill_error(
                    &mut result.errors,
                    format!("Skipped Cursor bubble {bubble_id}: invalid rule filename ({error})."),
                );
                continue;
            }
        };
        if !cursor_rule_targets_skill(&filename) {
            continue;
        }
        let Some((skill_name, canonical_skill_path)) =
            resolve_cursor_skill_path(&filename, workspace.as_deref(), home, paths, runtime_roots)
        else {
            result.skipped = result.skipped.saturating_add(1);
            push_cursor_backfill_error(
                &mut result.errors,
                format!("Skipped Cursor bubble {bubble_id}: skill path is unavailable or unsafe."),
            );
            continue;
        };

        let identity = (
            composer_id.to_string(),
            bubble_id.to_string(),
            canonical_skill_path.clone(),
        );
        if !seen_candidates.insert(identity) {
            continue;
        }
        result.discovered = result.discovered.saturating_add(1);
        let used_at = row
            .get::<_, Option<String>>(1)
            .ok()
            .flatten()
            .and_then(|value| normalize_cursor_timestamp(&value));
        match record_cursor_skill_candidate(
            composer_id,
            bubble_id,
            &skill_name,
            &canonical_skill_path,
            workspace.as_deref(),
            runtime_roots,
            used_at,
            managed_database,
        ) {
            Ok(record) if record.upgraded => {
                result.upgraded = result.upgraded.saturating_add(1);
            }
            Ok(record) if record.deduplicated => {
                result.deduplicated = result.deduplicated.saturating_add(1);
            }
            Ok(_) => {
                result.recorded = result.recorded.saturating_add(1);
            }
            Err(error) => {
                result.skipped = result.skipped.saturating_add(1);
                push_cursor_backfill_error(
                    &mut result.errors,
                    format!("Skipped Cursor skill {skill_name}: {error}"),
                );
            }
        }
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn record_cursor_skill_candidate(
    composer_id: &str,
    bubble_id: &str,
    skill_name: &str,
    canonical_skill_path: &Path,
    workspace: Option<&Path>,
    runtime_roots: &[PathBuf],
    used_at: Option<String>,
    connection: &mut Connection,
) -> Result<SkillUsageRecordResult> {
    let (runtime_root, _) = infer_usage_runtime_from_skill_path_with_roots(
        canonical_skill_path,
        "cursor",
        Some(runtime_roots),
        workspace,
    )?;
    let source_kind = if canonical_skill_path
        .components()
        .any(|component| component.as_os_str() == ".system")
    {
        "system"
    } else {
        "regular"
    };
    let event_id = format!(
        "cursor:{composer_id}:{bubble_id}:{}",
        sha256(&canonical_skill_path.to_string_lossy())
    );
    record_skill_usage_on_connection(
        RecordSkillUsageRequest {
            skill_name: skill_name.to_string(),
            agent_id: "cursor".to_string(),
            runtime_root,
            event_id: Some(event_id),
            used_at,
            prompt_excerpt: None,
            metadata: Some(serde_json::json!({
                "source": "cursor_session_backfill",
                "skill_source_kind": source_kind
            })),
        },
        connection,
        true,
    )
}

fn cursor_rule_targets_skill(filename: &str) -> bool {
    let filename = Path::new(filename.trim())
        .file_name()
        .and_then(|value| value.to_str());
    matches!(filename, Some("SKILL.md" | "SKILL.md.mdc"))
}

fn resolve_cursor_skill_path(
    filename: &str,
    workspace: Option<&Path>,
    home: &Path,
    paths: &ManagedPaths,
    runtime_roots: &[PathBuf],
) -> Option<(String, PathBuf)> {
    let mut requested = cursor_rule_backing_path(filename.trim(), home)?;
    if requested
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return None;
    }
    if requested.is_relative() {
        requested = workspace?.join(requested);
    }
    let skill_name = requested
        .parent()?
        .file_name()?
        .to_str()?
        .trim()
        .to_string();
    if validate_skill_name(&skill_name).is_err() {
        return None;
    }

    let mut candidates = vec![requested];
    for root in runtime_roots {
        candidates.push(root.join(&skill_name).join("SKILL.md"));
        candidates.push(root.join(".system").join(&skill_name).join("SKILL.md"));
    }
    if let Some(workspace) = workspace {
        candidates.push(
            workspace
                .join(".cursor")
                .join("rules")
                .join(&skill_name)
                .join("SKILL.md"),
        );
    }
    candidates.sort();
    candidates.dedup();

    let allowed_roots =
        cursor_allowed_skill_roots(paths, runtime_roots, workspace.into_iter().collect());
    for candidate in candidates {
        if candidate.file_name().and_then(|value| value.to_str()) != Some("SKILL.md") {
            continue;
        }
        let Ok(canonical) = fs::canonicalize(&candidate) else {
            continue;
        };
        if !canonical.is_file() || !cursor_path_is_allowed(&canonical, &allowed_roots) {
            continue;
        }
        let canonical_name = canonical
            .parent()
            .and_then(Path::file_name)
            .and_then(|value| value.to_str());
        if canonical_name != Some(skill_name.as_str()) {
            continue;
        }
        return Some((skill_name, canonical));
    }
    None
}

fn cursor_rule_backing_path(filename: &str, home: &Path) -> Option<PathBuf> {
    let raw = if let Some(relative) = filename.strip_prefix("~/") {
        home.join(relative)
    } else {
        PathBuf::from(filename)
    };
    match raw.file_name().and_then(|value| value.to_str()) {
        Some("SKILL.md") => Some(raw),
        Some("SKILL.md.mdc") => Some(raw.with_file_name("SKILL.md")),
        _ => None,
    }
}

fn cursor_runtime_roots<'a>(
    home: &Path,
    paths: &ManagedPaths,
    workspaces: impl Iterator<Item = &'a Path>,
) -> Vec<PathBuf> {
    let mut roots = runtime_roots_under(home);
    for workspace in workspaces {
        for (_, root) in project_runtime_roots() {
            roots.push(workspace.join(root.relative_path));
        }
        roots.push(workspace.join(".cursor").join("rules"));
    }
    roots.push(paths.user_skills_root.clone());
    roots.push(paths.remote_skills_root.clone());
    roots.sort();
    roots.dedup();
    roots
}

fn cursor_allowed_skill_roots(
    paths: &ManagedPaths,
    runtime_roots: &[PathBuf],
    workspace: Vec<&Path>,
) -> Vec<PathBuf> {
    let mut roots = runtime_roots.to_vec();
    roots.push(paths.user_skills_root.clone());
    roots.push(paths.remote_skills_root.clone());
    for workspace in workspace {
        roots.push(workspace.join(".cursor").join("rules"));
    }
    roots.sort();
    roots.dedup();
    roots
}

fn cursor_path_is_allowed(canonical: &Path, allowed_roots: &[PathBuf]) -> bool {
    allowed_roots.iter().any(|root| {
        fs::canonicalize(root)
            .ok()
            .is_some_and(|canonical_root| canonical.starts_with(canonical_root))
    })
}

fn parse_cursor_bubble_key(key: &str) -> Option<(&str, &str)> {
    let remainder = key.strip_prefix("bubbleId:")?;
    let (composer_id, bubble_id) = remainder.split_once(':')?;
    if valid_cursor_identifier(composer_id) && valid_cursor_identifier(bubble_id) {
        Some((composer_id, bubble_id))
    } else {
        None
    }
}

fn valid_cursor_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.chars().count() <= MAX_CURSOR_IDENTIFIER_CHARS
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
}

fn normalize_cursor_timestamp(value: &str) -> Option<String> {
    let value = value.trim();
    if let Ok(timestamp) = DateTime::parse_from_rfc3339(value) {
        return Some(
            timestamp
                .with_timezone(&Utc)
                .to_rfc3339_opts(SecondsFormat::Secs, false),
        );
    }
    let milliseconds = value.parse::<i64>().ok()?;
    DateTime::<Utc>::from_timestamp_millis(milliseconds)
        .map(|timestamp| timestamp.to_rfc3339_opts(SecondsFormat::Secs, false))
}

fn push_cursor_backfill_error(errors: &mut Vec<String>, message: String) {
    if errors.len() < MAX_CURSOR_BACKFILL_ERRORS {
        errors.push(message);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cursor_backfill_temp_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        let path = std::env::temp_dir().join(format!(
            "skillbox-cursor-backfill-{label}-{}-{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn create_cursor_database(path: &Path) -> Connection {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        let connection = Connection::open(path).unwrap();
        connection
            .execute_batch(
                "
                CREATE TABLE cursorDiskKV (
                  key TEXT UNIQUE ON CONFLICT REPLACE,
                  value BLOB
                );
                CREATE TABLE composerHeaders (
                  composerId TEXT PRIMARY KEY,
                  workspaceId TEXT,
                  createdAt INTEGER,
                  lastUpdatedAt INTEGER,
                  isArchived INTEGER,
                  isSubagent INTEGER,
                  recency INTEGER,
                  checkpointAt INTEGER,
                  value TEXT
                );
                ",
            )
            .unwrap();
        connection
    }

    #[test]
    fn cursor_backfill_reads_only_human_skill_filenames_and_is_idempotent() {
        let root = cursor_backfill_temp_dir("records");
        let home = root.join("home");
        let managed_root = root.join("managed");
        let workspace = root.join("workspace");
        let skill_path = workspace
            .join(".cursor")
            .join("skills")
            .join("demo-skill")
            .join("SKILL.md");
        fs::create_dir_all(skill_path.parent().unwrap()).unwrap();
        fs::write(&skill_path, "---\nname: demo-skill\n---\nsecret body\n").unwrap();

        let cursor_path = root.join("state.vscdb");
        let connection = create_cursor_database(&cursor_path);
        connection
            .execute(
                "
                INSERT INTO composerHeaders (composerId, isSubagent, value)
                VALUES (?1, 0, ?2)
                ",
                params![
                    "composer-1",
                    serde_json::json!({
                        "workspaceIdentifier": {
                            "uri": {"fsPath": workspace}
                        }
                    })
                    .to_string()
                ],
            )
            .unwrap();
        connection
            .execute(
                "
                INSERT INTO composerHeaders (composerId, isSubagent, value)
                VALUES (?1, 1, ?2)
                ",
                params![
                    "composer-subagent",
                    serde_json::json!({
                        "workspaceIdentifier": {
                            "uri": {"fsPath": workspace}
                        }
                    })
                    .to_string()
                ],
            )
            .unwrap();
        let human_bubble = serde_json::json!({
            "type": 1,
            "createdAt": "2026-07-25T01:02:03.456Z",
            "text": "must never be persisted",
            "context": {
                "cursorRules": [
                    {
                        "filename": ".cursor/skills/demo-skill/SKILL.md",
                        "addedWithoutMention": false,
                        "text": "rule body must never be persisted"
                    },
                    {
                        "filename": ".cursor/skills/demo-skill/SKILL.md",
                        "addedWithoutMention": false
                    },
                    {
                        "filename": ".cursor/skills/demo-skill/SKILL.md",
                        "addedWithoutMention": true
                    }
                ]
            }
        });
        connection
            .execute(
                "INSERT INTO cursorDiskKV (key, value) VALUES (?1, ?2)",
                params!["bubbleId:composer-1:bubble-1", human_bubble.to_string()],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO cursorDiskKV (key, value) VALUES (?1, ?2)",
                params![
                    "bubbleId:composer-1:bubble-assistant",
                    serde_json::json!({
                        "type": 2,
                        "context": {
                            "cursorRules": [{
                                "filename": ".cursor/skills/demo-skill/SKILL.md",
                                "addedWithoutMention": false
                            }]
                        }
                    })
                    .to_string()
                ],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO cursorDiskKV (key, value) VALUES (?1, ?2)",
                params![
                    "bubbleId:composer-subagent:bubble-1",
                    serde_json::json!({
                        "type": 1,
                        "context": {
                            "cursorRules": [{
                                "filename": ".cursor/skills/demo-skill/SKILL.md",
                                "addedWithoutMention": false
                            }]
                        }
                    })
                    .to_string()
                ],
            )
            .unwrap();
        drop(connection);

        let read_only = open_cursor_database_read_only(&cursor_path).unwrap();
        let mut inspection = BackfillCodexSessionUsageResult::default();
        let loaded_sessions = load_cursor_composer_sessions(&read_only, &mut inspection).unwrap();
        assert_eq!(
            loaded_sessions
                .iter()
                .map(|session| session.composer_id.as_str())
                .collect::<Vec<_>>(),
            vec!["composer-1"]
        );
        assert_eq!(
            parse_cursor_bubble_key("bubbleId:composer-1:bubble-1"),
            Some(("composer-1", "bubble-1"))
        );
        drop(read_only);

        let request = BackfillCursorSessionUsageRequest {
            database_path: Some(cursor_path.clone()),
            ..BackfillCursorSessionUsageRequest::default()
        };
        let first =
            backfill_cursor_session_usage_for_home(request.clone(), &home, &managed_root).unwrap();
        assert_eq!(first.scanned_files, 1);
        assert_eq!(first.discovered, 1);
        assert_eq!(first.recorded, 1);
        assert_eq!(first.deduplicated, 0);
        assert!(first.errors.is_empty(), "{:?}", first.errors);

        let second = backfill_cursor_session_usage_for_home(request, &home, &managed_root).unwrap();
        assert_eq!(second.scanned_files, 1);
        assert_eq!(second.discovered, 1);
        assert_eq!(second.recorded, 0);
        assert_eq!(second.deduplicated, 1);

        let managed_paths = managed_paths(&managed_root);
        let managed_database = open_database(&managed_paths.database_path).unwrap();
        let (agent, event_id, prompt_excerpt, metadata): (String, String, Option<String>, String) =
            managed_database
                .query_row(
                    "
                SELECT agent_id, event_id, prompt_excerpt, metadata_json
                FROM skill_usage_events
                ",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )
                .unwrap();
        assert_eq!(agent, "cursor");
        assert!(event_id.starts_with("cursor:composer-1:bubble-1:"));
        assert_eq!(prompt_excerpt, None);
        assert!(metadata.contains("\"source\":\"cursor_session_backfill\""));
        assert!(!metadata.contains("must never be persisted"));
        assert_eq!(
            read_u32_preference(
                &managed_paths.database_path,
                "cursor_usage_backfill_scanned_sessions"
            )
            .unwrap(),
            Some(1)
        );
        let rankings = list_skill_usage_rankings_at(
            SkillUsageRankingRequest {
                range: SkillUsageRankingRange::AllTime,
                include_unmanaged: true,
                ..SkillUsageRankingRequest::default()
            },
            &managed_root,
            DateTime::parse_from_rfc3339("2026-07-26T00:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
        )
        .unwrap();
        assert_eq!(rankings.coverage.cursor_session_backfill_calls, 0);
        assert_eq!(rankings.coverage.history_references, 1);
        assert_eq!(rankings.coverage.claude_code_session_backfill_calls, 0);
        assert_eq!(rankings.coverage.scanned_cursor_sessions, 1);

        let cursor_database = Connection::open(cursor_path).unwrap();
        let source_rows: i64 = cursor_database
            .query_row("SELECT COUNT(*) FROM cursorDiskKV", [], |row| row.get(0))
            .unwrap();
        assert_eq!(source_rows, 3);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn cursor_backfill_fails_closed_for_unsupported_schema() {
        let root = cursor_backfill_temp_dir("schema");
        let cursor_path = root.join("state.vscdb");
        let connection = Connection::open(&cursor_path).unwrap();
        connection
            .execute("CREATE TABLE cursorDiskKV (key TEXT, value BLOB)", [])
            .unwrap();
        drop(connection);

        let error = backfill_cursor_session_usage_for_home(
            BackfillCursorSessionUsageRequest {
                database_path: Some(cursor_path),
                ..BackfillCursorSessionUsageRequest::default()
            },
            root.join("home"),
            root.join("managed"),
        )
        .unwrap_err();
        assert!(error.contains("composerHeaders.composerId is missing"));
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn cursor_skill_path_rejects_symlink_escape_from_allowed_root() {
        use std::os::unix::fs::symlink;

        let root = cursor_backfill_temp_dir("symlink-escape");
        let allowed_root = root.join("workspace/.cursor/skills");
        let outside_skill = root.join("outside/escaped-skill/SKILL.md");
        let linked_skill = allowed_root.join("escaped-skill");
        fs::create_dir_all(outside_skill.parent().unwrap()).unwrap();
        fs::create_dir_all(&allowed_root).unwrap();
        fs::write(&outside_skill, "---\nname: escaped-skill\n---\n").unwrap();
        symlink(outside_skill.parent().unwrap(), &linked_skill).unwrap();

        let candidate = linked_skill.join("SKILL.md");
        let canonical = fs::canonicalize(&candidate).unwrap();
        assert!(!cursor_path_is_allowed(&canonical, &[allowed_root]));

        fs::remove_dir_all(root).unwrap();
    }
}
