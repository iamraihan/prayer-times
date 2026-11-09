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
    /// State / province of a city picked from search (display only).
    pub region: Option<String>,
    /// Exact position of a city picked from search; times are then fetched
    /// by coordinates. `None` for locations typed by name (older versions,
    /// or a town the search doesn't know) — those are looked up by name.
    pub coordinates: Option<Coordinates>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(TS), ts(export))]
pub struct Coordinates {
    pub latitude: f64,
    pub longitude: f64,
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

/// One prayer's alarm settings: its switch (the prayer chips) and when the
/// user actually prays it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
#[cfg_attr(test, derive(TS), ts(export))]
pub struct PrayerSettings {
    pub enabled: bool,
    /// "I pray N min after it starts" (e.g. the mosque's jamaat): moves the
    /// Get-Ready and Main Alarms to that moment. `None` = at the start.
    /// Older versions wrote this as `reminderMinutes` (a delayed Main Alarm,
    /// which rang at the same moment).
    #[serde(alias = "reminderMinutes")]
    pub pray_after_minutes: Option<u32>,
}

impl Default for PrayerSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            pray_after_minutes: None,
        }
    }
}

/// One entry per prayer — a struct rather than a map so every prayer is
/// always present (same JSON shape as a `{ "fajr": …, … }` map). A prayer
/// missing from an older settings file gets its default.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(TS), ts(export))]
pub struct PrayerSettingsMap {
    #[serde(default)]
    pub fajr: PrayerSettings,
    #[serde(default)]
    pub dhuhr: PrayerSettings,
    #[serde(default)]
    pub asr: PrayerSettings,
    #[serde(default)]
    pub maghrib: PrayerSettings,
    #[serde(default)]
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
    /// "Get ready": a heads-up `get_ready_minutes` before each prayer starts.
    pub get_ready_enabled: bool,
    pub get_ready_minutes: u32,
    /// "Prayer time": the Main Alarm at each prayer's start.
    pub start_alarm_enabled: bool,
    /// "Don't miss it" in the UI.
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
            get_ready_enabled: false,
            get_ready_minutes: 10,
            start_alarm_enabled: true,
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
    /// When the user prays this prayer: its start, or "N min after"
    /// (for a "Get ready" alarm's countdown).
    pub pray_at: String,
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
    /// Set when the saved location looks wrong (a typed city name that the
    /// API matched to a place in another country); the UI asks the user to
    /// pick their city again.
    pub location_warning: Option<String>,
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
    /// The search result the user picked; `None` = look the city up by name.
    pub picked: Option<CityMatchDto>,
}

/// One city search result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(TS), ts(export))]
pub struct CityMatchDto {
    pub name: String,
    /// State / province, e.g. "West Bengal".
    pub region: Option<String>,
    pub country_code: String,
    pub latitude: f64,
    pub longitude: f64,
    pub timezone: String,
    pub population: Option<u32>,
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
        assert_eq!(json["prayerSettings"]["maghrib"]["enabled"], true);
        assert_eq!(json["getReadyEnabled"], false);
        assert_eq!(json["startAlarmEnabled"], true);
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
        // Missing prayers default to on; the new alarms take their defaults.
        assert!(s.prayer_settings.get(Prayer::Fajr).enabled);
        assert!(!s.get_ready_enabled);
        assert!(s.start_alarm_enabled);
    }

    #[test]
    fn old_alarm_delay_becomes_the_users_prayer_time() {
        // Older versions stored a delayed Main Alarm as `reminderMinutes`;
        // it rang at the same moment "I pray N min after" now does.
        let json = r#"{"prayerSettings":{"asr":{"enabled":false,"reminderMinutes":30},
            "dhuhr":{"enabled":true,"reminderMinutes":45},
            "maghrib":{"enabled":true,"reminderMinutes":null},"isha":{}}}"#;
        let s: Settings = serde_json::from_str(json).unwrap();
        assert!(!s.prayer_settings.get(Prayer::Asr).enabled);
        assert_eq!(s.prayer_settings.get(Prayer::Asr).pray_after_minutes, Some(30));
        assert_eq!(s.prayer_settings.get(Prayer::Dhuhr).pray_after_minutes, Some(45));
        assert_eq!(s.prayer_settings.get(Prayer::Maghrib).pray_after_minutes, None);
        assert!(s.prayer_settings.get(Prayer::Isha).enabled);
        assert_eq!(s.prayer_settings.get(Prayer::Isha).pray_after_minutes, None);
        // Written back under the new name only.
        let out = serde_json::to_value(&s).unwrap();
        assert_eq!(out["prayerSettings"]["dhuhr"]["prayAfterMinutes"], 45);
        assert!(out["prayerSettings"]["dhuhr"].get("reminderMinutes").is_none());
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
    fn locations_saved_before_city_search_still_load() {
        let json = r#"{"location":{"city":"Dhaka","country":"Bangladesh",
            "countryCode":"BD","timezone":"Asia/Dhaka"}}"#;
        let s: Settings = serde_json::from_str(json).unwrap();
        let loc = s.location.unwrap();
        assert_eq!(loc.region, None);
        assert_eq!(loc.coordinates, None);
    }

    #[test]
    fn picked_location_round_trips() {
        let loc = Location {
            city: "Kolkata".into(),
            country: "India".into(),
            country_code: "IN".into(),
            timezone: "Asia/Kolkata".into(),
            region: Some("West Bengal".into()),
            coordinates: Some(Coordinates { latitude: 22.56263, longitude: 88.36304 }),
        };
        let json = serde_json::to_string(&loc).unwrap();
        assert!(json.contains(r#""coordinates":{"latitude":22.56263,"longitude":88.36304}"#));
        let back: Location = serde_json::from_str(&json).unwrap();
        assert_eq!(back.coordinates, loc.coordinates);
    }

    #[test]
    fn unexpected_enum_values_fall_back_instead_of_failing() {
        assert_eq!(AsrSchool::from(7), AsrSchool::Standard);
        assert_eq!(TimeFormat::from("weird".to_string()), TimeFormat::H12);
    }
}
