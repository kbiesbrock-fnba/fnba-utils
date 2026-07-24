//! Filesystem-backed SQL query library.
//!
//! Saved queries live as `.sql` files under a root directory the user sets in
//! `config.yaml` (`sql_library.root`); subdirectories are the headings (nesting
//! arbitrary). The root is typically a WSL UNC path (`\\wsl$\Ubuntu\home\…\sql`
//! / `\\wsl.localhost\…`) or a plain drive path — `std::fs` handles all three
//! directly on Windows.
//!
//! The root is re-read from `config.yaml` on EVERY library call (see
//! [`require_root`]), so editing the file + hitting Refresh in the panel picks
//! up a new root with no app restart. The app never writes `config.yaml`.
//!
//! ## Path jail (security-critical)
//! Every relative path the frontend supplies is validated component-wise by
//! [`jail_join`] BEFORE being joined to the root. `canonicalize`-prefix checks
//! are deliberately NOT the primary gate: `canonicalize` on a UNC path yields
//! `\\?\UNC\…` forms and fails outright on not-yet-existing write targets. The
//! component walk rejects absolute paths, drive letters, UNC prefixes, `..`
//! traversal, and any hidden (`.`-prefixed) or Windows-illegal component.
//!
//! ## Error codes
//! Commands return `Err(String)` with a stable prefix the frontend branches on:
//! - `no-root`            — no root configured
//! - `unreachable: <msg>` — the root dir couldn't be read (WSL down, share gone)
//! - `path: <msg>`        — a jail / validation violation
//! - `io: <msg>`          — a per-file IO error

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::State;

use crate::config::AppConfig;
use crate::state::saved_queries::SavedQueriesState;
use crate::state::sql_library::SqlLibraryState;
use crate::util::paths::{wsl_home, wsl_path_to_windows};

/// Recursion / size guards for the tree walk.
const MAX_DEPTH: usize = 12;
const MAX_ENTRIES: usize = 5000;
/// Cap on how long a (possibly cold 9p) walk may run before we call the share
/// unreachable rather than hang the panel.
const WALK_TIMEOUT_SECS: u64 = 20;

/// Chars illegal in a Windows filename component (also illegal-ish on Linux for
/// `/`). Used both to reject supplied paths and to sanitize export names.
const ILLEGAL: &[char] = &['\\', '/', ':', '*', '?', '"', '<', '>', '|'];

// ─────────────────────────────────────────────────────────────────────────────
// Tree JSON shape
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SqlTreeNode {
    /// Display name: directory name for dirs; filename sans `.sql` for files.
    pub name: String,
    /// Path relative to the root, forward-slashed (e.g. `Projects/MIN-487/foo`).
    pub rel_path: String,
    pub is_dir: bool,
    /// Present only for directories.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<SqlTreeNode>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SqlLibraryTree {
    /// The configured root (Windows-reachable form).
    pub root: String,
    /// True once the entry cap tripped and the walk stopped early.
    pub truncated: bool,
    /// Top-level entries (children of the root), dirs first then files, alpha.
    pub entries: Vec<SqlTreeNode>,
}

/// Lightweight status the panel reads to decide setup-notice vs. tree, show the
/// configured root in the header, and open `config.yaml`.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SqlLibraryInfo {
    /// Normalized root from `config.yaml`, or `None` when the key is unset.
    pub root: Option<String>,
    /// One-time export stamp (epoch ms), or `None` if it hasn't run.
    pub exported_at: Option<i64>,
    /// Absolute path to `config.yaml` (for the "Open config" affordance).
    pub config_path: Option<String>,
}

fn epoch_ms_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

// ─────────────────────────────────────────────────────────────────────────────
// Root normalization
// ─────────────────────────────────────────────────────────────────────────────

/// Turn whatever the user typed/picked into a Windows-reachable root path.
///
/// - posix (`/home/…`, `/mnt/c/…`) and `~`-relative → `wsl_path_to_windows`
///   (`~` needs a live WSL to expand `$HOME`).
/// - drive paths (`C:\…`) and UNC (`\\wsl$\…`, `\\wsl.localhost\…`) pass
///   through unchanged — `std::fs` opens them directly.
fn normalize_root(input: &str) -> Result<String, String> {
    let raw = input.trim();
    if raw.is_empty() {
        return Err("path: root path is empty".into());
    }
    if raw.starts_with('/') {
        return Ok(wsl_path_to_windows(raw));
    }
    if raw == "~" || raw.starts_with("~/") {
        return match wsl_home() {
            Some(home) => {
                let posix = if raw == "~" {
                    home
                } else {
                    format!("{home}/{}", &raw[2..])
                };
                Ok(wsl_path_to_windows(&posix))
            }
            None => Err("unreachable: cannot resolve ~ (is WSL running?)".into()),
        };
    }
    // Windows drive path or UNC — already reachable.
    Ok(raw.to_string())
}

// ─────────────────────────────────────────────────────────────────────────────
// Path jail
// ─────────────────────────────────────────────────────────────────────────────

/// Validate a frontend-supplied relative path and join it under `root`.
///
/// Component-wise validation is the primary (and only trusted) gate — see the
/// module docs on why canonicalize-prefix comparison is unreliable for UNC and
/// not-yet-existing write targets. When `require_sql` is set the final segment
/// must carry a `.sql` extension (case-insensitive).
fn jail_join(root: &Path, rel: &str, require_sql: bool) -> Result<PathBuf, String> {
    let normalized = rel.replace('\\', "/");
    let normalized = normalized.trim();
    if normalized.is_empty() {
        return Err("path: empty relative path".into());
    }
    if normalized.starts_with("//") {
        return Err("path: UNC paths are not allowed".into());
    }
    if normalized.starts_with('/') {
        return Err("path: absolute paths are not allowed".into());
    }
    // Drive-letter prefix, e.g. `C:` / `c:/…`.
    let bytes = normalized.as_bytes();
    if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        return Err("path: drive letters are not allowed".into());
    }

    let mut out = root.to_path_buf();
    for comp in normalized.split('/') {
        if comp.is_empty() || comp == "." {
            return Err("path: invalid empty or '.' segment".into());
        }
        if comp == ".." {
            return Err("path: parent traversal is not allowed".into());
        }
        // Hidden entries are never surfaced by the tree, so a legitimate op
        // never targets one — reject uniformly (stricter than write-only).
        if comp.starts_with('.') {
            return Err("path: hidden names are not allowed".into());
        }
        if comp.chars().any(|c| ILLEGAL.contains(&c) || (c as u32) < 0x20) {
            return Err("path: illegal characters in path segment".into());
        }
        if comp.ends_with('.') || comp.ends_with(' ') {
            return Err("path: segment cannot end with '.' or space".into());
        }
        out.push(comp);
    }

    if require_sql {
        let ok = out
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("sql"))
            .unwrap_or(false);
        if !ok {
            return Err("path: file must have a .sql extension".into());
        }
    }
    Ok(out)
}

/// Re-read `config.yaml` and resolve `sql_library.root` to a Windows-reachable
/// `PathBuf`. `no-root` when the key is unset; normalization errors (e.g. `~`
/// with WSL down) propagate as `unreachable:`. Reading fresh each call is what
/// lets an edited config.yaml take effect on the next Refresh.
fn require_root() -> Result<PathBuf, String> {
    match AppConfig::load().sql_library_root() {
        Some(raw) => Ok(PathBuf::from(normalize_root(&raw)?)),
        None => Err("no-root".to_string()),
    }
}

/// Walk the root on a blocking worker, bounded by a timeout so a cold 9p share
/// can't hang the panel. Returns the top-level entries + the `truncated` flag,
/// or a coded `unreachable:` error.
async fn walk_root(root: PathBuf) -> Result<(Vec<SqlTreeNode>, bool), String> {
    let walk = tauri::async_runtime::spawn_blocking(move || {
        let mut budget = MAX_ENTRIES;
        let mut truncated = false;
        walk_dir(&root, "", 0, &mut budget, &mut truncated).map(|entries| (entries, truncated))
    });
    match tokio::time::timeout(std::time::Duration::from_secs(WALK_TIMEOUT_SECS), walk).await {
        Ok(Ok(Ok(pair))) => Ok(pair),
        Ok(Ok(Err(e))) => Err(format!("unreachable: {e}")),
        Ok(Err(e)) => Err(format!("unreachable: walk task failed: {e}")),
        Err(_) => Err("unreachable: timed out reading the library (is WSL running?)".into()),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tree walk
// ─────────────────────────────────────────────────────────────────────────────

/// Recursively collect dirs + `.sql` files under `dir`. Skips dot-entries and
/// prunes directories that contain no `.sql` anywhere beneath them. `budget` is
/// the remaining entry allowance; when it hits zero the walk stops and
/// `truncated` is set.
fn walk_dir(
    dir: &Path,
    rel_prefix: &str,
    depth: usize,
    budget: &mut usize,
    truncated: &mut bool,
) -> std::io::Result<Vec<SqlTreeNode>> {
    if depth >= MAX_DEPTH {
        return Ok(Vec::new());
    }
    let mut nodes: Vec<SqlTreeNode> = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        if *budget == 0 {
            *truncated = true;
            break;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        // `file_type()` does NOT follow symlinks, so a symlinked dir reads as a
        // symlink (neither is_dir nor is_file) and is skipped — cheap cycle guard.
        let ft = match entry.file_type() {
            Ok(t) => t,
            Err(_) => continue,
        };
        let rel = if rel_prefix.is_empty() {
            name.clone()
        } else {
            format!("{rel_prefix}/{name}")
        };
        if ft.is_dir() {
            let children = walk_dir(&entry.path(), &rel, depth + 1, budget, truncated)?;
            if children.is_empty() {
                // No `.sql` beneath — prune the heading entirely.
                continue;
            }
            *budget = budget.saturating_sub(1);
            nodes.push(SqlTreeNode {
                name,
                rel_path: rel,
                is_dir: true,
                children: Some(children),
            });
        } else if ft.is_file() {
            if !name.to_ascii_lowercase().ends_with(".sql") {
                continue;
            }
            *budget = budget.saturating_sub(1);
            let display = name[..name.len() - 4].to_string();
            nodes.push(SqlTreeNode {
                name: display,
                rel_path: rel,
                is_dir: false,
                children: None,
            });
        }
    }
    // Dirs first, then files; case-insensitive alpha within each.
    nodes.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a
            .name
            .to_ascii_lowercase()
            .cmp(&b.name.to_ascii_lowercase()),
    });
    Ok(nodes)
}

// ─────────────────────────────────────────────────────────────────────────────
// Export migration (one-time)
// ─────────────────────────────────────────────────────────────────────────────

/// Sanitize a group / query name into a single safe path component: drop
/// illegal chars, collapse whitespace, trim leading dots (would hide) and
/// trailing dots/spaces (illegal on Windows).
fn sanitize_component(name: &str) -> String {
    let stripped: String = name
        .chars()
        .filter(|c| !ILLEGAL.contains(c) && (*c as u32) >= 0x20)
        .collect();
    let collapsed = stripped.split_whitespace().collect::<Vec<_>>().join(" ");
    collapsed
        .trim_matches(|c: char| c == '.' || c == ' ')
        .to_string()
}

/// Pick a non-colliding `<base>.sql` (or `<base> (n).sql`) filename in `dir`,
/// considering both files already on disk and names claimed earlier this run.
fn unique_sql_name(dir: &Path, base: &str, used: &mut HashSet<String>) -> String {
    let mut candidate = format!("{base}.sql");
    let mut n = 2;
    while used.contains(&candidate.to_ascii_lowercase()) || dir.join(&candidate).exists() {
        candidate = format!("{base} ({n}).sql");
        n += 1;
    }
    used.insert(candidate.to_ascii_lowercase());
    candidate
}

/// Export every query in the current SQLite saved-query store to
/// `<root>/<group>/<name>.sql` (root-level when ungrouped). Best-effort: a
/// per-file failure is logged and skipped so one bad name can't abort the run.
/// Returns the count written. Nothing is deleted from the old store.
fn export_saved_queries(root: &Path, queries: &SavedQueriesState) -> Result<u32, String> {
    let groups = queries.list_groups()?;
    let group_names: HashMap<String, String> = groups
        .into_iter()
        .map(|g| (g.id, g.name))
        .collect();
    let all = queries.list_queries()?;

    // Track claimed names per destination dir for collision resolution.
    let mut used_by_dir: HashMap<PathBuf, HashSet<String>> = HashMap::new();
    let mut count = 0u32;

    for q in all {
        let dir = match q
            .group_id
            .as_ref()
            .and_then(|id| group_names.get(id))
            .map(|n| sanitize_component(n))
            .filter(|n| !n.is_empty())
        {
            Some(group_dir) => root.join(group_dir),
            None => root.to_path_buf(),
        };
        if let Err(e) = std::fs::create_dir_all(&dir) {
            eprintln!("sql_library export: mkdir {} failed: {e}", dir.display());
            continue;
        }
        let mut base = sanitize_component(&q.name);
        if base.is_empty() {
            base = "query".to_string();
        }
        let used = used_by_dir.entry(dir.clone()).or_default();
        let fname = unique_sql_name(&dir, &base, used);
        let path = dir.join(&fname);
        match std::fs::write(&path, &q.sql) {
            Ok(()) => count += 1,
            Err(e) => eprintln!("sql_library export: write {} failed: {e}", path.display()),
        }
    }
    Ok(count)
}

// ─────────────────────────────────────────────────────────────────────────────
// Commands
// ─────────────────────────────────────────────────────────────────────────────

/// Library status: the config-derived root (or `None`), the export stamp, and
/// the `config.yaml` path. Re-reads `config.yaml` each call.
#[tauri::command]
pub async fn get_sql_library(lib: State<'_, SqlLibraryState>) -> Result<SqlLibraryInfo, String> {
    // Normalize for display when possible; fall back to the raw configured
    // value if normalization can't complete (e.g. `~` while WSL is down) so the
    // header still shows what the user set.
    let root = AppConfig::load()
        .sql_library_root()
        .map(|raw| normalize_root(&raw).unwrap_or(raw));
    Ok(SqlLibraryInfo {
        root,
        exported_at: lib.exported_at(),
        config_path: AppConfig::config_path().map(|p| p.to_string_lossy().into_owned()),
    })
}

/// Recursive walk of the config-defined root → nested tree of dirs + `.sql`
/// files.
///
/// The one-time legacy export runs LAZILY here: the first time the tree loads
/// successfully (root configured AND reachable) with no export stamp yet, the
/// SQLite saved-query store is exported into the root, stamped, and the tree is
/// re-walked so the fresh files show immediately. Keyed on the stamp, so it
/// runs once ever and never again on a root change.
#[tauri::command]
pub async fn sql_library_tree(
    lib: State<'_, SqlLibraryState>,
    queries: State<'_, SavedQueriesState>,
) -> Result<SqlLibraryTree, String> {
    let root = require_root()?;
    let root_str = root.to_string_lossy().into_owned();

    // Walk first — success is our proof the root is reachable.
    let (entries, truncated) = walk_root(root.clone()).await?;

    if lib.exported_at().is_none() {
        // Reachable + never exported → run the one-time export, then re-walk so
        // the exported files are in the returned tree. `SavedQueriesState` holds
        // a Mutex<Connection> that isn't Send across spawn_blocking, so the
        // export runs inline (it only writes small files).
        match export_saved_queries(&root, queries.inner()) {
            Ok(n) => eprintln!("sql_library: exported {n} legacy queries into {}", root.display()),
            Err(e) => eprintln!("sql_library: legacy export error (stamping anyway): {e}"),
        }
        let _ = lib.mark_exported(epoch_ms_now());
        if let Ok((entries2, truncated2)) = walk_root(root).await {
            return Ok(SqlLibraryTree {
                root: root_str,
                truncated: truncated2,
                entries: entries2,
            });
        }
    }

    Ok(SqlLibraryTree {
        root: root_str,
        truncated,
        entries,
    })
}

/// Read a `.sql` file's contents.
#[tauri::command]
pub async fn sql_library_read(rel: String) -> Result<String, String> {
    let root = require_root()?;
    let path = jail_join(&root, &rel, true)?;
    let p = path.clone();
    let read = tauri::async_runtime::spawn_blocking(move || std::fs::read_to_string(&p))
        .await
        .map_err(|e| format!("io: read task failed: {e}"))?;
    read.map_err(classify_io)
}

/// Write `content` to a `.sql` file, creating parent dirs within the root.
#[tauri::command]
pub async fn sql_library_write(rel: String, content: String) -> Result<(), String> {
    let root = require_root()?;
    let path = jail_join(&root, &rel, true)?;
    let p = path.clone();
    tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).map_err(classify_io)?;
        }
        std::fs::write(&p, content.as_bytes()).map_err(classify_io)
    })
    .await
    .map_err(|e| format!("io: write task failed: {e}"))?
}

/// Create a directory (and any missing parents) within the root.
#[tauri::command]
pub async fn sql_library_mkdir(rel: String) -> Result<(), String> {
    let root = require_root()?;
    let path = jail_join(&root, &rel, false)?;
    let p = path.clone();
    tauri::async_runtime::spawn_blocking(move || std::fs::create_dir_all(&p).map_err(classify_io))
        .await
        .map_err(|e| format!("io: mkdir task failed: {e}"))?
}

/// Delete a `.sql` file or an EMPTY directory. Non-empty dirs are rejected.
#[tauri::command]
pub async fn sql_library_delete(rel: String) -> Result<(), String> {
    let root = require_root()?;
    let path = jail_join(&root, &rel, false)?;
    let p = path.clone();
    tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
        let meta = std::fs::symlink_metadata(&p).map_err(classify_io)?;
        if meta.is_dir() {
            // remove_dir refuses a non-empty dir — exactly the guard we want.
            // Probe for entries to give a clear message rather than matching on
            // the (version-dependent) ErrorKind for "not empty".
            std::fs::remove_dir(&p).map_err(|e| {
                let non_empty = std::fs::read_dir(&p)
                    .map(|mut it| it.next().is_some())
                    .unwrap_or(false);
                if non_empty {
                    "io: directory is not empty".to_string()
                } else {
                    classify_io(e)
                }
            })
        } else {
            std::fs::remove_file(&p).map_err(classify_io)
        }
    })
    .await
    .map_err(|e| format!("io: delete task failed: {e}"))?
}

/// Rename/move a file or directory to `new_rel` within the root. When the
/// source is a file, the target must keep a `.sql` extension.
#[tauri::command]
pub async fn sql_library_rename(rel: String, new_rel: String) -> Result<(), String> {
    let root = require_root()?;
    let from = jail_join(&root, &rel, false)?;
    let from_meta = std::fs::symlink_metadata(&from).map_err(classify_io)?;
    let is_file = from_meta.is_file();
    let to = jail_join(&root, &new_rel, is_file)?;
    let (f, t) = (from.clone(), to.clone());
    tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
        if t.exists() {
            return Err("io: a file or folder with that name already exists".into());
        }
        if let Some(parent) = t.parent() {
            std::fs::create_dir_all(parent).map_err(classify_io)?;
        }
        std::fs::rename(&f, &t).map_err(classify_io)
    })
    .await
    .map_err(|e| format!("io: rename task failed: {e}"))?
}

/// Map an IO error to a coded string the frontend can branch on. `NotFound`
/// keeps an `io:` prefix (per-file), everything else is treated as a per-file
/// `io:` too — reachability problems surface from the tree walk instead.
fn classify_io(e: std::io::Error) -> String {
    format!("io: {e}")
}
