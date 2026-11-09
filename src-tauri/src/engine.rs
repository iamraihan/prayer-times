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
use crate::schedule::{self, Due, EventKind, PlannedEvent};
use crate::sound::Sound;
use crate::{alarm, runtime, settings as settings_io, timeutil, tray};

pub const TICK_SECONDS: u64 = 15;
/// Sound + popup auto-stop when nobody answers.
const ALARM_AUTO_STOP_SECS: i64 = 2 * 60;
/// Minimum wait between retries of a failed month fetch.
const RETRY_AFTER: StdDuration = StdDuration::from_secs(60);

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum AlarmKind {
    Start,
    Reminder,
    PreEnd,
    Snooze,
}

impl AlarmKind {
    pub fn id(self) -> &'static str {
        match self {
            AlarmKind::Start => "start",
            AlarmKind::Reminder => "reminder",
            AlarmKind::PreEnd => "preEnd",
            AlarmKind::Snooze => "snooze",
        }
    }
}

impl From<EventKind> for AlarmKind {
    fn from(kind: EventKind) -> Self {
        match kind {
            EventKind::Start => AlarmKind::Start,
            EventKind::PreEnd => AlarmKind::PreEnd,
        }
    }
}

pub struct ActiveAlarmState {
    pub id: String,
    pub prayer: Prayer,
    pub kind: AlarmKind,
    pub fired_at: DateTime<Utc>,
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
    pub city: String,
    pub country: String,
    pub method: u8,
    pub school: AsrSchool,
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

    pub fn alarm_done(&mut self) {
        let prayer = self.active_alarm.as_ref().map(|a| a.prayer);
        if let Some(prayer) = prayer {
            self.set_prayer_done(prayer, true);
        }
        self.clear_alarm();
    }

    pub fn alarm_snooze(&mut self, minutes: i64) {
        if let Some(alarm_state) = self.active_alarm.as_ref() {
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
                ends_at: a.ends_at.to_rfc3339(),
                message: a.message.clone(),
            }),
            api_error: self.last_error.clone(),
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

        let mut jobs = Vec::new();
        for key in keys {
            if self.months.contains_key(&key) {
                continue;
            }
            if let Some(cache) = aladhan::load_month(
                &self.cache_dir,
                self.settings.method,
                self.settings.school,
                key.0,
                key.1,
            ) {
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
                city: location.city.clone(),
                country: location.country_code.clone(),
                method: self.settings.method,
                school: self.settings.school,
            });
        }
        jobs
    }

    pub fn store_month(&mut self, key: (i32, u32), cache: aladhan::MonthCache) {
        aladhan::save_month(
            &self.cache_dir,
            &cache,
            self.settings.method,
            self.settings.school,
        );
        self.months.insert(key, cache);
        self.last_error = None;
    }

    pub fn note_fetch_error(&mut self, error: String) {
        self.last_error = Some(error);
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

        let events = schedule::compute(&day, tz, &self.settings);

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
                if let Some(event) = events.iter().find(|e| e.prayer == prayer) {
                    let id = format!("{}|{}|snooze|{}", today, prayer.id(), encoded_at);
                    self.fire(prayer, AlarmKind::Snooze, event.start, event.end, now, id);
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
                    // A delayed main alarm speaks the "you still have …"
                    // message, otherwise "it's time to pray".
                    let kind = if event.kind == EventKind::Start && event.delayed {
                        AlarmKind::Reminder
                    } else {
                        AlarmKind::from(event.kind)
                    };
                    self.fire(event.prayer, kind, event.start, event.end, now, id);
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

        self.update_status_tray(&day, tz, now, &events);
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
        start: DateTime<Tz>,
        end: DateTime<Tz>,
        now: DateTime<Tz>,
        id: String,
    ) {
        self.fired.insert(id.clone());
        self.dirty = true;

        let message = self.build_message(prayer, kind, start, end, now);
        self.active_alarm = Some(ActiveAlarmState {
            id,
            prayer,
            kind,
            fired_at: Utc::now(),
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
        start: DateTime<Tz>,
        end: DateTime<Tz>,
        now: DateTime<Tz>,
    ) -> AlarmMessageDto {
        let mode_24h = self.settings.time_format.is_24h();
        let fmt = |d: &DateTime<Tz>| alarm::fmt_time(d, mode_24h);
        let left = (end - now).num_minutes().max(0);
        let title = prayer.title();
        match kind {
            AlarmKind::Start => AlarmMessageDto {
                title: format!("🕌 {title} time"),
                body: format!(
                    "It's time to pray {title} ({}). Its window ends at {} — about {left} min left.",
                    fmt(&start),
                    fmt(&end)
                ),
            },
            AlarmKind::Reminder => AlarmMessageDto {
                title: format!("{title}: {left} minutes left"),
                body: format!(
                    "You still have about {left} minutes left to pray {title}. It ends at {}.",
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
        let events = schedule::compute(&day, tz, &self.settings);

        if let Some((event, is_current)) = schedule::featured(&events, now, |p| self.is_done(p)) {
            return Some(NextInfo {
                prayer: event.prayer,
                start: event.start,
                end: event.end,
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
        events: &[PlannedEvent],
    ) {
        let mode_24h = self.settings.time_format.is_24h();
        let fmt = |d: &DateTime<Tz>| alarm::fmt_time(d, mode_24h);

        let next = self.compute_next(now, tz);

        let (l1, l2, tooltip, done_label, done_enabled) =
            if let Some(active) = &self.active_alarm {
                (
                    format!("⏰ {} — prayer time", active.prayer.title()),
                    active.message.body.clone(),
                    format!("Prayer Times — {} alarm", active.prayer.title()),
                    format!("✓ Mark {} done", active.prayer.title()),
                    true,
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
        let (done_label, done_enabled) = match next {
            Some(_) => (done_label, done_enabled && !events.is_empty()),
            None => (done_label, false),
        };

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

fn location_key(settings: &Settings) -> Option<(String, String)> {
    settings
        .location
        .as_ref()
        .map(|l| (l.city.clone(), l.country_code.clone()))
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
            let params = aladhan::FetchParams {
                city: &job.city,
                country: &job.country,
                method: job.method,
                school: job.school,
            };
            let result = aladhan::fetch_month(&job.client, &params, job.key.0, job.key.1);
            results.push((job.key, result));
        }

        {
            let shared = app.state::<crate::SharedEngine>();
            let Ok(mut engine) = shared.engine.lock() else {
                break;
            };
            for (key, result) in results {
                match result {
                    Ok(cache) => engine.store_month(key, cache),
                    Err(e) => engine.note_fetch_error(e),
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
