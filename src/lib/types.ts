/**
 * Frontend view of the IPC types. The shapes themselves are generated from
 * the Rust DTOs into `./bindings/` by `cargo test` (ts-rs) — change the Rust
 * struct, rerun `cargo test`, and these follow. Only UI-only constants live
 * here.
 */
import type { Prayer } from "./bindings/Prayer";

export type { Prayer } from "./bindings/Prayer";
export type { AlarmKind } from "./bindings/AlarmKind";
export type { AsrSchool } from "./bindings/AsrSchool";
export type { TimeFormat } from "./bindings/TimeFormat";
export type { Location } from "./bindings/Location";
export type { Settings } from "./bindings/Settings";
export type { PreviewRequest } from "./bindings/PreviewRequest";
export type { PrayerSettings as PrayerReminderSettings } from "./bindings/PrayerSettings";
export type { PrayerTimeDto as PrayerTime } from "./bindings/PrayerTimeDto";
export type { DayTimingsDto as DayTimings } from "./bindings/DayTimingsDto";
export type { AlarmMessageDto as AlarmMessage } from "./bindings/AlarmMessageDto";
export type { ActiveAlarmDto as ActiveAlarm } from "./bindings/ActiveAlarmDto";
export type { NextPrayerDto as NextPrayerInfo } from "./bindings/NextPrayerDto";
export type { AppStateDto as AppState } from "./bindings/AppStateDto";
export type { LocationPreviewDto as LocationPreview } from "./bindings/LocationPreviewDto";

export const PRAYERS = ["fajr", "dhuhr", "asr", "maghrib", "isha"] as const satisfies readonly Prayer[];

export const PRAYER_LABELS: Record<Prayer, string> = {
  fajr: "Fajr",
  dhuhr: "Dhuhr",
  asr: "Asr",
  maghrib: "Maghrib",
  isha: "Isha",
};

export const PRAYER_EMOJIS: Record<Prayer, string> = {
  fajr: "🌅",
  dhuhr: "☀️",
  asr: "🌤️",
  maghrib: "🌇",
  isha: "🌙",
};
