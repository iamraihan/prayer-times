import { create } from "zustand";
import { getVersion } from "@tauri-apps/api/app";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { openUrl } from "@tauri-apps/plugin-opener";
import { friendlyError } from "../lib/errors";

export const RELEASES_URL = "https://github.com/iamraihan/prayer-times/releases/latest";

/**
 * Silent in-place updates on macOS require an Apple Developer ID-signed
 * build; until one exists, macOS falls back to opening the release page.
 */
export const IS_MAC = /Mac/i.test(navigator.platform || navigator.userAgent);

type Phase =
  | "idle" // nothing checked yet (startup / silent check failed)
  | "checking"
  | "available"
  | "downloading"
  | "up-to-date"
  | "error";

interface UpdateStore {
  phase: Phase;
  /** The pending update object — carries downloadAndInstall(). */
  update: Update | null;
  currentVersion: string | null;
  newVersion: string | null;
  notes: string | null;
  downloadedBytes: number;
  totalBytes: number | null;
  error: string | null;
  /** User closed the banner — stays hidden until the next check. */
  dismissed: boolean;
  init: () => Promise<void>;
  checkForUpdates: (silent: boolean) => Promise<void>;
  install: () => Promise<void>;
  openDownloadPage: () => void;
  dismiss: () => void;
}

export const useUpdateStore = create<UpdateStore>((set, get) => ({
  phase: "idle",
  update: null,
  currentVersion: null,
  newVersion: null,
  notes: null,
  downloadedBytes: 0,
  totalBytes: null,
  error: null,
  dismissed: false,

  init: async () => {
    if (get().currentVersion) return;
    try {
      set({ currentVersion: await getVersion() });
    } catch {
      // Non-fatal — the UI just omits the version line.
    }
    await get().checkForUpdates(true);
  },

  checkForUpdates: async (silent) => {
    const { phase } = get();
    if (phase === "checking" || phase === "downloading") return;
    set({ phase: "checking", error: null });
    try {
      const update = await check();
      if (update) {
        set({
          phase: "available",
          update,
          newVersion: update.version,
          notes: update.body ?? null,
          dismissed: false,
        });
      } else {
        set({ phase: "up-to-date", update: null, newVersion: null, notes: null });
      }
    } catch (e) {
      // Startup checks stay quiet — no error UI without a user action.
      set(silent ? { phase: "idle" } : { phase: "error", error: friendlyError(e) });
    }
  },

  install: async () => {
    const update = get().update;
    if (!update) return;
    set({ phase: "downloading", downloadedBytes: 0, totalBytes: null, error: null });
    try {
      await update.downloadAndInstall((event) => {
        switch (event.event) {
          case "Started":
            set({ totalBytes: event.data.contentLength ?? null });
            break;
          case "Progress":
            set({ downloadedBytes: get().downloadedBytes + event.data.chunkLength });
            break;
          case "Finished":
            break;
        }
      });
      await relaunch();
    } catch (e) {
      set({ phase: "error", error: friendlyError(e) });
    }
  },

  openDownloadPage: () => {
    void openUrl(RELEASES_URL);
  },

  dismiss: () => set({ dismissed: true }),
}));
