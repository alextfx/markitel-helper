//! Markitel Helper — Tauri 2.x app entry + plugin wiring.
//!
//! The helper is a long-running menu-bar/tray app. On first launch it
//! waits for a `markitel://pair?code=XXXXXX` deep link (either from the
//! website or a "Pair" button in the tray). After pairing it:
//!
//!   1. stores the user's API key in the OS keychain
//!   2. discovers MT5 installs on this machine
//!   3. asks the user to close MT5 if running, then writes the keyed EA,
//!      whitelists markitel.com in terminal.ini / common.ini, and
//!      (pending Phase 0 spike) writes a "Markitel" MT5 profile
//!   4. offers to launch MT5
//!   5. keeps the tray icon green while the backend reports a recent
//!      heartbeat on this user's connection.

mod api;
mod commands;
mod config;
mod ea_writer;
mod ini_writer;
mod keychain;
mod mt5_discovery;
mod mt5_launcher;
mod pairing;
mod profile_writer;
mod telemetry;

use tauri::{
    image::Image,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager, WindowEvent,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::init();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_deep_link::init())
        .setup(|app| {
            // ── Tray icon + menu ──
            let show_item = MenuItem::with_id(app, "show", "Open Markitel Helper…", true, None::<&str>)?;
            let pair_item = MenuItem::with_id(app, "pair", "Pair with Markitel…", true, None::<&str>)?;
            let rotate_item = MenuItem::with_id(app, "rotate", "Rotate API Key", true, None::<&str>)?;
            let reinstall_item = MenuItem::with_id(app, "reinstall", "Reinstall EA", true, None::<&str>)?;
            let separator = MenuItem::with_id(app, "sep", "—", false, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(
                app,
                &[&show_item, &pair_item, &rotate_item, &reinstall_item, &separator, &quit_item],
            )?;

            let _tray = TrayIconBuilder::with_id("main-tray")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(win) = app.get_webview_window("main") {
                            let _ = win.show();
                            let _ = win.set_focus();
                        }
                    }
                    "pair" => {
                        if let Some(win) = app.get_webview_window("main") {
                            let _ = win.show();
                            let _ = win.set_focus();
                            let _ = app.emit("helper://navigate", "pair");
                        }
                    }
                    "rotate" => {
                        let _ = app.emit("helper://rotate-key", ());
                    }
                    "reinstall" => {
                        let _ = app.emit("helper://reinstall-ea", ());
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        if let Some(win) = tray.app_handle().get_webview_window("main") {
                            let _ = win.show();
                            let _ = win.set_focus();
                        }
                    }
                })
                .build(app)?;

            // ── Deep-link handler ──
            //
            // When the user clicks `markitel://pair?code=XXXXXX`, Tauri's
            // deep-link plugin fires this callback. We parse the URL,
            // extract the code, and kick off the exchange flow. The
            // actual exchange happens in pairing::consume_deep_link.
            let app_handle = app.handle().clone();
            app.deep_link().on_open_url(move |event| {
                for url in event.urls() {
                    log::info!("deep-link received: {}", url);
                    pairing::handle_deep_link(&app_handle, url.clone());
                }
            });

            // ── Register already-pending deep link (macOS sometimes
            //    delivers the URL before setup() finishes). ──
            #[cfg(any(target_os = "macos", target_os = "windows"))]
            {
                let _ = app.deep_link().register("markitel");
            }

            // ── Hide window on close button (don't actually quit) ──
            if let Some(win) = app.get_webview_window("main") {
                let win_clone = win.clone();
                win.on_window_event(move |event| {
                    if let WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = win_clone.hide();
                    }
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::status,
            commands::start_pairing,
            commands::pair_with_code,
            commands::discover_mt5,
            commands::install_ea,
            commands::rotate_key,
            commands::launch_mt5,
            commands::is_mt5_running,
            commands::log_telemetry,
            commands::get_helper_version,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Markitel Helper");
}
