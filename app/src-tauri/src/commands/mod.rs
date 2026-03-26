pub mod assume_identity;
pub mod right_lookup;

#[tauri::command]
pub async fn hide_window(window: tauri::WebviewWindow) {
    let _ = window.hide();
}
