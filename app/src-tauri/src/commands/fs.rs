//! Filesystem introspection commands for the soft-command layer.
//! No persistent state; no Tauri manages required.

use crate::util::paths::{windows_path_to_wsl, wsl_home, wsl_path_to_windows};

/// Resolved path returned to the frontend for every path the user types.
///
/// `posix`   — the canonical WSL posix form (~ expanded, /mnt/c/… or /home/…)
/// `windows` — the Windows-reachable open form. Drive-backed paths (`/mnt/<d>/…`
///             and native `C:\…`) resolve to a DIRECT drive path (`C:\…`): the
///             `\\wsl.localhost\Ubuntu\mnt\…` UNC is NOT served by WSL's 9p file
///             server, so explorer/idea can't open it. Genuine WSL paths
///             (`/home/…`) resolve to `\\wsl.localhost\Ubuntu\…`.
/// `exists`/`is_file`/`is_dir` — statted via `windows`.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedPath {
    pub posix: String,
    pub windows: String,
    pub exists: bool,
    pub is_file: bool,
    pub is_dir: bool,
}

/// Resolve any path the user typed in the palette (posix, tilde, or Windows
/// drive form) into the representations the frontend needs.
///
/// 1. Derive the posix form: drive paths (`C:\…`) → `/mnt/<d>/…` via
///    `windows_path_to_wsl`; a leading `~` expands via `wsl_home()`; otherwise
///    the input is already posix.
/// 2. `windows = wsl_path_to_windows(&posix)` — the form Windows can actually
///    open (drive path for `/mnt`, UNC for WSL-native paths).
/// 3. Stat `windows` for existence/type.
///
/// Never errors — a bad path returns exists=false, is_file=false, is_dir=false.
#[tauri::command]
pub fn resolve_path(path: String) -> ResolvedPath {
    let raw = path.trim();

    // Detect a native Windows drive path: `^[A-Za-z]:[\\/]`.
    let is_drive_path = {
        let mut chars = raw.chars();
        let c1 = chars.next().map(|c| c.is_ascii_alphabetic()).unwrap_or(false);
        let c2 = chars.next().map(|c| c == ':').unwrap_or(false);
        let c3 = chars.next().map(|c| c == '/' || c == '\\').unwrap_or(false);
        c1 && c2 && c3
    };

    let posix = if is_drive_path {
        windows_path_to_wsl(raw)
    } else if raw == "~" {
        wsl_home().unwrap_or_else(|| raw.to_string())
    } else if let Some(suffix) = raw.strip_prefix("~/") {
        match wsl_home() {
            Some(home) => format!("{home}/{suffix}"),
            None => raw.to_string(),
        }
    } else {
        raw.to_string()
    };

    let windows = wsl_path_to_windows(&posix);

    let (exists, is_file, is_dir) = match std::fs::metadata(&windows) {
        Ok(m) => (true, m.is_file(), m.is_dir()),
        Err(_) => (false, false, false),
    };

    ResolvedPath {
        posix,
        windows,
        exists,
        is_file,
        is_dir,
    }
}

/// Open a file or folder in Notepad++. Accepts WSL or Windows paths.
/// Notepad++ is only available when `notepad++.exe` / `notepadpp.exe` is on
/// the Windows PATH. Falls back silently to Explorer on failure so the action
/// never errors visibly.
///
/// `--multiInst -nosession <file>` opens a fresh Notepad++ instance rather than
/// hijacking an existing session.
#[tauri::command]
pub fn open_in_notepadpp(path: String) -> Result<(), String> {
    let windows = wsl_path_to_windows(&windows_path_to_wsl(&path));

    // Try the two most common Notepad++ binary names. Corporate installs vary.
    let launched = ["notepad++.exe", "notepadpp.exe"].iter().any(|bin| {
        std::process::Command::new(bin)
            .args(["--multiInst", "-nosession", &windows])
            .spawn()
            .is_ok()
    });

    if launched {
        return Ok(());
    }

    // Fall back to Explorer (opens with the registered handler or Explorer).
    std::process::Command::new("explorer.exe")
        .arg(&windows)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Failed to open path: {e}"))
}
