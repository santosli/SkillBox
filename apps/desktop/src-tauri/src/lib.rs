use serde::Serialize;
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::Manager;
use tauri_plugin_updater::{Update, UpdaterExt};

#[derive(Default)]
struct PendingAppUpdate(Mutex<Option<Update>>);

#[derive(Debug)]
enum AppUpdateError {
    NoPendingUpdate,
    Updater(String),
}

impl std::fmt::Display for AppUpdateError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoPendingUpdate => {
                formatter.write_str("There is no pending app update to install.")
            }
            Self::Updater(message) => formatter.write_str(message),
        }
    }
}

impl From<tauri_plugin_updater::Error> for AppUpdateError {
    fn from(error: tauri_plugin_updater::Error) -> Self {
        Self::Updater(error.to_string())
    }
}

impl Serialize for AppUpdateError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppUpdateResponse {
    current_version: String,
    available: bool,
    disabled: bool,
    version: String,
    date: String,
    body: String,
    checked_at: String,
    message: String,
}

fn app_update_checked_at() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_default()
}

fn app_updater_disabled() -> bool {
    cfg!(debug_assertions) || !cfg!(target_os = "macos")
}

fn app_update_disabled_response(current_version: &str, message: &str) -> AppUpdateResponse {
    AppUpdateResponse {
        current_version: current_version.to_string(),
        available: false,
        disabled: true,
        version: String::new(),
        date: String::new(),
        body: String::new(),
        checked_at: app_update_checked_at(),
        message: message.to_string(),
    }
}

fn app_update_response_from_update(update: &Update) -> AppUpdateResponse {
    AppUpdateResponse {
        current_version: update.current_version.clone(),
        available: true,
        disabled: false,
        version: update.version.clone(),
        date: update
            .date
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_default(),
        body: update.body.clone().unwrap_or_default(),
        checked_at: app_update_checked_at(),
        message: "App update available.".to_string(),
    }
}

fn app_update_no_update_response(current_version: &str) -> AppUpdateResponse {
    AppUpdateResponse {
        current_version: current_version.to_string(),
        available: false,
        disabled: false,
        version: String::new(),
        date: String::new(),
        body: String::new(),
        checked_at: app_update_checked_at(),
        message: "SkillBox is up to date.".to_string(),
    }
}

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

fn validate_local_file_path(path: &str) -> Result<PathBuf, String> {
    let trimmed = path.trim();
    if trimmed.is_empty() || trimmed.chars().any(|character| character == '\0') {
        return Err("Invalid local file path.".to_string());
    }

    let path = PathBuf::from(trimmed);
    if !path.is_absolute() {
        return Err("Only absolute local file paths can be opened.".to_string());
    }

    let metadata =
        std::fs::metadata(&path).map_err(|error| format!("Local file does not exist: {error}"))?;
    if !metadata.is_file() {
        return Err("Local path is not a file.".to_string());
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
fn open_local_file(path: String) -> Result<(), String> {
    let path = validate_local_file_path(&path)?;
    let status = std::process::Command::new("open")
        .arg(&path)
        .status()
        .map_err(|error| format!("Unable to open local file: {error}"))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "Local file open command failed with status {status}."
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
async fn change_skill_kind(
    skill_name: String,
    skill_type: skillbox_core::SkillKind,
) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let result = skillbox_core::change_skill_kind(
            &skill_name,
            skill_type,
            skillbox_core::default_managed_root(),
        )?;
        serde_json::to_value(result).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("Skill type change task failed: {error}"))?
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
async fn install_github_remote_skill(
    request: skillbox_core::InstallGithubRemoteSkillRequest,
) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let result = skillbox_core::install_github_remote_skill(
            request,
            skillbox_core::default_managed_root(),
        )?;
        serde_json::to_value(result).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("Remote skill install task failed: {error}"))?
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
async fn list_history(request: skillbox_core::HistoryFilter) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let result = skillbox_core::list_history(request, skillbox_core::default_managed_root())?;
        serde_json::to_value(result).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("History list task failed: {error}"))?
}

#[tauri::command]
async fn record_skill_usage(
    request: skillbox_core::RecordSkillUsageRequest,
) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let result =
            skillbox_core::record_skill_usage(request, skillbox_core::default_managed_root())?;
        serde_json::to_value(result).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("Skill usage record task failed: {error}"))?
}

#[tauri::command]
fn usage_hook_statuses() -> Result<Value, String> {
    let result = skillbox_core::usage_hook_statuses()?;
    serde_json::to_value(result).map_err(|error| error.to_string())
}

#[tauri::command]
async fn install_usage_hook(target: String) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let target = skillbox_core::parse_usage_hook_target(&target)?;
        let result = skillbox_core::install_usage_hook(target)?;
        serde_json::to_value(result).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("Usage hook install task failed: {error}"))?
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

#[tauri::command]
async fn check_app_update(
    app: tauri::AppHandle,
    pending_update: tauri::State<'_, PendingAppUpdate>,
) -> Result<AppUpdateResponse, AppUpdateError> {
    if app_updater_disabled() {
        *pending_update.0.lock().unwrap() = None;
        return Ok(app_update_disabled_response(
            env!("CARGO_PKG_VERSION"),
            "App updater is disabled in development or unsupported builds.",
        ));
    }

    let update = app.updater()?.check().await?;
    match update {
        Some(update) => {
            let response = app_update_response_from_update(&update);
            *pending_update.0.lock().unwrap() = Some(update);
            Ok(response)
        }
        None => {
            *pending_update.0.lock().unwrap() = None;
            Ok(app_update_no_update_response(env!("CARGO_PKG_VERSION")))
        }
    }
}

#[tauri::command]
async fn install_app_update(
    app: tauri::AppHandle,
    pending_update: tauri::State<'_, PendingAppUpdate>,
) -> Result<(), AppUpdateError> {
    if app_updater_disabled() {
        return Err(AppUpdateError::Updater(
            "App updater is disabled in development or unsupported builds.".to_string(),
        ));
    }

    let update = pending_update
        .0
        .lock()
        .unwrap()
        .take()
        .ok_or(AppUpdateError::NoPendingUpdate)?;

    update.download_and_install(|_, _| {}, || {}).await?;
    app.restart();
}

pub fn run() {
    let result = tauri::Builder::default()
        .setup(|app| {
            app.handle()
                .plugin(tauri_plugin_updater::Builder::new().build())?;
            app.manage(PendingAppUpdate::default());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            open_external_url,
            open_local_path,
            open_local_file,
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
            change_skill_kind,
            deploy_skill,
            undeploy_skill,
            parse_github_url,
            install_github_remote_skill,
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
            list_history,
            record_skill_usage,
            usage_hook_statuses,
            install_usage_hook,
            list_workspaces,
            scan_workspaces,
            add_workspace,
            forget_workspace,
            check_app_update,
            install_app_update
        ])
        .run(tauri::generate_context!());

    if let Err(error) = result {
        eprintln!("failed to run SkillBox: {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        validate_external_github_url, validate_local_file_path, validate_local_folder_path,
    };

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

    #[test]
    fn validates_existing_absolute_local_files() {
        let cwd = std::env::current_dir().unwrap();
        let file_path = [
            cwd.join("tauri.conf.json"),
            cwd.join("apps/desktop/src-tauri/tauri.conf.json"),
        ]
        .into_iter()
        .find(|path| path.is_file())
        .unwrap();

        assert_eq!(
            validate_local_file_path(file_path.to_str().unwrap()).unwrap(),
            file_path
        );
    }

    #[test]
    fn rejects_non_local_file_paths() {
        let cwd = std::env::current_dir().unwrap();

        assert!(validate_local_file_path("https://github.com/owner/repo").is_err());
        assert!(validate_local_file_path("relative/path").is_err());
        assert!(validate_local_file_path(cwd.to_str().unwrap()).is_err());
    }

    #[test]
    fn app_update_disabled_response_is_serializable() {
        let response = super::app_update_disabled_response(
            "0.3.0",
            "App updater is disabled in development or unsupported builds.",
        );

        assert_eq!(response.current_version, "0.3.0");
        assert!(!response.available);
        assert!(response.disabled);
        assert_eq!(
            response.message,
            "App updater is disabled in development or unsupported builds."
        );
        assert!(serde_json::to_value(response).unwrap()["currentVersion"].is_string());
    }

    #[test]
    fn app_update_error_serializes_as_a_user_message() {
        assert_eq!(
            serde_json::to_string(&super::AppUpdateError::NoPendingUpdate).unwrap(),
            "\"There is no pending app update to install.\""
        );
    }
}
