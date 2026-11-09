import { CloudOff, MapPin, TriangleAlert } from "lucide-react";
import { useAppState } from "../store/useAppStore";
import { NextPrayerCard } from "../components/NextPrayerCard";
import { PrayerList } from "../components/PrayerList";
import { AlarmBanner } from "../components/AlarmBanner";
import { methodName } from "../lib/methods";
import { PRAYER_LABELS, type Prayer } from "../lib/types";
import { fmtClock } from "../lib/time";

export function TodayPage() {
  const state = useAppState();
  const today = state?.today;
  const location = state?.settings.location;
  const mode = state?.settings.timeFormat ?? "12h";
  const snoozes = Object.entries(state?.snoozes ?? {}) as [Prayer, string][];

  return (
    <div className="space-y-5">
      <header className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h1 className="text-xl font-semibold">
            {today ? today.gregorian : "Today"}
          </h1>
          <p className="text-sm text-slate-400">
            {today ? today.hijri : "…"}
          </p>
        </div>
        <div className="flex items-center gap-2 text-sm text-slate-400">
          {location && (
            <span className="flex items-center gap-1.5 rounded-full border border-slate-800 bg-slate-900/70 px-3 py-1">
              <MapPin className="h-3.5 w-3.5 text-emerald-400" />
              {location.city}, {location.country}
            </span>
          )}
          {(today?.fromCache || state?.apiError) && (
            <span
              className="flex items-center gap-1.5 rounded-full border border-amber-900/60 bg-amber-950/40 px-3 py-1 text-amber-400"
              title={state?.apiError ?? undefined}
            >
              <CloudOff className="h-3.5 w-3.5" />
              offline · cached times
            </span>
          )}
        </div>
      </header>

      {state?.locationWarning && (
        <div
          role="alert"
          className="flex items-start gap-3 rounded-xl border border-amber-900/60 bg-amber-950/40 px-4 py-3 text-sm text-amber-300"
        >
          <TriangleAlert className="mt-0.5 h-4 w-4 shrink-0" />
          <p>{state.locationWarning}</p>
        </div>
      )}

      {snoozes.length > 0 && (
        <div className="flex flex-wrap gap-2">
          {snoozes.map(([prayer, at]) => (
            <span
              key={prayer}
              className="rounded-full border border-amber-800/60 bg-amber-950/30 px-3 py-1 text-xs text-amber-300"
              title={`Snoozed — the alarm popup will show again at this time`}
            >
              🔔 {PRAYER_LABELS[prayer]} reminder at {fmtClock(at, mode)}
            </span>
          ))}
        </div>
      )}

      <AlarmBanner />
      <NextPrayerCard />
      <PrayerList />

      <p className="pb-2 text-center text-xs text-slate-600">
        {location
          ? `${methodName(state?.settings.method ?? 3)} · ${
              state?.settings.school === 1 ? "Hanafi Asr" : "Standard Asr"
            } · times for ${location.timezone}`
          : ""}
      </p>
    </div>
  );
}
