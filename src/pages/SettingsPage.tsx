import { useEffect, useMemo, useRef, useState } from "react";
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
import { useAppState } from "../store/useAppStore";
import { IS_MAC, useUpdateStore } from "../store/useUpdateStore";
import {
  getAutostart,
  resetAlarmTone,
  setAlarmTone,
  setAutostart as setAutostartCmd,
  testSound,
  updateSettings,
} from "../lib/actions";
import { METHODS } from "../lib/methods";
import { PRAYERS, PRAYER_LABELS, type Prayer, type Settings } from "../lib/types";
import { fmtClock, parseHM } from "../lib/time";
import { cn } from "../lib/cn";

/**
 * Settings are edited as a local draft and pushed to the backend on Save.
 * Changing method/school/location makes the backend refetch the schedule.
 */
export function SettingsPage() {
  const state = useAppState();
  const settings = state?.settings ?? null;
  const [draft, setDraft] = useState<Settings | null>(settings);
  const [saveState, setSaveState] = useState<"idle" | "saving" | "saved">("idle");
  const [toast, setToast] = useState<ToastData | null>(null);
  const [changingLocation, setChangingLocation] = useState(false);
  const [toneBusy, setToneBusy] = useState(false);
  const [toneError, setToneError] = useState<string | null>(null);
  const [autostart, setAutostart] = useState(false);
  const updates = useUpdateStore();

  useEffect(() => {
    void getAutostart().then(setAutostart).catch(() => undefined);
  }, []);

  // Applies instantly via the OS launcher entry; reverts on failure.
  async function toggleAutostart(enabled: boolean) {
    setAutostart(enabled);
    try {
      setAutostart(await setAutostartCmd(enabled));
    } catch (e) {
      setAutostart(!enabled);
      setToneError(String(e));
    }
  }

  // "Alarm at 12:16 PM" — today's start time + the draft's delay minutes.
  function reminderHint(prayer: Prayer): string | null {
    const mins = draft?.prayerSettings[prayer].reminderMinutes;
    const hm = state?.today?.timings.find((t) => t.prayer === prayer)?.start;
    const start = hm ? parseHM(hm) : null;
    if (!start) return null;
    const d = new Date();
    d.setHours(start.h, start.m + (mins ?? 0), 0, 0);
    const mode = draft?.timeFormat ?? "12h";
    return `Alarm at ${fmtClock(d, mode)}`;
  }

  // Rebase the draft whenever the backend pushes new settings (tone picked,
  // volume saved, location changed…): fields the user hasn't edited take the
  // new value, edited fields keep the user's pending change. Comparing the
  // draft against the *new* settings (the old logic) left the draft stale
  // after any instant change, e.g. a just-picked tone showed as "unsaved".
  const prevSettings = useRef<Settings | null>(settings);
  useEffect(() => {
    const prev = prevSettings.current;
    prevSettings.current = settings;
    setDraft((d) => (d && prev && settings ? rebase(d, prev, settings) : d ?? settings));
  }, [settings]);

  // Volume applies instantly (like the tone): debounce while dragging, then
  // persist the *saved* settings + new volume so other pending draft edits
  // aren't committed behind the user's back.
  const volumeTimer = useRef<number | undefined>(undefined);
  const latestSettings = useRef(settings);
  latestSettings.current = settings;
  useEffect(() => () => window.clearTimeout(volumeTimer.current), []);

  function changeVolume(volume: number) {
    patch({ soundVolume: volume });
    window.clearTimeout(volumeTimer.current);
    volumeTimer.current = window.setTimeout(() => {
      const base = latestSettings.current;
      if (!base || base.soundVolume === volume) return;
      updateSettings({ ...base, soundVolume: volume }).catch((e) =>
        setToast({ kind: "error", message: `Couldn't save volume: ${String(e)}` }),
      );
    }, 300);
  }

  const dirty = useMemo(
    () => Boolean(draft && settings && !sameJson(draft, settings)),
    [draft, settings],
  );

  // A new edit while the "Saved" confirmation is showing invalidates it.
  useEffect(() => {
    if (dirty && saveState === "saved") setSaveState("idle");
  }, [dirty, saveState]);

  if (!draft || !settings) {
    return (
      <div className="p-6 text-sm text-slate-400">Loading settings…</div>
    );
  }

  function patch(p: Partial<Settings>) {
    setDraft((d) => (d ? { ...d, ...p } : d));
  }

  function patchPrayer(prayer: (typeof PRAYERS)[number], p: Partial<Settings["prayerSettings"][keyof Settings["prayerSettings"]]>) {
    setDraft((d) =>
      d
        ? {
            ...d,
            prayerSettings: {
              ...d.prayerSettings,
              [prayer]: { ...d.prayerSettings[prayer], ...p },
            },
          }
        : d,
    );
  }

  async function save() {
    if (!draft) return;
    // The draft already carries the volume; a pending debounced volume save
    // would otherwise fire with the old base and revert this save.
    window.clearTimeout(volumeTimer.current);
    setSaveState("saving");
    try {
      await updateSettings(draft);
      setSaveState("saved");
      setToast({ kind: "success", message: "Settings saved" });
      // Revert the button label once the confirmation has been seen.
      window.setTimeout(
        () => setSaveState((s) => (s === "saved" ? "idle" : s)),
        2500,
      );
    } catch (e) {
      setSaveState("idle");
      setToast({ kind: "error", message: `Couldn't save settings: ${String(e)}` });
    }
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
      setToneError(String(e));
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
      setToneError(String(e));
    } finally {
      setToneBusy(false);
    }
  }

  const location = settings.location;

  return (
    <div className="space-y-9 pb-6">
      <h1 className="px-1 text-xl font-semibold tracking-[-0.01em] text-slate-50">Settings</h1>

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

      <Section title="Alarms" description="How you're alerted when an alarm rings.">
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
          hint="Applies instantly"
          disabled={!draft.soundEnabled}
        >
          <div className="flex w-64 items-center gap-3">
            <SliderField
              aria-label="Alarm volume"
              disabled={!draft.soundEnabled}
              value={Math.round(draft.soundVolume * 100)}
              onChange={(e) => changeVolume(Number(e.target.value) / 100)}
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
            <Volume2 className="h-4 w-4" aria-hidden /> Test
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
        {toneError && (
          <p role="alert" className="px-4 py-3 text-[13px] text-red-300">
            {toneError}
          </p>
        )}
      </Section>

      <Section
        title="Alarm timing"
        description="Each prayer's alarm rings this many minutes after it starts. Leave the box empty to ring right at the start."
      >
        {PRAYERS.map((p) => {
          const ps = draft.prayerSettings[p];
          return (
            <SettingRow
              key={p}
              label={PRAYER_LABELS[p]}
              hint={ps.enabled ? (reminderHint(p) ?? "Alarms on") : "Alarms off"}
              disabled={!ps.enabled}
            >
              <UnitInput
                unit="min"
                min={1}
                max={600}
                placeholder="—"
                disabled={!ps.enabled}
                value={ps.reminderMinutes ?? ""}
                onChange={(e) => {
                  const raw = e.target.value;
                  patchPrayer(p, {
                    reminderMinutes: raw === "" ? null : clamp(Number(raw), 1, 600),
                  });
                }}
                aria-label={`Minutes after ${PRAYER_LABELS[p]} starts`}
              />
              <Toggle
                label={`${PRAYER_LABELS[p]} alarms`}
                checked={ps.enabled}
                onChange={(v) => patchPrayer(p, { enabled: v })}
              />
            </SettingRow>
          );
        })}
        <SettingRow
          label="Before-end warning"
          hint="A final alarm if a prayer isn't marked done before its time ends"
        >
          <UnitInput
            unit="min"
            min={1}
            max={120}
            disabled={!draft.preEndWarningEnabled}
            value={draft.preEndWarningMinutes}
            onChange={(e) => patch({ preEndWarningMinutes: clamp(Number(e.target.value), 1, 120) })}
            aria-label="Minutes before the prayer time ends"
          />
          <Toggle
            label="Before-end warning"
            checked={draft.preEndWarningEnabled}
            onChange={(v) => patch({ preEndWarningEnabled: v })}
          />
        </SettingRow>
      </Section>

      <Section title="General">
        <SettingRow label="Open at login" hint="Starts quietly in the tray when you sign in. Applies instantly.">
          <Toggle label="Open at login" checked={autostart} onChange={(v) => void toggleAutostart(v)} />
        </SettingRow>
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

      {(dirty || saveState === "saving") && (
        <div className="fade-in sticky bottom-3 z-30">
          <div className="flex items-center justify-between gap-3 rounded-xl border border-slate-700 bg-slate-900/95 py-2 pr-2 pl-4 shadow-[0_16px_40px_-12px_rgb(0_0_0/0.7)] backdrop-blur">
            <p className="text-sm text-slate-300">You have unsaved changes</p>
            <div className="flex gap-2">
              <Button
                variant="ghost"
                disabled={saveState === "saving"}
                onClick={() => {
                  window.clearTimeout(volumeTimer.current);
                  setDraft(settings);
                }}
              >
                Discard
              </Button>
              <Button variant="primary" disabled={saveState === "saving"} onClick={() => void save()}>
                {saveState === "saving" ? (
                  <Loader2 className="h-4 w-4 animate-spin" aria-hidden />
                ) : (
                  <Check className="h-4 w-4" aria-hidden />
                )}
                {saveState === "saving" ? "Saving…" : "Save changes"}
              </Button>
            </div>
          </div>
        </div>
      )}

      {changingLocation && (
        <div className="fixed inset-0 z-50 overflow-y-auto bg-slate-950/95">
          <SetupWizard onFinished={() => setChangingLocation(false)} onCancel={() => setChangingLocation(false)} />
        </div>
      )}

      {toast && <Toast toast={toast} onDismiss={() => setToast(null)} />}
    </div>
  );
}

function clamp(v: number, lo: number, hi: number): number {
  if (!Number.isFinite(v)) return lo;
  return Math.min(hi, Math.max(lo, Math.round(v)));
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
