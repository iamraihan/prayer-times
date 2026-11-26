# Research: following an organization's prayer times

Status: research note, 2026-11-26. Nothing here is implemented yet.

**Question.** An organization (a mosque, say) publishes its prayer times at a public URL that needs no auth to read. A user can "join" it. While they are joined, the prayers the organization covers take the organization's times and can't be changed. The other prayers keep coming from Aladhan and stay adjustable. How does a user join, does joining need any auth, and what is the simplest design?

**Short answer.** Joining needs **no auth**. To join, the app saves a feed URL in `Settings` on this device and fetches it read-only over HTTPS. Nobody's identity is involved, and the server never learns who "joined". Auth exists only on the **write side**: the organization uploads its file to its own website or host, using whatever login that host already has. That is outside this app. The simplest first version is **"paste the organization's feed link"** (plus an optional access key), followed by a confirmation screen. Deep links and a short-code directory can come later.

**Decided 2026-11-26** (see §4–§6; later the same day `ETag` was dropped and the daily fetch fixed at 13:00 Location Time, see ADR 0002): the feed is a flat list of the current **jamaat times** for 1–5 prayers, no dates; they lock those prayers' Own Prayer Time only, and the Prayer Window stays on Aladhan; the app polls once a day (no webhook) with `ETag`; an optional access key goes in a `Bearer` header.

---

## 1. What the code does today

### Fetching the Timetable from Aladhan

- The engine thread runs `plan_fetches` under the lock. It wants the months of yesterday, today and tomorrow (location dates), looks for each in memory, then the disk cache, then plans a network fetch. A failed fetch is retried at most once a minute (`src-tauri/src/engine.rs:624-674`, `RETRY_AFTER` at `engine.rs:45`).
- The fetch runs **outside the lock** in `run_loop`, and the results are stored after it (`engine.rs:1290-1335`). `store_month` drops a result whose `cache_id` no longer matches the settings, for example after the location changed mid-fetch (`engine.rs:686-708`).
- The client is `aladhan::fetch_month`, which calls `calendar` (by coordinates) or `calendarByCity` and sends `method`, `school` and `timezonestring` (`src-tauri/src/aladhan.rs:162-189`, `259-303`). The timezone comes from each day's `data.meta.timezone`; the top-level `meta` is null (`aladhan.rs:12-13`, `63-71`). Times such as `"17:46 (+06)"` are parsed by `parse_hm`, which removes the suffix (`aladhan.rs:218-222`).
- The parsed day is `DayTimings { date, readable, hijri, times: HashMap<Prayer, NaiveTime>, sunrise, midnight }` (`aladhan.rs:102-112`). A prayer missing from the response is simply missing from `times` (`aladhan.rs:224-253`). This internal type predates the "struct per prayer" rule. It never crosses IPC, but new types should not copy it.
- **Caching.** There is one file per (location, method, school, year, month): `cache/calendar-{cache_id}-{yyyy}-{mm}.json` (`aladhan.rs:341-363`). Once a month is on disk **it is never fetched again** (`engine.rs:650-653`). That is safe for computed times. An organization's feed can change mid-month (Ramadan, a corrected typo), so it needs a refresh policy that the Aladhan cache doesn't have.

### Per-prayer user settings

- `PrayerSettings { enabled, pray_after_minutes }` holds the prayer's alarm switch and its **Own Prayer Time** offset ("I pray N min after it starts") (`src-tauri/src/models.rs:178-200`). There is **no per-prayer adjustment to the start time**. A user "adjusting a prayer's time" today means only the Own Prayer Time, which moves the Get-Ready and Main Alarms and leaves the Prayer Window and Before-End Warning where they are (GLOSSARY "Own Prayer Time").
- `PrayerSettingsMap` has one field per prayer, each `#[serde(default)]` (`models.rs:202-240`). This is the shape CLAUDE.md requires instead of `HashMap<Prayer, _>`.
- In the UI, `SettingsPage.tsx` edits `prayAfterMinutes` (`src/pages/SettingsPage.tsx:327-328`). The `update_settings` command takes a **whole `Settings`** from the UI (`src-tauri/src/commands.rs:168-183`). Any lock therefore has to be enforced in Rust, not only by greying out a control.

### How settings and runtime state persist

- `Settings` is `#[serde(rename_all = "camelCase", default)]` (`models.rs:242-270`). A field missing from an older file takes its default. Nothing uses `deny_unknown_fields` (no match in `src-tauri/src`), so an **older** app version silently ignores a new field it doesn't know.
- `settings_from_json` returns `None` on any parse failure, and `settings::load` then falls back to `Settings::default()`. That wipes everything, location included (`models.rs:304-320`, `src-tauri/src/settings.rs:18-26`). Closed sets use lenient deserializers for this reason (`AsrSchool` `models.rs:102-133`, `TimeFormat` `models.rs:155-176`).
- `runtime.json` (`RuntimeState`: day, fired, snoozes, done) is also `serde(default)` (`src-tauri/src/runtime.rs:12-23`).
- Backups carry the full `Settings` (`src-tauri/src/backup.rs:19-21`). `settings_to_apply` keeps this device's location and alarm tone and takes everything else from the backup (`backup.rs:53-59`), so a membership would travel with a backup.

### How the schedule is computed

- `schedule::window(prayer, day, tz)` resolves the start from `day.times`, and the end from `end_time_of`: Fajr → sunrise, Dhuhr → Asr, Asr → Maghrib, Maghrib → Isha, Isha → midnight. It rolls to the next date when the end comes before the start (Isha past 00:00) (`src-tauri/src/schedule.rs:84-109`).
- `timeutil::resolve` returns `None` in a DST gap and the first instant in an overlap. `resolve_after` rolls to the next day (`src-tauri/src/timeutil.rs:11-35`).
- `compute` turns `DayTimings + Settings` into the Get-Ready, Main Alarm and Before-End Warning events. `pray_time` clamps the Own Prayer Time into `[start, end − 5 min]` (`schedule.rs:64-82`, `157-208`).
- `prayer_day` and `day_start` use **yesterday's** `DayTimings` for Isha past midnight (`schedule.rs:111-152`; ADR 0001).
- The engine reads days with `Engine::day_timings(date)`, straight from `months` (`engine.rs:988-994`). The UI DTO is built in `day_timings_dto` (`engine.rs:1210-1235`).

### Where an organization source plugs in

The natural seam is **between `day_timings(date)` and everything that consumes it**. A pure `schedule::merge(aladhan_day, org_day) -> (DayTimings, locks)` produces the day the rest of the code already understands. `window`, `end_time_of`, `compute`, `prayer_day`, `featured` and `missed_now` then need no change, except that the Own Prayer Time can come from the organization (§5). The feed fetch copies the existing pattern: it is planned under the lock, fetched outside it, and stored with a staleness check (`engine.rs:97-117`, `1290-1335`).

### The "change settings" path today

`apply_settings` calls `reset_schedule` when the location or calculation changes, and that clears **Done marks, fired events and snoozes** (`engine.rs:246-262`, `975-986`). Joining or leaving should **not** go through `reset_schedule`. Done marks belong to the Prayer Day, not to the source of the times. A changed event time is already handled by `Due::Rearm` (`schedule.rs:244-266`).

---

## 2. How a user joins: options from simplest up

All of these end the same way. A feed URL ends up in `Settings`, the app fetches it read-only, and it shows a confirmation screen ("Follow *East Town Mosque*? Its jamaat times for Fajr, Dhuhr and Asr will be used and locked").

| # | Mechanism | What the user does | What has to exist | Trade-offs |
|---|---|---|---|---|
| a | **Paste feed URL** | Copies a link from the mosque's website, WhatsApp group or newsletter into Settings → "Follow an organization" | Only the organization's static JSON file | No infrastructure and no plugin. Works on every OS. Typing a long URL by hand is clumsy, but people usually paste. A wrong URL fails validation instead of silently applying. |
| a′ | **Domain shortcut** | Types `easttownmosque.org` and the app tries a fixed path on that host | The organization puts the file at a known path | Nicer to type. But RFC 8615 says "Applications that wish to mint new well-known URIs MUST register them" ([RFC 8615 §3](https://www.rfc-editor.org/rfc/rfc8615)), so a `/.well-known/…` path would need an IANA registration. A plain fixed path (e.g. `/prayer-times.json`) avoids that, but it can collide with the site's own content. Worth adding only once several organizations exist. |
| b | **Short join code** (`ETM-4821`) resolved through a directory | Types a code | **A directory service run by this project**: hosting, moderation, takedowns, uptime | Best typing experience. It creates a central point that can impersonate any mosque, it can go down, and someone has to run it. It also needs write-side auth for organizations to register. That is the opposite of "simplest". |
| c | **Invite link / deep link** (`prayertimes://follow?feed=https%3A%2F%2F…`) | Clicks a link on the mosque's site | Tauri deep-link plugin + single-instance plugin | One click. Caveats from the [Tauri deep-linking docs](https://v2.tauri.app/plugin/deep-linking/): desktop schemes are declared under `plugins > deep-link > desktop > schemes`. On Linux and Windows the link arrives as a CLI argument to a **new process**, so the single-instance plugin is needed (with its `deep-link` feature, "the first plugin you register"). "Registering deep links at runtime is not possible on macOS", and testing there needs the bundled app installed in `/Applications`. Moving an AppImage breaks the link because it records the executable's absolute path. Desktop links generally work only for installed apps. The docs also warn "The user could trigger a fake deep link manually by including the URL as argument", so the link must still be validated and confirmed. The repo has neither plugin today (`src-tauri/Cargo.toml:23-29`). |
| d | **QR code** | Scans a poster | A link (a or c) encoded as QR | Not a mechanism in itself, only a way to carry the URL. Desktop machines rarely have a camera pointed at a poster. A QR that opens an https page on the mosque's site with a "copy link" and an "open in app" button helps phone users get the link to their desktop. Low value for a desktop-only app. |

### Does joining need auth?

No. The answer follows from the data flow:

- **Read side (the user joins).** Reading a public timetable is an anonymous HTTPS `GET`, the same as the current Aladhan calls, which also use no key (`aladhan.rs:259-274`). "Membership" is one local setting. The organization can't tell who follows it beyond ordinary access logs, which is better for privacy. It also can't revoke a follower, and it doesn't need to.
- **Write side (the organization publishes).** Only the organization edits its file. That needs auth, but it is the auth of **their** web host (WordPress admin, GitHub Pages push rights, an S3 bucket policy). The app never sees it. A real-world example of this shape: the BC Muslim Association serves per-organization daily times from `https://org.thebcma.com/api/Prayertimes/GetPrayertimeByDate?organizationId=7&dt={date}`, and the page that documents it describes no API key ([Kelowna Islamic Center docs](https://kelowna-islamic-center.github.io/documentation/server-side/prayer-times-api)).
- **Integrity is what matters, not identity**: "can a stranger make my alarms wrong?". HTTPS answers that, plus an optional signature (§3). Neither needs accounts.

Auth would only become necessary for features nobody asked for yet: private or members-only timetables, follower counts, or organizations registering in a directory (option b).

---

## 3. Trust and integrity without accounts

- **HTTPS only.** Refuse `http://` at join time. The HTTP client already uses rustls (`src-tauri/Cargo.toml:34-38`). Cap the response size (e.g. 1 MiB) and set a timeout, as `http_client_with_timeout` already does for other requests (`commands.rs:29-40`).
- **Confirm before applying.** Show the organization name, the host, the IANA timezone, and the covered prayers with their times. The deep-link docs' warning about forged links (above) applies just as much to a pasted URL.
- **Optional signed feed (later).** With HTTPS alone, a compromised or expired domain, or a hijacked CMS, can push wrong times. A detached signature closes that gap:
  - [minisign](https://jedisct1.github.io/minisign/) "uses the highly secure Ed25519 public-key signature system". `minisign -G` makes a key pair, `minisign -Sm feed.json` writes `feed.json.minisig`, and the signature has a **trusted comment** covered by the signature, which could hold the feed's `updatedAt`. New implementations should use the pre-hashed (`ED`) format.
  - Verify in Rust with [`minisign-verify`](https://docs.rs/minisign-verify/latest/minisign_verify/): "zero-dependency verification of Minisign signatures", MIT licensed, with `PublicKey::from_base64`, `Signature::decode` and `PublicKey::verify`.
  - The project already uses minisign: the updater's `pubkey` in `src-tauri/tauri.conf.json:43` decodes to `untrusted comment: minisign public key: F33B9693DC9D3F39`, and the Tauri docs say the updater signature "cannot be disabled" ([Tauri updater](https://v2.tauri.app/plugin/updater/)). The maintainers and CI (`.github/workflows/release.yml:96-97`) already know the tool.
  - Key distribution: put the public key in the **join link** (e.g. `…/feed.json#key=RWQ…`) and save it with the membership. From then on the app rejects a feed that isn't signed by that key (trust on first use). This needs no accounts. A key rotation means re-joining.
- **When the feed fails** (keep last good, never silently mix in a bad one):

| Situation | Behaviour |
|---|---|
| Unreachable (offline, 5xx) | Keep using the **last good cached feed**. Retry with backoff. Show a quiet "Couldn't reach East Town Mosque, using times from <date>" only after, for example, 24 h of failures. |
| Malformed, unknown prayer key, wrong signature (later), timezone ≠ the user's Location timezone | Reject the **whole** new document and keep the previous good one. Show a warning with the reason. Never apply part of a bad document. |

---

## 4. Feed format

### What exists

- **Aladhan** returns a month as `data[]`. Each day has `timings` (`Fajr`, `Sunrise`, `Dhuhr`, `Asr`, `Maghrib`, `Isha`, `Midnight`, …) as `"HH:MM (+06)"` strings, `date.gregorian.date` as `DD-MM-YYYY`, and the IANA zone in `meta.timezone`. This is visible in the fixture at `aladhan.rs:379-391` and in the parser at `aladhan.rs:29-96`. I couldn't check this against the live docs today: `aladhan.com/prayer-times-api` is a client-rendered page, and `api.aladhan.com` (OpenAPI spec and endpoints) returned HTTP 502 "failure to get a peer from the ring-balancer" every time I tried on 2026-11-26. Statements here about Aladhan come from the repo's own parser and tests.
- **Mawaqit**: "Our API is currently private and not publicly available" ([Mawaqit help](https://help.mawaqit.net/en/articles/11991838-can-i-use-your-api)). Community wrappers scrape undocumented endpoints. Don't build on them.
- **MasjidBox**: "Masjidbox doesn't offer a public API for prayer times". It offers CSV or Excel exports of "yearly or monthly schedules, including Athan and Iqamah times" ([MasjidBox support](https://support.masjidbox.com/does-masjidbox-have-a-public-api-for-prayer-times-sfnfd)). So a mosque using MasjidBox could turn that export into a static file. A small converter script could live in this repo later.
- **BCMA** (above): one flat object per day, with pairs of start and iqamah fields (`"fajr": "4:02:00 AM"`, `"fajrIqama": "5:00:00 AM"`) and a date in `MM/DD/YYYY` ([Kelowna Islamic Center docs](https://kelowna-islamic-center.github.io/documentation/server-side/prayer-times-api)). This confirms that real mosque feeds publish **both** the start and the congregation time.
- **iCalendar (RFC 5545)** is a generic option. Local times are tied to a zone through `TZID`, and "An individual "VTIMEZONE" calendar component MUST be specified for each unique "TZID" parameter value". The spec also says "This document does not define a naming convention for time zone identifiers" ([RFC 5545 §3.2.19](https://www.rfc-editor.org/rfc/rfc5545#section-3.2.19)). That makes it heavy to parse (VTIMEZONE rules, recurrence) and loose about IANA names. It's a good **export** format for calendar apps and a poor **input** format here.

No open standard for mosque timetables turned up with first-party docs. A small JSON format of our own is the pragmatic choice.

### Decided format (version 1)

Decided with the user on 2026-11-26. The organization edits its times on its own dashboard, and the feed returns only the **current jamaat times**: no dates, no per-day table, no periods. Jamaat times change rarely, so the app treats the latest values as valid for every day until the feed changes.

```json
{
  "name": "Baitul Aman Masjid",
  "timezone": "Asia/Dhaka",
  "updatedAt": "2026-11-26T10:00:00+06:00",
  "prayers": {
    "fajr": "05:30",
    "dhuhr": "13:15",
    "asr": "16:00"
  }
}
```

Rules:

- `prayers` keys are exactly `fajr`, `dhuhr`, `asr`, `maghrib`, `isha` (the GLOSSARY spellings, lower case, same as Aladhan's keys). Any 1 to 5 of them may be present. A missing key means that prayer isn't covered. Any other key under `prayers` (`fazr`, `zuhr`, `johor`) makes the document invalid, and the preview says "unknown prayer `zuhr`, did you mean `dhuhr`?". Unknown keys **outside** `prayers` are ignored, so the format can grow.
- Each value is `HH:MM`, 24-hour, wall clock in Location Time. It is the **congregation time**, which becomes that prayer's Own Prayer Time (§5). It is **not** a Prayer Window start: starts, sunrise and midnight always come from Aladhan.
- `timezone` is required and must be an IANA name equal to the user's `Location.timezone`. Otherwise the app refuses to follow (preview) or rejects the update (refresh), because a time in another zone would be meaningless here.
- `name` is required (shown in the UI). `updatedAt` is optional (shown as "updated 2 days ago").
- DST needs nothing special: each day the app resolves the wall-clock time with `timeutil::resolve(tz, date, time)` (`timeutil.rs:11-19`). A time in a DST gap gives no Own Prayer Time override that day (the user's own `prayAfterMinutes` applies), and an overlap takes the first instant.
- Isha: the value belongs to the Prayer Day it is resolved for. A congregation time after 00:00 (e.g. `"00:30"`) is resolved with `resolve_after` against that day's Isha start, as window ends are (`schedule.rs:100-109`).
- Not in version 1: Jumu'ah (later feature), Eid, Taraweeh, start-time overrides, signatures.

### Fetching: polling, not a webhook

A webhook means the organization's server calls the client. A desktop app has no public address to call, so this is **polling**: the app asks the feed URL.

- Fetch when the user follows or saves the organization settings, at launch if the last good fetch is older than 24 h, once a day while running, after the device wakes, and on a "Refresh now" button.
- Send `If-None-Match` with the stored `ETag`; a `304 Not Modified` costs the organization almost nothing.
- A changed time applies from the next event computation; an event that moves is re-armed by `Due::Rearm` (`schedule.rs:244-266`).

### Optional access key

- One optional field in the organization settings, "Access key". When set, the app sends `Authorization: Bearer <key>` (never in the URL, which ends up in server logs).
- It is **not** real security: every member gets the same key, and anyone can read it from the app or the settings file. It lets the organization rotate keys, count use, and rate-limit callers without one. Protection against excess calls comes mainly from the once-a-day fetch and `304` responses.
- v1 stores it in `settings.json` (plain text); the OS keychain (`keyring` crate) is a later improvement.
- HTTP `401`/`403`: keep the last good feed and show "access key rejected".

---

## 5. Locking semantics

- **What locks.** A prayer is locked exactly when its key is in the last good feed's `prayers`. Its Own Prayer Time is the organization's time, and the user's `prayAfterMinutes` is ignored and its editor disabled. Prayers not in the feed keep the user's own `prayAfterMinutes` ("jamaat" offset from the Aladhan start), as today.
- **The Prayer Window never changes.** Starts, sunrise and midnight come from Aladhan, so window ends and the Before-End Warning are unaffected by following. (This supersedes the earlier idea of feed start times, which would have let an organization's Asr move Dhuhr's end.)
- **Out-of-window times.** `pray_time` today clamps into `[start, end − 5 min]` (`schedule.rs:64-82`). For an organization time outside that range (Asr `"12:00"`, or inside the last 5 minutes), **don't clamp**: ignore the override for that day, use the user's own setting, and show a warning. Clamping would ring at a time the masjid never announced.
- **Never locked:** the alarm switch (`PrayerSettings.enabled`) and the global Get-Ready / Before-End minutes. They are preferences, not times.
- **Overlay, don't overwrite.** Don't copy organization values into `PrayerSettings`; apply them at compute time. Leaving restores the user's own settings automatically, and the lock holds even though `update_settings` takes a whole `Settings` from the UI (`commands.rs:168-183`).
- **Done marks, fired events, snoozes.** Keep them on join, leave and refresh (don't call `reset_schedule`, `engine.rs:975-986`). The events are re-derived on the next tick.
- **History.** Unaffected, because it is keyed by Prayer Day and prayer (`models.rs:551-560`).

---

## 6. Recommendation

### Version 1 (ship first)

1. **Join by pasting an https feed URL** (plus optional access key) in Settings → "Masjid / Organization". A read-only, async preview command (like `preview_location`, `commands.rs:54-88`) fetches and validates the feed and shows name, host, timezone and the covered prayers with their times. "Follow" saves it; "Leave" clears it. No accounts.
2. **Rust model** (`models.rs`, following the CLAUDE.md rules):
   - `Settings.organization: Option<OrganizationLink>`. `Settings` already has `serde(default)`, so old files load as `None`, and older app versions ignore the field.
   - `OrganizationLink { feed_url: String, access_key: Option<String> }`, `#[serde(rename_all = "camelCase", default)]`, `#[ts(export)]`, re-exported from `src/lib/types.ts`.
   - Feed types (deserialize only): `OrganizationFeed { name: String, timezone: String, updated_at: Option<DateTime<FixedOffset>>, prayers: OrganizationPrayers }`, where `OrganizationPrayers` is a **struct with one field per prayer** (`fajr: Option<NaiveTime>, … isha: Option<NaiveTime>`), `deny_unknown_fields` on this struct only, never `HashMap<Prayer, _>`.
   - IPC: the snapshot gets `organization: Option<OrganizationStatusDto>` (name, last refreshed, warning). `PrayerTimeDto` (`models.rs:564-577`) gets `locked: bool` plus the Own Prayer Time instant, so the UI shows a lock icon and disables the editor. Commands `preview_organization(feedUrl, accessKey)`, `follow_organization(feedUrl, accessKey)`, `leave_organization()`, `refresh_organization()`, each added to the `Commands` table in `src/lib/ipc.ts:32`.
3. **Pure logic in `schedule.rs`**: `compute` takes an optional `&OrganizationPrayers` and, per prayer, uses the organization time when present and valid for that day, else `prayAfterMinutes`. Failing tests first: 3 of 5 covered; leaving restores `prayAfterMinutes`; organization time outside the window or in its last 5 minutes is ignored with a warning; DST gap and overlap; Isha congregation after 00:00; the 1st of a month; feed timezone ≠ Location timezone rejected; unknown prayer key rejected.
4. **Engine**: an organization fetch job next to the month jobs, planned under the lock and fetched outside it (`engine.rs:1290-1335`), on the schedule in §4 with `ETag`. Store the last good document and its `ETag` as `cache/organization.json`. Drop a result whose `feed_url` no longer matches the settings.
5. **Failure policy**: unreachable or `5xx` → keep the last good feed, warn after 24 h; malformed, unknown prayer key, wrong timezone, `401`/`403` → reject the whole document, keep the last good one, warn. No last good feed → no overrides at all.
6. Add `#[ignore = "network"]` tests for the feed fetch next to `commands::network_tests` (`commands.rs:443`), using a fixture feed on a stable host.
7. A one-page "How to publish your masjid's times" guide for organizations (the format in §4, how to host a static JSON file).

### Later

- **Signed feeds** (minisign, with the public key carried in the join link, verified with `minisign-verify`).
- **Deep-link "Follow" button** (`prayertimes://follow?feed=…`) with the deep-link and single-instance plugins, keeping in mind the macOS, AppImage and installed-app caveats above. Always show the same confirmation screen.
- **Jumu'ah** time (`"jumuah"` key) replacing Dhuhr's Own Prayer Time on Fridays.
- Access key in the OS keychain instead of `settings.json`.
- A **converter** from MasjidBox or Excel exports to the feed format.
- A domain shortcut (a fixed path, or a registered well-known URI) or a short-code directory, only if there is demand. The directory brings hosting, moderation and write-side auth for organizations.
- Following an organization's **location** (`coordinates` in the feed), so sunrise and midnight come from the mosque's position rather than the user's city.

### Not verified

- Aladhan's live API docs. The API returned 502 throughout this session, and the response-shape statements come from `aladhan.rs`.
- Whether Aladhan's `tune` parameter could do per-prayer minute offsets. This isn't needed for the design, because organization times are absolute, and the app doesn't send it today (`aladhan.rs:171-189`).
- How the Tauri deep-link plugin registers the scheme for `.deb` and `.rpm` installs on Linux. The plugin page doesn't cover `.desktop` files.
