use crate::*;
use std::io::{BufRead, BufReader, Read};

const MAX_CURSOR_TRANSCRIPT_ERRORS: usize = 20;
const MAX_CURSOR_TRANSCRIPT_FILES: usize = 100_000;
const MAX_CURSOR_TRANSCRIPT_DEPTH: usize = 4;
const MAX_CURSOR_TRANSCRIPT_FILE_BYTES: u64 = 32 * 1024 * 1024;
const MAX_CURSOR_TRANSCRIPT_LINE_BYTES: usize = 2 * 1024 * 1024;
const MAX_CURSOR_TRANSCRIPT_CANDIDATES_PER_FILE: usize = 4_096;
const MAX_CURSOR_TOOL_PATH_BYTES: usize = 16 * 1024;
const MAX_CURSOR_SKILL_MD_BYTES: u64 = 1024 * 1024;

#[derive(Debug)]
struct CursorTranscriptFile {
    path: PathBuf,
    transcript_id: String,
}

#[derive(Debug)]
struct CursorTranscriptReadCandidate {
    transcript_id: String,
    line_index: usize,
    turn_key: String,
    raw_path: String,
    used_at: String,
}

#[derive(Debug)]
struct ValidatedCursorTranscriptSkill {
    name: String,
    evidence_path: PathBuf,
    historical_missing: bool,
}

#[derive(Debug)]
struct CursorTranscriptExtraction {
    content_hash: String,
    candidates: Vec<CursorTranscriptReadCandidate>,
    read_candidates: usize,
    read_file_candidates: usize,
}

#[derive(Default)]
struct CursorTranscriptParseState {
    current_user_turn: Option<usize>,
    next_user_turn: usize,
    read_candidates: usize,
    read_file_candidates: usize,
    candidates: Vec<CursorTranscriptReadCandidate>,
}

enum BoundedLine {
    Eof,
    Line(Vec<u8>),
    Oversized,
}

pub(crate) fn backfill_cursor_agent_transcript_usage(
    projects_root: &Path,
    allowed_skill_root: &Path,
    runtime_roots: &[PathBuf],
    managed_database: &mut Connection,
) -> Result<BackfillCodexSessionUsageResult> {
    let projects_root = canonical_directory(projects_root, "Cursor projects root")?;
    let allowed_skill_root_lexical =
        normalize_lexical_path(&expand_home(allowed_skill_root.to_path_buf()));
    let allowed_skill_root = canonical_directory(allowed_skill_root, "Allowed skill root")?;
    let mut result = BackfillCodexSessionUsageResult::default();
    let mut files = Vec::new();
    collect_cursor_transcript_files(&projects_root, &mut files, &mut result)?;
    files.sort_by(|left, right| left.path.cmp(&right.path));

    let mut seen_transcript_hashes = HashSet::new();
    let mut seen_event_ids = HashSet::new();
    for transcript in files {
        result.scanned_files = result.scanned_files.saturating_add(1);
        result.scanned_cursor_transcript_files =
            result.scanned_cursor_transcript_files.saturating_add(1);
        let extraction = match extract_cursor_transcript_read_candidates(&transcript) {
            Ok(extraction) => extraction,
            Err(error) => {
                result.skipped = result.skipped.saturating_add(1);
                push_cursor_transcript_error(
                    &mut result.errors,
                    format!("Cursor transcript {}: {error}", transcript.transcript_id),
                );
                continue;
            }
        };
        result.cursor_transcript_read_candidates = result
            .cursor_transcript_read_candidates
            .saturating_add(extraction.read_candidates);
        result.cursor_transcript_read_file_candidates = result
            .cursor_transcript_read_file_candidates
            .saturating_add(extraction.read_file_candidates);
        if !seen_transcript_hashes.insert(extraction.content_hash) {
            result.cursor_transcript_duplicate_files =
                result.cursor_transcript_duplicate_files.saturating_add(1);
            result.deduplicated = result
                .deduplicated
                .saturating_add(extraction.candidates.len());
            continue;
        }

        let mut seen_file_event_ids = HashSet::new();
        for candidate in extraction.candidates {
            let skill = match validate_cursor_transcript_skill_path(
                &candidate.raw_path,
                &allowed_skill_root_lexical,
                &allowed_skill_root,
            ) {
                Ok(skill) => skill,
                Err(error) => {
                    result.skipped = result.skipped.saturating_add(1);
                    result.cursor_transcript_unsafe_rejected =
                        result.cursor_transcript_unsafe_rejected.saturating_add(1);
                    push_cursor_transcript_error(
                        &mut result.errors,
                        format!(
                            "Cursor transcript {} line {}: {error}",
                            candidate.transcript_id,
                            candidate.line_index.saturating_add(1)
                        ),
                    );
                    continue;
                }
            };
            let event_id = cursor_transcript_event_id(&candidate, &skill.evidence_path);
            if !seen_file_event_ids.insert(event_id.clone()) {
                result.deduplicated = result.deduplicated.saturating_add(1);
                result.cursor_transcript_turn_duplicates =
                    result.cursor_transcript_turn_duplicates.saturating_add(1);
                continue;
            }
            if !seen_event_ids.insert(event_id.clone()) {
                result.deduplicated = result.deduplicated.saturating_add(1);
                continue;
            }

            result.discovered = result.discovered.saturating_add(1);
            result.inferred_cursor_transcript_calls =
                result.inferred_cursor_transcript_calls.saturating_add(1);
            if skill.historical_missing {
                result.cursor_transcript_historical_missing = result
                    .cursor_transcript_historical_missing
                    .saturating_add(1);
            }
            match record_cursor_transcript_read(
                &candidate,
                &skill,
                event_id,
                runtime_roots,
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
                    push_cursor_transcript_error(
                        &mut result.errors,
                        format!("Cursor transcript read for {}: {error}", skill.name),
                    );
                }
            }
        }
    }

    Ok(result)
}

pub(crate) fn merge_cursor_agent_transcript_backfill_result(
    target: &mut BackfillCodexSessionUsageResult,
    transcript: BackfillCodexSessionUsageResult,
) {
    target.scanned_files = target
        .scanned_files
        .saturating_add(transcript.scanned_files);
    target.discovered = target.discovered.saturating_add(transcript.discovered);
    target.recorded = target.recorded.saturating_add(transcript.recorded);
    target.deduplicated = target.deduplicated.saturating_add(transcript.deduplicated);
    target.upgraded = target.upgraded.saturating_add(transcript.upgraded);
    target.skipped = target.skipped.saturating_add(transcript.skipped);
    target.scanned_cursor_transcript_files = target
        .scanned_cursor_transcript_files
        .saturating_add(transcript.scanned_cursor_transcript_files);
    target.inferred_cursor_transcript_calls = target
        .inferred_cursor_transcript_calls
        .saturating_add(transcript.inferred_cursor_transcript_calls);
    target.cursor_transcript_read_candidates = target
        .cursor_transcript_read_candidates
        .saturating_add(transcript.cursor_transcript_read_candidates);
    target.cursor_transcript_read_file_candidates = target
        .cursor_transcript_read_file_candidates
        .saturating_add(transcript.cursor_transcript_read_file_candidates);
    target.cursor_transcript_turn_duplicates = target
        .cursor_transcript_turn_duplicates
        .saturating_add(transcript.cursor_transcript_turn_duplicates);
    target.cursor_transcript_duplicate_files = target
        .cursor_transcript_duplicate_files
        .saturating_add(transcript.cursor_transcript_duplicate_files);
    target.cursor_transcript_historical_missing = target
        .cursor_transcript_historical_missing
        .saturating_add(transcript.cursor_transcript_historical_missing);
    target.cursor_transcript_unsafe_rejected = target
        .cursor_transcript_unsafe_rejected
        .saturating_add(transcript.cursor_transcript_unsafe_rejected);
    for error in transcript.errors {
        push_cursor_transcript_error(&mut target.errors, error);
    }
}

fn canonical_directory(path: &Path, label: &str) -> Result<PathBuf> {
    let metadata =
        fs::symlink_metadata(path).map_err(|error| format!("{label} is unavailable: {error}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(format!("{label} must be a real directory."));
    }
    fs::canonicalize(path).map_err(|error| format!("Unable to resolve {label}: {error}"))
}

fn collect_cursor_transcript_files(
    projects_root: &Path,
    files: &mut Vec<CursorTranscriptFile>,
    result: &mut BackfillCodexSessionUsageResult,
) -> Result<()> {
    let projects = fs::read_dir(projects_root)
        .map_err(|error| format!("Unable to read Cursor projects root: {error}"))?;
    for project in projects {
        let project = match project {
            Ok(project) => project,
            Err(error) => {
                result.skipped = result.skipped.saturating_add(1);
                push_cursor_transcript_error(
                    &mut result.errors,
                    format!("Unable to inspect a Cursor project entry: {error}"),
                );
                continue;
            }
        };
        let project_path = project.path();
        let metadata = match fs::symlink_metadata(&project_path) {
            Ok(metadata) => metadata,
            Err(error) => {
                result.skipped = result.skipped.saturating_add(1);
                push_cursor_transcript_error(
                    &mut result.errors,
                    format!("Unable to inspect a Cursor project: {error}"),
                );
                continue;
            }
        };
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            continue;
        }
        let transcripts_root = project_path.join("agent-transcripts");
        let Ok(metadata) = fs::symlink_metadata(&transcripts_root) else {
            continue;
        };
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            continue;
        }
        collect_cursor_transcript_files_under(&transcripts_root, 0, files, result)?;
        if files.len() >= MAX_CURSOR_TRANSCRIPT_FILES {
            push_cursor_transcript_error(
                &mut result.errors,
                format!(
                    "Stopped after the {MAX_CURSOR_TRANSCRIPT_FILES}-file Cursor transcript limit."
                ),
            );
            break;
        }
    }
    Ok(())
}

fn collect_cursor_transcript_files_under(
    current: &Path,
    depth: usize,
    files: &mut Vec<CursorTranscriptFile>,
    result: &mut BackfillCodexSessionUsageResult,
) -> Result<()> {
    if depth > MAX_CURSOR_TRANSCRIPT_DEPTH || files.len() >= MAX_CURSOR_TRANSCRIPT_FILES {
        return Ok(());
    }
    let entries = match fs::read_dir(current) {
        Ok(entries) => entries,
        Err(error) => {
            result.skipped = result.skipped.saturating_add(1);
            push_cursor_transcript_error(
                &mut result.errors,
                format!("Unable to read a Cursor transcript directory: {error}"),
            );
            return Ok(());
        }
    };
    for entry in entries {
        if files.len() >= MAX_CURSOR_TRANSCRIPT_FILES {
            break;
        }
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                result.skipped = result.skipped.saturating_add(1);
                push_cursor_transcript_error(
                    &mut result.errors,
                    format!("Unable to inspect a Cursor transcript entry: {error}"),
                );
                continue;
            }
        };
        let path = entry.path();
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) => {
                result.skipped = result.skipped.saturating_add(1);
                push_cursor_transcript_error(
                    &mut result.errors,
                    format!("Unable to inspect a Cursor transcript path: {error}"),
                );
                continue;
            }
        };
        if metadata.file_type().is_symlink() {
            continue;
        }
        if metadata.is_dir() {
            collect_cursor_transcript_files_under(&path, depth.saturating_add(1), files, result)?;
            continue;
        }
        if !metadata.is_file()
            || path.extension().and_then(|extension| extension.to_str()) != Some("jsonl")
        {
            continue;
        }
        let Some(transcript_id) = path
            .file_stem()
            .and_then(|value| value.to_str())
            .map(str::to_string)
        else {
            result.skipped = result.skipped.saturating_add(1);
            continue;
        };
        if !valid_cursor_transcript_id(&transcript_id) {
            result.skipped = result.skipped.saturating_add(1);
            continue;
        }
        files.push(CursorTranscriptFile {
            path,
            transcript_id,
        });
    }
    Ok(())
}

fn valid_cursor_transcript_id(value: &str) -> bool {
    !value.is_empty()
        && value.chars().count() <= 256
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
}

fn extract_cursor_transcript_read_candidates(
    transcript: &CursorTranscriptFile,
) -> Result<CursorTranscriptExtraction> {
    let file = fs::File::open(&transcript.path)
        .map_err(|error| format!("Unable to open transcript: {error}"))?;
    let metadata = file
        .metadata()
        .map_err(|error| format!("Unable to inspect transcript: {error}"))?;
    if !metadata.is_file() {
        return Err("Transcript is not a regular file.".to_string());
    }
    if metadata.len() > MAX_CURSOR_TRANSCRIPT_FILE_BYTES {
        return Err(format!(
            "Transcript exceeds the {MAX_CURSOR_TRANSCRIPT_FILE_BYTES}-byte safety limit."
        ));
    }
    let modified = metadata
        .modified()
        .map_err(|error| format!("Transcript modification time is unavailable: {error}"))?;
    let used_at = DateTime::<Utc>::from(modified).to_rfc3339_opts(SecondsFormat::Secs, false);
    let mut reader = BufReader::new(file).take(MAX_CURSOR_TRANSCRIPT_FILE_BYTES.saturating_add(1));
    let mut line_index = 0usize;
    let mut parse_state = CursorTranscriptParseState::default();
    let mut content_hasher = Sha256::new();

    loop {
        match read_bounded_line(&mut reader, MAX_CURSOR_TRANSCRIPT_LINE_BYTES)
            .map_err(|error| format!("Unable to read transcript: {error}"))?
        {
            BoundedLine::Eof => break,
            BoundedLine::Oversized => {
                return Err(format!(
                    "Transcript line {} exceeds the {MAX_CURSOR_TRANSCRIPT_LINE_BYTES}-byte safety limit.",
                    line_index.saturating_add(1)
                ));
            }
            BoundedLine::Line(line) => {
                content_hasher.update(&line);
                if !line.iter().all(u8::is_ascii_whitespace) {
                    let value =
                        serde_json::from_slice::<serde_json::Value>(&line).map_err(|_| {
                            format!(
                                "Transcript line {} contains invalid JSON.",
                                line_index.saturating_add(1)
                            )
                        })?;
                    collect_cursor_transcript_candidates_from_value(
                        &value,
                        transcript,
                        line_index,
                        &used_at,
                        &mut parse_state,
                    )?;
                    if parse_state.candidates.len() > MAX_CURSOR_TRANSCRIPT_CANDIDATES_PER_FILE {
                        return Err(format!(
                            "Transcript exceeds the {MAX_CURSOR_TRANSCRIPT_CANDIDATES_PER_FILE}-candidate safety limit."
                        ));
                    }
                }
                line_index = line_index.saturating_add(1);
            }
        }
    }
    if reader.limit() == 0 {
        return Err(format!(
            "Transcript exceeds the {MAX_CURSOR_TRANSCRIPT_FILE_BYTES}-byte safety limit."
        ));
    }
    let content_hash = content_hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    Ok(CursorTranscriptExtraction {
        content_hash,
        candidates: parse_state.candidates,
        read_candidates: parse_state.read_candidates,
        read_file_candidates: parse_state.read_file_candidates,
    })
}

fn read_bounded_line(reader: &mut impl BufRead, max_bytes: usize) -> std::io::Result<BoundedLine> {
    let mut line = Vec::new();
    let mut oversized = false;
    let mut read_any = false;
    loop {
        let available = reader.fill_buf()?;
        if available.is_empty() {
            return Ok(if !read_any {
                BoundedLine::Eof
            } else if oversized {
                BoundedLine::Oversized
            } else {
                BoundedLine::Line(line)
            });
        }
        read_any = true;
        let end = available
            .iter()
            .position(|byte| *byte == b'\n')
            .map(|index| index.saturating_add(1))
            .unwrap_or(available.len());
        if !oversized {
            if line.len().saturating_add(end) > max_bytes {
                oversized = true;
                line.clear();
            } else {
                line.extend_from_slice(&available[..end]);
            }
        }
        let ended = available.get(end.saturating_sub(1)) == Some(&b'\n');
        reader.consume(end);
        if ended {
            return Ok(if oversized {
                BoundedLine::Oversized
            } else {
                BoundedLine::Line(line)
            });
        }
    }
}

fn collect_cursor_transcript_candidates_from_value(
    value: &serde_json::Value,
    transcript: &CursorTranscriptFile,
    line_index: usize,
    used_at: &str,
    state: &mut CursorTranscriptParseState,
) -> Result<()> {
    let role = value.get("role").and_then(|value| value.as_str());
    if role == Some("user") {
        state.current_user_turn = Some(state.next_user_turn);
        state.next_user_turn = state.next_user_turn.saturating_add(1);
        return Ok(());
    }
    if role != Some("assistant") {
        return Ok(());
    }
    let Some(content) = value
        .get("message")
        .and_then(|message| message.get("content"))
        .and_then(|content| content.as_array())
    else {
        return Ok(());
    };
    for block in content {
        if block.get("type").and_then(|value| value.as_str()) != Some("tool_use") {
            continue;
        }
        let Some(tool_name) = block.get("name").and_then(|value| value.as_str()) else {
            continue;
        };
        if !matches!(tool_name, "Read" | "ReadFile") {
            continue;
        }
        let Some(raw_path) = block
            .get("input")
            .and_then(|input| input.get("path"))
            .and_then(|value| value.as_str())
        else {
            continue;
        };
        if raw_path.len() > MAX_CURSOR_TOOL_PATH_BYTES {
            return Err(format!(
                "Transcript line {} contains an oversized tool path.",
                line_index.saturating_add(1)
            ));
        }
        if Path::new(raw_path)
            .file_name()
            .and_then(|value| value.to_str())
            != Some("SKILL.md")
        {
            continue;
        }
        if tool_name == "ReadFile" {
            state.read_file_candidates = state.read_file_candidates.saturating_add(1);
            continue;
        }
        state.read_candidates = state.read_candidates.saturating_add(1);
        state.candidates.push(CursorTranscriptReadCandidate {
            transcript_id: transcript.transcript_id.clone(),
            line_index,
            turn_key: state
                .current_user_turn
                .map(|turn| format!("user-{turn}"))
                .unwrap_or_else(|| "unattributed".to_string()),
            raw_path: raw_path.to_string(),
            used_at: used_at.to_string(),
        });
    }
    Ok(())
}

fn validate_cursor_transcript_skill_path(
    raw_path: &str,
    allowed_skill_root_lexical: &Path,
    allowed_skill_root: &Path,
) -> Result<ValidatedCursorTranscriptSkill> {
    let expanded = expand_home(PathBuf::from(raw_path));
    if !expanded.is_absolute() {
        return Err("Skill path must be absolute.".to_string());
    }
    if expanded
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err("Skill path cannot contain parent traversal.".to_string());
    }
    if expanded.file_name().and_then(|value| value.to_str()) != Some("SKILL.md") {
        return Err("Tool path must end with SKILL.md.".to_string());
    }
    let lexical = normalize_lexical_path(&expanded);
    let parent_name = lexical
        .parent()
        .and_then(Path::file_name)
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_string();
    validate_skill_name(&parent_name)?;

    let metadata = match fs::symlink_metadata(&lexical) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let evidence_path = resolve_missing_cursor_skill_path(
                &lexical,
                allowed_skill_root_lexical,
                allowed_skill_root,
            )?;
            return Ok(ValidatedCursorTranscriptSkill {
                name: parent_name,
                evidence_path,
                historical_missing: true,
            });
        }
        Err(_) => return Err("SKILL.md is unreadable.".to_string()),
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("SKILL.md must be a regular file, not a symlink or directory.".to_string());
    }
    if metadata.len() > MAX_CURSOR_SKILL_MD_BYTES {
        return Err(format!(
            "SKILL.md exceeds the {MAX_CURSOR_SKILL_MD_BYTES}-byte safety limit."
        ));
    }
    let canonical =
        fs::canonicalize(&lexical).map_err(|_| "SKILL.md is missing or unreadable.".to_string())?;
    if !canonical.starts_with(allowed_skill_root) {
        return Err("SKILL.md resolves outside the allowed skill root.".to_string());
    }

    let mut file = fs::File::open(&canonical).map_err(|_| "SKILL.md is unreadable.".to_string())?;
    let mut bytes = Vec::new();
    file.by_ref()
        .take(MAX_CURSOR_SKILL_MD_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_| "SKILL.md is unreadable.".to_string())?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_CURSOR_SKILL_MD_BYTES {
        return Err(format!(
            "SKILL.md exceeds the {MAX_CURSOR_SKILL_MD_BYTES}-byte safety limit."
        ));
    }
    let content =
        String::from_utf8(bytes).map_err(|_| "SKILL.md must contain valid UTF-8.".to_string())?;
    let document = parse_skill_frontmatter_document(&content)?;
    let name = if document.metadata.name.trim().is_empty() {
        canonical
            .parent()
            .and_then(Path::file_name)
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_string()
    } else {
        document.metadata.name.trim().to_string()
    };
    validate_skill_name(&name)?;
    Ok(ValidatedCursorTranscriptSkill {
        name,
        evidence_path: canonical,
        historical_missing: false,
    })
}

fn resolve_missing_cursor_skill_path(
    missing_skill_path: &Path,
    allowed_skill_root_lexical: &Path,
    allowed_skill_root: &Path,
) -> Result<PathBuf> {
    if !missing_skill_path.starts_with(allowed_skill_root_lexical) {
        return Err("Historical SKILL.md path is outside the allowed skill root.".to_string());
    }

    let mut ancestor = missing_skill_path.parent();
    while let Some(path) = ancestor {
        match fs::symlink_metadata(path) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() {
                    if let Ok(canonical) = fs::canonicalize(path) {
                        if !canonical.starts_with(allowed_skill_root) {
                            return Err(
                                "Historical SKILL.md ancestor resolves outside the allowed skill root."
                                    .to_string(),
                            );
                        }
                    }
                    ancestor = path.parent();
                    continue;
                }
                if !metadata.is_dir() {
                    return Err(
                        "Missing historical SKILL.md has a non-directory ancestor.".to_string()
                    );
                }
                let canonical = fs::canonicalize(path)
                    .map_err(|_| "Historical SKILL.md ancestor is unreadable.".to_string())?;
                if !canonical.starts_with(allowed_skill_root) {
                    return Err(
                        "Historical SKILL.md ancestor resolves outside the allowed skill root."
                            .to_string(),
                    );
                }
                let suffix = missing_skill_path
                    .strip_prefix(path)
                    .map_err(|_| "Unable to normalize historical SKILL.md path.".to_string())?;
                return Ok(normalize_lexical_path(&canonical.join(suffix)));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                ancestor = path.parent();
            }
            Err(_) => return Err("Historical SKILL.md ancestor is unreadable.".to_string()),
        }
    }
    Err("Historical SKILL.md has no readable local ancestor.".to_string())
}

fn cursor_transcript_event_id(
    candidate: &CursorTranscriptReadCandidate,
    evidence_skill_path: &Path,
) -> String {
    let path_hash = sha256(&evidence_skill_path.to_string_lossy());
    format!(
        "cursor-agent-transcript:{}:{}:{}",
        candidate.transcript_id, candidate.turn_key, path_hash
    )
}

fn record_cursor_transcript_read(
    candidate: &CursorTranscriptReadCandidate,
    skill: &ValidatedCursorTranscriptSkill,
    event_id: String,
    runtime_roots: &[PathBuf],
    managed_database: &mut Connection,
) -> Result<SkillUsageRecordResult> {
    let (runtime_root, _) = infer_usage_runtime_from_skill_path_with_roots(
        &skill.evidence_path,
        "cursor",
        Some(runtime_roots),
        None,
    )?;
    let source_kind = if skill
        .evidence_path
        .components()
        .any(|component| component.as_os_str() == ".system")
    {
        "system"
    } else {
        "regular"
    };
    record_skill_usage_on_connection(
        RecordSkillUsageRequest {
            skill_name: skill.name.clone(),
            agent_id: "cursor".to_string(),
            runtime_root,
            event_id: Some(event_id),
            used_at: Some(candidate.used_at.clone()),
            prompt_excerpt: None,
            metadata: Some(serde_json::json!({
                "source": "cursor_agent_transcript_read",
                "tool": "Read",
                "historical_missing": skill.historical_missing,
                "skill_source_kind": source_kind
            })),
        },
        managed_database,
        true,
    )
}

fn push_cursor_transcript_error(errors: &mut Vec<String>, message: String) {
    if errors.len() < MAX_CURSOR_TRANSCRIPT_ERRORS {
        errors.push(message);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn transcript_temp_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        let root = std::env::temp_dir().join(format!(
            "skillbox-cursor-transcripts-{label}-{}-{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn write_skill(path: &Path, name: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            path,
            format!("---\nname: {name}\ndescription: test\n---\n# Test\n"),
        )
        .unwrap();
    }

    fn write_transcript(path: &Path, values: &[serde_json::Value]) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut file = fs::File::create(path).unwrap();
        for value in values {
            writeln!(file, "{}", serde_json::to_string(value).unwrap()).unwrap();
        }
    }

    fn transcript_row(role: &str, content: Vec<serde_json::Value>) -> serde_json::Value {
        serde_json::json!({
            "role": role,
            "message": {"content": content}
        })
    }

    fn tool(name: &str, path: impl AsRef<Path>) -> serde_json::Value {
        serde_json::json!({
            "type": "tool_use",
            "name": name,
            "input": {"path": path.as_ref()}
        })
    }

    fn transcript_path(projects: &Path, project: &str, id: &str) -> PathBuf {
        projects
            .join(project)
            .join("agent-transcripts")
            .join(id)
            .join(format!("{id}.jsonl"))
    }

    fn run_provider(
        root: &Path,
        projects: &Path,
    ) -> (BackfillCodexSessionUsageResult, ManagedPaths) {
        let managed = root.join("managed");
        let paths = ensure_managed_layout(&managed).unwrap();
        let mut connection = open_database(&paths.database_path).unwrap();
        let runtime_roots = runtime_roots_under(root);
        let result =
            backfill_cursor_agent_transcript_usage(projects, root, &runtime_roots, &mut connection)
                .unwrap();
        (result, paths)
    }

    #[test]
    fn cursor_transcripts_record_read_as_inferred_exclude_read_file_and_use_mtime() {
        let root = transcript_temp_dir("reads");
        let projects = root.join(".cursor/projects");
        fs::create_dir_all(&projects).unwrap();
        let first_skill = root.join("skills-cursor/first/SKILL.md");
        let second_skill = root.join("skills-cursor/second/SKILL.md");
        write_skill(&first_skill, "first");
        write_skill(&second_skill, "second");
        let transcript = transcript_path(
            &projects,
            "project-one",
            "11111111-1111-1111-1111-111111111111",
        );
        write_transcript(
            &transcript,
            &[
                transcript_row("user", vec![]),
                transcript_row(
                    "assistant",
                    vec![tool("Read", &first_skill), tool("ReadFile", &second_skill)],
                ),
            ],
        );
        let expected_used_at =
            DateTime::<Utc>::from(fs::metadata(&transcript).unwrap().modified().unwrap())
                .to_rfc3339_opts(SecondsFormat::Secs, false);

        let (result, paths) = run_provider(&root, &projects);
        assert_eq!(result.scanned_cursor_transcript_files, 1);
        assert_eq!(result.cursor_transcript_read_candidates, 1);
        assert_eq!(result.cursor_transcript_read_file_candidates, 1);
        assert_eq!(
            result.inferred_cursor_transcript_calls, 1,
            "{:?}",
            result.errors
        );
        assert_eq!(result.recorded, 1);
        assert!(result.errors.is_empty(), "{:?}", result.errors);

        let connection = open_database(&paths.database_path).unwrap();
        let rows: Vec<(String, String, String, Option<String>)> = {
            let mut statement = connection
                .prepare(
                    "
                    SELECT evidence_class, used_at, metadata_json, prompt_excerpt
                    FROM skill_usage_events
                    ORDER BY skill_name
                    ",
                )
                .unwrap();
            statement
                .query_map([], |row| {
                    Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
                })
                .unwrap()
                .collect::<std::result::Result<Vec<_>, _>>()
                .unwrap()
        };
        assert_eq!(rows.len(), 1);
        assert!(rows.iter().all(|row| row.0 == "inferred"));
        assert!(rows.iter().all(|row| row.1 == expected_used_at));
        assert!(rows.iter().all(|row| row
            .2
            .contains("\"source\":\"cursor_agent_transcript_read\"")));
        assert!(rows.iter().all(|row| row.3.is_none()));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn cursor_transcripts_ignore_non_read_tools_prose_and_user_messages() {
        let root = transcript_temp_dir("ignore");
        let projects = root.join(".cursor/projects");
        fs::create_dir_all(&projects).unwrap();
        let skill = root.join("skills-cursor/demo/SKILL.md");
        write_skill(&skill, "demo");
        write_transcript(
            &transcript_path(
                &projects,
                "project-one",
                "22222222-2222-2222-2222-222222222222",
            ),
            &[
                transcript_row(
                    "user",
                    vec![serde_json::json!({
                        "type": "text",
                        "text": skill.to_string_lossy()
                    })],
                ),
                transcript_row(
                    "assistant",
                    vec![
                        serde_json::json!({
                            "type": "text",
                            "text": format!("Read {}", skill.display())
                        }),
                        serde_json::json!({
                            "type": "tool_use",
                            "name": "Shell",
                            "input": {"command": format!("cat {}", skill.display())}
                        }),
                        serde_json::json!({
                            "type": "tool_use",
                            "name": "Grep",
                            "input": {"pattern": "SKILL.md"}
                        }),
                    ],
                ),
            ],
        );

        let (result, _) = run_provider(&root, &projects);
        assert_eq!(result.discovered, 0);
        assert_eq!(result.recorded, 0);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn cursor_transcripts_dedupe_per_turn_and_identical_transcript_copies() {
        let root = transcript_temp_dir("dedupe");
        let projects = root.join(".cursor/projects");
        fs::create_dir_all(&projects).unwrap();
        let skill = root.join("skills-cursor/demo/SKILL.md");
        write_skill(&skill, "demo");
        let id = "33333333-3333-3333-3333-333333333333";
        let values = [
            transcript_row("user", vec![]),
            transcript_row(
                "assistant",
                vec![tool("Read", &skill), tool("Read", &skill)],
            ),
            transcript_row("user", vec![]),
            transcript_row("assistant", vec![tool("Read", &skill)]),
        ];
        write_transcript(&transcript_path(&projects, "project-one", id), &values);
        write_transcript(&transcript_path(&projects, "empty-window", id), &values);

        let (first, paths) = run_provider(&root, &projects);
        assert_eq!(first.scanned_cursor_transcript_files, 2);
        assert_eq!(first.cursor_transcript_read_candidates, 6);
        assert_eq!(first.inferred_cursor_transcript_calls, 2);
        assert_eq!(first.cursor_transcript_turn_duplicates, 1);
        assert_eq!(first.cursor_transcript_duplicate_files, 1);
        assert_eq!(first.recorded, 2);
        assert_eq!(first.deduplicated, 4);

        let mut connection = open_database(&paths.database_path).unwrap();
        let runtime_roots = runtime_roots_under(&root);
        let second = backfill_cursor_agent_transcript_usage(
            &projects,
            &root,
            &runtime_roots,
            &mut connection,
        )
        .unwrap();
        assert_eq!(second.recorded, 0);
        assert_eq!(second.inferred_cursor_transcript_calls, 2);
        assert_eq!(second.deduplicated, 6);
        let count: i64 = connection
            .query_row("SELECT COUNT(*) FROM skill_usage_events", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 2);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn cursor_transcripts_accept_safe_historical_missing_but_reject_unsafe_paths() {
        let root = transcript_temp_dir("paths");
        let projects = root.join(".cursor/projects");
        fs::create_dir_all(&projects).unwrap();
        let ordinary = root.join("notes.md");
        fs::write(&ordinary, "not a skill").unwrap();
        let historical_skill = root.join("missing/SKILL.md");
        write_skill(&historical_skill, "missing");
        fs::remove_file(&historical_skill).unwrap();
        write_transcript(
            &transcript_path(
                &projects,
                "project-one",
                "44444444-4444-4444-4444-444444444444",
            ),
            &[transcript_row(
                "assistant",
                vec![
                    tool("Read", "relative/SKILL.md"),
                    tool("Read", root.join("skills-cursor/demo/../escaped/SKILL.md")),
                    tool("Read", &historical_skill),
                    tool("ReadFile", ordinary),
                ],
            )],
        );

        let (result, paths) = run_provider(&root, &projects);
        assert_eq!(result.recorded, 1);
        assert_eq!(result.inferred_cursor_transcript_calls, 1);
        assert_eq!(result.cursor_transcript_historical_missing, 1);
        assert_eq!(result.cursor_transcript_unsafe_rejected, 2);
        assert_eq!(result.skipped, 2);
        assert_eq!(result.errors.len(), 2);
        let mut connection = open_database(&paths.database_path).unwrap();
        let metadata_json: String = connection
            .query_row("SELECT metadata_json FROM skill_usage_events", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert!(metadata_json.contains("\"historical_missing\":true"));
        let replay = backfill_cursor_agent_transcript_usage(
            &projects,
            &root,
            &runtime_roots_under(&root),
            &mut connection,
        )
        .unwrap();
        assert_eq!(replay.recorded, 0);
        assert_eq!(replay.deduplicated, 1);
        assert_eq!(replay.cursor_transcript_historical_missing, 1);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn cursor_transcripts_reject_missing_paths_outside_the_allowed_root() {
        let root = transcript_temp_dir("missing-outside");
        let allowed = root.join("home");
        fs::create_dir_all(&allowed).unwrap();
        let allowed_lexical = normalize_lexical_path(&allowed);
        let allowed = fs::canonicalize(allowed).unwrap();
        let outside = root.join("outside/demo/SKILL.md");
        let error = validate_cursor_transcript_skill_path(
            outside.to_str().unwrap(),
            &allowed_lexical,
            &allowed,
        )
        .unwrap_err();
        assert!(error.contains("outside the allowed skill root"));
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn cursor_transcripts_retain_broken_skill_directory_as_historical_evidence_only() {
        use std::os::unix::fs::symlink;

        let root = transcript_temp_dir("broken-skill-directory");
        let projects = root.join(".cursor/projects");
        let runtime_root = root.join("skills-cursor");
        fs::create_dir_all(&projects).unwrap();
        fs::create_dir_all(&runtime_root).unwrap();
        let broken_skill = runtime_root.join("frontend-design");
        symlink(root.join("removed/frontend-design"), &broken_skill).unwrap();
        let historical_skill = broken_skill.join("SKILL.md");
        write_transcript(
            &transcript_path(
                &projects,
                "project-one",
                "45454545-4545-4545-4545-454545454545",
            ),
            &[transcript_row(
                "assistant",
                vec![tool("Read", &historical_skill)],
            )],
        );

        let (result, paths) = run_provider(&root, &projects);
        assert_eq!(result.recorded, 1);
        assert_eq!(result.inferred_cursor_transcript_calls, 1);
        assert_eq!(result.cursor_transcript_historical_missing, 1);
        assert_eq!(result.cursor_transcript_unsafe_rejected, 0);

        let rankings = list_skill_usage_rankings(
            SkillUsageRankingRequest {
                range: SkillUsageRankingRange::AllTime,
                include_unmanaged: true,
                ..SkillUsageRankingRequest::default()
            },
            &paths.root,
        )
        .unwrap();
        let row = rankings
            .rows
            .iter()
            .find(|row| row.skill_name == "frontend-design")
            .expect("historical ranking row");
        assert_eq!(row.inferred_count, 1);
        assert!(row.source_missing);

        let error = preview_usage_skill_import_for_source(
            PreviewUsageSkillImportRequest {
                skill_name: row.skill_name.clone(),
                source_kind: Some(row.source_kind),
                source_id: Some(row.source_id.clone()),
                source_runtime_roots: row.source_runtime_roots.clone(),
                ranking_request: Some(SkillUsageRankingRequest {
                    range: SkillUsageRankingRange::AllTime,
                    include_unmanaged: true,
                    ..SkillUsageRankingRequest::default()
                }),
                ranking_generated_at: Some(rankings.generated_at.clone()),
                runtime_root: None,
            },
            &paths.root,
        )
        .unwrap_err();
        assert!(
            error.contains("Unable to locate") || error.contains("No recoverable local source"),
            "{error}"
        );

        let metadata_json: String = open_database(&paths.database_path)
            .unwrap()
            .query_row("SELECT metadata_json FROM skill_usage_events", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert!(metadata_json.contains("\"historical_missing\":true"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn cursor_transcripts_reject_historical_paths_without_a_safe_real_parent() {
        let root = transcript_temp_dir("no-safe-parent");
        let lexical_root = root.join("missing-home");
        let missing = lexical_root.join("skills/demo/SKILL.md");
        let error = resolve_missing_cursor_skill_path(&missing, &lexical_root, &root).unwrap_err();
        assert!(error.contains("outside the allowed skill root"));
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn cursor_transcripts_skip_source_symlinks_and_reject_skill_symlink_escape() {
        use std::os::unix::fs::symlink;

        let root = transcript_temp_dir("symlinks");
        let projects = root.join(".cursor/projects");
        fs::create_dir_all(&projects).unwrap();
        let outside = transcript_temp_dir("outside");
        let outside_skill = outside.join("escaped/SKILL.md");
        write_skill(&outside_skill, "escaped");
        let linked_skill = root.join("skills-cursor/escaped/SKILL.md");
        fs::create_dir_all(linked_skill.parent().unwrap()).unwrap();
        symlink(&outside_skill, &linked_skill).unwrap();
        let linked_parent = root.join("skills-cursor/linked-parent");
        symlink(outside.join("escaped"), &linked_parent).unwrap();
        let id = "55555555-5555-5555-5555-555555555555";
        write_transcript(
            &transcript_path(&projects, "project-one", id),
            &[transcript_row(
                "assistant",
                vec![
                    tool("Read", &linked_skill),
                    tool("Read", linked_parent.join("missing/SKILL.md")),
                ],
            )],
        );
        let outside_transcript = outside.join("transcript.jsonl");
        write_transcript(
            &outside_transcript,
            &[transcript_row(
                "assistant",
                vec![tool("Read", &outside_skill)],
            )],
        );
        symlink(
            &outside_transcript,
            projects
                .join("project-one/agent-transcripts")
                .join("linked-transcript.jsonl"),
        )
        .unwrap();

        let linked_project = projects.join("linked-project");
        symlink(projects.join("project-one"), &linked_project).unwrap();
        let (result, _) = run_provider(&root, &projects);
        assert_eq!(result.scanned_cursor_transcript_files, 1);
        assert_eq!(result.recorded, 0);
        assert_eq!(result.skipped, 2);
        assert_eq!(result.cursor_transcript_unsafe_rejected, 2);
        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(outside).unwrap();
    }

    #[test]
    fn cursor_transcripts_bound_malformed_oversized_and_invalid_skill_inputs() {
        let root = transcript_temp_dir("bounds");
        let projects = root.join(".cursor/projects");
        fs::create_dir_all(&projects).unwrap();
        let oversized_skill = root.join("skills-cursor/large/SKILL.md");
        fs::create_dir_all(oversized_skill.parent().unwrap()).unwrap();
        let file = fs::File::create(&oversized_skill).unwrap();
        file.set_len(MAX_CURSOR_SKILL_MD_BYTES.saturating_add(1))
            .unwrap();
        let malformed_skill = root.join("skills-cursor/malformed/SKILL.md");
        fs::create_dir_all(malformed_skill.parent().unwrap()).unwrap();
        fs::write(&malformed_skill, "---\nname: [broken\n---\n").unwrap();
        write_transcript(
            &transcript_path(
                &projects,
                "project-one",
                "66666666-6666-6666-6666-666666666666",
            ),
            &[transcript_row(
                "assistant",
                vec![
                    tool("Read", &oversized_skill),
                    tool("ReadFile", &malformed_skill),
                ],
            )],
        );
        let malformed_transcript = transcript_path(
            &projects,
            "project-two",
            "77777777-7777-7777-7777-777777777777",
        );
        fs::create_dir_all(malformed_transcript.parent().unwrap()).unwrap();
        fs::write(&malformed_transcript, b"{not-json}\n").unwrap();
        let oversized_line = transcript_path(
            &projects,
            "project-three",
            "88888888-8888-8888-8888-888888888888",
        );
        fs::create_dir_all(oversized_line.parent().unwrap()).unwrap();
        fs::write(
            &oversized_line,
            vec![b'x'; MAX_CURSOR_TRANSCRIPT_LINE_BYTES.saturating_add(1)],
        )
        .unwrap();
        let oversized_file = transcript_path(
            &projects,
            "project-four",
            "99999999-9999-9999-9999-999999999999",
        );
        fs::create_dir_all(oversized_file.parent().unwrap()).unwrap();
        let file = fs::File::create(&oversized_file).unwrap();
        file.set_len(MAX_CURSOR_TRANSCRIPT_FILE_BYTES.saturating_add(1))
            .unwrap();

        let (result, _) = run_provider(&root, &projects);
        assert_eq!(result.scanned_cursor_transcript_files, 4);
        assert_eq!(result.recorded, 0);
        assert_eq!(result.cursor_transcript_read_file_candidates, 1);
        assert!(result.skipped >= 4);
        assert!(result.errors.len() <= MAX_CURSOR_TRANSCRIPT_ERRORS);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn cursor_backfill_uses_transcripts_when_state_database_is_absent_and_replays_idempotently() {
        let root = transcript_temp_dir("integrated");
        let home = root.join("home");
        let managed = root.join("managed");
        let projects = home.join(".cursor/projects");
        fs::create_dir_all(&projects).unwrap();
        let skill = home.join("skills-cursor/demo/SKILL.md");
        write_skill(&skill, "demo");
        write_transcript(
            &transcript_path(
                &projects,
                "project-one",
                "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
            ),
            &[transcript_row("assistant", vec![tool("Read", &skill)])],
        );
        let request = BackfillCursorSessionUsageRequest {
            projects_root: Some(projects),
            ..BackfillCursorSessionUsageRequest::default()
        };

        let first =
            backfill_cursor_session_usage_for_home(request.clone(), &home, &managed).unwrap();
        assert_eq!(first.scanned_cursor_state_sessions, 0);
        assert_eq!(first.scanned_cursor_transcript_files, 1);
        assert_eq!(first.inferred_cursor_transcript_calls, 1);
        assert_eq!(first.recorded, 1);

        let second = backfill_cursor_session_usage_for_home(request, &home, &managed).unwrap();
        assert_eq!(second.recorded, 0);
        assert_eq!(second.deduplicated, 1);
        let audit = usage_audit(&managed).unwrap();
        assert_eq!(audit.confirmed_calls, 0);
        assert_eq!(audit.inferred_calls, 1);
        assert_eq!(audit.inferred_cursor_transcript_calls, 1);
        assert_eq!(audit.cursor_transcript_read_candidates, 1);
        assert_eq!(audit.history_references, 0);
        assert_eq!(audit.scanned_cursor_transcript_files, 1);
        fs::remove_dir_all(root).unwrap();
    }
}
