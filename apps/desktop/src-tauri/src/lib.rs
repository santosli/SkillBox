use serde_json::Value;

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
fn import_candidates(items: Vec<skillbox_core::ImportRequestItem>) -> Result<Value, String> {
    let result = skillbox_core::import_candidates(items, skillbox_core::default_managed_root())?;
    serde_json::to_value(result).map_err(|error| error.to_string())
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
fn sync_user_skills_git(request: skillbox_core::UserSkillsSyncRequest) -> Result<Value, String> {
    let result =
        skillbox_core::sync_user_skills_git(request, skillbox_core::default_managed_root())?;
    serde_json::to_value(result).map_err(|error| error.to_string())
}

#[tauri::command]
fn check_remote_skill_updates() -> Result<Value, String> {
    let result = skillbox_core::check_remote_skill_updates(skillbox_core::default_managed_root())?;
    serde_json::to_value(result).map_err(|error| error.to_string())
}

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            managed_paths,
            managed_state,
            managed_preferences,
            set_skip_local_import_confirmation,
            set_status_refresh_interval_minutes,
            scan_skills,
            scan_import_candidates,
            import_candidates,
            parse_github_url,
            user_skills_git_status,
            user_skills_git_changes,
            set_user_skills_git_remote,
            sync_user_skills_git,
            check_remote_skill_updates
        ])
        .run(tauri::generate_context!())
        .expect("failed to run SkillBox");
}
