//! On-disk storage for File Viewer documents (JSON and Markdown). The webview
//! can't touch the filesystem directly, so the frontend round-trips doc
//! bodies through these commands: content lives under
//! `%LOCALAPPDATA%\fnba-utils\markdown-docs\` (shared by both kinds — see
//! `viewer_docs_dir()`'s doc comment for why the folder name didn't change)
//! and localStorage holds only the returned path. `cleanup_viewer_docs`
//! sweeps files orphaned by a crash (no surviving registry entry references
//! them).

use std::collections::HashSet;
use std::path::PathBuf;

use tauri_plugin_dialog::DialogExt;

use crate::state::paths::viewer_docs_dir;

/// Which viewer a doc/dialog operation is for. An unrecognized string coming
/// over IPC becomes a deserialization error rather than silently falling
/// through to a default kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ViewerKind {
    Json,
    Markdown,
}

impl ViewerKind {
    fn doc_extension(self) -> &'static str {
        match self {
            ViewerKind::Json => "json",
            ViewerKind::Markdown => "md",
        }
    }
}

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

/// Ensure `path` names a doc file (`.md` or `.json`) directly inside the docs
/// dir. Returns the path on success. Guards against traversal (`..`) and
/// writes elsewhere.
fn validate_in_docs_dir(path: &str) -> Result<PathBuf, String> {
    let p = PathBuf::from(path);
    match p.extension().and_then(|e| e.to_str()) {
        Some("md") | Some("json") => {}
        _ => return Err("not a viewer document".into()),
    }
    let parent = p.parent().ok_or_else(|| "invalid path".to_string())?;
    let canon_parent = parent
        .canonicalize()
        .map_err(|e| format!("invalid path: {e}"))?;
    let canon_dir = viewer_docs_dir()
        .canonicalize()
        .map_err(|e| format!("docs dir error: {e}"))?;
    if canon_parent != canon_dir {
        return Err("path is outside the markdown-docs directory".into());
    }
    Ok(p)
}

/// Write `content` to `<docs-dir>/<sanitized-label>.<ext>`, returning the
/// absolute path. Re-writing the same label overwrites that doc's file.
#[tauri::command]
pub fn write_viewer_doc(label: String, kind: ViewerKind, content: String) -> Result<String, String> {
    let path = viewer_docs_dir().join(format!("{}.{}", sanitize_label(&label), kind.doc_extension()));
    std::fs::write(&path, content).map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().into_owned())
}

/// Read a doc body; a missing file yields an empty string (the doc was never
/// persisted, or was cleaned up).
#[tauri::command]
pub fn read_viewer_doc(path: String) -> Result<String, String> {
    let p = validate_in_docs_dir(&path)?;
    match std::fs::read_to_string(&p) {
        Ok(s) => Ok(s),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(e.to_string()),
    }
}

/// Delete a doc file (idempotent — a missing file is success).
#[tauri::command]
pub fn delete_viewer_doc(path: String) -> Result<(), String> {
    let p = validate_in_docs_dir(&path)?;
    match std::fs::remove_file(&p) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

/// Delete every doc file (`.md` or `.json`) in the docs dir whose canonical
/// path is NOT in `keep_paths`. Called at startup with the paths still
/// referenced by the viewer registry, so crash-orphaned docs of either kind
/// get reclaimed. Returns the count removed.
#[tauri::command]
pub fn cleanup_viewer_docs(keep_paths: Vec<String>) -> Result<u32, String> {
    let dir = viewer_docs_dir();
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
        match path.extension().and_then(|e| e.to_str()) {
            Some("md") | Some("json") => {}
            _ => continue,
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

// ---------------------------------------------------------------------------
// Real-file commands — native dialog → user-chosen Windows path
// ---------------------------------------------------------------------------

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewerFile {
    pub path: String,
    pub content: String,
}

/// Dialog title/filter-name/extensions for a given kind's Open and Save-As
/// pickers. Markdown's values are exactly today's (open allows a broader set
/// including plain `.txt`; save is narrower). JSON gets one extension set
/// (`json`, `jsonc`) shared by both pickers.
struct DialogSpec {
    open_title: &'static str,
    open_extensions: &'static [&'static str],
    save_title: &'static str,
    save_extensions: &'static [&'static str],
    filter_name: &'static str,
}

fn dialog_spec(kind: ViewerKind) -> DialogSpec {
    match kind {
        ViewerKind::Markdown => DialogSpec {
            open_title: "Open Markdown file",
            open_extensions: &["md", "markdown", "mdown", "mkd", "txt"],
            save_title: "Save Markdown as",
            save_extensions: &["md", "markdown"],
            filter_name: "Markdown",
        },
        ViewerKind::Json => DialogSpec {
            open_title: "Open JSON file",
            open_extensions: &["json", "jsonc"],
            save_title: "Save JSON file as",
            save_extensions: &["json", "jsonc"],
            filter_name: "JSON",
        },
    }
}

/// Native "open file" dialog → read the chosen file. Returns None if the user
/// cancels. The path is the real Windows path (not a scratch doc).
#[tauri::command]
pub async fn open_viewer_file(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    kind: ViewerKind,
) -> Result<Option<ViewerFile>, String> {
    let spec = dialog_spec(kind);
    let (tx, rx) = tokio::sync::oneshot::channel::<Option<PathBuf>>();
    app.dialog()
        .file()
        .set_title(spec.open_title)
        .add_filter(spec.filter_name, spec.open_extensions)
        .set_parent(&window)
        .pick_file(move |p| {
            let _ = tx.send(p.and_then(|p| p.into_path().ok()));
        });
    let picked = rx.await.map_err(|e| format!("Picker dropped: {e}"))?;
    match picked {
        None => Ok(None),
        Some(path) => {
            let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
            Ok(Some(ViewerFile {
                path: path.to_string_lossy().into_owned(),
                content,
            }))
        }
    }
}

/// Native "save as" dialog → write `content` to the chosen path. Returns the
/// chosen path (so the caller can bind the window to it) or None on cancel.
#[tauri::command]
pub async fn save_viewer_file_as(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    kind: ViewerKind,
    content: String,
    suggested_name: Option<String>,
) -> Result<Option<String>, String> {
    let spec = dialog_spec(kind);
    let (tx, rx) = tokio::sync::oneshot::channel::<Option<PathBuf>>();
    let mut builder = app
        .dialog()
        .file()
        .set_title(spec.save_title)
        .add_filter(spec.filter_name, spec.save_extensions)
        .set_parent(&window);
    if let Some(name) = suggested_name {
        builder = builder.set_file_name(name);
    }
    builder.save_file(move |p| {
        let _ = tx.send(p.and_then(|p| p.into_path().ok()));
    });
    let chosen = rx.await.map_err(|e| format!("Picker dropped: {e}"))?;
    match chosen {
        None => Ok(None),
        Some(path) => {
            std::fs::write(&path, content).map_err(|e| e.to_string())?;
            Ok(Some(path.to_string_lossy().into_owned()))
        }
    }
}

/// Write `content` through to an already-bound real file path (Ctrl+S on a
/// document the user previously opened or saved-as). Plain write to the given
/// path — the path originates from a prior native dialog selection.
#[tauri::command]
pub fn save_viewer_file(path: String, content: String) -> Result<(), String> {
    std::fs::write(&path, content).map_err(|e| e.to_string())
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewerFileStat {
    /// Modification time in epoch milliseconds (0 if unavailable).
    pub mtime_ms: f64,
    pub size: u64,
    pub exists: bool,
}

fn mtime_ms(meta: &std::fs::Metadata) -> f64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as f64)
        .unwrap_or(0.0)
}

/// Lightweight disk fingerprint for a bound file. `exists: false` means the
/// file was removed/renamed externally. Used on window focus to detect that
/// another program changed the file underneath us.
#[tauri::command]
pub fn stat_viewer_file(path: String) -> ViewerFileStat {
    match std::fs::metadata(&path) {
        Ok(m) => ViewerFileStat { mtime_ms: mtime_ms(&m), size: m.len(), exists: true },
        Err(_) => ViewerFileStat { mtime_ms: 0.0, size: 0, exists: false },
    }
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewerFileRead {
    pub content: String,
    pub mtime_ms: f64,
    pub size: u64,
}

/// Re-read a bound file's current contents plus its fingerprint, so a "reload
/// from disk" updates both the buffer and the change-detection baseline atomically.
#[tauri::command]
pub fn read_viewer_file(path: String) -> Result<ViewerFileRead, String> {
    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let m = std::fs::metadata(&path).map_err(|e| e.to_string())?;
    Ok(ViewerFileRead { content, mtime_ms: mtime_ms(&m), size: m.len() })
}
