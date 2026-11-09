//! Tauri IPC commands. Every mutating command returns a fresh `AppStateDto`
//! so the frontend can apply it without waiting for the next engine tick.

use std::path::PathBuf;
use std::time::Duration;

use chrono::Utc;
use tauri::{AppHandle, State};

use crate::aladhan::{self, FetchParams, Place};
use crate::engine::{self, Engine};
use crate::models::{
    AppStateDto, BackupPreviewDto, HistoryMonthDto, HistoryStatus, AsrSchool, CityMatchDto, Coordinates, Location, LocationPreviewDto, Prayer,
    PreviewRequest, Settings,
};
use crate::{geocode, timeutil, tzcountry};
use crate::SharedEngine;

fn with_engine<R>(shared: &SharedEngine, f: impl FnOnce(&mut Engine) -> R) -> R {
    let mut engine = shared.engine.lock().expect("engine lock");
    f(&mut engine)
}

#[tauri::command]
pub fn get_state(shared: State<'_, SharedEngine>) -> AppStateDto {
    with_engine(&shared, |engine| engine.snapshot())
}

fn http_client() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(20))
        .user_agent(concat!("prayer-times/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| e.to_string())
}

/// City search for the location picker (towns in `country_code` only).
#[tauri::command]
pub async fn search_cities(query: String, country_code: String) -> Result<Vec<CityMatchDto>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        geocode::search(&http_client()?, &query, &country_code)
    })
    .await
    .map_err(|e| format!("search task failed: {e}"))?
}

/// Fetches a live day for the wizard without touching any state. A picked
/// search result is used as-is (exact coordinates); a typed name is looked
/// up by the API and rejected when it lands in another country.
#[tauri::command]
pub async fn preview_location(req: PreviewRequest) -> Result<LocationPreviewDto, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let client = http_client()?;
        let (location, typed_day) = match req.picked {
            Some(picked) => (picked_location(picked, &req.country, &req.country_code)?, None),
            None => {
                let (location, day) = typed_location(&client, &req)?;
                (location, Some(day))
            }
        };

        let tz = timeutil::parse_tz(&location.timezone)
            .ok_or_else(|| format!("unknown timezone: {}", location.timezone))?;
        let city_today = Utc::now().with_timezone(&tz).date_naive();
        let day = match typed_day {
            Some(day) if day.date == city_today => day,
            _ => {
                let params = FetchParams {
                    place: Place::of(&location),
                    method: req.method,
                    school: req.school,
                };
                aladhan::fetch_day(&client, &params, city_today)?.0
            }
        };

        Ok(LocationPreviewDto {
            day: engine::day_timings_dto(&day, Some(tz), false),
            location,
        })
    })
    .await
    .map_err(|e| format!("preview task failed: {e}"))?
}

fn picked_location(picked: CityMatchDto, country: &str, country_code: &str) -> Result<Location, String> {
    if !picked.country_code.eq_ignore_ascii_case(country_code) {
        return Err(format!("{} isn't in {country}.", picked.name));
    }
    if timeutil::parse_tz(&picked.timezone).is_none() {
        return Err(format!("unknown timezone: {}", picked.timezone));
    }
    Ok(Location {
        city: picked.name,
        country: country.to_string(),
        country_code: country_code.to_ascii_uppercase(),
        timezone: picked.timezone,
        region: picked.region,
        coordinates: Some(Coordinates {
            latitude: picked.latitude,
            longitude: picked.longitude,
        }),
    })
}

/// Looks a typed name up by the API (which also tells the timezone).
fn typed_location(
    client: &reqwest::blocking::Client,
    req: &PreviewRequest,
) -> Result<(Location, aladhan::DayTimings), String> {
    let city = req.city.trim();
    let params = FetchParams {
        place: Place::City {
            city,
            country: &req.country_code,
        },
        method: req.method,
        school: req.school,
    };
    // Device-local date is a guess; the caller refetches if the city's
    // calendar day differs.
    let (day, timezone) = aladhan::fetch_day(client, &params, Utc::now().date_naive())?;
    if timezone.is_empty() {
        return Err("API response did not include a timezone for this city".to_string());
    }
    if tzcountry::zone_in_country(&timezone, &req.country_code) == Some(false) {
        return Err(format!(
            "Couldn't find “{city}” in {}: the closest match is in the {timezone} timezone. \
             Check the spelling, or pick your city from the search results.",
            req.country
        ));
    }
    let location = Location {
        city: city.to_string(),
        country: req.country.clone(),
        country_code: req.country_code.clone(),
        timezone,
        region: None,
        coordinates: None,
    };
    Ok((location, day))
}

#[tauri::command]
pub fn save_location(
    shared: State<'_, SharedEngine>,
    location: Location,
    method: u8,
    school: AsrSchool,
) -> Result<AppStateDto, String> {
    // Same check as the preview, in case a mismatched location gets here.
    if let Some(warning) = engine::location_warning(&location) {
        return Err(warning);
    }
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

/// Marks or unmarks a prayer for the current Prayer Day. Marking a prayer
/// that hasn't started yet is refused.
#[tauri::command]
pub fn set_prayer_done(
    shared: State<'_, SharedEngine>,
    prayer: Prayer,
    done: bool,
) -> Result<AppStateDto, String> {
    with_engine(&shared, |engine| {
        engine.set_prayer_done(prayer, done)?;
        engine.tick();
        Ok(engine.snapshot())
    })
}

/// The History tab's month (`month` is 1–12).
#[tauri::command]
pub fn get_history_month(shared: State<'_, SharedEngine>, year: i32, month: u32) -> HistoryMonthDto {
    with_engine(&shared, |engine| engine.history_month(year, month))
}

/// Sets one prayer on `day` (`YYYY-MM-DD`) from the History tab; `None`
/// clears it. Returns that day's month.
#[tauri::command]
pub fn set_history_entry(
    shared: State<'_, SharedEngine>,
    day: String,
    prayer: Prayer,
    status: Option<HistoryStatus>,
) -> Result<HistoryMonthDto, String> {
    let date = chrono::NaiveDate::parse_from_str(&day, "%Y-%m-%d")
        .map_err(|_| format!("invalid day: {day}"))?;
    let dto = with_engine(&shared, |engine| {
        engine.set_history_entry(date, prayer, status)?;
        engine.tick();
        Ok::<_, String>(engine.history_month(chrono::Datelike::year(&date), chrono::Datelike::month(&date)))
    })?;
    Ok(dto)
}

/// Backups larger than this are refused (decades of history fit in ~2 MB).
const MAX_BACKUP_BYTES: u64 = 20 * 1024 * 1024;

fn read_backup(path: &str) -> Result<crate::backup::BackupFile, String> {
    let size = std::fs::metadata(path)
        .map_err(|e| format!("couldn't open the file: {e}"))?
        .len();
    if size > MAX_BACKUP_BYTES {
        return Err("This file is too large to be a Prayer Times backup".into());
    }
    let text = std::fs::read_to_string(path).map_err(|e| format!("couldn't read the file: {e}"))?;
    crate::backup::parse(&text)
}

/// Writes settings + history to `path` (chosen with the save dialog).
#[tauri::command]
pub fn export_backup(shared: State<'_, SharedEngine>, path: String) -> Result<(), String> {
    let text = with_engine(&shared, |engine| engine.export_backup())?;
    std::fs::write(&path, text).map_err(|e| format!("couldn't write the backup: {e}"))
}

/// What importing `path` would bring; changes nothing.
#[tauri::command]
pub fn preview_backup(path: String) -> Result<BackupPreviewDto, String> {
    Ok(crate::backup::preview(&read_backup(&path)?))
}

/// Merges `path`'s history (nothing is deleted) and, when
/// `include_settings`, applies its settings.
#[tauri::command]
pub fn import_backup(
    shared: State<'_, SharedEngine>,
    path: String,
    include_settings: bool,
) -> Result<AppStateDto, String> {
    let file = read_backup(&path)?;
    let dto = with_engine(&shared, |engine| {
        engine.import_backup(file, include_settings)?;
        engine.tick();
        Ok::<_, String>(engine.snapshot())
    })?;
    engine::wake_up(&shared);
    Ok(dto)
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

/// Dev builds only (`pnpm tauri dev`): shows a pretend alarm to check the
/// popup design. Release builds refuse it.
#[tauri::command]
pub fn preview_alarm(
    shared: State<'_, SharedEngine>,
    kind: engine::AlarmKind,
) -> Result<AppStateDto, String> {
    if !cfg!(debug_assertions) {
        return Err("Alarm preview is only available in development builds".into());
    }
    with_engine(&shared, |engine| {
        engine.preview_alarm(kind)?;
        Ok(engine.snapshot())
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

/// Live-network checks of the location flow. Not run by default (CI has no
/// business depending on third-party APIs): `cargo test -- --ignored`.
#[cfg(test)]
mod network_tests {
    use super::*;

    fn typed(city: &str, country: &str, code: &str) -> PreviewRequest {
        PreviewRequest {
            city: city.to_string(),
            country: country.to_string(),
            country_code: code.to_string(),
            method: 3,
            school: AsrSchool::Standard,
            picked: None,
        }
    }

    #[test]
    #[ignore = "network"]
    fn search_finds_real_towns_and_nothing_for_typos() {
        let client = http_client().unwrap();
        let kolk = geocode::search(&client, "Kolk", "IN").unwrap();
        assert_eq!(kolk[0].name, "Kolkata");
        assert_eq!(kolk[0].timezone, "Asia/Kolkata");
        assert!(geocode::search(&client, "Londn", "GB").unwrap().is_empty());
        // Country filter: no Mumbai in Bangladesh.
        assert!(geocode::search(&client, "Mumbai", "BD")
            .unwrap()
            .iter()
            .all(|m| m.country_code == "BD"));
    }

    #[test]
    #[ignore = "network"]
    fn typed_city_in_wrong_country_is_rejected() {
        let client = http_client().unwrap();
        let err = typed_location(&client, &typed("Londn", "United Kingdom", "GB")).unwrap_err();
        assert!(err.contains("America/New_York"), "{err}");
        assert!(typed_location(&client, &typed("Mumbai", "Bangladesh", "BD")).is_err());
        let (ok, _) = typed_location(&client, &typed("Dhka", "Bangladesh", "BD")).unwrap();
        assert_eq!(ok.timezone, "Asia/Dhaka");
    }

    #[test]
    #[ignore = "network"]
    fn picked_city_gets_same_times_as_city_lookup() {
        let client = http_client().unwrap();
        let london = geocode::search(&client, "London", "GB").unwrap().remove(0);
        let location = picked_location(london, "United Kingdom", "GB").unwrap();
        let date = chrono::NaiveDate::from_ymd_opt(2026, 11, 9).unwrap();
        let params = |place| FetchParams {
            place,
            method: 3,
            school: AsrSchool::Standard,
        };
        let (by_coords, tz) = aladhan::fetch_day(&client, &params(Place::of(&location)), date).unwrap();
        let (by_name, _) = aladhan::fetch_day(
            &client,
            &params(Place::City {
                city: "London",
                country: "GB",
            }),
            date,
        )
        .unwrap();
        assert_eq!(tz, "Europe/London");
        let fajr = |d: &aladhan::DayTimings| d.times[&Prayer::Fajr];
        let diff = (fajr(&by_coords) - fajr(&by_name)).num_minutes().abs();
        assert!(diff <= 2, "coords {} vs name {}", fajr(&by_coords), fajr(&by_name));

        let month = aladhan::fetch_month(&client, &params(Place::of(&location)), 2026, 11).unwrap();
        assert_eq!(month.days.len(), 30);
        assert_eq!(month.timezone, "Europe/London");
    }
}
