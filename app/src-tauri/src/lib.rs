mod aumid;
mod clipboard;
mod commands;
mod config;
mod db;
mod display_watch;
mod models;
mod standup_db;
mod state;
mod util;
mod widget_focus;

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
    pub use crate::clipboard::listener::{
        install_self_write_store, install_test_user_picker, spawn, ClipboardEventSender,
    };
}

/// Re-exported for the `fnba-clipd` daemon. The daemon registers the toast
/// AppUserModelID for its own process and raises the PII-protection toast
/// itself — it captures + substitutes the clipboard and runs even when the
/// fnba-utils UI process is closed, so it (not the UI) owns the notification.
pub mod notifications {
    pub use crate::aumid::{ensure_registered, show_pii_protected};
}

/// Re-exported so the daemon can run the same legacy-file migration the UI
/// process runs. Today both processes resolve the same legacy and new paths
/// for `clipboard.db`, so the migration sweep is just orphan cleanup — but
/// keeping the call symmetrical means a future state file added to the
/// daemon won't silently miss migration if the daemon starts before the UI.
pub mod app_paths {
    pub use crate::state::paths::{app_data_dir, data_file, migrate_legacy_files};
}

use tauri::{
    menu::{AboutMetadata, Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager, RunEvent,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

/// Collapse the Docker widget when the user switches to another window. The
/// widget is intentionally non-focusable, so it never holds the foreground and
/// receives no OS blur event — instead we watch for any change in the
/// foreground window, which means "the user looked away", and emit a defocus
/// event the widget listens for.
#[cfg(windows)]
fn spawn_foreground_watch(app: AppHandle) {
    use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
    std::thread::spawn(move || {
        let mut last: isize = unsafe { GetForegroundWindow().0 as isize };
        loop {
            std::thread::sleep(std::time::Duration::from_millis(200));
            let cur = unsafe { GetForegroundWindow().0 as isize };
            if cur != last {
                last = cur;
                let _ = app.emit("docker-widget-defocus", ());
            }
        }
    });
}

#[cfg(not(windows))]
fn spawn_foreground_watch(_app: AppHandle) {}

/// Position the always-on Docker widget flush above the taskbar on the primary
/// monitor. Restores a saved X that still lands on the primary monitor, else
/// centres horizontally; the bottom edge is always pinned to the work-area
/// bottom (taskbar top). Called once at startup and again whenever the display
/// topology changes (dock/undock) — both the primary monitor identity and the
/// work-area rect can shift when a laptop is docked/undocked.
///
/// Returns `true` only if it moved the window to a validated location. Returns
/// `false` — WITHOUT moving the window — when the primary monitor is unknown,
/// the work-area bottom can't be resolved sanely, or the computed rect lands
/// off every live monitor. A `false` result means the topology is still
/// unsettled (Windows reporting stale monitor/work-area data mid dock-change);
/// the caller retries rather than overriding the OS's off-screen auto-move with
/// garbage coordinates.
fn position_docker_widget(app: &AppHandle) -> bool {
    reposition_docker_widget(app, false)
}

/// Terminal fallback for the retry chain: force-centre the widget on the
/// current primary monitor's work area. Ignores any saved X. Used only when the
/// validating reposition never settled but a primary monitor exists — better a
/// centred widget on a live monitor than a window stranded off-screen.
fn force_center_docker_widget(app: &AppHandle) -> bool {
    reposition_docker_widget(app, true)
}

fn reposition_docker_widget(app: &AppHandle, force_center: bool) -> bool {
    let Some(w) = app.get_webview_window("docker-widget") else {
        return false;
    };
    let saved_pos = app
        .try_state::<state::docker_widget::DockerWidgetState>()
        .and_then(|s| s.position());

    // No identifiable primary monitor — can't compute a target. Leave the
    // window where the OS put it and let the caller retry.
    let Ok(Some(monitor)) = w.primary_monitor() else {
        return false;
    };
    let mon_size = monitor.size();
    let mon_pos = monitor.position();
    let win_size = w.outer_size().unwrap_or(tauri::PhysicalSize::new(280, 96));

    let mon_top = mon_pos.y;
    let mon_bottom = mon_pos.y + mon_size.height as i32;

    // Y: pin the bottom edge to the work-area bottom (taskbar top). Only trust a
    // value that falls within the primary monitor's vertical span — a stale
    // reading from a just-detached display lands outside it. If it can't be
    // resolved sanely we bail (normal path) or fall back to the monitor's own
    // bottom minus a taskbar clearance (force-centre path).
    let sane_bottom = match commands::docker::work_area_bottom() {
        Some(b) if b > mon_top && b <= mon_bottom => Some(b),
        _ => None,
    };
    let bottom = match sane_bottom {
        Some(b) => b,
        None if force_center => mon_bottom - 48,
        None => return false,
    };

    // X: force-centre ignores the saved X; otherwise restore a saved X that's
    // still within the primary monitor, else centre horizontally.
    let center_x = mon_pos.x + (mon_size.width as i32 - win_size.width as i32) / 2;
    let x = if force_center {
        center_x
    } else {
        saved_pos
            .and_then(|(sx, _sy)| {
                if sx >= mon_pos.x && sx < mon_pos.x + mon_size.width as i32 {
                    Some(sx)
                } else {
                    None
                }
            })
            .unwrap_or(center_x)
    };
    let y = bottom - win_size.height as i32;

    // Validate the final rect intersects some live monitor before committing
    // (skipped on the force-centre path — that rect is derived from the primary
    // monitor we just resolved, so it's live by construction).
    if !force_center
        && !rect_intersects_any_monitor(app, x, y, win_size.width as i32, win_size.height as i32)
    {
        return false;
    }

    let _ = w.set_position(tauri::Position::Physical(tauri::PhysicalPosition::new(
        x, y,
    )));
    true
}

/// Whether the rect `(x, y, w, h)` (physical px) overlaps any monitor currently
/// reported by `available_monitors()`. Guards against committing coordinates on
/// a display that was just detached.
fn rect_intersects_any_monitor(app: &AppHandle, x: i32, y: i32, w: i32, h: i32) -> bool {
    let monitors = match app.available_monitors() {
        Ok(m) => m,
        Err(_) => return false,
    };
    let (rl, rt, rr, rb) = (x, y, x + w, y + h);
    monitors.iter().any(|m| {
        let mp = m.position();
        let ms = m.size();
        let (ml, mt, mr, mb) = (mp.x, mp.y, mp.x + ms.width as i32, mp.y + ms.height as i32);
        // Standard half-open AABB overlap test.
        rl < mr && rr > ml && rt < mb && rb > mt
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Must run before any State::load() below — those loaders read from the
    // new `%LOCALAPPDATA%\fnba-utils\` location and we want them to find any
    // files left behind by older builds.
    state::paths::migrate_legacy_files();

    // Register our AppUserModelID before any window or notification is created.
    // The portable build has no installer-created Start Menu shortcut, so
    // without this Windows silently drops the PII-protection toast. See
    // `aumid` for the full rationale.
    aumid::ensure_registered();

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
        .manage(state::saved_queries::SavedQueriesState::load())
        .manage(state::sql_library::SqlLibraryState::load())
        .manage(state::clipboard_history::ClipboardHistoryState::load())
        .manage(state::test_users::TestUsersState::load())
        .manage(state::docker_widget::DockerWidgetState::load())
        .manage(clipboard::ForegroundCapture::default())
        .manage(commands::clipboard_manager::RevealTokens::default())
        .setup(|app| {
            // --- System Tray ---
            let show = MenuItem::with_id(app, "show", "Show Palette", true, None::<&str>)?;
            // Surface the clipboard daemon's version (independent of the app's,
            // see clipd/Cargo.toml) by reading the on-disk fnba-clipd.exe — the
            // binary the daemon runs from after the launch-time respawn.
            let clipd_line = match clipboard::daemon::daemon_version() {
                Some(v) => format!("Clipboard daemon (fnba-clipd): v{v}"),
                None => "Clipboard daemon (fnba-clipd): not detected".to_string(),
            };
            let about = PredefinedMenuItem::about(
                app,
                Some("About FNBA Utils"),
                Some(AboutMetadata {
                    name: Some("FNBA Utils".to_string()),
                    version: Some(env!("APP_VERSION").to_string()),
                    comments: Some(clipd_line),
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

            // --- Global Hotkey: Win+V / Win+Shift+V (Clipboard Manager) ---
            // Installed FIRST in setup so a downstream `RegisterHotKey`
            // failure (DLP/EDR agents on managed machines commonly claim
            // `Win+Shift+*` chords) doesn't short-circuit setup before the
            // LL hook is in place. Uses a `WH_KEYBOARD_LL` hook rather than
            // `RegisterHotKey` — see `clipboard::hotkey` for rationale.
            clipboard::hotkey::spawn(app.handle().clone());

            // --- Global Shortcut: Win+Shift+F (command palette) ---
            // All `on_shortcut` calls below are best-effort: a single failure
            // (typically a corporate DLP agent owning the chord) must NOT
            // take down the rest of setup — that regression is what broke
            // Win+V on managed machines historically.
            if let Err(e) = app.global_shortcut().on_shortcut(
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
                                // If the display topology changed while the
                                // palette was hidden, its WebView2 surface can
                                // come back stalled (backdrop paints, card never
                                // does). Kick it once now that it's visible.
                                if display_watch::take_display_changed_flag() {
                                    display_watch::kick_window(&w);
                                }
                                // Signal the frontend that the palette just
                                // became visible so it can (re)focus the search
                                // input. CommandInput's onMounted focus fires
                                // only once (the component stays mounted across
                                // hide/show), so without this the caret
                                // intermittently fails to land in the box.
                                let _ = app.emit("palette-shown", ());
                            }
                        }
                    }
                },
            ) {
                eprintln!("Failed to register Super+Shift+F: {e}");
            }

            // --- Global Shortcut: Win+Shift+N (launch into MRU project) ---
            // Looks up the most-recently-used project from ProjectsState and
            // emits `mc-mru-launch` with its cwd + displayName. The Mission
            // Control window's frontend listens and calls the same
            // start_new_claude_session pipeline the palette uses.
            //
            // No-op (silently) if the registry is empty; user can use
            // Win+Shift+F → "new claude" to seed the registry.
            if let Err(e) = app.global_shortcut().on_shortcut(
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
            ) {
                eprintln!("Failed to register Super+Shift+N: {e}");
            }

            // --- Global Shortcut: Ctrl+Shift+Tab (cycle session-detail windows) ---
            // When you're juggling multiple session-detail panels, this is the
            // fast path to "focus the next one." Order = label asc, which is
            // stable per session_id hash.
            if let Err(e) = app.global_shortcut().on_shortcut(
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
            ) {
                eprintln!("Failed to register Control+Shift+Tab: {e}");
            }

            // --- Global Shortcut: Win+Shift+C (Mission Control) ---
            if let Err(e) = app.global_shortcut().on_shortcut(
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
            ) {
                eprintln!("Failed to register Super+Shift+C: {e}");
            }

            // --- Global Shortcut: Win+Shift+J (JSON / Markdown Viewer / Switcher) ---
            // No viewers open → emit to the main window to spawn a fresh one.
            // Viewers exist → show the switcher overlay (json-switcher window).
            // Covers both json-viewer:* and markdown-viewer:* windows.
            if let Err(e) = app.global_shortcut().on_shortcut(
                "Super+Shift+J",
                move |app: &AppHandle, _shortcut, event| {
                    if event.state == ShortcutState::Pressed {
                        let has_viewer = app
                            .webview_windows()
                            .keys()
                            .any(|l| l.starts_with("json-viewer:") || l.starts_with("markdown-viewer:"));
                        if !has_viewer {
                            // No viewers open: skip the switcher, spawn a fresh one via the
                            // always-alive main webview (single source of truth for window opts).
                            // Scoped to "main" so future hashless windows can't double-spawn.
                            let _ = app.emit_to("main", "json-viewer-new", ());
                        } else if let Some(sw) = app.get_webview_window("json-switcher") {
                            let _ = sw.center();
                            let _ = sw.show();
                            let _ = sw.set_focus();
                            let _ = app.emit("json-switcher-refresh", ());
                        }
                    }
                },
            ) {
                eprintln!("Failed to register Super+Shift+J: {e}");
            }

            // --- Clipboard daemon (fnba-clipd.exe) ---
            // Capture lives in a separate background process so it keeps
            // running even when fnba-utils itself is closed. We ensure it's
            // alive on startup and register it for Windows auto-start on
            // login. See `clipboard::daemon`.
            if let Err(e) = clipboard::daemon::ensure_running_and_registered() {
                eprintln!("fnba-clipd auto-launch failed: {e}");
            }

            // --- Global Shortcut: Win+Shift+D (Standup Panel) ---
            // Registered unconditionally; the panel window only exists when the
            // standup feature is enabled, so the shortcut is a no-op otherwise.
            if let Err(e) = app.global_shortcut().on_shortcut(
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
            ) {
                eprintln!("Failed to register Super+Shift+D: {e}");
            }

            // --- Docker Widget ---
            // Always-on-top ambient gadget on the PRIMARY ("main") monitor,
            // pinned flush above the taskbar. The bottom edge is always anchored
            // to the work-area bottom (taskbar top); only the horizontal position
            // is restored from a saved value (so it can be nudged left/right).
            //
            // Positioning is factored into `position_docker_widget` so the
            // display-change watcher can re-run it after a dock/undock. It's
            // fallible now (validates against the live monitor layout); at
            // startup the topology is settled, so a false result is just logged.
            if !position_docker_widget(app.handle()) {
                eprintln!(
                    "docker-widget: initial positioning deferred (no valid monitor/work-area yet)"
                );
            }

            if let Some(w) = app.get_webview_window("docker-widget") {
                // Allow the window to shrink to the heading height — the OS
                // default minimum tracking size for a resizable window would
                // otherwise clamp it, leaving the heading floating above the
                // taskbar with empty space below.
                let _ = w.set_min_size(Some(tauri::LogicalSize::new(1.0, 1.0)));

                let _ = w.set_always_on_top(true);
                // Intentionally NOT calling set_focus() — the widget is ambient
                // and must not steal focus from the user's active window.

                // Let the click-outside hook hit-test against this window.
                widget_focus::track_window(&w);
            }

            // Start the Docker background poll thread (emits `docker-status`),
            // the foreground watch (collapses on window switch / Alt-Tab), and
            // the click-outside hook (collapses on a click off the widget) —
            // both emit `docker-widget-defocus`.
            commands::docker::spawn_poll_thread(app.handle().clone());
            spawn_foreground_watch(app.handle().clone());
            widget_focus::spawn(app.handle().clone());

            // Watch for display-topology changes (dock/undock, monitor add/
            // remove, taskbar move). On a settled change it re-pins the docker
            // widget and notifies windows so nothing is stranded on a detached
            // display until the next app restart.
            display_watch::install(app.handle().clone());

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::hide_window,
            commands::open_app_data_folder,
            config::get_app_config,
            commands::standup::run_standup,
            commands::standup::preview_standup,
            commands::standup::post_standup_to_teams,
            commands::standup::copy_standup_report,
            commands::standup::get_standup_report,
            commands::standup::get_standup_last_run,
            commands::standup::get_standup_panel_state,
            commands::standup::set_issue_hidden,
            commands::standup::clear_hidden_issues,
            commands::standup::set_issue_order,
            commands::standup::clear_manual_order,
            commands::standup::set_standup_issue_post_to_teams,
            commands::standup::get_run_snapshot,
            commands::standup::get_issue_detail,
            commands::assume_identity::get_identity_data,
            commands::assume_identity::execute_assume_identity,
            commands::assume_identity::save_custom_entry,
            commands::assume_identity::delete_custom_entry,
            commands::assume_identity::pin_favorite,
            commands::assume_identity::remove_favorite,
            commands::assume_identity::mark_favorite_used,
            commands::directory::get_all_rights,
            commands::directory::get_right_associates,
            commands::directory::search_associates,
            commands::directory::get_associate_rights,
            commands::directory::get_assume_login,
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
            commands::saved_queries::list_sql_groups,
            commands::saved_queries::add_sql_group,
            commands::saved_queries::rename_sql_group,
            commands::saved_queries::set_sql_group_color,
            commands::saved_queries::set_sql_group_pinned,
            commands::saved_queries::reorder_sql_groups,
            commands::saved_queries::remove_sql_group,
            commands::saved_queries::list_sql_queries,
            commands::saved_queries::add_sql_query,
            commands::saved_queries::update_sql_query,
            commands::saved_queries::move_sql_query_to_group,
            commands::saved_queries::remove_sql_query,
            commands::saved_queries::record_sql_query_used,
            commands::saved_queries::migrate_legacy_sql_queries,
            commands::sql_library::get_sql_library,
            commands::sql_library::sql_library_tree,
            commands::sql_library::sql_library_read,
            commands::sql_library::sql_library_write,
            commands::sql_library::sql_library_mkdir,
            commands::sql_library::sql_library_delete,
            commands::sql_library::sql_library_rename,
            commands::clipboard_manager::list_clipboard_entries,
            commands::clipboard_manager::get_clipboard_entry,
            commands::clipboard_manager::paste_clipboard_entry,
            commands::clipboard_manager::request_sensitive_reveal,
            commands::clipboard_manager::delete_clipboard_entry,
            commands::clipboard_manager::pin_clipboard_entry,
            commands::clipboard_manager::set_clipboard_entry_label,
            commands::clipboard_manager::update_clipboard_entry_content,
            commands::clipboard_manager::set_clipboard_entry_sensitivity,
            commands::clipboard_manager::clear_clipboard_history,
            commands::clipboard_manager::get_clipboard_settings,
            commands::clipboard_manager::set_clipboard_settings,
            commands::clipboard_manager::hide_clipboard_window,
            commands::clipboard_manager::get_clipboard_max_captured_at,
            commands::clipboard_manager::list_test_users,
            commands::clipboard_manager::upsert_test_user,
            commands::clipboard_manager::delete_test_user,
            commands::clipboard_manager::set_test_user_enabled,
            commands::json_viewer::copy_text,
            commands::markdown_docs::write_markdown_doc,
            commands::markdown_docs::read_markdown_doc,
            commands::markdown_docs::delete_markdown_doc,
            commands::markdown_docs::cleanup_markdown_docs,
            commands::markdown_docs::open_markdown_file,
            commands::markdown_docs::save_markdown_as,
            commands::markdown_docs::save_markdown_file,
            commands::markdown_docs::stat_markdown_file,
            commands::markdown_docs::read_markdown_file,
            commands::terminal::run_in_terminal,
            commands::fs::resolve_path,
            commands::fs::open_in_notepadpp,
            commands::fs::reveal_in_explorer,
            commands::docker::get_docker_status,
            commands::docker::docker_start,
            commands::docker::docker_stop,
            commands::docker::docker_restart,
            commands::docker::docker_logs,
            commands::docker::list_pinned_containers,
            commands::docker::pin_container,
            commands::docker::unpin_container,
            commands::docker::save_docker_widget_position,
            commands::docker::get_docker_widget_position,
            commands::docker::docker_widget_anchor_bottom,
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
