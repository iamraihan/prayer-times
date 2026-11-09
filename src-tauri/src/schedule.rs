//! Pure schedule computation: turns a day's timings + settings into the list
//! of alarm events for that day, and holds the small decisions the engine
//! makes on every tick (which Prayer Day is active, whether an event is due,
//! which prayer to feature). No I/O, no clock — `now` is always passed in —
//! so everything here is unit-tested below.

use chrono::{DateTime, Duration, NaiveDate, NaiveTime};
use chrono_tz::Tz;

use crate::aladhan::DayTimings;
use crate::models::{Prayer, Settings};
use crate::timeutil;

/// Fire events up to this long after their scheduled time; older ones are
/// skipped silently (app was closed / machine asleep).
pub const GRACE_SECS: i64 = 10 * 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventKind {
    /// The prayer's main alarm.
    Start,
    /// Final warning before the window closes.
    PreEnd,
}

impl EventKind {
    pub fn id(self) -> &'static str {
        match self {
            EventKind::Start => "start",
            EventKind::PreEnd => "preEnd",
        }
    }
}

#[derive(Debug, Clone)]
pub struct PlannedEvent {
    pub kind: EventKind,
    /// True when the main alarm is delayed past the start time
    /// (reminder_minutes set) — the engine uses the "you still have …"
    /// message for it.
    pub delayed: bool,
    pub prayer: Prayer,
    /// When this event fires.
    pub at: DateTime<Tz>,
    /// Window start (the prayer's own time).
    pub start: DateTime<Tz>,
    /// Window end (when the prayer's time closes).
    pub end: DateTime<Tz>,
}

/// The end of a prayer's valid window:
/// Fajr → sunrise, Dhuhr → Asr, Asr → Maghrib, Maghrib → Isha, Isha → midnight.
pub fn end_time_of(prayer: Prayer, day: &DayTimings) -> NaiveTime {
    let fallback = |a: Option<&NaiveTime>, b: NaiveTime| a.copied().unwrap_or(b);
    match prayer {
        Prayer::Fajr => day.sunrise,
        Prayer::Dhuhr => fallback(
            day.times.get(&Prayer::Asr),
            fallback(day.times.get(&Prayer::Maghrib), day.sunrise),
        ),
        Prayer::Asr => fallback(day.times.get(&Prayer::Maghrib), day.sunrise),
        Prayer::Maghrib => fallback(day.times.get(&Prayer::Isha), day.sunrise),
        Prayer::Isha => day.midnight,
    }
}

/// A prayer's window as real instants in `tz`. The end rolls to the next
/// calendar day when it is earlier than the start (Isha → midnight 00:40).
/// `None` when the prayer is missing, a time falls in a DST gap, or the
/// window would be empty.
pub fn window(prayer: Prayer, day: &DayTimings, tz: Tz) -> Option<(DateTime<Tz>, DateTime<Tz>)> {
    let start_t = *day.times.get(&prayer)?;
    let start = timeutil::resolve(tz, day.date, start_t)?;
    let end = timeutil::resolve_after(tz, day.date, end_time_of(prayer, day), start_t)?;
    (end > start).then_some((start, end))
}

/// The Prayer Day that `now` belongs to: the calendar date, except while
/// yesterday's Isha window is still open past midnight — then it is
/// yesterday. Per-day state (fired, done, snoozes) resets when this changes.
pub fn prayer_day(now: DateTime<Tz>, tz: Tz, yesterday: Option<&DayTimings>) -> NaiveDate {
    let today = now.date_naive();
    if let Some(y) = yesterday {
        if today.pred_opt() == Some(y.date) {
            if let Some((_, end)) = window(Prayer::Isha, y, tz) {
                if now < end {
                    return y.date;
                }
            }
        }
    }
    today
}

pub fn compute(day: &DayTimings, tz: Tz, settings: &Settings) -> Vec<PlannedEvent> {
    let mut events = Vec::new();

    for prayer in Prayer::ALL {
        let ps = settings.prayer_settings.get(prayer);
        if !ps.enabled {
            continue;
        }
        let Some((start, end)) = window(prayer, day, tz) else {
            continue;
        };

        // Main alarm: exactly at the start time, or delayed N minutes after
        // it when a value is set ("you still have … minutes"). The delay is
        // clamped so the alarm never lands in the last 5 minutes of the
        // window (that's the before-end warning's job).
        let delay = ps.reminder_minutes;
        let (at, delayed) = match delay {
            Some(mins) => {
                let target = start
                    .checked_add_signed(Duration::minutes(mins as i64))
                    .unwrap_or(start);
                let latest = end
                    .checked_sub_signed(Duration::minutes(5))
                    .unwrap_or(start);
                let at = if latest > start {
                    target.min(latest)
                } else {
                    start
                };
                (at, at > start)
            }
            None => (start, false),
        };
        events.push(PlannedEvent {
            kind: EventKind::Start,
            delayed,
            prayer,
            at,
            start,
            end,
        });

        // Final warning shortly before the window closes ("you will miss it").
        if settings.pre_end_warning_enabled && settings.pre_end_warning_minutes > 0 {
            if let Some(at) =
                end.checked_sub_signed(Duration::minutes(settings.pre_end_warning_minutes as i64))
            {
                if at > start + Duration::minutes(2) {
                    events.push(PlannedEvent {
                        kind: EventKind::PreEnd,
                        delayed: false,
                        prayer,
                        at,
                        start,
                        end,
                    });
                }
            }
        }
    }

    events
}

/// Unique id for an event within its Prayer Day, used to track "already fired".
pub fn event_id(date: NaiveDate, prayer: Prayer, kind: EventKind) -> String {
    format!("{date}|{}|{}", prayer.id(), kind.id())
}

/// What the engine should do with one planned event on this tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Due {
    /// Fire it now.
    Fire,
    /// Its time passed beyond the grace window and it never fired: mark it
    /// fired without ringing.
    SkipMissed,
    /// It was fired, but a settings change moved it back into the future:
    /// forget the fired mark so it can ring again.
    Rearm,
    /// Nothing to do.
    Wait,
}

pub fn due(at: DateTime<Tz>, now: DateTime<Tz>, already_fired: bool) -> Due {
    match (at <= now, already_fired) {
        (true, true) => Due::Wait,
        (true, false) if now - at <= Duration::seconds(GRACE_SECS) => Due::Fire,
        (true, false) => Due::SkipMissed,
        (false, true) => Due::Rearm,
        (false, false) => Due::Wait,
    }
}

/// The event the UI should feature: the open window of a prayer that isn't
/// done (`true` = current), else the earliest upcoming main alarm (`false`).
/// `None` means nothing is left in this Prayer Day.
pub fn featured<'a>(
    events: &'a [PlannedEvent],
    now: DateTime<Tz>,
    is_done: impl Fn(Prayer) -> bool,
) -> Option<(&'a PlannedEvent, bool)> {
    let mut upcoming: Option<&PlannedEvent> = None;
    for event in events {
        if is_done(event.prayer) {
            continue;
        }
        if now >= event.start && now < event.end {
            return Some((event, true));
        }
        if event.kind == EventKind::Start && event.at > now {
            let better = upcoming.map(|u| event.at < u.at).unwrap_or(true);
            if better {
                upcoming = Some(event);
            }
        }
    }
    upcoming.map(|e| (e, false))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::PrayerSettings;
    use chrono::TimeZone;
    use std::collections::HashMap;

    const DHAKA: Tz = chrono_tz::Asia::Dhaka;
    const LONDON: Tz = chrono_tz::Europe::London;

    fn t(hm: &str) -> NaiveTime {
        NaiveTime::parse_from_str(hm, "%H:%M").unwrap()
    }

    fn d(ymd: &str) -> NaiveDate {
        NaiveDate::parse_from_str(ymd, "%Y-%m-%d").unwrap()
    }

    /// [fajr, sunrise, dhuhr, asr, maghrib, isha, midnight]
    fn day(date: &str, hm: [&str; 7]) -> DayTimings {
        let times = HashMap::from([
            (Prayer::Fajr, t(hm[0])),
            (Prayer::Dhuhr, t(hm[2])),
            (Prayer::Asr, t(hm[3])),
            (Prayer::Maghrib, t(hm[4])),
            (Prayer::Isha, t(hm[5])),
        ]);
        DayTimings {
            date: d(date),
            readable: date.to_string(),
            hijri: String::new(),
            times,
            sunrise: t(hm[1]),
            midnight: t(hm[6]),
        }
    }

    /// A typical Dhaka day: Isha's window closes before midnight.
    fn dhaka_day(date: &str) -> DayTimings {
        day(
            date,
            ["05:00", "06:15", "11:50", "15:40", "17:20", "18:40", "23:15"],
        )
    }

    /// A London summer day: Isha's window closes after midnight.
    fn london_summer_day(date: &str) -> DayTimings {
        day(
            date,
            ["02:45", "04:45", "13:05", "17:25", "21:20", "22:55", "01:05"],
        )
    }

    fn at(tz: Tz, date: &str, hm: &str) -> DateTime<Tz> {
        tz.from_local_datetime(&d(date).and_time(t(hm))).unwrap()
    }

    fn no_reminders() -> Settings {
        let mut s = Settings::default();
        for p in Prayer::ALL {
            s.prayer_settings.get_mut(p).reminder_minutes = None;
        }
        s
    }

    fn find(events: &[PlannedEvent], prayer: Prayer, kind: EventKind) -> Option<&PlannedEvent> {
        events.iter().find(|e| e.prayer == prayer && e.kind == kind)
    }

    // -- end_time_of / window ------------------------------------------------

    #[test]
    fn windows_chain_into_each_other() {
        let day = dhaka_day("2026-11-09");
        assert_eq!(end_time_of(Prayer::Fajr, &day), t("06:15"));
        assert_eq!(end_time_of(Prayer::Dhuhr, &day), t("15:40"));
        assert_eq!(end_time_of(Prayer::Asr, &day), t("17:20"));
        assert_eq!(end_time_of(Prayer::Maghrib, &day), t("18:40"));
        assert_eq!(end_time_of(Prayer::Isha, &day), t("23:15"));
    }

    #[test]
    fn isha_window_crosses_midnight() {
        let day = london_summer_day("2026-06-20");
        let (start, end) = window(Prayer::Isha, &day, LONDON).unwrap();
        assert_eq!(start, at(LONDON, "2026-06-20", "22:55"));
        assert_eq!(end, at(LONDON, "2026-06-21", "01:05"));
    }

    #[test]
    fn missing_prayer_has_no_window() {
        let mut day = dhaka_day("2026-11-09");
        day.times.remove(&Prayer::Asr);
        assert!(window(Prayer::Asr, &day, DHAKA).is_none());
        // Dhuhr then ends at Maghrib instead.
        assert_eq!(end_time_of(Prayer::Dhuhr, &day), t("17:20"));
    }

    // -- prayer_day -----------------------------------------------------------

    #[test]
    fn prayer_day_is_calendar_day_normally() {
        let y = dhaka_day("2026-11-08");
        let now = at(DHAKA, "2026-11-09", "00:10");
        assert_eq!(prayer_day(now, DHAKA, Some(&y)), d("2026-11-09"));
    }

    #[test]
    fn prayer_day_stays_yesterday_while_isha_is_open() {
        let y = london_summer_day("2026-06-20");
        let now = at(LONDON, "2026-06-21", "00:30");
        assert_eq!(prayer_day(now, LONDON, Some(&y)), d("2026-06-20"));
    }

    #[test]
    fn prayer_day_rolls_when_isha_closes() {
        let y = london_summer_day("2026-06-20");
        let now = at(LONDON, "2026-06-21", "01:05");
        assert_eq!(prayer_day(now, LONDON, Some(&y)), d("2026-06-21"));
    }

    #[test]
    fn prayer_day_ignores_timings_that_are_not_yesterday() {
        let stale = london_summer_day("2026-06-19");
        let now = at(LONDON, "2026-06-21", "00:30");
        assert_eq!(prayer_day(now, LONDON, Some(&stale)), d("2026-06-21"));
        assert_eq!(prayer_day(now, LONDON, None), d("2026-06-21"));
    }

    // -- compute ----------------------------------------------------------------

    #[test]
    fn default_settings_plan_delayed_alarm_and_pre_end_warning() {
        let day = dhaka_day("2026-11-09");
        let events = compute(&day, DHAKA, &Settings::default());

        let dhuhr = find(&events, Prayer::Dhuhr, EventKind::Start).unwrap();
        assert_eq!(dhuhr.at, at(DHAKA, "2026-11-09", "12:20"));
        assert!(dhuhr.delayed);

        let pre_end = find(&events, Prayer::Dhuhr, EventKind::PreEnd).unwrap();
        assert_eq!(pre_end.at, at(DHAKA, "2026-11-09", "15:30"));

        // Maghrib has no delay by default.
        let maghrib = find(&events, Prayer::Maghrib, EventKind::Start).unwrap();
        assert_eq!(maghrib.at, at(DHAKA, "2026-11-09", "17:20"));
        assert!(!maghrib.delayed);
    }

    #[test]
    fn alarm_at_start_when_no_reminder() {
        let day = dhaka_day("2026-11-09");
        let events = compute(&day, DHAKA, &no_reminders());
        let fajr = find(&events, Prayer::Fajr, EventKind::Start).unwrap();
        assert_eq!(fajr.at, fajr.start);
        assert!(!fajr.delayed);
    }

    #[test]
    fn delay_is_clamped_to_five_minutes_before_end() {
        let day = dhaka_day("2026-11-09");
        let mut s = Settings::default();
        *s.prayer_settings.get_mut(Prayer::Fajr) = PrayerSettings {
            enabled: true,
            reminder_minutes: Some(500),
        };
        let events = compute(&day, DHAKA, &s);
        let fajr = find(&events, Prayer::Fajr, EventKind::Start).unwrap();
        assert_eq!(fajr.at, at(DHAKA, "2026-11-09", "06:10"));
        assert!(fajr.delayed);
    }

    #[test]
    fn disabled_prayer_has_no_events() {
        let day = dhaka_day("2026-11-09");
        let mut s = Settings::default();
        s.prayer_settings.get_mut(Prayer::Asr).enabled = false;
        let events = compute(&day, DHAKA, &s);
        assert!(events.iter().all(|e| e.prayer != Prayer::Asr));
    }

    #[test]
    fn pre_end_warning_can_be_turned_off() {
        let day = dhaka_day("2026-11-09");
        let mut s = Settings::default();
        s.pre_end_warning_enabled = false;
        let events = compute(&day, DHAKA, &s);
        assert!(events.iter().all(|e| e.kind == EventKind::Start));
    }

    #[test]
    fn pre_end_warning_skipped_when_too_close_to_start() {
        // A 10-minute Maghrib window with a 9-minute warning would ring
        // 1 minute after the start: dropped.
        let mut day = dhaka_day("2026-11-09");
        day.times.insert(Prayer::Isha, t("17:30"));
        let mut s = no_reminders();
        s.pre_end_warning_minutes = 9;
        let events = compute(&day, DHAKA, &s);
        assert!(find(&events, Prayer::Maghrib, EventKind::PreEnd).is_none());
    }

    #[test]
    fn isha_pre_end_warning_lands_after_midnight() {
        let day = london_summer_day("2026-06-20");
        let events = compute(&day, LONDON, &Settings::default());
        let warn = find(&events, Prayer::Isha, EventKind::PreEnd).unwrap();
        assert_eq!(warn.at, at(LONDON, "2026-06-21", "00:55"));
    }

    #[test]
    fn event_id_format() {
        assert_eq!(
            event_id(d("2026-10-17"), Prayer::Dhuhr, EventKind::PreEnd),
            "2026-10-17|dhuhr|preEnd"
        );
    }

    // -- due ---------------------------------------------------------------------

    #[test]
    fn due_decisions() {
        let ev = at(DHAKA, "2026-11-09", "12:00");
        let mins = |m| ev + Duration::minutes(m);
        assert_eq!(due(ev, mins(-1), false), Due::Wait);
        assert_eq!(due(ev, ev, false), Due::Fire);
        assert_eq!(due(ev, mins(10), false), Due::Fire);
        assert_eq!(due(ev, mins(11), false), Due::SkipMissed);
        assert_eq!(due(ev, mins(1), true), Due::Wait);
        assert_eq!(due(ev, mins(60), true), Due::Wait);
        assert_eq!(due(ev, mins(-5), true), Due::Rearm);
    }

    // -- featured ------------------------------------------------------------

    #[test]
    fn featured_prefers_open_window() {
        let day = dhaka_day("2026-11-09");
        let events = compute(&day, DHAKA, &Settings::default());
        let now = at(DHAKA, "2026-11-09", "12:00");
        let (e, current) = featured(&events, now, |_| false).unwrap();
        assert_eq!(e.prayer, Prayer::Dhuhr);
        assert!(current);
    }

    #[test]
    fn featured_skips_done_and_picks_next_start() {
        let day = dhaka_day("2026-11-09");
        let events = compute(&day, DHAKA, &Settings::default());
        let now = at(DHAKA, "2026-11-09", "12:00");
        let (e, current) = featured(&events, now, |p| p == Prayer::Dhuhr).unwrap();
        assert_eq!(e.prayer, Prayer::Asr);
        assert!(!current);
    }

    #[test]
    fn featured_between_windows_is_next_prayer() {
        // 06:30: Fajr closed at sunrise, Dhuhr not started yet.
        let day = dhaka_day("2026-11-09");
        let events = compute(&day, DHAKA, &Settings::default());
        let now = at(DHAKA, "2026-11-09", "06:30");
        let (e, current) = featured(&events, now, |_| false).unwrap();
        assert_eq!(e.prayer, Prayer::Dhuhr);
        assert!(!current);
    }

    #[test]
    fn featured_isha_after_midnight() {
        let day = london_summer_day("2026-06-20");
        let events = compute(&day, LONDON, &Settings::default());
        let now = at(LONDON, "2026-06-21", "00:30");
        let (e, current) = featured(&events, now, |_| false).unwrap();
        assert_eq!(e.prayer, Prayer::Isha);
        assert!(current);
    }

    #[test]
    fn featured_none_after_last_window() {
        let day = dhaka_day("2026-11-09");
        let events = compute(&day, DHAKA, &Settings::default());
        let now = at(DHAKA, "2026-11-09", "23:30");
        assert!(featured(&events, now, |_| false).is_none());
    }
}
