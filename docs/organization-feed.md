# Publishing your jamaat times (for masjids and organizations)

Prayer Times can follow the jamaat times of your masjid or organization. You publish one small JSON file at an `https://` link and share the link with your members. Each member pastes it into **Settings → Masjid / Organization**. Members don't need accounts.

The prayers you list are set to your times and members can't change them. Any prayer you leave out keeps the time the app calculates, which members can adjust themselves. When you change a time, members get it the next day at 13:00, or straight away if they press **Refresh now**.

## The file

Two ready-made examples are in this repository. Copy one and change the values:

- [`docs/examples/organization-feed.json`](examples/organization-feed.json): all five prayers.
- [`docs/examples/organization-feed-partial.json`](examples/organization-feed-partial.json): Fajr, Dhuhr and Asr only. Maghrib and Isha are `null`, so members set those themselves.

To try one in the app, paste its raw link, for example `https://raw.githubusercontent.com/iamraihan/prayer-times/main/docs/examples/organization-feed-partial.json`. The examples are for `Asia/Dhaka`, so the app's location must be a city in Bangladesh. Their times are late-November jamaat times, so in another season some may fall outside a prayer's window and be ignored for that day.

```json
{
  "name": "Baitul Aman Masjid",
  "timezone": "Asia/Dhaka",
  "updatedAt": "2026-11-26T10:00:00+06:00",
  "prayers": {
    "fajr": "05:30",
    "dhuhr": "13:15",
    "asr": "16:00",
    "maghrib": "17:25",
    "isha": "19:45"
  }
}
```

| Field | Required | What it is |
|---|---|---|
| `name` | yes | Your masjid's or organization's name, as members will see it. |
| `timezone` | yes | Your [IANA timezone](https://en.wikipedia.org/wiki/List_of_tz_database_time_zones), e.g. `Asia/Dhaka`, `Europe/London`. It must match the member's city: a member elsewhere can't follow you. |
| `updatedAt` | no | When you last changed the times, in [RFC 3339](https://www.rfc-editor.org/rfc/rfc3339) format. |
| `prayers` | yes | The jamaat time of each prayer you set. |

### Rules for `prayers`

- Use exactly these names: `fajr`, `dhuhr`, `asr`, `maghrib`, `isha`. Spellings such as `fazr`, `zuhr`, `johor`, `magrib` or `esha`, and capitals (`Fajr`), are rejected. The member sees which name to use.
- Write times as `HH:MM` on the 24-hour clock, in your timezone: `05:30`, `13:15`, `19:45`. Times such as `5:30`, `1:15 PM` and `13:15:00` are rejected.
- List only the prayers you set, from 1 to 5. Leave a prayer out, or set it to `null`, if members choose its time themselves:
  ```json
  "prayers": { "fajr": "05:30", "dhuhr": "13:15", "asr": "16:00", "maghrib": null, "isha": null }
  ```
- Each time must fall inside that prayer's time, and not in its last 5 minutes. A time outside it (Asr at `12:00`, say) is ignored that day, and the member's own setting applies.
- Other prayers such as Jumu'ah aren't supported yet. Fields the app doesn't know are ignored outside `prayers`, but rejected inside it.

The app rejects the whole file if anything in it is wrong. Members keep your last good times until you fix it.

## Hosting the file

Anywhere that serves the file over `https://` works:

- your own website, e.g. `https://example.org/prayer-times.json`;
- GitHub Pages, or the "Raw" link of a file in a public GitHub repository or Gist;
- your dashboard, if it can return this JSON at a fixed link.

The file must be under 1 MB (a real one is a few hundred bytes). The link must not redirect to plain `http://`.

## Access key (optional)

If you set an access key, give it to your members along with the link. The app sends it with every request as:

```
Authorization: Bearer <key>
```

Your server can then turn away requests without the key, count use, and replace the key when you want to. Answer `401` or `403` to a wrong key, and members will see "The feed rejected the access key."

An access key doesn't keep your times secret: every member has the same key, and it is saved on their computer. Use it to limit use, not to protect private information.

## How often the app asks

Each member's app fetches the file:

- when they follow, or press Save after changing the link or key;
- once a day at 13:00, their local time (or as soon as the app runs after that time);
- when they press **Refresh now**.

If a fetch fails, the app tries again 15 minutes later and keeps your last good times in the meantime.
