# Architecture

The app is a two-process Tauri 2 desktop app: a web frontend (two windows)
and a Rust backend that owns all scheduling, so alarms fire even when the
windows are hidden.

```
┌────────────────────────── Frontend (TS/React) ──────────────────────────┐
│  main window (index.html)                    alarm window (alarm.html) │
│  SetupWizard / TodayPage / SettingsPage      AlarmWindow (popup)       │
│        └──────────────┬──────────────────────────┘                     │
│                 useAppStore (zustand)  ←—— "app-state" events ──┐      │
│                 lib/actions.ts  ─── invoke() ─────────┐          │      │
└───────────────────────────────────────────────────────┼──────────┼──────┘
                                                        ▼          │
┌────────────────────────── Rust backend ───────────────────────┐   │
│  commands.rs  (IPC: get_state, set_prayer_done, alarm_*, …)   │   │
│      │ mutate + wake                                         │   │
│      ▼                                                       │   │
│  SharedEngine { Mutex<Engine>, wake: Condvar }                │   │
│      ▲ locked by commands (short) and by ──┐                 │   │
│      │                                     │                 │   │
│  engine.rs  run_loop thread ───────────────┘                 │   │
│    every 15s (or woken):                                     │   │
│      plan_fetches → fetch month(s) OUTSIDE the lock           │   │
│      tick(): compute events → fire due ones ──────────── sound.rs     │
│                update tray ────────────────────────────► tray.rs ──┘  │
│                emit "app-state" snapshot                       │      │
│                                                               │      │
│  schedule.rs  pure: DayTimings + Settings → [PlannedEvent]     │      │
│  aladhan.rs   API client + month disk cache                    │      │
│  alarm.rs     notification + always-on-top popup window        │      │
│  models.rs    domain + DTOs (camelCase on the wire)            │      │
│  settings.rs / runtime.rs   JSON persistence                   │      │
│  timeutil.rs  location-timezone-aware datetime helpers         │      │
└────────────────────────────────────────────────────────────────┘
```

## Data flow

1. **Startup** — `Engine::new` loads `settings.json` + `runtime.json`,
   builds the tray and (hidden) alarm window, spawns the engine thread.
2. **Month load** — `plan_fetches` wants today's and tomorrow's months:
   memory → disk cache → network (throttled to one retry/minute). Months
   are cached per `(method, school, year, month)`.
3. **Every tick (15 s)** — `schedule::compute` derives the day's events;
   due events (within a 10-minute grace for missed ones) fire; the tray and
   both windows get a fresh snapshot.
4. **User actions** — invoke a command → engine mutates under a short lock
   → the command returns the new snapshot → the store applies it
   immediately (no waiting for the next tick).

## Event lifecycle (per prayer, per day)

`id = "{date}|{prayer}|{start|reminder|preEnd}"`

- "day" here is the **Prayer Day** (`schedule::prayer_day`): the calendar
  date, except while yesterday's Isha window is still open past midnight.
- `fired` (persisted in `runtime.json`) prevents replays after restart;
  on restart, events older than the grace window are marked fired silently.
- `done` marks suppress all remaining events of that prayer for the day and
   clear any active alarm/snooze.
- Changing settings re-derives events; an event whose new time lies in the
   future is allowed to fire again (removed from `fired`).
- Snoozes live in `runtime.snoozes` as one-shot reminders (prayer → due
  instant) and survive restarts.

## Frontend structure

```
src/
  main.tsx / App.tsx          window shell: splash → wizard → tabs
  alarm/                      second window's entry + popup UI
  pages/                      TodayPage, SettingsPage
  components/                 feature components + ui/ primitives
  store/useAppStore.ts        single zustand store (state + 1s clock)
  lib/                        bindings/ (generated from models.rs), types,
                               typed ipc wrappers,
                               actions, time format helpers, static data
```

Rules of thumb:

- **One store, backend as source of truth.** UI never computes schedule
  logic; it renders the snapshot and sends intents via `lib/actions.ts`.
- **IPC types are generated, never hand-written**: every Rust type that
  crosses IPC derives `ts_rs::TS`; `cargo test` writes its TypeScript twin
  to `src/lib/bindings/` (commit the result — CI fails on a stale diff).
  `src/lib/types.ts` only re-exports them; `src/lib/ipc.ts` maps each command
  name to its argument and result types.

## Extending

- **New page (e.g. Qibla, history)**: add to `pages/`, add a tab in
  `App.tsx`'s `MainLayout`. For data: add a Rust command in `commands.rs`,
  a typed wrapper in `lib/ipc.ts`, call it from the page.
- **New alarm kind** (e.g. sunrise reminder): extend `EventKind` in
  `schedule.rs` + the message builder in `engine.rs`; nothing else changes.
- **Prayer history**: `runtime.rs` already keys done marks per day — swap
  the single `runtime.json` object for an append-only history file and add
  a summary command.
- **Autostart on login**: add `tauri-plugin-autostart`, init in `lib.rs`,
  toggle in Settings.
- **Unit tests**: run `cargo test` in `src-tauri/` (CI runs it on all three
  OSes). Scheduling decisions live in `schedule.rs` as pure functions that
  take `now` as a parameter (`compute`, `window`, `prayer_day`, `due`,
  `featured`) and are tested there; `timeutil.rs` and `aladhan.rs` parsing
  have tests too. Put new decision logic in `schedule.rs`, not `engine.rs`,
  so it can be tested without a running app.

## Deliberate decisions

- **Tick loop, not `sleep_until` tasks**: alarms survive system
  sleep/wake and clock changes; missed events fire late within grace.
- **Wall-clock math in the location's IANA zone** (from the API), never the
  device zone.
- **Per-day state follows the Prayer Day, not the calendar day** — see
  [ADR 0001](docs/adr/0001-prayer-day-rollover.md).
- **Month-granularity caching** keeps the app fully offline for the current
  month and costs one request per month online.
- **Fetches happen only on the engine thread, outside the state lock**, so
  IPC commands never block on the network.
- **Close-to-tray**: closing the main window hides it; the process keeps
  running so alarms fire (Quit lives in the tray menu).
