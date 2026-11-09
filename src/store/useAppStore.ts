import { create } from "zustand";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { api } from "../lib/ipc";
import type { AppState } from "../lib/types";
import { friendlyError } from "../lib/errors";

interface AppStore {
  state: AppState | null;
  /** Ticking epoch ms — re-renders countdowns once a second. */
  now: number;
  ready: boolean;
  initError: string | null;
  init: () => Promise<void>;
  apply: (s: AppState) => void;
  retry: () => void;
}

let unlisten: UnlistenFn | null = null;

export const useAppStore = create<AppStore>((set, get) => ({
  state: null,
  now: Date.now(),
  ready: false,
  initError: null,

  init: async () => {
    set({ initError: null });
    try {
      const s = await api.getState();
      set({ state: s, ready: true });
    } catch (e) {
      set({ initError: friendlyError(e), ready: true });
      return;
    }
    // The backend pushes a fresh snapshot after every tick / state change.
    if (!unlisten) {
      unlisten = await listen<AppState>("app-state", (e) =>
        set({ state: e.payload }),
      );
    }
  },

  apply: (s) => set({ state: s, now: Date.now() }),

  retry: () => {
    void get().init();
  },
}));

let clockStarted = false;
/** Starts the one-second countdown ticker (idempotent, per-window). */
export function startClock(): void {
  if (clockStarted) return;
  clockStarted = true;
  setInterval(() => useAppStore.setState({ now: Date.now() }), 1000);
}

export function useAppState(): AppState | null {
  return useAppStore((s) => s.state);
}

export function useNow(): number {
  return useAppStore((s) => s.now);
}
