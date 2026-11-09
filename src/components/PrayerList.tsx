import type { ReactNode } from "react";
import { Check, Sunrise } from "lucide-react";
import { useAppState, useNow } from "../store/useAppStore";
import { PRAYERS, PRAYER_LABELS, type Prayer, type PrayerTime } from "../lib/types";
import { fmtHM, parseHM, type TimeFormat } from "../lib/time";
import { setPrayerDone } from "../lib/actions";
import { PrayerIcon } from "./PrayerIcon";
import { cn } from "../lib/cn";

type Status = "done" | "current" | "past" | "upcoming";

/**
 * Today's five prayers with start/end times, live status and the
 * "prayer done" checkbox that silences the remaining alarms for that prayer.
 */
export function PrayerList() {
  const state = useAppState();
  const now = useNow();

  if (!state?.today) {
    return (
      <div className="rounded-xl border border-slate-800/80 bg-slate-900/50 px-5 py-8 text-center text-sm text-slate-400">
        Waiting for today's times…
      </div>
    );
  }

  const mode: TimeFormat = state.settings.timeFormat;
  const today = state.today;
  const nextPrayer = state.next?.prayer;
  const done = state.doneToday;
  const times = PRAYERS.flatMap((p) => today.timings.filter((t) => t.prayer === p));

  return (
    <section aria-label="Today's prayers">
      <ul className="divide-y divide-slate-800/70 overflow-hidden rounded-xl border border-slate-800/80 bg-slate-900/50">
        {times.map((t) => (
          <PrayerRow
            key={t.prayer}
            time={t}
            mode={mode}
            done={Boolean(done[t.prayer])}
            isNext={nextPrayer === t.prayer}
            now={now}
            // Sunrise is where Fajr's window ends, so it sits right below it.
            after={t.prayer === "fajr" ? <SunriseRow time={fmtHM(today.sunrise, mode)} /> : null}
          />
        ))}
      </ul>
      <p className="mt-2.5 px-1 text-[12px] leading-relaxed text-slate-500">
        Each prayer's time ends when the next begins — Fajr at sunrise, Isha at midnight.
      </p>
    </section>
  );
}

interface PrayerRowProps {
  time: PrayerTime;
  mode: TimeFormat;
  done: boolean;
  isNext: boolean;
  now: number;
  after: ReactNode;
}

function PrayerRow({ time, mode, done, isNext, now, after }: PrayerRowProps) {
  const prayer: Prayer = time.prayer;
  // The backend sends each window as exact instants in the location's
  // timezone, so this works when the device is in another zone and when
  // Isha's window runs past midnight. Fall back to same-day wall-clock math
  // for older snapshots without them.
  const startMs = time.startAt ? Date.parse(time.startAt) : wallClockToday(now, time.start);
  const endMs = time.endsAt ? Date.parse(time.endsAt) : wallClockToday(now, time.ends);
  const started = now >= startMs;
  const inWindow = started && now < endMs;

  const status: Status = done ? "done" : inWindow ? "current" : started ? "past" : "upcoming";

  return (
    <>
      <li
        className={cn(
          "flex items-center gap-4 px-4 py-3.5 transition-colors",
          status === "current" && "bg-emerald-400/[0.05]",
          isNext && status === "upcoming" && "bg-slate-800/30",
        )}
        aria-current={status === "current" ? "time" : undefined}
      >
        <DoneCheckbox
          checked={done}
          // It can't have been prayed before it starts.
          disabledHint={!done && !started ? `You can mark it after it starts at ${fmtHM(time.start, mode)}` : null}
          label={`${PRAYER_LABELS[prayer]} prayed`}
          onChange={(v) => void setPrayerDone(prayer, v).catch(() => {})}
        />
        <PrayerIcon
          prayer={prayer}
          className={cn(
            "h-[18px] w-[18px] shrink-0",
            status === "current" ? "text-emerald-300" : "text-slate-500",
          )}
        />
        <div className="min-w-0 flex-1">
          <p
            className={cn(
              "font-medium",
              status === "done" && "text-slate-500 line-through decoration-slate-600",
              status === "current" && "text-emerald-200",
              status === "past" && "text-slate-300",
              status === "upcoming" && "text-slate-100",
            )}
          >
            {PRAYER_LABELS[prayer]}
          </p>
          <p className="tnum mt-0.5 text-[13px] text-slate-500">
            until {fmtHM(time.ends, mode)}
            {status === "current" && <span className="text-emerald-300/90"> · now</span>}
            {status === "past" && <span className="text-slate-400"> · time ended</span>}
          </p>
        </div>
        <span
          className={cn(
            "tnum text-[15px] font-medium",
            status === "done" ? "text-slate-500" : "text-slate-100",
            status === "current" && "text-emerald-200",
          )}
        >
          {fmtHM(time.start, mode)}
        </span>
      </li>
      {after}
    </>
  );
}

function SunriseRow({ time }: { time: string }) {
  return (
    <li className="flex items-center gap-4 px-4 py-2 text-[13px] text-slate-500">
      {/* Checkbox-column spacer keeps the icon and name columns aligned. */}
      <span className="w-5 shrink-0" aria-hidden />
      <Sunrise className="h-[18px] w-[18px] shrink-0" strokeWidth={1.75} aria-hidden />
      <span className="flex-1">Sunrise</span>
      <span className="tnum">{time}</span>
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
  disabledHint,
  label,
  onChange,
}: {
  checked: boolean;
  /** Set (with the reason) while the prayer can't be marked yet. */
  disabledHint: string | null;
  label: string;
  onChange: (v: boolean) => void;
}) {
  const disabled = disabledHint !== null;
  return (
    <button
      type="button"
      role="checkbox"
      aria-checked={checked}
      aria-label={disabled ? `${label}: ${disabledHint}` : label}
      aria-disabled={disabled}
      disabled={disabled}
      title={disabledHint ?? (checked ? "Marked as prayed" : "Mark as prayed")}
      onClick={() => onChange(!checked)}
      className={cn(
        "flex h-5 w-5 shrink-0 items-center justify-center rounded-[6px] border transition-colors duration-150",
        checked
          ? "border-emerald-600 bg-emerald-600 text-white"
          : disabled
            ? "cursor-not-allowed border-slate-700 border-dashed"
            : "border-slate-600 hover:border-emerald-400",
      )}
    >
      {checked && <Check className="h-3.5 w-3.5" strokeWidth={3} aria-hidden />}
    </button>
  );
}
