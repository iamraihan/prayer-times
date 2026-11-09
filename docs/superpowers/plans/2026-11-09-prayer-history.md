# Prayer History Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Keep a permanent per-day record of Done/Missed prayers in SQLite, show it in a History tab, and move it between devices with a JSON backup.

**Architecture:** A new `history.rs` owns a `HistoryStore` (rusqlite) and pure functions (streak, totals, merge). The engine keeps its in-memory `done` map as the authority for the current Prayer Day and mirrors every change into the store; a pure `schedule::missed_now` decides, each tick, which closed windows become `missed`. New commands expose a month view, past-day edits and backup export/import; a React History page renders them.

**Tech Stack:** Rust (Tauri 2, rusqlite `bundled`, chrono/chrono-tz, serde, ts-rs), React + TS (zustand, Tailwind), `@tauri-apps/plugin-dialog`.

**Spec:** `docs/superpowers/specs/2026-11-09-prayer-history-design.md`

## Global Constraints

- Days are **Prayer Days** (`schedule::prayer_day`) as `'YYYY-MM-DD'` in the **location's** timezone; never the device date.
- Every IPC type: `Serialize`/`Deserialize` + `TS`, `#[serde(rename_all = "camelCase")]`, `#[cfg_attr(test, derive(TS), ts(export))]`, re-exported from `src/lib/types.ts`; every command in the `Commands` table of `src/lib/ipc.ts` with camelCase keys matching Rust parameter names.
- Closed sets are enums (`HistoryStatus` = `done` | `missed`; prayer ids `fajr|dhuhr|asr|maghrib|isha`). "One value per prayer" is a struct with one field per prayer, never a map.
- Database file: `history.db` in `settings_io::config_dir`. Table and columns exactly as in the spec (`prayer_log(day, prayer, status, done_at, source)`, PK `(day, prayer)`), `PRAGMA user_version = 1`.
- `source` values: `app` | `edited` | `imported`.
- Missed grace = `schedule::GRACE_SECS` (10 min).
- A history failure must never stop alarms: store errors are logged and surfaced as `HistoryMonthDto.error`, never `panic`/`?` out of `tick`.
- Backup file: `{"format":"prayer-times-backup","version":1,"exportedAt",...,"settings",...,"history":[...]}`; default name `prayer-times-backup-YYYY-MM-DD.json`.
- Merge: `done` beats `missed`; two `done` keep the earlier `doneAt`; never delete.
- Before finishing: `cargo test` (in `src-tauri/`) and `pnpm build` pass; `src/lib/bindings/` committed.

## Review Focus

1. **Corrupt or unopenable `history.db`** (file is garbage / locked): app starts, alarms ring, History shows an error. Test in Task 3 (`engine_survives_unopenable_history` via `HistoryStore::open` on a directory path returning `Err`, engine field `None`).
2. **Import of a malformed or foreign JSON file** (wrong `format`, `version: 2`, truncated): rejected with a clear message, nothing changed. Test in Task 4 (`rejects_wrong_format_and_future_version`).
3. **Re-importing the same backup twice**: no duplicates, counts unchanged. Test in Task 4 (`importing_twice_is_idempotent`).
4. **Isha closing after 00:00 is recorded under the previous Prayer Day**, not the calendar date. Test in Task 2 (`missed_isha_after_midnight_belongs_to_previous_prayer_day`).
5. **Editing a future day or marking today's still-open prayer "missed"** is refused. Test in Task 3 (`set_history_entry_rejects_future_and_open_missed`), as a pure `history::edit_allowed`.

---

### Task 1: HistoryStore (SQLite) + types

**Files:**
- Modify: `src-tauri/Cargo.toml` (add `rusqlite = { version = "0.32", features = ["bundled"] }`)
- Modify: `src-tauri/src/models.rs` (new types)
- Create: `src-tauri/src/history.rs`; register `mod history;` in `src-tauri/src/lib.rs`
- Modify: `src/lib/types.ts` (re-exports)

**Interfaces:**
- Produces (models.rs, all ts-exported):
  - `enum HistoryStatus { Done, Missed }` (serde `"done"`/`"missed"`)
  - `enum HistorySource { App, Edited, Imported }` (not exported to TS; stored as text)
  - `struct DayStatusesDto { fajr, dhuhr, asr, maghrib, isha: Option<HistoryStatus> }` with `get(Prayer)` / `set(Prayer, Option<HistoryStatus>)`
  - `struct HistoryEntry { day: NaiveDate, prayer: Prayer, status: HistoryStatus, done_at: Option<DateTime<Utc>> }` (serde camelCase, `day` as `YYYY-MM-DD`; used in the backup file, not TS-exported)
- Produces (history.rs):
  - `HistoryStore::open(path: &Path) -> Result<HistoryStore, String>` (creates table, sets `user_version`)
  - `HistoryStore::open_in_memory() -> Result<HistoryStore, String>` (tests)
  - `fn set(&self, day: NaiveDate, prayer: Prayer, status: Option<HistoryStatus>, done_at: Option<DateTime<Utc>>, source: HistorySource) -> Result<(), String>` (`None` deletes)
  - `fn record_missed(&self, day: NaiveDate, prayer: Prayer) -> Result<(), String>` (`INSERT OR IGNORE`, never overwrites a `done`)
  - `fn done_on(&self, day: NaiveDate) -> Result<HashMap<Prayer, DateTime<Utc>>, String>`
  - `fn range(&self, from: NaiveDate, to: NaiveDate) -> Result<Vec<HistoryEntry>, String>` (inclusive)
  - `fn all(&self) -> Result<Vec<HistoryEntry>, String>`
  - `fn merge(&self, incoming: &[HistoryEntry]) -> Result<usize, String>` (returns rows changed; uses `merge_entry` from Task 2, `source = imported`, one transaction)

- [ ] **Step 1: Write failing tests** in `history.rs` `#[cfg(test)]`:
  - `set_then_done_on_round_trips`: set Dhuhr done at `2026-11-09T06:00:00Z` → `done_on(2026-11-09)` has Dhuhr with that instant.
  - `set_none_deletes`: after `set(.., None, ..)` `range` is empty.
  - `record_missed_never_overwrites_done`: set Asr done, `record_missed` Asr → status still `Done`.
  - `range_is_inclusive_and_ordered`: rows on 10-31, 11-01, 11-30, 12-01; `range(11-01, 11-30)` returns exactly the two November rows ordered by day, then prayer order (Fajr..Isha).
  - `open_sets_user_version_1`.
- [ ] **Step 2:** `cd src-tauri && cargo test history::` → FAIL (module/functions missing).
- [ ] **Step 3:** Implement types and `HistoryStore` (`rusqlite::Connection` inside; store `prayer` via `Prayer::id()`, parse back with a `Prayer::from_id(&str) -> Option<Prayer>` added to models.rs; skip unparsable rows).
- [ ] **Step 4:** `cargo test` → all PASS; bindings regenerate `HistoryStatus.ts`, `DayStatusesDto.ts`.
- [ ] **Step 5:** Commit `feat(history): add SQLite history store`.

### Task 2: Pure decisions (missed, streak, totals, merge, edit rules)

**Files:**
- Modify: `src-tauri/src/schedule.rs` (`missed_now` + tests)
- Modify: `src-tauri/src/history.rs` (pure fns + tests)

**Interfaces:**
- Consumes: `schedule::PrayerWindow`, `schedule::windows`, `HistoryEntry`, `DayStatusesDto`.
- Produces:
  - `schedule::missed_now(windows: &[PrayerWindow], now: DateTime<Tz>, is_done: impl Fn(Prayer) -> bool, already_logged: impl Fn(Prayer) -> bool) -> Vec<Prayer>` — prayers with `end <= now`, `now - end <= GRACE_SECS`, not done, not logged.
  - `history::merge_entry(existing: Option<&HistoryEntry>, incoming: &HistoryEntry) -> Option<HistoryEntry>` — `Some(new row)` when it should be written, `None` when existing wins.
  - `history::streak(days: &BTreeMap<NaiveDate, DayStatusesDto>, today: NaiveDate) -> u32` — per spec: count back from yesterday (or from today if all five of today are `Done`); a day counts when it has ≥1 row and every row is `Done`.
  - `history::totals(days: &BTreeMap<NaiveDate, DayStatusesDto>) -> PrayerTotalsDto` with `PrayerTotalDto { done: u32, recorded: u32 }` per prayer (struct per prayer, ts-exported in models.rs).
  - `history::edit_allowed(day: NaiveDate, prayer: Prayer, status: Option<HistoryStatus>, current_day: NaiveDate, window_closed: bool) -> Result<(), String>` — refuses `day > current_day`; refuses `Missed` for `day == current_day && !window_closed`.

- [ ] **Step 1: Write failing tests:**
  - schedule.rs: `missed_when_window_closes_while_running` (Dhaka fixture, Dhuhr end 15:40, now 15:45 → `[Dhuhr]`); `not_missed_after_grace` (now 15:51 → `[]`); `done_or_logged_not_missed`; `missed_isha_after_midnight_belongs_to_previous_prayer_day` (London 2026-06-20 timings, now 2026-06-21 01:08 → `[Isha]` from the 06-20 windows, and `prayer_day(now)` is 06-20); `dst_day_end_resolves` (London 2026-03-29, any window closing after 01:00 resolves and is reported once).
  - history.rs: `done_beats_missed`, `missed_does_not_replace_done`, `earlier_done_at_wins`, `streak_counts_back_from_yesterday`, `streak_includes_today_when_all_five_done`, `no_record_day_breaks_streak`, `streak_across_month_boundary` (10-31 → 11-01), `totals_count_done_over_recorded`, `edit_rules` (future refused; open-window missed refused; past ok).
- [ ] **Step 2:** `cargo test` → FAIL.
- [ ] **Step 3:** Implement the five functions.
- [ ] **Step 4:** `cargo test` → PASS.
- [ ] **Step 5:** Commit `feat(history): missed/streak/merge decisions`.

### Task 3: Engine wiring + history commands

**Files:**
- Modify: `src-tauri/src/engine.rs`, `src-tauri/src/commands.rs`, `src-tauri/src/lib.rs` (handler list), `src-tauri/src/models.rs` (DTOs)
- Modify: `src/lib/ipc.ts`, `src/lib/types.ts`, `src/lib/actions.ts`

**Interfaces:**
- Consumes: Task 1 store, Task 2 fns.
- Produces:
  - models.rs: `HistoryDayDto { day: String, prayers: DayStatusesDto, editable: bool }`, `HistoryMonthDto { year: i32, month: u32, today: String, days: Vec<HistoryDayDto>, streak: u32, totals: PrayerTotalsDto, error: Option<String> }` (one `HistoryDayDto` per calendar day of the month; future days `editable: false`, all `None`).
  - Engine fields: `history: Option<HistoryStore>`, `history_error: Option<String>`, `history_logged: HashSet<Prayer>` (cleared at rollover).
  - Engine methods: `history_month(&self, year: i32, month: u32) -> HistoryMonthDto`; `set_history_entry(&mut self, day: NaiveDate, prayer: Prayer, status: Option<HistoryStatus>) -> Result<(), String>`.
  - Commands: `get_history_month(year: i32, month: u32) -> HistoryMonthDto`; `set_history_entry(day: String, prayer: Prayer, status: Option<HistoryStatus>) -> Result<HistoryMonthDto, String>` (returns the month containing `day`).
  - ipc.ts: `get_history_month: [{ year: number; month: number }, HistoryMonth]`, `set_history_entry: [{ day: string; prayer: Prayer; status: HistoryStatus | null }, HistoryMonth]`.

Behaviour (spec "When rows are written" + "Which store is in charge"):
- `Engine::new`: `HistoryStore::open(cache_dir.join("history.db"))`; on `Err`, `history = None`, `history_error = Some(msg)`, `eprintln!`.
- `set_prayer_done`: after the in-memory change, mirror to the store (`Done` with `Utc::now()`, `source = app`; un-done → `None`) for `self.day`.
- `tick`, after events are processed: `schedule::missed_now(&schedule::windows(&day, tz, &settings), now, is_done, |p| history_logged.contains(&p))` → `record_missed(today, p)` + insert into `history_logged`.
- Rollover branch: after clearing, `self.done = store.done_on(today)` when the store is present (this also restores marks after `reset_schedule`).
- `set_history_entry`: `edit_allowed(...)` first; if `day == current Prayer Day` route through `set_prayer_done` (Done ↔ None); else `store.set(..., source = edited)`.

- [ ] **Step 1: Write failing tests** (engine needs an `AppHandle`, so test the pieces): in history.rs `month_dto_marks_future_days_not_editable` for a pure `history::month_days(year, month, entries: &[HistoryEntry], current_day: NaiveDate) -> Vec<HistoryDayDto>`; `set_history_entry_rejects_future_and_open_missed` via `edit_allowed`; `engine_survives_unopenable_history`: `HistoryStore::open(dir_path)` where `dir_path` is a directory returns `Err` (the engine's handling is the `match` above).
- [ ] **Step 2:** `cargo test` → FAIL.
- [ ] **Step 3:** Implement `month_days`, DTOs, engine fields/methods, commands; add both commands to `generate_handler!` and to `Commands` in ipc.ts with `api.getHistoryMonth` / `api.setHistoryEntry` wrappers.
- [ ] **Step 4:** `cargo test` → PASS; `pnpm build` → PASS.
- [ ] **Step 5:** Commit `feat(history): record done/missed from the engine`.

### Task 4: Backup export / preview / import

**Files:**
- Create: `src-tauri/src/backup.rs` (register in lib.rs)
- Modify: `src-tauri/src/commands.rs`, `src-tauri/src/engine.rs`, `src-tauri/src/models.rs`, `src-tauri/src/lib.rs`, `src/lib/ipc.ts`, `src/lib/types.ts`

**Interfaces:**
- Produces:
  - backup.rs: `struct BackupFile { format: String, version: u32, exported_at: String, settings: Settings, history: Vec<HistoryEntry> }` (serde camelCase); `const FORMAT: &str = "prayer-times-backup"`, `const VERSION: u32 = 1`; `fn to_json(settings: &Settings, history: Vec<HistoryEntry>, now: DateTime<Utc>) -> String`; `fn parse(text: &str) -> Result<BackupFile, String>` (rejects wrong `format` or `version > 1` with a message naming the problem); `fn preview(file: &BackupFile) -> BackupPreviewDto`.
  - models.rs: `BackupPreviewDto { entries: u32, days: u32, first_day: Option<String>, last_day: Option<String>, location: Option<String>, has_custom_tone: bool }` (ts-exported).
  - Engine: `export_backup(&self) -> Result<String, String>`; `import_backup(&mut self, file: BackupFile, include_settings: bool) -> Result<(), String>` — `store.merge(&file.history)`; if `include_settings`: drop `alarm_sound`, apply via existing `apply_settings` (it already keeps the device's own tone), and `save_location` when the location differs; then reload today's `done` from the store.
  - Commands: `export_backup(path: String) -> Result<(), String>`, `preview_backup(path: String) -> Result<BackupPreviewDto, String>`, `import_backup(path: String, include_settings: bool) -> Result<AppStateDto, String>`; matching ipc.ts entries (`{ path: string }`, `{ path: string; includeSettings: boolean }`).

- [ ] **Step 1: Write failing tests** in backup.rs: `round_trips_settings_and_history`; `rejects_wrong_format_and_future_version` (`"format":"other"` → Err mentioning "not a Prayer Times backup"; `"version":2` → Err mentioning "newer version"); `truncated_json_is_rejected`; `preview_counts_days_and_range`; in history.rs `importing_twice_is_idempotent` (merge same 3 entries twice → second call returns 0, `all().len() == 3`).
- [ ] **Step 2:** `cargo test` → FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** `cargo test` → PASS; `pnpm build` → PASS.
- [ ] **Step 5:** Commit `feat(history): backup export and import`.

### Task 5: History tab + Settings backup buttons

**Files:**
- Create: `src/pages/HistoryPage.tsx`
- Modify: `src/App.tsx` (`type Tab = "today" | "history" | "settings"`, tab button), `src/pages/SettingsPage.tsx` (General section: "Export backup…", "Import backup…"), `src/lib/actions.ts`

**Interfaces:**
- Consumes: `api.getHistoryMonth`, `api.setHistoryEntry`, `api.exportBackup`, `api.previewBackup`, `api.importBackup`; `save`/`open` from `@tauri-apps/plugin-dialog` (already permitted by `dialog:default`; verify `save` works, else add `dialog:allow-save` in `src-tauri/capabilities/default.json`).

Behaviour (spec "History tab"; plain UI, the user will polish later):
- Header: month name + ‹ › arrows; "Streak: N days"; per-prayer "Fajr 18/22".
- Grid: 7 columns, one cell per day, 5 dots in prayer order: done = emerald, missed = red-400/70, no record = slate outline; today ringed; future dimmed.
- Click an `editable` day → panel listing 5 prayers, each cycling no record → done → missed → no record (today: done ↔ no record only); each click calls `setHistoryEntry` and re-renders from the returned month.
- `error` set → red note above the grid; grid still renders.
- Export: `save({ defaultPath: "prayer-times-backup-YYYY-MM-DD.json" })` → `exportBackup`; toast on success/failure.
- Import: `open({ filters: [{ name: "Backup", extensions: ["json"] }] })` → `previewBackup` → inline confirm card ("312 days of history (2025-01-03 – 2026-11-08), settings for Dhaka" + "Also use these settings" checkbox, default checked when `!state.hasLocation`; note "custom alarm tone isn't included" when `hasCustomTone`) → `importBackup`.

- [ ] **Step 1:** Build page and buttons.
- [ ] **Step 2:** `pnpm build` → PASS.
- [ ] **Step 3:** Manual check in `pnpm tauri dev`: mark a prayer done → dot turns green; edit yesterday; export, then import the same file → counts unchanged.
- [ ] **Step 4:** Update `README.md` (History + backup section) and `ARCHITECTURE.md` (history.rs/backup.rs in the diagram; replace the "Prayer history" line under Extending). Commit `feat(history): history tab and backup UI`.
