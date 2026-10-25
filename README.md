# Prayer Times

[![CI](https://github.com/iamraihan/prayer-times/actions/workflows/ci.yml/badge.svg)](https://github.com/iamraihan/prayer-times/actions/workflows/ci.yml)
[![Release](https://github.com/iamraihan/prayer-times/actions/workflows/release.yml/badge.svg)](https://github.com/iamraihan/prayer-times/actions/workflows/release.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

A cross-platform (Windows, macOS, Ubuntu/Linux) prayer-times app built with
Tauri 2, using the [Aladhan API](https://aladhan.com/prayer-times-api).

**Website:** [raihanali.com/prayer-times](https://raihanali.com/prayer-times) —
feature overview and downloads for Windows, macOS & Linux.

- **System tray status** — next prayer, countdown and window end time in the
  tray tooltip/menu (Windows taskbar corner, macOS menu bar, Ubuntu app
  indicator). Left-click the tray icon to open the app; closing the window
  keeps the app running in the tray.
- **Alarms that follow the whole prayer window** — see the table below.
- **First-run wizard** — pick country + city, calculation method and Asr
  school; a live preview is fetched before anything is saved.
- **Offline-friendly** — each month's timetable is cached on disk
  (one API call per month in the happy path).
- **In-app auto-updates** — the app checks for updates on launch and from
  Settings; see below.

## Install

Grab the latest installer for your OS from the
[Releases page](https://github.com/iamraihan/prayer-times/releases/latest):

| OS            | File to download |
| ------------- | ---------------- |
| Windows       | `Prayer Times_x.y.z_x64-setup.exe` (recommended — supports in-app updates) or the `.msi` |
| macOS         | `_aarch64.dmg` (Apple Silicon) or `_x86_64.dmg` (Intel) |
| Ubuntu/Linux  | the `.AppImage` (supports in-app updates), or `.deb` / `.rpm` |

## Auto-updates

The app checks for updates on every launch and on demand from
**Settings → Updates**.

- **Windows and Linux (AppImage)**: updates download and install inside the
  app, which then restarts into the new version — no manual download.
- **macOS**: the app detects updates and links to the release page (silent
  self-updates on macOS require an Apple Developer ID certificate).
- Updates are cryptographically signed — the app refuses anything not
  signed with the maintainer's key.

## Alarm behavior

For each prayer (Fajr, Dhuhr, Asr, Maghrib, Isha) the scheduler plans:

| Alarm        | When it fires                                              | Default |
| ------------ | ---------------------------------------------------------- | ------- |
| Main         | at the prayer's start time — or N minutes after it when a per-prayer delay is set ("you still have N minutes") | +30 min delay (Maghrib: exactly at start) |
| Before-end   | N minutes before the window closes — "you will miss it"     | 10 min  |
| Snooze       | when a "Remind me again in N minutes" answer is due         | user-set minutes |

Each alarm shows **all three channels** (each can be toggled in Settings):

1. a system notification (Windows/macOS notification center, Ubuntu daemon),
2. a looping alarm sound — the built-in chime, or your own audio file
   (Settings → **Alarm tone → Choose file…**, mp3/wav/ogg/flac up to 20 MB;
   the file is copied into the app's data folder so it persists) — it
   auto-stops after 2 minutes if unanswered,
3. a small always-on-top popup with the actions below.

From the popup, the in-app banner, or the tray menu you can:

- **✓ Mark as done** — silences *all remaining alarms for that prayer today*
  (e.g. marking Dhuhr done stops its reminder and before-end warning).
- **Remind me again in N minutes** — preset chips (5/10/15/30) or a custom
  number of minutes; the alarm re-fires then.
- **Dismiss for now** — stops the sound; later reminders still fire because
  the prayer isn't marked done.

A prayer window closes at: Fajr → sunrise, Dhuhr → Asr, Asr → Maghrib,
Maghrib → Isha, Isha → midnight.

## Tech stack

- **Frontend**: React 19 + TypeScript + Vite + Tailwind CSS v4, zustand store,
  lucide icons. Two pages builds: `index.html` (main window) and
  `alarm.html` (popup window).
- **Backend**: Rust (Tauri 2) — scheduling, tray, alarms, caching.
  `chrono-tz` for location-timezone-correct math, `rodio` for a synthesized
  chime (no audio assets), `reqwest` (rustls) for the Aladhan API.

See **ARCHITECTURE.md** for the module map and how to extend the app.

## Getting started

Prerequisites: [Node](https://nodejs.org) 20+, [pnpm](https://pnpm.io), and a
Rust toolchain per the [Tauri prerequisites](https://tauri.app/start/prerequisites/).

### One-time setup on Windows

```powershell
winget install Rustlang.Rustup
winget install Microsoft.VisualStudio.2022.BuildTools --override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
```

(WebView2 is pre-installed on Windows 10/11.)

### One-time setup on Ubuntu

```bash
sudo apt update
sudo apt install -y build-essential curl libssl-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev libwebkit2gtk-4.1-dev libasound2-dev
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### One-time setup on macOS

```bash
xcode-select --install
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### Run in dev mode

```bash
pnpm install      # once, after cloning / pulling new dependencies
pnpm tauri dev
```

What happens and what to expect:

- Vite starts on `http://localhost:1420` and Cargo compiles the Rust backend
  in **debug** mode, then the app window opens.
- The **first** `pnpm tauri dev` (or the first after dependency changes)
  compiles everything — allow 5–10 minutes. After that, startup is seconds.
- **Frontend changes** (TS/TSX/CSS) hot-reload instantly in the open window.
- **Rust changes** (`src-tauri/src/**`) recompile and restart the app
  automatically (~1–2 minutes).
- Stop it with `Ctrl+C` in the terminal. The tray icon and background
  scheduler only run while dev mode is running.
- Debug builds include devtools: right-click in the app → *Inspect Element*
  for the web console (frontend errors, IPC failures).
- `pnpm dev` (frontend only, in a browser) does **not** work — the app calls
  Tauri IPC commands that only exist inside the desktop shell.

Fast feedback while coding Rust (no window, much quicker than a full build):

```bash
cd src-tauri
cargo check        # type-check the backend in seconds
```

### Building a release

```bash
pnpm tauri build
```

This compiles the frontend, builds Rust in **release** mode (optimized,
slower — allow ~10–15 minutes on a cold cache, ~2–3 minutes incremental) and
produces the installers for the OS you run it on:

| Platform | Standalone app | Installers |
| -------- | -------------- | ---------- |
| Windows  | `…\release\prayer-times.exe` | `bundle\nsis\Prayer Times_x.y.z_x64-setup.exe`, `bundle\msi\Prayer Times_x.y.z_x64_en-US.msi` |
| macOS    | `…/release/bundle/macos/Prayer Times.app` | `bundle/dmg/Prayer Times_x.y.z_aarch64.dmg` |
| Linux    | `…/release/prayer-times` | `bundle/deb/…_amd64.deb`, `bundle/appimage/….AppImage`, `bundle/rpm/…` |

Notes:

- **Output location**: by default everything lands in `src-tauri/target/`.
  On this machine `.cargo/config.toml` redirects it to
  `E:\cargo-target\prayer\` to protect the nearly-full C: drive — don't
  delete that file.
- Build only one installer type to save time:
  `pnpm tauri build --bundles nsis` (or `msi`, `dmg`, `deb`, `appimage`).
- **Releasing a new version**: see [RELEASING.md](RELEASING.md) — bump the
  version in three files, push, then tag `vX.Y.Z`; GitHub Actions builds all
  three platforms and publishes the release with signed update files.
- **Custom app icon** (replaces the placeholder):
  `pnpm tauri icon path/to/icon.png` regenerates all sizes in
  `src-tauri/icons/`, then rebuild.
- You must build **on** each target OS (Windows exe on Windows, dmg on macOS,
  deb/AppImage on Linux). For automatic multi-platform builds from one
  checkout, use the official
  [tauri-action](https://github.com/tauri-apps/tauri-action) GitHub Actions
  workflow later.

## Notes

- **Data lives in** the OS config dir (e.g.
  `%APPDATA%\com.prayertimes.app` / `~/.config/com.prayertimes.app`):
  `settings.json`, `runtime.json` (fired alarms / done marks for today) and
  `cache/` (one timetable file per month + method).
- **Windows dev notifications**: `pnpm tauri dev` may show plain toast
  notifications; installed builds use the proper app identity.
- **Tray icon**: replace the placeholder icons in `src-tauri/icons/` with
  your own (`pnpm tauri icon path/to/icon.png`).
- Times are computed for the **selected city's timezone** (from the API), so
  alarms are correct even if the device clock timezone differs.
