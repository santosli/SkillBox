use serde_json::Value;

#[tauri::command]
fn managed_paths() -> Result<Value, String> {
    serde_json::to_value(skillbox_core::managed_paths(
        skillbox_core::default_managed_root(),
    ))
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn scan_skills() -> Result<Value, String> {
    let scan = skillbox_core::scan_skill_roots(&skillbox_core::default_runtime_roots())?;
    serde_json::to_value(scan).map_err(|error| error.to_string())
}

#[tauri::command]
fn parse_github_url(url: String) -> Result<Value, String> {
    let source = skillbox_github::parse_github_skill_url(&url)?;
    serde_json::to_value(source).map_err(|error| error.to_string())
}

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            managed_paths,
            scan_skills,
            parse_github_url
        ])
        .run(tauri::generate_context!())
        .expect("failed to run SkillBox");
}
