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
    install_usage_hook_for_home_with_audit(target, home_dir(), default_managed_root())
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

pub(crate) fn install_usage_hook_for_home_with_audit(
    target: UsageHookTarget,
    home: impl AsRef<Path>,
    managed_root: impl AsRef<Path>,
) -> Result<UsageHookInstallResult> {
    let home = home.as_ref().to_path_buf();
    let managed_root = managed_root.as_ref().to_path_buf();
    audited_operation(
        OperationStart {
            operation_type: "install_usage_hook".to_string(),
            actor: "core".to_string(),
            entity_type: "agent_config".to_string(),
            entity_name: format!("{target:?}"),
            summary: "Install usage hook".to_string(),
            payload: serde_json::json!({"target": target}),
        },
        &managed_root,
        || install_usage_hook_for_home_and_managed_root(target, &home, &managed_root),
        |result| {
            (
                if result.installed {
                    format!("Installed {} usage hook", result.status.label)
                } else {
                    format!("{} usage hook already installed", result.status.label)
                },
                serde_json::json!({
                    "target": result.target,
                    "configPath": result.status.config_path,
                    "backupPath": result.backup_path,
                    "installed": result.installed
                }),
            )
        },
    )
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
    let preferred_runtime_context = hook
        .get("runtime_root")
        .or_else(|| hook.get("runtimeRoot"))
        .or_else(|| hook.get("cwd"))
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from);
    let agent = normalize_usage_hook_agent(agent)?;
    let skill_refs = extract_skill_refs_from_transcript(&transcript, turn_id);
    let mut recorded = Vec::new();
    let mut skipped = Vec::new();

    for (index, skill_ref) in skill_refs.into_iter().enumerate() {
        match usage_request_from_skill_ref_with_roots(UsageRequestFromSkillRef {
            skill_ref: &skill_ref,
            hook_agent: &agent,
            session_id,
            turn_id,
            index,
            hook_event,
            model,
            runtime_roots: None,
            preferred_runtime_context: preferred_runtime_context.as_deref(),
        }) {
            Ok(request) => {
                match record_trusted_generated_skill_usage(request, managed_root.as_ref()) {
                    Ok(result) => recorded.push(result),
                    Err(error) => skipped.push(format!("{}: {error}", skill_ref.name)),
                }
            }
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
            let mut changed = replaced;
            for nested in object.values_mut() {
                changed |= replace_json_command(nested, old_commands, new_command);
            }
            changed
        }
        serde_json::Value::Array(values) => {
            let mut changed = false;
            for nested in values {
                changed |= replace_json_command(nested, old_commands, new_command);
            }
            changed
        }
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
    pub(crate) name: String,
    pub(crate) path: PathBuf,
    pub(crate) prompt_excerpt: Option<String>,
}

pub(crate) fn normalize_usage_hook_agent(value: &str) -> Result<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "codex" | "codex-app" | "codex-cli" | "agents" => Ok("codex".to_string()),
        "claude" | "claude-code" | "claude-code-cli" => Ok("claude-code".to_string()),
        "cursor" | "cursor-agent" | "cursor-cli" => Ok("cursor".to_string()),
        other => Err(format!("Unknown usage hook agent: {other}")),
    }
}

pub(crate) struct UsageRequestFromSkillRef<'a> {
    pub(crate) skill_ref: &'a HookSkillRef,
    pub(crate) hook_agent: &'a str,
    pub(crate) session_id: &'a str,
    pub(crate) turn_id: Option<&'a str>,
    pub(crate) index: usize,
    pub(crate) hook_event: &'a str,
    pub(crate) model: &'a str,
    pub(crate) runtime_roots: Option<&'a [PathBuf]>,
    pub(crate) preferred_runtime_context: Option<&'a Path>,
}

pub(crate) fn usage_request_from_skill_ref_with_roots(
    input: UsageRequestFromSkillRef<'_>,
) -> Result<RecordSkillUsageRequest> {
    let (runtime_root, agent_id) = infer_usage_runtime_from_skill_path_with_roots(
        &input.skill_ref.path,
        input.hook_agent,
        input.runtime_roots,
        input.preferred_runtime_context,
    )?;
    let path_hash = &sha256(&input.skill_ref.path.to_string_lossy())[..12];
    let turn = input.turn_id.unwrap_or("session");
    let metadata = serde_json::json!({
        "source": "agent_hook",
        "hook_agent": input.hook_agent,
        "hook_event": input.hook_event,
        "model": input.model,
        "skill_source_kind": usage_source_kind_from_skill_path(
            &input.skill_ref.path,
            &runtime_root
        )
    });
    Ok(RecordSkillUsageRequest {
        skill_name: input.skill_ref.name.clone(),
        agent_id,
        runtime_root,
        event_id: Some(format!(
            "{}:{}:{}:{}:{}:{path_hash}",
            input.hook_agent, input.session_id, turn, input.index, input.skill_ref.name
        )),
        used_at: None,
        prompt_excerpt: input.skill_ref.prompt_excerpt.clone(),
        metadata: Some(metadata),
    })
}

pub(crate) fn infer_usage_runtime_from_skill_path_with_roots(
    skill_path: &Path,
    hook_agent: &str,
    runtime_roots: Option<&[PathBuf]>,
    preferred_runtime_context: Option<&Path>,
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
            Some(".codex") | Some(".agents") => Some("codex"),
            Some(".claude") => Some("claude-code"),
            Some(".cursor") => Some("cursor"),
            _ => None,
        };
        if let Some(agent_id) = agent_id {
            return Ok((
                fs::canonicalize(ancestor).unwrap_or_else(|_| ancestor.to_path_buf()),
                agent_id.to_string(),
            ));
        }
    }

    let owned_roots;
    let roots = match runtime_roots {
        Some(roots) => roots,
        None => {
            owned_roots = global_runtime_roots();
            owned_roots.as_slice()
        }
    };
    if let Some((runtime_root, agent_id)) = agent_runtime_root_for_managed_skill_path(
        &expanded,
        hook_agent,
        roots,
        preferred_runtime_context,
    ) {
        return Ok((runtime_root, agent_id));
    }

    let fallback_root = expanded
        .parent()
        .and_then(|path| path.parent())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| expanded.parent().unwrap_or(&expanded).to_path_buf());
    let agent_id = match hook_agent {
        "claude-code" | "claude" => "claude-code",
        "cursor" => "cursor",
        _ => "codex",
    };
    Ok((
        fs::canonicalize(&fallback_root).unwrap_or(fallback_root),
        agent_id.to_string(),
    ))
}

fn agent_runtime_root_for_managed_skill_path(
    skill_path: &Path,
    hook_agent: &str,
    runtime_roots: &[PathBuf],
    preferred_runtime_context: Option<&Path>,
) -> Option<(PathBuf, String)> {
    let skill_name = managed_skill_name_from_path(skill_path)?;
    let canonical_skill_path =
        fs::canonicalize(skill_path).unwrap_or_else(|_| skill_path.to_path_buf());
    let mut matches = Vec::new();
    for root in runtime_roots {
        let link = root.join(&skill_name);
        let Ok(metadata) = fs::symlink_metadata(&link) else {
            continue;
        };
        if !metadata.file_type().is_symlink() {
            continue;
        }
        let Ok(target) = fs::read_link(&link) else {
            continue;
        };
        let resolved = if target.is_absolute() {
            fs::canonicalize(&target).unwrap_or(target)
        } else {
            fs::canonicalize(root.join(&target)).unwrap_or_else(|_| root.join(&skill_name))
        };
        if !(resolved == canonical_skill_path
            || canonical_skill_path.starts_with(&resolved)
            || resolved.starts_with(&canonical_skill_path))
        {
            continue;
        }
        let agent_id = root
            .parent()
            .and_then(|path| path.file_name())
            .and_then(|name| name.to_str())
            .and_then(|name| match name {
                ".codex" | ".agents" => Some("codex"),
                ".claude" => Some("claude-code"),
                ".cursor" => Some("cursor"),
                _ => None,
            })
            .unwrap_or(match hook_agent {
                "claude-code" | "claude" => "claude-code",
                "cursor" => "cursor",
                _ => "codex",
            });
        let root = fs::canonicalize(root).unwrap_or_else(|_| root.clone());
        let preference = runtime_context_preference(&root, preferred_runtime_context);
        matches.push((preference, root, agent_id.to_string()));
    }
    matches.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then_with(|| left.1.cmp(&right.1))
            .then_with(|| left.2.cmp(&right.2))
    });
    matches
        .into_iter()
        .next()
        .map(|(_, root, agent_id)| (root, agent_id))
}

fn runtime_context_preference(runtime_root: &Path, context: Option<&Path>) -> u8 {
    let Some(context) = context else {
        return 2;
    };
    let context = expand_home(context.to_path_buf());
    let context = fs::canonicalize(&context).unwrap_or(context);
    if usage_runtime_key(runtime_root) == usage_runtime_key(&context) {
        return 0;
    }
    let workspace = runtime_root
        .parent()
        .and_then(Path::parent)
        .unwrap_or(runtime_root);
    if runtime_root.starts_with(&context)
        || context.starts_with(runtime_root)
        || context.starts_with(workspace)
    {
        return 1;
    }
    2
}

fn usage_source_kind_from_skill_path(skill_path: &Path, runtime_root: &Path) -> &'static str {
    let skill_path = expand_home(skill_path.to_path_buf());
    let system_root = runtime_root.join(".system");
    if skill_path.starts_with(&system_root) {
        "system"
    } else {
        "regular"
    }
}

fn managed_skill_name_from_path(path: &Path) -> Option<String> {
    let parts = path
        .components()
        .filter_map(|component| match component {
            Component::Normal(name) => name.to_str().map(str::to_string),
            _ => None,
        })
        .collect::<Vec<_>>();
    for index in 0..parts.len() {
        if matches!(parts[index].as_str(), "remote-skills" | "user-skills") {
            return parts
                .get(index + 1)
                .cloned()
                .filter(|name| !name.is_empty());
        }
    }
    None
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

pub(crate) fn extract_explicit_skill_refs_from_text(text: &str) -> Vec<HookSkillRef> {
    let mut remaining = text;
    let mut skills = Vec::new();
    while let Some(start) = remaining.find("[$") {
        let invocation = &remaining[start + 2..];
        let Some(label_end) = invocation.find("](") else {
            break;
        };
        let name = invocation[..label_end].trim();
        let destination = &invocation[label_end + 2..];
        let Some(destination_end) = destination.find(')') else {
            break;
        };
        let path = destination[..destination_end]
            .trim()
            .trim_matches(|character| matches!(character, '<' | '>'));
        if !name.is_empty()
            && name.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.' | ':')
            })
            && Path::new(path).file_name().and_then(|value| value.to_str()) == Some("SKILL.md")
        {
            skills.push(HookSkillRef {
                name: name.to_string(),
                path: PathBuf::from(path),
                prompt_excerpt: None,
            });
        }
        remaining = &destination[destination_end + 1..];
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
        let expanded = expand_home(skill.path.clone());
        let normalized_path = fs::canonicalize(&expanded).unwrap_or(expanded);
        let key = format!("{}\n{}", skill.name.trim(), normalized_path.display());
        if seen.insert(key) {
            deduped.push(skill);
        }
    }
    deduped
}
