import { useCallback, useEffect, useRef, useState } from "react";
import { ChevronLeft, ChevronRight } from "lucide-react";
import { api } from "../lib/ipc";
import { cn } from "../lib/cn";
import { useAppState } from "../store/useAppStore";
import { BackupControls } from "../components/BackupControls";
import { Section } from "../components/ui/Section";
import { Segmented } from "../components/ui/Segmented";
import { IconButton } from "../components/ui/IconButton";
import {
  PRAYERS,
  PRAYER_LABELS,
  type HistoryDay,
  type HistoryMonth,
  type HistoryStatus,
  type Prayer,
} from "../lib/types";
import { friendlyError } from "../lib/errors";

const WEEKDAYS = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
const MONTHS = [
  "January", "February", "March", "April", "May", "June",
  "July", "August", "September", "October", "November", "December",
];

/** "2026-11-09" → [2026, 11, 9]; dates come from Rust as location dates. */
function ymd(day: string): [number, number, number] | null {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(day);
  return m ? [Number(m[1]), Number(m[2]), Number(m[3])] : null;
}

/** Today's date as `YYYY-MM-DD` in `timeZone` (the location's), not the
 * device's. */
function locationDate(timeZone: string | null): string {
  try {
    return new Intl.DateTimeFormat("en-CA", {
      timeZone: timeZone ?? undefined,
      year: "numeric",
      month: "2-digit",
      day: "2-digit",
    }).format(new Date());
  } catch {
    return new Date().toISOString().slice(0, 10);
  }
}

/** "2026-11-09" → "November 9, 2026" (for screen readers and summaries). */
function humanDate(day: string): string {
  const parts = ymd(day);
  return parts ? `${MONTHS[parts[1] - 1]} ${parts[2]}, ${parts[0]}` : day;
}

/** Weekday of a calendar date, independent of the device's timezone. */
function weekday(year: number, month: number, day: number): number {
  return new Date(Date.UTC(year, month - 1, day)).getUTCDay();
}

/**
 * History: a month of Prayer Days with each prayer's recorded status, the
 * current streak and this month's totals. Click a day to correct it.
 */
export function HistoryPage() {
  const state = useAppState();
  const todayStr = state?.today?.date ?? null;
  const doneToday = state?.doneToday;

  const [ym, setYm] = useState<[number, number] | null>(null);
  const [data, setData] = useState<HistoryMonth | null>(null);
  const [selected, setSelected] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const editorRef = useRef<HTMLDivElement>(null);

  // The editor sits below the calendar; bring it into view when a day is
  // picked so the click visibly does something.
  useEffect(() => {
    if (!selected) return;
    const reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    editorRef.current?.scrollIntoView({ block: "nearest", behavior: reduce ? "auto" : "smooth" });
  }, [selected]);

  const timezone = state?.settings.location?.timezone ?? null;

  // Start on the current Prayer Day's month; without a timetable (offline,
  // nothing cached) on today's date in the location's timezone.
  useEffect(() => {
    if (ym) return;
    const parts = ymd(todayStr ?? locationDate(timezone));
    if (parts) setYm([parts[0], parts[1]]);
  }, [todayStr, timezone, ym]);

  const load = useCallback(async () => {
    if (!ym) return;
    try {
      setData(await api.getHistoryMonth(ym[0], ym[1]));
      setError(null);
    } catch (e) {
      setError(friendlyError(e));
    }
  }, [ym]);

  // Reload on month change and whenever today's Done marks change.
  useEffect(() => {
    void load();
  }, [load, doneToday]);

  async function setStatus(day: string, prayer: Prayer, status: HistoryStatus | null) {
    setError(null);
    try {
      setData(await api.setHistoryEntry(day, prayer, status));
    } catch (e) {
      setError(friendlyError(e));
    }
  }

  function shiftMonth(delta: number) {
    if (!ym) return;
    const index = ym[0] * 12 + (ym[1] - 1) + delta;
    setYm([Math.floor(index / 12), (index % 12) + 1]);
    setSelected(null);
  }

  if (!ym || !data) {
    return error ? (
      <p role="alert" className="text-sm text-red-300">
        Couldn't load history: {error}
      </p>
    ) : (
      <p className="text-sm text-slate-400">Loading history…</p>
    );
  }

  const first = data.days[0] ? ymd(data.days[0].day) : null;
  const lead = first ? weekday(first[0], first[1], first[2]) : 0;
  const selectedDay = data.days.find((d) => d.day === selected) ?? null;
  const todayParts = ymd(data.today);
  const atCurrentMonth = todayParts !== null && todayParts[0] === ym[0] && todayParts[1] === ym[1];

  return (
    <div className="space-y-6">
      <div data-tour="history-header" className="flex flex-wrap items-center justify-between gap-3">
        <div className="flex items-center gap-1">
          <IconButton label="Previous month" onClick={() => shiftMonth(-1)}>
            <ChevronLeft className="h-4 w-4" aria-hidden />
          </IconButton>
          <h1 className="tnum w-40 text-center text-[15px] font-semibold text-slate-50">
            {MONTHS[ym[1] - 1]} {ym[0]}
          </h1>
          <IconButton label="Next month" onClick={() => shiftMonth(1)} disabled={atCurrentMonth}>
            <ChevronRight className="h-4 w-4" aria-hidden />
          </IconButton>
        </div>
        {data.streak > 0 && (
          <p className="text-[13px] text-slate-400">
            <span className="tnum font-semibold text-slate-100">{data.streak}</span>{" "}
            {data.streak === 1 ? "day" : "days"} with every prayer done
          </p>
        )}
      </div>

      {(data.error || error) && (
        <p role="alert" className="rounded-lg border border-red-400/30 bg-red-500/10 px-3 py-2 text-[13px] text-red-200">
          {error ?? "Your prayer history couldn't be read or saved just now. Alarms still work — try again in a moment."}
        </p>
      )}

      <div>
        <div className="mb-1.5 grid grid-cols-7 gap-1.5 text-center text-[11px] font-medium text-slate-500">
          {WEEKDAYS.map((w) => (
            <div key={w}>{w}</div>
          ))}
        </div>
        <div data-tour="history-days" className="grid grid-cols-7 gap-1.5">
          {Array.from({ length: lead }, (_, i) => (
            <div key={`lead-${i}`} aria-hidden />
          ))}
          {data.days.map((d) => (
            <DayCell
              key={d.day}
              day={d}
              isToday={d.day === data.today}
              selected={d.day === selected}
              onSelect={() => setSelected(d.day === selected ? null : d.day)}
            />
          ))}
        </div>
        <Legend />
      </div>

      {selectedDay && (
        <div ref={editorRef} className="scroll-mb-4">
          <DayEditor
            day={selectedDay}
            isToday={selectedDay.day === data.today}
            onSet={(p, s) => void setStatus(selectedDay.day, p, s)}
          />
        </div>
      )}

      <Section
        title="This month"
        description="Days each prayer was done, out of the days it has a record. Days the app wasn't running have no record and don't count as missed."
      >
        <div className="grid grid-cols-5 divide-x divide-slate-800/80">
          {PRAYERS.map((p) => {
            const t = data.totals[p];
            return (
              <div key={p} className="px-3 py-3 text-center">
                <p className="text-[12px] text-slate-400">{PRAYER_LABELS[p]}</p>
                <p className="tnum mt-0.5 text-[15px] font-semibold text-slate-50">{t.done}</p>
                <p className="tnum text-[11px] text-slate-500">
                  {t.recorded === 0 ? "no record" : `of ${t.recorded} ${t.recorded === 1 ? "day" : "days"}`}
                </p>
              </div>
            );
          })}
        </div>
      </Section>

      <Section title="Backup" tour="history-backup">
        <BackupControls historyOnly onImported={() => void load()} />
      </Section>
    </div>
  );
}

/** One prayer's mark, told apart by shape as well as colour (readable with
 * colour blindness): done = filled, missed = hollow ring, no record = dash. */
function Mark({ status, size = "sm" }: { status: HistoryStatus | null; size?: "sm" | "md" }) {
  const box = size === "md" ? "h-2.5 w-2.5" : "h-2 w-2";
  if (status === "done") return <span className={cn(box, "rounded-full bg-emerald-400")} />;
  if (status === "missed") return <span className={cn(box, "rounded-full border-[1.5px] border-slate-400")} />;
  return (
    <span className={cn(box, "flex items-center justify-center")}>
      <span className="h-px w-full bg-slate-600" />
    </span>
  );
}

function DayCell({
  day,
  isToday,
  selected,
  onSelect,
}: {
  day: HistoryDay;
  isToday: boolean;
  selected: boolean;
  onSelect: () => void;
}) {
  const n = ymd(day.day)?.[2] ?? 0;
  const done = PRAYERS.filter((p) => day.prayers[p] === "done").length;
  const missed = PRAYERS.filter((p) => day.prayers[p] === "missed").length;
  return (
    <button
      type="button"
      disabled={!day.editable}
      aria-pressed={selected}
      aria-label={`${humanDate(day.day)}: ${done} done, ${missed} missed`}
      onClick={onSelect}
      className={cn(
        "flex h-14 flex-col items-center justify-center gap-1.5 rounded-lg border text-[13px] transition-colors",
        "focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-emerald-400",
        selected
          ? "border-emerald-500/60 bg-emerald-500/10"
          : "border-slate-800/80 bg-slate-900/50 hover:border-slate-600",
        isToday && !selected && "border-slate-500",
        !day.editable && "cursor-default opacity-40 hover:border-slate-800/80",
      )}
    >
      <span className={cn("tnum", isToday ? "font-semibold text-slate-50" : "text-slate-300")}>{n}</span>
      <span className="flex gap-1" aria-hidden>
        {PRAYERS.map((p) => (
          <Mark key={p} status={day.prayers[p]} />
        ))}
      </span>
    </button>
  );
}

function Legend() {
  return (
    <div className="mt-3 flex flex-wrap gap-x-4 gap-y-1 px-1 text-[12px] text-slate-400">
      {(
        [
          ["done", "Done"],
          ["missed", "Missed"],
          [null, "No record"],
        ] as const
      ).map(([status, label]) => (
        <span key={label} className="flex items-center gap-1.5">
          <span aria-hidden className="flex">
            <Mark status={status} size="md" />
          </span>
          {label}
        </span>
      ))}
      <span>Dots in order: Fajr, Dhuhr, Asr, Maghrib, Isha</span>
    </div>
  );
}

const CHOICES: { status: HistoryStatus | null; label: string }[] = [
  { status: "done", label: "Done" },
  { status: "missed", label: "Missed" },
  { status: null, label: "No record" },
];

function DayEditor({
  day,
  isToday,
  onSet,
}: {
  day: HistoryDay;
  isToday: boolean;
  onSet: (prayer: Prayer, status: HistoryStatus | null) => void;
}) {
  const parts = ymd(day.day);
  const title = parts
    ? `${MONTHS[parts[1] - 1]} ${parts[2]}${isToday ? " (today)" : ""}`
    : day.day;
  return (
    <Section
      title={title}
      description={
        isToday
          ? "Marking done here works like the Today tab and stops that prayer's alarms."
          : "Fix a day you forgot to mark."
      }
    >
      {PRAYERS.map((p) => (
        <div key={p} className="flex min-h-12 items-center justify-between gap-4 px-4 py-2">
          <span className="text-sm text-slate-100">{PRAYER_LABELS[p]}</span>
          <Segmented
            ariaLabel={`${PRAYER_LABELS[p]} status`}
            items={CHOICES.map((c) => {
              const active = day.prayers[p] === c.status;
              return {
                key: c.label,
                label: c.label,
                pressed: active,
                tone: c.status === "done" ? "accent" : "neutral",
                onClick: () => {
                  if (!active) onSet(p, c.status);
                },
              };
            })}
          />
        </div>
      ))}
    </Section>
  );
}
