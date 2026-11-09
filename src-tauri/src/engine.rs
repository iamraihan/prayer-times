//! The scheduler engine: a background thread that ticks every 15 seconds,
//! computes which alarm events are due, fires them (sound + notification +
//! popup), updates the tray, and pushes state snapshots to the frontend.
//!
//! Design notes:
//! - Tick-based (wall-clock comparison) rather than `sleep_until` tasks, so
//!   alarms still fire correctly after system sleep/wake or clock changes.
//! - Events missed while the app was closed fire late within a 10-minute
//!   grace window; beyond that they are marked as fired silently.
//! - Per-day state follows the Prayer Day (see `schedule::prayer_day`), not
//!   the calendar day, so an Isha window that runs past midnight keeps its
//!   alarms, snoozes and done mark until it closes.
//! - The decisions themselves live in `schedule` (pure, unit-tested); this
//!   module applies them and performs the side effects.
//! - Network fetches happen on the engine thread outside the state lock.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration as StdDuration, Instant};

use chrono::{DateTime, Datelike, Duration, NaiveDate, Utc};
use chrono_tz::Tz;
use tauri::{AppHandle, Emitter, Manager};

use crate::aladhan::{self, DayTimings};
use crate::models::{
    ActiveAlarmDto, AlarmMessageDto, AppStateDto, AsrSchool, DayTimingsDto, Location, NextPrayerDto,
    Prayer, PrayerTimeDto, Settings,
};
use crate::schedule::{self, Due, EventKind};
use crate::sound::Sound;
use crate::{alarm, runtime, settings as settings_io, timeutil, tray};

pub const TICK_SECONDS: u64 = 15;
/// Sound + popup auto-stop when nobody answers.
const ALARM_AUTO_STOP_SECS: i64 = 2 * 60;
/// Minimum wait between retries of a failed month fetch.
const RETRY_AFTER: StdDuration = StdDuration::from_secs(60);

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub enum AlarmKind {
    /// "Get ready": a heads-up before the window opens.
    GetReady,
    Start,
    PreEnd,
    Snooze,
}

impl From<EventKind> for AlarmKind {
    fn from(kind: EventKind) -> Self {
        match kind {
            EventKind::GetReady => AlarmKind::GetReady,
            EventKind::Start => AlarmKind::Start,
            EventKind::PreEnd => AlarmKind::PreEnd,
        }
    }
}

/// Alarm ids of dev-only previews (`preview_alarm`) start with this. A
/// preview's actions never mark a prayer done or snooze it.
const PREVIEW_ID_PREFIX: &str = "preview|";

fn is_preview(id: &str) -> bool {
    id.starts_with(PREVIEW_ID_PREFIX)
}

pub struct ActiveAlarmState {
    pub id: String,
    pub prayer: Prayer,
    pub kind: AlarmKind,
    pub fired_at: DateTime<Utc>,
    /// The user's prayer time, for a "Get ready" alarm's countdown.
    pub pray_at: DateTime<Tz>,
    /// When the prayer's window opens (before it, it can't be marked done).
    pub window_start: DateTime<Tz>,
    /// Prayer-window end, for the popup's live "ends in" countdown.
    pub ends_at: DateTime<Tz>,
    pub message: AlarmMessageDto,
}

pub struct NextInfo {
    pub prayer: Prayer,
    pub start: DateTime<Tz>,
    pub end: DateTime<Tz>,
    pub is_current: bool,
}

/// A month fetch planned under the lock, executed outside it.
pub struct FetchJob {
    pub key: (i32, u32),
    pub client: reqwest::blocking::Client,
    pub location: Location,
    pub method: u8,
    pub school: AsrSchool,
    /// What the result belongs to (`FetchParams::cache_id`). The result is
    /// dropped if the location/method/school changed while it was in flight.
    pub cache_id: String,
}

impl FetchJob {
    pub fn params(&self) -> aladhan::FetchParams<'_> {
        aladhan::FetchParams {
            place: aladhan::Place::of(&self.location),
            method: self.method,
            school: self.school,
        }
    }
}

pub struct Engine {
    pub app: AppHandle,
    pub client: reqwest::blocking::Client,
    pub cache_dir: PathBuf,
    pub settings: Settings,
    months: HashMap<(i32, u32), aladhan::MonthCache>,
    fetch_attempts: HashMap<(i32, u32), Instant>,

    day: Option<NaiveDate>,
    fired: HashSet<String>,
    snoozes: HashMap<Prayer, DateTime<Utc>>,
    done: HashMap<Prayer, DateTime<Utc>>,
    active_alarm: Option<ActiveAlarmState>,
    pub last_error: Option<String>,
    sound: Sound,
    dirty: bool,

    tray_l1: String,
    tray_l2: String,
    tray_tooltip: String,
    tray_done_label: String,
    tray_done_enabled: bool,
    window_title: String,
}

impl Engine {
    pub fn new(app: AppHandle) -> Result<Self, String> {
        let cache_dir = settings_io::config_dir(&app)?;
        let settings = settings_io::load(&app);
        let rt = runtime::load(&cache_dir);
        let client = reqwest::blocking::Client::builder()
            .timeout(StdDuration::from_secs(20))
            .user_agent(concat!("prayer-times/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|e| format!("http client: {e}"))?;

        let mut snoozes = HashMap::new();
        for (prayer, raw) in rt.snoozes {
            if let Some(dt) = runtime::decode_dt(&raw) {
                snoozes.insert(prayer, dt);
            }
        }
        let mut done = HashMap::new();
        for (prayer, raw) in rt.done {
            if let Some(dt) = runtime::decode_dt(&raw) {
                done.insert(prayer, dt);
            }
        }

        let mut sound = Sound::new();
        let custom_tone = settings
            .alarm_sound
            .as_ref()
            .map(|name| cache_dir.join(name))
            .filter(|path| path.is_file());
        sound.load(custom_tone);

        Ok(Engine {
            app,
            client,
            cache_dir,
            settings,
            months: HashMap::new(),
            fetch_attempts: HashMap::new(),
            day: rt.day,
            fired: rt.fired.into_iter().collect(),
            snoozes,
            done,
            active_alarm: None,
            last_error: None,
            sound,
            dirty: false,
            tray_l1: String::new(),
            tray_l2: String::new(),
            tray_tooltip: String::new(),
            tray_done_label: String::new(),
            tray_done_enabled: false,
            window_title: String::new(),
        })
    }

    // -- public API used by commands --------------------------------------

    pub fn apply_settings(&mut self, mut settings: Settings) -> Result<(), String> {
        // The alarm tone is owned by set_alarm_tone/reset_alarm_tone (they
        // apply instantly); ignore whatever value a settings draft carries,
        // otherwise "Save changes" would reset a just-picked tone to default.
        settings.alarm_sound = self.settings.alarm_sound.clone();
        let calc_changed =
            settings.method != self.settings.method || settings.school != self.settings.school;
        let loc_changed = location_key(&settings) != location_key(&self.settings);
        self.settings = settings;
        if calc_changed || loc_changed {
            self.reset_schedule();
        }
        self.dirty = true;
        settings_io::save(&self.app, &self.settings)
    }

    /// Copies a user-chosen audio file into the app config dir and makes it
    /// the alarm tone. Validation (extension/size/decodable) happens in the
    /// command layer.
    pub fn set_alarm_tone(&mut self, source: PathBuf, extension: &str) -> Result<(), String> {
        let dest = self.cache_dir.join(format!("alarm-tone.{extension}"));
        // Copy first so a failed copy leaves the current tone intact.
        std::fs::copy(&source, &dest)
            .map_err(|e| format!("couldn't copy the tone file: {e}"))?;
        // Then drop any previous custom tone with a different extension.
        if let Ok(entries) = std::fs::read_dir(&self.cache_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if name.starts_with("alarm-tone.") && name != format!("alarm-tone.{extension}") {
                    let _ = std::fs::remove_file(entry.path());
                }
            }
        }
        self.settings.alarm_sound = Some(format!("alarm-tone.{extension}"));
        self.sound.load(Some(dest));
        self.dirty = true;
        settings_io::save(&self.app, &self.settings)
            .map_err(|e| format!("couldn't save settings: {e}"))
    }

    pub fn reset_alarm_tone(&mut self) -> Result<(), String> {
        if let Some(name) = self.settings.alarm_sound.take() {
            let _ = std::fs::remove_file(self.cache_dir.join(name));
        }
        self.sound.load(None);
        self.dirty = true;
        settings_io::save(&self.app, &self.settings)
            .map_err(|e| format!("couldn't save settings: {e}"))
    }

    pub fn save_location(&mut self, location: Location, method: u8, school: AsrSchool) {
        self.settings.method = method;
        self.settings.school = school;
        self.settings.location = Some(location);
        self.reset_schedule();
        let _ = settings_io::save(&self.app, &self.settings);
        self.dirty = true;
    }

    pub fn set_prayer_done(&mut self, prayer: Prayer, done: bool) {
        if done {
            self.done.insert(prayer, Utc::now());
            self.snoozes.remove(&prayer);
            if self.active_alarm.as_ref().map(|a| a.prayer) == Some(prayer) {
                self.clear_alarm();
            }
        } else {
            self.done.remove(&prayer);
        }
        self.dirty = true;
    }

    fn showing_preview(&self) -> bool {
        self.active_alarm.as_ref().is_some_and(|a| is_preview(&a.id))
    }

    /// Dev only: shows a pretend alarm of `kind` for the featured prayer
    /// through the normal channels (sound, notification, popup), so the
    /// design can be checked without waiting for a real one. It touches no
    /// Done marks, snoozes or fired ids.
    pub fn preview_alarm(&mut self, kind: AlarmKind) -> Result<(), String> {
        if self.active_alarm.is_some() && !self.showing_preview() {
            return Err("A real alarm is ringing; answer it first".into());
        }
        let tz = self
            .settings
            .location
            .as_ref()
            .and_then(|l| timeutil::parse_tz(&l.timezone))
            .ok_or("Set a location first")?;
        let now = Utc::now().with_timezone(&tz);
        let prayer = self
            .compute_next(now, tz)
            .map_or(Prayer::Dhuhr, |n| n.prayer);
        let (pray_at, end) = match kind {
            AlarmKind::GetReady => (now + Duration::minutes(10), now + Duration::minutes(90)),
            AlarmKind::PreEnd => (now - Duration::minutes(50), now + Duration::minutes(10)),
            AlarmKind::Start | AlarmKind::Snooze => (now, now + Duration::minutes(90)),
        };
        let start = if kind == AlarmKind::GetReady { pray_at } else { pray_at.min(now) };
        let id = format!("{PREVIEW_ID_PREFIX}{}", prayer.id());
        self.fire(prayer, kind, (start, pray_at, end), now, id.clone());
        self.fired.remove(&id);
        Ok(())
    }

    pub fn alarm_done(&mut self) {
        if self.showing_preview() {
            self.clear_alarm();
            return;
        }
        let prayer = self.active_alarm.as_ref().map(|a| a.prayer);
        if let Some(prayer) = prayer {
            self.set_prayer_done(prayer, true);
        }
        self.clear_alarm();
    }

    pub fn alarm_snooze(&mut self, minutes: i64) {
        if let Some(alarm_state) = self.active_alarm.as_ref().filter(|a| !is_preview(&a.id)) {
            let due = Utc::now() + Duration::minutes(minutes.clamp(1, 720));
            self.snoozes.insert(alarm_state.prayer, due);
            self.dirty = true;
        }
        self.clear_alarm();
    }

    pub fn alarm_dismiss(&mut self) {
        self.clear_alarm();
        self.dirty = true;
    }

    /// Plays the current tone briefly. `volume` previews the slider's
    /// position; `None` uses the saved volume.
    pub fn test_sound(&mut self, volume: Option<f32>) {
        let volume = volume.unwrap_or(self.settings.sound_volume);
        self.sound.play_loop(volume);
        let app = self.app.clone();
        std::thread::spawn(move || {
            std::thread::sleep(StdDuration::from_millis(2500));
            let shared = app.state::<crate::SharedEngine>();
            if let Ok(mut engine) = shared.engine.lock() {
                if engine.active_alarm.is_none() {
                    engine.sound.stop();
                }
            };
        });
    }

    /// Tray "✓ Mark done" targets the prayer whose window is open, or the
    /// next upcoming one.
    pub fn next_or_current_prayer(&mut self) -> Option<Prayer> {
        let tz_name = self.settings.location.as_ref()?.timezone.clone();
        let tz = timeutil::parse_tz(&tz_name)?;
        let now = Utc::now().with_timezone(&tz);
        self.compute_next(now, tz).map(|n| n.prayer)
    }

    pub fn snapshot(&self) -> AppStateDto {
        let settings = self.settings.clone();
        let has_location = settings.location.is_some();
        let location_warning = settings.location.as_ref().and_then(location_warning);
        let mut today_dto = None;
        let mut next_dto = None;

        if let Some(location) = &settings.location {
            if let Some(tz) = timeutil::parse_tz(&location.timezone) {
                let now = Utc::now().with_timezone(&tz);
                let pday = self.prayer_day(now, tz);
                if let Some(day) = self.day_timings(pday) {
                    let from_cache = self.month_from_cache(pday).unwrap_or(false);
                    today_dto = Some(day_timings_dto(&day, Some(tz), from_cache));
                    if let Some(next) = self.compute_next(now, tz) {
                        next_dto = Some(NextPrayerDto {
                            prayer: next.prayer,
                            start_at: next.start.to_rfc3339(),
                            ends_at: next.end.to_rfc3339(),
                        });
                    }
                }
            }
        }

        AppStateDto {
            has_location,
            settings,
            today: today_dto,
            next: next_dto,
            done_today: self.done.iter().map(|(prayer, _)| (*prayer, true)).collect(),
            snoozes: self
                .snoozes
                .iter()
                .map(|(prayer, at)| (*prayer, at.to_rfc3339()))
                .collect(),
            active_alarm: self.active_alarm.as_ref().map(|a| ActiveAlarmDto {
                id: a.id.clone(),
                prayer: a.prayer,
                kind: a.kind,
                started_at: a.fired_at.to_rfc3339(),
                pray_at: a.pray_at.to_rfc3339(),
                ends_at: a.ends_at.to_rfc3339(),
                message: a.message.clone(),
            }),
            api_error: self.last_error.clone(),
            location_warning,
        }
    }

    // -- fetch planning ----------------------------------------------------

    pub fn plan_fetches(&mut self) -> Vec<FetchJob> {
        let Some(location) = self.settings.location.clone() else {
            return Vec::new();
        };
        let Some(tz) = timeutil::parse_tz(&location.timezone) else {
            return Vec::new();
        };
        // Dates in the *location's* zone (not UTC): yesterday for an Isha
        // window still open past midnight, today for the schedule, tomorrow
        // for the "next prayer" rollover to tomorrow's Fajr.
        let today = Utc::now().with_timezone(&tz).date_naive();
        let wanted = [today.pred_opt(), Some(today), today.succ_opt()];

        let mut keys: Vec<(i32, u32)> = Vec::new();
        for date in wanted.into_iter().flatten() {
            let key = (date.year(), date.month());
            if !keys.contains(&key) {
                keys.push(key);
            }
        }

        let cache_id = self.cache_id(&location);
        let mut jobs = Vec::new();
        for key in keys {
            if self.months.contains_key(&key) {
                continue;
            }
            if let Some(cache) = aladhan::load_month(&self.cache_dir, &cache_id, key.0, key.1) {
                self.months.insert(key, cache);
                continue;
            }
            let recently_tried = self
                .fetch_attempts
                .get(&key)
                .map(|t| t.elapsed() < RETRY_AFTER)
                .unwrap_or(false);
            if recently_tried {
                continue;
            }
            self.fetch_attempts.insert(key, Instant::now());
            jobs.push(FetchJob {
                key,
                client: self.client.clone(),
                location: location.clone(),
                method: self.settings.method,
                school: self.settings.school,
                cache_id: cache_id.clone(),
            });
        }
        jobs
    }

    fn cache_id(&self, location: &Location) -> String {
        aladhan::FetchParams {
            place: aladhan::Place::of(location),
            method: self.settings.method,
            school: self.settings.school,
        }
        .cache_id()
    }

    /// Whether a finished fetch still matches the current settings.
    fn is_current(&self, job_cache_id: &str) -> bool {
        self.settings
            .location
            .as_ref()
            .is_some_and(|l| self.cache_id(l) == job_cache_id)
    }

    pub fn store_month(&mut self, job: &FetchJob, cache: aladhan::MonthCache) {
        if !self.is_current(&job.cache_id) {
            return;
        }
        aladhan::save_month(&self.cache_dir, &job.cache_id, &cache);
        self.months.insert(job.key, cache);
        self.last_error = None;
    }

    pub fn note_fetch_error(&mut self, job: &FetchJob, error: String) {
        if self.is_current(&job.cache_id) {
            self.last_error = Some(error);
        }
    }

    // -- the tick ----------------------------------------------------------

    pub fn tick(&mut self) {
        let Some(location) = self.settings.location.clone() else {
            self.apply_tray(
                "Prayer Times".to_string(),
                "Set your location to begin".to_string(),
                "Prayer Times — set your location".to_string(),
                false,
                String::new(),
            );
            self.push_state();
            return;
        };
        let Some(tz) = timeutil::parse_tz(&location.timezone) else {
            self.last_error = Some(format!("unknown timezone: {}", location.timezone));
            self.push_state();
            return;
        };
        let now = Utc::now().with_timezone(&tz);
        let today = self.prayer_day(now, tz);

        // Prayer Day rollover: reset per-day state.
        if self.day != Some(today) {
            self.day = Some(today);
            self.fired.clear();
            self.snoozes.clear();
            self.done.clear();
            self.clear_alarm();
            self.dirty = true;
        }

        let Some(day) = self.day_timings(today) else {
            self.apply_tray(
                "Prayer Times".to_string(),
                match &self.last_error {
                    Some(e) => format!("Can't fetch times: {e}"),
                    None => "Loading today's times…".to_string(),
                },
                "Prayer Times".to_string(),
                false,
                String::new(),
            );
            self.push_state();
            return;
        };

        let yesterday = today.pred_opt().and_then(|d| self.day_timings(d));
        let begins = schedule::day_start(&day, tz, yesterday.as_ref());
        let events = schedule::compute(&day, tz, &self.settings, begins);
        let windows = schedule::windows(&day, tz, &self.settings);

        // Due snoozes (one-shot, then removed from the map).
        let due_snoozes: Vec<(Prayer, String)> = self
            .snoozes
            .iter()
            .filter(|(_, at)| **at <= now)
            .map(|(prayer, at)| (*prayer, runtime::encode_dt(at)))
            .collect();
        for (prayer, encoded_at) in due_snoozes {
            self.snoozes.remove(&prayer);
            self.dirty = true;
            if !self.is_done(prayer) {
                // From the window, not the planned events, so a snooze still
                // rings after its prayer's alarms are switched off.
                if let Some(w) = windows.iter().find(|w| w.prayer == prayer) {
                    let after = self.settings.prayer_settings.get(prayer).pray_after_minutes;
                    let pray_at = schedule::pray_time(w.start, w.end, after);
                    let id = format!("{}|{}|snooze|{}", today, prayer.id(), encoded_at);
                    self.fire(prayer, AlarmKind::Snooze, (w.start, pray_at, w.end), now, id);
                }
            }
        }

        // Planned events.
        for event in &events {
            if self.is_done(event.prayer) {
                continue;
            }
            let id = schedule::event_id(today, event.prayer, event.kind);
            match schedule::due(event.at, now, self.fired.contains(&id)) {
                Due::Fire => {
                    let kind = AlarmKind::from(event.kind);
                    self.fire(event.prayer, kind, (event.start, event.pray_at, event.end), now, id);
                }
                Due::SkipMissed => {
                    // Missed for too long (app closed / asleep): skip silently.
                    self.fired.insert(id);
                    self.dirty = true;
                }
                Due::Rearm => {
                    // Settings changed and moved this event back into the future.
                    self.fired.remove(&id);
                    self.dirty = true;
                }
                Due::Wait => {}
            }
        }

        // Auto-stop a long unanswered alarm (sound/popup; done still possible
        // from the main window or the tray).
        if let Some(active) = &self.active_alarm {
            if (Utc::now() - active.fired_at) > Duration::seconds(ALARM_AUTO_STOP_SECS) {
                self.clear_alarm();
                self.dirty = true;
            }
        }

        self.update_status_tray(&day, tz, now);
        self.push_state();

        if self.dirty {
            self.persist_runtime();
            self.dirty = false;
        }
    }

    // -- internals ----------------------------------------------------------

    fn fire(
        &mut self,
        prayer: Prayer,
        kind: AlarmKind,
        (start, pray_at, end): (DateTime<Tz>, DateTime<Tz>, DateTime<Tz>),
        now: DateTime<Tz>,
        id: String,
    ) {
        self.fired.insert(id.clone());
        self.dirty = true;

        let message = self.build_message(prayer, kind, pray_at, end, now);
        self.active_alarm = Some(ActiveAlarmState {
            id,
            prayer,
            kind,
            fired_at: Utc::now(),
            pray_at,
            window_start: start,
            ends_at: end,
            message: message.clone(),
        });

        if self.settings.sound_enabled {
            self.sound.play_loop(self.settings.sound_volume);
        }
        if self.settings.notifications_enabled {
            alarm::notify(&self.app, &message);
        }
        if self.settings.popup_enabled {
            alarm::show_window(&self.app);
        } else {
            alarm::hide_window(&self.app);
        }
    }

    fn build_message(
        &self,
        prayer: Prayer,
        kind: AlarmKind,
        pray_at: DateTime<Tz>,
        end: DateTime<Tz>,
        now: DateTime<Tz>,
    ) -> AlarmMessageDto {
        let mode_24h = self.settings.time_format.is_24h();
        let fmt = |d: &DateTime<Tz>| alarm::fmt_time(d, mode_24h);
        let left = (end - now).num_minutes().max(0);
        let title = prayer.title();
        let until = (pray_at - now).num_minutes().max(0);
        match kind {
            AlarmKind::GetReady => AlarmMessageDto {
                title: format!("🕌 {title} in {until} minutes"),
                body: format!(
                    "{title} at {} — about {until} min from now. Time to get ready.",
                    fmt(&pray_at)
                ),
            },
            // A snooze taken on a "Get ready" can come due before the
            // user's prayer time.
            AlarmKind::Snooze if now < pray_at => AlarmMessageDto {
                title: format!("🕌 Reminder: {title}"),
                body: format!(
                    "{title} at {} — about {until} min from now.",
                    fmt(&pray_at)
                ),
            },
            AlarmKind::Start => AlarmMessageDto {
                title: format!("🕌 {title} time"),
                body: format!(
                    "It's time to pray {title} ({}). Its window ends at {} — about {left} min left.",
                    fmt(&pray_at),
                    fmt(&end)
                ),
            },
            AlarmKind::PreEnd => AlarmMessageDto {
                title: format!("⚠️ {title} is ending"),
                body: format!(
                    "Only {left} minutes left — you will miss {title}! It ends at {}.",
                    fmt(&end)
                ),
            },
            AlarmKind::Snooze => AlarmMessageDto {
                title: format!("🕌 Reminder: {title}"),
                body: format!(
                    "Time to pray {title}. About {left} minutes left until {}.",
                    fmt(&end)
                ),
            },
        }
    }

    fn clear_alarm(&mut self) {
        self.active_alarm = None;
        self.sound.stop();
        alarm::hide_window(&self.app);
    }

    fn is_done(&self, prayer: Prayer) -> bool {
        self.done.contains_key(&prayer)
    }

    fn reset_schedule(&mut self) {
        self.months.clear();
        self.fetch_attempts.clear();
        self.day = None;
        self.fired.clear();
        self.snoozes.clear();
        self.done.clear();
        self.clear_alarm();
        self.last_error = None;
        self.dirty = true;
    }

    fn day_timings(&self, date: NaiveDate) -> Option<DayTimings> {
        self.months
            .get(&(date.year(), date.month()))?
            .days
            .get(&date.day())
            .cloned()
    }

    fn month_from_cache(&self, date: NaiveDate) -> Option<bool> {
        self.months
            .get(&(date.year(), date.month()))
            .map(|m| m.from_cache)
    }

    /// The Prayer Day `now` belongs to (yesterday while its Isha window is
    /// still open past midnight).
    fn prayer_day(&self, now: DateTime<Tz>, tz: Tz) -> NaiveDate {
        let yesterday = now
            .date_naive()
            .pred_opt()
            .and_then(|d| self.day_timings(d));
        schedule::prayer_day(now, tz, yesterday.as_ref())
    }

    /// The prayer the UI should feature: the one whose window is currently
    /// open (and not marked done), else the next upcoming start (possibly
    /// the next Prayer Day's Fajr).
    fn compute_next(&self, now: DateTime<Tz>, tz: Tz) -> Option<NextInfo> {
        let pday = self.prayer_day(now, tz);
        let day = self.day_timings(pday)?;
        let windows = schedule::windows(&day, tz, &self.settings);

        if let Some((w, is_current)) = schedule::featured(&windows, now, |p| self.is_done(p)) {
            return Some(NextInfo {
                prayer: w.prayer,
                start: w.start,
                end: w.end,
                is_current,
            });
        }

        // After Isha: the next Prayer Day's Fajr.
        let next_day = self.day_timings(pday.succ_opt()?)?;
        let (start, end) = schedule::window(Prayer::Fajr, &next_day, tz)?;
        Some(NextInfo {
            prayer: Prayer::Fajr,
            start,
            end,
            is_current: false,
        })
    }

    fn update_status_tray(
        &mut self,
        day: &DayTimings,
        tz: Tz,
        now: DateTime<Tz>,
    ) {
        let mode_24h = self.settings.time_format.is_24h();
        let fmt = |d: &DateTime<Tz>| alarm::fmt_time(d, mode_24h);

        let next = self.compute_next(now, tz);

        let (l1, l2, tooltip, done_label, done_enabled) =
            if let Some(active) = &self.active_alarm {
                let title = active.prayer.title();
                let line = match active.kind {
                    AlarmKind::GetReady => format!("⏰ Get ready — {title}"),
                    AlarmKind::PreEnd => format!("⚠️ {title} is ending"),
                    AlarmKind::Start | AlarmKind::Snooze => format!("⏰ {title} — prayer time"),
                };
                // Like the popup: nothing to mark before the prayer starts.
                let started = now >= active.window_start;
                (
                    line,
                    active.message.body.clone(),
                    format!("Prayer Times — {title} alarm"),
                    format!("✓ Mark {title} done"),
                    started,
                )
            } else if let Some(next) = &next {
                let target = if next.is_current {
                    next.end
                } else {
                    next.start
                };
                let base = if next.is_current {
                    format!("{} — ends {}", next.prayer.title(), fmt(&next.end))
                } else {
                    format!("Next: {} {}", next.prayer.title(), fmt(&next.start))
                };
                let second = format!(
                    "in {} · window ends {}",
                    short_remaining(target, now),
                    fmt(&next.end)
                );
                (
                    base,
                    second,
                    format!("Prayer Times — {}", next.prayer.title()),
                    format!("✓ Mark {} done", next.prayer.title()),
                    true,
                )
            } else {
                (
                    "All prayers done today 🎉".to_string(),
                    format!(
                        "Times for {} — check back tomorrow",
                        day.readable
                    ),
                    "Prayer Times".to_string(),
                    String::new(),
                    false,
                )
            };

        // Suppress the "mark done" item when there is nothing left to mark.
        // Based on the featured prayer, not on alarms: it works with every
        // alarm switched off.
        let done_enabled = done_enabled && next.is_some();

        self.apply_tray(l1, l2, tooltip, done_enabled, done_label);
    }

    fn apply_tray(
        &mut self,
        l1: String,
        l2: String,
        tooltip: String,
        done_enabled: bool,
        done_label: String,
    ) {
        let title = format!("Prayer Times — {l1}");
        if l1 != self.tray_l1
            || l2 != self.tray_l2
            || tooltip != self.tray_tooltip
            || done_enabled != self.tray_done_enabled
            || done_label != self.tray_done_label
        {
            self.tray_l1 = l1.clone();
            self.tray_l2 = l2.clone();
            self.tray_tooltip = tooltip.clone();
            self.tray_done_enabled = done_enabled;
            self.tray_done_label = done_label.clone();

            let app = self.app.clone();
            let thread_app = app.clone();
            let _ = app.run_on_main_thread(move || {
                tray::update(&thread_app, &l1, &l2, &tooltip, done_enabled, &done_label);
            });
        }
        if title != self.window_title {
            self.window_title = title.clone();
            if let Some(win) = self.app.get_webview_window("main") {
                let _ = win.set_title(&title);
            }
        }
    }

    fn push_state(&self) {
        let _ = self.app.emit("app-state", self.snapshot());
    }

    fn persist_runtime(&self) {
        let state = runtime::RuntimeState {
            day: self.day,
            fired: self.fired.iter().cloned().collect(),
            snoozes: self
                .snoozes
                .iter()
                .map(|(prayer, at)| (*prayer, runtime::encode_dt(at)))
                .collect(),
            done: self
                .done
                .iter()
                .map(|(prayer, at)| (*prayer, runtime::encode_dt(at)))
                .collect(),
        };
        runtime::save(&self.cache_dir, &state);
    }
}

fn short_remaining(target: DateTime<Tz>, now: DateTime<Tz>) -> String {
    let secs = (target - now).num_seconds().max(0);
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    if h > 0 {
        format!("{h}h {m}m")
    } else {
        format!("{m}m")
    }
}

/// Identity of the saved location: changing any of it refetches the schedule.
fn location_key(settings: &Settings) -> Option<String> {
    settings.location.as_ref().map(|l| {
        let params = aladhan::FetchParams {
            place: aladhan::Place::of(l),
            method: 0,
            school: AsrSchool::Standard,
        };
        params.cache_id()
    })
}

/// A typed city name that the API matched to a place in another country
/// ("Londn, United Kingdom" → America/New_York) gives wrong times silently;
/// flag it so the user re-picks the city from search.
pub fn location_warning(location: &Location) -> Option<String> {
    match crate::tzcountry::zone_in_country(&location.timezone, &location.country_code) {
        Some(false) => Some(format!(
            "“{}” was matched to a place in the {} timezone, which isn't in {}, so these \
             times are probably for the wrong place. Pick your city again in Settings → Location.",
            location.city, location.timezone, location.country
        )),
        _ => None,
    }
}

/// Builds the wire format for one day. With `tz`, each prayer also carries
/// its window as exact instants (so the UI never does wall-clock math).
pub fn day_timings_dto(day: &DayTimings, tz: Option<Tz>, from_cache: bool) -> DayTimingsDto {
    DayTimingsDto {
        date: day.date.format("%Y-%m-%d").to_string(),
        gregorian: day.readable.clone(),
        hijri: day.hijri.clone(),
        sunrise: day.sunrise.format("%H:%M").to_string(),
        midnight: day.midnight.format("%H:%M").to_string(),
        timings: Prayer::ALL
            .iter()
            .filter_map(|prayer| {
                day.times.get(prayer).map(|&start| {
                    let window = tz.and_then(|tz| schedule::window(*prayer, day, tz));
                    PrayerTimeDto {
                        prayer: *prayer,
                        start: start.format("%H:%M").to_string(),
                        ends: schedule::end_time_of(*prayer, day)
                            .format("%H:%M")
                            .to_string(),
                        start_at: window.map(|(s, _)| s.to_rfc3339()),
                        ends_at: window.map(|(_, e)| e.to_rfc3339()),
                    }
                })
            })
            .collect(),
        from_cache,
    }
}

/// Marks the current/next prayer done from the tray menu (main thread).
pub fn tray_mark_done(app: &AppHandle) {
    let shared = app.state::<crate::SharedEngine>();
    let Ok(mut engine) = shared.engine.lock() else {
        return;
    };
    // A dev preview is only closed: it must never mark a real prayer done.
    if engine.showing_preview() {
        engine.alarm_dismiss();
        engine.tick();
        return;
    }
    if let Some(prayer) = engine.next_or_current_prayer() {
        engine.set_prayer_done(prayer, true);
        engine.tick();
    }
}

/// Wakes the engine thread so planned fetches/ticks run immediately
/// (called after settings/location changes).
pub fn wake_up(shared: &crate::SharedEngine) {
    if let Ok(mut flag) = shared.wake.0.lock() {
        *flag = true;
        shared.wake.1.notify_all();
    }
}

/// The engine thread: wait → plan fetches → fetch (outside the lock) → tick.
pub fn run_loop(app: AppHandle, wake: Arc<(Mutex<bool>, Condvar)>) {
    loop {
        let jobs = {
            let shared = app.state::<crate::SharedEngine>();
            let Ok(mut engine) = shared.engine.lock() else {
                break;
            };
            engine.plan_fetches()
        };

        let mut results = Vec::new();
        for job in jobs {
            let result = aladhan::fetch_month(&job.client, &job.params(), job.key.0, job.key.1);
            results.push((job, result));
        }

        {
            let shared = app.state::<crate::SharedEngine>();
            let Ok(mut engine) = shared.engine.lock() else {
                break;
            };
            for (job, result) in results {
                match result {
                    Ok(cache) => engine.store_month(&job, cache),
                    Err(e) => engine.note_fetch_error(&job, e),
                }
            }
            engine.tick();
        }

        // Sleep until the next tick or a wake signal.
        if let Ok(mut flag) = wake.0.lock() {
            if !*flag {
                match wake
                    .1
                    .wait_timeout(flag, StdDuration::from_secs(TICK_SECONDS))
                {
                    Ok((mut guard, _)) => *guard = false,
                    Err(poisoned) => *poisoned.into_inner().0 = false,
                }
            } else {
                *flag = false;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_ids_never_look_like_real_events() {
        assert!(is_preview("preview|dhuhr"));
        let real = schedule::event_id(
            chrono::NaiveDate::from_ymd_opt(2026, 11, 10).unwrap(),
            Prayer::Dhuhr,
            EventKind::Start,
        );
        assert!(!is_preview(&real));
        assert!(!is_preview(&format!("{real}|snooze|x")));
    }

    fn typed(city: &str, country: &str, code: &str, timezone: &str) -> Location {
        Location {
            city: city.to_string(),
            country: country.to_string(),
            country_code: code.to_string(),
            timezone: timezone.to_string(),
            region: None,
            coordinates: None,
        }
    }

    #[test]
    fn warns_when_typed_city_landed_in_another_country() {
        let londn = typed("Londn", "United Kingdom", "GB", "America/New_York");
        let warning = location_warning(&londn).unwrap();
        assert!(warning.contains("Londn") && warning.contains("America/New_York"));
        assert!(location_warning(&typed("Mumbai", "Bangladesh", "BD", "Asia/Kolkata")).is_some());
    }

    #[test]
    fn no_warning_for_matching_or_unknown_zones() {
        assert!(location_warning(&typed("London", "United Kingdom", "GB", "Europe/London")).is_none());
        assert!(location_warning(&typed("Dhaka", "Bangladesh", "BD", "Asia/Dhaka")).is_none());
        assert!(location_warning(&typed("Kolkata", "India", "IN", "Asia/Calcutta")).is_none());
    }

    #[test]
    fn location_key_changes_with_city_or_coordinates() {
        let mut a = Settings::default();
        a.location = Some(typed("Dhaka", "Bangladesh", "BD", "Asia/Dhaka"));
        let mut b = a.clone();
        assert_eq!(location_key(&a), location_key(&b));
        if let Some(l) = b.location.as_mut() {
            l.coordinates = Some(crate::models::Coordinates {
                latitude: 23.7104,
                longitude: 90.40744,
            });
        }
        assert_ne!(location_key(&a), location_key(&b));
    }
}
