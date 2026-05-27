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

    // Register our toast AppUserModelID for this process. The daemon
    // auto-starts at login independently of fnba-utils, so it can't rely on
    // the UI process having registered the AUMID — without this its toasts
    // would silently fail (see fnba_utils_lib::aumid for the full rationale).
    fnba_utils_lib::notifications::ensure_registered();

    let history = std::sync::Arc::new(fnba_utils_lib::clipboard_state::load());
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

    // Install the self-write registry BEFORE spawning the listener. The
    // listener consumes a mark on each clipboard update to skip the echo of a
    // write we made ourselves — both the daemon's own PII substitution and a
    // paste from the UI's clipboard manager (which records its mark in the
    // same shared DB). Without this, the substituted "safe" value would be
    // re-scanned, re-substituted, and re-written in an infinite loop.
    {
        let h_mark = history.clone();
        let h_check = history.clone();
        fnba_utils_lib::clipboard_listener::install_self_write_store(
            move |hash: &str| h_mark.mark_self_write(hash),
            move |hash: &str| h_check.is_self_write(hash),
        );
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
            // Capture the toast trigger before `entry` is moved into the
            // insert. We only notify on auto-protected content: `pii_kinds` is
            // populated solely when our detector matched, so OS-marker-only
            // sensitivity (empty kinds) is skipped. Both a fresh insert and a
            // re-copy (Touched) count as a deliberate copy worth notifying on.
            let pii_kinds = (entry.sensitive && !entry.pii_kinds.is_empty())
                .then(|| entry.pii_kinds.join(", "));

            match history.insert_or_touch(entry) {
                Ok(fnba_utils_lib::clipboard_state::InsertOutcome::Inserted(_)) => {
                    let _ = history.prune(&settings);
                    if let Some(kinds) = pii_kinds {
                        fnba_utils_lib::notifications::show_pii_protected(&kinds);
                    }
                }
                Ok(fnba_utils_lib::clipboard_state::InsertOutcome::Touched(_)) => {
                    if let Some(kinds) = pii_kinds {
                        fnba_utils_lib::notifications::show_pii_protected(&kinds);
                    }
                }
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
