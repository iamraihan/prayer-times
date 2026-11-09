//! Settings persistence — a plain JSON file in the OS config dir.

use std::path::PathBuf;

use tauri::{AppHandle, Manager};

use crate::models::Settings;

pub fn config_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("no config dir: {e}"))?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create config dir: {e}"))?;
    Ok(dir)
}

pub fn load(app: &AppHandle) -> Settings {
    let Ok(dir) = config_dir(app) else {
        return Settings::default();
    };
    std::fs::read_to_string(dir.join("settings.json"))
        .ok()
        .and_then(|text| crate::models::settings_from_json(&text))
        .unwrap_or_default()
}

pub fn save(app: &AppHandle, settings: &Settings) -> Result<(), String> {
    let dir = config_dir(app)?;
    let json = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("settings.json"), json).map_err(|e| e.to_string())
}
