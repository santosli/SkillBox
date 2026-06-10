use crate::*;

pub fn usage_hook_statuses() -> Result<Vec<UsageHookStatus>> {
    usage_hook_statuses_for_home_and_managed_root(home_dir(), default_managed_root())
}

pub fn usage_hook_statuses_for_home(home: impl AsRef<Path>) -> Result<Vec<UsageHookStatus>> {
    let home = home.as_ref();
    usage_hook_statuses_for_home_and_managed_root(home, home.join(".skillbox"))
}

pub(crate) fn usage_hook_statuses_for_home_and_managed_root(
    home: impl AsRef<Path>,
    managed_root: impl AsRef<Path>,
) -> Result<Vec<UsageHookStatus>> {
    let home = home.as_ref();
    let database_path = managed_paths(managed_root.as_ref().to_path_buf()).database_path;
    [
        UsageHookTarget::CodexApp,
        UsageHookTarget::CodexCli,
        UsageHookTarget::ClaudeCodeCli,
    ]
    .into_iter()
    .map(|target| usage_hook_status_for_home(target, home, &database_path))
    .collect()
}

pub fn install_usage_hook(target: UsageHookTarget) -> Result<UsageHookInstallResult> {
    install_usage_hook_for_home_and_managed_root(target, home_dir(), default_managed_root())
}

pub fn install_usage_hook_for_home(
    target: UsageHookTarget,
    home: impl AsRef<Path>,
) -> Result<UsageHookInstallResult> {
    let home = home.as_ref();
    install_usage_hook_for_home_and_managed_root(target, home, home.join(".skillbox"))
}

pub(crate) fn install_usage_hook_for_home_and_managed_root(
    target: UsageHookTarget,
    home: impl AsRef<Path>,
    managed_root: impl AsRef<Path>,
) -> Result<UsageHookInstallResult> {
    let home = home.as_ref();
    let database_path = managed_paths(managed_root.as_ref().to_path_buf()).database_path;
    write_usage_hook_runner(home)?;
    let config_path = usage_hook_config_path(target, home);
    let command = usage_hook_command_for_home(target, home);
    let mut config = read_hook_config_json(&config_path)?;

    if json_has_hook_command(&config, &command) {
        return Ok(UsageHookInstallResult {
            target,
            installed: false,
            backup_path: None,
            status: usage_hook_status_for_home(target, home, &database_path)?,
        });
    }

    let replaced = replace_usage_hook_command(&mut config, target, &command);
    if !replaced {
        inject_stop_hook_command(&mut config, &command)?;
    }
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let backup_path = if config_path.exists() {
        let backup_path = next_usage_hook_backup_path(&config_path);
        fs::copy(&config_path, &backup_path).map_err(|error| {
            format!(
                "Failed to back up {} to {}: {error}",
                config_path.display(),
                backup_path.display()
            )
        })?;
        Some(backup_path)
    } else {
        None
    };
    let json = serde_json::to_string_pretty(&config).map_err(|error| error.to_string())?;
    fs::write(&config_path, format!("{json}\n")).map_err(|error| error.to_string())?;

    Ok(UsageHookInstallResult {
        target,
        installed: true,
        backup_path,
        status: usage_hook_status_for_home(target, home, &database_path)?,
    })
}

pub fn parse_usage_hook_target(value: &str) -> Result<UsageHookTarget> {
    match value.trim().to_ascii_lowercase().as_str() {
        "codex-app" | "codex_app" => Ok(UsageHookTarget::CodexApp),
        "codex-cli" | "codex_cli" | "agents" => Ok(UsageHookTarget::CodexCli),
        "claude-code" | "claude_code" | "claude-code-cli" | "claude_code_cli" | "claude" => {
            Ok(UsageHookTarget::ClaudeCodeCli)
        }
        other => Err(format!("Unknown usage hook target: {other}")),
    }
}

pub fn record_skill_usage_from_hook(
    agent: &str,
    hook_input: &str,
    managed_root: impl AsRef<Path>,
) -> Result<UsageHookRecordResult> {
    let hook: serde_json::Value =
        serde_json::from_str(hook_input).map_err(|error| format!("Invalid hook JSON: {error}"))?;
    let Some(transcript_path) = hook
        .get("transcript_path")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(UsageHookRecordResult {
            recorded: Vec::new(),
            skipped: vec!["missing transcript_path".to_string()],
        });
    };
    let transcript = fs::read_to_string(expand_home(PathBuf::from(transcript_path)))
        .map_err(|error| format!("Unable to read hook transcript: {error}"))?;
    let turn_id = hook.get("turn_id").and_then(|value| value.as_str());
    let session_id = hook
        .get("session_id")
        .and_then(|value| value.as_str())
        .unwrap_or("unknown-session");
    let hook_event = hook
        .get("hook_event_name")
        .and_then(|value| value.as_str())
        .unwrap_or("unknown");
    let model = hook
        .get("model")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    let agent = normalize_usage_hook_agent(agent)?;
    let skill_refs = extract_skill_refs_from_transcript(&transcript, turn_id);
    let mut recorded = Vec::new();
    let mut skipped = Vec::new();

    for (index, skill_ref) in skill_refs.into_iter().enumerate() {
        match usage_request_from_skill_ref(
            &skill_ref, &agent, session_id, turn_id, index, hook_event, model,
        ) {
            Ok(request) => match record_skill_usage(request, managed_root.as_ref()) {
                Ok(result) => recorded.push(result),
                Err(error) => skipped.push(format!("{}: {error}", skill_ref.name)),
            },
            Err(error) => skipped.push(format!("{}: {error}", skill_ref.name)),
        }
    }

    Ok(UsageHookRecordResult { recorded, skipped })
}

pub(crate) fn usage_hook_status_for_home(
    target: UsageHookTarget,
    home: &Path,
    database_path: &Path,
) -> Result<UsageHookStatus> {
    let config_path = usage_hook_config_path(target, home);
    let command = usage_hook_command_for_home(target, home);
    let installed = read_hook_config_json(&config_path)
        .map(|config| {
            json_has_hook_command(&config, &command)
                && usage_hook_wrapper_path(home).is_file()
                && usage_hook_runner_path(home).is_file()
        })
        .unwrap_or(false);
    let trust_required = installed
        && usage_hook_target_requires_trust(target)
        && !usage_hook_has_recorded_agent_hook(database_path, usage_hook_agent_arg(target));
    Ok(UsageHookStatus {
        target,
        label: usage_hook_label(target).to_string(),
        config_path,
        command,
        installed,
        trust_required,
        activation_note: usage_hook_activation_note(trust_required),
        shared_config_key: usage_hook_shared_config_key(target).to_string(),
    })
}

pub(crate) fn usage_hook_has_recorded_agent_hook(database_path: &Path, agent: &str) -> bool {
    if !database_path.is_file() {
        return false;
    }
    let Ok(connection) = open_database(database_path) else {
        return false;
    };
    let Ok(mut statement) =
        connection.prepare("SELECT metadata_json FROM skill_usage_events WHERE agent_id = ?1")
    else {
        return false;
    };
    let Ok(rows) = statement.query_map(params![agent], |row| row.get::<_, String>(0)) else {
        return false;
    };

    let has_recorded_hook = rows.filter_map(|row| row.ok()).any(|metadata_json| {
        serde_json::from_str::<serde_json::Value>(&metadata_json)
            .ok()
            .is_some_and(|metadata| {
                metadata.get("source").and_then(|value| value.as_str()) == Some("agent_hook")
                    && metadata.get("hook_agent").and_then(|value| value.as_str()) == Some(agent)
            })
    });
    has_recorded_hook
}

pub(crate) fn usage_hook_label(target: UsageHookTarget) -> &'static str {
    match target {
        UsageHookTarget::CodexApp => "Codex App",
        UsageHookTarget::CodexCli => "Codex CLI",
        UsageHookTarget::ClaudeCodeCli => "Claude Code CLI",
    }
}

pub(crate) fn usage_hook_shared_config_key(target: UsageHookTarget) -> &'static str {
    match target {
        UsageHookTarget::CodexApp | UsageHookTarget::CodexCli => "codex",
        UsageHookTarget::ClaudeCodeCli => "claude-code",
    }
}

pub(crate) fn usage_hook_target_requires_trust(target: UsageHookTarget) -> bool {
    matches!(
        target,
        UsageHookTarget::CodexApp | UsageHookTarget::CodexCli
    )
}

pub(crate) fn usage_hook_activation_note(trust_required: bool) -> Option<String> {
    if trust_required {
        return Some(
            "Review and trust this hook in Codex /hooks before automatic counting can run."
                .to_string(),
        );
    }
    None
}

pub(crate) fn usage_hook_command_for_home(target: UsageHookTarget, home: &Path) -> String {
    let agent = usage_hook_agent_arg(target);
    format!(
        "{} {agent}",
        shell_quote_path(&usage_hook_wrapper_path(home))
    )
}

pub(crate) fn usage_hook_agent_arg(target: UsageHookTarget) -> &'static str {
    match target {
        UsageHookTarget::CodexApp | UsageHookTarget::CodexCli => "codex",
        UsageHookTarget::ClaudeCodeCli => "claude-code",
    }
}

pub(crate) fn usage_hook_runner_dir(home: &Path) -> PathBuf {
    home.join(".skillbox").join("bin")
}

pub(crate) fn usage_hook_runner_path(home: &Path) -> PathBuf {
    usage_hook_runner_dir(home).join("skillbox-usage-hook-runner")
}

pub(crate) fn usage_hook_wrapper_path(home: &Path) -> PathBuf {
    usage_hook_runner_dir(home).join("skillbox-usage-hook")
}

pub(crate) fn write_usage_hook_runner(home: &Path) -> Result<()> {
    let runner_dir = usage_hook_runner_dir(home);
    fs::create_dir_all(&runner_dir).map_err(|error| error.to_string())?;
    let runner_path = usage_hook_runner_path(home);
    let wrapper_path = usage_hook_wrapper_path(home);
    let current_exe = std::env::current_exe()
        .map_err(|error| format!("Unable to locate current executable: {error}"))?;
    let temporary_runner = runner_path.with_extension("tmp");

    fs::copy(&current_exe, &temporary_runner).map_err(|error| {
        format!(
            "Failed to copy usage hook runner from {} to {}: {error}",
            current_exe.display(),
            temporary_runner.display()
        )
    })?;
    fs::rename(&temporary_runner, &runner_path).map_err(|error| error.to_string())?;
    set_executable_permission(&runner_path)?;

    let wrapper = format!(
        "#!/bin/sh\nexec {} usage-hook \"$@\"\n",
        shell_quote_path(&runner_path)
    );
    fs::write(&wrapper_path, wrapper).map_err(|error| error.to_string())?;
    set_executable_permission(&wrapper_path)?;
    Ok(())
}

pub(crate) fn set_executable_permission(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(path)
            .map_err(|error| error.to_string())?
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).map_err(|error| error.to_string())?;
    }
    Ok(())
}

pub(crate) fn shell_quote_path(path: &Path) -> String {
    let value = path.to_string_lossy();
    format!("'{}'", value.replace('\'', "'\\''"))
}

pub(crate) fn legacy_usage_hook_commands(target: UsageHookTarget) -> Vec<String> {
    let agent = usage_hook_agent_arg(target);
    vec![
        format!("skillbox usage-hook {agent}"),
        format!("skillbox-cli usage-hook {agent}"),
    ]
}

pub(crate) fn usage_hook_config_path(target: UsageHookTarget, home: &Path) -> PathBuf {
    match target {
        UsageHookTarget::CodexApp | UsageHookTarget::CodexCli => {
            home.join(".codex").join("hooks.json")
        }
        UsageHookTarget::ClaudeCodeCli => home.join(".claude").join("settings.json"),
    }
}

pub(crate) fn read_hook_config_json(path: &Path) -> Result<serde_json::Value> {
    if !path.exists() {
        return Ok(serde_json::json!({}));
    }
    let input = fs::read_to_string(path).map_err(|error| error.to_string())?;
    if input.trim().is_empty() {
        return Ok(serde_json::json!({}));
    }
    let value: serde_json::Value = serde_json::from_str(&input)
        .map_err(|error| format!("Invalid hook config {}: {error}", path.display()))?;
    if !value.is_object() {
        return Err(format!(
            "Hook config must be a JSON object: {}",
            path.display()
        ));
    }
    Ok(value)
}

pub(crate) fn inject_stop_hook_command(
    config: &mut serde_json::Value,
    command: &str,
) -> Result<()> {
    let Some(root) = config.as_object_mut() else {
        return Err("Hook config must be a JSON object.".to_string());
    };
    let hooks = root.entry("hooks").or_insert_with(|| serde_json::json!({}));
    let Some(hooks) = hooks.as_object_mut() else {
        return Err("Hook config field `hooks` must be a JSON object.".to_string());
    };
    let stop = hooks
        .entry("Stop")
        .or_insert_with(|| serde_json::Value::Array(Vec::new()));
    let Some(stop) = stop.as_array_mut() else {
        return Err("Hook config field `hooks.Stop` must be an array.".to_string());
    };
    stop.push(serde_json::json!({
        "hooks": [{
            "type": "command",
            "command": command,
            "timeout": 5,
            "statusMessage": "Recording SkillBox usage"
        }]
    }));
    Ok(())
}

pub(crate) fn replace_usage_hook_command(
    config: &mut serde_json::Value,
    target: UsageHookTarget,
    command: &str,
) -> bool {
    let legacy_commands = legacy_usage_hook_commands(target);
    replace_json_command(config, &legacy_commands, command)
}

pub(crate) fn replace_json_command(
    value: &mut serde_json::Value,
    old_commands: &[String],
    new_command: &str,
) -> bool {
    match value {
        serde_json::Value::Object(object) => {
            let mut replaced = false;
            if object
                .get("command")
                .and_then(|value| value.as_str())
                .is_some_and(|value| should_replace_usage_hook_command(value, old_commands))
            {
                object.insert(
                    "command".to_string(),
                    serde_json::Value::String(new_command.to_string()),
                );
                replaced = true;
            }
            object.values_mut().fold(replaced, |changed, nested| {
                replace_json_command(nested, old_commands, new_command) || changed
            })
        }
        serde_json::Value::Array(values) => values.iter_mut().fold(false, |changed, nested| {
            replace_json_command(nested, old_commands, new_command) || changed
        }),
        _ => false,
    }
}

pub(crate) fn should_replace_usage_hook_command(command: &str, old_commands: &[String]) -> bool {
    if old_commands.iter().any(|old| old == command) {
        return true;
    }
    old_commands.iter().any(|old| {
        let Some(agent) = old.strip_prefix("skillbox usage-hook ") else {
            return false;
        };
        command.ends_with(&format!(" usage-hook {agent}"))
            && (command.contains("skillbox-cli")
                || command.contains("skillbox-desktop")
                || command.contains("skillbox-usage-hook"))
    })
}

pub(crate) fn json_has_hook_command(value: &serde_json::Value, command: &str) -> bool {
    match value {
        serde_json::Value::Object(object) => {
            object
                .get("command")
                .and_then(|value| value.as_str())
                .is_some_and(|value| value == command)
                || object
                    .values()
                    .any(|nested| json_has_hook_command(nested, command))
        }
        serde_json::Value::Array(values) => values
            .iter()
            .any(|nested| json_has_hook_command(nested, command)),
        _ => false,
    }
}

pub(crate) fn next_usage_hook_backup_path(path: &Path) -> PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_millis();
    for attempt in 0..100 {
        let suffix = if attempt == 0 {
            format!("skillbox-backup-{timestamp}")
        } else {
            format!("skillbox-backup-{timestamp}-{attempt}")
        };
        let candidate = PathBuf::from(format!("{}.{}", path.display(), suffix));
        if !candidate.exists() {
            return candidate;
        }
    }
    PathBuf::from(format!(
        "{}.skillbox-backup-{timestamp}-fallback",
        path.display()
    ))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HookSkillRef {
    name: String,
    path: PathBuf,
    prompt_excerpt: Option<String>,
}

pub(crate) fn normalize_usage_hook_agent(value: &str) -> Result<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "codex" | "codex-app" | "codex-cli" | "agents" => Ok("codex".to_string()),
        "claude" | "claude-code" | "claude-code-cli" => Ok("claude-code".to_string()),
        other => Err(format!("Unknown usage hook agent: {other}")),
    }
}

pub(crate) fn usage_request_from_skill_ref(
    skill_ref: &HookSkillRef,
    hook_agent: &str,
    session_id: &str,
    turn_id: Option<&str>,
    index: usize,
    hook_event: &str,
    model: &str,
) -> Result<RecordSkillUsageRequest> {
    let (runtime_root, agent_id) =
        infer_usage_runtime_from_skill_path(&skill_ref.path, hook_agent)?;
    let path_hash = &sha256(&skill_ref.path.to_string_lossy())[..12];
    let turn = turn_id.unwrap_or("session");
    let metadata = serde_json::json!({
        "source": "agent_hook",
        "hook_agent": hook_agent,
        "hook_event": hook_event,
        "model": model
    });
    Ok(RecordSkillUsageRequest {
        skill_name: skill_ref.name.clone(),
        agent_id,
        runtime_root,
        event_id: Some(format!(
            "{hook_agent}:{session_id}:{turn}:{index}:{}:{path_hash}",
            skill_ref.name
        )),
        used_at: None,
        prompt_excerpt: skill_ref.prompt_excerpt.clone(),
        metadata: Some(metadata),
    })
}

pub(crate) fn infer_usage_runtime_from_skill_path(
    skill_path: &Path,
    hook_agent: &str,
) -> Result<(PathBuf, String)> {
    let expanded = expand_home(skill_path.to_path_buf());
    for ancestor in expanded.ancestors() {
        if ancestor.file_name().and_then(|name| name.to_str()) != Some("skills") {
            continue;
        }
        let parent = ancestor
            .parent()
            .and_then(|path| path.file_name())
            .and_then(|name| name.to_str());
        let agent_id = match parent {
            Some(".codex") => Some("codex"),
            Some(".agents") => Some("agents"),
            Some(".claude") => Some("claude"),
            _ => None,
        };
        if let Some(agent_id) = agent_id {
            return Ok((
                fs::canonicalize(ancestor).unwrap_or_else(|_| ancestor.to_path_buf()),
                agent_id.to_string(),
            ));
        }
    }

    let fallback_root = expanded
        .parent()
        .and_then(|path| path.parent())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| expanded.parent().unwrap_or(&expanded).to_path_buf());
    let agent_id = match hook_agent {
        "claude-code" => "claude",
        _ => "codex",
    };
    Ok((
        fs::canonicalize(&fallback_root).unwrap_or(fallback_root),
        agent_id.to_string(),
    ))
}

pub(crate) fn extract_skill_refs_from_transcript(
    transcript: &str,
    turn_id: Option<&str>,
) -> Vec<HookSkillRef> {
    let values: Vec<serde_json::Value> = transcript
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .collect();
    let selected = if values
        .iter()
        .any(|value| value.get("type").and_then(|value| value.as_str()) == Some("turn_context"))
    {
        select_turn_context_transcript_values(&values, turn_id)
    } else {
        select_task_complete_turn_values(&values, turn_id)
    };
    let prompt_excerpt = extract_prompt_excerpt_from_values(&selected);
    let mut skills = Vec::new();
    for value in selected {
        visit_json_strings(value, &mut |text| {
            skills.extend(extract_skill_refs_from_text(text));
        });
    }
    let mut skills = dedupe_hook_skill_refs(skills);
    for skill in &mut skills {
        skill.prompt_excerpt = prompt_excerpt.clone();
    }
    skills
}

pub(crate) fn select_turn_context_transcript_values<'a>(
    values: &'a [serde_json::Value],
    turn_id: Option<&str>,
) -> Vec<&'a serde_json::Value> {
    let mut current_turn: Option<String> = None;
    let mut selected = Vec::new();
    for value in values {
        if value.get("type").and_then(|value| value.as_str()) == Some("turn_context") {
            current_turn = value
                .get("payload")
                .and_then(|payload| payload.get("turn_id"))
                .and_then(|turn| turn.as_str())
                .map(ToString::to_string);
            continue;
        }
        if turn_id.is_some() && current_turn.as_deref() != turn_id {
            continue;
        }
        selected.push(value);
    }
    selected
}

pub(crate) fn select_task_complete_turn_values<'a>(
    values: &'a [serde_json::Value],
    turn_id: Option<&str>,
) -> Vec<&'a serde_json::Value> {
    let Some(turn_id) = turn_id else {
        return values.iter().collect();
    };
    let Some(end) = values
        .iter()
        .position(|value| task_complete_turn_id(value) == Some(turn_id))
    else {
        return values.iter().collect();
    };
    let start = values[..end]
        .iter()
        .rposition(|value| task_complete_turn_id(value).is_some())
        .map(|index| index + 1)
        .unwrap_or(0);

    values[start..=end].iter().collect()
}

pub(crate) fn task_complete_turn_id(value: &serde_json::Value) -> Option<&str> {
    if value.get("type").and_then(|value| value.as_str()) != Some("event_msg") {
        return None;
    }
    let payload = value.get("payload")?;
    if payload.get("type").and_then(|value| value.as_str()) != Some("task_complete") {
        return None;
    }
    payload.get("turn_id").and_then(|value| value.as_str())
}

pub(crate) fn extract_prompt_excerpt_from_values(values: &[&serde_json::Value]) -> Option<String> {
    let mut prompts = Vec::new();
    for value in values {
        collect_user_message_text(value, &mut prompts);
    }
    prompts
        .iter()
        .rev()
        .find_map(|prompt| normalize_usage_prompt_excerpt(Some(prompt)))
}

pub(crate) fn collect_user_message_text(value: &serde_json::Value, prompts: &mut Vec<String>) {
    let Some(payload) = value.get("payload") else {
        return;
    };
    if payload.get("type").and_then(|value| value.as_str()) == Some("user_message") {
        if let Some(message) = payload.get("message").and_then(|value| value.as_str()) {
            prompts.push(message.to_string());
        }
        return;
    }
    if payload.get("type").and_then(|value| value.as_str()) != Some("message") {
        return;
    }
    if payload.get("role").and_then(|value| value.as_str()) != Some("user") {
        return;
    }

    if let Some(content) = payload.get("content") {
        collect_message_content_text(content, prompts);
    }
    if let Some(text) = payload.get("text").and_then(|value| value.as_str()) {
        prompts.push(text.to_string());
    }
}

pub(crate) fn collect_message_content_text(value: &serde_json::Value, prompts: &mut Vec<String>) {
    match value {
        serde_json::Value::String(text) => prompts.push(text.to_string()),
        serde_json::Value::Array(values) => {
            for nested in values {
                collect_message_content_text(nested, prompts);
            }
        }
        serde_json::Value::Object(object) => {
            if let Some(text) = object.get("text").and_then(|value| value.as_str()) {
                prompts.push(text.to_string());
            }
        }
        _ => {}
    }
}

pub(crate) fn visit_json_strings(value: &serde_json::Value, visitor: &mut impl FnMut(&str)) {
    match value {
        serde_json::Value::String(text) => visitor(text),
        serde_json::Value::Array(values) => {
            for nested in values {
                visit_json_strings(nested, visitor);
            }
        }
        serde_json::Value::Object(object) => {
            for nested in object.values() {
                visit_json_strings(nested, visitor);
            }
        }
        _ => {}
    }
}

pub(crate) fn extract_skill_refs_from_text(text: &str) -> Vec<HookSkillRef> {
    let mut remaining = text;
    let mut skills = Vec::new();
    while let Some(start) = remaining.find("<skill>") {
        let after_start = &remaining[start + "<skill>".len()..];
        let Some(end) = after_start.find("</skill>") else {
            break;
        };
        let block = &after_start[..end];
        if let (Some(name), Some(path)) = (xml_tag_text(block, "name"), xml_tag_text(block, "path"))
        {
            skills.push(HookSkillRef {
                name: name.trim().to_string(),
                path: PathBuf::from(path.trim()),
                prompt_excerpt: None,
            });
        }
        remaining = &after_start[end + "</skill>".len()..];
    }
    skills
}

pub(crate) fn xml_tag_text(input: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let after_open = input.split_once(&open)?.1;
    let value = after_open.split_once(&close)?.0;
    Some(value.to_string())
}

pub(crate) fn dedupe_hook_skill_refs(skills: Vec<HookSkillRef>) -> Vec<HookSkillRef> {
    let mut seen = HashSet::new();
    let mut deduped = Vec::new();
    for skill in skills {
        let key = format!("{}\n{}", skill.name, skill.path.display());
        if seen.insert(key) {
            deduped.push(skill);
        }
    }
    deduped
}
