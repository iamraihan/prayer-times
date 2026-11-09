/**
 * Typed wrappers around Tauri IPC commands defined in `src-tauri/src/commands.rs`.
 * Every mutating command returns the fresh `AppState` so the UI can apply it
 * immediately instead of waiting for the next backend tick.
 */
import { invoke } from "@tauri-apps/api/core";
import type { AppState, Location, LocationPreview, Prayer, Settings } from "./types";

export interface PreviewRequest {
  city: string;
  country: string;
  countryCode: string;
  method: number;
  school: number;
}

export const api = {
  getState: (): Promise<AppState> => invoke("get_state"),

  previewLocation: (req: PreviewRequest): Promise<LocationPreview> =>
    invoke("preview_location", { req }),

  saveLocation: (
    location: Location,
    method: number,
    school: number,
  ): Promise<AppState> => invoke("save_location", { location, method, school }),

  updateSettings: (settings: Settings): Promise<AppState> =>
    invoke("update_settings", { settings }),

  setPrayerDone: (prayer: Prayer, done: boolean): Promise<AppState> =>
    invoke("set_prayer_done", { prayer, done }),

  alarmDone: (): Promise<AppState> => invoke("alarm_done"),

  alarmSnooze: (minutes: number): Promise<AppState> =>
    invoke("alarm_snooze", { minutes }),

  alarmDismiss: (): Promise<AppState> => invoke("alarm_dismiss"),

  /** `volume` (0–1) previews an unsaved slider value; omit to use the saved one. */
  testSound: (volume?: number): Promise<void> =>
    invoke("test_sound", { volume: volume ?? null }),

  setAlarmTone: (path: string): Promise<AppState> =>
    invoke("set_alarm_tone", { path }),

  resetAlarmTone: (): Promise<AppState> => invoke("reset_alarm_tone"),

  getAutostart: (): Promise<boolean> => invoke("get_autostart"),

  setAutostart: (enabled: boolean): Promise<boolean> =>
    invoke("set_autostart", { enabled }),
};
