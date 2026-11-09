//! Prayer history: a permanent record of each prayer per Prayer Day (done
//! or missed), kept in SQLite (`history.db` next to `settings.json`). The
//! engine's in-memory Done marks stay authoritative for the current Prayer
//! Day; this is the record written alongside them.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use chrono::{DateTime, Datelike, NaiveDate, Utc};
use rusqlite::{params, Connection, OptionalExtension};

use crate::models::{
    DayStatusesDto, HistoryDayDto, HistoryEntry, HistorySource, HistoryStatus, Prayer,
    PrayerTotalsDto,
};

const SCHEMA_VERSION: i64 = 1;

pub struct HistoryStore {
    conn: Connection,
}

fn err(e: rusqlite::Error) -> String {
    format!("history database: {e}")
}

impl HistoryStore {
    pub fn open(path: &Path) -> Result<HistoryStore, String> {
        Self::init(Connection::open(path).map_err(err)?)
    }

    #[cfg(test)]
    pub fn open_in_memory() -> Result<HistoryStore, String> {
        Self::init(Connection::open_in_memory().map_err(err)?)
    }

    fn init(conn: Connection) -> Result<HistoryStore, String> {
        conn.busy_timeout(std::time::Duration::from_secs(2)).map_err(err)?;
        let version: i64 = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(err)?;
        // A database a newer app version migrated is left as it is; this
        // version can't read it safely (alarms are unaffected).
        if version > SCHEMA_VERSION {
            return Err(
                "History was saved by a newer version of Prayer Times; update the app to see it".into(),
            );
        }
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS prayer_log (
                day     TEXT NOT NULL,
                prayer  TEXT NOT NULL,
                status  TEXT NOT NULL,
                done_at TEXT,
                source  TEXT NOT NULL,
                PRIMARY KEY (day, prayer)
            );",
        )
        .map_err(err)?;
        if version < SCHEMA_VERSION {
            conn.pragma_update(None, "user_version", SCHEMA_VERSION)
                .map_err(err)?;
        }
        Ok(HistoryStore { conn })
    }

    #[cfg(test)]
    pub fn user_version(&self) -> Result<i64, String> {
        self.conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(err)
    }

    /// Writes one row; `status: None` deletes it ("no record").
    pub fn set(
        &self,
        day: NaiveDate,
        prayer: Prayer,
        status: Option<HistoryStatus>,
        done_at: Option<DateTime<Utc>>,
        source: HistorySource,
    ) -> Result<(), String> {
        let day = day.to_string();
        match status {
            None => self
                .conn
                .execute(
                    "DELETE FROM prayer_log WHERE day = ?1 AND prayer = ?2",
                    params![day, prayer.id()],
                )
                .map(|_| ())
                .map_err(err),
            Some(status) => self
                .conn
                .execute(
                    "INSERT INTO prayer_log (day, prayer, status, done_at, source)
                     VALUES (?1, ?2, ?3, ?4, ?5)
                     ON CONFLICT(day, prayer) DO UPDATE SET
                       status = excluded.status, done_at = excluded.done_at,
                       source = excluded.source",
                    params![
                        day,
                        prayer.id(),
                        status.id(),
                        done_at.map(|t| t.to_rfc3339()),
                        source.id()
                    ],
                )
                .map(|_| ())
                .map_err(err),
        }
    }

    /// Records `missed` unless the prayer already has a row (never
    /// overwrites a Done).
    pub fn record_missed(&self, day: NaiveDate, prayer: Prayer) -> Result<(), String> {
        self.conn
            .execute(
                "INSERT OR IGNORE INTO prayer_log (day, prayer, status, done_at, source)
                 VALUES (?1, ?2, 'missed', NULL, 'app')",
                params![day.to_string(), prayer.id()],
            )
            .map(|_| ())
            .map_err(err)
    }

    /// The Done marks recorded for `day`.
    pub fn done_on(&self, day: NaiveDate) -> Result<HashMap<Prayer, DateTime<Utc>>, String> {
        Ok(self
            .range(day, day)?
            .into_iter()
            .filter(|e| e.status == HistoryStatus::Done)
            // A Done without a time (edited or imported) still counts.
            .map(|e| (e.prayer, e.done_at.unwrap_or_else(Utc::now)))
            .collect())
    }

    /// Rows with `from <= day <= to`, ordered by day then prayer order.
    pub fn range(&self, from: NaiveDate, to: NaiveDate) -> Result<Vec<HistoryEntry>, String> {
        self.query(
            "SELECT day, prayer, status, done_at FROM prayer_log WHERE day >= ?1 AND day <= ?2",
            params![from.to_string(), to.to_string()],
        )
    }

    pub fn all(&self) -> Result<Vec<HistoryEntry>, String> {
        self.query("SELECT day, prayer, status, done_at FROM prayer_log", params![])
    }

    /// One row, if recorded.
    pub fn get(&self, day: NaiveDate, prayer: Prayer) -> Result<Option<HistoryEntry>, String> {
        self.conn
            .query_row(
                "SELECT day, prayer, status, done_at FROM prayer_log WHERE day = ?1 AND prayer = ?2",
                params![day.to_string(), prayer.id()],
                |row| Ok(parse_row(row)),
            )
            .optional()
            .map(Option::flatten)
            .map_err(err)
    }

    /// Merges imported rows (`merge_entry` decides each one) in a single
    /// transaction. Returns how many rows changed.
    pub fn merge(&self, incoming: &[HistoryEntry]) -> Result<usize, String> {
        let tx = self.conn.unchecked_transaction().map_err(err)?;
        let mut changed = 0;
        for row in incoming {
            let existing = self.get(row.day, row.prayer)?;
            if let Some(new) = merge_entry(existing.as_ref(), row) {
                self.set(new.day, new.prayer, Some(new.status), new.done_at, HistorySource::Imported)?;
                changed += 1;
            }
        }
        tx.commit().map_err(err)?;
        Ok(changed)
    }

    fn query(&self, sql: &str, args: impl rusqlite::Params) -> Result<Vec<HistoryEntry>, String> {
        let mut stmt = self.conn.prepare(sql).map_err(err)?;
        // Unparsable rows (a newer version's values) are skipped, not fatal.
        let mut rows: Vec<HistoryEntry> = stmt
            .query_map(args, |row| Ok(parse_row(row)))
            .map_err(err)?
            .filter_map(|r| r.ok().flatten())
            .collect();
        rows.sort_by_key(|e| (e.day, Prayer::ALL.iter().position(|p| *p == e.prayer)));
        Ok(rows)
    }
}

// ---------------------------------------------------------------------------
// Pure decisions (unit-tested below)
// ---------------------------------------------------------------------------

/// Import merge for one (day, prayer): `Some(row)` when `incoming` should be
/// written. Done beats missed; between two Done marks the earlier one stays.
/// Never deletes.
pub fn merge_entry(existing: Option<&HistoryEntry>, incoming: &HistoryEntry) -> Option<HistoryEntry> {
    let Some(old) = existing else {
        return Some(incoming.clone());
    };
    let wins = match (old.status, incoming.status) {
        (HistoryStatus::Missed, HistoryStatus::Done) => true,
        (HistoryStatus::Done, HistoryStatus::Done) => match (old.done_at, incoming.done_at) {
            (Some(a), Some(b)) => b < a,
            (None, Some(_)) => true,
            _ => false,
        },
        _ => false,
    };
    wins.then(|| incoming.clone())
}

fn all_rows_done(s: &DayStatusesDto) -> bool {
    let rows: Vec<HistoryStatus> = Prayer::ALL.iter().filter_map(|p| s.get(*p)).collect();
    !rows.is_empty() && rows.iter().all(|r| *r == HistoryStatus::Done)
}

/// Consecutive Prayer Days, ending yesterday (or today when all five are
/// already Done), where at least one prayer is recorded and every recorded
/// prayer is Done. A missed prayer or a day without records breaks it.
pub fn streak(days: &BTreeMap<NaiveDate, DayStatusesDto>, today: NaiveDate) -> u32 {
    let today_complete = days
        .get(&today)
        .is_some_and(|s| Prayer::ALL.iter().all(|p| s.get(*p) == Some(HistoryStatus::Done)));
    let mut day = if today_complete { Some(today) } else { today.pred_opt() };
    let mut count = 0;
    while let Some(d) = day {
        if !days.get(&d).is_some_and(all_rows_done) {
            break;
        }
        count += 1;
        day = d.pred_opt();
    }
    count
}

/// Per prayer: Done count out of recorded days.
pub fn totals(days: &BTreeMap<NaiveDate, DayStatusesDto>) -> PrayerTotalsDto {
    let mut out = PrayerTotalsDto::default();
    for s in days.values() {
        for p in Prayer::ALL {
            if let Some(status) = s.get(p) {
                let t = out.get_mut(p);
                t.recorded += 1;
                if status == HistoryStatus::Done {
                    t.done += 1;
                }
            }
        }
    }
    out
}

/// Groups rows by day.
pub fn by_day(entries: &[HistoryEntry]) -> BTreeMap<NaiveDate, DayStatusesDto> {
    let mut out: BTreeMap<NaiveDate, DayStatusesDto> = BTreeMap::new();
    for e in entries {
        out.entry(e.day).or_default().set(e.prayer, Some(e.status));
    }
    out
}

/// Every calendar day of `year`-`month` with its recorded statuses; days
/// after `current_day` are not editable. Empty for an invalid month.
pub fn month_days(
    year: i32,
    month: u32,
    entries: &[HistoryEntry],
    current_day: NaiveDate,
) -> Vec<HistoryDayDto> {
    let grouped = by_day(entries);
    let Some(first) = NaiveDate::from_ymd_opt(year, month, 1) else {
        return Vec::new();
    };
    first
        .iter_days()
        .take_while(|d| d.month() == month)
        .map(|d| HistoryDayDto {
            day: d.to_string(),
            prayers: grouped.get(&d).cloned().unwrap_or_default(),
            editable: d <= current_day,
        })
        .collect()
}

/// Where a prayer's window stands now (only consulted for today).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowState {
    pub started: bool,
    pub closed: bool,
}

/// Whether the History tab may set `day`'s prayer to `status`. Future days
/// are refused. For today: "done" only once the prayer has started, and
/// "missed" only once its time has ended (it isn't decided before).
pub fn edit_allowed(
    day: NaiveDate,
    status: Option<HistoryStatus>,
    current_day: NaiveDate,
    window: WindowState,
) -> Result<(), String> {
    if day > current_day {
        return Err("Future days can't be edited".into());
    }
    if day == current_day {
        match status {
            Some(HistoryStatus::Done) if !window.started => {
                return Err("This prayer hasn't started yet".into());
            }
            Some(HistoryStatus::Missed) if !window.closed => {
                return Err("This prayer's time is still open".into());
            }
            _ => {}
        }
    }
    Ok(())
}

fn parse_row(row: &rusqlite::Row<'_>) -> Option<HistoryEntry> {
    let day: String = row.get(0).ok()?;
    let prayer: String = row.get(1).ok()?;
    let status: String = row.get(2).ok()?;
    let done_at: Option<String> = row.get(3).ok()?;
    Some(HistoryEntry {
        day: NaiveDate::parse_from_str(&day, "%Y-%m-%d").ok()?,
        prayer: Prayer::from_id(&prayer)?,
        status: HistoryStatus::from_id(&status)?,
        done_at: done_at
            .and_then(|t| DateTime::parse_from_rfc3339(&t).ok())
            .map(|t| t.with_timezone(&Utc)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    fn utc(s: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc)
    }

    #[test]
    fn set_then_done_on_round_trips() {
        let store = HistoryStore::open_in_memory().unwrap();
        let at = utc("2026-11-09T06:00:00Z");
        store
            .set(d("2026-11-09"), Prayer::Dhuhr, Some(HistoryStatus::Done), Some(at), HistorySource::App)
            .unwrap();
        let done = store.done_on(d("2026-11-09")).unwrap();
        assert_eq!(done.get(&Prayer::Dhuhr), Some(&at));
        assert_eq!(done.len(), 1);
    }

    #[test]
    fn set_none_deletes() {
        let store = HistoryStore::open_in_memory().unwrap();
        store
            .set(d("2026-11-09"), Prayer::Asr, Some(HistoryStatus::Missed), None, HistorySource::App)
            .unwrap();
        store.set(d("2026-11-09"), Prayer::Asr, None, None, HistorySource::Edited).unwrap();
        assert!(store.range(d("2026-11-01"), d("2026-11-30")).unwrap().is_empty());
    }

    #[test]
    fn record_missed_never_overwrites_done() {
        let store = HistoryStore::open_in_memory().unwrap();
        let at = utc("2026-11-09T10:00:00Z");
        store
            .set(d("2026-11-09"), Prayer::Asr, Some(HistoryStatus::Done), Some(at), HistorySource::App)
            .unwrap();
        store.record_missed(d("2026-11-09"), Prayer::Asr).unwrap();
        let rows = store.range(d("2026-11-09"), d("2026-11-09")).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].status, HistoryStatus::Done);
        assert_eq!(rows[0].done_at, Some(at));
    }

    #[test]
    fn range_is_inclusive_and_ordered() {
        let store = HistoryStore::open_in_memory().unwrap();
        for (day, prayer) in [
            ("2026-12-01", Prayer::Fajr),
            ("2026-11-30", Prayer::Isha),
            ("2026-11-30", Prayer::Fajr),
            ("2026-11-01", Prayer::Maghrib),
            ("2026-10-31", Prayer::Isha),
        ] {
            store.record_missed(d(day), prayer).unwrap();
        }
        let rows = store.range(d("2026-11-01"), d("2026-11-30")).unwrap();
        let got: Vec<(NaiveDate, Prayer)> = rows.iter().map(|r| (r.day, r.prayer)).collect();
        assert_eq!(
            got,
            vec![
                (d("2026-11-01"), Prayer::Maghrib),
                (d("2026-11-30"), Prayer::Fajr),
                (d("2026-11-30"), Prayer::Isha),
            ]
        );
    }

    #[test]
    fn open_sets_user_version_1() {
        let store = HistoryStore::open_in_memory().unwrap();
        assert_eq!(store.user_version().unwrap(), 1);
    }

    // -- pure decisions ----------------------------------------------------

    fn entry(day: &str, prayer: Prayer, status: HistoryStatus, done_at: Option<&str>) -> HistoryEntry {
        HistoryEntry { day: d(day), prayer, status, done_at: done_at.map(utc) }
    }

    #[test]
    fn done_beats_missed() {
        let old = entry("2026-11-09", Prayer::Asr, HistoryStatus::Missed, None);
        let new = entry("2026-11-09", Prayer::Asr, HistoryStatus::Done, Some("2026-11-09T10:00:00Z"));
        assert_eq!(merge_entry(Some(&old), &new), Some(new.clone()));
    }

    #[test]
    fn missed_does_not_replace_done() {
        let old = entry("2026-11-09", Prayer::Asr, HistoryStatus::Done, Some("2026-11-09T10:00:00Z"));
        let new = entry("2026-11-09", Prayer::Asr, HistoryStatus::Missed, None);
        assert_eq!(merge_entry(Some(&old), &new), None);
    }

    #[test]
    fn earlier_done_at_wins() {
        let early = entry("2026-11-09", Prayer::Asr, HistoryStatus::Done, Some("2026-11-09T10:00:00Z"));
        let late = entry("2026-11-09", Prayer::Asr, HistoryStatus::Done, Some("2026-11-09T11:00:00Z"));
        assert_eq!(merge_entry(Some(&late), &early), Some(early.clone()));
        assert_eq!(merge_entry(Some(&early), &late), None);
        assert_eq!(merge_entry(Some(&early), &early), None);
    }

    #[test]
    fn merge_into_nothing_writes() {
        let new = entry("2026-11-09", Prayer::Asr, HistoryStatus::Missed, None);
        assert_eq!(merge_entry(None, &new), Some(new.clone()));
    }

    fn all_done() -> DayStatusesDto {
        let mut s = DayStatusesDto::default();
        for p in Prayer::ALL {
            s.set(p, Some(HistoryStatus::Done));
        }
        s
    }

    fn days(list: &[(&str, DayStatusesDto)]) -> BTreeMap<NaiveDate, DayStatusesDto> {
        list.iter().map(|(day, s)| (d(day), s.clone())).collect()
    }

    #[test]
    fn streak_counts_back_from_yesterday() {
        let mut partial_today = DayStatusesDto::default();
        partial_today.set(Prayer::Fajr, Some(HistoryStatus::Done));
        let map = days(&[
            ("2026-11-07", all_done()),
            ("2026-11-08", all_done()),
            ("2026-11-09", partial_today),
        ]);
        assert_eq!(streak(&map, d("2026-11-09")), 2);
    }

    #[test]
    fn streak_includes_today_when_all_five_done() {
        let map = days(&[("2026-11-08", all_done()), ("2026-11-09", all_done())]);
        assert_eq!(streak(&map, d("2026-11-09")), 2);
    }

    #[test]
    fn no_record_day_breaks_streak() {
        // 11-07 missing entirely; a day with a missed prayer also breaks it.
        let mut with_missed = all_done();
        with_missed.set(Prayer::Asr, Some(HistoryStatus::Missed));
        let map = days(&[
            ("2026-11-05", all_done()),
            ("2026-11-06", all_done()),
            ("2026-11-08", all_done()),
        ]);
        assert_eq!(streak(&map, d("2026-11-09")), 1);
        let map = days(&[("2026-11-07", all_done()), ("2026-11-08", with_missed)]);
        assert_eq!(streak(&map, d("2026-11-09")), 0);
    }

    #[test]
    fn day_with_only_some_prayers_recorded_counts_when_all_rows_done() {
        // Alarms off for Isha: no row, the rest done.
        let mut four = all_done();
        four.set(Prayer::Isha, None);
        let map = days(&[("2026-11-08", four)]);
        assert_eq!(streak(&map, d("2026-11-09")), 1);
    }

    #[test]
    fn streak_across_month_boundary() {
        let map = days(&[("2026-10-31", all_done()), ("2026-11-01", all_done())]);
        assert_eq!(streak(&map, d("2026-11-02")), 2);
    }

    #[test]
    fn totals_count_done_over_recorded() {
        let mut a = all_done();
        a.set(Prayer::Fajr, Some(HistoryStatus::Missed));
        let mut b = DayStatusesDto::default();
        b.set(Prayer::Fajr, Some(HistoryStatus::Done));
        let t = totals(&days(&[("2026-11-01", a), ("2026-11-02", b)]));
        assert_eq!((t.fajr.done, t.fajr.recorded), (1, 2));
        assert_eq!((t.dhuhr.done, t.dhuhr.recorded), (1, 1));
    }

    fn win(started: bool, closed: bool) -> WindowState {
        WindowState { started, closed }
    }

    #[test]
    fn edit_rules() {
        let today = d("2026-11-09");
        let open = win(true, false);
        assert!(edit_allowed(d("2026-11-10"), Some(HistoryStatus::Done), today, win(true, true)).is_err());
        assert!(edit_allowed(today, Some(HistoryStatus::Missed), today, open).is_err());
        assert!(edit_allowed(today, Some(HistoryStatus::Missed), today, win(true, true)).is_ok());
        assert!(edit_allowed(today, Some(HistoryStatus::Done), today, open).is_ok());
        assert!(edit_allowed(today, None, today, open).is_ok());
        assert!(edit_allowed(d("2026-11-01"), Some(HistoryStatus::Missed), today, win(false, false)).is_ok());
    }

    #[test]
    fn today_cannot_be_marked_done_before_the_prayer_starts() {
        let today = d("2026-11-10");
        let e = edit_allowed(today, Some(HistoryStatus::Done), today, win(false, false)).unwrap_err();
        assert!(e.contains("hasn't started"), "{e}");
        assert!(edit_allowed(today, None, today, win(false, false)).is_ok());
    }

    #[test]
    fn month_days_cover_the_month_and_mark_future_days_not_editable() {
        let entries = vec![
            entry("2026-11-01", Prayer::Fajr, HistoryStatus::Done, Some("2026-11-01T00:00:00Z")),
            entry("2026-11-09", Prayer::Asr, HistoryStatus::Missed, None),
        ];
        let out = month_days(2026, 11, &entries, d("2026-11-09"));
        assert_eq!(out.len(), 30);
        assert_eq!(out[0].day, "2026-11-01");
        assert_eq!(out[0].prayers.fajr, Some(HistoryStatus::Done));
        assert!(out[0].editable);
        assert_eq!(out[8].prayers.asr, Some(HistoryStatus::Missed));
        assert!(out[8].editable);
        assert!(!out[9].editable, "11-10 is in the future");
        assert_eq!(out[29].day, "2026-11-30");
    }

    #[test]
    fn month_days_handles_february_and_bad_months() {
        assert_eq!(month_days(2028, 2, &[], d("2028-03-01")).len(), 29);
        assert!(month_days(2026, 13, &[], d("2026-11-09")).is_empty());
    }

    #[test]
    fn importing_twice_is_idempotent() {
        let store = HistoryStore::open_in_memory().unwrap();
        let rows = vec![
            entry("2026-11-01", Prayer::Fajr, HistoryStatus::Done, Some("2026-11-01T00:10:00Z")),
            entry("2026-11-01", Prayer::Asr, HistoryStatus::Missed, None),
            entry("2026-11-02", Prayer::Isha, HistoryStatus::Done, None),
        ];
        assert_eq!(store.merge(&rows).unwrap(), 3);
        assert_eq!(store.merge(&rows).unwrap(), 0);
        assert_eq!(store.all().unwrap().len(), 3);
    }

    #[test]
    fn import_upgrades_missed_to_done_but_never_deletes() {
        let store = HistoryStore::open_in_memory().unwrap();
        store.record_missed(d("2026-11-01"), Prayer::Asr).unwrap();
        store.record_missed(d("2026-11-03"), Prayer::Fajr).unwrap();
        let rows = vec![entry("2026-11-01", Prayer::Asr, HistoryStatus::Done, Some("2026-11-01T10:00:00Z"))];
        assert_eq!(store.merge(&rows).unwrap(), 1);
        let all = store.all().unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(store.get(d("2026-11-01"), Prayer::Asr).unwrap().unwrap().status, HistoryStatus::Done);
    }

    fn temp_db(name: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("prayer-history-{name}-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        path
    }

    #[test]
    fn reopening_keeps_rows() {
        let path = temp_db("reopen");
        HistoryStore::open(&path).unwrap().record_missed(d("2026-11-01"), Prayer::Asr).unwrap();
        let again = HistoryStore::open(&path).unwrap();
        assert_eq!(again.all().unwrap().len(), 1);
        assert_eq!(again.user_version().unwrap(), 1);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_newer_schema_is_refused_and_left_untouched() {
        let path = temp_db("newer");
        {
            let conn = Connection::open(&path).unwrap();
            conn.pragma_update(None, "user_version", 2).unwrap();
        }
        let e = HistoryStore::open(&path).err().expect("must refuse a newer schema");
        assert!(e.contains("newer version"), "{e}");
        let conn = Connection::open(&path).unwrap();
        let v: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0)).unwrap();
        assert_eq!(v, 2, "the newer version's stamp must not be reset");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn done_without_a_time_still_counts_as_done_today() {
        let store = HistoryStore::open_in_memory().unwrap();
        store
            .set(d("2026-11-09"), Prayer::Asr, Some(HistoryStatus::Done), None, HistorySource::Imported)
            .unwrap();
        assert!(store.done_on(d("2026-11-09")).unwrap().contains_key(&Prayer::Asr));
    }

    #[test]
    fn open_on_a_directory_fails_instead_of_panicking() {
        let dir = std::env::temp_dir();
        assert!(HistoryStore::open(&dir).is_err());
    }
}
