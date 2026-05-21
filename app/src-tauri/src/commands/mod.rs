pub mod assume_identity;
pub mod claude_io;
pub mod clipboard_manager;
pub mod mission_control;
pub mod projects;
pub mod right_lookup;
pub mod standup;

#[tauri::command]
pub async fn hide_window(window: tauri::WebviewWindow) {
    let _ = window.hide();
}
