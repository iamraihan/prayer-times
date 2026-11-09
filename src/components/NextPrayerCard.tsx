import { CalendarClock, Hourglass } from "lucide-react";
import { useAppState, useNow } from "../store/useAppStore";
import { PRAYER_EMOJIS, PRAYER_LABELS } from "../lib/types";
import { countdown, fmtHM, type TimeFormat } from "../lib/time";

/**
 * The hero card: current prayer window (countdown to its end) or the next
 * upcoming prayer (countdown to its start).
 */
export function NextPrayerCard() {
  const state = useAppState();
  const now = useNow();

  if (!state?.today) {
    return (
      <div className="rounded-2xl border border-slate-800 bg-slate-900/60 p-6 text-center">
        <p className="text-sm text-slate-400">
          {state?.apiError
            ? "Can't reach the prayer times service. Retrying in the background…"
            : "Loading today's prayer times…"}
        </p>
      </div>
    );
  }

  const next = state.next;
  if (!next) {
    return (
      <div className="rounded-2xl border border-emerald-800/50 bg-emerald-950/30 p-6 text-center">
        <p className="text-lg font-medium text-emerald-300">
          All prayers are done for today 🎉
        </p>
        <p className="mt-1 text-sm text-slate-400">
          Times for tomorrow will appear after midnight.
        </p>
      </div>
    );
  }

  const mode: TimeFormat = state.settings.timeFormat;
  const startMs = new Date(next.startAt).getTime();
  const endMs = new Date(next.endsAt).getTime();
  const isCurrent = now >= startMs && now < endMs;
  const targetMs = isCurrent ? endMs : startMs;
  const remaining = countdown(targetMs - now);

  // Progress through the current prayer window.
  const progress = isCurrent
    ? Math.min(1, Math.max(0, (now - startMs) / Math.max(1, endMs - startMs)))
    : 0;

  return (
    <div className="overflow-hidden rounded-2xl border border-slate-800 bg-gradient-to-br from-emerald-950/40 via-slate-900/60 to-slate-900/60">
      <div className="p-6">
        <p className="text-xs font-semibold uppercase tracking-wider text-emerald-400">
          {isCurrent ? "Current prayer" : "Next prayer"}
        </p>
        <div className="mt-2 flex flex-wrap items-end justify-between gap-4">
          <div className="flex items-center gap-3">
            <span className="text-4xl">{PRAYER_EMOJIS[next.prayer]}</span>
            <div>
              <h2 className="text-3xl font-semibold leading-tight">
                {PRAYER_LABELS[next.prayer]}
              </h2>
              <p className="mt-0.5 font-mono text-sm text-slate-300">
                starts {fmtHM(hmOfDay(next.startAt), mode)} · ends{" "}
                {fmtHM(hmOfDay(next.endsAt), mode)}
              </p>
            </div>
          </div>
          <div className="text-right">
            <p className="font-mono text-3xl font-semibold tabular-nums text-emerald-300">
              {remaining}
            </p>
            <p className="mt-0.5 flex items-center justify-end gap-1 text-xs text-slate-400">
              {isCurrent ? (
                <>
                  <Hourglass className="h-3.5 w-3.5" /> until it ends
                </>
              ) : (
                <>
                  <CalendarClock className="h-3.5 w-3.5" /> until it starts
                </>
              )}
            </p>
          </div>
        </div>
        {isCurrent && (
          <div className="mt-5 h-1.5 overflow-hidden rounded-full bg-slate-800">
            <div
              className="h-full rounded-full bg-emerald-500 transition-all"
              style={{ width: `${progress * 100}%` }}
            />
          </div>
        )}
      </div>
    </div>
  );
}

/** "HH:MM" for an ISO string in the location's timezone (already offset-encoded). */
/** "HH:MM" in the *location's* timezone: the backend's RFC3339 strings carry
 * the location offset ("2026-11-09T12:20:00+06:00"), so read the wall clock
 * straight from the string instead of converting to the device zone. */
function hmOfDay(iso: string): string {
  const m = /T(\d{2}):(\d{2})/.exec(iso);
  return m ? `${m[1]}:${m[2]}` : "--:--";
}
