use crate::*;
use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader};

const MAX_BACKFILL_ERRORS: usize = 20;
const MAX_PARENT_DEPTH: usize = 64;
const MAX_CLAUDE_JSONL_LINE_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone)]
struct ClaudeTranscriptNode {
    parent_uuid: Option<String>,
    prompt_id: Option<String>,
    uuid: String,
}

#[derive(Debug, Clone)]
struct ClaudeSkillSignal {
    name: String,
    report_unresolved: bool,
    runtime_context: Option<PathBuf>,
    used_at: Option<String>,
    evidence_signal: &'static str,
    prompt_id: Option<String>,
    parent_uuid: Option<String>,
    uuid: Option<String>,
    fallback_index: usize,
}

#[derive(Debug, Clone)]
struct ClaudeSessionSkillCandidate {
    session_id: String,
    sidechain_id: String,
    turn_key: String,
    used_at: Option<String>,
    runtime_context: Option<PathBuf>,
    skill: HookSkillRef,
    evidence_signal: &'static str,
}

#[derive(Debug, Default)]
struct ClaudeSessionExtraction {
    candidates: Vec<ClaudeSessionSkillCandidate>,
    errors: Vec<String>,
    skipped: usize,
}

pub fn backfill_claude_code_session_usage(
    request: BackfillClaudeCodeSessionUsageRequest,
    managed_root: impl AsRef<Path>,
) -> Result<BackfillCodexSessionUsageResult> {
    backfill_claude_code_session_usage_with_progress(request, managed_root, |_| {})
}

pub fn backfill_claude_code_session_usage_with_progress<F>(
    request: BackfillClaudeCodeSessionUsageRequest,
    managed_root: impl AsRef<Path>,
    progress: F,
) -> Result<BackfillCodexSessionUsageResult>
where
    F: FnMut(UsageBackfillProgress),
{
    backfill_claude_code_session_usage_for_home_with_progress(
        request,
        home_dir(),
        managed_root,
        progress,
    )
}

#[cfg(test)]
pub(crate) fn backfill_claude_code_session_usage_for_home(
    request: BackfillClaudeCodeSessionUsageRequest,
    home: impl AsRef<Path>,
    managed_root: impl AsRef<Path>,
) -> Result<BackfillCodexSessionUsageResult> {
    backfill_claude_code_session_usage_for_home_with_progress(request, home, managed_root, |_| {})
}

pub(crate) fn backfill_claude_code_session_usage_for_home_with_progress<F>(
    request: BackfillClaudeCodeSessionUsageRequest,
    home: impl AsRef<Path>,
    managed_root: impl AsRef<Path>,
    mut progress: F,
) -> Result<BackfillCodexSessionUsageResult>
where
    F: FnMut(UsageBackfillProgress),
{
    let home = home.as_ref();
    let managed_root = managed_root.as_ref();
    let paths = ensure_managed_layout(managed_root.to_path_buf())?;
    let mut connection = open_database(&paths.database_path).map_err(|error| error.to_string())?;
    let runtime_roots = claude_runtime_roots(home);
    let projects_root = expand_home(
        request
            .projects_root
            .unwrap_or_else(|| home.join(".claude").join("projects")),
    );
    if !projects_root.is_absolute() {
        return Err("Claude Code projects root must be an absolute path.".to_string());
    }

    progress(UsageBackfillProgress::new(
        "claude-code",
        "collecting",
        0,
        None,
    ));
    let mut result = BackfillCodexSessionUsageResult::default();
    let mut files = Vec::new();
    if projects_root.is_dir() {
        collect_claude_jsonl_files(&projects_root, &mut files)?;
    }
    files.sort();
    let total = files.len();
    progress(UsageBackfillProgress::new(
        "claude-code",
        "scanning",
        0,
        Some(total),
    ));

    for (index, path) in files.into_iter().enumerate() {
        result.scanned_files = result.scanned_files.saturating_add(1);
        match extract_claude_session_skill_candidates(&path, &runtime_roots, home) {
            Ok(extraction) => {
                result.skipped = result.skipped.saturating_add(extraction.skipped);
                for error in extraction.errors {
                    push_backfill_error(&mut result.errors, format!("{}: {error}", path.display()));
                }
                for candidate in extraction.candidates {
                    result.discovered = result.discovered.saturating_add(1);
                    match record_claude_session_skill_candidate(
                        &candidate,
                        &mut connection,
                        &runtime_roots,
                    ) {
                        Ok(record) => {
                            if record.upgraded {
                                result.upgraded = result.upgraded.saturating_add(1);
                            } else if record.deduplicated {
                                result.deduplicated = result.deduplicated.saturating_add(1);
                            } else {
                                result.recorded = result.recorded.saturating_add(1);
                            }
                        }
                        Err(error) => {
                            result.skipped = result.skipped.saturating_add(1);
                            push_backfill_error(
                                &mut result.errors,
                                format!("{}: {error}", candidate.skill.name),
                            );
                        }
                    }
                }
            }
            Err(error) => {
                result.skipped = result.skipped.saturating_add(1);
                push_backfill_error(&mut result.errors, format!("{}: {error}", path.display()));
            }
        }
        progress(UsageBackfillProgress::new(
            "claude-code",
            "scanning",
            index + 1,
            Some(total),
        ));
    }
    progress(UsageBackfillProgress::new(
        "claude-code",
        "complete",
        result.scanned_files,
        Some(total),
    ));

    let scanned_files = u32::try_from(result.scanned_files).unwrap_or(u32::MAX);
    if let Err(error) = write_u32_preference(
        &paths.database_path,
        "claude_code_usage_backfill_scanned_files",
        scanned_files,
    ) {
        push_backfill_error(
            &mut result.errors,
            format!("Unable to persist Claude Code scan coverage: {error}"),
        );
    }
    if let Err(error) = persist_usage_backfill_audit(
        &paths.database_path,
        "claude_code_session_backfill",
        result.scanned_files,
        &result,
    ) {
        push_backfill_error(
            &mut result.errors,
            format!("Unable to persist Claude Code usage audit: {error}"),
        );
    }

    Ok(result)
}

fn record_claude_session_skill_candidate(
    candidate: &ClaudeSessionSkillCandidate,
    connection: &mut Connection,
    runtime_roots: &[PathBuf],
) -> Result<SkillUsageRecordResult> {
    let mut request = usage_request_from_skill_ref_with_roots(UsageRequestFromSkillRef {
        skill_ref: &candidate.skill,
        hook_agent: "claude-code",
        session_id: &candidate.session_id,
        turn_id: Some(&candidate.turn_key),
        index: 0,
        hook_event: "session_backfill",
        model: "",
        runtime_roots: Some(runtime_roots),
        preferred_runtime_context: candidate.runtime_context.as_deref(),
    })?;
    request.event_id = Some(format!(
        "claude-code-backfill:{}",
        &sha256(&format!(
            "{}\n{}\n{}\n{}",
            candidate.session_id,
            candidate.sidechain_id,
            candidate.turn_key,
            canonical_usage_skill_key(&candidate.skill.path)
        ))[..24]
    ));
    request.used_at = candidate.used_at.clone();
    request.prompt_excerpt = None;
    if let Some(metadata) = request
        .metadata
        .as_mut()
        .and_then(|value| value.as_object_mut())
    {
        metadata.insert(
            "source".to_string(),
            serde_json::Value::String("claude_code_session_backfill".to_string()),
        );
        metadata.insert(
            "provider".to_string(),
            serde_json::Value::String("claude-code".to_string()),
        );
        metadata.insert(
            "sidechain".to_string(),
            serde_json::Value::Bool(candidate.sidechain_id != "main"),
        );
        metadata.insert(
            "evidence_signal".to_string(),
            serde_json::Value::String(candidate.evidence_signal.to_string()),
        );
    }
    record_skill_usage_on_connection(request, connection, true)
}

fn extract_claude_session_skill_candidates(
    path: &Path,
    runtime_roots: &[PathBuf],
    home: &Path,
) -> Result<ClaudeSessionExtraction> {
    let file = fs::File::open(path).map_err(|error| format!("Unable to read session: {error}"))?;
    let reader = BufReader::new(file);
    let mut extraction = ClaudeSessionExtraction::default();
    let mut nodes = HashMap::new();
    let mut pending_signals = Vec::new();
    let mut session_id = None;
    let mut fallback_cwd = None;
    let mut index = 0usize;

    for line in reader.lines() {
        let line = line.map_err(|error| format!("Unable to read session line: {error}"))?;
        if line.trim().is_empty() {
            continue;
        }
        if line.len() > MAX_CLAUDE_JSONL_LINE_BYTES {
            extraction.skipped = extraction.skipped.saturating_add(1);
            push_backfill_error(
                &mut extraction.errors,
                format!("skipped oversized JSON line over {MAX_CLAUDE_JSONL_LINE_BYTES} bytes"),
            );
            continue;
        }
        let value = match serde_json::from_str::<serde_json::Value>(&line) {
            Ok(value) if value.is_object() => value,
            _ => {
                extraction.skipped = extraction.skipped.saturating_add(1);
                push_backfill_error(
                    &mut extraction.errors,
                    "skipped invalid JSON line".to_string(),
                );
                continue;
            }
        };
        if let Some(node) = claude_transcript_node(&value) {
            nodes.insert(node.uuid.clone(), node);
        }
        if session_id.is_none() {
            session_id = value
                .get("sessionId")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string);
        }
        if fallback_cwd.is_none() {
            fallback_cwd = claude_record_cwd(&value);
        }
        let used_at = value
            .get("timestamp")
            .and_then(|value| value.as_str())
            .map(str::to_string);
        let runtime_context = claude_record_cwd(&value).or_else(|| fallback_cwd.clone());
        let prompt_id = optional_trimmed_string(&value, "promptId");
        let parent_uuid = optional_trimmed_string(&value, "parentUuid");
        let uuid = optional_trimmed_string(&value, "uuid");
        match value.get("type").and_then(|value| value.as_str()) {
            Some("user") => {
                if let Some(text) = claude_user_record_text(&value) {
                    for (name, report_unresolved) in claude_command_skill_names(&text) {
                        pending_signals.push(ClaudeSkillSignal {
                            name,
                            report_unresolved,
                            runtime_context: runtime_context.clone(),
                            used_at: used_at.clone(),
                            evidence_signal: "native_skill_command",
                            prompt_id: prompt_id.clone(),
                            parent_uuid: parent_uuid.clone(),
                            uuid: uuid.clone(),
                            fallback_index: index,
                        });
                    }
                }
            }
            Some("assistant") => {
                for name in claude_skill_tool_names(&value) {
                    pending_signals.push(ClaudeSkillSignal {
                        name,
                        report_unresolved: true,
                        runtime_context: runtime_context.clone(),
                        used_at: used_at.clone(),
                        evidence_signal: "native_skill_tool",
                        prompt_id: prompt_id.clone(),
                        parent_uuid: parent_uuid.clone(),
                        uuid: uuid.clone(),
                        fallback_index: index,
                    });
                }
            }
            _ => {}
        }
        index = index.saturating_add(1);
    }

    let session_id = session_id.unwrap_or_else(|| claude_session_id_from_path(path));
    let sidechain_id = claude_sidechain_id(path);
    let mut seen_signals = HashSet::new();
    let mut seen = HashSet::new();
    for signal in pending_signals {
        let turn_key = claude_turn_key_from_parts(
            signal.prompt_id.as_deref(),
            signal.parent_uuid.as_deref(),
            signal.uuid.as_deref(),
            &nodes,
            signal.fallback_index,
        );
        if !seen_signals.insert(format!("{turn_key}\n{}", signal.name)) {
            continue;
        }
        let Some(skill_path) = resolve_claude_skill_path(
            &signal.name,
            signal.runtime_context.as_deref(),
            runtime_roots,
            home,
        ) else {
            if signal.report_unresolved {
                extraction.skipped = extraction.skipped.saturating_add(1);
                push_backfill_error(
                    &mut extraction.errors,
                    format!("unable to resolve invoked skill {}", signal.name),
                );
            }
            continue;
        };
        let skill_key = canonical_usage_skill_key(&skill_path);
        if !seen.insert(format!("{turn_key}\n{skill_key}")) {
            continue;
        }
        extraction.candidates.push(ClaudeSessionSkillCandidate {
            session_id: session_id.clone(),
            sidechain_id: sidechain_id.clone(),
            turn_key,
            used_at: signal.used_at,
            runtime_context: signal.runtime_context,
            skill: HookSkillRef {
                name: signal.name,
                path: skill_path,
                prompt_excerpt: None,
            },
            evidence_signal: signal.evidence_signal,
        });
    }

    extraction.candidates.sort_by(|left, right| {
        left.turn_key
            .cmp(&right.turn_key)
            .then_with(|| left.skill.name.cmp(&right.skill.name))
            .then_with(|| left.skill.path.cmp(&right.skill.path))
    });
    Ok(extraction)
}

fn optional_trimmed_string(value: &serde_json::Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn claude_transcript_node(value: &serde_json::Value) -> Option<ClaudeTranscriptNode> {
    let uuid = optional_trimmed_string(value, "uuid")?;
    Some(ClaudeTranscriptNode {
        parent_uuid: optional_trimmed_string(value, "parentUuid"),
        prompt_id: optional_trimmed_string(value, "promptId"),
        uuid,
    })
}

fn claude_turn_key_from_parts(
    prompt_id: Option<&str>,
    parent_uuid: Option<&str>,
    uuid: Option<&str>,
    nodes: &HashMap<String, ClaudeTranscriptNode>,
    fallback_index: usize,
) -> String {
    if let Some(prompt_id) = prompt_id.filter(|value| !value.is_empty()) {
        return format!("prompt:{prompt_id}");
    }

    let mut current = parent_uuid.map(str::to_string);
    let mut root_uuid = uuid.map(str::to_string);
    let mut visited = HashSet::new();
    for _ in 0..MAX_PARENT_DEPTH {
        let Some(next_uuid) = current else {
            break;
        };
        if !visited.insert(next_uuid.clone()) {
            break;
        }
        let Some(node) = nodes.get(&next_uuid) else {
            root_uuid = Some(next_uuid);
            break;
        };
        if let Some(prompt_id) = node.prompt_id.as_deref() {
            return format!("prompt:{prompt_id}");
        }
        root_uuid = Some(node.uuid.clone());
        current = node.parent_uuid.clone();
    }

    root_uuid
        .map(|uuid| format!("uuid:{uuid}"))
        .unwrap_or_else(|| format!("line:{fallback_index}"))
}

fn claude_user_record_text(value: &serde_json::Value) -> Option<String> {
    let message = value.get("message")?;
    if message.get("role").and_then(|value| value.as_str()) != Some("user") {
        return None;
    }
    let content = message.get("content")?;
    match content {
        serde_json::Value::String(text) => Some(text.to_string()),
        serde_json::Value::Array(blocks) => {
            let texts = blocks
                .iter()
                .filter(|block| block.get("type").and_then(|value| value.as_str()) == Some("text"))
                .filter_map(|block| block.get("text").and_then(|value| value.as_str()))
                .collect::<Vec<_>>();
            (!texts.is_empty()).then(|| texts.join("\n"))
        }
        _ => None,
    }
}

fn claude_command_skill_names(text: &str) -> Vec<(String, bool)> {
    if !text.trim_start().starts_with("<command-message>") {
        return Vec::new();
    }
    let Some(command_name) = xml_tag_text(text, "command-name") else {
        return Vec::new();
    };
    let Some(command_message) = xml_tag_text(text, "command-message") else {
        return Vec::new();
    };
    let skill_format = text.contains("<skill-format>true</skill-format>");
    let name = command_name.trim();
    let name = if skill_format {
        name.trim_start_matches('/')
    } else {
        let Some(name) = name.strip_prefix('/') else {
            return Vec::new();
        };
        name
    };
    if command_message.trim().trim_start_matches('/') != name {
        return Vec::new();
    }
    normalized_claude_skill_name(name)
        .map(|name| (name, skill_format))
        .into_iter()
        .collect()
}

fn claude_skill_tool_names(value: &serde_json::Value) -> Vec<String> {
    value
        .get("message")
        .and_then(|message| message.get("content"))
        .and_then(|content| content.as_array())
        .into_iter()
        .flatten()
        .filter(|block| {
            block.get("type").and_then(|value| value.as_str()) == Some("tool_use")
                && block
                    .get("name")
                    .and_then(|value| value.as_str())
                    .is_some_and(|name| name.eq_ignore_ascii_case("skill"))
        })
        .filter_map(|block| {
            let input = block.get("input")?;
            input
                .get("skill")
                .or_else(|| input.get("skill_name"))
                .or_else(|| input.get("name"))
                .and_then(|value| value.as_str())
                .and_then(normalized_claude_skill_name)
        })
        .collect()
}

fn normalized_claude_skill_name(value: &str) -> Option<String> {
    let name = value.trim();
    if name.is_empty()
        || !name.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
        })
        || validate_skill_name(name).is_err()
    {
        return None;
    }
    Some(name.to_string())
}

fn resolve_claude_skill_path(
    skill_name: &str,
    runtime_context: Option<&Path>,
    runtime_roots: &[PathBuf],
    home: &Path,
) -> Option<PathBuf> {
    let context = runtime_context.map(|path| expand_home(path.to_path_buf()));
    let mut preferred_roots = Vec::new();
    if let Some(context) = &context {
        preferred_roots.extend(claude_context_skill_roots(context));
    }
    preferred_roots.extend(claude_home_skill_roots(home));

    for root in &preferred_roots {
        if let Some(path) = existing_skill_md_path(root, skill_name) {
            return Some(path);
        }
    }

    let mut candidates = runtime_roots
        .iter()
        .filter_map(|root| existing_skill_md_path(root, skill_name))
        .collect::<Vec<_>>();
    candidates.sort();
    candidates.dedup_by(|left, right| {
        canonical_usage_skill_key(left) == canonical_usage_skill_key(right)
    });
    (candidates.len() == 1).then(|| candidates.remove(0))
}

fn claude_context_skill_roots(context: &Path) -> Vec<PathBuf> {
    vec![
        context.join(".claude/skills"),
        context.join(".agents/skills"),
        context.join(".codex/skills"),
        context.join(".cursor/skills"),
    ]
}

fn claude_home_skill_roots(home: &Path) -> Vec<PathBuf> {
    vec![
        home.join(".claude/skills"),
        home.join(".agents/skills"),
        home.join(".codex/skills"),
        home.join(".cursor/skills"),
        home.join(".skillbox/user-skills"),
        home.join(".skillbox/remote-skills"),
    ]
}

fn existing_skill_md_path(root: &Path, skill_name: &str) -> Option<PathBuf> {
    let path = root.join(skill_name).join("SKILL.md");
    path.is_file()
        .then(|| fs::canonicalize(&path).unwrap_or(path))
}

fn is_claude_runtime_root(root: &Path) -> bool {
    root.file_name().and_then(|value| value.to_str()) == Some("skills")
        && root
            .parent()
            .and_then(|path| path.file_name())
            .and_then(|value| value.to_str())
            == Some(".claude")
}

fn claude_runtime_roots(home: &Path) -> Vec<PathBuf> {
    runtime_roots_under(home)
        .into_iter()
        .filter(|root| is_claude_runtime_root(root))
        .collect()
}

fn canonical_usage_skill_key(path: &Path) -> String {
    fs::canonicalize(path)
        .unwrap_or_else(|_| expand_home(path.to_path_buf()))
        .to_string_lossy()
        .to_string()
}

fn claude_record_cwd(value: &serde_json::Value) -> Option<PathBuf> {
    value
        .get("cwd")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

fn claude_session_id_from_path(path: &Path) -> String {
    if path
        .parent()
        .and_then(|parent| parent.file_name())
        .and_then(|value| value.to_str())
        == Some("subagents")
    {
        return path
            .parent()
            .and_then(Path::parent)
            .and_then(|parent| parent.file_name())
            .and_then(|value| value.to_str())
            .unwrap_or("unknown-session")
            .to_string();
    }
    path.file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("unknown-session")
        .to_string()
}

fn claude_sidechain_id(path: &Path) -> String {
    if path
        .parent()
        .and_then(|parent| parent.file_name())
        .and_then(|value| value.to_str())
        == Some("subagents")
    {
        return format!(
            "sidechain:{}",
            path.file_stem()
                .and_then(|value| value.to_str())
                .unwrap_or("unknown")
        );
    }
    "main".to_string()
}

fn collect_claude_jsonl_files(root: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    let entries = fs::read_dir(root)
        .map_err(|error| format!("Unable to read {}: {error}", root.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("Unable to read {}: {error}", root.display()))?;
        let path = entry.path();
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(_) => continue,
        };
        if metadata.file_type().is_symlink() {
            continue;
        }
        if metadata.is_dir() {
            collect_claude_jsonl_files(&path, files)?;
            continue;
        }
        if path.extension().and_then(|value| value.to_str()) == Some("jsonl") {
            files.push(path);
        }
    }
    Ok(())
}

fn push_backfill_error(errors: &mut Vec<String>, message: String) {
    if errors.len() < MAX_BACKFILL_ERRORS {
        errors.push(message);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_skill(root: &Path, name: &str) {
        let skill = root.join(name);
        fs::create_dir_all(&skill).unwrap();
        fs::write(
            skill.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: Test skill\n---\n"),
        )
        .unwrap();
    }

    fn write_jsonl(path: &Path, values: &[serde_json::Value]) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        let content = values
            .iter()
            .map(serde_json::Value::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(path, format!("{content}\n")).unwrap();
    }

    #[test]
    fn claude_backfill_deduplicates_trusted_turn_signals_and_sidechains() {
        let root = std::env::temp_dir().join(format!(
            "skillbox-claude-backfill-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or(Duration::ZERO)
                .as_nanos()
        ));
        let home = root.join("home");
        let managed_root = root.join("SkillBox");
        let project = root.join("project");
        let projects_root = home.join(".claude/projects");
        write_skill(&home.join(".claude/skills"), "alpha");
        fs::create_dir_all(&project).unwrap();

        let main_session = projects_root.join("-project/session-1.jsonl");
        write_jsonl(
            &main_session,
            &[
                serde_json::json!({
                    "type": "user",
                    "uuid": "user-1",
                    "parentUuid": null,
                    "promptId": "prompt-1",
                    "sessionId": "session-1",
                    "timestamp": "2026-07-24T10:00:00Z",
                    "cwd": project,
                    "message": {
                        "role": "user",
                        "content": "<command-message>alpha</command-message>\n<command-name>/alpha</command-name>"
                    }
                }),
                serde_json::json!({
                    "type": "assistant",
                    "uuid": "assistant-1",
                    "parentUuid": "user-1",
                    "sessionId": "session-1",
                    "timestamp": "2026-07-24T10:00:01Z",
                    "cwd": project,
                    "message": {
                        "role": "assistant",
                        "content": [{
                            "type": "tool_use",
                            "name": "Skill",
                            "input": { "skill": "alpha" }
                        }]
                    }
                }),
                serde_json::json!({
                    "type": "user",
                    "uuid": "user-2",
                    "parentUuid": "assistant-1",
                    "promptId": "prompt-2",
                    "sessionId": "session-1",
                    "timestamp": "2026-07-24T11:00:00Z",
                    "cwd": project,
                    "message": {
                        "role": "user",
                        "content": "<command-message>model</command-message>\n<command-name>/model</command-name>"
                    }
                }),
                serde_json::json!({
                    "type": "assistant",
                    "uuid": "assistant-2",
                    "parentUuid": "user-2",
                    "sessionId": "session-1",
                    "timestamp": "2026-07-24T11:00:01Z",
                    "cwd": project,
                    "message": {
                        "role": "assistant",
                        "content": [{
                            "type": "tool_use",
                            "name": "Skill",
                            "input": { "skill": "alpha" }
                        }]
                    }
                }),
            ],
        );
        let sidechain = projects_root.join("-project/session-1/subagents/agent-one.jsonl");
        write_jsonl(
            &sidechain,
            &[serde_json::json!({
                "type": "assistant",
                "uuid": "assistant-sidechain",
                "parentUuid": null,
                "sessionId": "session-1",
                "timestamp": "2026-07-24T12:00:00Z",
                "cwd": project,
                "isSidechain": true,
                "message": {
                    "role": "assistant",
                    "content": [{
                        "type": "tool_use",
                        "name": "Skill",
                        "input": { "skill": "alpha" }
                    }]
                }
            })],
        );

        let request = BackfillClaudeCodeSessionUsageRequest {
            projects_root: Some(projects_root.clone()),
        };
        let first =
            backfill_claude_code_session_usage_for_home(request.clone(), &home, &managed_root)
                .unwrap();
        assert_eq!(first.scanned_files, 2);
        assert_eq!(first.discovered, 3);
        assert_eq!(first.recorded, 3);
        assert_eq!(first.deduplicated, 0);
        assert_eq!(first.skipped, 0);

        let second =
            backfill_claude_code_session_usage_for_home(request, &home, &managed_root).unwrap();
        assert_eq!(second.recorded, 0);
        assert_eq!(second.deduplicated, 3);

        let paths = ensure_managed_layout(&managed_root).unwrap();
        let connection = open_database(&paths.database_path).unwrap();
        let count: i64 = connection
            .query_row("SELECT COUNT(*) FROM skill_usage_events", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 3);
        let private_payloads: i64 = connection
            .query_row(
                "
                SELECT COUNT(*)
                FROM skill_usage_events
                WHERE prompt_excerpt IS NOT NULL
                   OR json_extract(metadata_json, '$.source') != 'claude_code_session_backfill'
                   OR agent_id != 'claude-code'
                ",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(private_payloads, 0);
        drop(connection);

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
        assert_eq!(rankings.coverage.claude_code_session_backfill_calls, 3);
        assert_eq!(rankings.coverage.cursor_session_backfill_calls, 0);
        assert_eq!(rankings.coverage.scanned_claude_code_session_files, 2);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn claude_backfill_resolves_skills_from_agents_runtime_when_claude_root_is_empty() {
        let root = std::env::temp_dir().join(format!(
            "skillbox-claude-agents-resolve-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or(Duration::ZERO)
                .as_nanos()
        ));
        let home = root.join("home");
        let managed_root = root.join("SkillBox");
        let project = root.join("project");
        let projects_root = home.join(".claude/projects");
        write_skill(&home.join(".agents/skills"), "nightwatch-video");
        fs::create_dir_all(home.join(".claude/skills")).unwrap();
        fs::create_dir_all(&project).unwrap();
        write_jsonl(
            &projects_root.join("-project/session-agents.jsonl"),
            &[serde_json::json!({
                "type": "assistant",
                "uuid": "assistant-1",
                "parentUuid": null,
                "promptId": "prompt-1",
                "sessionId": "session-agents",
                "timestamp": "2026-09-19T10:00:00Z",
                "cwd": project,
                "message": {
                    "role": "assistant",
                    "content": [{
                        "type": "tool_use",
                        "name": "skill",
                        "input": { "skill": "nightwatch-video" }
                    }]
                }
            })],
        );

        let result = backfill_claude_code_session_usage_for_home(
            BackfillClaudeCodeSessionUsageRequest {
                projects_root: Some(projects_root),
            },
            &home,
            &managed_root,
        )
        .unwrap();
        assert_eq!(result.discovered, 1, "{:?}", result.errors);
        assert_eq!(result.recorded, 1);
        assert_eq!(result.skipped, 0);

        let _ = fs::remove_dir_all(root);
    }
}
