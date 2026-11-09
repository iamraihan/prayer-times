import type { ReactNode } from "react";
import { Check, Sunrise } from "lucide-react";
import { useAppState, useNow } from "../store/useAppStore";
import {
  PRAYERS,
  PRAYER_EMOJIS,
  PRAYER_LABELS,
  type Prayer,
  type PrayerTime,
} from "../lib/types";
import { fmtHM, parseHM, type TimeFormat } from "../lib/time";
import { setPrayerDone } from "../lib/actions";
import { cn } from "../lib/cn";

/**
 * Today's five prayers with start/end times, live status and the
 * "prayer done" checkbox that silences the remaining alarms for that prayer.
 */
export function PrayerList() {
  const state = useAppState();
  const now = useNow();

  if (!state?.today) {
    return (
      <div className="rounded-2xl border border-slate-800 bg-slate-900/60 p-6 text-center text-sm text-slate-400">
        Waiting for today's times…
      </div>
    );
  }

  const mode: TimeFormat = state.settings.timeFormat;
  const today = state.today;
  const nextPrayer = state.next?.prayer;
  const done = state.doneToday ?? {};

  return (
    <div className="overflow-hidden rounded-2xl border border-slate-800 bg-slate-900/60">
      <ul className="divide-y divide-slate-800/80">
        {PRAYERS.flatMap((p) => {
          const t = today.timings.find((x) => x.prayer === p);
          if (!t) return [];
          return [t];
        }).flatMap((t: PrayerTime) => {
          const rows: ReactNode[] = [];
          if (t.prayer === "fajr") {
            rows.push(
              <li
                key="sunrise"
                className="flex items-center justify-between px-5 py-3 text-slate-500"
              >
                <span className="flex items-center gap-3 text-sm">
                  <Sunrise className="h-4 w-4" /> Sunrise
                </span>
                <span className="font-mono text-sm">{fmtHM(today.sunrise, mode)}</span>
              </li>,
            );
          }
          rows.push(
            <PrayerRow
              key={t.prayer}
              time={t}
              mode={mode}
              done={Boolean(done[t.prayer])}
              isNext={nextPrayer === t.prayer}
              now={now}
            />,
          );
          return rows;
        })}
      </ul>
      <p className="border-t border-slate-800/80 px-5 py-2.5 text-xs text-slate-500">
        "Ends" is when the prayer's time window closes (Fajr at sunrise, Isha at
        midnight).
      </p>
    </div>
  );
}

interface PrayerRowProps {
  time: PrayerTime;
  mode: TimeFormat;
  done: boolean;
  isNext: boolean;
  now: number;
}

function PrayerRow({ time, mode, done, isNext, now }: PrayerRowProps) {
  const prayer: Prayer = time.prayer;
  // The backend sends each window as exact instants in the location's
  // timezone, so this works when the device is in another zone and when
  // Isha's window runs past midnight. Fall back to same-day wall-clock math
  // for older snapshots without them.
  const startMs = time.startAt ? Date.parse(time.startAt) : wallClockToday(now, time.start);
  const endMs = time.endsAt ? Date.parse(time.endsAt) : wallClockToday(now, time.ends);
  const started = now >= startMs;
  const inWindow = started && now < endMs;

  const status: "done" | "current" | "past" | "upcoming" = done
    ? "done"
    : inWindow
      ? "current"
      : started
        ? "past"
        : "upcoming";

  return (
    <li
      className={cn(
        "flex items-center justify-between gap-4 px-5 py-3.5",
        status === "current" && "bg-emerald-950/20",
        isNext && status !== "done" && "bg-slate-800/30",
      )}
    >
      <div className="flex min-w-0 items-center gap-4">
        <DoneCheckbox
          checked={done}
          onChange={(v) => void setPrayerDone(prayer, v)}
        />
        <div className="min-w-0">
          <p
            className={cn(
              "flex items-center gap-2 font-medium",
              status === "done" && "text-slate-500 line-through",
              status === "past" && !done && "text-slate-400",
              status !== "done" && status !== "past" && "text-slate-100",
            )}
          >
            <span>{PRAYER_EMOJIS[prayer]}</span>
            {PRAYER_LABELS[prayer]}
            {status === "current" && (
              <span className="rounded-full bg-emerald-600/20 px-2 py-0.5 text-[10px] font-semibold uppercase tracking-wide text-emerald-400">
                now
              </span>
            )}
            {status === "past" && !done && (
              <span className="rounded-full bg-amber-600/15 px-2 py-0.5 text-[10px] font-semibold uppercase tracking-wide text-amber-400">
                passed
              </span>
            )}
          </p>
          <p className="mt-0.5 font-mono text-xs text-slate-500">
            ends {fmtHM(time.ends, mode)}
          </p>
        </div>
      </div>
      <span className="font-mono text-lg tabular-nums">
        {fmtHM(time.start, mode)}
      </span>
    </li>
  );
}

function wallClockToday(now: number, hm: string): number {
  const t = parseHM(hm);
  if (!t) return NaN;
  const d = new Date(now);
  return new Date(d.getFullYear(), d.getMonth(), d.getDate(), t.h, t.m).getTime();
}

function DoneCheckbox({
  checked,
  onChange,
}: {
  checked: boolean;
  onChange: (v: boolean) => void;
}) {
  return (
    <button
      type="button"
      role="checkbox"
      aria-checked={checked}
      aria-label="Mark prayer as done"
      onClick={() => onChange(!checked)}
      className={cn(
        "flex h-6 w-6 shrink-0 items-center justify-center rounded-md border transition-colors",
        "focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-emerald-400",
        checked
          ? "border-emerald-500 bg-emerald-600 text-white"
          : "border-slate-600 hover:border-emerald-500",
      )}
    >
      {checked && <Check className="h-4 w-4" />}
    </button>
  );
}
