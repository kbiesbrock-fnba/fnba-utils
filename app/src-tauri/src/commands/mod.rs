pub mod assume_identity;

#[tauri::command]
pub async fn hide_window(window: tauri::WebviewWindow) {
    let _ = window.hide();
}
