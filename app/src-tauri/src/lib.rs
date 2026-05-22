mod clipboard;
mod commands;
mod config;
mod db;
mod models;
mod standup_db;
mod state;

/// Public re-exports for the `fnba-clipd` daemon binary, which only needs
/// the clipboard subsystem (no Tauri / commands / standup).
pub mod clipboard_state {
    pub use crate::state::clipboard_history::{
        ClipboardHistoryState, ClipboardSettings, InsertOutcome, NewClipboardEntry,
    };

    pub fn load() -> ClipboardHistoryState {
        ClipboardHistoryState::load()
    }
}

pub mod test_users_state {
    pub use crate::state::test_users::{TestCard, TestUser, TestUsersState};

    pub fn load() -> TestUsersState {
        TestUsersState::load()
    }
}

pub mod clipboard_listener {
    pub use crate::clipboard::listener::{install_test_user_picker, spawn, ClipboardEventSender};
}

use tauri::{
    menu::{AboutMetadata, Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager, RunEvent,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
use tauri_plugin_notification::NotificationExt;

/// Polls the shared clipboard DB and fires a toast notification each time a
/// new PII-protected entry lands. Detects "new" via captured_at watermarking
/// — the same entry being touched (recopied) bumps its captured_at, which we
/// treat as a fresh notification trigger because the user just copied that
/// sensitive content again.
fn run_pii_watcher(handle: AppHandle) {
    let mut last_seen: i64 = 0;
    // Skip the first tick — we want to ignore whatever was already in the DB
    // when the app started.
    let mut initialized = false;
    loop {
        std::thread::sleep(std::time::Duration::from_millis(1200));
        let Some(history) = handle.try_state::<state::clipboard_history::ClipboardHistoryState>()
        else {
            continue;
        };
        let latest = history.max_captured_at().unwrap_or(0);
        if !initialized {
            last_seen = latest;
            initialized = true;
            continue;
        }
        if latest <= last_seen {
            continue;
        }
        // Pull the most recent entry and check whether it was auto-protected
        // (sensitive AND pii_kinds non-empty). pii_kinds is only populated
        // when our PII detector fired — OS-marker-only sensitivity leaves it
        // empty, and we don't want to notify on those.
        let rows = match history.list(None, None, false, 1, 0) {
            Ok(r) => r,
            Err(_) => {
                last_seen = latest;
                continue;
            }
        };
        last_seen = latest;
        let Some(top) = rows.into_iter().next() else {
            continue;
        };
        if !top.sensitive || top.pii_kinds.is_empty() {
            continue;
        }
        let kinds = top.pii_kinds.join(", ");
        let _ = handle
            .notification()
            .builder()
            .title("Clipboard protected")
            .body(format!(
                "Detected PII ({kinds}). Clipboard replaced with safe test data. \
                 Win+V then Ctrl+Shift+Enter for the original."
            ))
            .show();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .manage(config::AppConfig::load())
        .manage(models::mission_control::ClaudeIoState::new())
        .manage(models::mission_control::SqlQueryState::new())
        .manage(state::owned_sessions::OwnedSessionsState::load())
        .manage(state::projects::ProjectsState::load())
        .manage(state::clipboard_history::ClipboardHistoryState::load())
        .manage(state::test_users::TestUsersState::load())
        .manage(clipboard::ForegroundCapture::default())
        .manage(commands::clipboard_manager::RevealTokens::default())
        .setup(|app| {
            // --- System Tray ---
            let show = MenuItem::with_id(app, "show", "Show Palette", true, None::<&str>)?;
            let about = PredefinedMenuItem::about(
                app,
                Some("About FNBA Utils"),
                Some(AboutMetadata {
                    name: Some("FNBA Utils".to_string()),
                    version: Some(env!("APP_VERSION").to_string()),
                    copyright: Some("FNBA".to_string()),
                    ..Default::default()
                }),
            )?;
            let sep = PredefinedMenuItem::separator(app)?;
            let quit = MenuItem::with_id(app, "quit", "Quit FNBA Utils", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &sep, &about, &quit])?;

            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip(format!("FNBA Utils {}", env!("APP_VERSION")))
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "quit" => app.exit(0),
                    "show" => {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                    _ => {}
                })
                .build(app)?;

            // --- Global Shortcut: Win+Shift+F (command palette) ---
            app.global_shortcut().on_shortcut(
                "Super+Shift+F",
                move |app: &AppHandle, _shortcut, event| {
                    if event.state == ShortcutState::Pressed {
                        if let Some(w) = app.get_webview_window("main") {
                            if w.is_visible().unwrap_or(false) {
                                let _ = w.hide();
                            } else {
                                // Cover the full monitor so the backdrop overlay dims the screen
                                if let Ok(Some(monitor)) = w.current_monitor() {
                                    let size = monitor.size();
                                    let pos = monitor.position();
                                    let _ = w.set_size(tauri::Size::Physical(
                                        tauri::PhysicalSize::new(size.width, size.height),
                                    ));
                                    let _ = w.set_position(tauri::Position::Physical(
                                        tauri::PhysicalPosition::new(pos.x, pos.y),
                                    ));
                                }
                                let _ = w.show();
                                let _ = w.set_focus();
                            }
                        }
                    }
                },
            )?;

            // --- Global Shortcut: Win+Shift+N (launch into MRU project) ---
            // Looks up the most-recently-used project from ProjectsState and
            // emits `mc-mru-launch` with its cwd + displayName. The Mission
            // Control window's frontend listens and calls the same
            // start_new_claude_session pipeline the palette uses.
            //
            // No-op (silently) if the registry is empty; user can use
            // Win+Shift+F → "new claude" to seed the registry.
            app.global_shortcut().on_shortcut(
                "Super+Shift+N",
                move |app: &AppHandle, _shortcut, event| {
                    if event.state != ShortcutState::Pressed {
                        return;
                    }
                    let Some(projects) = app.try_state::<state::projects::ProjectsState>() else {
                        return;
                    };
                    if let Some(p) = projects.most_recent_used() {
                        let _ = app.emit(
                            "mc-mru-launch",
                            serde_json::json!({
                                "cwd": p.cwd,
                                "displayName": p.display_name,
                            }),
                        );
                        // Make sure Mission Control is showing so its listener
                        // is awake and the new session-detail window has a
                        // place to anchor to.
                        if let Some(w) = app.get_webview_window("mission-control") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                },
            )?;

            // --- Global Shortcut: Ctrl+Shift+Tab (cycle session-detail windows) ---
            // When you're juggling multiple session-detail panels, this is the
            // fast path to "focus the next one." Order = label asc, which is
            // stable per session_id hash.
            app.global_shortcut().on_shortcut(
                "Control+Shift+Tab",
                move |app: &AppHandle, _shortcut, event| {
                    if event.state != ShortcutState::Pressed {
                        return;
                    }
                    let mut panels: Vec<(String, tauri::WebviewWindow)> = app
                        .webview_windows()
                        .into_iter()
                        .filter(|(label, _)| label.starts_with("session-detail:"))
                        .collect();
                    if panels.is_empty() {
                        return;
                    }
                    panels.sort_by(|a, b| a.0.cmp(&b.0));
                    // Find the currently-focused panel, focus the next in order
                    // (or the first if none focused / focused isn't a panel).
                    let current = panels
                        .iter()
                        .position(|(_, w)| w.is_focused().unwrap_or(false));
                    let next_idx = current.map(|i| (i + 1) % panels.len()).unwrap_or(0);
                    if let Some((_, w)) = panels.get(next_idx) {
                        let _ = w.show();
                        let _ = w.set_focus();
                    }
                },
            )?;

            // --- Global Shortcut: Win+Shift+C (Mission Control) ---
            app.global_shortcut().on_shortcut(
                "Super+Shift+C",
                move |app: &AppHandle, _shortcut, event| {
                    if event.state == ShortcutState::Pressed {
                        if let Some(w) = app.get_webview_window("mission-control") {
                            if w.is_visible().unwrap_or(false) {
                                let _ = w.hide();
                                for (label, win) in app.webview_windows() {
                                    if label.starts_with("session-detail:")
                                        || label.starts_with("sql-query:")
                                    {
                                        let _ = win.hide();
                                    }
                                }
                            } else {
                                // Position at bottom-left of the current monitor
                                if let Ok(Some(monitor)) = w.current_monitor() {
                                    let mon_size = monitor.size();
                                    let mon_pos = monitor.position();
                                    let win_size = w.outer_size().unwrap_or(
                                        tauri::PhysicalSize::new(320, 560),
                                    );
                                    let margin = 16;
                                    let x = mon_pos.x + margin;
                                    let y = mon_pos.y + mon_size.height as i32
                                        - win_size.height as i32
                                        - margin
                                        - 48; // taskbar clearance
                                    let _ = w.set_position(tauri::Position::Physical(
                                        tauri::PhysicalPosition::new(x, y),
                                    ));
                                }
                                let _ = w.show();
                                let _ = w.set_focus();
                                let _ = app.emit("mc-shown", ());
                            }
                        }
                    }
                },
            )?;

            // --- Clipboard daemon (fnba-clipd.exe) ---
            // Capture lives in a separate background process so it keeps
            // running even when fnba-utils itself is closed. We ensure it's
            // alive on startup and register it for Windows auto-start on
            // login. See `clipboard::daemon`.
            if let Err(e) = clipboard::daemon::ensure_running_and_registered() {
                eprintln!("fnba-clipd auto-launch failed: {e}");
            }

            // --- Global Hotkey: Win+V (Clipboard Manager) ---
            // Replaces the native Windows clipboard history. Uses a
            // WH_KEYBOARD_LL hook (not RegisterHotKey) so we see Win+V before
            // the shell does and can swallow it. See `clipboard::hotkey` for
            // rationale.
            clipboard::hotkey::spawn(app.handle().clone());

            // --- PII Protection Watcher ---
            // The daemon process auto-replaces the OS clipboard when it
            // detects PII in a fresh capture. This thread (in the UI process)
            // polls the shared DB and fires a Windows toast notification
            // every time a new PII-protected entry lands, so the user knows
            // the clipboard was replaced before they paste. The original is
            // still in history — Win+V then Ctrl+Shift+Enter retrieves it.
            //
            // Notification only fires when fnba-utils.exe is open. The
            // protection itself runs unconditionally in the daemon.
            {
                let handle = app.handle().clone();
                std::thread::Builder::new()
                    .name("fnba-pii-watcher".into())
                    .spawn(move || run_pii_watcher(handle))
                    .expect("failed to spawn pii watcher");
            }

            // --- Global Shortcut: Win+Shift+D (Standup Panel) ---
            // Registered unconditionally; the panel window only exists when the
            // standup feature is enabled, so the shortcut is a no-op otherwise.
            app.global_shortcut().on_shortcut(
                "Super+Shift+D",
                move |app: &AppHandle, _shortcut, event| {
                    if event.state == ShortcutState::Pressed {
                        if let Some(w) = app.get_webview_window("standup-panel") {
                            if w.is_visible().unwrap_or(false) {
                                let _ = w.hide();
                            } else {
                                // Position at bottom-right of the current monitor
                                if let Ok(Some(monitor)) = w.current_monitor() {
                                    let mon_size = monitor.size();
                                    let mon_pos = monitor.position();
                                    let win_size = w.outer_size().unwrap_or(
                                        tauri::PhysicalSize::new(360, 640),
                                    );
                                    let margin = 16;
                                    let x = mon_pos.x + mon_size.width as i32
                                        - win_size.width as i32
                                        - margin;
                                    let y = mon_pos.y + mon_size.height as i32
                                        - win_size.height as i32
                                        - margin
                                        - 48; // taskbar clearance
                                    let _ = w.set_position(tauri::Position::Physical(
                                        tauri::PhysicalPosition::new(x, y),
                                    ));
                                }
                                let _ = w.show();
                                let _ = w.set_focus();
                                let _ = app.emit("standup-panel-shown", ());
                            }
                        }
                    }
                },
            )?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::hide_window,
            config::get_app_config,
            commands::standup::run_standup,
            commands::standup::preview_standup,
            commands::standup::post_standup_to_teams,
            commands::standup::get_standup_report,
            commands::standup::get_standup_last_run,
            commands::standup::get_standup_panel_state,
            commands::standup::set_issue_hidden,
            commands::standup::clear_hidden_issues,
            commands::standup::set_issue_order,
            commands::standup::clear_manual_order,
            commands::standup::get_run_snapshot,
            commands::standup::get_issue_detail,
            commands::assume_identity::get_identity_data,
            commands::assume_identity::execute_assume_identity,
            commands::assume_identity::save_custom_entry,
            commands::assume_identity::delete_custom_entry,
            commands::right_lookup::get_all_rights,
            commands::right_lookup::get_right_associates,
            commands::right_lookup::search_associates,
            commands::right_lookup::get_associate_rights,
            commands::mission_control::get_claude_sessions,
            commands::mission_control::get_connection_statuses,
            commands::mission_control::execute_sql_query,
            commands::mission_control::kill_sql_query,
            commands::mission_control::get_session_detail,
            commands::mission_control::kill_session,
            commands::mission_control::open_in_explorer,
            commands::claude_io::start_new_claude_session,
            commands::claude_io::start_claude_session,
            commands::claude_io::send_claude_message,
            commands::claude_io::write_session_pty,
            commands::claude_io::resize_session_pty,
            commands::claude_io::stop_claude_session,
            commands::claude_io::disconnect_session,
            commands::claude_io::interrupt_claude_session,
            commands::claude_io::update_session_label,
            commands::claude_io::pick_directory,
            commands::claude_io::open_path_in_editor,
            commands::claude_io::list_session_history,
            commands::claude_io::forget_session_history,
            commands::claude_io::resume_owned_session,
            commands::claude_io::attach_tmux_session,
            commands::projects::list_projects,
            commands::projects::add_project,
            commands::projects::update_project,
            commands::projects::remove_project,
            commands::projects::record_project_used,
            commands::clipboard_manager::list_clipboard_entries,
            commands::clipboard_manager::get_clipboard_entry,
            commands::clipboard_manager::paste_clipboard_entry,
            commands::clipboard_manager::request_sensitive_reveal,
            commands::clipboard_manager::delete_clipboard_entry,
            commands::clipboard_manager::pin_clipboard_entry,
            commands::clipboard_manager::clear_clipboard_history,
            commands::clipboard_manager::get_clipboard_settings,
            commands::clipboard_manager::set_clipboard_settings,
            commands::clipboard_manager::hide_clipboard_window,
            commands::clipboard_manager::get_clipboard_max_captured_at,
            commands::clipboard_manager::list_test_users,
            commands::clipboard_manager::upsert_test_user,
            commands::clipboard_manager::delete_test_user,
            commands::clipboard_manager::set_test_user_enabled,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    // Graceful Ctrl+C: destroy the webview before exiting so WebView2
    // can unregister its window classes without error.
    let handle = app.handle().clone();
    ctrlc::set_handler(move || {
        eprintln!("Ctrl+C received — shutting down FNBA Utils…");
        for (_label, win) in handle.webview_windows() {
            let _ = win.destroy();
        }
        handle.exit(0);
    })
    .expect("failed to set Ctrl+C handler");

    app.run(|_app, event| {
        if let RunEvent::Exit = event {
            eprintln!("FNBA Utils exited.");
        }
    });
}
