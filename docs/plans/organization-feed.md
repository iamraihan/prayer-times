# Plan: follow an Organization's jamaat times

Status: implemented on this branch, 2026-11-26. Changed during implementation: no `ETag`; the feed is fetched on Follow/Save, then daily at 13:00 Location Time (`refresh_due`); a prayer set to `null` is not covered; `own_time_source` lives on `PrayerTimeDto`, not `PrayerWindow`. Branch `feat/organization-feed`. Design and sources: [`docs/research/organization-prayer-times.md`](../research/organization-prayer-times.md) (§4–§6 are the decided design).

## Goal

A user pastes an Organization's feed URL (plus an optional access key) and follows it. For every prayer the feed lists, the Own Prayer Time becomes the Organization's time and can't be edited. The other prayers behave as today. The Prayer Window, sunrise and midnight always come from Aladhan. The app polls the feed (no webhook), at most about once a day, and keeps the last good copy when something goes wrong. Leaving the Organization restores the user's own settings.

Out of scope: Jumu'ah, signed feeds, deep links, the OS keychain, start-time overrides.

## Feed (wire format, version 1)

```json
{
  "name": "Baitul Aman Masjid",
  "timezone": "Asia/Dhaka",
  "updatedAt": "2026-11-26T10:00:00+06:00",
  "prayers": { "fajr": "05:30", "dhuhr": "13:15", "asr": "16:00" }
}
```

`prayers` holds 1–5 of `fajr`, `dhuhr`, `asr`, `maghrib`, `isha` as `HH:MM` in Location Time. Any other key under `prayers` is an error. `timezone` must equal the Location's. `updatedAt` is optional.

## Module design (codebase-design)

Four modules, each deep: a small interface over most of the feature's rules.

| Module | Interface | What it hides |
|---|---|---|
| `organization::parse_feed` (new file `src-tauri/src/organization.rs`) | `parse_feed(json: &str, location_tz: &str) -> Result<OrganizationFeed, FeedError>` | JSON shape, `HH:MM` parsing, unknown-prayer hints (`zuhr` → `dhuhr`), timezone check, "at least one prayer" |
| `organization::fetch` | `fetch(client, link: &OrganizationLink, etag: Option<&str>) -> Result<FetchOutcome, FetchError>`, where `FetchOutcome = NotModified \| Fresh { body, etag }` | https-only check, `Authorization: Bearer`, `If-None-Match`, `304`, `401`/`403` → `FetchError::AccessKeyRejected`, 1 MiB cap, timeout |
| `organization::OrganizationState` | `refresh_due(now) -> bool`, `record(result, location_tz, now)`, `prayers() -> Option<&OrganizationPrayers>`, `status() -> OrganizationStatusDto`; serde for `cache/organization.json` | Refresh cadence (24 h after success, 15 min retry after failure), keep last good feed, which warning to show and when (unreachable for over 24 h, rejected document, access key rejected) |
| `schedule::own_prayer_time` | `own_prayer_time(prayer, start, end, date, tz, after: Option<u32>, organization: Option<&OrganizationPrayers>) -> OwnPrayerTime { at, source }`, with `source: OwnTimeSource = User \| Organization \| OrganizationIgnored` | Organization override vs `pray_time` offset, the out-of-window and last-5-minutes rule (ignore, don't clamp), DST gap and overlap, Isha congregation after 00:00 |

`PrayerWindow` gains `pray_at` and `source`, and `schedule::windows` / `compute` take `organization: Option<&OrganizationPrayers>`. Both callers that compute an Own Prayer Time today (`schedule.rs:167` and the snooze path at `engine.rs:786`) then read `w.pray_at`, so one function decides it.

The engine stays thin. It holds an `OrganizationState`, plans one fetch job under the lock when `refresh_due` says so, runs `fetch` outside the lock (as `run_loop` does for months, `engine.rs:1290-1335`), and calls `record`. It never calls `reset_schedule` for Organization changes.

## Seams under test (tdd: to confirm before the first test)

| # | Seam | Catches | Misses |
|---|---|---|---|
| S1 | `parse_feed` | Typos, wrong timezone, bad times, empty feed, unknown fields outside `prayers` ignored | Network and HTTP behaviour |
| S2 | `own_prayer_time` plus one `compute` test | Locking, partial coverage, leaving restores `prayAfterMinutes`, out-of-window ignored, DST, Isha after 00:00, the Main Alarm and Get-Ready Alarm moving | Whether the engine passes the right feed in |
| S3 | `OrganizationState` | Cadence, keep-last-good, warnings, `NotModified` refreshing the timestamp, a feed for another timezone after the Location changed | The real HTTP client |
| S4 | `fetch` against an in-process `std::net::TcpListener` (offline, runs in CI) | Header sent, `If-None-Match`, `304`, `401`, oversize body, `http://` refused | TLS and real hosts: one `#[ignore = "network"]` test covers that |

No tests on the engine or commands (they need an `AppHandle`), and none on the frontend beyond `tsc`. Their logic stays in S1–S4.

## Slices (vertical, red → green, one at a time)

Each slice: write one failing test, write the minimum code to pass it, repeat for the slice's next test. At the end, `cargo test` (in `src-tauri/`) and `pnpm build` pass and the regenerated bindings are part of the commit. Commits are local only. The user pushes.

### Slice 1: parse the feed (S1)

1. Valid 3-prayer feed → `OrganizationPrayers { fajr: Some(05:30), dhuhr: Some(13:15), asr: Some(16:00), maghrib: None, isha: None }`.
2. `"zuhr"` → `FeedError::UnknownPrayer { key: "zuhr", hint: Some("dhuhr") }`. Same for `fazr` and `johor`.
3. `timezone` ≠ the Location's → `FeedError::TimezoneMismatch`.
4. `"5:3"`, `"25:00"` → `FeedError::BadTime { prayer }`.
5. `"prayers": {}` → `FeedError::NoPrayers`.
6. Extra top-level key (`"jumuah"`, `"logo"`) → accepted and ignored.

Model (`models.rs`):
- `OrganizationPrayers`: one `Option<NaiveTime>` field per prayer, `deny_unknown_fields` on this struct only.
- `OrganizationFeed`: `name`, `timezone`, `updated_at: Option<DateTime<FixedOffset>>`, `prayers`.
- `OrganizationLink`: `feed_url`, `access_key: Option<String>`, IPC.
- `Settings.organization: Option<OrganizationLink>`, covered by the existing `serde(default)`.

Add one `models.rs` test: a settings file written by v0.5.1 (no `organization`) still loads with its location intact.

### Slice 2: Own Prayer Time from the Organization (S2)

Test location: London (DST) plus one non-UTC zone such as `Asia/Dhaka`, reusing the helpers in `schedule.rs` tests.

1. Asr covered at 16:00 → `at` = 16:00 that date, `source = Organization`, the user's `prayAfterMinutes = 30` ignored.
2. Maghrib not covered → `pray_time(start, end, Some(30))`, `source = User`.
3. No Organization (left) → identical to today's `pray_time` for all five prayers.
4. Organization Asr before Asr's window start, or inside the last 5 minutes of the window → user offset used, `source = OrganizationIgnored`.
5. Organization time in a DST-gap minute (London, last Sunday of March) → `OrganizationIgnored`. In the overlap (last Sunday of October) → first instant.
6. Isha congregation `"00:30"` with Isha starting 22:40 and midnight 00:55 → next calendar date 00:30, `Organization`.
7. The 1st of a month → the override still resolves on that date (guards against month-keyed lookups).
8. `compute` with Asr covered → the Main Alarm and Get-Ready Alarm move to 16:00, the Before-End Warning doesn't move.

Then switch `windows`, `compute` and the snooze path (`engine.rs:786`) to `w.pray_at`. Existing tests must stay green with `organization: None`.

### Slice 3: refresh and failure policy (S3)

1. Never fetched → `refresh_due` is true.
2. Success at T → not due at T + 23 h, due at T + 24 h.
3. Failure → not due for 15 min, due after that.
4. `Fresh` with a valid body → `prayers()` returns it, status warning cleared.
5. `Fresh` with an invalid body → previous `prayers()` kept, warning names the error.
6. Unreachable for under 24 h → no warning. Over 24 h → "Couldn't update <name>".
7. `AccessKeyRejected` → previous prayers kept, warning "access key rejected".
8. `NotModified` → last-success time updated, prayers unchanged.
9. Stored feed's timezone no longer matches the Location → `prayers()` is `None` and a warning shows.
10. A round trip through serde (the `cache/organization.json` shape) keeps the feed, `ETag` and timestamps.

### Slice 4: HTTP (S4)

A test helper spins up a `TcpListener` on `127.0.0.1:0`, answers one canned response and returns the request it received.
- `http://` is only allowed for `127.0.0.1` in tests, behind a `#[cfg(test)]` switch. Release builds refuse every `http://` URL.

Tests:
1. Bearer header sent when there is an access key, absent without one.
2. `If-None-Match` sent when there is an `ETag`. A `304` → `NotModified`.
3. `401` → `AccessKeyRejected`. `500` → `Unreachable`.
4. Body over 1 MiB → `TooLarge`.
5. `http://example.org` → `InsecureUrl` without sending a request.
6. `#[ignore = "network"]`: fetch a fixture feed from a stable https host, next to `commands::network_tests` (`commands.rs:443`).

### Slice 5: engine and commands (no new tests; S1–S4 cover the logic)

- `Engine`:
  - Holds `organization: OrganizationState`, loaded from `cache/organization.json` at start.
  - `plan_organization_fetch()` returns at most one job when `refresh_due`.
  - Store results via `record`, dropping a result whose `feed_url` changed meanwhile (like `is_current`, `engine.rs:686-691`).
  - Pass `self.organization.prayers()` to `windows` / `compute`.
- `apply_settings` keeps `self.settings.organization`, the same way it keeps `alarm_sound` (`engine.rs:250`). Only the Organization commands change it, so a stale Settings draft can't drop or re-add a membership.
- Commands, each added to the `Commands` table in `src/lib/ipc.ts`, with argument keys checked by hand against the Rust names:
  - `preview_organization({ feedUrl, accessKey })` (async, read-only, like `preview_location`)
  - `follow_organization({ feedUrl, accessKey })`
  - `leave_organization({})`
  - `refresh_organization({})` (forces a fetch, ignores `refresh_due`)
- Saving the Organization settings calls `follow_organization` again, which re-fetches.
- DTOs:
  - `PrayerTimeDto` gains `prayAt: Option<String>` (RFC3339) and `ownTimeSource: OwnTimeSource`.
  - `AppStateDto` gains `organization: Option<OrganizationStatusDto>` with name, host, covered prayers, last updated and warning.
- Backup: `settings_to_apply` keeps this device's `organization`, as it keeps the location (`backup.rs:53-59`). The feed is tied to the Location. Add a test in `backup.rs`.

### Slice 6: frontend

- Re-export the new types from `src/lib/types.ts`.
- `SettingsPage.tsx`:
  - New "Masjid / Organization" section: URL field, optional access key field, Preview → confirmation (name, host, covered prayers and times) → Follow. When following: status line, "Refresh now", "Leave".
  - For locked prayers, disable the "pray after" editor (`SettingsPage.tsx:327`) and show "Set by <name>".
- `PrayerList.tsx`: a lock mark and the Organization's time on covered prayers. A notice when `ownTimeSource` is `OrganizationIgnored`.
- Check in the running app (`run` skill): follow a local fixture feed, see 3 prayers locked, leave, see the user's offsets back.

### Slice 7: docs (domain-modeling)

- `GLOSSARY.md`:
  - **Organization**: a masjid or group whose jamaat times the user follows. UI: "Masjid / Organization".
  - **Organization Feed**: the JSON document at its URL.
  - **Followed Prayer**: a prayer the feed covers, whose Own Prayer Time is locked.
  - Update **Own Prayer Time** to say it can come from an Organization.
- `docs/adr/0002-organization-feed-polling-and-jamaat-only.md`: polling, not webhooks. The feed sets the Own Prayer Time only, not window starts. Times outside the window are ignored, not clamped.
- `docs/organization-feed.md`: a one-page guide for Organizations. The format, the prayer key spellings, hosting a static file (GitHub Pages, own site), `ETag`, and what the access key does and doesn't protect.
- `ARCHITECTURE.md`: the Organization fetch job next to the month jobs.

## Done when

- [ ] `cargo test` and `pnpm build` pass. `cargo test -- --ignored` passes for the new network test.
- [ ] `src/lib/bindings/` has no uncommitted diff.
- [ ] Manual check from slice 6 done.
- [ ] Done marks survive following, leaving and refreshing (manual check: mark Fajr done, follow, leave).
