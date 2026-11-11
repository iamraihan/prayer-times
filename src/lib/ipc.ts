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
  AlarmKind,
  AppState,
  AsrSchool,
  BackupPreview,
  HistoryMonth,
  HistoryStatus,
  CityMatch,
  Location,
  LocationPreview,
  Prayer,
  PreviewRequest,
  Settings,
  TourStep,
  TourStepStatus,
} from "./types";

export type { PreviewRequest } from "./types";

type NoArgs = Record<string, never>;

interface Commands {
  get_state: [NoArgs, AppState];
  search_cities: [{ query: string; countryCode: string }, CityMatch[]];
  preview_location: [{ req: PreviewRequest }, LocationPreview];
  save_location: [{ location: Location; method: number; school: AsrSchool }, AppState];
  update_settings: [{ settings: Settings }, AppState];
  set_prayer_done: [{ prayer: Prayer; done: boolean }, AppState];
  get_history_month: [{ year: number; month: number }, HistoryMonth];
  set_history_entry: [{ day: string; prayer: Prayer; status: HistoryStatus | null }, HistoryMonth];
  export_backup: [{ path: string }, void];
  preview_backup: [{ path: string }, BackupPreview];
  import_backup: [{ path: string; includeSettings: boolean }, AppState];
  mark_tour_steps: [{ steps: TourStep[]; status: TourStepStatus }, AppState];
  reset_tours: [NoArgs, AppState];
  alarm_done: [NoArgs, AppState];
  alarm_snooze: [{ minutes: number }, AppState];
  alarm_dismiss: [NoArgs, AppState];
  /** Development builds only; release builds reject it. */
  preview_alarm: [{ kind: AlarmKind }, AppState];
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

  searchCities: (query: string, countryCode: string) =>
    call("search_cities", { query, countryCode }),

  previewLocation: (req: PreviewRequest) => call("preview_location", { req }),

  saveLocation: (location: Location, method: number, school: AsrSchool) =>
    call("save_location", { location, method, school }),

  updateSettings: (settings: Settings) => call("update_settings", { settings }),

  setPrayerDone: (prayer: Prayer, done: boolean) => call("set_prayer_done", { prayer, done }),

  alarmDone: () => call("alarm_done", none),

  alarmSnooze: (minutes: number) => call("alarm_snooze", { minutes }),

  alarmDismiss: () => call("alarm_dismiss", none),

  getHistoryMonth: (year: number, month: number) => call("get_history_month", { year, month }),

  setHistoryEntry: (day: string, prayer: Prayer, status: HistoryStatus | null) =>
    call("set_history_entry", { day, prayer, status }),

  exportBackup: (path: string) => call("export_backup", { path }),

  previewBackup: (path: string) => call("preview_backup", { path }),

  importBackup: (path: string, includeSettings: boolean) =>
    call("import_backup", { path, includeSettings }),

  markTourSteps: (steps: TourStep[], status: TourStepStatus) =>
    call("mark_tour_steps", { steps, status }),

  resetTours: () => call("reset_tours", none),

  previewAlarm: (kind: AlarmKind) => call("preview_alarm", { kind }),

  /** `volume` (0–1) previews an unsaved slider value; omit to use the saved one. */
  testSound: (volume?: number) => call("test_sound", { volume: volume ?? null }),

  setAlarmTone: (path: string) => call("set_alarm_tone", { path }),

  resetAlarmTone: () => call("reset_alarm_tone", none),

  getAutostart: () => call("get_autostart", none),

  setAutostart: (enabled: boolean) => call("set_autostart", { enabled }),
};
