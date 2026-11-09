//! Tauri IPC commands. Every mutating command returns a fresh `AppStateDto`
//! so the frontend can apply it without waiting for the next engine tick.

use std::path::PathBuf;
use std::time::Duration;

use chrono::Utc;
use tauri::{AppHandle, State};

use crate::aladhan;
use crate::engine::{self, Engine};
use crate::models::{
    AppStateDto, AsrSchool, Location, LocationPreviewDto, Prayer, PreviewRequest, Settings,
};
use crate::timeutil;
use crate::SharedEngine;

fn with_engine<R>(shared: &SharedEngine, f: impl FnOnce(&mut Engine) -> R) -> R {
    let mut engine = shared.engine.lock().expect("engine lock");
    f(&mut engine)
}

#[tauri::command]
pub fn get_state(shared: State<'_, SharedEngine>) -> AppStateDto {
    with_engine(&shared, |engine| engine.snapshot())
}

/// Fetches a live day for the wizard without touching any state.
#[tauri::command]
pub async fn preview_location(req: PreviewRequest) -> Result<LocationPreviewDto, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(20))
            .build()
            .map_err(|e| e.to_string())?;
        let params = aladhan::FetchParams {
            city: req.city.trim(),
            country: &req.country_code,
            method: req.method,
            school: req.school,
        };

        // Ask with today (device-local) first; if the city's calendar day
        // differs (timezone offset), re-ask for the correct day.
        let guess = Utc::now().date_naive();
        let (day, tz_name) = aladhan::fetch_day_by_city(&client, &params, guess)?;
        let tz = timeutil::parse_tz(&tz_name).ok_or_else(|| {
            if tz_name.is_empty() {
                "API response did not include a timezone for this city".to_string()
            } else {
                format!("API returned unknown timezone: {tz_name}")
            }
        })?;
        let city_today = Utc::now().with_timezone(&tz).date_naive();
        let day = if city_today != guess {
            aladhan::fetch_day_by_city(&client, &params, city_today)?.0
        } else {
            day
        };

        let location = Location {
            city: req.city.trim().to_string(),
            country: req.country,
            country_code: req.country_code,
            timezone: tz_name,
        };
        Ok(LocationPreviewDto {
            location,
            day: engine::day_timings_dto(&day, Some(tz), false),
        })
    })
    .await
    .map_err(|e| format!("preview task failed: {e}"))?
}

#[tauri::command]
pub fn save_location(
    shared: State<'_, SharedEngine>,
    location: Location,
    method: u8,
    school: AsrSchool,
) -> Result<AppStateDto, String> {
    let dto = with_engine(&shared, |engine| {
        engine.save_location(location, method, school);
        engine.tick();
        engine.snapshot()
    });
    engine::wake_up(&shared);
    Ok(dto)
}

#[tauri::command]
pub fn update_settings(
    shared: State<'_, SharedEngine>,
    settings: Settings,
) -> Result<AppStateDto, String> {
    let (saved, dto) = with_engine(&shared, |engine| {
        let saved = engine.apply_settings(settings);
        engine.tick();
        (saved, engine.snapshot())
    });
    engine::wake_up(&shared);
    saved.map_err(|e| format!("couldn't write settings file: {e}"))?;
    Ok(dto)
}

#[tauri::command]
pub fn set_prayer_done(
    shared: State<'_, SharedEngine>,
    prayer: Prayer,
    done: bool,
) -> AppStateDto {
    with_engine(&shared, |engine| {
        engine.set_prayer_done(prayer, done);
        engine.tick();
        engine.snapshot()
    })
}

#[tauri::command]
pub fn alarm_done(shared: State<'_, SharedEngine>) -> AppStateDto {
    with_engine(&shared, |engine| {
        engine.alarm_done();
        engine.tick();
        engine.snapshot()
    })
}

#[tauri::command]
pub fn alarm_snooze(shared: State<'_, SharedEngine>, minutes: i64) -> AppStateDto {
    with_engine(&shared, |engine| {
        engine.alarm_snooze(minutes);
        engine.tick();
        engine.snapshot()
    })
}

#[tauri::command]
pub fn alarm_dismiss(shared: State<'_, SharedEngine>) -> AppStateDto {
    with_engine(&shared, |engine| {
        engine.alarm_dismiss();
        engine.tick();
        engine.snapshot()
    })
}

#[tauri::command]
pub fn test_sound(shared: State<'_, SharedEngine>, volume: Option<f32>) {
    with_engine(&shared, |engine| engine.test_sound(volume));
}

/// Makes a user-picked audio file the alarm tone: validates it, copies it
/// into the app config dir (so it survives reboots and file moves) and
/// switches the audio thread to it.
#[tauri::command]
pub fn set_alarm_tone(shared: State<'_, SharedEngine>, path: String) -> Result<AppStateDto, String> {
    let source = PathBuf::from(&path);
    let extension = source
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .ok_or_else(|| "file has no extension".to_string())?;
    if !matches!(extension.as_str(), "mp3" | "wav" | "ogg" | "oga" | "flac") {
        return Err(format!(
            "unsupported format .{extension} — use mp3, wav, ogg or flac"
        ));
    }
    let meta = std::fs::metadata(&source).map_err(|e| format!("can't read file: {e}"))?;
    if meta.len() > 20 * 1024 * 1024 {
        return Err("file is larger than 20 MB".to_string());
    }
    if !crate::sound::is_decodable(&source) {
        return Err(
            "couldn't decode this file as audio — try mp3, wav, ogg or flac".to_string(),
        );
    }

    with_engine(&shared, |engine| {
        let result = engine.set_alarm_tone(source, &extension);
        engine.tick();
        result.map(|()| engine.snapshot())
    })
}

#[tauri::command]
pub fn reset_alarm_tone(shared: State<'_, SharedEngine>) -> Result<AppStateDto, String> {
    with_engine(&shared, |engine| {
        let result = engine.reset_alarm_tone();
        engine.tick();
        result.map(|()| engine.snapshot())
    })
}

// -- autostart (Start with Windows / launch at login) -----------------------

#[tauri::command]
pub fn get_autostart(app: AppHandle) -> bool {
    use tauri_plugin_autostart::ManagerExt;
    app.autolaunch().is_enabled().unwrap_or(false)
}

#[tauri::command]
pub fn set_autostart(app: AppHandle, enabled: bool) -> Result<bool, String> {
    use tauri_plugin_autostart::ManagerExt;
    let autolaunch = app.autolaunch();
    if enabled {
        autolaunch.enable().map_err(|e| e.to_string())?;
    } else {
        autolaunch.disable().map_err(|e| e.to_string())?;
    }
    Ok(autolaunch.is_enabled().unwrap_or(enabled))
}
