mod commands;
mod config;
mod db;
mod models;
mod standup_db;

use tauri::{
    menu::{AboutMetadata, Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager, RunEvent,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_shell::init())
        .manage(config::AppConfig::load())
        .manage(models::mission_control::ClaudeIoState::new())
        .manage(models::mission_control::SqlQueryState::new())
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
            commands::mission_control::start_claude_session,
            commands::mission_control::send_claude_message,
            commands::mission_control::stop_claude_session,
            commands::mission_control::open_in_explorer,
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
