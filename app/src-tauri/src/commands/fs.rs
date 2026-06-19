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
/// 1. Derive the posix form: drive paths (`C:\…`) and UNC (`\\wsl.localhost\…`)
///    → `windows_path_to_wsl`; a leading `~` expands via `wsl_home()`; otherwise
///    the input is already posix.
/// 2. `windows = wsl_path_to_windows(&posix)` — the form Windows can actually
///    open (drive path for `/mnt`, UNC for WSL-native paths).
/// 3. Stat `windows` for existence/type — on a blocking worker, time-bounded.
///
/// Async + `spawn_blocking` + timeout are deliberate: statting a cold/slow
/// `\\wsl.localhost\…` 9p share can take many seconds (Windows may even
/// cold-start the distro). A synchronous command does this on the UI thread and
/// freezes the palette ("not responding"); the timeout caps the wait and the
/// open/copy actions still work off the `windows` form even when it elapses.
///
/// Never errors — a bad/slow path returns exists=false, is_file=false, is_dir=false.
#[tauri::command]
pub async fn resolve_path(path: String) -> ResolvedPath {
    let raw_fallback = path.clone();

    // Derivation runs off the UI thread because `wsl_home()` (leading `~`) does a
    // blocking WSL round-trip.
    let (posix, windows) = tauri::async_runtime::spawn_blocking(move || {
        let raw = path.trim();

        // Detect a native Windows drive path: `^[A-Za-z]:[\\/]`.
        let is_drive_path = {
            let mut chars = raw.chars();
            let c1 = chars.next().map(|c| c.is_ascii_alphabetic()).unwrap_or(false);
            let c2 = chars.next().map(|c| c == ':').unwrap_or(false);
            let c3 = chars.next().map(|c| c == '/' || c == '\\').unwrap_or(false);
            c1 && c2 && c3
        };

        let posix = if is_drive_path || raw.starts_with('\\') || raw.starts_with("//") {
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
        (posix, windows)
    })
    .await
    .unwrap_or_else(|_| (raw_fallback.clone(), raw_fallback));

    // Stat the Windows form on a blocking worker, bounded so a hung 9p share
    // can't stall the action. On timeout/error: exists=false.
    let win = windows.clone();
    let stat = tauri::async_runtime::spawn_blocking(move || std::fs::metadata(&win));
    let (exists, is_file, is_dir) =
        match tokio::time::timeout(std::time::Duration::from_secs(3), stat).await {
            Ok(Ok(Ok(m))) => (true, m.is_file(), m.is_dir()),
            _ => (false, false, false),
        };

    ResolvedPath {
        posix,
        windows,
        exists,
        is_file,
        is_dir,
    }
}

/// Open a file in Notepad++. Accepts WSL or Windows paths.
///
/// `std::process::Command` uses `CreateProcessW`, which resolves bare names only
/// against `PATH` — and Notepad++ is not on `PATH` by default, nor does
/// `CreateProcessW` consult the "App Paths" registry key. So we probe the
/// standard install dirs (`%ProgramFiles%\Notepad++\notepad++.exe`, …) and
/// launch the absolute path. If Notepad++ can't be found we open the file with
/// its OS-registered handler via `cmd /c start` (ShellExecute) — `explorer.exe
/// <file>` only opens folders, so it would silently do nothing for a file.
///
/// `-multiInst -nosession <file>` opens a fresh Notepad++ instance rather than
/// hijacking an existing session.
#[tauri::command]
pub async fn open_in_notepadpp(path: String) -> Result<(), String> {
    let windows = wsl_path_to_windows(&windows_path_to_wsl(&path));

    let exe = notepadpp_exe();
    if std::process::Command::new(&exe)
        .args(["-multiInst", "-nosession", &windows])
        .spawn()
        .is_ok()
    {
        return Ok(());
    }

    // Notepad++ not found / failed to launch: open with the default handler so
    // the action is never a silent no-op.
    std::process::Command::new("cmd")
        .args(["/C", "start", "", &windows])
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Failed to open in Notepad++: {e}"))
}

/// Resolve a launchable `notepad++.exe`: probe the standard install dirs via the
/// `ProgramFiles` / `ProgramW6432` / `ProgramFiles(x86)` env vars and return the
/// first that exists; otherwise a bare `notepad++.exe` (resolved against `PATH`
/// at spawn time).
fn notepadpp_exe() -> std::path::PathBuf {
    use std::path::PathBuf;
    for var in ["ProgramFiles", "ProgramW6432", "ProgramFiles(x86)"] {
        if let Ok(dir) = std::env::var(var) {
            let p = PathBuf::from(dir).join("Notepad++").join("notepad++.exe");
            if p.exists() {
                return p;
            }
        }
    }
    PathBuf::from("notepad++.exe")
}

/// Reveal a file in Windows Explorer with the file selected/highlighted.
/// Accepts WSL or Windows paths. Uses `explorer.exe /select,<file>` so the
/// containing folder opens with the target highlighted (unlike a bare path arg,
/// which opens the file/folder directly).
#[tauri::command]
pub async fn reveal_in_explorer(path: String) -> Result<(), String> {
    let windows = wsl_path_to_windows(&windows_path_to_wsl(&path));
    std::process::Command::new("explorer.exe")
        .arg(format!("/select,{windows}"))
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Failed to reveal in explorer: {e}"))
}
