import { AlarmClock, CloudOff, TriangleAlert } from "lucide-react";
import { useAppState } from "../store/useAppStore";
import { NextPrayerCard } from "../components/NextPrayerCard";
import { PrayerList } from "../components/PrayerList";
import { AlarmBanner } from "../components/AlarmBanner";
import { methodName } from "../lib/methods";
import { PRAYER_LABELS, type Prayer } from "../lib/types";
import { fmtClockIn, fmtLongDate } from "../lib/time";

export function TodayPage() {
  const state = useAppState();
  const today = state?.today;
  const location = state?.settings.location;
  const mode = state?.settings.timeFormat ?? "12h";
  const snoozes = Object.entries(state?.snoozes ?? {}) as [Prayer, string][];
  const offline = Boolean(today?.fromCache || state?.apiError);

  return (
    <div className="space-y-6">
      <header className="flex flex-wrap items-end justify-between gap-x-4 gap-y-2 px-1">
        <div>
          <h1 className="text-xl font-semibold tracking-[-0.01em] text-slate-50">
            {today ? fmtLongDate(today.date) : "Today"}
          </h1>
          <p className="mt-0.5 text-sm text-slate-400">{today ? today.hijri : " "}</p>
        </div>
        {offline && (
          <p
            className="flex items-center gap-1.5 text-[13px] text-amber-300/90"
            title={state?.apiError ?? undefined}
          >
            <CloudOff className="h-3.5 w-3.5" aria-hidden />
            Offline — showing saved times
          </p>
        )}
      </header>

      {state?.locationWarning && (
        <div
          role="alert"
          className="flex items-start gap-3 rounded-xl border border-amber-500/25 bg-amber-500/[0.07] px-4 py-3 text-sm leading-relaxed text-amber-200"
        >
          <TriangleAlert className="mt-0.5 h-4 w-4 shrink-0 text-amber-400" aria-hidden />
          <p>{state.locationWarning}</p>
        </div>
      )}

      <AlarmBanner />
      <NextPrayerCard />

      {snoozes.length > 0 && location && (
        <ul className="space-y-1 px-1">
          {snoozes.map(([prayer, at]) => (
            <li key={prayer} className="flex items-center gap-2 text-[13px] text-slate-400">
              <AlarmClock className="h-3.5 w-3.5 text-amber-400" aria-hidden />
              {PRAYER_LABELS[prayer]} reminder snoozed until{" "}
              <span className="tnum text-slate-200">{fmtClockIn(at, location.timezone, mode)}</span>
            </li>
          ))}
        </ul>
      )}

      <PrayerList />

      {location && state && (
        <p className="px-1 pb-2 text-center text-[12px] text-slate-500">
          {methodName(state.settings.method)} ·{" "}
          {state.settings.school === 1 ? "Hanafi Asr" : "Standard Asr"} · {location.timezone}
        </p>
      )}
    </div>
  );
}
