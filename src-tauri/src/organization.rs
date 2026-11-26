//! Following an Organization (a masjid or group) that publishes its jamaat
//! times as an Organization Feed: a small JSON document at a URL the user
//! pastes in. See `docs/plans/organization-feed.md`.


use std::fmt;

use chrono::{DateTime, Duration, FixedOffset, NaiveTime, Utc};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::models::{
    OrganizationLink, OrganizationPrayerDto, OrganizationPreviewDto, OrganizationStatusDto, Prayer,
};

/// A parsed, validated Organization Feed.
#[derive(Debug, Clone, PartialEq)]
pub struct OrganizationFeed {
    pub name: String,
    /// IANA timezone; always equal to the Location's (checked by `parse_feed`).
    pub timezone: String,
    pub updated_at: Option<DateTime<FixedOffset>>,
    pub prayers: OrganizationPrayers,
}

/// The Organization's congregation time for each prayer it covers, in
/// Location Time. `None` = not covered: the user's own setting applies.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct OrganizationPrayers {
    pub fajr: Option<NaiveTime>,
    pub dhuhr: Option<NaiveTime>,
    pub asr: Option<NaiveTime>,
    pub maghrib: Option<NaiveTime>,
    pub isha: Option<NaiveTime>,
}

impl OrganizationPrayers {
    pub fn get(&self, prayer: Prayer) -> Option<NaiveTime> {
        match prayer {
            Prayer::Fajr => self.fajr,
            Prayer::Dhuhr => self.dhuhr,
            Prayer::Asr => self.asr,
            Prayer::Maghrib => self.maghrib,
            Prayer::Isha => self.isha,
        }
    }

    fn slot(&mut self, prayer: Prayer) -> &mut Option<NaiveTime> {
        match prayer {
            Prayer::Fajr => &mut self.fajr,
            Prayer::Dhuhr => &mut self.dhuhr,
            Prayer::Asr => &mut self.asr,
            Prayer::Maghrib => &mut self.maghrib,
            Prayer::Isha => &mut self.isha,
        }
    }
}

/// Why a feed document was rejected. `Display` is the message shown to the
/// user (in the preview, or as a warning after a refresh).
#[derive(Debug, Clone, PartialEq)]
pub enum FeedError {
    /// Not JSON, or a required field (`name`, `timezone`, `prayers`) is
    /// missing or has the wrong type.
    Malformed(String),
    UnknownPrayer { key: String, hint: Option<Prayer> },
    BadTime { prayer: Prayer, value: String },
    BadUpdatedAt(String),
    TimezoneMismatch { feed: String, location: String },
    NoPrayers,
}

impl fmt::Display for FeedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FeedError::Malformed(detail) => write!(f, "The feed isn't valid: {detail}"),
            FeedError::UnknownPrayer { key, hint: Some(p) } => {
                write!(f, "Unknown prayer \"{key}\" in the feed. Did you mean \"{}\"?", p.id())
            }
            FeedError::UnknownPrayer { key, hint: None } => write!(
                f,
                "Unknown prayer \"{key}\" in the feed. Use fajr, dhuhr, asr, maghrib or isha."
            ),
            FeedError::BadTime { prayer, value } => write!(
                f,
                "{}'s time \"{value}\" isn't a 24-hour HH:MM time.",
                prayer.title()
            ),
            FeedError::BadUpdatedAt(value) => {
                write!(f, "\"updatedAt\" (\"{value}\") isn't an RFC 3339 date and time.")
            }
            FeedError::TimezoneMismatch { feed, location } => write!(
                f,
                "The feed's times are for {feed}, but your location is in {location}."
            ),
            FeedError::NoPrayers => write!(f, "The feed doesn't list any prayer times."),
        }
    }
}

/// Parses and validates an Organization Feed for a Location in `location_tz`.
/// The whole document is rejected on any error: a partly applied feed could
/// ring at times the Organization never announced.
pub fn parse_feed(json: &str, location_tz: &str) -> Result<OrganizationFeed, FeedError> {
    let feed = parse_document(json)?;
    if feed.timezone != location_tz {
        return Err(FeedError::TimezoneMismatch {
            feed: feed.timezone,
            location: location_tz.to_string(),
        });
    }
    Ok(feed)
}

/// `parse_feed` without the Location check (for a stored document).
fn parse_document(json: &str) -> Result<OrganizationFeed, FeedError> {
    let raw: RawFeed =
        serde_json::from_str(json).map_err(|e| FeedError::Malformed(e.to_string()))?;

    let name = raw.name.trim();
    if name.is_empty() {
        return Err(FeedError::Malformed("\"name\" is empty".to_string()));
    }
    let updated_at = raw
        .updated_at
        .map(|v| DateTime::parse_from_rfc3339(&v).map_err(|_| FeedError::BadUpdatedAt(v)))
        .transpose()?;

    let mut prayers = OrganizationPrayers::default();
    for (key, value) in raw.prayers {
        let Some(prayer) = Prayer::from_id(&key) else {
            let hint = spelling_hint(&key);
            return Err(FeedError::UnknownPrayer { key, hint });
        };
        if value.is_null() {
            continue; // listed but not covered
        }
        let time = value
            .as_str()
            .and_then(parse_hh_mm)
            .ok_or_else(|| FeedError::BadTime {
                prayer,
                value: value.as_str().map_or_else(|| value.to_string(), str::to_string),
            })?;
        *prayers.slot(prayer) = Some(time);
    }
    if prayers == OrganizationPrayers::default() {
        return Err(FeedError::NoPrayers);
    }

    Ok(OrganizationFeed {
        name: name.to_string(),
        timezone: raw.timezone,
        updated_at,
        prayers,
    })
}

/// The wire shape. Unknown top-level fields are ignored so the format can
/// grow; unknown keys under `prayers` are errors (most likely a typo), and a
/// prayer set to `null` is not covered.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawFeed {
    name: String,
    timezone: String,
    #[serde(default)]
    updated_at: Option<String>,
    prayers: Map<String, Value>,
}

/// Exactly `HH:MM`, 24-hour, two digits each.
fn parse_hh_mm(raw: &str) -> Option<NaiveTime> {
    let b = raw.as_bytes();
    if b.len() != 5 || b[2] != b':' || ![0, 1, 3, 4].iter().all(|&i| b[i].is_ascii_digit()) {
        return None;
    }
    let digits = |i: usize| u32::from(b[i] - b'0') * 10 + u32::from(b[i + 1] - b'0');
    NaiveTime::from_hms_opt(digits(0), digits(3), 0)
}

/// The prayer an unknown key most likely meant: a different capitalisation,
/// a common transliteration, or a spelling one or two letters off.
fn spelling_hint(key: &str) -> Option<Prayer> {
    let key = key.trim().to_lowercase();
    if let Some(p) = Prayer::from_id(&key) {
        return Some(p);
    }
    const ALIASES: [(&str, Prayer); 6] = [
        ("johor", Prayer::Dhuhr),
        ("zohr", Prayer::Dhuhr),
        ("zuhur", Prayer::Dhuhr),
        ("dhuhur", Prayer::Dhuhr),
        ("asor", Prayer::Asr),
        ("eesha", Prayer::Isha),
    ];
    if let Some((_, p)) = ALIASES.iter().find(|(alias, _)| *alias == key) {
        return Some(*p);
    }
    Prayer::ALL
        .into_iter()
        .map(|p| (edit_distance(&key, p.id()), p))
        .filter(|(d, _)| *d <= 2)
        .min_by_key(|(d, _)| *d)
        .map(|(_, p)| p)
}

/// Levenshtein distance (the keys are a few ASCII letters).
fn edit_distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut cur = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != *cb);
            cur.push((prev[j] + cost).min(prev[j + 1] + 1).min(cur[j] + 1));
        }
        prev = cur;
    }
    prev[b.len()]
}

// ---------------------------------------------------------------------------
// Fetching
// ---------------------------------------------------------------------------

/// Why a feed couldn't be downloaded.
#[derive(Debug, Clone, PartialEq)]
pub enum FetchError {
    /// Not an `https://` URL.
    InsecureUrl,
    /// Offline, DNS, timeout, TLS, or a 5xx/other unexpected status.
    Unreachable(String),
    /// HTTP 401 or 403.
    AccessKeyRejected,
    /// HTTP 404.
    NotFound,
    /// The body is bigger than `MAX_FEED_BYTES`.
    TooLarge,
}

impl fmt::Display for FetchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FetchError::InsecureUrl => write!(f, "The feed link must start with https://."),
            FetchError::Unreachable(detail) => write!(f, "Couldn't reach the feed ({detail})."),
            FetchError::AccessKeyRejected => write!(f, "The feed rejected the access key."),
            FetchError::NotFound => write!(f, "No feed was found at that link."),
            FetchError::TooLarge => write!(f, "The feed is too large to be a prayer-times feed."),
        }
    }
}

/// A prayer-times feed is a few hundred bytes; anything this big is
/// something else (a web page, a mistake).
pub const MAX_FEED_BYTES: u64 = 1024 * 1024;

/// Downloads the feed at `link` (https only), sending the access key as a
/// Bearer token when there is one. Returns the body; `parse_feed` checks it.
pub fn fetch(client: &reqwest::blocking::Client, link: &OrganizationLink) -> Result<String, FetchError> {
    fetch_url(client, &link.feed_url, link.access_key.as_deref(), false)
}

/// `allow_local_http` lets tests use a plain-http server on 127.0.0.1.
fn fetch_url(
    client: &reqwest::blocking::Client,
    url: &str,
    access_key: Option<&str>,
    allow_local_http: bool,
) -> Result<String, FetchError> {
    let allowed = |u: &reqwest::Url| {
        u.scheme() == "https"
            || (allow_local_http && u.scheme() == "http" && u.host_str() == Some("127.0.0.1"))
    };
    let parsed = reqwest::Url::parse(url.trim()).map_err(|_| FetchError::InsecureUrl)?;
    if !allowed(&parsed) {
        return Err(FetchError::InsecureUrl);
    }

    let mut request = client.get(parsed).header(reqwest::header::ACCEPT, "application/json");
    if let Some(key) = access_key.map(str::trim).filter(|k| !k.is_empty()) {
        request = request.bearer_auth(key);
    }
    let response = request
        .send()
        .map_err(|e| FetchError::Unreachable(e.without_url().to_string()))?;
    // A redirect must not downgrade to plain http.
    if !allowed(response.url()) {
        return Err(FetchError::InsecureUrl);
    }
    let status = response.status();
    match status.as_u16() {
        401 | 403 => return Err(FetchError::AccessKeyRejected),
        404 | 410 => return Err(FetchError::NotFound),
        _ if !status.is_success() => return Err(FetchError::Unreachable(format!("HTTP {status}"))),
        _ => {}
    }
    if response.content_length().is_some_and(|n| n > MAX_FEED_BYTES) {
        return Err(FetchError::TooLarge);
    }
    let mut body = String::new();
    use std::io::Read as _;
    response
        .take(MAX_FEED_BYTES + 1)
        .read_to_string(&mut body)
        .map_err(|e| FetchError::Unreachable(e.to_string()))?;
    if body.len() as u64 > MAX_FEED_BYTES {
        return Err(FetchError::TooLarge);
    }
    Ok(body)
}

// ---------------------------------------------------------------------------
// Refresh and failure policy
// ---------------------------------------------------------------------------

/// The daily refresh happens at this Location Time (or at the first tick
/// after it, when the app wasn't running then).
pub const DAILY_REFRESH_AT: (u32, u32) = (13, 0);
/// After a failed fetch, wait this long before trying again.
pub const RETRY_AFTER_MINUTES: i64 = 15;
/// Warn about an unreachable feed only after it has failed for this long.
pub const WARN_UNREACHABLE_AFTER_HOURS: i64 = 24;

/// What the app knows about the followed Organization's feed: the last good
/// document and how fetching it has gone. Saved as `cache/organization.json`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct OrganizationState {
    /// The feed this state belongs to.
    feed_url: String,
    /// The last good document, exactly as downloaded.
    body: Option<String>,
    /// `body`, parsed (rebuilt by `from_json`).
    #[serde(skip)]
    feed: Option<OrganizationFeed>,
    last_success: Option<DateTime<Utc>>,
    /// The latest failed fetch, if it came after `last_success`.
    last_failure: Option<Failure>,
    /// The first failure since `last_success`.
    failing_since: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Failure {
    at: DateTime<Utc>,
    message: String,
    /// Offline / server down: usually passes, so warn only after a day.
    /// Everything else (bad document, rejected key, 404) needs someone to
    /// act, so it shows at once.
    transient: bool,
}

impl OrganizationState {
    /// A fresh state for the feed at `feed_url` (nothing fetched yet).
    pub fn for_feed(feed_url: &str) -> Self {
        Self {
            feed_url: feed_url.to_string(),
            ..Self::default()
        }
    }

    pub fn feed_url(&self) -> &str {
        &self.feed_url
    }

    /// Whether to fetch the feed now (`now` in Location Time): never
    /// fetched, or not fetched since the latest `DAILY_REFRESH_AT` — but not
    /// within `RETRY_AFTER_MINUTES` of a failed attempt.
    pub fn refresh_due(&self, now: DateTime<Tz>) -> bool {
        if let Some(f) = &self.last_failure {
            if now.with_timezone(&Utc) - f.at < Duration::minutes(RETRY_AFTER_MINUTES) {
                return false;
            }
        }
        let Some(last) = self.last_success else {
            return true;
        };
        let local_now = now.naive_local();
        let (h, m) = DAILY_REFRESH_AT;
        let Some(today_at) = local_now.date().and_hms_opt(h, m, 0) else {
            return true;
        };
        let boundary = if local_now >= today_at {
            today_at
        } else {
            today_at - Duration::days(1)
        };
        last.with_timezone(&now.timezone()).naive_local() < boundary
    }

    /// Records a fetch: `Ok(body)` is a downloaded document. A document
    /// that fails `parse_feed` is a failure: the last good one stays.
    pub fn record(&mut self, result: Result<String, FetchError>, location_tz: &str, now: DateTime<Utc>) {
        let failure = match result {
            Ok(body) => match parse_feed(&body, location_tz) {
                Ok(feed) => {
                    self.body = Some(body);
                    self.feed = Some(feed);
                    self.last_success = Some(now);
                    self.last_failure = None;
                    self.failing_since = None;
                    return;
                }
                Err(e) => Failure {
                    at: now,
                    message: e.to_string(),
                    transient: false,
                },
            },
            Err(e) => Failure {
                at: now,
                message: e.to_string(),
                transient: matches!(e, FetchError::Unreachable(_)),
            },
        };
        self.failing_since.get_or_insert(now);
        self.last_failure = Some(failure);
    }

    /// The last good feed's times, if they are for `location_tz`.
    pub fn prayers(&self, location_tz: &str) -> Option<&OrganizationPrayers> {
        self.feed
            .as_ref()
            .filter(|f| f.timezone == location_tz)
            .map(|f| &f.prayers)
    }

    /// The last good feed, whatever its timezone.
    pub fn feed(&self) -> Option<&OrganizationFeed> {
        self.feed.as_ref()
    }

    /// The warning to show, if any (`now` in Location Time).
    pub fn warning(&self, now: DateTime<Tz>) -> Option<String> {
        let location_tz = now.timezone().name();
        if let Some(feed) = self.feed.as_ref().filter(|f| f.timezone != location_tz) {
            return Some(format!(
                "{}'s times are for {}, but your location is in {location_tz}, so your own settings apply.",
                feed.name, feed.timezone
            ));
        }
        let failure = self.last_failure.as_ref()?;
        let (Some(feed), Some(last)) = (&self.feed, self.last_success) else {
            return Some(failure.message.clone());
        };
        if !failure.transient {
            return Some(format!(
                "Couldn't update {}: {} Using its times from {}.",
                feed.name,
                failure.message,
                short_date(last, now.timezone())
            ));
        }
        let since = self.failing_since.unwrap_or(failure.at);
        (now.with_timezone(&Utc) - since >= Duration::hours(WARN_UNREACHABLE_AFTER_HOURS)).then(|| {
            format!(
                "Couldn't reach {} since {}. Using its times from then.",
                feed.name,
                short_date(last, now.timezone())
            )
        })
    }

    pub fn last_success(&self) -> Option<DateTime<Utc>> {
        self.last_success
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// `None` when the file is unreadable: start over (the feed is fetched
    /// again).
    pub fn from_json(text: &str) -> Option<Self> {
        let mut state: Self = serde_json::from_str(text).ok()?;
        state.feed = match &state.body {
            Some(body) => Some(parse_document(body).ok()?),
            None => None,
        };
        Some(state)
    }
}

fn cache_path(dir: &std::path::Path) -> std::path::PathBuf {
    dir.join("cache").join("organization.json")
}

/// The saved state for `link`, or a fresh one (a file for another feed, or
/// an unreadable one, is ignored: the feed is fetched again).
pub fn load(dir: &std::path::Path, link: &OrganizationLink) -> OrganizationState {
    std::fs::read_to_string(cache_path(dir))
        .ok()
        .and_then(|text| OrganizationState::from_json(&text))
        .filter(|st| st.feed_url() == link.feed_url)
        .unwrap_or_else(|| OrganizationState::for_feed(&link.feed_url))
}

pub fn save(dir: &std::path::Path, state: &OrganizationState) {
    let path = cache_path(dir);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(path, state.to_json());
}

pub fn remove(dir: &std::path::Path) {
    let _ = std::fs::remove_file(cache_path(dir));
}

// ---------------------------------------------------------------------------
// What the UI shows
// ---------------------------------------------------------------------------

fn host_of(url: &str) -> String {
    reqwest::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_string))
        .unwrap_or_default()
}

fn prayer_dtos(prayers: &OrganizationPrayers) -> Vec<OrganizationPrayerDto> {
    Prayer::ALL
        .into_iter()
        .filter_map(|p| {
            prayers.get(p).map(|t| OrganizationPrayerDto {
                prayer: p,
                time: t.format("%H:%M").to_string(),
            })
        })
        .collect()
}

pub fn preview_dto(feed_url: &str, feed: &OrganizationFeed) -> OrganizationPreviewDto {
    OrganizationPreviewDto {
        name: feed.name.clone(),
        host: host_of(feed_url),
        timezone: feed.timezone.clone(),
        updated_at: feed.updated_at.map(|t| t.to_rfc3339()),
        prayers: prayer_dtos(&feed.prayers),
    }
}

/// The followed Organization's status (`now` in Location Time).
pub fn status_dto(link: &OrganizationLink, state: &OrganizationState, now: DateTime<Tz>) -> OrganizationStatusDto {
    let tz = now.timezone();
    OrganizationStatusDto {
        feed_url: link.feed_url.clone(),
        host: host_of(&link.feed_url),
        name: state.feed().map(|f| f.name.clone()),
        prayers: state.prayers(tz.name()).map(prayer_dtos).unwrap_or_default(),
        updated_at: state.feed().and_then(|f| f.updated_at).map(|t| t.to_rfc3339()),
        last_fetched_at: state.last_success().map(|t| t.with_timezone(&tz).to_rfc3339()),
        warning: state.warning(now),
    }
}

/// "25 Nov" in `tz`.
fn short_date(at: DateTime<Utc>, tz: Tz) -> String {
    at.with_timezone(&tz).format("%-d %b").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    const DHAKA: &str = "Asia/Dhaka";

    fn hm(h: u32, m: u32) -> NaiveTime {
        NaiveTime::from_hms_opt(h, m, 0).unwrap()
    }

    fn feed_with_prayers(prayers: &str) -> String {
        format!(r#"{{"name":"Baitul Aman Masjid","timezone":"Asia/Dhaka","prayers":{prayers}}}"#)
    }

    #[test]
    fn a_feed_covering_three_prayers_leaves_the_other_two_uncovered() {
        let json = r#"{
            "name": "Baitul Aman Masjid",
            "timezone": "Asia/Dhaka",
            "updatedAt": "2026-11-26T10:00:00+06:00",
            "prayers": { "fajr": "05:30", "dhuhr": "13:15", "asr": "16:00" }
        }"#;
        let feed = parse_feed(json, DHAKA).unwrap();
        assert_eq!(feed.name, "Baitul Aman Masjid");
        assert_eq!(feed.timezone, DHAKA);
        assert_eq!(
            feed.updated_at,
            Some(DateTime::parse_from_rfc3339("2026-11-26T10:00:00+06:00").unwrap())
        );
        assert_eq!(
            feed.prayers,
            OrganizationPrayers {
                fajr: Some(hm(5, 30)),
                dhuhr: Some(hm(13, 15)),
                asr: Some(hm(16, 0)),
                maghrib: None,
                isha: None,
            }
        );
    }

    #[test]
    fn updated_at_is_optional() {
        let feed = parse_feed(&feed_with_prayers(r#"{"isha":"20:00"}"#), DHAKA).unwrap();
        assert_eq!(feed.updated_at, None);
        assert_eq!(feed.prayers.get(Prayer::Isha), Some(hm(20, 0)));
    }

    #[test]
    fn a_misspelt_prayer_is_rejected_with_the_right_spelling() {
        for (key, meant) in [
            ("zuhr", Prayer::Dhuhr),
            ("johor", Prayer::Dhuhr),
            ("fazr", Prayer::Fajr),
            ("magrib", Prayer::Maghrib),
            ("esha", Prayer::Isha),
            ("asar", Prayer::Asr),
            ("Fajr", Prayer::Fajr),
        ] {
            let json = feed_with_prayers(&format!(r#"{{"{key}":"05:30"}}"#));
            assert_eq!(
                parse_feed(&json, DHAKA),
                Err(FeedError::UnknownPrayer { key: key.to_string(), hint: Some(meant) }),
                "{key}"
            );
        }
    }

    #[test]
    fn an_unrelated_key_under_prayers_is_rejected_without_a_hint() {
        let json = feed_with_prayers(r#"{"fajr":"05:30","jumuah":"13:30"}"#);
        assert_eq!(
            parse_feed(&json, DHAKA),
            Err(FeedError::UnknownPrayer { key: "jumuah".to_string(), hint: None })
        );
    }

    #[test]
    fn a_feed_for_another_timezone_is_rejected() {
        let json = r#"{"name":"East Town Mosque","timezone":"Europe/London",
            "prayers":{"fajr":"06:30"}}"#;
        assert_eq!(
            parse_feed(json, DHAKA),
            Err(FeedError::TimezoneMismatch {
                feed: "Europe/London".to_string(),
                location: DHAKA.to_string(),
            })
        );
    }

    #[test]
    fn times_must_be_24_hour_hh_mm() {
        for bad in ["5:30", "5:3", "25:00", "13:60", "1:15 PM", "13:15:00", ""] {
            let json = feed_with_prayers(&format!(r#"{{"dhuhr":"{bad}"}}"#));
            assert_eq!(
                parse_feed(&json, DHAKA),
                Err(FeedError::BadTime { prayer: Prayer::Dhuhr, value: bad.to_string() }),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn a_time_that_is_not_a_string_is_rejected() {
        let json = feed_with_prayers(r#"{"asr":1600}"#);
        assert_eq!(
            parse_feed(&json, DHAKA),
            Err(FeedError::BadTime { prayer: Prayer::Asr, value: "1600".to_string() })
        );
    }

    #[test]
    fn null_means_not_covered() {
        // Dashboards often send all five keys and leave some empty.
        let json = feed_with_prayers(
            r#"{"fajr":"05:30","dhuhr":"13:15","asr":"16:00","maghrib":null,"isha":null}"#,
        );
        let feed = parse_feed(&json, DHAKA).unwrap();
        assert_eq!(feed.prayers.get(Prayer::Asr), Some(hm(16, 0)));
        assert_eq!(feed.prayers.get(Prayer::Maghrib), None);
        assert_eq!(feed.prayers.get(Prayer::Isha), None);
        let all_null = feed_with_prayers(
            r#"{"fajr":null,"dhuhr":null,"asr":null,"maghrib":null,"isha":null}"#,
        );
        assert_eq!(parse_feed(&all_null, DHAKA), Err(FeedError::NoPrayers));
    }

    #[test]
    fn a_feed_without_prayers_is_rejected() {
        assert_eq!(parse_feed(&feed_with_prayers("{}"), DHAKA), Err(FeedError::NoPrayers));
    }

    #[test]
    fn a_bad_updated_at_is_rejected() {
        let json = r#"{"name":"M","timezone":"Asia/Dhaka","updatedAt":"yesterday",
            "prayers":{"fajr":"05:30"}}"#;
        assert_eq!(
            parse_feed(json, DHAKA),
            Err(FeedError::BadUpdatedAt("yesterday".to_string()))
        );
    }

    #[test]
    fn missing_name_timezone_or_prayers_is_malformed() {
        for json in [
            r#"{"timezone":"Asia/Dhaka","prayers":{"fajr":"05:30"}}"#,
            r#"{"name":"M","prayers":{"fajr":"05:30"}}"#,
            r#"{"name":"M","timezone":"Asia/Dhaka"}"#,
            r#"{"name":"  ","timezone":"Asia/Dhaka","prayers":{"fajr":"05:30"}}"#,
            r#"<html>Not found</html>"#,
        ] {
            assert!(
                matches!(parse_feed(json, DHAKA), Err(FeedError::Malformed(_))),
                "{json}"
            );
        }
    }

    #[test]
    fn unknown_top_level_fields_are_ignored() {
        // The format can grow (e.g. Jumu'ah, a logo) without breaking this
        // version.
        let json = r#"{"name":"M","timezone":"Asia/Dhaka","logo":"https://x/logo.png",
            "jumuah":"13:30","prayers":{"maghrib":"17:20"}}"#;
        let feed = parse_feed(json, DHAKA).unwrap();
        assert_eq!(feed.prayers.get(Prayer::Maghrib), Some(hm(17, 20)));
    }

    #[test]
    fn error_messages_name_the_fix() {
        let err = FeedError::UnknownPrayer { key: "zuhr".into(), hint: Some(Prayer::Dhuhr) };
        assert_eq!(err.to_string(), "Unknown prayer \"zuhr\" in the feed. Did you mean \"dhuhr\"?");
    }

    // -- OrganizationState ------------------------------------------------------

    const DHAKA_TZ: Tz = chrono_tz::Asia::Dhaka;

    fn dhaka(ymd_hm: &str) -> DateTime<Tz> {
        use chrono::TimeZone;
        let naive = chrono::NaiveDateTime::parse_from_str(ymd_hm, "%Y-%m-%d %H:%M").unwrap();
        DHAKA_TZ.from_local_datetime(&naive).unwrap()
    }

    fn utc(ymd_hm: &str) -> DateTime<Utc> {
        dhaka(ymd_hm).with_timezone(&Utc)
    }

    const GOOD: &str = r#"{"name":"Baitul Aman Masjid","timezone":"Asia/Dhaka",
        "prayers":{"fajr":"05:30","asr":"16:00"}}"#;
    const NEWER: &str = r#"{"name":"Baitul Aman Masjid","timezone":"Asia/Dhaka",
        "prayers":{"fajr":"05:45","asr":"15:50"}}"#;

    fn fetched_at(when: &str) -> OrganizationState {
        let mut st = OrganizationState::default();
        st.record(Ok(GOOD.to_string()), DHAKA, utc(when));
        st
    }

    #[test]
    fn a_feed_never_fetched_is_due() {
        assert!(OrganizationState::default().refresh_due(dhaka("2026-11-26 09:00")));
    }

    #[test]
    fn the_feed_is_refetched_once_a_day_at_13_00() {
        let st = fetched_at("2026-11-26 10:00");
        assert!(!st.refresh_due(dhaka("2026-11-26 12:59")));
        assert!(st.refresh_due(dhaka("2026-11-26 13:00")));
        let st = fetched_at("2026-11-26 13:05");
        assert!(!st.refresh_due(dhaka("2026-11-26 23:00")));
        assert!(!st.refresh_due(dhaka("2026-11-27 12:59")));
        assert!(st.refresh_due(dhaka("2026-11-27 13:00")));
    }

    #[test]
    fn a_missed_13_00_is_caught_up_at_the_next_launch() {
        // Fetched Monday 14:00; the app was closed until Wednesday 09:00.
        let st = fetched_at("2026-11-23 14:00");
        assert!(st.refresh_due(dhaka("2026-11-25 09:00")));
    }

    #[test]
    fn a_failed_fetch_is_retried_after_15_minutes() {
        let mut st = fetched_at("2026-11-26 10:00");
        st.record(Err(FetchError::Unreachable("offline".into())), DHAKA, utc("2026-11-26 13:00"));
        assert!(!st.refresh_due(dhaka("2026-11-26 13:14")));
        assert!(st.refresh_due(dhaka("2026-11-26 13:15")));
        // A never-fetched feed backs off too.
        let mut st = OrganizationState::default();
        st.record(Err(FetchError::Unreachable("offline".into())), DHAKA, utc("2026-11-26 09:00"));
        assert!(!st.refresh_due(dhaka("2026-11-26 09:10")));
        assert!(st.refresh_due(dhaka("2026-11-26 09:15")));
    }

    #[test]
    fn a_good_document_replaces_the_times_and_clears_the_warning() {
        let mut st = fetched_at("2026-11-25 13:00");
        st.record(Err(FetchError::AccessKeyRejected), DHAKA, utc("2026-11-26 13:00"));
        assert!(st.warning(dhaka("2026-11-26 13:00")).is_some());
        st.record(Ok(NEWER.to_string()), DHAKA, utc("2026-11-26 13:15"));
        assert_eq!(st.prayers(DHAKA).unwrap().asr, Some(hm(15, 50)));
        assert_eq!(st.warning(dhaka("2026-11-26 13:15")), None);
        assert_eq!(st.last_success(), Some(utc("2026-11-26 13:15")));
    }

    #[test]
    fn a_bad_document_keeps_the_last_good_times() {
        let mut st = fetched_at("2026-11-25 13:00");
        st.record(Ok(r#"{"name":"x","timezone":"Asia/Dhaka","prayers":{"zuhr":"13:00"}}"#.into()), DHAKA, utc("2026-11-26 13:00"));
        assert_eq!(st.prayers(DHAKA).unwrap().asr, Some(hm(16, 0)));
        let warning = st.warning(dhaka("2026-11-26 13:00")).unwrap();
        assert!(warning.contains("Did you mean \"dhuhr\""), "{warning}");
        // Still counted as a failure: retried, not trusted.
        assert_eq!(st.last_success(), Some(utc("2026-11-25 13:00")));
    }

    #[test]
    fn an_unreachable_feed_warns_only_after_a_day() {
        let mut st = fetched_at("2026-11-25 13:00");
        st.record(Err(FetchError::Unreachable("offline".into())), DHAKA, utc("2026-11-26 13:00"));
        st.record(Err(FetchError::Unreachable("offline".into())), DHAKA, utc("2026-11-26 20:00"));
        assert_eq!(st.warning(dhaka("2026-11-26 20:00")), None);
        st.record(Err(FetchError::Unreachable("offline".into())), DHAKA, utc("2026-11-27 13:30"));
        let warning = st.warning(dhaka("2026-11-27 13:30")).unwrap();
        assert!(warning.contains("Baitul Aman Masjid"), "{warning}");
        assert_eq!(st.prayers(DHAKA).unwrap().fajr, Some(hm(5, 30)));
    }

    #[test]
    fn a_feed_never_reached_warns_at_once() {
        let mut st = OrganizationState::default();
        st.record(Err(FetchError::NotFound), DHAKA, utc("2026-11-26 09:00"));
        assert_eq!(st.warning(dhaka("2026-11-26 09:00")), Some(FetchError::NotFound.to_string()));
        assert_eq!(st.prayers(DHAKA), None);
    }

    #[test]
    fn a_rejected_access_key_warns_at_once_and_keeps_the_times() {
        let mut st = fetched_at("2026-11-25 13:00");
        st.record(Err(FetchError::AccessKeyRejected), DHAKA, utc("2026-11-26 13:00"));
        let warning = st.warning(dhaka("2026-11-26 13:00")).unwrap();
        assert!(warning.contains("access key"), "{warning}");
        assert!(st.prayers(DHAKA).is_some());
    }

    #[test]
    fn stored_times_for_another_timezone_are_not_used() {
        // The user moved to London after following a Dhaka masjid.
        let st = fetched_at("2026-11-25 13:00");
        assert_eq!(st.prayers("Europe/London"), None);
        assert_eq!(st.feed().unwrap().name, "Baitul Aman Masjid");
    }

    #[test]
    fn the_state_survives_a_restart() {
        let mut st = fetched_at("2026-11-25 13:00");
        st.record(Err(FetchError::Unreachable("offline".into())), DHAKA, utc("2026-11-26 13:00"));
        let back = OrganizationState::from_json(&st.to_json()).unwrap();
        assert_eq!(back, st);
        assert_eq!(back.prayers(DHAKA).unwrap().asr, Some(hm(16, 0)));
        assert!(!back.refresh_due(dhaka("2026-11-26 13:10")));
        assert_eq!(OrganizationState::from_json("not json"), None);
    }

    // -- fetch (against a one-shot local server) ----------------------------------

    use std::io::{Read as _, Write as _};
    use std::sync::mpsc;

    /// Serves `response` once on 127.0.0.1 and hands back the raw request.
    fn serve_once(response: String) -> (String, mpsc::Receiver<String>) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/prayer-times.json", listener.local_addr().unwrap());
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = Vec::new();
            let mut buf = [0u8; 1024];
            while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                let n = stream.read(&mut buf).unwrap();
                if n == 0 {
                    break;
                }
                request.extend_from_slice(&buf[..n]);
            }
            let _ = stream.write_all(response.as_bytes());
            let _ = tx.send(String::from_utf8_lossy(&request).to_string());
        });
        (url, rx)
    }

    fn http(status: &str, body: &str) -> String {
        format!(
            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
    }

    fn client() -> reqwest::blocking::Client {
        reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .unwrap()
    }

    #[test]
    fn fetch_returns_the_body_and_sends_the_access_key() {
        let (url, request) = serve_once(http("200 OK", GOOD));
        assert_eq!(fetch_url(&client(), &url, Some("k-123"), true).unwrap(), GOOD);
        let request = request.recv().unwrap().to_lowercase();
        assert!(request.contains("authorization: bearer k-123"), "{request}");
    }

    #[test]
    fn fetch_without_an_access_key_sends_no_authorization() {
        let (url, request) = serve_once(http("200 OK", GOOD));
        fetch_url(&client(), &url, None, true).unwrap();
        assert!(!request.recv().unwrap().to_lowercase().contains("authorization"));
    }

    #[test]
    fn fetch_maps_http_errors() {
        for (status, expected) in [
            ("401 Unauthorized", FetchError::AccessKeyRejected),
            ("403 Forbidden", FetchError::AccessKeyRejected),
            ("404 Not Found", FetchError::NotFound),
        ] {
            let (url, _rx) = serve_once(http(status, ""));
            assert_eq!(fetch_url(&client(), &url, None, true), Err(expected), "{status}");
        }
        let (url, _rx) = serve_once(http("500 Internal Server Error", ""));
        assert!(matches!(fetch_url(&client(), &url, None, true), Err(FetchError::Unreachable(_))));
    }

    #[test]
    fn fetch_refuses_a_huge_body() {
        let big = "x".repeat(MAX_FEED_BYTES as usize + 1);
        let (url, _rx) = serve_once(http("200 OK", &big));
        assert_eq!(fetch_url(&client(), &url, None, true), Err(FetchError::TooLarge));
    }

    #[test]
    fn fetch_refuses_plain_http_without_sending_anything() {
        let link = OrganizationLink {
            feed_url: "http://baitulaman.example/prayer-times.json".into(),
            access_key: Some("k-123".into()),
        };
        assert_eq!(fetch(&client(), &link), Err(FetchError::InsecureUrl));
        // Local http is for tests only, and only for 127.0.0.1.
        assert_eq!(
            fetch_url(&client(), "http://baitulaman.example/x.json", None, true),
            Err(FetchError::InsecureUrl)
        );
        assert_eq!(fetch_url(&client(), "not a url", None, false), Err(FetchError::InsecureUrl));
    }

    #[test]
    fn nothing_listening_is_unreachable() {
        let port = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        let url = format!("http://127.0.0.1:{port}/feed.json");
        assert!(matches!(fetch_url(&client(), &url, None, true), Err(FetchError::Unreachable(_))));
    }

    #[test]
    fn the_status_lists_covered_prayers_in_order_in_location_time() {
        let link = OrganizationLink {
            feed_url: "https://baitulaman.example/prayer-times.json".into(),
            access_key: None,
        };
        let st = fetched_at("2026-11-26 13:00");
        let dto = status_dto(&link, &st, dhaka("2026-11-26 14:00"));
        assert_eq!(dto.host, "baitulaman.example");
        assert_eq!(dto.name.as_deref(), Some("Baitul Aman Masjid"));
        let covered: Vec<_> = dto.prayers.iter().map(|p| (p.prayer, p.time.as_str())).collect();
        assert_eq!(covered, [(Prayer::Fajr, "05:30"), (Prayer::Asr, "16:00")]);
        assert_eq!(dto.last_fetched_at.as_deref(), Some("2026-11-26T13:00:00+06:00"));
        assert_eq!(dto.warning, None);
    }

    // -- The published examples (docs/examples/) ---------------------------------

    const EXAMPLE_FULL: &str = include_str!("../../docs/examples/organization-feed.json");
    const EXAMPLE_PARTIAL: &str = include_str!("../../docs/examples/organization-feed-partial.json");

    #[test]
    fn the_published_examples_are_valid_feeds() {
        let full = parse_feed(EXAMPLE_FULL, DHAKA).unwrap();
        assert!(Prayer::ALL.iter().all(|p| full.prayers.get(*p).is_some()));
        let partial = parse_feed(EXAMPLE_PARTIAL, DHAKA).unwrap();
        let covered: Vec<_> = Prayer::ALL.into_iter().filter(|p| partial.prayers.get(*p).is_some()).collect();
        assert_eq!(covered, [Prayer::Fajr, Prayer::Dhuhr, Prayer::Asr]);
    }
}

/// Live fetch of the example feed published in this repository. Not run by
/// default (CI doesn't depend on the network): `cargo test -- --ignored`.
/// Needs `docs/examples/` pushed to `main`.
#[cfg(test)]
mod network_tests {
    use super::*;

    #[test]
    #[ignore = "network"]
    fn the_example_feed_on_github_can_be_followed() {
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(20))
            .build()
            .unwrap();
        let link = OrganizationLink {
            feed_url: "https://raw.githubusercontent.com/iamraihan/prayer-times/main/docs/examples/organization-feed-partial.json".into(),
            access_key: None,
        };
        let body = fetch(&client, &link).unwrap();
        let feed = parse_feed(&body, "Asia/Dhaka").unwrap();
        assert_eq!(feed.prayers.get(Prayer::Maghrib), None);
        assert!(feed.prayers.get(Prayer::Fajr).is_some());
    }
}
