//! Domain models and DTOs shared with the frontend (camelCase on the wire).
//!
//! Every type that crosses IPC derives `TS`; `cargo test` regenerates its
//! TypeScript twin in `src/lib/bindings/`. Never hand-write those types.

use std::collections::HashMap;
use std::fmt;

use serde::{Deserialize, Serialize};
#[cfg(test)]
use ts_rs::TS;

// ---------------------------------------------------------------------------
// Core domain
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(test, derive(TS), ts(export))]
pub enum Prayer {
    Fajr,
    Dhuhr,
    Asr,
    Maghrib,
    Isha,
}

impl Prayer {
    pub const ALL: [Prayer; 5] = [
        Prayer::Fajr,
        Prayer::Dhuhr,
        Prayer::Asr,
        Prayer::Maghrib,
        Prayer::Isha,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Prayer::Fajr => "fajr",
            Prayer::Dhuhr => "dhuhr",
            Prayer::Asr => "asr",
            Prayer::Maghrib => "maghrib",
            Prayer::Isha => "isha",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Prayer::Fajr => "Fajr",
            Prayer::Dhuhr => "Dhuhr",
            Prayer::Asr => "Asr",
            Prayer::Maghrib => "Maghrib",
            Prayer::Isha => "Isha",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(TS), ts(export))]
pub struct Location {
    pub city: String,
    /// Human-readable country name (display only).
    pub country: String,
    /// ISO 3166-1 alpha-2 code — what the Aladhan API expects.
    pub country_code: String,
    /// IANA timezone of the location (times are interpreted in this zone).
    pub timezone: String,
}

/// Juristic rule for when Asr starts. Numeric on the wire and in cache file
/// names (the Aladhan `school` parameter): 0 = Standard, 1 = Hanafi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(test, derive(TS), ts(export, type = "0 | 1"))]
pub enum AsrSchool {
    /// Shafi'i, Maliki, Hanbali.
    #[default]
    Standard,
    Hanafi,
}

impl From<u8> for AsrSchool {
    /// Lenient so an unexpected stored value can't fail the whole settings
    /// file (which would reset every setting to its default).
    fn from(value: u8) -> Self {
        if value == 1 {
            AsrSchool::Hanafi
        } else {
            AsrSchool::Standard
        }
    }
}

impl From<AsrSchool> for u8 {
    fn from(school: AsrSchool) -> u8 {
        match school {
            AsrSchool::Standard => 0,
            AsrSchool::Hanafi => 1,
        }
    }
}

impl Serialize for AsrSchool {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u8((*self).into())
    }
}

impl<'de> Deserialize<'de> for AsrSchool {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(u8::deserialize(deserializer)?.into())
    }
}

impl fmt::Display for AsrSchool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", u8::from(*self))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(test, derive(TS), ts(export, type = "\"12h\" | \"24h\""))]
pub enum TimeFormat {
    #[default]
    H12,
    H24,
}

impl TimeFormat {
    pub fn is_24h(self) -> bool {
        self == TimeFormat::H24
    }
}

impl From<String> for TimeFormat {
    /// Lenient for the same reason as [`AsrSchool`].
    fn from(value: String) -> Self {
        if value == "24h" {
            TimeFormat::H24
        } else {
            TimeFormat::H12
        }
    }
}

impl Serialize for TimeFormat {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(if self.is_24h() { "24h" } else { "12h" })
    }
}

impl<'de> Deserialize<'de> for TimeFormat {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(String::deserialize(deserializer)?.into())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(TS), ts(export))]
pub struct PrayerSettings {
    pub enabled: bool,
    /// Minutes to delay the prayer's main alarm after its start time.
    /// `None` = alarm exactly at the start time (default for Maghrib).
    pub reminder_minutes: Option<u32>,
}

impl PrayerSettings {
    pub fn default_for(prayer: Prayer) -> Self {
        Self {
            enabled: true,
            // Maghrib's window is short: no mid-window reminder by default.
            reminder_minutes: if prayer == Prayer::Maghrib {
                None
            } else {
                Some(30)
            },
        }
    }
}

/// One entry per prayer — a struct rather than a map so every prayer is
/// always present (same JSON shape as a `{ "fajr": …, … }` map). A prayer
/// missing from an older settings file gets its default.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(TS), ts(export))]
pub struct PrayerSettingsMap {
    #[serde(default = "PrayerSettingsMap::default_fajr")]
    pub fajr: PrayerSettings,
    #[serde(default = "PrayerSettingsMap::default_dhuhr")]
    pub dhuhr: PrayerSettings,
    #[serde(default = "PrayerSettingsMap::default_asr")]
    pub asr: PrayerSettings,
    #[serde(default = "PrayerSettingsMap::default_maghrib")]
    pub maghrib: PrayerSettings,
    #[serde(default = "PrayerSettingsMap::default_isha")]
    pub isha: PrayerSettings,
}

impl PrayerSettingsMap {
    pub fn get(&self, prayer: Prayer) -> &PrayerSettings {
        match prayer {
            Prayer::Fajr => &self.fajr,
            Prayer::Dhuhr => &self.dhuhr,
            Prayer::Asr => &self.asr,
            Prayer::Maghrib => &self.maghrib,
            Prayer::Isha => &self.isha,
        }
    }

    pub fn get_mut(&mut self, prayer: Prayer) -> &mut PrayerSettings {
        match prayer {
            Prayer::Fajr => &mut self.fajr,
            Prayer::Dhuhr => &mut self.dhuhr,
            Prayer::Asr => &mut self.asr,
            Prayer::Maghrib => &mut self.maghrib,
            Prayer::Isha => &mut self.isha,
        }
    }

    fn default_fajr() -> PrayerSettings {
        PrayerSettings::default_for(Prayer::Fajr)
    }
    fn default_dhuhr() -> PrayerSettings {
        PrayerSettings::default_for(Prayer::Dhuhr)
    }
    fn default_asr() -> PrayerSettings {
        PrayerSettings::default_for(Prayer::Asr)
    }
    fn default_maghrib() -> PrayerSettings {
        PrayerSettings::default_for(Prayer::Maghrib)
    }
    fn default_isha() -> PrayerSettings {
        PrayerSettings::default_for(Prayer::Isha)
    }
}

impl Default for PrayerSettingsMap {
    fn default() -> Self {
        Self {
            fajr: Self::default_fajr(),
            dhuhr: Self::default_dhuhr(),
            asr: Self::default_asr(),
            maghrib: Self::default_maghrib(),
            isha: Self::default_isha(),
        }
    }
}

/// `serde(default)`: a field missing from an older settings file takes its
/// default instead of failing the whole file (which would reset everything,
/// location included).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
#[cfg_attr(test, derive(TS), ts(export))]
pub struct Settings {
    pub location: Option<Location>,
    /// Aladhan calculation method id.
    pub method: u8,
    pub school: AsrSchool,
    pub time_format: TimeFormat,
    pub notifications_enabled: bool,
    pub sound_enabled: bool,
    pub sound_volume: f32,
    /// File name of a custom alarm tone inside the app config dir
    /// (e.g. "alarm-tone.mp3"); `None` = built-in chime.
    pub alarm_sound: Option<String>,
    pub popup_enabled: bool,
    pub pre_end_warning_enabled: bool,
    pub pre_end_warning_minutes: u32,
    pub prayer_settings: PrayerSettingsMap,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            location: None,
            method: 3, // Muslim World League
            school: AsrSchool::Standard,
            time_format: TimeFormat::H12,
            notifications_enabled: true,
            sound_enabled: true,
            sound_volume: 0.8,
            alarm_sound: None,
            popup_enabled: true,
            pre_end_warning_enabled: true,
            pre_end_warning_minutes: 10,
            prayer_settings: PrayerSettingsMap::default(),
        }
    }
}

// ---------------------------------------------------------------------------
// DTOs pushed to the frontend
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(TS), ts(export))]
pub struct PrayerTimeDto {
    pub prayer: Prayer,
    /// "HH:MM" location-local
    pub start: String,
    /// "HH:MM" location-local
    pub ends: String,
    /// Window start as an RFC3339 instant (None if unresolvable, e.g. DST gap).
    pub start_at: Option<String>,
    /// Window end as an RFC3339 instant; may be on the next calendar day.
    pub ends_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(TS), ts(export))]
pub struct DayTimingsDto {
    /// "YYYY-MM-DD" location-local
    pub date: String,
    pub gregorian: String,
    pub hijri: String,
    pub sunrise: String,
    pub midnight: String,
    pub timings: Vec<PrayerTimeDto>,
    pub from_cache: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(TS), ts(export))]
pub struct AlarmMessageDto {
    pub title: String,
    pub body: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(TS), ts(export))]
pub struct ActiveAlarmDto {
    pub id: String,
    pub prayer: Prayer,
    pub kind: crate::engine::AlarmKind,
    pub started_at: String,
    /// End of the prayer window (for the popup's live countdown).
    pub ends_at: String,
    pub message: AlarmMessageDto,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(TS), ts(export))]
pub struct NextPrayerDto {
    pub prayer: Prayer,
    pub start_at: String,
    pub ends_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(TS), ts(export))]
pub struct AppStateDto {
    pub has_location: bool,
    pub settings: Settings,
    pub today: Option<DayTimingsDto>,
    pub next: Option<NextPrayerDto>,
    pub done_today: HashMap<Prayer, bool>,
    /// Pending snoozed reminders: prayer -> due instant (RFC3339).
    pub snoozes: HashMap<Prayer, String>,
    pub active_alarm: Option<ActiveAlarmDto>,
    pub api_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(TS), ts(export))]
pub struct LocationPreviewDto {
    pub location: Location,
    pub day: DayTimingsDto,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(TS), ts(export))]
pub struct PreviewRequest {
    pub city: String,
    pub country: String,
    pub country_code: String,
    pub method: u8,
    pub school: AsrSchool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_wire_format_is_stable() {
        let mut s = Settings::default();
        s.school = AsrSchool::Hanafi;
        s.time_format = TimeFormat::H24;
        let json = serde_json::to_value(&s).unwrap();
        assert_eq!(json["school"], 1);
        assert_eq!(json["timeFormat"], "24h");
        assert_eq!(json["prayerSettings"]["maghrib"]["reminderMinutes"], serde_json::Value::Null);
    }

    #[test]
    fn old_settings_files_still_load() {
        let json = r#"{"location":null,"method":3,"school":1,"timeFormat":"24h",
            "notificationsEnabled":true,"soundEnabled":true,"soundVolume":0.5,
            "alarmSound":null,"popupEnabled":true,"preEndWarningEnabled":true,
            "preEndWarningMinutes":10,"prayerSettings":{}}"#;
        let s: Settings = serde_json::from_str(json).unwrap();
        assert_eq!(s.school, AsrSchool::Hanafi);
        assert!(s.time_format.is_24h());
        // Missing prayers get their own defaults (Maghrib: no delay).
        assert_eq!(s.prayer_settings.get(Prayer::Fajr).reminder_minutes, Some(30));
        assert_eq!(s.prayer_settings.get(Prayer::Maghrib).reminder_minutes, None);
    }

    #[test]
    fn settings_missing_fields_keep_the_rest() {
        let json = r#"{"location":{"city":"Dhaka","country":"Bangladesh",
            "countryCode":"BD","timezone":"Asia/Dhaka"},"method":1}"#;
        let s: Settings = serde_json::from_str(json).unwrap();
        assert_eq!(s.location.unwrap().city, "Dhaka");
        assert_eq!(s.method, 1);
        assert_eq!(s.pre_end_warning_minutes, 10);
    }

    #[test]
    fn unexpected_enum_values_fall_back_instead_of_failing() {
        assert_eq!(AsrSchool::from(7), AsrSchool::Standard);
        assert_eq!(TimeFormat::from("weird".to_string()), TimeFormat::H12);
    }
}
