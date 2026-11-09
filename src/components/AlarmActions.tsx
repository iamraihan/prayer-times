import { useState } from "react";
import { Check } from "lucide-react";
import { Button } from "./ui/Button";
import { alarmDismiss, alarmDone, alarmSnooze } from "../lib/actions";
import { cn } from "../lib/cn";
import type { AlarmKind, AppState, Prayer } from "../lib/types";

const SNOOZE_PRESETS = [5, 10, 15, 30];

/**
 * Actions available while an alarm is active:
 * mark done (silences this prayer for the rest of the day),
 * remind-me-later (snooze N minutes, with a custom input),
 * or dismiss (stop the sound; later reminders still fire).
 *
 * The primary action never records a prayer the user hasn't prayed yet:
 * - before the window opens, only "Got it" (it can't be done yet);
 * - on a "Get ready" inside the window, "Got it" leads and "Mark as done"
 *   is a quieter second choice (for someone who already prayed alone);
 * - otherwise "Mark as done" leads.
 *
 * Shared by the in-app banner and the standalone alarm popup window.
 */
export function AlarmActions({
  done,
  kind,
  beforeStart,
}: {
  done: boolean;
  kind: AlarmKind;
  beforeStart: boolean;
}) {
  const [minutes, setMinutes] = useState(20);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const customValid = Number.isFinite(minutes) && minutes >= 1 && minutes <= 720;

  async function run(fn: () => Promise<void>, failure: string) {
    if (busy) return;
    setBusy(true);
    setError(null);
    try {
      await fn();
    } catch {
      setError(failure);
      setBusy(false);
    }
  }

  const gotIt = (
    <Button
      variant="primary"
      size="lg"
      className="flex-1"
      disabled={busy}
      onClick={() => void run(alarmDismiss, "Couldn't stop the alarm. Try again.")}
    >
      <Check className="h-[18px] w-[18px]" aria-hidden />
      Got it
    </Button>
  );
  const markDone = (primary: boolean) => (
    <Button
      variant={primary ? "primary" : "ghost"}
      size="lg"
      className={cn(primary && "flex-1")}
      disabled={busy || done}
      onClick={() => void run(alarmDone, "Couldn't mark it as done. Try again.")}
    >
      {primary && <Check className="h-[18px] w-[18px]" aria-hidden />}
      {done ? "Marked as done" : "Mark as done"}
    </Button>
  );

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center gap-2">
        {beforeStart ? (
          gotIt
        ) : kind === "getReady" ? (
          <>
            {gotIt}
            {markDone(false)}
          </>
        ) : (
          <>
            {markDone(true)}
            <Button
              variant="ghost"
              size="lg"
              disabled={busy}
              onClick={() => void run(alarmDismiss, "Couldn't stop the alarm. Try again.")}
            >
              Dismiss
            </Button>
          </>
        )}
      </div>

      <div className="space-y-2">
        <p className="text-[13px] text-slate-400">Remind me in</p>
        <div className="flex flex-wrap items-center gap-2">
          <div
            role="group"
            aria-label="Snooze presets"
            className="flex overflow-hidden rounded-lg border border-slate-700 bg-slate-950/40"
          >
            {SNOOZE_PRESETS.map((m, i) => (
              <button
                key={m}
                type="button"
                disabled={busy}
                onClick={() => void run(() => alarmSnooze(m), "Couldn't snooze. Try again.")}
                className={cn(
                  "tnum h-8 px-3 text-[13px] text-slate-200 transition-colors hover:bg-slate-800 disabled:opacity-45",
                  i > 0 && "border-l border-slate-700",
                )}
                aria-label={`Remind me in ${m} minutes`}
              >
                {m} min
              </button>
            ))}
          </div>
          <form
            className="flex items-center gap-1.5"
            onSubmit={(e) => {
              e.preventDefault();
              if (customValid) void run(() => alarmSnooze(minutes), "Couldn't snooze. Try again.");
            }}
          >
            <div className="relative">
              <input
                type="number"
                min={1}
                max={720}
                value={minutes}
                onChange={(e) => setMinutes(Number(e.target.value))}
                className="tnum h-8 w-20 rounded-lg border border-slate-700 bg-slate-950/40 pr-10 pl-2 text-right text-[13px] text-slate-100 focus:border-emerald-500 focus:outline-none focus-visible:ring-2 focus-visible:ring-emerald-500/30"
                aria-label="Custom snooze minutes"
              />
              <span className="pointer-events-none absolute top-1/2 right-3 -translate-y-1/2 text-[13px] text-slate-500">
                min
              </span>
            </div>
            <Button type="submit" size="sm" disabled={busy || !customValid}>
              Snooze
            </Button>
          </form>
        </div>
      </div>

      {error && (
        <p role="alert" className="text-[13px] text-red-300">
          {error}
        </p>
      )}
    </div>
  );
}

/** Whether `prayer`'s window has not opened yet at `now` (from the
 * snapshot's RFC3339 start instant). Unknown counts as started. */
export function beforeWindowStart(state: AppState | null, prayer: Prayer, now: number): boolean {
  const startAt = state?.today?.timings.find((t) => t.prayer === prayer)?.startAt;
  const ms = startAt ? Date.parse(startAt) : NaN;
  return !isNaN(ms) && now < ms;
}

/** Message titles are built for OS notifications and may lead with an emoji;
 * the in-app surfaces draw their own icon. */
export function plainTitle(title: string): string {
  return title.replace(/^[^\p{L}\p{N}]+/u, "");
}
