//! Tour progress: which Tour Steps the user has seen (viewed or skipped),
//! kept in SQLite (`app.db`, beside `history.db`). Separate from history so
//! neither database's schema version ties the other, and not part of
//! backups, so restoring one never brings a Tour back.

use std::path::Path;

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};

use crate::models::{TourStep, TourStepStatus};

const SCHEMA_VERSION: i64 = 1;

pub struct TourStore {
    conn: Connection,
}

fn err(e: rusqlite::Error) -> String {
    format!("tour progress: {e}")
}

impl TourStore {
    pub fn open(path: &Path) -> Result<TourStore, String> {
        Self::init(Connection::open(path).map_err(err)?)
    }

    #[cfg(test)]
    pub fn open_in_memory() -> Result<TourStore, String> {
        Self::init(Connection::open_in_memory().map_err(err)?)
    }

    fn init(conn: Connection) -> Result<TourStore, String> {
        conn.busy_timeout(std::time::Duration::from_secs(2)).map_err(err)?;
        let version: i64 = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(err)?;
        if version > SCHEMA_VERSION {
            return Err("Tour progress was saved by a newer version of Prayer Times".into());
        }
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS tour_progress (
                step   TEXT PRIMARY KEY,
                status TEXT NOT NULL,
                at     TEXT NOT NULL
            );",
        )
        .map_err(err)?;
        if version < SCHEMA_VERSION {
            conn.pragma_update(None, "user_version", SCHEMA_VERSION)
                .map_err(err)?;
        }
        Ok(TourStore { conn })
    }

    /// Every recorded step. Ids this version doesn't know (written by a
    /// newer one) are skipped.
    pub fn seen(&self) -> Result<Vec<TourStep>, String> {
        let mut stmt = self.conn.prepare("SELECT step FROM tour_progress").map_err(err)?;
        let ids = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(err)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(err)?;
        Ok(ids.iter().filter_map(|id| TourStep::from_id(id)).collect())
    }

    /// Records `steps`; a step already recorded keeps its first record.
    pub fn mark(&self, steps: &[TourStep], status: TourStepStatus, now: DateTime<Utc>) -> Result<(), String> {
        let tx = self.conn.unchecked_transaction().map_err(err)?;
        let at = now.to_rfc3339();
        for step in steps {
            tx.execute(
                "INSERT OR IGNORE INTO tour_progress (step, status, at) VALUES (?1, ?2, ?3)",
                params![step.id(), status.id(), at],
            )
            .map_err(err)?;
        }
        tx.commit().map_err(err)
    }

    /// Forgets every step, so every Tour shows again.
    pub fn reset(&self) -> Result<(), String> {
        self.conn.execute("DELETE FROM tour_progress", []).map_err(err)?;
        Ok(())
    }
}

/// What the snapshot reports: `None` when progress can't be read, so the UI
/// shows no Tours rather than the same ones on every launch.
pub fn seen_tour_steps(store: Option<&TourStore>) -> Option<Vec<TourStep>> {
    match store?.seen() {
        Ok(steps) => Some(steps),
        Err(e) => {
            eprintln!("{e}");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use rusqlite::OptionalExtension;

    fn store() -> TourStore {
        TourStore::open_in_memory().unwrap()
    }

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 11, 11, 9, 0, 0).unwrap()
    }

    fn later() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 11, 11, 18, 30, 0).unwrap()
    }

    fn sorted(mut steps: Vec<TourStep>) -> Vec<TourStep> {
        steps.sort_by_key(|s| s.id());
        steps
    }

    impl TourStore {
        fn row(&self, step: &str) -> Option<(String, String)> {
            self.conn
                .query_row(
                    "SELECT status, at FROM tour_progress WHERE step = ?1",
                    [step],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()
                .unwrap()
        }
    }

    #[test]
    fn a_fresh_store_has_seen_nothing() {
        assert!(store().seen().unwrap().is_empty());
    }

    #[test]
    fn marked_steps_are_seen() {
        let s = store();
        s.mark(&[TourStep::TodayNextPrayer, TourStep::TodayTray], TourStepStatus::Viewed, now())
            .unwrap();
        assert_eq!(
            sorted(s.seen().unwrap()),
            sorted(vec![TourStep::TodayNextPrayer, TourStep::TodayTray])
        );
    }

    #[test]
    fn marking_again_keeps_the_first_record() {
        let s = store();
        s.mark(&[TourStep::HistoryBackup], TourStepStatus::Skipped, now()).unwrap();
        s.mark(&[TourStep::HistoryBackup], TourStepStatus::Viewed, later()).unwrap();
        assert_eq!(s.row("historyBackup"), Some(("skipped".into(), now().to_rfc3339())));
    }

    #[test]
    fn reset_forgets_every_step() {
        let s = store();
        s.mark(&[TourStep::SettingsAlarms, TourStep::HistoryCalendar], TourStepStatus::Viewed, now())
            .unwrap();
        s.reset().unwrap();
        assert!(s.seen().unwrap().is_empty());
    }

    #[test]
    fn an_unknown_step_id_is_ignored() {
        let s = store();
        s.conn
            .execute(
                "INSERT INTO tour_progress VALUES ('qiblaCompass', 'viewed', '2026-11-11T00:00:00+00:00')",
                [],
            )
            .unwrap();
        s.mark(&[TourStep::SettingsSound], TourStepStatus::Viewed, now()).unwrap();
        assert_eq!(s.seen().unwrap(), vec![TourStep::SettingsSound]);
    }

    #[test]
    fn a_newer_database_is_refused() {
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "user_version", 2).unwrap();
        assert!(TourStore::init(conn).is_err());
    }

    #[test]
    fn step_ids_round_trip_and_match_serde() {
        for s in TourStep::ALL {
            assert_eq!(TourStep::from_id(s.id()), Some(s));
            assert_eq!(serde_json::to_string(&s).unwrap(), format!("\"{}\"", s.id()));
        }
        for s in [TourStepStatus::Viewed, TourStepStatus::Skipped] {
            assert_eq!(serde_json::to_string(&s).unwrap(), format!("\"{}\"", s.id()));
        }
    }

    #[test]
    fn no_store_means_no_tours() {
        assert_eq!(seen_tour_steps(None), None);
    }
}
