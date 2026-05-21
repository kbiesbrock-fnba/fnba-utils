//! Daemon launcher + Windows auto-start registration for `fnba-clipd.exe`.
//!
//! Responsibilities:
//! 1. Locate `fnba-clipd.exe` (assumed to sit next to the currently-running
//!    `fnba-utils.exe` — true in both `cargo` dev builds and the portable zip).
//! 2. Spawn it detached. The daemon's own singleton mutex makes the spawn
//!    no-op if it's already running, so we can call this unconditionally on
//!    every fnba-utils startup.
//! 3. Register the daemon under `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`
//!    so Windows starts it at login even without fnba-utils being launched.
//!    Idempotent — overwrites with the current path on every run so moving
//!    the portable folder self-heals on the next fnba-utils launch.
//!
//! Non-Windows: all functions are no-ops so the crate compiles cross-platform.

use std::path::PathBuf;

const RUN_KEY_VALUE: &str = "fnba-clipd";
const RUN_KEY_PATH: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

pub fn ensure_running_and_registered() -> Result<(), String> {
    let exe = locate_daemon_exe()?;
    // Only register for Windows auto-start in release builds; dev builds live
    // under `target/debug/` and that path goes stale the moment the dev
    // session ends, so we'd just leave a broken Run key behind. In debug we
    // actively unregister to clean up any stale entry from prior dev runs.
    #[cfg(all(windows, not(debug_assertions)))]
    register_autostart(&exe)?;
    #[cfg(all(windows, debug_assertions))]
    let _ = unregister_autostart();
    spawn_detached(&exe)?;
    Ok(())
}

fn locate_daemon_exe() -> Result<PathBuf, String> {
    let here = std::env::current_exe()
        .map_err(|e| format!("current_exe: {e}"))?;
    let dir = here
        .parent()
        .ok_or_else(|| "current_exe has no parent dir".to_string())?;
    let candidate = dir.join("fnba-clipd.exe");
    if !candidate.exists() {
        return Err(format!(
            "fnba-clipd.exe not found next to fnba-utils.exe at {}",
            candidate.display()
        ));
    }
    Ok(candidate)
}

#[cfg(windows)]
fn spawn_detached(exe: &PathBuf) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    // DETACHED_PROCESS (0x00000008) — no console window, no parent stdio
    // inheritance. CREATE_NEW_PROCESS_GROUP (0x00000200) — survives our exit.
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    std::process::Command::new(exe)
        .creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("spawn fnba-clipd: {e}"))?;
    Ok(())
}

#[cfg(not(windows))]
fn spawn_detached(_exe: &PathBuf) -> Result<(), String> {
    Ok(())
}

#[cfg(windows)]
#[allow(dead_code)] // unused in debug builds (we don't register there)
fn register_autostart(exe: &PathBuf) -> Result<(), String> {
    use winreg::enums::*;
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (run_key, _) = hkcu
        .create_subkey(RUN_KEY_PATH)
        .map_err(|e| format!("open HKCU Run key: {e}"))?;
    // The Run key value must be quoted if the path contains spaces.
    let path_str = exe.to_string_lossy().into_owned();
    let quoted = if path_str.contains(' ') {
        format!("\"{path_str}\"")
    } else {
        path_str
    };
    run_key
        .set_value(RUN_KEY_VALUE, &quoted)
        .map_err(|e| format!("set Run key value: {e}"))?;
    Ok(())
}

#[cfg(not(windows))]
fn register_autostart(_exe: &PathBuf) -> Result<(), String> {
    Ok(())
}

/// Remove the Windows auto-start registration. Used by debug builds to keep
/// dev state clean, and reserved for future "uninstall capture daemon" UX.
#[cfg(windows)]
pub fn unregister_autostart() -> Result<(), String> {
    use winreg::enums::*;
    use winreg::RegKey;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let run_key = hkcu
        .open_subkey_with_flags(RUN_KEY_PATH, KEY_SET_VALUE)
        .map_err(|e| format!("open HKCU Run key: {e}"))?;
    let _ = run_key.delete_value(RUN_KEY_VALUE);
    Ok(())
}
