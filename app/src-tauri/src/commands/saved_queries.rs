//! CRUD over the SQLite-backed registry of saved SQL queries and groups.
//! The frontend `useSqlQuery` composable invokes these.
//!
//! Mutating commands emit `sql-queries-changed` so every open SQL panel
//! refreshes its local cache — the data is global, not per-connection.

use crate::state::saved_queries::{
    LegacySavedQuery, SavedQueriesState, SavedSqlQuery, SqlGroup,
};
use tauri::{AppHandle, Emitter};

const CHANGED_EVENT: &str = "sql-queries-changed";

fn emit_changed(app: &AppHandle) {
    let _ = app.emit(CHANGED_EVENT, ());
}

// ---------- Groups ----------

#[tauri::command]
pub async fn list_sql_groups(
    state: tauri::State<'_, SavedQueriesState>,
) -> Result<Vec<SqlGroup>, String> {
    state.list_groups()
}

#[tauri::command]
pub async fn add_sql_group(
    name: String,
    app: AppHandle,
    state: tauri::State<'_, SavedQueriesState>,
) -> Result<SqlGroup, String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("Group name cannot be empty".into());
    }
    let group = state.add_group(name)?;
    emit_changed(&app);
    Ok(group)
}

#[tauri::command]
pub async fn rename_sql_group(
    id: String,
    name: String,
    app: AppHandle,
    state: tauri::State<'_, SavedQueriesState>,
) -> Result<(), String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("Group name cannot be empty".into());
    }
    state.rename_group(&id, &name)?;
    emit_changed(&app);
    Ok(())
}

#[tauri::command]
pub async fn set_sql_group_color(
    id: String,
    color: Option<String>,
    app: AppHandle,
    state: tauri::State<'_, SavedQueriesState>,
) -> Result<(), String> {
    let color = color.and_then(|c| {
        let trimmed = c.trim().to_string();
        if trimmed.is_empty() { None } else { Some(trimmed) }
    });
    state.set_group_color(&id, color.as_deref())?;
    emit_changed(&app);
    Ok(())
}

#[tauri::command]
pub async fn set_sql_group_pinned(
    id: String,
    pinned: bool,
    app: AppHandle,
    state: tauri::State<'_, SavedQueriesState>,
) -> Result<(), String> {
    state.set_group_pinned(&id, pinned)?;
    emit_changed(&app);
    Ok(())
}

#[tauri::command]
pub async fn reorder_sql_groups(
    ids: Vec<String>,
    app: AppHandle,
    state: tauri::State<'_, SavedQueriesState>,
) -> Result<(), String> {
    state.reorder_groups(&ids)?;
    emit_changed(&app);
    Ok(())
}

#[tauri::command]
pub async fn remove_sql_group(
    id: String,
    app: AppHandle,
    state: tauri::State<'_, SavedQueriesState>,
) -> Result<bool, String> {
    let removed = state.remove_group(&id)?;
    if removed {
        emit_changed(&app);
    }
    Ok(removed)
}

// ---------- Queries ----------

#[tauri::command]
pub async fn list_sql_queries(
    state: tauri::State<'_, SavedQueriesState>,
) -> Result<Vec<SavedSqlQuery>, String> {
    state.list_queries()
}

#[tauri::command]
pub async fn add_sql_query(
    name: String,
    sql: String,
    database: String,
    group_id: Option<String>,
    app: AppHandle,
    state: tauri::State<'_, SavedQueriesState>,
) -> Result<SavedSqlQuery, String> {
    let name = name.trim().to_string();
    let sql = sql.trim().to_string();
    if name.is_empty() {
        return Err("Query name cannot be empty".into());
    }
    if sql.is_empty() {
        return Err("Query SQL cannot be empty".into());
    }
    let saved = state.add_query(name, sql, database, group_id)?;
    emit_changed(&app);
    Ok(saved)
}

#[tauri::command]
pub async fn update_sql_query(
    id: String,
    name: String,
    sql: String,
    database: String,
    app: AppHandle,
    state: tauri::State<'_, SavedQueriesState>,
) -> Result<(), String> {
    let name = name.trim().to_string();
    let sql = sql.trim().to_string();
    if name.is_empty() {
        return Err("Query name cannot be empty".into());
    }
    if sql.is_empty() {
        return Err("Query SQL cannot be empty".into());
    }
    state.update_query(&id, &name, &sql, &database)?;
    emit_changed(&app);
    Ok(())
}

#[tauri::command]
pub async fn move_sql_query_to_group(
    id: String,
    group_id: Option<String>,
    app: AppHandle,
    state: tauri::State<'_, SavedQueriesState>,
) -> Result<(), String> {
    state.move_query_to_group(&id, group_id.as_deref())?;
    emit_changed(&app);
    Ok(())
}

#[tauri::command]
pub async fn remove_sql_query(
    id: String,
    app: AppHandle,
    state: tauri::State<'_, SavedQueriesState>,
) -> Result<bool, String> {
    let removed = state.remove_query(&id)?;
    if removed {
        emit_changed(&app);
    }
    Ok(removed)
}

#[tauri::command]
pub async fn record_sql_query_used(
    id: String,
    app: AppHandle,
    state: tauri::State<'_, SavedQueriesState>,
) -> Result<(), String> {
    state.record_used(&id)?;
    emit_changed(&app);
    Ok(())
}

#[tauri::command]
pub async fn migrate_legacy_sql_queries(
    entries: Vec<LegacySavedQuery>,
    app: AppHandle,
    state: tauri::State<'_, SavedQueriesState>,
) -> Result<u32, String> {
    let migrated = state.migrate_legacy(entries)?;
    if migrated > 0 {
        emit_changed(&app);
    }
    Ok(migrated)
}
