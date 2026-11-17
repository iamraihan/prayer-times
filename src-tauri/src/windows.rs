//! Small window helpers.

use std::sync::Mutex;

use tauri::webview::{NewWindowResponse, WebviewWindowBuilder};
use tauri::{AppHandle, Manager, WebviewUrl, utils::config::Color};

const MAIN_WINDOW: &str = "main";
const GUIDE_WINDOW: &str = "guide";

/// The tutorial video on YouTube.
const GUIDE_VIDEO_ID: &str = "35BldPFeyjc";

/// The video's YouTube watch page.
///
/// Not the `/embed/` player: YouTube refuses to play embeds without a web
/// Referer ("Error 153"), and the app's pages come from `tauri://localhost`
/// on Linux and macOS, which sends none, whether the embed sits in an
/// `<iframe>` or is the window's top-level page. The watch page needs no
/// Referer; `GUIDE_STYLE` trims it down to just the player.
fn guide_url() -> String {
    format!("https://www.youtube.com/watch?v={GUIDE_VIDEO_ID}")
}

/// The app's darkest surface (`slate-950` in `styles.css`). Painted by the
/// window before the page loads, and by `GUIDE_STYLE` behind the player, so
/// the window never flashes white.
const GUIDE_BACKGROUND: Color = Color(0x0d, 0x14, 0x18, 0xff);

/// Pins YouTube's player over the whole window and hides the rest of the
/// watch page (header, comments, recommendations). If YouTube renames these
/// ids the window degrades to the normal watch page, which still plays.
/// Dialogs (cookie consent, sign-in prompts) are left alone so they stay usable.
///
/// No `color-scheme` here: YouTube reads it as a theme change and reloads the
/// page (`&themeRefresh=1`).
///
/// Until the player renders, the window shows the app's background with a
/// quiet loading line instead of a white page or YouTube's layout.
/// `--guide-bg` is set from `GUIDE_BACKGROUND` by `guide_script`.
const GUIDE_STYLE: &str = r#"
html, body {
  overflow: hidden !important;
  background: var(--guide-bg) !important;
}
html::before {
  content: "Loading video guide…";
  position: fixed; inset: 0; z-index: 0;
  display: grid; place-items: center;
  font: 500 13px/1.4 ui-sans-serif, system-ui, -apple-system, "Segoe UI", Roboto, sans-serif;
  color: #93a2a6;
  pointer-events: none;
}
#masthead-container, ytd-masthead, #secondary, #below, #related, #comments,
ytd-mini-guide-renderer, tp-yt-app-drawer { display: none !important; }
/* End of the video: no "Up next" countdown, suggestion grid or autoplay
   switch. The video ends on itself, with the replay button. */
#movie_player .ytp-autonav-endscreen-countdown-overlay,
#movie_player .html5-endscreen, #movie_player .ytp-ce-element,
#movie_player .ytp-autonav-toggle-button-container,
#movie_player .ytp-autonav-toggle { display: none !important; }
/* Offline: a message over the player instead of a frozen frame. */
html.guide-offline::after {
  content: "You’re offline. The video continues when you’re back online.";
  position: fixed; inset: 0; z-index: 2147483647;
  display: grid; place-items: center; padding: 24px; text-align: center;
  background: rgba(13, 20, 24, 0.92);
  font: 500 14px/1.5 ui-sans-serif, system-ui, -apple-system, "Segoe UI", Roboto, sans-serif;
  color: #cdd6d8;
}
#movie_player {
  position: fixed !important; inset: 0 !important;
  width: 100vw !important; height: 100vh !important;
  z-index: 2147483647 !important; background: #000 !important;
}
#movie_player .html5-video-container,
#movie_player video.html5-main-video {
  top: 0 !important; left: 0 !important;
  width: 100% !important; height: 100% !important; object-fit: contain !important;
}
"#;

/// Initialization script for the guide window: runs at the start of every
/// top-level page load, before YouTube's own scripts.
///
/// - Only on `www.youtube.com`. Consent pages (`consent.youtube.com`,
///   `consent.google.com`) render untouched, so their buttons stay reachable.
/// - Keeps the window on the guide video. YouTube moves between pages without
///   reloading (logo, channel name, end-screen videos), which no navigation
///   handler sees, so the script watches the address and sends any other page
///   back to the video. Without this the user would land on a stripped page
///   with no header, no scrolling and no way back.
/// - Turns YouTube's autoplay-next off. Otherwise, at the end, YouTube moves
///   on to another video, the line above sends it back, YouTube resumes the
///   guide at its end, and the cycle repeats. It only clicks the switch while
///   it is on: clicking player controls on every tick closes any menu the user
///   opens (quality, speed).
/// - Marks the page offline (`guide-offline`) while the connection is down,
///   so the window says so instead of showing a frozen frame.
fn guide_script() -> Result<String, String> {
    let Color(r, g, b, _) = GUIDE_BACKGROUND;
    let style = format!(":root {{ --guide-bg: #{r:02x}{g:02x}{b:02x}; }}\n{GUIDE_STYLE}");
    let style = serde_json::to_string(&style).map_err(|e| e.to_string())?;
    let home = serde_json::to_string(&guide_url()).map_err(|e| e.to_string())?;
    let video = serde_json::to_string(GUIDE_VIDEO_ID).map_err(|e| e.to_string())?;
    Ok(format!(
        r##"(() => {{
  if (location.hostname !== "www.youtube.com") return;
  const onGuide = () =>
    location.pathname === "/watch" &&
    new URLSearchParams(location.search).get("v") === {video};
  const s = document.createElement("style");
  s.textContent = {style};
  (document.head || document.documentElement).appendChild(s);
  const markOffline = () =>
    document.documentElement.classList.toggle("guide-offline", !navigator.onLine);
  addEventListener("online", markOffline);
  addEventListener("offline", markOffline);
  markOffline();
  setInterval(() => {{
    if (!onGuide()) return location.replace({home});
    const autonav = document.querySelector('#movie_player [aria-checked="true"].ytp-autonav-toggle-button');
    if (autonav) (autonav.closest("button") || autonav).click();
  }}, 400);
}})();"##
    ))
}

/// Shows and focuses a window, restoring it if minimized. False if the
/// window doesn't exist.
fn focus_window(app: &AppHandle, label: &str) -> bool {
    let Some(win) = app.get_webview_window(label) else {
        return false;
    };
    let _ = win.unminimize();
    let _ = win.show();
    let _ = win.set_focus();
    true
}

/// Shows and focuses the main window (tray click / "Open" menu item).
pub fn show_main(app: &AppHandle) {
    focus_window(app, MAIN_WINDOW);
}

/// Brings the video guide to the front if its window is already open.
pub fn focus_guide(app: &AppHandle) -> bool {
    focus_window(app, GUIDE_WINDOW)
}

/// Serializes the guide's "focus it, else build it" step. Two clicks (header
/// and Settings) can both pass the reachability check; without this the
/// second `build()` fails because the label is already taken.
static GUIDE_OPEN_LOCK: Mutex<()> = Mutex::new(());

/// Opens the video guide in its own window, or focuses it if already open.
///
/// The window shows a remote page, so it is deliberately left out of every
/// capability (`capabilities/default.json`): YouTube gets no IPC access.
/// Closing it destroys the window, which stops playback.
pub fn open_guide(app: &AppHandle) -> Result<(), String> {
    let _guard = GUIDE_OPEN_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    if focus_guide(app) {
        return Ok(());
    }
    let url = guide_url().parse().map_err(|e| format!("bad guide URL: {e}"))?;
    WebviewWindowBuilder::new(app, GUIDE_WINDOW, WebviewUrl::External(url))
        .initialization_script(&guide_script()?)
        .on_navigation(guide_may_load)
        // `target=_blank` links and `window.open` would otherwise open a
        // browser tab or a bare popup, depending on the platform.
        .on_new_window(|_, _| NewWindowResponse::Deny)
        .background_color(GUIDE_BACKGROUND)
        .title("Prayer Times — Video Guide")
        .inner_size(960.0, 540.0)
        .min_inner_size(480.0, 270.0)
        .center()
        .focused(true)
        .build()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Navigation rule for the guide window, applied to the page and its frames.
///
/// YouTube's page creates `about:blank`, `blob:` and similar frames, so every
/// non-web URL is allowed. Web URLs must be on YouTube, or one of the consent
/// pages EU visitors are redirected through; anything else (ad frames, Google
/// sign-in, outside links) is blocked. Nothing is handed to the system
/// browser: frames navigate too, and each one would open a stray browser tab.
fn guide_may_load(url: &tauri::Url) -> bool {
    if !matches!(url.scheme(), "http" | "https") {
        return true;
    }
    let host = url.host_str().unwrap_or("");
    host == "youtube.com" || host.ends_with(".youtube.com") || host == "consent.google.com"
}

#[cfg(test)]
mod tests {
    use super::*;

    fn may_load(url: &str) -> bool {
        guide_may_load(&url.parse().unwrap())
    }

    #[test]
    fn guide_window_allows_youtube_consent_and_internal_frames() {
        assert!(may_load("https://www.youtube.com/watch?v=35BldPFeyjc"));
        assert!(may_load("https://consent.youtube.com/m"));
        assert!(may_load("https://consent.google.com/ml"));
        assert!(may_load("about:blank"));
        assert!(may_load("blob:https://www.youtube.com/0b5c"));
    }

    #[test]
    fn guide_window_blocks_other_web_pages() {
        assert!(!may_load("https://notyoutube.com/"));
        assert!(!may_load("https://github.com/iamraihan/prayer-times"));
        assert!(!may_load("https://googleads.g.doubleclick.net/pagead/id"));
        assert!(!may_load("https://accounts.google.com/ServiceLogin"));
        assert!(!may_load("https://www.google.com/"));
    }

    #[test]
    fn guide_script_carries_the_background_and_video() {
        let script = guide_script().unwrap();
        assert!(script.contains("--guide-bg: #0d1418"));
        assert!(script.contains(r#""35BldPFeyjc""#));
        assert!(script.contains(r#""https://www.youtube.com/watch?v=35BldPFeyjc""#));
    }
}
