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
/// No `-multiInst`: Notepad++ is single-instance by default, so passing just the
/// file path opens it as a new tab in the already-running window (or starts one
/// if none is open) rather than spawning a fresh instance each time.
#[tauri::command]
pub async fn open_in_notepadpp(path: String) -> Result<(), String> {
    let windows = wsl_path_to_windows(&windows_path_to_wsl(&path));

    let exe = notepadpp_exe();
    if std::process::Command::new(&exe)
        .arg(&windows)
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

/// Open a path with its OS-registered handler — the same thing a double-click in
/// Explorer does (PDF → your PDF reader, .xlsx → Excel, folder → Explorer).
/// Accepts WSL or Windows paths.
///
/// `ShellExecuteW` rather than `cmd /C start "" <path>`: `cmd` re-parses its own
/// command line, so a path containing `&`, `^`, or `%VAR%` is a quoting hazard
/// that Rust's `Command` arg escaping does NOT cover (it escapes for
/// `CommandLineToArgvW`, not for the shell). ShellExecuteW takes the path as a
/// single wide string with no parsing in between.
///
/// COM is initialized on the worker thread because some shell handlers need an
/// apartment; `ShellExecuteW` succeeds without it for common file types but not
/// universally. `RPC_E_CHANGED_MODE` (already initialized as MTA) is not fatal —
/// we just skip the matching `CoUninitialize`.
#[tauri::command]
pub async fn open_with_default(path: String) -> Result<(), String> {
    let windows = wsl_path_to_windows(&windows_path_to_wsl(&path));

    // ShellExecuteW blocks while the handler starts; keep it off the UI thread.
    tauri::async_runtime::spawn_blocking(move || shell_open(&windows))
        .await
        .map_err(|e| format!("Failed to open: {e}"))?
}

#[cfg(windows)]
fn shell_open(windows_path: &str) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::Com::{
        CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    let wide: Vec<u16> = std::ffi::OsStr::new(windows_path)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    unsafe {
        let hr = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let owns_com = hr.is_ok();

        // Null verb = the file type's default verb ("open" for most, "openfolder"
        // for a directory). Null HWND — no owner window for handler error dialogs.
        let rc = ShellExecuteW(
            HWND(std::ptr::null_mut()),
            PCWSTR::null(),
            PCWSTR(wide.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        );

        if owns_com {
            CoUninitialize();
        }

        // Legacy API: the HINSTANCE is an error code when <= 32.
        let code = rc.0 as isize;
        if code > 32 {
            Ok(())
        } else {
            Err(match code {
                2 | 3 => format!("Not found: {windows_path}"),
                31 => format!("No app is associated with this file type: {windows_path}"),
                _ => format!("Failed to open {windows_path} (ShellExecute code {code})"),
            })
        }
    }
}

#[cfg(not(windows))]
fn shell_open(_windows_path: &str) -> Result<(), String> {
    Err("open_with_default is Windows-only".to_string())
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

/// Read a file's full text content. Accepts WSL or Windows paths. Backs the
/// palette's "Open in Markdown/JSON Viewer" soft commands for a pasted path —
/// those viewers only take a content string, not a path.
#[tauri::command]
pub async fn read_text_file(path: String) -> Result<String, String> {
    let windows = wsl_path_to_windows(&windows_path_to_wsl(&path));
    tauri::async_runtime::spawn_blocking(move || std::fs::read_to_string(&windows))
        .await
        .map_err(|e| format!("Failed to read {path}: {e}"))?
        .map_err(|e| format!("Failed to read {path}: {e}"))
}
