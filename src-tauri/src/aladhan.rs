//! Aladhan API client (https://aladhan.com/prayer-times-api).
//!
//! The app fetches one month per request (`calendarByCity`) and caches it on
//! disk, which means one network call per month in the happy path and full
//! offline operation for already-fetched months.
//!
//! Endpoint quirks handled here:
//! - times may carry a suffix like " (+06)" or " (EET)" → stripped when parsing
//! - the *ByCity endpoints geocode a free-typed name and return placeholder
//!   coordinates, so the matched place can't be shown; a city picked from
//!   search (`geocode.rs`) is fetched by exact coordinates instead
//! - the envelope-level `meta` is null; the timezone lives in each day's
//!   `data.meta.timezone`

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use chrono::{Datelike, NaiveDate, NaiveTime};
use serde::{Deserialize, Serialize};

use crate::models::{AsrSchool, Coordinates, Location, Prayer};

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

/// Where to calculate times for.
#[derive(Debug, Clone, Copy)]
pub enum Place<'a> {
    /// A free-typed city name; Aladhan geocodes it (may guess wrong).
    City {
        city: &'a str,
        /// ISO alpha-2 country code
        country: &'a str,
    },
    /// Exact position of a city picked from search, in a known timezone.
    Coordinates {
        coordinates: Coordinates,
        timezone: &'a str,
    },
}

impl<'a> Place<'a> {
    pub fn of(location: &'a Location) -> Self {
        match location.coordinates {
            Some(coordinates) => Place::Coordinates {
                coordinates,
                timezone: &location.timezone,
            },
            None => Place::City {
                city: &location.city,
                country: &location.country_code,
            },
        }
    }
}

pub struct FetchParams<'a> {
    pub place: Place<'a>,
    pub method: u8,
    pub school: AsrSchool,
}

impl FetchParams<'_> {
    /// (endpoint for a month, endpoint for one day) — without the date part.
    fn endpoints(&self) -> (&'static str, &'static str) {
        match self.place {
            Place::City { .. } => ("calendarByCity", "timingsByCity"),
            Place::Coordinates { .. } => ("calendar", "timings"),
        }
    }

    fn query(&self) -> Vec<(&'static str, String)> {
        let mut q = match self.place {
            Place::City { city, country } => vec![
                ("city", city.to_string()),
                ("country", country.to_string()),
            ],
            Place::Coordinates {
                coordinates,
                timezone,
            } => vec![
                ("latitude", coordinates.latitude.to_string()),
                ("longitude", coordinates.longitude.to_string()),
                ("timezonestring", timezone.to_string()),
            ],
        };
        q.push(("method", self.method.to_string()));
        q.push(("school", self.school.to_string()));
        q
    }

    /// Identifies these parameters in the month cache file name, so a cache
    /// written for one location (or method/school) is never read for another.
    pub fn cache_id(&self) -> String {
        let place = match self.place {
            Place::City { city, country } => {
                format!("city:{}|{}", city.trim().to_lowercase(), country.to_lowercase())
            }
            Place::Coordinates {
                coordinates,
                timezone,
            } => format!(
                "coords:{:.4},{:.4}|{timezone}",
                coordinates.latitude, coordinates.longitude
            ),
        };
        format!("{}-{}-{:016x}", self.method, self.school, fnv1a(place.as_bytes()))
    }
}

/// 64-bit FNV-1a: a tiny hash that, unlike `DefaultHasher`, is stable
/// across Rust versions (cache file names must not change on upgrade).
fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, b| {
        (hash ^ u64::from(*b)).wrapping_mul(0x0000_0100_0000_01b3)
    })
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
    let url = format!("{BASE}/{}/{year}/{month}", params.endpoints().0);
    let resp: Envelope<Vec<RawDay>> = client
        .get(url.as_str())
        .query(&params.query())
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
        return Err(match params.place {
            Place::City { city, .. } => {
                format!("empty calendar for {month}/{year} (city '{city}' not recognized?)")
            }
            Place::Coordinates { .. } => format!("empty calendar for {month}/{year}"),
        });
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

pub fn fetch_day(
    client: &reqwest::blocking::Client,
    params: &FetchParams<'_>,
    date: NaiveDate,
) -> Result<(DayTimings, String), String> {
    let url = format!(
        "{BASE}/{}/{:02}-{:02}-{}",
        params.endpoints().1,
        date.day(),
        date.month(),
        date.year()
    );
    let resp: Envelope<RawDay> = client
        .get(url.as_str())
        .query(&params.query())
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
// Disk cache — one file per (location, method, school, year, month)
// ---------------------------------------------------------------------------

/// `cache_id` comes from [`FetchParams::cache_id`].
fn month_cache_path(dir: &Path, cache_id: &str, year: i32, month: u32) -> PathBuf {
    dir.join("cache")
        .join(format!("calendar-{cache_id}-{year}-{month:02}.json"))
}

pub fn load_month(dir: &Path, cache_id: &str, year: i32, month: u32) -> Option<MonthCache> {
    let path = month_cache_path(dir, cache_id, year, month);
    let text = std::fs::read_to_string(path).ok()?;
    let mut cache: MonthCache = serde_json::from_str(&text).ok()?;
    cache.from_cache = true;
    Some(cache)
}

pub fn save_month(dir: &Path, cache_id: &str, cache: &MonthCache) {
    let path = month_cache_path(dir, cache_id, cache.year, cache.month);
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

    fn location(city: &str, coordinates: Option<Coordinates>) -> Location {
        Location {
            city: city.to_string(),
            country: "India".to_string(),
            country_code: "IN".to_string(),
            timezone: "Asia/Kolkata".to_string(),
            region: None,
            coordinates,
        }
    }

    const KOLKATA: Coordinates = Coordinates {
        latitude: 22.56263,
        longitude: 88.36304,
    };

    fn params(place: Place<'_>) -> FetchParams<'_> {
        FetchParams {
            place,
            method: 1,
            school: AsrSchool::Hanafi,
        }
    }

    #[test]
    fn picked_city_is_fetched_by_coordinates() {
        let loc = location("Kolkata", Some(KOLKATA));
        let p = params(Place::of(&loc));
        assert_eq!(p.endpoints(), ("calendar", "timings"));
        let q = p.query();
        assert!(q.contains(&("latitude", "22.56263".to_string())));
        assert!(q.contains(&("longitude", "88.36304".to_string())));
        assert!(q.contains(&("timezonestring", "Asia/Kolkata".to_string())));
        assert!(q.contains(&("school", "1".to_string())));
        assert!(!q.iter().any(|(k, _)| *k == "city"));
    }

    #[test]
    fn typed_city_is_fetched_by_name() {
        let loc = location("Kolkata", None);
        let p = params(Place::of(&loc));
        assert_eq!(p.endpoints(), ("calendarByCity", "timingsByCity"));
        let q = p.query();
        assert!(q.contains(&("city", "Kolkata".to_string())));
        assert!(q.contains(&("country", "IN".to_string())));
    }

    #[test]
    fn cache_id_differs_per_location_and_settings() {
        let kolkata = location("Kolkata", Some(KOLKATA));
        let mumbai = location(
            "Mumbai",
            Some(Coordinates {
                latitude: 19.07283,
                longitude: 72.88261,
            }),
        );
        let typed = location("Kolkata", None);
        let id = |l: &Location| params(Place::of(l)).cache_id();
        assert_ne!(id(&kolkata), id(&mumbai));
        assert_ne!(id(&kolkata), id(&typed));
        // Same place → same id; case/whitespace of a typed name don't matter.
        assert_eq!(id(&typed), id(&location(" kolkata ", None)));
        let mut other_method = params(Place::of(&kolkata));
        other_method.method = 3;
        assert_ne!(other_method.cache_id(), id(&kolkata));
        // Stable format (file names must survive upgrades).
        assert!(id(&typed).starts_with("1-1-"));
        assert_eq!(fnv1a(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a(b"a"), 0xaf63_dc4c_8601_ec8c);
    }

    #[test]
    fn day_without_sunrise_is_rejected() {
        let mut raw: RawDay = serde_json::from_str(RAW_DAY).unwrap();
        raw.timings.sunrise = None;
        assert!(to_day(&raw).is_none());
    }
}
