//! Prayer Times — Tauri application entry.
//!
//! Module map (see ARCHITECTURE.md for the full picture):
//! - `models`    — domain types + DTOs shared with the frontend
//! - `aladhan`   — Aladhan API client + month disk cache
//! - `schedule`  — pure day → alarm-events computation
//! - `engine`    — ticking scheduler, alarm firing, tray/status updates
//! - `sound`     — synthesized alarm chime (rodio)
//! - `alarm`     — notification + always-on-top popup window
//! - `tray`      — system tray icon/menu (Windows tray, macOS menu bar,
//!                 Ubuntu app indicator)
//! - `settings`  — settings.json persistence
//! - `runtime`   — per-day runtime state persistence (fired/snooze/done)
//! - `commands`  — IPC commands

mod alarm;
mod backup;
mod aladhan;
mod commands;
mod engine;
mod geocode;
mod history;
mod models;
mod runtime;
mod schedule;
mod settings;
mod sound;
mod timeutil;
mod tray;
mod tzcountry;
mod windows;

use std::sync::{Arc, Condvar, Mutex};

use tauri::Manager;

/// Shared application state: the engine behind a mutex plus the condvar used
/// to wake the engine thread immediately after settings changes.
pub struct SharedEngine {
    pub engine: Mutex<engine::Engine>,
    pub wake: Arc<(Mutex<bool>, Condvar)>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let wake: Arc<(Mutex<bool>, Condvar)> = Arc::new((Mutex::new(false), Condvar::new()));

    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        // Boot launches pass --hidden so the app starts silently in the tray.
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--hidden"]),
        ))
        .on_window_event(|window, event| {
            // Closing the main window hides it — the app keeps running in the
            // tray so alarms still fire. Quit via the tray menu.
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                match window.label() {
                    "main" | "alarm" => {
                        api.prevent_close();
                        let _ = window.hide();
                    }
                    _ => {}
                }
            }
        })
        .setup({
            let wake = wake.clone();
            move |app| {
                let handle = app.handle().clone();

                let engine = engine::Engine::new(handle.clone())
                    .map_err(|e| Box::<dyn std::error::Error>::from(e))?;
                app.manage(SharedEngine {
                    engine: Mutex::new(engine),
                    wake: wake.clone(),
                });

                tray::build(&handle)?;
                alarm::ensure_window(&handle);

                // The main window is created hidden (see tauri.conf.json) to
                // avoid a flash on boot launches; show it on normal starts.
                let hidden_launch = std::env::args().any(|arg| arg == "--hidden");
                if !hidden_launch {
                    windows::show_main(&handle);
                }

                std::thread::spawn(move || engine::run_loop(handle, wake));
                Ok(())
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_state,
            commands::search_cities,
            commands::preview_location,
            commands::save_location,
            commands::update_settings,
            commands::set_prayer_done,
            commands::get_history_month,
            commands::set_history_entry,
            commands::export_backup,
            commands::preview_backup,
            commands::import_backup,
            commands::alarm_done,
            commands::alarm_snooze,
            commands::alarm_dismiss,
            commands::preview_alarm,
            commands::test_sound,
            commands::set_alarm_tone,
            commands::reset_alarm_tone,
            commands::get_autostart,
            commands::set_autostart,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
