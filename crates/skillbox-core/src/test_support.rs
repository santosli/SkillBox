use super::*;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) fn record_test_call(
    mut request: RecordSkillUsageRequest,
    managed_root: impl AsRef<Path>,
) -> Result<SkillUsageRecordResult> {
    let source = request
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.get("source"))
        .and_then(|value| value.as_str());
    let source = source.map(str::to_string);
    if !matches!(
        source.as_deref(),
        Some(
            "agent_hook"
                | "codex_session_backfill"
                | "claude_code_session_backfill"
                | "cursor_agent_transcript_read"
        )
    ) {
        let metadata = request
            .metadata
            .get_or_insert_with(|| serde_json::json!({}));
        if let Some(object) = metadata.as_object_mut() {
            object.insert(
                "source".to_string(),
                serde_json::Value::String("agent_hook".to_string()),
            );
        }
    }
    if source.as_deref() == Some("claude_code_session_backfill") {
        if let Some(object) = request
            .metadata
            .as_mut()
            .and_then(|value| value.as_object_mut())
        {
            object.insert(
                "evidence_signal".to_string(),
                serde_json::Value::String("native_skill_tool".to_string()),
            );
        }
    }
    record_trusted_generated_skill_usage(request, managed_root)
}

pub(crate) fn database_migration_backups(database_path: &Path) -> Vec<PathBuf> {
    let prefix = format!(
        "{}.pre-migration-",
        database_path.file_name().unwrap().to_string_lossy()
    );
    let mut backups = fs::read_dir(database_path.parent().unwrap())
        .unwrap()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(&prefix) && name.ends_with(".bak"))
        })
        .collect::<Vec<_>>();
    backups.sort();
    backups
}

pub(crate) fn github_update_preview(
    source_url: &str,
    managed_root: &std::path::Path,
) -> GithubCollectionUpdatePreview {
    match preview_github_skill_collection_update(
        PreviewGithubSkillCollectionRequest {
            source_url: source_url.to_string(),
        },
        managed_root,
    )
    .unwrap()
    {
        GithubCollectionUpdatePreviewResult::Update { preview } => *preview,
        other => panic!("expected collection update preview, got {other:?}"),
    }
}

pub(crate) fn github_collection_child_selection(
    child: &ImportCandidateCollectionChild,
    skill_type: SkillKind,
) -> ImportCollectionChildSelection {
    ImportCollectionChildSelection {
        relative_path: child.relative_path.clone(),
        group_id: child.group_id.clone(),
        variant_id: child.variant_id.clone(),
        skill_type,
    }
}

pub(crate) fn commit_and_push_collection(work: &std::path::Path, message: &str) {
    run_git(work, &["add", "."]);
    run_git(
        work,
        &[
            "-c",
            "user.name=SkillBox",
            "-c",
            "user.email=skillbox@example.invalid",
            "commit",
            "-m",
            message,
        ],
    );
    run_git(work, &["push", "origin", "main"]);
}

pub(crate) fn temp_dir(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("skillbox-{label}-{nanos}"));
    fs::create_dir_all(&dir).unwrap();
    dir
}

pub(crate) fn write_test_executable(path: &Path, body: &str) {
    fs::write(path, body).unwrap();
    let mut permissions = fs::metadata(path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).unwrap();
}

pub(crate) fn make_skill(path: &std::path::Path, name: &str, description: &str) {
    make_skill_with_body(path, name, description, "");
}

pub(crate) fn insert_legacy_usage_event(
    database_path: &std::path::Path,
    skill_name: &str,
    agent_id: &str,
    runtime_root: &std::path::Path,
    used_at: &str,
    event_id: &str,
) {
    let connection = open_database(database_path).unwrap();
    let runtime_root =
        fs::canonicalize(runtime_root).unwrap_or_else(|_| runtime_root.to_path_buf());
    let runtime_root_value = runtime_root.to_string_lossy().to_string();
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
              metadata_json,
              evidence_class,
              evidence_sources_json
            )
            VALUES (
              ?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL,
              '{\"source\":\"agent_hook\"}',
              'confirmed',
              '[{\"source\":\"agent_hook\",\"evidence_class\":\"confirmed\"}]'
            )
            ",
            rusqlite::params![
                format!("legacy-{event_id}"),
                event_id,
                skill_name,
                agent_id,
                runtime_root_value,
                used_at,
                used_at,
            ],
        )
        .unwrap();
    connection
        .execute(
            "
            INSERT INTO skill_usage_stats (
              skill_name, agent_id, runtime_root, usage_count, last_used_at
            )
            VALUES (?1, ?2, ?3, 1, ?4)
            ON CONFLICT(skill_name, agent_id, runtime_root) DO UPDATE SET
              usage_count = skill_usage_stats.usage_count + 1,
              last_used_at = MAX(skill_usage_stats.last_used_at, excluded.last_used_at)
            ",
            rusqlite::params![skill_name, agent_id, runtime_root_value, used_at],
        )
        .unwrap();
}

pub(crate) fn make_skill_with_body(
    path: &std::path::Path,
    name: &str,
    description: &str,
    extra_body: &str,
) {
    fs::create_dir_all(path).unwrap();
    fs::write(
        path.join("SKILL.md"),
        format!(
            "---
name: {name}
description: \"{description}\"
---

# {name}
{extra_body}
"
        ),
    )
    .unwrap();
}

pub(crate) fn candidate<'a>(candidates: &'a [ImportCandidate], name: &str) -> &'a ImportCandidate {
    candidates
        .iter()
        .find(|candidate| candidate.name == name)
        .unwrap_or_else(|| panic!("candidate not found: {name}"))
}

pub(crate) fn remote_status<'a>(
    statuses: &'a [RemoteSkillUpdateStatus],
    skill_name: &str,
) -> &'a RemoteSkillUpdateStatus {
    statuses
        .iter()
        .find(|status| status.skill_name == skill_name)
        .unwrap_or_else(|| panic!("remote status not found: {skill_name}"))
}

pub(crate) fn workspace<'a>(workspaces: &'a [Workspace], path: &std::path::Path) -> &'a Workspace {
    let canonical = fs::canonicalize(path).unwrap();
    workspaces
        .iter()
        .find(|workspace| workspace.canonical_path == canonical)
        .unwrap_or_else(|| panic!("workspace not found: {}", path.display()))
}

pub(crate) fn write_remote_source(
    remote_root: &std::path::Path,
    repo_url: &std::path::Path,
    installed_sha: &str,
) {
    fs::create_dir_all(remote_root).unwrap();
    fs::write(
        remote_root.join("source.json"),
        format!(
            r#"{{
  "type": "github",
  "repoUrl": "{}",
  "ref": "main",
  "installedSha": "{}"
}}"#,
            repo_url.display(),
            installed_sha
        ),
    )
    .unwrap();
}

pub(crate) fn write_remote_source_with_json(remote_root: &std::path::Path, json: &str) {
    fs::create_dir_all(remote_root).unwrap();
    fs::write(remote_root.join("source.json"), json).unwrap();
}

pub(crate) fn bare_remote(label: &str) -> PathBuf {
    let remote = temp_dir(label).join("remote.git");
    let output = std::process::Command::new("git")
        .arg("init")
        .arg("--bare")
        .arg(&remote)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    remote
}

pub(crate) fn bare_remote_with_main(label: &str) -> PathBuf {
    let remote = bare_remote(label);
    let work = temp_dir(&format!("{label}-work"));
    run_git(&work, &["init", "-b", "main"]);
    fs::write(work.join("README.md"), "remote").unwrap();
    run_git(&work, &["add", "."]);
    run_git(
        &work,
        &[
            "-c",
            "user.name=SkillBox",
            "-c",
            "user.email=skillbox@example.invalid",
            "commit",
            "-m",
            "Initial",
        ],
    );
    run_git(
        &work,
        &["remote", "add", "origin", remote.to_str().unwrap()],
    );
    run_git(&work, &["push", "origin", "main"]);
    remote
}

pub(crate) fn bare_remote_with_skill_content(
    label: &str,
    skill_name: &str,
    description: &str,
    body: &str,
) -> PathBuf {
    let remote = bare_remote(label);
    let work = temp_dir(&format!("{label}-work"));
    run_git(&work, &["init", "-b", "main"]);
    let skill_dir = work.join("skills").join(skill_name);
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(
        skill_dir.join("SKILL.md"),
        format!(
            "---
name: {skill_name}
description: \"{description}\"
---

# {skill_name}
{body}
"
        ),
    )
    .unwrap();
    run_git(&work, &["add", "."]);
    run_git(
        &work,
        &[
            "-c",
            "user.name=SkillBox",
            "-c",
            "user.email=skillbox@example.invalid",
            "commit",
            "-m",
            "Add skill",
        ],
    );
    run_git(
        &work,
        &["remote", "add", "origin", remote.to_str().unwrap()],
    );
    run_git(&work, &["push", "-u", "origin", "main"]);
    remote
}

pub(crate) fn bare_remote_with_multiple_skill_content(
    label: &str,
    names: &[&str],
) -> (PathBuf, PathBuf) {
    let remote = bare_remote(label);
    let work = temp_dir(&format!("{label}-work"));
    run_git(&work, &["init", "-b", "main"]);
    for name in names {
        let skill_dir = work.join("skills").join(name);
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: \"{name} skill\"\n---\n\n# {name}\n"),
        )
        .unwrap();
    }
    run_git(&work, &["add", "."]);
    run_git(
        &work,
        &[
            "-c",
            "user.name=SkillBox",
            "-c",
            "user.email=skillbox@example.invalid",
            "commit",
            "-m",
            "Add skills",
        ],
    );
    run_git(
        &work,
        &["remote", "add", "origin", remote.to_str().unwrap()],
    );
    run_git(&work, &["push", "-u", "origin", "main"]);
    (remote, work)
}

pub(crate) fn bare_remote_with_root_skill_content(
    label: &str,
    skill_name: &str,
    description: &str,
    body: &str,
) -> (PathBuf, PathBuf) {
    let remote = bare_remote(label);
    let work = temp_dir(&format!("{label}-work"));
    run_git(&work, &["init", "-b", "main"]);
    make_skill_with_body(&work, skill_name, description, body);
    fs::write(work.join("README.md"), format!("# {skill_name}\n")).unwrap();
    fs::write(work.join(".gitignore"), "*.tmp\n").unwrap();
    fs::create_dir_all(work.join("assets")).unwrap();
    fs::write(work.join("assets/prompt.txt"), "prompt\n").unwrap();
    run_git(&work, &["add", "."]);
    run_git(
        &work,
        &[
            "-c",
            "user.name=SkillBox",
            "-c",
            "user.email=skillbox@example.invalid",
            "commit",
            "-m",
            "Add root skill",
        ],
    );
    run_git(
        &work,
        &["remote", "add", "origin", remote.to_str().unwrap()],
    );
    run_git(&work, &["push", "-u", "origin", "main"]);
    (remote, work)
}

pub(crate) fn github_source_url(owner: &str, repo: &str, skill_name: &str) -> String {
    format!("https://github.com/{owner}/{repo}/tree/main/skills/{skill_name}")
}

pub(crate) fn github_install_preview(
    source_url: &str,
    target_root: Option<PathBuf>,
    managed_root: &std::path::Path,
) -> GithubRemoteSkillInstallPreview {
    preview_github_remote_skill_install(
        PreviewGithubRemoteSkillInstallRequest {
            source_url: source_url.to_string(),
            target_root,
        },
        managed_root,
    )
    .unwrap()
}

pub(crate) static GIT_CONFIG_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub(crate) struct GitConfigRewriteGuard {
    _lock: std::sync::MutexGuard<'static, ()>,
    _rewrite: skillbox_git::TestTrustedUrlRewriteGuard,
}

impl Drop for GitConfigRewriteGuard {
    fn drop(&mut self) {}
}

pub(crate) fn github_repo_rewrite(
    owner: &str,
    repo: &str,
    remote: &std::path::Path,
) -> GitConfigRewriteGuard {
    let lock = GIT_CONFIG_LOCK.lock().unwrap();
    let rewrite = skillbox_git::test_trusted_url_rewrite(
        format!("file://{}", remote.display()),
        format!("https://github.com/{owner}/{repo}.git"),
    );

    GitConfigRewriteGuard {
        _lock: lock,
        _rewrite: rewrite,
    }
}

pub(crate) fn remote_head(remote: &std::path::Path) -> String {
    let output = std::process::Command::new("git")
        .arg("ls-remote")
        .arg(remote)
        .arg("main")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout)
        .split_whitespace()
        .next()
        .unwrap()
        .to_string()
}

pub(crate) fn run_git(cwd: &std::path::Path, args: &[&str]) {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
