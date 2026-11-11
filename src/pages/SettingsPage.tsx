import { useEffect, useRef, useState, type InputHTMLAttributes } from "react";
import {
  Check,
  Download,
  ExternalLink,
  FolderOpen,
  Loader2,
  MapPin,
  RotateCcw,
  TriangleAlert,
  Undo2,
  Volume2,
} from "lucide-react";
import { open } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import { Button } from "../components/ui/Button";
import { Section, SettingRow } from "../components/ui/Section";
import { Toast, type ToastData } from "../components/ui/Toast";
import { Toggle } from "../components/ui/Toggle";
import { Select, SliderField, UnitInput } from "../components/ui/fields";
import { SetupWizard } from "../components/SetupWizard";
import { BackupControls } from "../components/BackupControls";
import { useAppState } from "../store/useAppStore";
import { IS_MAC, useUpdateStore } from "../store/useUpdateStore";
import {
  getAutostart,
  previewAlarm,
  resetAlarmTone,
  resetTours,
  setAlarmTone,
  setAutostart as setAutostartCmd,
  testSound,
  updateSettings,
} from "../lib/actions";
import { METHODS } from "../lib/methods";
import { PRAYERS, PRAYER_LABELS, type Prayer, type Settings } from "../lib/types";
import { cn } from "../lib/cn";
import { fmtHourMinute, parseHM } from "../lib/time";
import { friendlyError } from "../lib/errors";
import { forgetShownTours } from "../lib/useTour";

/**
 * Settings are edited as a local draft and pushed to the backend on Save.
 * Changing method/school/location makes the backend refetch the schedule.
 */
export function SettingsPage() {
  const state = useAppState();
  const settings = state?.settings ?? null;
  const [draft, setDraft] = useState<Settings | null>(settings);
  const [saveState, setSaveState] = useState<"idle" | "saving" | "saved" | "error">("idle");
  const [toast, setToast] = useState<ToastData | null>(null);
  const [changingLocation, setChangingLocation] = useState(false);
  const [toneBusy, setToneBusy] = useState(false);
  const [toneError, setToneError] = useState<string | null>(null);
  const [autostart, setAutostart] = useState(false);
  const updates = useUpdateStore();

  useEffect(() => {
    void getAutostart().then(setAutostart).catch(() => undefined);
  }, []);

  const [autostartError, setAutostartError] = useState<string | null>(null);
  const [tourError, setTourError] = useState<string | null>(null);

  async function replayTours() {
    setTourError(null);
    forgetShownTours();
    try {
      await resetTours();
    } catch (e) {
      setTourError(`Couldn't reset the tours. ${friendlyError(e)}`);
    }
  }

  // Applies instantly via the OS launcher entry; reverts on failure.
  async function toggleAutostart(enabled: boolean) {
    setAutostart(enabled);
    setAutostartError(null);
    try {
      setAutostart(await setAutostartCmd(enabled));
    } catch (e) {
      setAutostart(!enabled);
      setAutostartError(`Couldn't change this. ${friendlyError(e)}`);
    }
  }

  // "Starts 11:45 AM · you pray 1:00 PM" from today's timetable strings
  // (location time, no device clock). Mirrors `schedule::pray_time`: never in
  // the window's last 5 minutes.
  function prayHint(prayer: Prayer): string | null {
    const row = state?.today?.timings.find((t) => t.prayer === prayer);
    const start = row ? parseHM(row.start) : null;
    const end = row ? parseHM(row.ends) : null;
    if (!start || !end) return null;
    const mode = draft?.timeFormat ?? "12h";
    const startMin = start.h * 60 + start.m;
    let endMin = end.h * 60 + end.m;
    if (endMin <= startMin) endMin += 24 * 60;
    const after = draft?.prayerSettings[prayer].prayAfterMinutes ?? 0;
    const prayMin = Math.max(startMin, Math.min(startMin + after, endMin - 5));
    const clock = (total: number) =>
      fmtHourMinute(Math.floor(total / 60) % 24, total % 60, mode);
    if (prayMin === startMin) {
      // Minutes set, but the window is too short to move into (<= 5 min).
      return after > 0
        ? `Starts ${clock(startMin)} · you pray at the start (its time is too short to move)`
        : `Starts ${clock(startMin)} · you pray at the start`;
    }
    const clamped = prayMin < startMin + after;
    return `Starts ${clock(startMin)} · you pray ${clock(prayMin)}${
      clamped ? " (kept 5 min before its time ends)" : ""
    }`;
  }

  // Rebase the draft whenever the backend pushes new settings (tone picked,
  // a save landing, location changed…): fields the user hasn't edited since
  // take the new value, an edit still waiting to be saved keeps the user's
  // value.
  const prevSettings = useRef<Settings | null>(settings);
  useEffect(() => {
    const prev = prevSettings.current;
    prevSettings.current = settings;
    setDraft((d) => (d && prev && settings ? rebase(d, prev, settings) : d ?? settings));
  }, [settings]);

  // Every change applies instantly. Saves are serialized — one in flight, and
  // whatever changed meanwhile goes out right after — so responses can't
  // arrive out of order and revert a newer edit.
  const saveTimer = useRef<number | undefined>(undefined);
  const pending = useRef<Settings | null>(null);
  const inFlight = useRef(false);

  async function flush() {
    window.clearTimeout(saveTimer.current);
    if (inFlight.current || !pending.current) return;
    const next = pending.current;
    pending.current = null;
    inFlight.current = true;
    setSaveState("saving");
    try {
      await updateSettings(next);
      setSaveState(pending.current ? "saving" : "saved");
    } catch (e) {
      // Keep the edit queued so Retry resends it.
      pending.current ??= next;
      setSaveState("error");
      setToast({ kind: "error", message: `Couldn't save settings. ${friendlyError(e)}` });
    } finally {
      inFlight.current = false;
    }
    if (pending.current && saveTimer.current === undefined) void flush();
  }

  function queueSave(next: Settings, delayMs: number) {
    pending.current = next;
    window.clearTimeout(saveTimer.current);
    saveTimer.current = window.setTimeout(() => {
      saveTimer.current = undefined;
      void flush();
    }, delayMs);
  }

  // Leaving the page must not drop an edit still in its debounce window.
  const flushRef = useRef(flush);
  flushRef.current = flush;
  useEffect(() => () => void flushRef.current(), []);

  if (!draft || !settings) {
    return <div className="p-6 text-sm text-slate-400">Loading settings…</div>;
  }

  /** Applies `p` now; `delayMs` debounces typing and dragging. */
  function patch(p: Partial<Settings>, delayMs = 0) {
    if (!draft) return;
    const next = { ...draft, ...p };
    setDraft(next);
    queueSave(next, delayMs);
  }

  function patchPrayer(
    prayer: Prayer,
    p: Partial<Settings["prayerSettings"][Prayer]>,
    delayMs = 0,
  ) {
    if (!draft) return;
    patch(
      {
        prayerSettings: {
          ...draft.prayerSettings,
          [prayer]: { ...draft.prayerSettings[prayer], ...p },
        },
      },
      delayMs,
    );
  }

  // The tone applies immediately (file operation), outside the draft/Save flow.
  async function chooseTone() {
    setToneError(null);
    try {
      const picked = await open({
        multiple: false,
        filters: [{ name: "Audio", extensions: ["mp3", "wav", "ogg", "flac"] }],
      });
      if (typeof picked !== "string" || !picked) return;
      setToneBusy(true);
      await setAlarmTone(picked);
    } catch (e) {
      setToneError(friendlyError(e));
    } finally {
      setToneBusy(false);
    }
  }

  async function useDefaultTone() {
    setToneError(null);
    setToneBusy(true);
    try {
      await resetAlarmTone();
    } catch (e) {
      setToneError(friendlyError(e));
    } finally {
      setToneBusy(false);
    }
  }

  const location = settings.location;

  return (
    <div className="space-y-9 pb-6">
      <div className="flex items-baseline justify-between gap-4 px-1">
        <h1 className="text-xl font-semibold tracking-[-0.01em] text-slate-50">Settings</h1>
        <SaveStatus state={saveState} onRetry={() => void flush()} />
      </div>

      <Section title="Location" description="Prayer times are calculated for this place.">
        <SettingRow
          label={
            location ? (
              <span className="flex items-center gap-2">
                <MapPin className="h-4 w-4 shrink-0 text-emerald-400" aria-hidden />
                {[location.city, location.region, location.country].filter(Boolean).join(", ")}
              </span>
            ) : (
              "No location set"
            )
          }
          hint={location ? location.timezone : undefined}
        >
          <Button onClick={() => setChangingLocation(true)}>Change…</Button>
        </SettingRow>
        {state?.locationWarning && (
          <p role="alert" className="flex items-start gap-2.5 px-4 py-3 text-[13px] leading-relaxed text-amber-200">
            <TriangleAlert className="mt-0.5 h-4 w-4 shrink-0 text-amber-400" aria-hidden />
            {state.locationWarning}
          </p>
        )}
        {location && !location.coordinates && !state?.locationWarning && (
          <p className="px-4 py-3 text-[13px] leading-relaxed text-slate-400">
            This city was set by name. Choose Change… and pick it from the list to get times for
            its exact position.
          </p>
        )}
      </Section>

      <Section title="Calculation" description="Changing these refetches the timetable.">
        <SettingRow label="Calculation method" hint="Sets the sun angles used for Fajr and Isha">
          <Select
            className="w-80 max-w-full"
            value={draft.method}
            onChange={(e) => patch({ method: Number(e.target.value) })}
            aria-label="Calculation method"
          >
            {METHODS.map((m) => (
              <option key={m.id} value={m.id}>
                {m.name}
              </option>
            ))}
          </Select>
        </SettingRow>
        <SettingRow label="Asr" hint="Hanafi starts Asr later">
          <Select
            className="w-80 max-w-full"
            value={draft.school}
            onChange={(e) => patch({ school: Number(e.target.value) === 1 ? 1 : 0 })}
            aria-label="Asr calculation"
          >
            <option value={0}>Standard (Shafi, Maliki, Hanbali)</option>
            <option value={1}>Hanafi</option>
          </Select>
        </SettingRow>
        <SettingRow label="Time format">
          <Select
            className="w-80 max-w-full"
            value={draft.timeFormat}
            onChange={(e) => patch({ timeFormat: e.target.value === "24h" ? "24h" : "12h" })}
            aria-label="Time format"
          >
            <option value="12h">12-hour (5:45 PM)</option>
            <option value="24h">24-hour (17:45)</option>
          </Select>
        </SettingRow>
      </Section>

      <Section
        title="When you pray"
        description="Set this if you pray later than the start, for example with your mosque. It follows the start time every day."
      >
        {PRAYERS.map((p) => {
          const ps = draft.prayerSettings[p];
          return (
            <SettingRow
              key={p}
              label={PRAYER_LABELS[p]}
              hint={ps.enabled ? prayHint(p) : "Alarms off for this prayer"}
              disabled={!ps.enabled}
            >
              <MinutesInput
                min={0}
                max={600}
                allowEmpty
                placeholder="0"
                disabled={!ps.enabled}
                value={ps.prayAfterMinutes}
                onCommit={(v) => patchPrayer(p, { prayAfterMinutes: v === 0 ? null : v }, 600)}
                aria-label={`Minutes after ${PRAYER_LABELS[p]} starts that you pray`}
              />
              <span className="whitespace-nowrap text-[13px] text-slate-400">after start</span>
            </SettingRow>
          );
        })}
      </Section>

      <Section
        title="When alarms ring"
        tour="settings-alarms"
        description="Turn on the alarms you want. They apply to every prayer."
      >
        <SettingRow
          label="Get ready"
          hint="Rings 5–60 min before you pray"
          disabled={!draft.getReadyEnabled}
        >
          <MinutesInput
            min={5}
            max={60}
            disabled={!draft.getReadyEnabled}
            value={draft.getReadyMinutes}
            onCommit={(v) => v !== null && patch({ getReadyMinutes: v }, 600)}
            aria-label="Minutes before you pray"
          />
          <Toggle
            label="Get ready"
            checked={draft.getReadyEnabled}
            onChange={(v) => patch({ getReadyEnabled: v })}
          />
        </SettingRow>
        <SettingRow
          label="Prayer time"
          hint="Rings when you pray"
          disabled={!draft.startAlarmEnabled}
        >
          <Toggle
            label="Prayer time"
            checked={draft.startAlarmEnabled}
            onChange={(v) => patch({ startAlarmEnabled: v })}
          />
        </SettingRow>
        <SettingRow
          label="Don't miss it"
          hint="Last call, 1–120 min before a prayer's time ends. Skipped once you mark it done."
          disabled={!draft.preEndWarningEnabled}
        >
          <MinutesInput
            min={1}
            max={120}
            disabled={!draft.preEndWarningEnabled}
            value={draft.preEndWarningMinutes}
            onCommit={(v) => v !== null && patch({ preEndWarningMinutes: v }, 600)}
            aria-label="Minutes before the prayer time ends"
          />
          <Toggle
            label="Don't miss it"
            checked={draft.preEndWarningEnabled}
            onChange={(v) => patch({ preEndWarningEnabled: v })}
          />
        </SettingRow>
        <SettingRow label="Ring for" hint="Click a prayer to turn its alarms off">
          <div role="group" aria-label="Prayers with alarms" className="flex flex-wrap gap-1.5">
            {PRAYERS.map((p) => {
              const on = draft.prayerSettings[p].enabled;
              return (
                <button
                  key={p}
                  type="button"
                  aria-pressed={on}
                  onClick={() => patchPrayer(p, { enabled: !on })}
                  className={cn(
                    "h-8 rounded-full border px-3 text-[13px] transition-colors",
                    on
                      ? "border-emerald-500/40 bg-emerald-500/15 text-emerald-200 hover:bg-emerald-500/25"
                      : "border-slate-700 text-slate-500 hover:bg-slate-800 hover:text-slate-300",
                  )}
                >
                  {PRAYER_LABELS[p]}
                </button>
              );
            })}
          </div>
        </SettingRow>
      </Section>

      <Section title="How alarms ring" description="Applies to every alarm." tour="settings-sound">
        <SettingRow label="System notifications" hint="Shown in your notification center">
          <Toggle
            label="System notifications"
            checked={draft.notificationsEnabled}
            onChange={(v) => patch({ notificationsEnabled: v })}
          />
        </SettingRow>
        <SettingRow label="Alarm popup" hint="Small always-on-top window">
          <Toggle
            label="Alarm popup"
            checked={draft.popupEnabled}
            onChange={(v) => patch({ popupEnabled: v })}
          />
        </SettingRow>
        <SettingRow label="Alarm sound" hint="Plays until you respond">
          <Toggle
            label="Alarm sound"
            checked={draft.soundEnabled}
            onChange={(v) => patch({ soundEnabled: v })}
          />
        </SettingRow>
        <SettingRow
          label="Volume"
          disabled={!draft.soundEnabled}
        >
          <div className="flex w-64 items-center gap-3">
            <SliderField
              aria-label="Alarm volume"
              disabled={!draft.soundEnabled}
              value={Math.round(draft.soundVolume * 100)}
              onChange={(e) => patch({ soundVolume: Number(e.target.value) / 100 }, 300)}
            />
            <span className="tnum w-9 text-right text-[13px] text-slate-400">
              {Math.round(draft.soundVolume * 100)}%
            </span>
          </div>
          <Button
            size="sm"
            variant="ghost"
            onClick={() => void testSound(draft.soundVolume)}
            aria-label="Play the alarm tone"
          >
            <Volume2 className="h-4 w-4" aria-hidden /> Play tone
          </Button>
        </SettingRow>
        <SettingRow
          label="Alarm tone"
          hint={settings.alarmSound ? `Custom: ${settings.alarmSound}` : "Default chime"}
        >
          {settings.alarmSound && (
            <Button size="sm" variant="ghost" disabled={toneBusy} onClick={() => void useDefaultTone()}>
              <Undo2 className="h-3.5 w-3.5" aria-hidden /> Use default
            </Button>
          )}
          <Button size="sm" disabled={toneBusy} onClick={() => void chooseTone()}>
            <FolderOpen className="h-3.5 w-3.5" aria-hidden /> Choose file…
          </Button>
        </SettingRow>
        {/* Development builds only: a tool for checking the popup design. */}
        {import.meta.env.DEV && (
          <SettingRow
            label="Preview an alarm (dev)"
            hint="Development builds only. Shows the popup, notification and sound. Nothing is marked done or snoozed."
          >
            {(
              [
                ["getReady", "Get ready"],
                ["start", "Prayer time"],
                ["preEnd", "Don't miss it"],
              ] as const
            ).map(([kind, label]) => (
              <Button
                key={kind}
                size="sm"
                variant="ghost"
                onClick={() => void previewAlarm(kind).catch((e) => setToneError(friendlyError(e)))}
              >
                {label}
              </Button>
            ))}
          </SettingRow>
        )}
        {toneError && (
          <p role="alert" className="px-4 py-3 text-[13px] text-red-300">
            {toneError}
          </p>
        )}
      </Section>

      <Section title="General">
        <SettingRow label="Open at login" hint="Starts quietly in the tray when you sign in">
          <Toggle label="Open at login" checked={autostart} onChange={(v) => void toggleAutostart(v)} />
        </SettingRow>
        {autostartError && (
          <p role="alert" className="px-4 py-3 text-[13px] text-red-300">
            {autostartError}
          </p>
        )}
        <div data-tour="settings-backup" className="divide-y divide-slate-800/80">
          <BackupControls />
        </div>
        <SettingRow label="Replay tours" hint="Show the tips for each tab again">
          <Button size="sm" onClick={() => void replayTours()}>
            Replay
          </Button>
        </SettingRow>
        {tourError && (
          <p role="alert" className="px-4 py-3 text-[13px] text-red-300">
            {tourError}
          </p>
        )}
      </Section>

      <Section
        title="Updates"
        description={
          IS_MAC
            ? "Checked at launch. On macOS, updates download from GitHub."
            : "Checked at launch and installed inside the app."
        }
      >
        <SettingRow
          label={updates.currentVersion ? `Version ${updates.currentVersion}` : "Version"}
          hint={
            updates.phase === "up-to-date"
              ? "You're on the latest version"
              : updates.phase === "downloading"
                ? "Downloading… the app restarts when ready"
                : undefined
          }
        >
          <Button
            size="sm"
            disabled={updates.phase === "checking" || updates.phase === "downloading"}
            onClick={() => void updates.checkForUpdates(false)}
          >
            <RotateCcw
              className={cn("h-3.5 w-3.5", updates.phase === "checking" && "animate-spin")}
              aria-hidden
            />
            {updates.phase === "checking" ? "Checking…" : "Check for updates"}
          </Button>
        </SettingRow>
        {updates.phase === "available" && (
          <div className="space-y-3 px-4 py-4">
            <p className="text-sm text-slate-200">
              Version <span className="font-medium text-emerald-300">{updates.newVersion}</span> is
              available.
            </p>
            {updates.notes && (
              <pre className="max-h-32 overflow-y-auto rounded-lg bg-slate-950/60 p-3 font-sans text-[13px] leading-relaxed whitespace-pre-wrap text-slate-400">
                {updates.notes}
              </pre>
            )}
            {IS_MAC ? (
              <Button size="sm" variant="primary" onClick={updates.openDownloadPage}>
                <ExternalLink className="h-3.5 w-3.5" aria-hidden /> Download from GitHub
              </Button>
            ) : (
              <Button size="sm" variant="primary" onClick={() => void updates.install()}>
                <Download className="h-3.5 w-3.5" aria-hidden /> Install update
              </Button>
            )}
          </div>
        )}
        {updates.phase === "error" && (
          <p role="alert" className="px-4 py-3 text-[13px] text-red-300">
            {updates.error}
          </p>
        )}
      </Section>

      <footer className="flex flex-wrap items-center justify-between gap-2 px-1 text-[13px] text-slate-500">
        <p>
          Prayer Times by <span className="text-slate-300">Raihan Ali</span>
        </p>
        <div className="flex items-center gap-4">
          <button
            type="button"
            className="inline-flex items-center gap-1 hover:text-slate-200"
            onClick={() => void openUrl("https://github.com/iamraihan/prayer-times")}
          >
            GitHub <ExternalLink className="h-3 w-3" aria-hidden />
          </button>
          <button
            type="button"
            className="inline-flex items-center gap-1 hover:text-slate-200"
            onClick={() => void openUrl("https://raihanali.com/prayer-times")}
          >
            raihanali.com <ExternalLink className="h-3 w-3" aria-hidden />
          </button>
        </div>
      </footer>

      {changingLocation && (
        <div className="fixed inset-0 z-50 overflow-y-auto bg-slate-950/95">
          <SetupWizard onFinished={() => setChangingLocation(false)} onCancel={() => setChangingLocation(false)} />
        </div>
      )}

      {toast && <Toast toast={toast} onDismiss={() => setToast(null)} />}
    </div>
  );
}

// Every Settings field, checked by the compiler: adding a field to
// `Settings` without listing it here is a type error, so rebase can't miss it.
const SETTINGS_FIELDS: { [K in keyof Settings]-?: true } = {
  location: true,
  method: true,
  school: true,
  timeFormat: true,
  notificationsEnabled: true,
  soundEnabled: true,
  soundVolume: true,
  alarmSound: true,
  popupEnabled: true,
  getReadyEnabled: true,
  getReadyMinutes: true,
  startAlarmEnabled: true,
  preEndWarningEnabled: true,
  preEndWarningMinutes: true,
  prayerSettings: true,
};
const SETTINGS_KEYS = Object.keys(SETTINGS_FIELDS) as (keyof Settings)[];

/** Three-way merge per top-level field: untouched draft fields follow `next`. */
function rebase(draft: Settings, prev: Settings, next: Settings): Settings {
  const out: Settings = { ...next };
  for (const key of SETTINGS_KEYS) keepEdit(out, draft, prev, key);
  // The tone is always backend-owned (applied instantly, never via Save).
  out.alarmSound = next.alarmSound;
  return out;
}

function keepEdit<K extends keyof Settings>(
  out: Settings,
  draft: Settings,
  prev: Settings,
  key: K,
): void {
  if (!sameJson(draft[key], prev[key])) out[key] = draft[key];
}

// Settings serialize from Rust structs, so key order is fixed and a plain
// JSON comparison is exact.
function sameJson(a: unknown, b: unknown): boolean {
  return JSON.stringify(a) === JSON.stringify(b);
}

function SaveStatus({
  state,
  onRetry,
}: {
  state: "idle" | "saving" | "saved" | "error";
  onRetry: () => void;
}) {
  return (
    <p aria-live="polite" className="flex items-center gap-1.5 text-[13px] text-slate-500">
      {state === "saving" && (
        <>
          <Loader2 className="h-3.5 w-3.5 animate-spin" aria-hidden /> Saving…
        </>
      )}
      {state === "saved" && (
        <>
          <Check className="h-3.5 w-3.5 text-emerald-400" aria-hidden /> All changes saved
        </>
      )}
      {state === "error" && (
        <>
          <span className="text-red-300">Not saved.</span>
          <button
            type="button"
            onClick={onRetry}
            className="text-slate-300 underline underline-offset-2 hover:text-slate-100"
          >
            Retry
          </button>
        </>
      )}
    </p>
  );
}

/**
 * Minutes box that keeps whatever is typed and only commits valid values
 * (whole numbers in range, or empty when `allowEmpty`). Leaving the box
 * shows the last committed value again, so a half-typed or out-of-range
 * entry never gets saved.
 */
function MinutesInput({
  value,
  min,
  max,
  allowEmpty,
  onCommit,
  ...rest
}: {
  value: number | null;
  min: number;
  max: number;
  allowEmpty?: boolean;
  onCommit: (value: number | null) => void;
} & Omit<InputHTMLAttributes<HTMLInputElement>, "value" | "onChange" | "min" | "max">) {
  const shown = value === null ? "" : String(value);
  const [text, setText] = useState(shown);
  const focused = useRef(false);
  useEffect(() => {
    if (!focused.current) setText(shown);
  }, [shown]);

  function parse(raw: string): number | null | undefined {
    const t = raw.trim();
    if (t === "") return allowEmpty ? null : undefined;
    const n = Number(t);
    return Number.isInteger(n) && n >= min && n <= max ? n : undefined;
  }

  return (
    <UnitInput
      unit="min"
      min={min}
      max={max}
      value={text}
      aria-invalid={parse(text) === undefined}
      onFocus={() => (focused.current = true)}
      onBlur={() => {
        focused.current = false;
        setText(shown);
      }}
      onChange={(e) => {
        setText(e.target.value);
        const v = parse(e.target.value);
        if (v !== undefined) onCommit(v);
      }}
      {...rest}
    />
  );
}
