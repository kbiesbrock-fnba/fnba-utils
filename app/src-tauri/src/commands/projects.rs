//! CRUD over the persistent project registry. The frontend `useProjects`
//! composable polls these on demand.

use crate::state::projects::{Project, ProjectsState};

#[tauri::command]
pub async fn list_projects(
    state: tauri::State<'_, ProjectsState>,
) -> Result<Vec<Project>, String> {
    Ok(state.list())
}

#[tauri::command]
pub async fn add_project(
    cwd: String,
    display_name: Option<String>,
    pinned: Option<bool>,
    notes: Option<String>,
    state: tauri::State<'_, ProjectsState>,
) -> Result<bool, String> {
    let cwd = cwd.trim().to_string();
    if cwd.is_empty() {
        return Err("Project cwd cannot be empty".into());
    }
    state.upsert(cwd, display_name, pinned, notes)
}

#[tauri::command]
pub async fn update_project(
    cwd: String,
    display_name: Option<String>,
    pinned: Option<bool>,
    notes: Option<String>,
    state: tauri::State<'_, ProjectsState>,
) -> Result<(), String> {
    state.upsert(cwd, display_name, pinned, notes)?;
    Ok(())
}

#[tauri::command]
pub async fn remove_project(
    cwd: String,
    state: tauri::State<'_, ProjectsState>,
) -> Result<bool, String> {
    state.remove(&cwd)
}

/// Bump the project's `last_used_at` to now (creating the entry if missing).
/// Called by the new-session launcher on successful spawn.
#[tauri::command]
pub async fn record_project_used(
    cwd: String,
    state: tauri::State<'_, ProjectsState>,
) -> Result<(), String> {
    state.record_used(&cwd)
}
