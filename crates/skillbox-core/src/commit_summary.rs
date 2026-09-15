use crate::*;
use std::fs::OpenOptions;
use std::io::{Read, Seek, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

const COMMIT_SUMMARY_CLI_PREFERENCE: &str = "commit_summary_cli";
const COMMIT_SUMMARY_CLI_TIMEOUT: Duration = Duration::from_secs(60);
const MAX_COMMIT_SUMMARY_PROMPT_CHARS: usize = 24 * 1024;
const MAX_COMMIT_SUMMARY_DIFF_CHARS: usize = 2 * 1024;
const MAX_COMMIT_MESSAGE_CHARS: usize = 200;
const DEFAULT_USER_SKILLS_COMMIT_MESSAGE: &str = "chore(github): sync user skills";
const CURSOR_AGENT_COMMIT_SUMMARY_MODEL: &str = "cursor-grok-4.6-high-fast";
static COMMIT_SUMMARY_OUTPUT_COUNTER: AtomicU64 = AtomicU64::new(0);

fn commit_summary_cli_timeout() -> Duration {
    if cfg!(test) {
        Duration::from_secs(5)
    } else {
        COMMIT_SUMMARY_CLI_TIMEOUT
    }
}

pub fn set_commit_summary_cli(
    managed_root: impl AsRef<Path>,
    cli_path: impl AsRef<str>,
) -> Result<ManagedPreferences> {
    let paths = ensure_managed_layout(managed_root.as_ref().to_path_buf())?;
    let stored = match validate_commit_summary_cli(cli_path.as_ref())? {
        None => String::new(),
        Some(path) => path.to_string_lossy().into_owned(),
    };
    write_string_preference(&paths.database_path, COMMIT_SUMMARY_CLI_PREFERENCE, &stored)?;
    managed_preferences(paths.root)
}

pub fn suggest_user_skills_commit_message(
    request: SuggestUserSkillsCommitRequest,
    managed_root: impl AsRef<Path>,
) -> Result<SuggestedUserSkillsCommit> {
    let managed_root = managed_root.as_ref();
    let changes = user_skills_git_changes(managed_root)?;
    let selected_paths = match request.selected_paths {
        None => changes
            .files
            .iter()
            .map(|file| file.path.clone())
            .collect::<Vec<_>>(),
        Some(paths) => validate_git_relative_paths(&paths)?,
    };
    let selected = selected_change_files(&changes.files, &selected_paths)?;
    let heuristic = heuristic_user_skills_commit_message(&selected);
    let paths = ensure_managed_layout(managed_root.to_path_buf())?;
    let purposes = skill_purposes(&paths.user_skills_root, &selected);

    let preferences = managed_preferences(managed_root)?;
    let Some(cli_path) = resolve_commit_summary_cli(
        &preferences.commit_summary_cli,
        &cursor_agent_search_paths(),
    )?
    else {
        return Ok(SuggestedUserSkillsCommit {
            message: heuristic,
            source: "heuristic".to_string(),
            cli_path: None,
        });
    };

    let prompt = commit_summary_prompt(&selected, &heuristic, &purposes);
    let message = run_commit_summary_cli(&cli_path, &prompt)?;
    Ok(SuggestedUserSkillsCommit {
        message: prefer_specific_commit_message(&message, &selected, &purposes),
        source: "cli".to_string(),
        cli_path: Some(cli_path.to_string_lossy().into_owned()),
    })
}

pub(crate) fn heuristic_user_skills_commit_message(files: &[UserSkillsGitChangeFile]) -> String {
    let skills = unique_skill_names(files);
    let labels = files
        .iter()
        .map(|file| git_status_label(&file.status))
        .collect::<HashSet<_>>();

    if files.is_empty() || skills.is_empty() {
        return DEFAULT_USER_SKILLS_COMMIT_MESSAGE.to_string();
    }

    if skills.len() == 1 {
        let skill = &skills[0];
        if labels.len() == 1 && labels.contains("Added") {
            return format!("feat(github): add {skill} skill");
        }
        if labels.len() == 1 && labels.contains("Deleted") {
            return format!("chore(github): remove {skill} skill");
        }
        if labels.len() == 1 && labels.contains("Renamed") {
            return format!("chore(github): rename {skill} skill");
        }
        return format!("feat(github): update {skill} skill");
    }

    if skills.len() == 2 {
        return format!("chore(github): sync {} and {} skills", skills[0], skills[1]);
    }

    format!(
        "chore(github): sync {} and {} more skills",
        skills[0],
        skills.len() - 1
    )
}

pub(crate) fn validate_commit_summary_cli(raw: &str) -> Result<Option<PathBuf>> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if trimmed.contains('\0') || trimmed.contains('\n') || trimmed.contains('\r') {
        return Err("Commit summary CLI path cannot contain control characters.".to_string());
    }

    let path = expand_user_path(trimmed)?;
    if !path.is_absolute() {
        if trimmed.chars().any(char::is_whitespace) {
            return Err(
                "Commit summary CLI must be an executable path only; do not include arguments."
                    .to_string(),
            );
        }
        return Err(
            "Commit summary CLI must be an absolute executable path, not a command line."
                .to_string(),
        );
    }

    match fs::symlink_metadata(&path) {
        Ok(metadata) => {
            if metadata.file_type().is_dir() {
                return Err("Commit summary CLI must be a file, not a directory.".to_string());
            }
        }
        Err(_) if trimmed.chars().any(char::is_whitespace) => {
            return Err(
                "Commit summary CLI must be an executable path only; do not include arguments."
                    .to_string(),
            );
        }
        Err(_) => {
            return Err(format!("Commit summary CLI not found: {}", path.display()));
        }
    }

    let metadata = fs::metadata(&path).map_err(|error| {
        format!(
            "Unable to read commit summary CLI {}: {error}",
            path.display()
        )
    })?;
    if !metadata.is_file() {
        return Err("Commit summary CLI must be a regular executable file.".to_string());
    }
    if metadata.permissions().mode() & 0o111 == 0 {
        return Err("Commit summary CLI must be executable.".to_string());
    }

    Ok(Some(path))
}

pub(crate) fn resolve_commit_summary_cli(
    configured: &str,
    search_paths: &[PathBuf],
) -> Result<Option<PathBuf>> {
    if !configured.trim().is_empty() {
        return validate_commit_summary_cli(configured);
    }

    for candidate in search_paths {
        if let Ok(Some(path)) = validate_commit_summary_cli(&candidate.to_string_lossy()) {
            return Ok(Some(path));
        }
    }

    Ok(None)
}

pub(crate) fn cursor_agent_search_paths() -> Vec<PathBuf> {
    if cfg!(test) {
        return Vec::new();
    }

    let mut paths = Vec::new();
    if let Some(home) = std::env::var_os("HOME") {
        let home = PathBuf::from(home);
        paths.push(home.join(".local/bin/agent"));
        paths.push(home.join(".cursor/bin/agent"));
    }
    paths.push(PathBuf::from("/usr/local/bin/agent"));
    paths
}

pub(crate) fn sanitize_commit_message(raw: &str) -> Result<String> {
    let mut text = raw.trim().to_string();
    if text.starts_with("```") {
        text = text
            .lines()
            .skip(1)
            .take_while(|line| !line.trim_start().starts_with("```"))
            .collect::<Vec<_>>()
            .join("\n");
    }

    let mut fallback = None;
    for line in text.lines() {
        let line = line
            .trim()
            .trim_matches(|ch| ch == '`' || ch == '"' || ch == '\'')
            .trim();
        if line.is_empty() {
            continue;
        }
        if line.chars().any(|ch| ch.is_control()) {
            continue;
        }
        if looks_like_commit_subject(line) {
            return clamp_commit_message(line);
        }
        if fallback.is_none() {
            fallback = Some(line.to_string());
        }
    }

    fallback
        .ok_or_else(|| "Commit summary CLI returned an empty message.".to_string())
        .and_then(|line| clamp_commit_message(&line))
}

fn selected_change_files(
    files: &[UserSkillsGitChangeFile],
    selected_paths: &[String],
) -> Result<Vec<UserSkillsGitChangeFile>> {
    if files.is_empty() {
        return Ok(Vec::new());
    }
    if selected_paths.is_empty() {
        return Err("Select at least one file to commit.".to_string());
    }

    let available = files
        .iter()
        .map(|file| file.path.as_str())
        .collect::<HashSet<_>>();
    for path in selected_paths {
        if !available.contains(path.as_str()) {
            return Err(format!(
                "Selected file is not in the current change list: {path}"
            ));
        }
    }

    Ok(files
        .iter()
        .filter(|file| selected_paths.iter().any(|path| path == &file.path))
        .cloned()
        .collect())
}

fn unique_skill_names(files: &[UserSkillsGitChangeFile]) -> Vec<String> {
    let mut names = Vec::new();
    let mut seen = HashSet::new();
    for file in files {
        let Some(name) = skill_name_from_path(&file.path) else {
            continue;
        };
        if seen.insert(name.clone()) {
            names.push(name);
        }
    }
    names
}

fn skill_name_from_path(path: &str) -> Option<String> {
    let mut parts = path.split('/').filter(|part| !part.is_empty());
    let first = parts.next()?;
    if parts.next().is_none() || first.starts_with('.') {
        return None;
    }
    Some(first.to_string())
}

fn git_status_label(status: &str) -> &'static str {
    if status.contains('D') {
        "Deleted"
    } else if status.contains('R') {
        "Renamed"
    } else if status.contains('A') || status.contains('?') {
        "Added"
    } else if status.contains('M') {
        "Modified"
    } else {
        "Changed"
    }
}

fn commit_summary_prompt(
    files: &[UserSkillsGitChangeFile],
    heuristic: &str,
    purposes: &[(String, String)],
) -> String {
    let mut prompt = String::from(
        "Write one Conventional Commit subject line for these user-skill changes.\n\
         Reply with only that line. No markdown, quotes, or commit body.\n\
         Use feat, fix, or chore, with scope github.\n\
         Include the skill folder name and what the skill does.\n\
         Never write \"add X skill\" or \"update X skill\" as the whole subject.\n",
    );
    prompt.push_str("Never emit: ");
    prompt.push_str(heuristic);
    prompt.push('\n');
    prompt.push_str(
        "Example: feat(github): add title-optimizer to generate evidence-grounded headlines\n\n",
    );

    if !purposes.is_empty() {
        prompt.push_str("Skill purpose:\n");
        for (name, description) in purposes {
            prompt.push_str(&format!("- {name}: {description}\n"));
        }
        prompt.push('\n');
    }

    prompt.push_str("Selected files:\n");
    for file in files {
        prompt.push_str(&format!(
            "- {} ({})\n",
            file.path,
            git_status_label(&file.status)
        ));
    }
    prompt.push_str("\nDiff excerpts:\n");

    for file in files {
        if prompt.len() >= MAX_COMMIT_SUMMARY_PROMPT_CHARS {
            break;
        }
        prompt.push_str("*** ");
        prompt.push_str(&file.path);
        prompt.push('\n');
        prompt.push_str(&truncate_chars(&file.diff, MAX_COMMIT_SUMMARY_DIFF_CHARS));
        prompt.push('\n');
    }

    truncate_chars(&prompt, MAX_COMMIT_SUMMARY_PROMPT_CHARS)
}

fn skill_purposes(
    user_skills_root: &Path,
    files: &[UserSkillsGitChangeFile],
) -> Vec<(String, String)> {
    unique_skill_names(files)
        .into_iter()
        .filter_map(|name| {
            let from_file = fs::read_to_string(user_skills_root.join(&name).join("SKILL.md"))
                .ok()
                .map(|text| parse_skill_frontmatter(&text).description)
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty());
            let description = from_file.or_else(|| description_from_diffs(&name, files))?;
            Some((name, description))
        })
        .collect()
}

fn description_from_diffs(skill: &str, files: &[UserSkillsGitChangeFile]) -> Option<String> {
    let skill_md = format!("{skill}/SKILL.md");
    let prefix = format!("{skill}/");
    files
        .iter()
        .filter(|file| file.path == skill_md || file.path.starts_with(&prefix))
        .find_map(|file| frontmatter_description(&file.diff))
}

fn frontmatter_description(text: &str) -> Option<String> {
    for raw in text.lines() {
        let line = strip_diff_line(raw);
        let Some(rest) = line.trim().strip_prefix("description:") else {
            continue;
        };
        let value = rest.trim().trim_matches('"').trim();
        if !value.is_empty() && value != ">" && value != "|" {
            return Some(value.to_string());
        }
    }
    None
}

fn strip_diff_line(line: &str) -> &str {
    if line.starts_with("+++") || line.starts_with("---") || line.starts_with("@@") {
        return "";
    }
    line.strip_prefix('+')
        .or_else(|| line.strip_prefix('-'))
        .or_else(|| line.strip_prefix(' '))
        .unwrap_or(line)
}

fn prefer_specific_commit_message(
    message: &str,
    files: &[UserSkillsGitChangeFile],
    purposes: &[(String, String)],
) -> String {
    if !is_generic_skill_subject(message, files) {
        return message.to_string();
    }
    specific_subject_from_purposes(files, purposes).unwrap_or_else(|| message.to_string())
}

fn is_generic_skill_subject(message: &str, files: &[UserSkillsGitChangeFile]) -> bool {
    let message = message.trim();
    if message == heuristic_user_skills_commit_message(files) {
        return true;
    }
    let skills = unique_skill_names(files);
    if skills.len() != 1 {
        return false;
    }
    let skill = &skills[0];
    [
        format!("feat(github): add {skill} skill"),
        format!("feat(github): update {skill} skill"),
        format!("chore(github): remove {skill} skill"),
        format!("chore(github): rename {skill} skill"),
        format!("chore(github): sync {skill} skill"),
    ]
    .iter()
    .any(|line| line == message)
}

fn specific_subject_from_purposes(
    files: &[UserSkillsGitChangeFile],
    purposes: &[(String, String)],
) -> Option<String> {
    let skills = unique_skill_names(files);
    if skills.len() != 1 {
        return None;
    }
    let skill = &skills[0];
    let purpose = purposes
        .iter()
        .find(|(name, _)| name == skill)
        .map(|(_, description)| description.as_str())?;
    if !purpose_is_useful(skill, purpose) {
        return None;
    }
    let clause = purpose_clause(purpose)?;
    let labels = files
        .iter()
        .map(|file| git_status_label(&file.status))
        .collect::<HashSet<_>>();
    if labels.len() == 1 && labels.contains("Deleted") {
        return None;
    }
    let message = if labels.len() == 1 && labels.contains("Added") {
        format!("feat(github): add {skill} to {clause}")
    } else {
        format!("feat(github): update {skill} to {clause}")
    };
    clamp_commit_message(&message).ok()
}

fn purpose_is_useful(skill: &str, description: &str) -> bool {
    let words = description.split_whitespace().count();
    if words < 4 {
        return false;
    }
    let normalized = description.to_lowercase().replace('-', " ");
    let skill_words = skill.replace('-', " ");
    normalized != skill_words && normalized != format!("{skill_words} skill")
}

fn purpose_clause(description: &str) -> Option<String> {
    let sentence = description
        .split(['.', '\n'])
        .next()?
        .trim()
        .trim_matches('"')
        .trim();
    if sentence.is_empty() {
        return None;
    }
    let mut clause = lowercase_first(sentence);
    if clause.chars().count() > 90 {
        clause = clause.chars().take(90).collect();
        if let Some(index) = clause.rfind(' ') {
            clause.truncate(index);
        }
    }
    let clause = clause.trim_end_matches([',', ';', ':']).trim().to_string();
    if clause.is_empty() {
        None
    } else {
        Some(clause)
    }
}

fn lowercase_first(value: &str) -> String {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) => first.to_lowercase().to_string() + chars.as_str(),
        None => String::new(),
    }
}

fn run_commit_summary_cli(cli_path: &Path, prompt: &str) -> Result<String> {
    let workspace = commit_summary_workspace()?;
    let output = if is_cursor_agent_cli(cli_path) {
        let mut command = Command::new(cli_path);
        command
            .current_dir(&workspace)
            .arg("--print")
            .arg("--mode")
            .arg("ask")
            .arg("--model")
            .arg(CURSOR_AGENT_COMMIT_SUMMARY_MODEL)
            .arg("--output-format")
            .arg("text")
            .arg("--sandbox")
            .arg("enabled")
            .arg("--trust")
            .arg("--workspace")
            .arg(&workspace)
            .arg(prompt);
        isolate_commit_summary_environment(&mut command);
        run_timed_command(
            command,
            None,
            commit_summary_cli_timeout(),
            "Commit summary CLI",
        )
    } else {
        let mut command = Command::new(cli_path);
        command.current_dir(&workspace);
        isolate_commit_summary_environment(&mut command);
        run_timed_command(
            command,
            Some(prompt.as_bytes()),
            commit_summary_cli_timeout(),
            "Commit summary CLI",
        )
    };
    let _ = fs::remove_dir_all(&workspace);
    let output = output?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let detail = stderr.trim();
        if detail.is_empty() {
            return Err(format!(
                "Commit summary CLI exited with status {}.",
                output.status
            ));
        }
        return Err(format!(
            "Commit summary CLI failed: {}",
            truncate_chars(detail, 500)
        ));
    }

    sanitize_commit_message(&String::from_utf8_lossy(&output.stdout))
}

fn is_cursor_agent_cli(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|name| name.to_str()),
        Some("agent" | "cursor-agent")
    )
}

fn isolate_commit_summary_environment(command: &mut Command) {
    for key in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_INDEX_FILE",
        "GIT_OBJECT_DIRECTORY",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    ] {
        command.env_remove(key);
    }
}

fn commit_summary_workspace() -> Result<PathBuf> {
    let id = COMMIT_SUMMARY_OUTPUT_COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "skillbox-commit-summary-{}-{id}",
        std::process::id()
    ));
    fs::create_dir_all(&path).map_err(|error| error.to_string())?;
    Ok(path)
}

fn run_timed_command(
    mut command: Command,
    stdin_data: Option<&[u8]>,
    timeout: Duration,
    label: &str,
) -> Result<Output> {
    let output_id = COMMIT_SUMMARY_OUTPUT_COUNTER.fetch_add(1, Ordering::Relaxed);
    let output_root = std::env::temp_dir().join(format!(
        "skillbox-commit-summary-output-{}-{output_id}",
        std::process::id()
    ));
    fs::create_dir_all(&output_root).map_err(|error| error.to_string())?;
    let stdout_path = output_root.join("stdout");
    let stderr_path = output_root.join("stderr");
    let mut stdout_file = OpenOptions::new()
        .create_new(true)
        .read(true)
        .write(true)
        .open(&stdout_path)
        .map_err(|error| error.to_string())?;
    let mut stderr_file = OpenOptions::new()
        .create_new(true)
        .read(true)
        .write(true)
        .open(&stderr_path)
        .map_err(|error| error.to_string())?;

    command.process_group(0);
    let mut child = command
        .stdin(if stdin_data.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::from(
            stdout_file.try_clone().map_err(|error| error.to_string())?,
        ))
        .stderr(Stdio::from(
            stderr_file.try_clone().map_err(|error| error.to_string())?,
        ))
        .spawn()
        .map_err(|error| {
            let _ = fs::remove_dir_all(&output_root);
            format!("{label} failed to start: {error}")
        })?;

    if let Some(bytes) = stdin_data {
        if let Some(mut stdin) = child.stdin.take() {
            if let Err(error) = stdin.write_all(bytes) {
                terminate_process_group(child.id());
                let _ = child.wait();
                let _ = fs::remove_dir_all(&output_root);
                return Err(format!("{label} failed to send prompt: {error}"));
            }
        }
    }

    let started_at = Instant::now();
    loop {
        if let Some(status) = child.try_wait().map_err(|error| error.to_string())? {
            let stdout = read_output_file(&mut stdout_file)?;
            let stderr = read_output_file(&mut stderr_file)?;
            let _ = fs::remove_dir_all(&output_root);
            return Ok(Output {
                status,
                stdout,
                stderr,
            });
        }

        if started_at.elapsed() >= timeout {
            terminate_process_group(child.id());
            let cleanup_started = Instant::now();
            while child
                .try_wait()
                .map_err(|error| error.to_string())?
                .is_none()
                && cleanup_started.elapsed() < Duration::from_millis(500)
            {
                thread::sleep(Duration::from_millis(10));
            }
            kill_process_group(child.id());
            let _ = child.wait();
            let _ = fs::remove_dir_all(&output_root);
            return Err(format!(
                "{label} timed out after {}.",
                format_timeout(timeout)
            ));
        }

        thread::sleep(Duration::from_millis(20));
    }
}

fn read_output_file(file: &mut std::fs::File) -> Result<Vec<u8>> {
    file.rewind().map_err(|error| error.to_string())?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    Ok(bytes)
}

fn terminate_process_group(pid: u32) {
    // SAFETY: negative pid targets only the isolated process group created for this command.
    unsafe {
        libc::kill(-(pid as i32), libc::SIGTERM);
    }
}

fn kill_process_group(pid: u32) {
    // SAFETY: negative pid targets only the isolated process group created for this command.
    unsafe {
        libc::kill(-(pid as i32), libc::SIGKILL);
    }
}

fn expand_user_path(raw: &str) -> Result<PathBuf> {
    if raw == "~" {
        return home_dir();
    }
    if let Some(rest) = raw.strip_prefix("~/") {
        return Ok(home_dir()?.join(rest));
    }
    Ok(PathBuf::from(raw))
}

fn home_dir() -> Result<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| "Unable to resolve home directory for commit summary CLI.".to_string())
}

fn looks_like_commit_subject(line: &str) -> bool {
    let types = [
        "feat", "fix", "docs", "style", "refactor", "perf", "test", "build", "ci", "chore",
    ];
    types
        .iter()
        .any(|kind| line.starts_with(&format!("{kind}:")) || line.starts_with(&format!("{kind}(")))
}

fn clamp_commit_message(line: &str) -> Result<String> {
    let mut message = line.trim().to_string();
    if message.chars().any(|ch| ch.is_control()) {
        return Err("Commit summary CLI returned control characters.".to_string());
    }
    if message.chars().count() > MAX_COMMIT_MESSAGE_CHARS {
        message = message.chars().take(MAX_COMMIT_MESSAGE_CHARS).collect();
    }
    if message.is_empty() {
        return Err("Commit summary CLI returned an empty message.".to_string());
    }
    Ok(message)
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let truncated = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        format!("{truncated}\n...")
    } else {
        truncated
    }
}

fn format_timeout(timeout: Duration) -> String {
    if timeout.as_secs() >= 1 {
        format!("{}s", timeout.as_secs())
    } else {
        format!("{}ms", timeout.as_millis())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heuristic_matches_selected_skill_operations() {
        let added = UserSkillsGitChangeFile {
            path: "dida-task-sync/SKILL.md".to_string(),
            status: "??".to_string(),
            diff: "diff".to_string(),
        };
        let modified = UserSkillsGitChangeFile {
            path: "codex-chat-sync/SKILL.md".to_string(),
            status: " M".to_string(),
            diff: "diff".to_string(),
        };
        let deleted = UserSkillsGitChangeFile {
            path: "old-skill/SKILL.md".to_string(),
            status: " D".to_string(),
            diff: "diff".to_string(),
        };

        assert_eq!(
            heuristic_user_skills_commit_message(std::slice::from_ref(&added)),
            "feat(github): add dida-task-sync skill"
        );
        assert_eq!(
            heuristic_user_skills_commit_message(std::slice::from_ref(&modified)),
            "feat(github): update codex-chat-sync skill"
        );
        assert_eq!(
            heuristic_user_skills_commit_message(&[deleted]),
            "chore(github): remove old-skill skill"
        );
        assert_eq!(
            heuristic_user_skills_commit_message(&[modified, added]),
            "chore(github): sync codex-chat-sync and dida-task-sync skills"
        );
    }

    #[test]
    fn commit_summary_prompt_asks_for_a_specific_subject() {
        let files = [UserSkillsGitChangeFile {
            path: "alpha/SKILL.md".to_string(),
            status: "??".to_string(),
            diff: "+name: alpha\n+description: Generate evidence-grounded titles from a topic\n"
                .to_string(),
        }];
        let heuristic = heuristic_user_skills_commit_message(&files);
        let prompt = commit_summary_prompt(
            &files,
            &heuristic,
            &[(
                "alpha".to_string(),
                "Generate evidence-grounded titles from a topic".to_string(),
            )],
        );
        assert!(prompt.contains("Never write"));
        assert!(prompt.contains(&heuristic));
        assert!(prompt.contains("Skill purpose:"));
        assert!(prompt.contains("Generate evidence-grounded titles from a topic"));
        assert!(prompt.contains("alpha/SKILL.md"));
    }

    #[test]
    fn prefer_specific_commit_message_replaces_generic_add_skill_line() {
        let files = [UserSkillsGitChangeFile {
            path: "content-title-optimizer/SKILL.md".to_string(),
            status: "??".to_string(),
            diff: String::new(),
        }];
        let purposes = [(
            "content-title-optimizer".to_string(),
            "Generate, compare, and revise evidence-grounded titles from a topic.".to_string(),
        )];
        assert_eq!(
            prefer_specific_commit_message(
                "feat(github): add content-title-optimizer skill",
                &files,
                &purposes
            ),
            "feat(github): add content-title-optimizer to generate, compare, and revise evidence-grounded titles from a topic"
        );
    }

    #[test]
    fn prefer_specific_commit_message_keeps_specific_cli_output() {
        let files = [UserSkillsGitChangeFile {
            path: "alpha/SKILL.md".to_string(),
            status: "??".to_string(),
            diff: String::new(),
        }];
        let message = "feat(github): add alpha to generate evidence-grounded titles";
        assert_eq!(
            prefer_specific_commit_message(message, &files, &[]),
            message
        );
    }

    #[test]
    fn sanitize_commit_message_unwraps_fences_and_prefers_conventional_line() {
        let raw = "Sure.\n```text\nfeat(github): update alpha skill\n```\n";
        assert_eq!(
            sanitize_commit_message(raw).unwrap(),
            "feat(github): update alpha skill"
        );
    }

    #[test]
    fn validate_commit_summary_cli_rejects_command_lines() {
        let error = validate_commit_summary_cli("agent -p --mode ask").unwrap_err();
        assert!(error.contains("executable path only"));
    }

    #[test]
    fn validate_commit_summary_cli_rejects_relative_paths() {
        let error = validate_commit_summary_cli("agent").unwrap_err();
        assert!(error.contains("absolute executable path"));
    }

    #[test]
    fn resolve_commit_summary_cli_skips_missing_search_paths() {
        let dir = std::env::temp_dir().join(format!(
            "skillbox-cli-search-{}-{}",
            std::process::id(),
            COMMIT_SUMMARY_OUTPUT_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&dir).unwrap();
        let agent = dir.join("agent");
        fs::write(&agent, "#!/bin/sh\nexit 0\n").unwrap();
        let mut permissions = fs::metadata(&agent).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&agent, permissions).unwrap();

        let resolved =
            resolve_commit_summary_cli("", &[dir.join("missing"), agent.clone()]).unwrap();
        assert_eq!(resolved, Some(agent));
        let _ = fs::remove_dir_all(&dir);
    }
}
