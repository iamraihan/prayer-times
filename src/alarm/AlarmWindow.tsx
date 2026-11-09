import { useEffect } from "react";
import { BellRing } from "lucide-react";
import { AlarmActions, beforeWindowStart, plainTitle } from "../components/AlarmActions";
import { PrayerIcon } from "../components/PrayerIcon";
import { startClock, useAppStore } from "../store/useAppStore";
import { PRAYER_LABELS } from "../lib/types";
import { countdown } from "../lib/time";

/**
 * Content of the always-on-top alarm popup window. The Rust backend shows and
 * hides this window; here we render the active alarm's message, the prayer's
 * done status, a live countdown to the window end, and the actions.
 */
export function AlarmWindow() {
  const init = useAppStore((s) => s.init);
  const alarm = useAppStore((s) => s.state?.activeAlarm ?? null);
  const done = useAppStore((s) => {
    const prayer = s.state?.activeAlarm?.prayer;
    return prayer ? (s.state?.doneToday ?? {})[prayer] === true : false;
  });
  const now = useAppStore((s) => s.now);
  const state = useAppStore((s) => s.state);

  useEffect(() => {
    startClock();
    void init();
  }, [init]);

  if (!alarm) {
    // Window is hidden by the backend when no alarm is active.
    return null;
  }

  const early = alarm.kind === "getReady";
  const targetMs = Date.parse(early ? alarm.prayAt : alarm.endsAt);
  const endsIn = isNaN(targetMs) ? null : countdown(targetMs - now);

  return (
    <div className="flex h-screen flex-col justify-center bg-slate-950 px-6 py-8">
      <div className="flex items-center gap-3">
        <span className="flex h-11 w-11 shrink-0 items-center justify-center rounded-full bg-emerald-400/10 text-emerald-300">
          <BellRing className="bell-ring h-5 w-5" aria-hidden />
        </span>
        <div className="min-w-0">
          <p className="flex items-center gap-1.5 text-[13px] text-slate-400">
            <PrayerIcon prayer={alarm.prayer} className="h-3.5 w-3.5" />
            {PRAYER_LABELS[alarm.prayer]}
            {endsIn && <span className="tnum">{early ? " in " : " · ends in "}{endsIn}</span>}
          </p>
          <h1 className="text-lg font-semibold leading-snug text-slate-50">
            {plainTitle(alarm.message.title)}
          </h1>
        </div>
      </div>

      <p className="mt-4 text-[15px] leading-relaxed text-slate-300">{alarm.message.body}</p>

      <div className="mt-8">
        <AlarmActions
          done={done}
          kind={alarm.kind}
          beforeStart={beforeWindowStart(state, alarm.prayer, now)}
        />
      </div>
    </div>
  );
}
