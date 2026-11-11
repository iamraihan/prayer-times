/**
 * Tour Steps: what each tip says and which control it points at. Rust
 * records which steps the user has seen (`AppState.seenTourSteps`); this
 * module decides which of the rest a tab's Tour shows.
 *
 * Keyed by the generated `TourStep` union, so `tsc` fails until every step
 * Rust knows has its text here. Key order is display order.
 */
import type { Settings, TourStep } from "./types";

export type TourName = "today" | "history" | "settings";

export interface StepDef {
  tour: TourName;
  /** `data-tour` value of the control to highlight; `null` = centered card. */
  target: string | null;
  title: string;
  description: string | ((s: Settings) => string);
  side?: "top" | "right" | "bottom" | "left" | undefined;
}

export const STEPS: Record<TourStep, StepDef> = {
  todayNextPrayer: {
    tour: "today",
    target: "next-prayer",
    title: "Your next prayer",
    description: (s) => `The countdown to the next prayer and how long its time lasts. ${alarmSummary(s)}`,
    side: "bottom",
  },
  todayPrayerList: {
    tour: "today",
    target: "prayer-list",
    title: "Mark prayers done",
    description:
      "Click a prayer once you've prayed it. Its remaining alarms stop for the day, and it's saved to your History.",
    side: "top",
  },
  todayTray: {
    tour: "today",
    target: null,
    title: "Alarms keep working",
    description:
      "Closing this window keeps Prayer Times running in the tray, so alarms still ring. Open it again from the tray icon.",
  },
  historyCalendar: {
    tour: "history",
    target: "history-header",
    title: "Your history",
    description: "Browse past months. Your streak counts the days with every prayer done.",
    side: "bottom",
  },
  historyEditDay: {
    tour: "history",
    target: "history-days",
    title: "Fix a past day",
    description: "Each day shows its prayers: filled is done, a ring is missed. Click a day to correct it.",
    side: "top",
  },
  historyBackup: {
    tour: "history",
    target: "history-backup",
    title: "Export and import",
    description:
      "Save your history to a file, and import it on a new computer. Importing adds to what's here; it never deletes.",
    side: "top",
  },
  settingsAlarms: {
    tour: "settings",
    target: "settings-alarms",
    title: "Choose your alarms",
    description:
      '"Get ready" rings before you pray, "Prayer time" when you pray, and "Don\'t miss it" before the time ends. Pick which prayers ring.',
    side: "top",
  },
  settingsSound: {
    tour: "settings",
    target: "settings-sound",
    title: "How it sounds",
    description: "Set the volume and press Play tone to hear the alarm.",
    side: "top",
  },
  settingsBackup: {
    tour: "settings",
    target: "settings-backup",
    title: "Full backup",
    description: "Export your settings and history together, or restore them from a backup file.",
    side: "top",
  },
};

const ORDER = Object.keys(STEPS) as TourStep[];

/** `tour`'s steps the user hasn't seen, in display order. */
export function pendingSteps(tour: TourName, seen: readonly TourStep[]): TourStep[] {
  return ORDER.filter((id) => STEPS[id].tour === tour && !seen.includes(id));
}

/** True when the user already saw part of `tour`, so what's left is new. */
export function isUpdateTour(tour: TourName, seen: readonly TourStep[]): boolean {
  return seen.some((id) => STEPS[id].tour === tour);
}

/** "Alarms will ring …" in the user's own settings. */
export function alarmSummary(settings: Settings): string {
  const parts: string[] = [];
  if (settings.getReadyEnabled) parts.push(`${settings.getReadyMinutes} minutes before each prayer`);
  if (settings.startAlarmEnabled) parts.push("when each prayer starts");
  if (settings.preEndWarningEnabled) {
    parts.push(`${settings.preEndWarningMinutes} minutes before its time ends, if you haven't marked it done`);
  }
  return parts.length === 0
    ? "All alarms are off right now."
    : `Alarms will ring ${parts.length > 1 ? `${parts.slice(0, -1).join(", ")} and ${parts[parts.length - 1] ?? ""}` : (parts[0] ?? "")}.`;
}
