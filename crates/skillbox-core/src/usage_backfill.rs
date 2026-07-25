use crate::*;
use std::io::{BufRead, BufReader};

const MAX_BACKFILL_ERRORS: usize = 20;

#[derive(Debug, Clone)]
struct CodexSessionSkillCandidate {
    session_id: String,
    turn_id: String,
    index: usize,
    used_at: Option<String>,
    runtime_context: Option<PathBuf>,
    skill: HookSkillRef,
}

pub fn backfill_codex_session_usage(
    request: BackfillCodexSessionUsageRequest,
    managed_root: impl AsRef<Path>,
) -> Result<BackfillCodexSessionUsageResult> {
    backfill_codex_session_usage_for_home(request, home_dir(), managed_root)
}

pub fn backfill_codex_session_usage_for_home(
    request: BackfillCodexSessionUsageRequest,
    home: impl AsRef<Path>,
    managed_root: impl AsRef<Path>,
) -> Result<BackfillCodexSessionUsageResult> {
    let home = home.as_ref();
    let managed_root = managed_root.as_ref();
    let paths = ensure_managed_layout(managed_root.to_path_buf())?;
    let mut connection = open_database(&paths.database_path).map_err(|error| error.to_string())?;
    let runtime_roots = runtime_roots_under(home);

    let mut roots = Vec::new();
    let sessions_root = request
        .sessions_root
        .clone()
        .unwrap_or_else(|| home.join(".codex").join("sessions"));
    if sessions_root.is_dir() {
        roots.push(sessions_root);
    }
    if request.include_archived {
        let archived_root = request
            .archived_sessions_root
            .clone()
            .unwrap_or_else(|| home.join(".codex").join("archived_sessions"));
        if archived_root.is_dir() {
            roots.push(archived_root);
        }
    }

    let mut result = BackfillCodexSessionUsageResult::default();
    let mut files = Vec::new();
    for root in &roots {
        collect_jsonl_files(root, &mut files)?;
    }
    files.sort();

    for path in files {
        result.scanned_files += 1;
        match extract_codex_session_skill_candidates(&path) {
            Ok((candidates, parse_errors)) => {
                if parse_errors > 0 {
                    result.skipped += parse_errors;
                    push_backfill_error(
                        &mut result.errors,
                        format!(
                            "{}: skipped {parse_errors} invalid JSON line{}",
                            path.display(),
                            if parse_errors == 1 { "" } else { "s" }
                        ),
                    );
                }
                for candidate in candidates {
                    result.discovered += 1;
                    match record_codex_session_skill_candidate(
                        &candidate,
                        &mut connection,
                        &runtime_roots,
                    ) {
                        Ok(record) => {
                            if record.deduplicated {
                                result.deduplicated += 1;
                            } else {
                                result.recorded += 1;
                            }
                        }
                        Err(error) => {
                            result.skipped += 1;
                            push_backfill_error(
                                &mut result.errors,
                                format!("{}: {error}", candidate.skill.name),
                            );
                        }
                    }
                }
            }
            Err(error) => {
                result.skipped += 1;
                push_backfill_error(&mut result.errors, format!("{}: {error}", path.display()));
            }
        }
    }

    let scanned_files = u32::try_from(result.scanned_files).unwrap_or(u32::MAX);
    if let Err(error) = write_u32_preference(
        &paths.database_path,
        "codex_usage_backfill_scanned_files",
        scanned_files,
    ) {
        push_backfill_error(
            &mut result.errors,
            format!("Unable to persist Codex scan coverage: {error}"),
        );
    }

    Ok(result)
}

fn record_codex_session_skill_candidate(
    candidate: &CodexSessionSkillCandidate,
    connection: &mut Connection,
    runtime_roots: &[PathBuf],
) -> Result<SkillUsageRecordResult> {
    let mut request = usage_request_from_skill_ref_with_roots(UsageRequestFromSkillRef {
        skill_ref: &candidate.skill,
        hook_agent: "codex",
        session_id: &candidate.session_id,
        turn_id: Some(&candidate.turn_id),
        index: candidate.index,
        hook_event: "session_backfill",
        model: "",
        runtime_roots: Some(runtime_roots),
        preferred_runtime_context: candidate.runtime_context.as_deref(),
    })?;
    request.used_at = candidate.used_at.clone();
    if let Some(metadata) = request.metadata.as_mut() {
        if let Some(object) = metadata.as_object_mut() {
            object.insert(
                "source".to_string(),
                serde_json::Value::String("codex_session_backfill".to_string()),
            );
        }
    }
    record_skill_usage_on_connection(request, connection, true)
}

fn extract_codex_session_skill_candidates(
    path: &Path,
) -> Result<(Vec<CodexSessionSkillCandidate>, usize)> {
    let file = fs::File::open(path).map_err(|error| format!("Unable to read session: {error}"))?;
    let reader = BufReader::new(file);
    let mut session_id = session_id_from_rollout_path(path);
    let mut session_runtime_context: Option<PathBuf> = None;
    let mut turn_id: Option<String> = None;
    let mut turn_used_at: Option<String> = None;
    let mut turn_skills: Vec<HookSkillRef> = Vec::new();
    let mut turn_prompts: Vec<String> = Vec::new();
    let mut has_turn_context = false;
    let mut results = Vec::new();
    let mut parse_errors = 0usize;

    for line in reader.lines() {
        let line = line.map_err(|error| format!("Unable to read session line: {error}"))?;
        if line.trim().is_empty() {
            continue;
        }
        let value: serde_json::Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(_) => {
                parse_errors += 1;
                continue;
            }
        };
        let event_type = value.get("type").and_then(|value| value.as_str());
        let timestamp = value
            .get("timestamp")
            .and_then(|value| value.as_str())
            .map(str::to_string);

        match event_type {
            Some("session_meta") => {
                if let Some(id) = session_id_from_session_meta(&value) {
                    session_id = id;
                }
                session_runtime_context = value
                    .get("payload")
                    .and_then(|payload| payload.get("cwd"))
                    .and_then(|value| value.as_str())
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(PathBuf::from);
            }
            Some("turn_context") => {
                flush_codex_session_turn(
                    &mut results,
                    &session_id,
                    turn_id.as_deref(),
                    turn_used_at.as_deref(),
                    session_runtime_context.as_deref(),
                    &mut turn_skills,
                    &mut turn_prompts,
                );
                has_turn_context = true;
                turn_id = value
                    .get("payload")
                    .and_then(|payload| payload.get("turn_id"))
                    .and_then(|value| value.as_str())
                    .map(str::to_string);
                turn_used_at = timestamp;
                turn_skills.clear();
                turn_prompts.clear();
            }
            _ => {
                if has_turn_context {
                    if turn_used_at.is_none() {
                        turn_used_at = timestamp;
                    }
                    collect_codex_user_input(&value, &mut turn_prompts, &mut turn_skills);
                    continue;
                }

                if turn_used_at.is_none() {
                    turn_used_at = timestamp.clone();
                }
                if turn_id.is_none() {
                    if let Some(complete_turn) = task_complete_turn_id(&value) {
                        turn_id = Some(complete_turn.to_string());
                    }
                }
                collect_codex_user_input(&value, &mut turn_prompts, &mut turn_skills);
                if task_complete_turn_id(&value).is_some() {
                    flush_codex_session_turn(
                        &mut results,
                        &session_id,
                        turn_id.as_deref(),
                        turn_used_at.as_deref(),
                        session_runtime_context.as_deref(),
                        &mut turn_skills,
                        &mut turn_prompts,
                    );
                    turn_id = None;
                    turn_used_at = None;
                }
            }
        }
    }

    flush_codex_session_turn(
        &mut results,
        &session_id,
        turn_id.as_deref(),
        turn_used_at.as_deref(),
        session_runtime_context.as_deref(),
        &mut turn_skills,
        &mut turn_prompts,
    );

    Ok((results, parse_errors))
}

fn collect_codex_user_input(
    value: &serde_json::Value,
    turn_prompts: &mut Vec<String>,
    turn_skills: &mut Vec<HookSkillRef>,
) {
    let first_new_prompt = turn_prompts.len();
    collect_user_message_text(value, turn_prompts);
    for prompt in &turn_prompts[first_new_prompt..] {
        turn_skills.extend(
            extract_skill_refs_from_text(prompt)
                .into_iter()
                .filter_map(auditable_codex_backfill_skill_ref),
        );
        turn_skills.extend(
            extract_explicit_skill_refs_from_text(prompt)
                .into_iter()
                .filter_map(auditable_codex_backfill_skill_ref),
        );
    }
}

fn auditable_codex_backfill_skill_ref(mut skill: HookSkillRef) -> Option<HookSkillRef> {
    let path = expand_home(skill.path);
    if !path.is_absolute() || path.file_name().and_then(|value| value.to_str()) != Some("SKILL.md")
    {
        return None;
    }
    skill.path = path;
    Some(skill)
}

fn flush_codex_session_turn(
    results: &mut Vec<CodexSessionSkillCandidate>,
    session_id: &str,
    turn_id: Option<&str>,
    used_at: Option<&str>,
    runtime_context: Option<&Path>,
    turn_skills: &mut Vec<HookSkillRef>,
    turn_prompts: &mut Vec<String>,
) {
    if turn_skills.is_empty() {
        turn_prompts.clear();
        return;
    }
    let turn = turn_id.unwrap_or("session").to_string();
    let prompt = turn_prompts.first().cloned();
    let skills = dedupe_hook_skill_refs(std::mem::take(turn_skills));
    for (index, mut skill) in skills.into_iter().enumerate() {
        if skill.prompt_excerpt.is_none() {
            skill.prompt_excerpt = prompt.clone();
        }
        results.push(CodexSessionSkillCandidate {
            session_id: session_id.to_string(),
            turn_id: turn.clone(),
            index,
            used_at: used_at.map(str::to_string),
            runtime_context: runtime_context.map(Path::to_path_buf),
            skill,
        });
    }
    turn_prompts.clear();
}

fn collect_jsonl_files(root: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
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
            collect_jsonl_files(&path, files)?;
            continue;
        }
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        if file_name.starts_with("rollout-")
            && file_name
                .rsplit_once('.')
                .is_some_and(|(_, ext)| ext.eq_ignore_ascii_case("jsonl"))
        {
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

fn session_id_from_rollout_path(path: &Path) -> String {
    path.file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("unknown-session")
        .to_string()
}

fn session_id_from_session_meta(value: &serde_json::Value) -> Option<String> {
    value
        .get("payload")
        .and_then(|payload| payload.get("id"))
        .and_then(|value| value.as_str())
        .map(str::to_string)
}

fn task_complete_turn_id(value: &serde_json::Value) -> Option<&str> {
    let payload = value.get("payload")?;
    if payload.get("type").and_then(|value| value.as_str()) != Some("task_complete") {
        return None;
    }
    payload.get("turn_id").and_then(|value| value.as_str())
}
