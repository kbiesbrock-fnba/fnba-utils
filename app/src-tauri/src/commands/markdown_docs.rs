//! On-disk storage for Markdown Viewer documents. The webview can't touch the
//! filesystem directly, so the frontend round-trips doc bodies through these
//! commands: content lives under `%LOCALAPPDATA%\fnba-utils\markdown-docs\` and
//! localStorage holds only the returned path. `cleanup_markdown_docs` sweeps
//! files orphaned by a crash (no surviving registry entry references them).

use std::collections::HashSet;
use std::path::PathBuf;

use crate::state::paths::markdown_docs_dir;

/// Map any char outside `[A-Za-z0-9._-]` to `_` so a window label
/// (e.g. `markdown-viewer:1718-0`, which contains a `:` illegal on Windows)
/// becomes a safe file stem.
fn sanitize_label(label: &str) -> String {
    label
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Ensure `path` names a `.md` file directly inside the docs dir. Returns the
/// path on success. Guards against traversal (`..`) and writes elsewhere.
fn validate_in_docs_dir(path: &str) -> Result<PathBuf, String> {
    let p = PathBuf::from(path);
    if p.extension().and_then(|e| e.to_str()) != Some("md") {
        return Err("not a markdown document".into());
    }
    let parent = p.parent().ok_or_else(|| "invalid path".to_string())?;
    let canon_parent = parent
        .canonicalize()
        .map_err(|e| format!("invalid path: {e}"))?;
    let canon_dir = markdown_docs_dir()
        .canonicalize()
        .map_err(|e| format!("docs dir error: {e}"))?;
    if canon_parent != canon_dir {
        return Err("path is outside the markdown-docs directory".into());
    }
    Ok(p)
}

/// Write `content` to `<docs-dir>/<sanitized-label>.md`, returning the absolute
/// path. Re-writing the same label overwrites that doc's file.
#[tauri::command]
pub fn write_markdown_doc(label: String, content: String) -> Result<String, String> {
    let path = markdown_docs_dir().join(format!("{}.md", sanitize_label(&label)));
    std::fs::write(&path, content).map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().into_owned())
}

/// Read a doc body; a missing file yields an empty string (the doc was never
/// persisted, or was cleaned up).
#[tauri::command]
pub fn read_markdown_doc(path: String) -> Result<String, String> {
    let p = validate_in_docs_dir(&path)?;
    match std::fs::read_to_string(&p) {
        Ok(s) => Ok(s),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(e.to_string()),
    }
}

/// Delete a doc file (idempotent — a missing file is success).
#[tauri::command]
pub fn delete_markdown_doc(path: String) -> Result<(), String> {
    let p = validate_in_docs_dir(&path)?;
    match std::fs::remove_file(&p) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

/// Delete every `.md` in the docs dir whose canonical path is NOT in
/// `keep_paths`. Called at startup with the paths still referenced by the
/// viewer registry, so crash-orphaned docs get reclaimed. Returns the count
/// removed.
#[tauri::command]
pub fn cleanup_markdown_docs(keep_paths: Vec<String>) -> Result<u32, String> {
    let dir = markdown_docs_dir();
    let keep: HashSet<PathBuf> = keep_paths
        .iter()
        .filter_map(|p| PathBuf::from(p).canonicalize().ok())
        .collect();
    let mut removed = 0u32;
    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(e) => return Err(e.to_string()),
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let canon = path.canonicalize().unwrap_or_else(|_| path.clone());
        if !keep.contains(&canon) {
            if std::fs::remove_file(&path).is_ok() {
                removed += 1;
            }
        }
    }
    Ok(removed)
}
