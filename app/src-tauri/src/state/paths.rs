//! Centralised on-disk locations for fnba-utils state.
//!
//! Everything the app persists — config, SQLite databases, MC registries,
//! user-editable identity overrides — lives under `%LOCALAPPDATA%\fnba-utils\`.
//! Both `fnba-utils.exe` and the `fnba-clipd.exe` daemon resolve the same
//! root via [`app_data_dir`] so they read/write the same files.
//!
//! `migrate_legacy_files` runs at startup from both processes (whichever wins
//! a race against the same legacy file is harmless: the loser sees the source
//! gone and the destination present, and a follow-up orphan sweep tidies up
//! any leftovers from a previously-interrupted migration).

use std::path::{Path, PathBuf};

const APP_DIR_NAME: &str = "fnba-utils";

/// `%LOCALAPPDATA%\fnba-utils`, creating it on demand.
///
/// Resolution order:
/// 1. `LOCALAPPDATA` env var (the canonical answer on Windows)
/// 2. `dirs::data_local_dir()` (cross-platform fallback — same on Windows,
///    `$XDG_DATA_HOME` or `~/.local/share` on Linux, used for `cargo build`
///    of unit tests without a real Windows env)
/// 3. The system temp dir as a last resort so we never panic at startup.
pub fn app_data_dir() -> PathBuf {
    let root = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .or_else(dirs::data_local_dir)
        .unwrap_or_else(std::env::temp_dir);
    let dir = root.join(APP_DIR_NAME);
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Convenience for `app_data_dir().join(name)`.
pub fn data_file(name: &str) -> PathBuf {
    app_data_dir().join(name)
}

/// Migrate state files written by older builds into [`app_data_dir`].
///
/// For each known legacy location:
/// - If the legacy file is missing, nothing to do.
/// - If the new file is missing, move the legacy file in.
/// - If both exist (an orphan from a previously-interrupted migration where
///   `copy` succeeded but `remove` failed), best-effort delete the orphan.
///
/// Best-effort: failures log to stderr and never block startup.
pub fn migrate_legacy_files() {
    let target = app_data_dir();
    let home = dirs::home_dir();
    let exe_resources = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(|p| p.join("resources")));

    // (legacy path, file name in new dir)
    let mut sources: Vec<(PathBuf, &str)> = Vec::new();

    if let Some(h) = home.as_ref() {
        sources.push((h.join(".fnba-utils").join("config.yaml"), "config.yaml"));
        sources.push((h.join(".fnba-utils").join("standup.db"), "standup.db"));
        sources.push((h.join(".assumeIdentity.json"), "assumeIdentity.json"));
    }
    if let Some(r) = exe_resources.as_ref() {
        sources.push((r.join("standup.db"), "standup.db"));
        sources.push((r.join("standup-last-run.json"), "standup-last-run.json"));
    }

    // MC JSON registries: WSL-home variants written by older builds.
    // (clipboard.db never lived under WSL — SQLite + SMB locking is unreliable
    // and the daemon has always used `data_local_dir()`.)
    let wsl_root = PathBuf::from(r"\\wsl.localhost\Ubuntu\home");
    if let Ok(entries) = std::fs::read_dir(&wsl_root) {
        for entry in entries.flatten() {
            let base = entry.path().join(".claude").join("fnba-mc");
            if base.is_dir() {
                sources.push((base.join("owned-sessions.json"), "owned-sessions.json"));
                sources.push((base.join("projects.json"), "projects.json"));
            }
        }
    }
    // Roaming-AppData fallback used by old builds when `data_local_dir()`
    // returned `None`. Vanishingly rare on real Windows sessions, but cheap to
    // cover.
    if let Some(roaming) = dirs::data_dir() {
        let base = roaming.join(APP_DIR_NAME);
        if base != target && base.is_dir() {
            sources.push((base.join("owned-sessions.json"), "owned-sessions.json"));
            sources.push((base.join("projects.json"), "projects.json"));
            sources.push((base.join("clipboard.db"), "clipboard.db"));
        }
    }

    for (legacy, new_name) in sources {
        let dest = target.join(new_name);
        if !legacy.exists() {
            continue;
        }
        if dest.exists() {
            // Orphan from a prior partial migration (copy ok, remove failed)
            // or a parallel run that won the race. The destination is the
            // source of truth; tidy up the stale legacy file.
            match std::fs::remove_file(&legacy) {
                Ok(()) => eprintln!("paths: cleaned orphan {}", legacy.display()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => eprintln!(
                    "paths: could not remove orphan {}: {e}",
                    legacy.display()
                ),
            }
            continue;
        }
        match move_file(&legacy, &dest) {
            Ok(()) => eprintln!("paths: migrated {} -> {}", legacy.display(), dest.display()),
            Err(e) => eprintln!(
                "paths: failed to migrate {} -> {}: {e}",
                legacy.display(),
                dest.display()
            ),
        }
    }
}

/// Rename across volumes can fail with `EXDEV`-equivalent; fall back to
/// copy + remove so a move from `\\wsl.localhost\...` to `C:\Users\...` works.
///
/// The migration's contract is "data is at the new path." A failed cleanup of
/// the legacy file leaves an orphan, which the next call to
/// [`migrate_legacy_files`] will sweep — so we return `Ok` once the copy
/// succeeds and only log the cleanup failure.
fn move_file(from: &Path, to: &Path) -> std::io::Result<()> {
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if std::fs::rename(from, to).is_ok() {
        return Ok(());
    }
    std::fs::copy(from, to)?;
    if let Err(e) = std::fs::remove_file(from) {
        eprintln!(
            "paths: copied {} -> {} but could not remove source: {e}",
            from.display(),
            to.display()
        );
    }
    Ok(())
}
