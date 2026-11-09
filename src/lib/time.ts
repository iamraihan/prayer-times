import type { TimeFormat } from "./types";

export type { TimeFormat };

/** Parses a 24h "HH:MM" string; `null` when it isn't one. */
export function parseHM(hm: string): { h: number; m: number } | null {
  const match = /^(\d{1,2}):(\d{2})$/.exec(hm.trim());
  if (!match) return null;
  const h = Number(match[1]);
  const m = Number(match[2]);
  return h < 24 && m < 60 ? { h, m } : null;
}

/** Formats a "HH:MM" 24h string per the requested format. */
export function fmtHM(hm: string, mode: TimeFormat = "12h"): string {
  const t = parseHM(hm);
  return t ? fmtHourMinute(t.h, t.m, mode) : hm;
}

/** Formats a Date (or ISO string) as a wall-clock time per the requested format. */
export function fmtClock(value: Date | string, mode: TimeFormat = "12h"): string {
  const d = typeof value === "string" ? new Date(value) : value;
  if (isNaN(d.getTime())) return "--:--";
  return fmtHourMinute(d.getHours(), d.getMinutes(), mode);
}

function fmtHourMinute(h: number, m: number, mode: TimeFormat): string {
  const mm = String(m).padStart(2, "0");
  if (mode === "24h") return `${String(h).padStart(2, "0")}:${mm}`;
  const suffix = h < 12 ? "AM" : "PM";
  const h12 = h % 12 === 0 ? 12 : h % 12;
  return `${h12}:${mm} ${suffix}`;
}

/** Compact human countdown like "2h 14m", "14m 30s", "45s" or "ended". */
export function countdown(ms: number): string {
  if (ms <= 0) return "ended";
  const total = Math.floor(ms / 1000);
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  if (h > 0) return m > 0 ? `${h}h ${m}m` : `${h}h`;
  if (m > 0) return `${m}m ${s.toString().padStart(2, "0")}s`;
  return `${s}s`;
}

/** Long-form remaining time like "2 hours 14 minutes" for notification copy. */
export function remainingWords(ms: number): string {
  const total = Math.max(0, Math.floor(ms / 60000));
  const h = Math.floor(total / 60);
  const m = total % 60;
  if (h > 0 && m > 0) return `${h} hour${h > 1 ? "s" : ""} ${m} minute${m > 1 ? "s" : ""}`;
  if (h > 0) return `${h} hour${h > 1 ? "s" : ""}`;
  return `${m} minute${m === 1 ? "" : "s"}`;
}
