use serde_json::Value;
use std::path::PathBuf;

fn validate_external_github_url(url: &str) -> Result<&str, String> {
    let trimmed = url.trim();
    let Some(rest) = trimmed.strip_prefix("https://github.com/") else {
        return Err("Only GitHub HTTPS URLs can be opened.".to_string());
    };

    if rest.is_empty()
        || trimmed.chars().any(char::is_whitespace)
        || trimmed
            .chars()
            .any(|character| matches!(character, '"' | '\'' | '<' | '>' | '\\'))
    {
        return Err("Invalid GitHub URL.".to_string());
    }

    Ok(trimmed)
}

#[tauri::command]
fn open_external_url(url: String) -> Result<(), String> {
    let url = validate_external_github_url(&url)?;
    let status = std::process::Command::new("open")
        .arg(url)
        .status()
        .map_err(|error| format!("Unable to open browser: {error}"))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("Browser open command failed with status {status}."))
    }
}

fn validate_local_folder_path(path: &str) -> Result<PathBuf, String> {
    let trimmed = path.trim();
    if trimmed.is_empty() || trimmed.chars().any(|character| character == '\0') {
        return Err("Invalid local folder path.".to_string());
    }

    let path = PathBuf::from(trimmed);
    if !path.is_absolute() {
        return Err("Only absolute local folder paths can be opened.".to_string());
    }

    let metadata = std::fs::metadata(&path)
        .map_err(|error| format!("Local skill folder does not exist: {error}"))?;
    if !metadata.is_dir() {
        return Err("Local skill path is not a folder.".to_string());
    }

    Ok(path)
}

#[tauri::command]
fn open_local_path(path: String) -> Result<(), String> {
    let path = validate_local_folder_path(&path)?;
    let status = std::process::Command::new("open")
        .arg(&path)
        .status()
        .map_err(|error| format!("Unable to open local folder: {error}"))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "Local folder open command failed with status {status}."
        ))
    }
}

#[tauri::command]
fn managed_paths() -> Result<Value, String> {
    serde_json::to_value(skillbox_core::managed_paths(
        skillbox_core::default_managed_root(),
    ))
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn managed_state() -> Result<Value, String> {
    let state = skillbox_core::managed_state(skillbox_core::default_managed_root())?;
    serde_json::to_value(state).map_err(|error| error.to_string())
}

#[tauri::command]
fn managed_preferences() -> Result<Value, String> {
    let preferences = skillbox_core::managed_preferences(skillbox_core::default_managed_root())?;
    serde_json::to_value(preferences).map_err(|error| error.to_string())
}

#[tauri::command]
fn set_skip_local_import_confirmation(skip: bool) -> Result<Value, String> {
    let preferences = skillbox_core::set_skip_local_import_confirmation(
        skillbox_core::default_managed_root(),
        skip,
    )?;
    serde_json::to_value(preferences).map_err(|error| error.to_string())
}

#[tauri::command]
fn set_status_refresh_interval_minutes(minutes: u32) -> Result<Value, String> {
    let preferences = skillbox_core::set_status_refresh_interval_minutes(
        skillbox_core::default_managed_root(),
        minutes,
    )?;
    serde_json::to_value(preferences).map_err(|error| error.to_string())
}

#[tauri::command]
fn set_remote_update_timeout_seconds(seconds: u32) -> Result<Value, String> {
    let preferences = skillbox_core::set_remote_update_timeout_seconds(
        skillbox_core::default_managed_root(),
        seconds,
    )?;
    serde_json::to_value(preferences).map_err(|error| error.to_string())
}

#[tauri::command]
fn scan_skills() -> Result<Value, String> {
    let scan = skillbox_core::scan_skill_roots(&skillbox_core::global_runtime_roots())?;
    serde_json::to_value(scan).map_err(|error| error.to_string())
}

#[tauri::command]
fn scan_import_candidates() -> Result<Value, String> {
    let scan = skillbox_core::scan_import_candidates(
        &skillbox_core::global_runtime_roots(),
        skillbox_core::default_managed_root(),
    )?;
    serde_json::to_value(scan).map_err(|error| error.to_string())
}

#[tauri::command]
fn scan_workspace_import_candidates(path: String) -> Result<Value, String> {
    let scan = skillbox_core::scan_import_candidates(
        &[std::path::PathBuf::from(path)],
        skillbox_core::default_managed_root(),
    )?;
    serde_json::to_value(scan).map_err(|error| error.to_string())
}

#[tauri::command]
async fn import_candidates(items: Vec<skillbox_core::ImportRequestItem>) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let result =
            skillbox_core::import_candidates(items, skillbox_core::default_managed_root())?;
        serde_json::to_value(result).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("Import candidates task failed: {error}"))?
}

#[tauri::command]
async fn deploy_skill(skill_name: String, target_root: String) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let result = skillbox_core::deploy_skill(
            &skill_name,
            skillbox_core::default_managed_root(),
            target_root,
        )?;
        serde_json::to_value(result).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("Skill deploy task failed: {error}"))?
}

#[tauri::command]
async fn undeploy_skill(skill_name: String, target_root: String) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let result = skillbox_core::undeploy_skill(
            &skill_name,
            skillbox_core::default_managed_root(),
            target_root,
        )?;
        serde_json::to_value(result).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("Skill undeploy task failed: {error}"))?
}

#[tauri::command]
fn parse_github_url(url: String) -> Result<Value, String> {
    let source = skillbox_github::parse_github_skill_url(&url)?;
    serde_json::to_value(source).map_err(|error| error.to_string())
}

#[tauri::command]
fn user_skills_git_status() -> Result<Value, String> {
    let status = skillbox_core::user_skills_git_status(skillbox_core::default_managed_root())?;
    serde_json::to_value(status).map_err(|error| error.to_string())
}

#[tauri::command]
fn user_skills_git_changes() -> Result<Value, String> {
    let changes = skillbox_core::user_skills_git_changes(skillbox_core::default_managed_root())?;
    serde_json::to_value(changes).map_err(|error| error.to_string())
}

#[tauri::command]
fn set_user_skills_git_remote(
    request: skillbox_core::UserSkillsGitRemoteRequest,
) -> Result<Value, String> {
    let status =
        skillbox_core::set_user_skills_git_remote(request, skillbox_core::default_managed_root())?;
    serde_json::to_value(status).map_err(|error| error.to_string())
}

#[tauri::command]
async fn sync_user_skills_git(
    request: skillbox_core::UserSkillsSyncRequest,
) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let result =
            skillbox_core::sync_user_skills_git(request, skillbox_core::default_managed_root())?;
        serde_json::to_value(result).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("User skills sync task failed: {error}"))?
}

#[tauri::command]
async fn check_remote_skill_updates(timeout_seconds: Option<u32>) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let result = if let Some(timeout_seconds) = timeout_seconds {
            skillbox_core::check_remote_skill_updates_with_timeout(
                skillbox_core::default_managed_root(),
                timeout_seconds,
            )?
        } else {
            skillbox_core::check_remote_skill_updates(skillbox_core::default_managed_root())?
        };
        serde_json::to_value(result).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("Remote update status check task failed: {error}"))?
}

#[tauri::command]
async fn check_remote_skill_update(
    skill_name: String,
    timeout_seconds: Option<u32>,
) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let result = if let Some(timeout_seconds) = timeout_seconds {
            skillbox_core::check_remote_skill_update_with_timeout(
                skillbox_core::default_managed_root(),
                &skill_name,
                timeout_seconds,
            )?
        } else {
            skillbox_core::check_remote_skill_update(
                skillbox_core::default_managed_root(),
                &skill_name,
            )?
        };
        serde_json::to_value(result).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("Remote update status check task failed: {error}"))?
}

#[tauri::command]
fn cached_remote_skill_updates() -> Result<Value, String> {
    let result = skillbox_core::cached_remote_skill_updates(skillbox_core::default_managed_root())?;
    serde_json::to_value(result).map_err(|error| error.to_string())
}

#[tauri::command]
async fn find_remote_source_candidates(skill_name: String) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let result = skillbox_core::find_remote_source_candidates(
            &skill_name,
            skillbox_core::default_managed_root(),
        )?;
        serde_json::to_value(result).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("Remote source search task failed: {error}"))?
}

#[tauri::command]
async fn preview_remote_source_binding(
    request: skillbox_core::RemoteSourceBindingRequest,
) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let result = skillbox_core::preview_remote_source_binding(
            request,
            skillbox_core::default_managed_root(),
        )?;
        serde_json::to_value(result).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("Remote source preview task failed: {error}"))?
}

#[tauri::command]
async fn bind_remote_source(
    request: skillbox_core::BindRemoteSourceRequest,
) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let result =
            skillbox_core::bind_remote_source(request, skillbox_core::default_managed_root())?;
        serde_json::to_value(result).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("Remote source bind task failed: {error}"))?
}

#[tauri::command]
fn list_remote_skill_versions(skill_name: String) -> Result<Value, String> {
    let result = skillbox_core::list_remote_skill_versions(
        &skill_name,
        skillbox_core::default_managed_root(),
    )?;
    serde_json::to_value(result).map_err(|error| error.to_string())
}

#[tauri::command]
fn list_user_skill_versions(skill_name: String) -> Result<Value, String> {
    let result = skillbox_core::list_user_skill_versions(
        &skill_name,
        skillbox_core::default_managed_root(),
    )?;
    serde_json::to_value(result).map_err(|error| error.to_string())
}

#[tauri::command]
async fn preview_remote_version_change(
    request: skillbox_core::RemoteVersionChangeRequest,
) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let result = skillbox_core::preview_remote_version_change(
            request,
            skillbox_core::default_managed_root(),
        )?;
        serde_json::to_value(result).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("Remote version preview task failed: {error}"))?
}

#[tauri::command]
async fn apply_remote_version_change(
    request: skillbox_core::RemoteVersionChangeApplyRequest,
) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let result = skillbox_core::apply_remote_version_change(
            request,
            skillbox_core::default_managed_root(),
        )?;
        serde_json::to_value(result).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("Remote version apply task failed: {error}"))?
}

#[tauri::command]
fn list_operations(request: skillbox_core::OperationFilter) -> Result<Value, String> {
    let result = skillbox_core::list_operations(request, skillbox_core::default_managed_root())?;
    serde_json::to_value(result).map_err(|error| error.to_string())
}

#[tauri::command]
fn list_workspaces() -> Result<Value, String> {
    let result = skillbox_core::list_workspaces(skillbox_core::default_managed_root())?;
    serde_json::to_value(result).map_err(|error| error.to_string())
}

#[tauri::command]
async fn scan_workspaces() -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let result = skillbox_core::scan_workspaces(skillbox_core::default_managed_root())?;
        serde_json::to_value(result).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("Workspace scan task failed: {error}"))?
}

#[tauri::command]
fn add_workspace(request: skillbox_core::WorkspaceAddRequest) -> Result<Value, String> {
    let result = skillbox_core::add_workspace(request, skillbox_core::default_managed_root())?;
    serde_json::to_value(result).map_err(|error| error.to_string())
}

#[tauri::command]
fn forget_workspace(path: String) -> Result<Value, String> {
    let result = skillbox_core::forget_workspace(path, skillbox_core::default_managed_root())?;
    serde_json::to_value(result).map_err(|error| error.to_string())
}

pub fn run() {
    let result = tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            open_external_url,
            open_local_path,
            managed_paths,
            managed_state,
            managed_preferences,
            set_skip_local_import_confirmation,
            set_status_refresh_interval_minutes,
            set_remote_update_timeout_seconds,
            scan_skills,
            scan_import_candidates,
            scan_workspace_import_candidates,
            import_candidates,
            deploy_skill,
            undeploy_skill,
            parse_github_url,
            user_skills_git_status,
            user_skills_git_changes,
            set_user_skills_git_remote,
            sync_user_skills_git,
            check_remote_skill_updates,
            check_remote_skill_update,
            cached_remote_skill_updates,
            find_remote_source_candidates,
            preview_remote_source_binding,
            bind_remote_source,
            list_remote_skill_versions,
            list_user_skill_versions,
            preview_remote_version_change,
            apply_remote_version_change,
            list_operations,
            list_workspaces,
            scan_workspaces,
            add_workspace,
            forget_workspace
        ])
        .run(tauri::generate_context!());

    if let Err(error) = result {
        eprintln!("failed to run SkillBox: {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::{validate_external_github_url, validate_local_folder_path};

    #[test]
    fn validates_github_https_urls_for_external_open() {
        assert_eq!(
            validate_external_github_url("https://github.com/owner/repo/tree/main/path/to/skill")
                .unwrap(),
            "https://github.com/owner/repo/tree/main/path/to/skill"
        );
    }

    #[test]
    fn rejects_non_github_external_urls() {
        assert!(validate_external_github_url("https://example.com/owner/repo").is_err());
        assert!(validate_external_github_url("http://github.com/owner/repo").is_err());
        assert!(validate_external_github_url("https://github.com/owner repo").is_err());
    }

    #[test]
    fn validates_existing_absolute_local_folders() {
        let cwd = std::env::current_dir().unwrap();

        assert_eq!(
            validate_local_folder_path(cwd.to_str().unwrap()).unwrap(),
            cwd
        );
    }

    #[test]
    fn rejects_non_local_folder_paths() {
        assert!(validate_local_folder_path("https://github.com/owner/repo").is_err());
        assert!(validate_local_folder_path("relative/path").is_err());
    }
}
