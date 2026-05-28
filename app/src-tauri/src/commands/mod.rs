pub mod assume_identity;
pub mod claude_io;
pub mod clipboard_manager;
pub mod directory;
pub mod mission_control;
pub mod projects;
pub mod standup;

#[tauri::command]
pub async fn hide_window(window: tauri::WebviewWindow) {
    let _ = window.hide();
}

/// Open `%LOCALAPPDATA%\fnba-utils\` in Windows Explorer. Wired to the
/// settings cog on the command palette so users can inspect / back up /
/// hand-edit `config.yaml`, `assumeIdentity.json`, etc. without hunting for
/// the path.
#[tauri::command]
pub async fn open_app_data_folder() -> Result<(), String> {
    let path = crate::state::paths::app_data_dir();
    std::process::Command::new("explorer.exe")
        .arg(&path)
        .spawn()
        .map_err(|e| format!("Failed to open {}: {e}", path.display()))?;
    Ok(())
}
