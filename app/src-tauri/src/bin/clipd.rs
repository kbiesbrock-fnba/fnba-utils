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
//! Singleton: takes a named mutex (`Global\fnba-clipd-singleton`) on startup;
//! a second instance sees `ERROR_ALREADY_EXISTS` and exits cleanly so that
//! "ensure daemon running" callers can spawn unconditionally.

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

    let history = fnba_utils_lib::clipboard_state::load();

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
    // Port choice: fixed-but-unusual loopback port. If something else is
    // already on it, treat as "another daemon is running" and exit. The
    // user can override by killing whatever else owns the port.
    const SINGLETON_PORT: u16 = 53_217;
    std::net::TcpListener::bind(("127.0.0.1", SINGLETON_PORT)).ok()
}
