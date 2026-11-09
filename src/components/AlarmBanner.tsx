import { BellRing } from "lucide-react";
import { useAppState, useNow } from "../store/useAppStore";
import { AlarmActions, plainTitle } from "./AlarmActions";
import { countdown } from "../lib/time";

/** In-app banner shown while an alarm is active (same actions as the popup). */
export function AlarmBanner() {
  const state = useAppState();
  const now = useNow();
  const alarm = state?.activeAlarm;
  if (!alarm) return null;

  const done = state.doneToday[alarm.prayer] === true;
  const endMs = Date.parse(alarm.endsAt);
  const endsIn = isNaN(endMs) ? null : countdown(endMs - now);

  return (
    <section
      role="alert"
      className="fade-in rounded-xl border border-emerald-400/25 bg-slate-900/80 p-5 shadow-[0_12px_32px_-12px_rgb(0_0_0/0.6)]"
    >
      <div className="mb-5 flex items-start gap-3.5">
        <span className="flex h-10 w-10 shrink-0 items-center justify-center rounded-full bg-emerald-400/10 text-emerald-300">
          <BellRing className="bell-ring h-5 w-5" aria-hidden />
        </span>
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-baseline justify-between gap-x-4">
            <h2 className="font-semibold text-slate-50">{plainTitle(alarm.message.title)}</h2>
            {endsIn && (
              <span className="tnum text-[13px] text-slate-400">ends in {endsIn}</span>
            )}
          </div>
          <p className="mt-1 text-sm leading-relaxed text-slate-300">{alarm.message.body}</p>
        </div>
      </div>
      <AlarmActions done={done} />
    </section>
  );
}
