//! Backup file: settings plus prayer history in one versioned JSON file, to
//! move to another device. Import merges history (never deletes) and may
//! apply the settings.

use std::collections::BTreeSet;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::models::{BackupPreviewDto, HistoryEntry, Settings};

pub const FORMAT: &str = "prayer-times-backup";
pub const VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupFile {
    pub format: String,
    pub version: u32,
    pub exported_at: String,
    pub settings: Settings,
    pub history: Vec<HistoryEntry>,
}

pub fn to_json(settings: &Settings, history: Vec<HistoryEntry>, now: DateTime<Utc>) -> String {
    let file = BackupFile {
        format: FORMAT.into(),
        version: VERSION,
        exported_at: now.to_rfc3339(),
        settings: settings.clone(),
        history,
    };
    serde_json::to_string_pretty(&file).unwrap_or_default()
}

/// Reads a backup file. The format and version are checked first, so a
/// foreign or newer file gets a clear message instead of a parse error.
pub fn parse(text: &str) -> Result<BackupFile, String> {
    let raw: serde_json::Value =
        serde_json::from_str(text).map_err(|_| "This file is damaged or not a backup (invalid JSON)".to_string())?;
    if raw.get("format").and_then(|f| f.as_str()) != Some(FORMAT) {
        return Err("This is not a Prayer Times backup file".into());
    }
    let version = raw.get("version").and_then(|v| v.as_u64()).unwrap_or(0);
    if version > VERSION as u64 {
        return Err("This backup was made by a newer version of Prayer Times; update the app first".into());
    }
    serde_json::from_value(raw).map_err(|e| format!("This backup is damaged: {e}"))
}

/// The settings an import applies: the backup's, except that this device
/// keeps its location when the backup has none, and always keeps its own
/// alarm tone (the tone file isn't in backups).
pub fn settings_to_apply(current: &Settings, mut incoming: Settings) -> Settings {
    if incoming.location.is_none() {
        incoming.location = current.location.clone();
    }
    incoming.alarm_sound = current.alarm_sound.clone();
    incoming
}

pub fn preview(file: &BackupFile) -> BackupPreviewDto {
    let days: BTreeSet<_> = file.history.iter().map(|e| e.day).collect();
    BackupPreviewDto {
        entries: file.history.len() as u32,
        days: days.len() as u32,
        first_day: days.first().map(|d| d.to_string()),
        last_day: days.last().map(|d| d.to_string()),
        location: file.settings.location.as_ref().map(|l| l.city.clone()),
        has_custom_tone: file.settings.alarm_sound.is_some(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{HistoryStatus, Location, Prayer};
    use chrono::NaiveDate;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    fn sample() -> (Settings, Vec<HistoryEntry>) {
        let mut settings = Settings::default();
        settings.location = Some(Location {
            city: "Dhaka".into(),
            country: "Bangladesh".into(),
            country_code: "BD".into(),
            timezone: "Asia/Dhaka".into(),
            region: None,
            coordinates: None,
        });
        settings.alarm_sound = Some("alarm-tone.mp3".into());
        let history = vec![
            HistoryEntry {
                day: d("2026-11-01"),
                prayer: Prayer::Fajr,
                status: HistoryStatus::Done,
                done_at: Some(DateTime::parse_from_rfc3339("2026-11-01T00:10:00Z").unwrap().with_timezone(&Utc)),
            },
            HistoryEntry { day: d("2026-11-01"), prayer: Prayer::Asr, status: HistoryStatus::Missed, done_at: None },
            HistoryEntry { day: d("2026-11-08"), prayer: Prayer::Isha, status: HistoryStatus::Missed, done_at: None },
        ];
        (settings, history)
    }

    #[test]
    fn round_trips_settings_and_history() {
        let (settings, history) = sample();
        let now = Utc::now();
        let text = to_json(&settings, history.clone(), now);
        let file = parse(&text).unwrap();
        assert_eq!(file.format, FORMAT);
        assert_eq!(file.version, VERSION);
        assert_eq!(file.history, history);
        assert_eq!(file.settings.location.unwrap().city, "Dhaka");
    }

    #[test]
    fn rejects_wrong_format_and_future_version() {
        let e = parse(r#"{"format":"other","version":1}"#).unwrap_err();
        assert!(e.contains("not a Prayer Times backup"), "{e}");
        let e = parse(r#"{"format":"prayer-times-backup","version":2,"settings":{},"history":[]}"#)
            .unwrap_err();
        assert!(e.contains("newer version"), "{e}");
        assert!(parse("[1,2,3]").unwrap_err().contains("not a Prayer Times backup"));
    }

    #[test]
    fn truncated_json_is_rejected() {
        let (settings, history) = sample();
        let text = to_json(&settings, history, Utc::now());
        let cut = &text[..text.len() / 2];
        assert!(parse(cut).is_err());
    }

    #[test]
    fn imported_settings_keep_this_devices_location_when_the_backup_has_none() {
        let (current, _) = sample();
        let mut incoming = Settings::default();
        incoming.location = None;
        incoming.method = 1;
        let merged = settings_to_apply(&current, incoming);
        assert_eq!(merged.location.unwrap().city, "Dhaka");
        assert_eq!(merged.method, 1);
    }

    #[test]
    fn preview_counts_days_and_range() {
        let (settings, history) = sample();
        let file = parse(&to_json(&settings, history, Utc::now())).unwrap();
        let p = preview(&file);
        assert_eq!(p.entries, 3);
        assert_eq!(p.days, 2);
        assert_eq!(p.first_day.as_deref(), Some("2026-11-01"));
        assert_eq!(p.last_day.as_deref(), Some("2026-11-08"));
        assert_eq!(p.location.as_deref(), Some("Dhaka"));
        assert!(p.has_custom_tone);
    }
}
