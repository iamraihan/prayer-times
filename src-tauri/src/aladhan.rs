//! Aladhan API client (https://aladhan.com/prayer-times-api).
//!
//! The app fetches one month per request (`calendarByCity`) and caches it on
//! disk, which means one network call per month in the happy path and full
//! offline operation for already-fetched months.
//!
//! Endpoint quirks handled here:
//! - times may carry a suffix like " (+06)" or " (EET)" → stripped when parsing
//! - `timingsByCity` returns placeholder coordinates, so lookups stay city-based
//! - the envelope-level `meta` is null; the timezone lives in each day's
//!   `data.meta.timezone`

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use chrono::{Datelike, NaiveDate, NaiveTime};
use serde::{Deserialize, Serialize};

use crate::models::{AsrSchool, Prayer};

pub const BASE: &str = "https://api.aladhan.com/v1";

// ---------------------------------------------------------------------------
// Raw API shapes
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[allow(dead_code)] // `code`/`status` are only deserialized for debugging
pub struct Envelope<D> {
    #[serde(default)]
    pub code: i32,
    #[serde(default)]
    pub status: String,
    pub data: D,
}

#[derive(Debug, Deserialize)]
pub struct RawMeta {
    pub timezone: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct RawTimings {
    #[serde(default)]
    pub fajr: Option<String>,
    #[serde(default)]
    pub sunrise: Option<String>,
    #[serde(default)]
    pub dhuhr: Option<String>,
    #[serde(default)]
    pub asr: Option<String>,
    #[serde(default)]
    pub maghrib: Option<String>,
    #[serde(default)]
    pub isha: Option<String>,
    #[serde(default)]
    pub midnight: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RawDay {
    pub timings: RawTimings,
    pub date: RawDate,
    /// The API serves timezone (and method info) per day; the envelope-level
    /// `meta` is null on current endpoints.
    #[serde(default)]
    pub meta: Option<RawMeta>,
}

#[derive(Debug, Deserialize)]
pub struct RawDate {
    pub readable: String,
    pub gregorian: RawGregorian,
    pub hijri: RawHijri,
}

#[derive(Debug, Deserialize)]
pub struct RawGregorian {
    /// "DD-MM-YYYY"
    pub date: String,
}

#[derive(Debug, Deserialize)]
pub struct RawHijri {
    pub day: String,
    pub month: RawHijriMonth,
    pub year: String,
}

#[derive(Debug, Deserialize)]
pub struct RawHijriMonth {
    pub en: String,
}

// ---------------------------------------------------------------------------
// Parsed model
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DayTimings {
    pub date: NaiveDate,
    /// "17 Oct 2026"
    pub readable: String,
    /// "6 Jumādá al-ūlá 1448"
    pub hijri: String,
    pub times: HashMap<Prayer, NaiveTime>,
    pub sunrise: NaiveTime,
    pub midnight: NaiveTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonthCache {
    pub year: i32,
    pub month: u32,
    pub timezone: String,
    #[serde(skip)]
    pub from_cache: bool,
    /// day-of-month -> timings
    pub days: BTreeMap<u32, DayTimings>,
}

pub struct FetchParams<'a> {
    pub city: &'a str,
    /// ISO alpha-2 country code
    pub country: &'a str,
    pub method: u8,
    pub school: AsrSchool,
}

/// "17:46 (+06)" / "17:46 (EET)" / "17:46" → NaiveTime
pub fn parse_hm(raw: &str) -> Option<NaiveTime> {
    let first = raw.trim().split_whitespace().next()?;
    NaiveTime::parse_from_str(first, "%H:%M").ok()
}

fn to_day(raw: &RawDay) -> Option<DayTimings> {
    let date = NaiveDate::parse_from_str(&raw.date.gregorian.date, "%d-%m-%Y").ok()?;
    let get = |s: &Option<String>| s.as_deref().and_then(parse_hm);

    let mut times = HashMap::new();
    let pairs = [
        (Prayer::Fajr, &raw.timings.fajr),
        (Prayer::Dhuhr, &raw.timings.dhuhr),
        (Prayer::Asr, &raw.timings.asr),
        (Prayer::Maghrib, &raw.timings.maghrib),
        (Prayer::Isha, &raw.timings.isha),
    ];
    for (prayer, slot) in pairs {
        if let Some(t) = get(slot) {
            times.insert(prayer, t);
        }
    }

    Some(DayTimings {
        date,
        readable: raw.date.readable.clone(),
        hijri: format!(
            "{} {} {}",
            raw.date.hijri.day, raw.date.hijri.month.en, raw.date.hijri.year
        ),
        times,
        sunrise: get(&raw.timings.sunrise)?,
        midnight: get(&raw.timings.midnight)?,
    })
}

// ---------------------------------------------------------------------------
// Network
// ---------------------------------------------------------------------------

pub fn fetch_month(
    client: &reqwest::blocking::Client,
    params: &FetchParams<'_>,
    year: i32,
    month: u32,
) -> Result<MonthCache, String> {
    let url = format!("{BASE}/calendarByCity/{year}/{month}");
    let resp: Envelope<Vec<RawDay>> = client
        .get(url.as_str())
        .query(&[
            ("city", params.city.to_string()),
            ("country", params.country.to_string()),
            ("method", params.method.to_string()),
            ("school", params.school.to_string()),
        ])
        .send()
        .map_err(|e| format!("network error: {e}"))?
        .error_for_status()
        .map_err(|e| format!("HTTP error: {e}"))?
        .json()
        .map_err(|e| format!("bad response: {e}"))?;

    let mut days = BTreeMap::new();
    for raw in &resp.data {
        if let Some(day) = to_day(raw) {
            days.insert(day.date.day(), day);
        }
    }
    if days.is_empty() {
        return Err(format!(
            "empty calendar for {month}/{year} (city '{}' not recognized?)",
            params.city
        ));
    }

    Ok(MonthCache {
        year,
        month,
        timezone: resp
            .data
            .first()
            .and_then(|raw| raw.meta.as_ref())
            .map(|m| m.timezone.clone())
            .unwrap_or_default(),
        from_cache: false,
        days,
    })
}

pub fn fetch_day_by_city(
    client: &reqwest::blocking::Client,
    params: &FetchParams<'_>,
    date: NaiveDate,
) -> Result<(DayTimings, String), String> {
    let url = format!(
        "{BASE}/timingsByCity/{:02}-{:02}-{}",
        date.day(),
        date.month(),
        date.year()
    );
    let resp: Envelope<RawDay> = client
        .get(url.as_str())
        .query(&[
            ("city", params.city.to_string()),
            ("country", params.country.to_string()),
            ("method", params.method.to_string()),
            ("school", params.school.to_string()),
        ])
        .send()
        .map_err(|e| format!("network error: {e}"))?
        .error_for_status()
        .map_err(|e| format!("HTTP error: {e}"))?
        .json()
        .map_err(|e| format!("bad response: {e}"))?;

    let timezone = resp
        .data
        .meta
        .as_ref()
        .map(|m| m.timezone.clone())
        .unwrap_or_default();
    let day = to_day(&resp.data).ok_or_else(|| "missing timings in response".to_string())?;
    Ok((day, timezone))
}

// ---------------------------------------------------------------------------
// Disk cache — one file per (method, school, year, month)
// ---------------------------------------------------------------------------

fn month_cache_path(
    dir: &Path,
    method: u8,
    school: AsrSchool,
    year: i32,
    month: u32,
) -> PathBuf {
    dir.join("cache").join(format!(
        "calendar-{method}-{school}-{year}-{month:02}.json"
    ))
}

pub fn load_month(
    dir: &Path,
    method: u8,
    school: AsrSchool,
    year: i32,
    month: u32,
) -> Option<MonthCache> {
    let path = month_cache_path(dir, method, school, year, month);
    let text = std::fs::read_to_string(path).ok()?;
    let mut cache: MonthCache = serde_json::from_str(&text).ok()?;
    cache.from_cache = true;
    Some(cache)
}

pub fn save_month(dir: &Path, cache: &MonthCache, method: u8, school: AsrSchool) {
    let path = month_cache_path(dir, method, school, cache.year, cache.month);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string(cache) {
        let _ = std::fs::write(path, json);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_hm_strips_zone_suffix() {
        let expected = NaiveTime::from_hms_opt(17, 46, 0);
        assert_eq!(parse_hm("17:46"), expected);
        assert_eq!(parse_hm("17:46 (+06)"), expected);
        assert_eq!(parse_hm(" 17:46 (EET) "), expected);
        assert_eq!(parse_hm(""), None);
        assert_eq!(parse_hm("5pm"), None);
    }

    const RAW_DAY: &str = r#"{
        "timings": {
            "Fajr": "04:58 (+06)", "Sunrise": "06:14 (+06)", "Dhuhr": "11:51 (+06)",
            "Asr": "15:38 (+06)", "Sunset": "17:22 (+06)", "Maghrib": "17:22 (+06)",
            "Isha": "18:39 (+06)", "Imsak": "04:48 (+06)", "Midnight": "23:51 (+06)"
        },
        "date": {
            "readable": "09 Nov 2026",
            "gregorian": { "date": "09-11-2026" },
            "hijri": { "day": "29", "month": { "en": "Jumādá al-ūlá" }, "year": "1448" }
        },
        "meta": { "timezone": "Asia/Dhaka" }
    }"#;

    #[test]
    fn parses_an_api_day() {
        let raw: RawDay = serde_json::from_str(RAW_DAY).unwrap();
        assert_eq!(raw.meta.as_ref().unwrap().timezone, "Asia/Dhaka");
        let day = to_day(&raw).unwrap();
        assert_eq!(day.date, NaiveDate::from_ymd_opt(2026, 11, 9).unwrap());
        assert_eq!(day.hijri, "29 Jumādá al-ūlá 1448");
        assert_eq!(day.times.len(), 5);
        assert_eq!(day.times[&Prayer::Isha], NaiveTime::from_hms_opt(18, 39, 0).unwrap());
        assert_eq!(day.midnight, NaiveTime::from_hms_opt(23, 51, 0).unwrap());
    }

    #[test]
    fn day_without_sunrise_is_rejected() {
        let mut raw: RawDay = serde_json::from_str(RAW_DAY).unwrap();
        raw.timings.sunrise = None;
        assert!(to_day(&raw).is_none());
    }
}
