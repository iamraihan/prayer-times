# Organizations are polled once a day and set only the Own Prayer Time

A user can follow an **Organization** (a masjid or any other group) by pasting the link to its **Organization Feed**. Three choices here look odd at first glance.

**Polling, not a webhook.** With a webhook, the Organization's server would call the app when its times change. A desktop app has no public address to call, so the app asks instead: when the user follows or saves, then once a day at 13:00 Location Time (or at the first tick after it, if the app wasn't running), with a 15-minute retry after a failure. Jamaat times change a few times a year, so a day's delay is fine, and one small request per member per day costs the Organization nothing. We dropped `ETag` / `304` for the same reason: it saves nothing worth the extra state.

**The feed sets the Own Prayer Time, not the Prayer Window.** A masjid announces when its congregation prays, not when the prayer's time begins or ends. So a Followed Prayer's Get-Ready and Main Alarms move to the masjid's time, while the window, and so the Before-End Warning, keep coming from the Timetable. If the feed set window starts, a masjid's Asr would also move Dhuhr's end, and its school would override the user's Asr School.

**A time outside the window is ignored, not clamped.** For the user's own "N min after" offset, `pray_time` clamps into `[start, end − 5 min]`. A masjid time outside that range (a typo like Asr `12:00`, or a time in a DST gap) is instead ignored for that day: the user's own setting applies and the UI says so. Clamping would ring at a time the masjid never announced.

## Considered Options

- **Accounts or join codes.** Rejected: reading a public timetable needs no identity, and a directory of codes would be a service to host and moderate. The optional Access Key covers Organizations that want to limit use.
- **A per-day timetable (dates or periods).** Rejected for version 1: Organizations edit their current times on their own dashboard, and the feed carries only those.
- **Copying the Organization's times into `PrayerSettings`.** Rejected: leaving would then have to restore the user's own values. The feed is applied at compute time instead (`schedule::own_prayer_time`), so leaving restores them for free, and a stale Settings draft can't unlock a Followed Prayer.

## Consequences

- When the feed can't be fetched or a new document is invalid, the last good document (`cache/organization.json`) keeps applying. An unreachable feed only shows a warning after a day; a rejected document or Access Key shows one at once.
- A feed whose `timezone` isn't the Location's is refused, and a stored one stops applying after the user moves.
- Following, leaving and refreshing never call `reset_schedule`: Done marks, fired alarms and Snoozes belong to the Prayer Day.
- Backups leave the Organization out (its Access Key shouldn't travel in a file), and a restored backup keeps this device's Organization, as it keeps the location.
