// Prevents the console window from flashing when launched at login.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! fnba-clipd — standalone clipboard capture daemon.
//!
//! Runs invisibly in the background (no UI, no Tauri/WebView), captures
//! `WM_CLIPBOARDUPDATE` events, and persists each new entry to the shared
//! SQLite database at `%LOCALAPPDATA%\fnba-utils\clipboard.db`.
//!
//! Designed to run from Windows login on, so the clipboard history is always
//! up-to-date — even when fnba-utils itself isn't running. fnba-utils owns
//! the display/UI surface and reads from the same DB.
//!
//! Lives in its own workspace member (not as a `[[bin]]` of the main package)
//! so `tauri_build::build()` in the parent doesn't co-embed its VERSIONINFO
//! resource into this binary — that would collide with the "FNBA Clipd"
//! resource emitted by `build.rs` and break Task Manager labelling.
//!
//! Singleton: binds a fixed loopback TCP port; a second instance fails the
//! bind and exits cleanly so that "ensure daemon running" callers can spawn
//! unconditionally.

#[cfg(not(windows))]
fn main() {
    eprintln!("fnba-clipd is Windows-only.");
}

#[cfg(windows)]
fn main() {
    let _singleton = match acquire_singleton() {
        Some(s) => s,
        None => return, // another instance already capturing; exit silently
    };

    // Mirror the UI process's startup sweep so the daemon doesn't load state
    // from `%LOCALAPPDATA%\fnba-utils\` while a legacy file still sits at the
    // old path. Idempotent: re-running it is just orphan cleanup.
    fnba_utils_lib::app_paths::migrate_legacy_files();

    let history = fnba_utils_lib::clipboard_state::load();
    let test_users = std::sync::Arc::new(fnba_utils_lib::test_users_state::load());

    // Install the test-user picker BEFORE spawning the listener so the very
    // first capture already has access to the substitution pool. The closure
    // owns a clone of the Arc so it outlives this scope.
    {
        let pool = test_users.clone();
        fnba_utils_lib::clipboard_listener::install_test_user_picker(move || {
            pool.pick_random_enabled().ok().flatten()
        });
    }

    let (tx, mut rx) =
        tokio::sync::mpsc::unbounded_channel::<fnba_utils_lib::clipboard_state::NewClipboardEntry>();
    fnba_utils_lib::clipboard_listener::spawn(tx);

    // Single-threaded tokio runtime is plenty — the actual capture happens on
    // a dedicated OS thread inside `clipboard::listener::spawn`. This runtime
    // exists only to drain the channel into SQLite.
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build tokio runtime");

    rt.block_on(async move {
        while let Some(entry) = rx.recv().await {
            let settings = history.get_settings().unwrap_or_default();
            if !settings.capture_enabled {
                continue;
            }
            if let Some(ref proc) = entry.source_process {
                let lc = proc.to_lowercase();
                if settings
                    .ignored_processes
                    .iter()
                    .any(|p| lc.contains(&p.to_lowercase()))
                {
                    continue;
                }
            }
            match history.insert_or_touch(entry) {
                Ok(fnba_utils_lib::clipboard_state::InsertOutcome::Inserted(_)) => {
                    let _ = history.prune(&settings);
                }
                Ok(fnba_utils_lib::clipboard_state::InsertOutcome::Touched(_)) => {}
                Err(e) => eprintln!("fnba-clipd insert failed: {e}"),
            }
        }
    });
}

/// Singleton via TCP port bind: only one process can hold the port, so a
/// successful bind == "we are the daemon." Held for the lifetime of the
/// process (returned listener is moved into `_singleton` in main and stays
/// alive). Loopback-only, high port chosen to minimize collision risk.
#[cfg(windows)]
fn acquire_singleton() -> Option<std::net::TcpListener> {
    const SINGLETON_PORT: u16 = 53_217;
    std::net::TcpListener::bind(("127.0.0.1", SINGLETON_PORT)).ok()
}
