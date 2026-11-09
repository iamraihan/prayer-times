import { useState } from "react";
import { open, save } from "@tauri-apps/plugin-dialog";
import { Button } from "./ui/Button";
import { SettingRow } from "./ui/Section";
import { Toast, type ToastData } from "./ui/Toast";
import { importBackup } from "../lib/actions";
import { api } from "../lib/ipc";
import { useAppState } from "../store/useAppStore";
import type { BackupPreview } from "../lib/types";
import { friendlyError } from "../lib/errors";

const MONTH_SHORT = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

/** "2026-11-09" → "9 Nov 2026". */
function readableDate(day: string): string {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(day);
  return m ? `${Number(m[3])} ${MONTH_SHORT[Number(m[2]) - 1]} ${m[1]}` : day;
}

/**
 * Backup rows (export / import with a confirm step) for a `Section`'s list.
 * Used in Settings → General and at the bottom of the History tab. With
 * `historyOnly` (History tab) an import never touches settings: no
 * "also use these settings" choice, and the summary talks about history only.
 * The exported file is the same full backup either way.
 */
export function BackupControls({
  onImported,
  historyOnly = false,
}: {
  onImported?: (() => void) | undefined;
  historyOnly?: boolean | undefined;
}) {
  const state = useAppState();
  const [busy, setBusy] = useState(false);
  const [toast, setToast] = useState<ToastData | null>(null);
  const [importing, setImporting] = useState<{
    path: string;
    preview: BackupPreview;
    includeSettings: boolean;
  } | null>(null);

  async function exportFile() {
    const today = state?.today?.date ?? "backup";
    const path = await save({
      defaultPath: `prayer-times-backup-${today}.json`,
      filters: [{ name: "Backup", extensions: ["json"] }],
    });
    if (!path) return;
    setBusy(true);
    try {
      await api.exportBackup(path);
      setToast({ kind: "success", message: "Backup saved." });
    } catch (e) {
      setToast({ kind: "error", message: `Couldn't export the backup. ${friendlyError(e)}` });
    } finally {
      setBusy(false);
    }
  }

  async function pickFile() {
    const path = await open({ multiple: false, filters: [{ name: "Backup", extensions: ["json"] }] });
    if (typeof path !== "string") return;
    try {
      const preview = await api.previewBackup(path);
      setImporting({ path, preview, includeSettings: !historyOnly && !state?.hasLocation });
    } catch (e) {
      setToast({ kind: "error", message: friendlyError(e) });
    }
  }

  async function confirmImport() {
    if (!importing) return;
    setBusy(true);
    try {
      await importBackup(importing.path, !historyOnly && importing.includeSettings);
      setImporting(null);
      setToast({ kind: "success", message: "Backup imported." });
      onImported?.();
    } catch (e) {
      setToast({ kind: "error", message: `Couldn't import the backup. ${friendlyError(e)}` });
    } finally {
      setBusy(false);
    }
  }

  const p = importing?.preview;
  return (
    <>
      <SettingRow
        label="Backup"
        hint={
          historyOnly
            ? "Save your prayer history to a file, or bring it in from another computer"
            : "Your prayer history and settings in one file, to move to another computer"
        }
      >
        <Button size="sm" disabled={busy} onClick={() => void exportFile()}>
          Export…
        </Button>
        <Button size="sm" variant="ghost" disabled={busy} onClick={() => void pickFile()}>
          Import…
        </Button>
      </SettingRow>
      {importing && p && (
        <div className="space-y-3 px-4 py-4">
          <p className="text-sm text-slate-200">
            {p.days} {p.days === 1 ? "day" : "days"} of history
            {p.firstDay && p.lastDay ? ` (${readableDate(p.firstDay)} – ${readableDate(p.lastDay)})` : ""}
            {!historyOnly && p.location ? `, settings for ${p.location}` : ""}.
          </p>
          <p className="text-[13px] text-slate-400">
            History is merged with what's here; nothing is deleted.
            {historyOnly
              ? " Your settings stay as they are."
              : p.hasCustomTone && " The custom alarm tone isn't in backups, so this computer keeps its own."}
          </p>
          {!historyOnly && (
            <label className="flex items-center gap-2 text-[13px] text-slate-200">
              <input
                type="checkbox"
                checked={importing.includeSettings}
                onChange={(e) => setImporting({ ...importing, includeSettings: e.target.checked })}
                className="h-4 w-4 accent-emerald-500"
              />
              Also use these settings (location, alarms)
            </label>
          )}
          <div className="flex gap-2">
            <Button size="sm" variant="primary" disabled={busy} onClick={() => void confirmImport()}>
              Import
            </Button>
            <Button size="sm" variant="ghost" disabled={busy} onClick={() => setImporting(null)}>
              Cancel
            </Button>
          </div>
        </div>
      )}
      {toast && <Toast toast={toast} onDismiss={() => setToast(null)} />}
    </>
  );
}
