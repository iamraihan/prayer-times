/**
 * UI actions — call an IPC command and apply the returned state to the store.
 * Keeping this in one module makes the data-flow easy to audit:
 * UI event -> action -> Rust -> new AppState -> store -> re-render.
 */
import { api, type PreviewRequest } from "./ipc";
import type { AlarmKind, AsrSchool, CityMatch, Location, LocationPreview, Prayer, Settings } from "./types";
import { useAppStore } from "../store/useAppStore";

function apply(s: Awaited<ReturnType<typeof api.getState>>): void {
  useAppStore.getState().apply(s);
}

export async function saveLocation(
  location: Location,
  method: number,
  school: AsrSchool,
): Promise<void> {
  apply(await api.saveLocation(location, method, school));
}

export async function updateSettings(settings: Settings): Promise<void> {
  apply(await api.updateSettings(settings));
}

export async function setPrayerDone(prayer: Prayer, done: boolean): Promise<void> {
  apply(await api.setPrayerDone(prayer, done));
}

export async function alarmDone(): Promise<void> {
  apply(await api.alarmDone());
}

export async function alarmSnooze(minutes: number): Promise<void> {
  apply(await api.alarmSnooze(minutes));
}

export async function alarmDismiss(): Promise<void> {
  apply(await api.alarmDismiss());
}

/** Dev builds only: shows a pretend alarm to check the popup design. */
export async function previewAlarm(kind: AlarmKind): Promise<void> {
  apply(await api.previewAlarm(kind));
}

export function searchCities(query: string, countryCode: string): Promise<CityMatch[]> {
  return api.searchCities(query, countryCode);
}

export function previewLocation(req: PreviewRequest): Promise<LocationPreview> {
  return api.previewLocation(req);
}

export function testSound(volume?: number): Promise<void> {
  return api.testSound(volume);
}

/** Picks a custom alarm tone; the file is validated + persisted by Rust. */
export async function setAlarmTone(path: string): Promise<void> {
  apply(await api.setAlarmTone(path));
}

export async function resetAlarmTone(): Promise<void> {
  apply(await api.resetAlarmTone());
}

export function getAutostart(): Promise<boolean> {
  return api.getAutostart();
}

export function setAutostart(enabled: boolean): Promise<boolean> {
  return api.setAutostart(enabled);
}
