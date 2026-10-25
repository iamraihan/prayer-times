import { useEffect, useMemo, useState, type ReactNode } from "react";
import { Check, Download, ExternalLink, FolderOpen, Loader2, MapPin, RotateCcw, Save, Undo2, Volume2 } from "lucide-react";
import { open } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import { Button } from "../components/ui/Button";
import { Card } from "../components/ui/Card";
import { Toast, type ToastData } from "../components/ui/Toast";
import { Toggle } from "../components/ui/Toggle";
import { NumberField, SelectField, SliderField } from "../components/ui/fields";
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
import { fmtClock } from "../lib/time";
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
    const mins = draft?.prayerSettings[prayer]?.reminderMinutes;
    const start = state?.today?.timings.find((t) => t.prayer === prayer)?.start;
    if (!start) return null;
    const [h, m] = start.split(":").map(Number);
    const d = new Date();
    d.setHours(h, (m ?? 0) + (mins ?? 0), 0, 0);
    const mode = draft?.timeFormat === "24h" ? "24h" : "12h";
    return `Alarm at ${fmtClock(d, mode)}`;
  }

  // Re-seed the draft whenever the backend pushes a different settings object
  // while the draft is unchanged (e.g. after location change).
  useEffect(() => {
    setDraft((d) => (d && settings && sameJson(d, settings) ? settings : d ?? settings));
  }, [settings]);

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

  return (
    <div className="space-y-5 pb-6">
      <div className="flex items-center justify-between">
        <h1 className="text-xl font-semibold">Settings</h1>
        <div className="flex gap-2">
          <Button
            variant="ghost"
            disabled={!dirty || saveState === "saving"}
            onClick={() => setDraft(settings)}
          >
            <RotateCcw className="h-4 w-4" /> Reset
          </Button>
          <Button
            variant="primary"
            disabled={!dirty || saveState === "saving"}
            onClick={() => void save()}
          >
            {saveState === "saving" ? (
              <Loader2 className="h-4 w-4 animate-spin" />
            ) : saveState === "saved" ? (
              <Check className="h-4 w-4" />
            ) : (
              <Save className="h-4 w-4" />
            )}{" "}
            {saveState === "saving" ? "Saving…" : saveState === "saved" ? "Saved" : "Save changes"}
          </Button>
        </div>
      </div>

      <Card
        title="General"
        description="App behaviour on system startup (applies instantly)."
      >
        <Row label="Start with Windows" hint="Runs quietly in the tray at login">
          <Toggle
            checked={autostart}
            onChange={(v) => void toggleAutostart(v)}
          />
        </Row>
      </Card>

      <Card title="Location" description="Prayer times and the timezone are resolved from your city.">
        <div className="flex flex-wrap items-center justify-between gap-3">
          {settings.location ? (
            <p className="flex items-center gap-2 text-sm text-slate-300">
              <MapPin className="h-4 w-4 text-emerald-400" />
              {settings.location.city}, {settings.location.country}
              <span className="text-xs text-slate-500">
                ({settings.location.timezone})
              </span>
            </p>
          ) : (
            <p className="text-sm text-slate-400">No location set.</p>
          )}
          <Button onClick={() => setChangingLocation(true)}>Change…</Button>
        </div>
      </Card>

      <Card
        title="Prayer time calculation"
        description="Method and Asr school come from the Aladhan API. Changing these refetches the schedule."
      >
        <div className="grid gap-4 sm:grid-cols-2">
          <SelectField
            label="Calculation method"
            value={draft.method}
            onChange={(e) => patch({ method: Number(e.target.value) })}
          >
            {METHODS.map((m) => (
              <option key={m.id} value={m.id}>
                {m.name}
              </option>
            ))}
          </SelectField>
          <SelectField
            label="Asr calculation"
            value={draft.school}
            onChange={(e) =>
              patch({ school: Number(e.target.value) === 1 ? 1 : 0 })
            }
          >
            <option value={0}>Standard (Shafi, Maliki, Hanbali)</option>
            <option value={1}>Hanafi</option>
          </SelectField>
          <SelectField
            label="Time format"
            value={draft.timeFormat}
            onChange={(e) =>
              patch({ timeFormat: e.target.value === "24h" ? "24h" : "12h" })
            }
          >
            <option value="12h">12-hour (5:45 PM)</option>
            <option value="24h">24-hour (17:45)</option>
          </SelectField>
        </div>
      </Card>

      <Card
        title="Alarms"
        description="How you get notified when a prayer alarm fires."
      >
        <div className="space-y-4">
          <Row
            label="System notifications"
            hint="Notification center / toast"
          >
            <Toggle
              checked={draft.notificationsEnabled}
              onChange={(v) => patch({ notificationsEnabled: v })}
            />
          </Row>
          <Row label="Alarm sound" hint="Plays until you respond">
            <Toggle
              checked={draft.soundEnabled}
              onChange={(v) => patch({ soundEnabled: v })}
            />
          </Row>
          <Row
            label="Alarm tone"
            hint={
              settings.alarmSound
                ? `Custom — ${settings.alarmSound} (applies instantly)`
                : "Default chime"
            }
          >
            <div className="flex items-center gap-2">
              <Button size="sm" disabled={toneBusy} onClick={() => void chooseTone()}>
                <FolderOpen className="h-3.5 w-3.5" /> Choose file…
              </Button>
              {settings.alarmSound && (
                <Button
                  size="sm"
                  variant="ghost"
                  disabled={toneBusy}
                  onClick={() => void useDefaultTone()}
                >
                  <Undo2 className="h-3.5 w-3.5" /> Default
                </Button>
              )}
            </div>
          </Row>
          {toneError && (
            <p className="-mt-2 rounded-lg border border-red-900/60 bg-red-950/40 px-3 py-2 text-xs text-red-300">
              {toneError}
            </p>
          )}
          <Row label="Alarm popup" hint="Small always-on-top window">
            <Toggle
              checked={draft.popupEnabled}
              onChange={(v) => patch({ popupEnabled: v })}
            />
          </Row>
          <div className="flex items-end gap-3">
            <div className="flex-1">
              <SliderField
                label={`Volume — ${Math.round(draft.soundVolume * 100)}%`}
                value={Math.round(draft.soundVolume * 100)}
                onChange={(e) =>
                  patch({ soundVolume: Number(e.target.value) / 100 })
                }
              />
            </div>
            <Button size="md" onClick={() => void testSound()}>
              <Volume2 className="h-4 w-4" /> Test
            </Button>
          </div>
        </div>
      </Card>

      <Card
        title="Before-end warning"
        description="If a prayer still isn't marked done shortly before its window closes, a final “you will miss it” alarm fires."
      >
        <div className="flex flex-wrap items-center gap-4">
          <Toggle
            checked={draft.preEndWarningEnabled}
            onChange={(v) => patch({ preEndWarningEnabled: v })}
          />
          <div className="w-40">
            <NumberField
              label="Minutes before end"
              min={1}
              max={120}
              disabled={!draft.preEndWarningEnabled}
              value={draft.preEndWarningMinutes}
              onChange={(e) =>
                patch({ preEndWarningMinutes: clamp(Number(e.target.value), 1, 120) })
              }
            />
          </div>
        </div>
      </Card>

      <Card
        title="Per-prayer alarm timing"
        description="How many minutes AFTER the prayer's start time its alarm fires — e.g. start 11:46 AM with 30 → one single alarm at 12:16 PM (“you still have … minutes”). Clear the number to make the alarm ring exactly at the start time (that's Maghrib's default). The before-end warning below is separate."
      >
        <ul className="divide-y divide-slate-800/70">
          {PRAYERS.map((p) => {
            const ps = draft.prayerSettings[p];
            return (
              <li
                key={p}
                className="flex flex-wrap items-center justify-between gap-3 py-3"
              >
                <div>
                  <p
                    className={cn(
                      "text-sm font-medium",
                      ps.enabled ? "text-slate-200" : "text-slate-500",
                    )}
                  >
                    {PRAYER_LABELS[p]}
                  </p>
                  <p className="text-xs text-slate-500">
                    {ps.enabled
                      ? (reminderHint(p) ?? "Alarms on")
                      : "All alarms off"}
                  </p>
                </div>
                <div className="flex items-center gap-4">
                  <div className="w-40">
                    <input
                      type="number"
                      min={1}
                      max={600}
                      placeholder="Off"
                      disabled={!ps.enabled}
                      value={ps.reminderMinutes ?? ""}
                      onChange={(e) => {
                        const raw = e.target.value;
                        patchPrayer(p, {
                          reminderMinutes:
                            raw === "" ? null : clamp(Number(raw), 1, 600),
                        });
                      }}
                      className={cn(
                        "h-9 w-full rounded-lg border border-slate-700 bg-slate-950/60 px-3 text-sm text-slate-100",
                        "placeholder:text-slate-600 focus:border-emerald-500 focus:outline-none disabled:opacity-40",
                      )}
                      aria-label={`Reminder minutes after ${PRAYER_LABELS[p]} starts`}
                    />
                  </div>
                  <Toggle
                    checked={ps.enabled}
                    onChange={(v) => patchPrayer(p, { enabled: v })}
                  />
                </div>
              </li>
            );
          })}
        </ul>
      </Card>

      <Card
        title="Updates"
        description="Checked automatically at launch. Windows and Linux updates install inside the app; macOS downloads from GitHub."
      >
        <div className="space-y-4">
          <Row
            label="Current version"
            hint={updates.currentVersion ? `v${updates.currentVersion}` : undefined}
          >
            <Button
              size="sm"
              disabled={updates.phase === "checking" || updates.phase === "downloading"}
              onClick={() => void updates.checkForUpdates(false)}
            >
              <RotateCcw
                className={cn(
                  "h-3.5 w-3.5",
                  updates.phase === "checking" && "animate-spin",
                )}
              />
              {updates.phase === "checking" ? "Checking…" : "Check for updates"}
            </Button>
          </Row>

          {updates.phase === "available" && (
            <div className="space-y-3">
              <p className="text-sm text-slate-300">
                <span className="font-medium text-emerald-300">v{updates.newVersion}</span> is
                available
                {updates.currentVersion ? ` (you have v${updates.currentVersion})` : ""}.
              </p>
              {updates.notes && (
                <pre className="max-h-32 overflow-y-auto whitespace-pre-wrap rounded-lg border border-slate-800 bg-slate-950/60 p-3 text-xs leading-relaxed text-slate-400">
                  {updates.notes}
                </pre>
              )}
              {IS_MAC ? (
                <Button size="sm" variant="primary" onClick={updates.openDownloadPage}>
                  <ExternalLink className="h-3.5 w-3.5" /> Download from GitHub
                </Button>
              ) : (
                <Button
                  size="sm"
                  variant="primary"
                  onClick={() => void updates.install()}
                >
                  <Download className="h-3.5 w-3.5" /> Install update
                </Button>
              )}
            </div>
          )}

          {updates.phase === "downloading" && (
            <p className="text-sm text-slate-300">
              Downloading update… the app restarts automatically when ready.
            </p>
          )}

          {updates.phase === "up-to-date" && (
            <p className="text-sm text-slate-400">You're on the latest version.</p>
          )}

          {updates.phase === "error" && (
            <p className="rounded-lg border border-red-900/60 bg-red-950/40 px-3 py-2 text-xs text-red-300">
              {updates.error}
            </p>
          )}
        </div>
      </Card>

      <Card title="About">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <p className="text-sm text-slate-400">
            Developed by <span className="font-medium text-slate-200">Raihan Ali</span>
          </p>
          <div className="flex items-center gap-2">
            <Button
              size="sm"
              variant="ghost"
              onClick={() => void openUrl("https://github.com/iamraihan/prayer-times")}
            >
              <ExternalLink className="h-3.5 w-3.5" /> GitHub
            </Button>
            <Button
              size="sm"
              variant="ghost"
              onClick={() => void openUrl("https://raihanali.com/prayer-times")}
            >
              <ExternalLink className="h-3.5 w-3.5" /> raihanali.com
            </Button>
          </div>
        </div>
      </Card>

      {changingLocation && (
        <div className="fixed inset-0 z-50 overflow-y-auto bg-slate-950/90 p-4">
          <SetupWizard onFinished={() => setChangingLocation(false)} />
        </div>
      )}

      {toast && <Toast toast={toast} onDismiss={() => setToast(null)} />}
    </div>
  );
}

function Row({
  label,
  hint,
  children,
}: {
  label: string;
  hint?: string;
  children: ReactNode;
}) {
  return (
    <div className="flex items-center justify-between gap-4">
      <div>
        <p className="text-sm text-slate-200">{label}</p>
        {hint && <p className="text-xs text-slate-500">{hint}</p>}
      </div>
      {children}
    </div>
  );
}

function clamp(v: number, lo: number, hi: number): number {
  if (!Number.isFinite(v)) return lo;
  return Math.min(hi, Math.max(lo, Math.round(v)));
}

function sameJson(a: unknown, b: unknown): boolean {
  return JSON.stringify(a) === JSON.stringify(b);
}
