import { useState } from "react";
import { Check } from "lucide-react";
import { Button } from "./ui/Button";
import { alarmDismiss, alarmDone, alarmSnooze } from "../lib/actions";
import { cn } from "../lib/cn";

const SNOOZE_PRESETS = [5, 10, 15, 30];

/**
 * Actions available while an alarm is active:
 * mark done (silences this prayer for the rest of the day),
 * remind-me-later (snooze N minutes, with a custom input),
 * or dismiss (stop the sound; later reminders still fire).
 *
 * Shared by the in-app banner and the standalone alarm popup window.
 */
export function AlarmActions({ done }: { done: boolean }) {
  const [minutes, setMinutes] = useState(10);
  const [busy, setBusy] = useState(false);
  const customValid = Number.isFinite(minutes) && minutes >= 1 && minutes <= 720;

  async function run(fn: () => Promise<void>) {
    if (busy) return;
    setBusy(true);
    try {
      await fn();
    } catch {
      setBusy(false);
    }
  }

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center gap-2">
        <Button
          variant="primary"
          size="lg"
          className="flex-1"
          disabled={busy || done}
          onClick={() => void run(alarmDone)}
        >
          <Check className="h-[18px] w-[18px]" aria-hidden />
          {done ? "Marked as done" : "Mark as done"}
        </Button>
        <Button variant="ghost" size="lg" disabled={busy} onClick={() => void run(alarmDismiss)}>
          Dismiss
        </Button>
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
              onClick={() => void run(() => alarmSnooze(m))}
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
            if (customValid) void run(() => alarmSnooze(minutes));
          }}
        >
          <div className="relative">
            <input
              type="number"
              min={1}
              max={720}
              value={minutes}
              onChange={(e) => setMinutes(Number(e.target.value))}
              className="tnum h-8 w-20 rounded-lg border border-slate-700 bg-slate-950/40 pr-10 pl-2 text-right text-[13px] text-slate-100 focus:border-emerald-500 focus:outline-none"
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
    </div>
  );
}

/** Message titles are built for OS notifications and may lead with an emoji;
 * the in-app surfaces draw their own icon. */
export function plainTitle(title: string): string {
  return title.replace(/^[^\p{L}\p{N}]+/u, "");
}
