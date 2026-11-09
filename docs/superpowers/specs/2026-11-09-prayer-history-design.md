# Prayer History — design

Date: 2026-11-09 · Status: draft, reviewed, awaiting approval

## Intent

Today the app forgets everything at each Prayer Day rollover: `runtime.json`
holds Done marks for the current Prayer Day only and is wiped when it
changes. Users want to look back ("did I pray all five this week?"), keep
their record when they change computer, and correct a day they forgot to
mark.

Success: a user opens a History tab and sees, at a glance, which prayers
they marked Done on each day; they can fix a past day; they can export one
file on the old computer and import it on the new one.

## Goals

- Keep a permanent record per Prayer Day and Prayer: **done** (with the
  instant it was marked) or **missed**.
- A **History** tab: month calendar, a streak, monthly totals.
- Edit a past day: tap a prayer to switch done ↔ missed.
- **Export / import** one file to move to another device.

## Non-goals

- Cloud sync or accounts.
- Qada (make-up prayer) tracking, notes, or optional prayers.
- Charts beyond the month view and totals.
- Reconstructing days the app never saw.

## Anti-goals (failure even if it "works")

- Showing **missed** for a day the app was not running. The user may have
  prayed; we don't know. That day must read as "no record", not "missed".
- An import that silently wipes or duplicates existing history.
- Any history failure (corrupt or locked database) breaking alarms. History
  is secondary: on error, alarms keep working and History shows an error.

## Constraints (from the project)

- Rust owns the data and logic; the UI renders DTOs (`ARCHITECTURE.md`).
  IPC types are generated with ts-rs (`CLAUDE.md`).
- Days are **Prayer Days** in the **location's timezone**
  (`schedule::prayer_day`), never the device's calendar date.
- Decision logic (what counts as missed, streaks, merge rules) goes in pure,
  unit-tested functions that take `now` as a parameter.
- The user approved SQLite for storage.

## Decided

### Storage

- SQLite via `rusqlite` with the `bundled` feature (no system library
  needed on Windows/macOS/Linux), file `history.db` in the app config dir
  next to `settings.json`.
- One table:

  ```sql
  CREATE TABLE prayer_log (
    day     TEXT NOT NULL,   -- Prayer Day, 'YYYY-MM-DD' (location date)
    prayer  TEXT NOT NULL,   -- 'fajr' | 'dhuhr' | 'asr' | 'maghrib' | 'isha'
    status  TEXT NOT NULL,   -- 'done' | 'missed'
    done_at TEXT,            -- RFC3339 instant when marked done, else NULL
    source  TEXT NOT NULL,   -- 'app' | 'edited' | 'imported'
    PRIMARY KEY (day, prayer)
  );
  ```

  Plus `PRAGMA user_version` for future migrations.
- No row = **no record** (the app didn't see that prayer, or its alarms
  were off).

### When rows are written

- **Mark done** (any surface: popup, banner, Today list, tray): upsert
  `done` for the current Prayer Day immediately.
- **Unmark** (Today list toggle): delete that row (back to "no record" until
  rollover decides).
- **Missed is recorded live, when the window closes while the app is
  running** (not at rollover). On each tick, for each enabled prayer of the
  current Prayer Day that is not Done: if its window end has passed and
  passed **less than 10 minutes ago** (the same grace as Missed Events,
  `schedule::GRACE_SECS`), write `missed`. If the end passed longer ago
  (app closed, laptop asleep), write nothing: that prayer stays "no record".
  This is a pure function `history::missed_now(windows, now, is_done,
  already_logged)` with tests (Isha closing after 00:00, sleep across the
  end, DST day).
- Because nothing waits for rollover, a location / method / timezone change
  (`reset_schedule`) needs no special handling: what already closed is
  already written.
- Days the app never ran get **no rows**.
- A prayer with alarms switched off gets no `missed` row; a Done mark on it
  is still recorded.

### Which store is in charge

- For the **current Prayer Day**, the engine's in-memory `done` map
  (persisted in `runtime.json`) stays authoritative: it drives alarms, the
  Today list and the tray, exactly as now. `history.db` is a record written
  alongside it.
- Editing **today** from the History tab goes through the engine's existing
  `set_prayer_done`, so it silences alarms like any Done mark. Only
  done ↔ not-done for today (missed isn't decided yet for open windows).
- **Future days** can't be edited.
- After `reset_schedule()` (location or method change) the engine reloads
  today's Done marks from `history.db`, so a settings change doesn't lose
  them.
- Any database error is logged and shown on the History tab; the engine
  keeps running and alarms are unaffected.

### History tab

- A third tab, "History", next to Today and Settings.
- **Month calendar**: one cell per day, five small dots in prayer order
  (green = done, red/muted = missed, empty outline = no record). Today is
  highlighted; future days blank. Previous/next month arrows.
- **Tap a day** → that day's five prayers with a toggle each (done ↔
  missed; "no record" can become done or missed). Edits set
  `source = 'edited'`.
- **Streak**: consecutive Prayer Days, ending yesterday (or today if all
  five are already done), where every prayer that has a row is done and
  there is at least one row. A missed prayer breaks it; a no-record day
  breaks it.
- **This month**: done count out of recorded per prayer
  (e.g. "Fajr 18/22").

### Export / import

- Export writes one JSON file (human readable, versioned), chosen with the
  existing dialog plugin, default name `prayer-times-backup-YYYY-MM-DD.json`:

  ```json
  { "format": "prayer-times-backup", "version": 1,
    "exportedAt": "…", "settings": { … }, "history": [ {day, prayer, status, doneAt} ] }
  ```
- Import reads the file, shows a summary first ("312 days of history,
  settings for Dhaka"), then:
  - **History merges** by (day, prayer). Rule: `done` beats `missed`;
    between two `done`, keep the earlier `doneAt`. Nothing is deleted.
  - **Settings**: a checkbox "Also use these settings (location, alarms)",
    on by default when this device has no location yet, off otherwise.
- Settings > General gets "Export backup…" and "Import backup…".
- The custom alarm tone file is **not** in the backup (it can be large);
  imported settings drop `alarmSound` and use the default chime, and the
  summary says so.
- Applying imported settings goes through the same path as saving Settings
  (`update_settings` / `save_location`); after it, today's Done marks are
  reloaded from the merged history (see above).

### IPC (new commands, all in `ipc.ts`'s `Commands` table)

- `get_history_month(year, month) -> HistoryMonthDto` (days with five
  statuses each, streak, per-prayer totals).
- `set_history_entry(day, prayer, status: Option<HistoryStatus>)`.
- `export_backup(path) -> ()`, `preview_backup(path) -> BackupPreviewDto`,
  `import_backup(path, include_settings: bool) -> AppStateDto`.
- `HistoryStatus` is an enum (`done` | `missed`), not a string.

### Code layout

- `src-tauri/src/history.rs`: SQLite access (open/migrate, upsert, query
  range, merge) behind a small struct; tests run on an in-memory database.
- Pure logic (missed-at-rollover selection, streak, totals, merge rule) as
  plain functions in `history.rs` with unit tests, including: Isha past
  midnight (Prayer Day ≠ calendar date), a location timezone different from
  the device's, the 1st of a month, a DST change day.
- `src/pages/HistoryPage.tsx` plus small components.

## Left to the builder

- Calendar visuals within the existing slate/emerald style.
- Exact DTO field names and error messages.
- Whether the engine holds the connection or opens one per command (it must
  never block the alarm tick for long).
- Indexes (the primary key is enough at this size: about 1,800 rows/year).
