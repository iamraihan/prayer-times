import { CheckCheck, CloudOff, Loader2 } from "lucide-react";
import type { ReactNode } from "react";
import { useAppState, useNow } from "../store/useAppStore";
import { PRAYER_LABELS } from "../lib/types";
import { countdown, fmtInstant } from "../lib/time";
import { PrayerIcon } from "./PrayerIcon";
import { cn } from "../lib/cn";

/**
 * The featured prayer: the open window (countdown to its end) or the next
 * one to begin (countdown to its start).
 */
export function NextPrayerCard() {
  const state = useAppState();
  const now = useNow();

  if (!state?.today) {
    return (
      <Shell>
        <Quiet
          icon={
            state?.apiError ? (
              <CloudOff className="h-5 w-5" aria-hidden />
            ) : (
              <Loader2 className="h-5 w-5 animate-spin" aria-hidden />
            )
          }
          title={state?.apiError ? "Can't reach the prayer times service" : "Loading today's times…"}
          detail={state?.apiError ? "Retrying in the background." : undefined}
        />
      </Shell>
    );
  }

  const next = state.next;
  if (!next) {
    return (
      <Shell>
        <Quiet
          icon={<CheckCheck className="h-5 w-5 text-emerald-400" aria-hidden />}
          title="All of today's prayers are done"
          detail="Tomorrow's times appear after Isha ends."
        />
      </Shell>
    );
  }

  const mode = state.settings.timeFormat;
  const startMs = Date.parse(next.startAt);
  const endMs = Date.parse(next.endsAt);
  const isCurrent = now >= startMs && now < endMs;
  const remaining = countdown((isCurrent ? endMs : startMs) - now);
  const progress = isCurrent
    ? Math.min(1, Math.max(0, (now - startMs) / Math.max(1, endMs - startMs)))
    : 0;
  const label = PRAYER_LABELS[next.prayer];

  return (
    <Shell current={isCurrent}>
      <div className="flex items-center gap-4 p-5 sm:p-6">
        <span
          className={cn(
            "flex h-12 w-12 shrink-0 items-center justify-center rounded-full",
            isCurrent ? "bg-emerald-400/10 text-emerald-300" : "bg-slate-800 text-slate-300",
          )}
        >
          <PrayerIcon prayer={next.prayer} className="h-6 w-6" />
        </span>
        <div className="min-w-0 flex-1">
          <h2 className="text-2xl font-semibold tracking-[-0.015em] text-slate-50">{label}</h2>
          <p className="tnum mt-0.5 text-sm text-slate-400">
            {fmtInstant(next.startAt, mode)} – {fmtInstant(next.endsAt, mode)}
          </p>
        </div>
        <div className="text-right" aria-live="off">
          <p
            className={cn(
              "tnum text-2xl font-semibold tracking-[-0.015em]",
              isCurrent ? "text-emerald-300" : "text-slate-50",
            )}
          >
            {remaining}
          </p>
          <p className="mt-0.5 text-[13px] text-slate-400">
            {isCurrent ? "left to pray" : `until ${label}`}
          </p>
        </div>
      </div>
      {isCurrent && (
        <div
          className="h-[3px] bg-slate-800"
          role="progressbar"
          aria-label={`${label} window elapsed`}
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={Math.round(progress * 100)}
        >
          <div
            className="h-full bg-emerald-400/80 transition-[width] duration-1000 ease-linear"
            style={{ width: `${progress * 100}%` }}
          />
        </div>
      )}
    </Shell>
  );
}

function Shell({ current, children }: { current?: boolean; children: ReactNode }) {
  return (
    <section
      className={cn(
        "overflow-hidden rounded-xl border bg-slate-900/70",
        current ? "border-emerald-400/20" : "border-slate-800/80",
      )}
    >
      {children}
    </section>
  );
}

function Quiet({ icon, title, detail }: { icon: ReactNode; title: string; detail?: string | undefined }) {
  return (
    <div className="flex items-center gap-4 p-5 sm:p-6">
      <span className="flex h-12 w-12 shrink-0 items-center justify-center rounded-full bg-slate-800 text-slate-400">
        {icon}
      </span>
      <div>
        <p className="font-medium text-slate-100">{title}</p>
        {detail && <p className="mt-0.5 text-[13px] text-slate-400">{detail}</p>}
      </div>
    </div>
  );
}
