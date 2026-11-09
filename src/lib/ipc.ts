/**
 * Typed wrappers around the Tauri IPC commands in `src-tauri/src/commands.rs`.
 * Every mutating command returns the fresh `AppState` so the UI can apply it
 * immediately instead of waiting for the next backend tick.
 *
 * `Commands` is the one table of command name → argument object → result.
 * Argument keys must match the Rust parameter names (camelCase); payload and
 * result types come from the generated bindings, so they track the Rust DTOs.
 */
import { invoke } from "@tauri-apps/api/core";
import type {
  AppState,
  AsrSchool,
  Location,
  LocationPreview,
  Prayer,
  PreviewRequest,
  Settings,
} from "./types";

export type { PreviewRequest } from "./types";

type NoArgs = Record<string, never>;

interface Commands {
  get_state: [NoArgs, AppState];
  preview_location: [{ req: PreviewRequest }, LocationPreview];
  save_location: [{ location: Location; method: number; school: AsrSchool }, AppState];
  update_settings: [{ settings: Settings }, AppState];
  set_prayer_done: [{ prayer: Prayer; done: boolean }, AppState];
  alarm_done: [NoArgs, AppState];
  alarm_snooze: [{ minutes: number }, AppState];
  alarm_dismiss: [NoArgs, AppState];
  test_sound: [{ volume: number | null }, void];
  set_alarm_tone: [{ path: string }, AppState];
  reset_alarm_tone: [NoArgs, AppState];
  get_autostart: [NoArgs, boolean];
  set_autostart: [{ enabled: boolean }, boolean];
}

function call<K extends keyof Commands>(cmd: K, args: Commands[K][0]): Promise<Commands[K][1]> {
  return invoke<Commands[K][1]>(cmd, args);
}

const none: NoArgs = {};

export const api = {
  getState: () => call("get_state", none),

  previewLocation: (req: PreviewRequest) => call("preview_location", { req }),

  saveLocation: (location: Location, method: number, school: AsrSchool) =>
    call("save_location", { location, method, school }),

  updateSettings: (settings: Settings) => call("update_settings", { settings }),

  setPrayerDone: (prayer: Prayer, done: boolean) => call("set_prayer_done", { prayer, done }),

  alarmDone: () => call("alarm_done", none),

  alarmSnooze: (minutes: number) => call("alarm_snooze", { minutes }),

  alarmDismiss: () => call("alarm_dismiss", none),

  /** `volume` (0–1) previews an unsaved slider value; omit to use the saved one. */
  testSound: (volume?: number) => call("test_sound", { volume: volume ?? null }),

  setAlarmTone: (path: string) => call("set_alarm_tone", { path }),

  resetAlarmTone: () => call("reset_alarm_tone", none),

  getAutostart: () => call("get_autostart", none),

  setAutostart: (enabled: boolean) => call("set_autostart", { enabled }),
};
