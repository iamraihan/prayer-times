# Prayer Times

Tauri 2 desktop app: a Rust backend owns all scheduling, and a React/TS frontend renders the snapshots it pushes. Read `ARCHITECTURE.md` before changing the engine, the IPC or the data flow. Use the terms defined in `GLOSSARY.md` (Prayer Window, Prayer Day, Main Alarm, Before-End Warning…) in code, comments and docs. `docs/adr/` explains decisions that look odd at first glance; read the relevant ADR before "fixing" one.

## Type safety

Rust is the single source of truth for every shape that crosses IPC.

- **Changing an IPC type:** edit the Rust struct or enum in `src-tauri/src/models.rs` (it derives `ts_rs::TS` with `#[ts(export)]`), then run `cargo test` in `src-tauri/`. That regenerates `src/lib/bindings/`; commit those files with the change. CI fails when they are stale. Treat `src/lib/bindings/` as read-only output.
- **New IPC type:** derive `Serialize`/`Deserialize` + `TS`, add `#[serde(rename_all = "camelCase")]` and `#[ts(export)]`, and re-export it from `src/lib/types.ts` under its frontend name.
- **New or changed command:** add it to the `Commands` table in `src/lib/ipc.ts`. The argument object's keys must match the Rust parameter names in camelCase; that part isn't checked by any tool, so verify it by hand.
- **A closed set of values** (a format, a kind, a school) is an enum on both sides, never a free `String`/`u8`. Model "one value per prayer" as a struct with one field per prayer (see `PrayerSettingsMap`), not a `HashMap<Prayer, _>`, so every prayer is always present.
- **Persisted types** (`Settings`, `runtime.json`) must load files written by older versions: give new fields a default (`Settings` has `#[serde(default)]`), and make enum deserializers fall back to a default instead of failing (see `AsrSchool`, `TimeFormat`). A failed load resets every setting, including the user's location.
- **Times:** do all date and time math in Rust, in the location's IANA timezone (`timeutil`). Send instants to the UI as RFC3339 strings that carry the location's offset. The UI compares instants (`Date.parse`) and reads the displayed `HH:MM` straight from those strings (or uses `parseHM`), never from the device clock's timezone.
- **TypeScript runs with the strictest flags** (`noUncheckedIndexedAccess`, `exactOptionalPropertyTypes`, …). Handle the `undefined` they surface; make optional props `prop?: T | undefined`.

## Testing

- Scheduling decisions live in `src-tauri/src/schedule.rs` as pure functions that take `now` as a parameter, unit-tested in the same file. Put new decision logic there and keep `engine.rs` to applying decisions and side effects (sound, tray, windows, disk).
- Every bug fix in time or schedule logic starts with a failing test that reproduces it: a DST gap or overlap, Isha past midnight, the 1st of a month, a location timezone different from the device's.
- Before finishing: `cargo test` (in `src-tauri/`) and `pnpm build` (runs `tsc`) both pass, and `src/lib/bindings/` has no uncommitted diff.
