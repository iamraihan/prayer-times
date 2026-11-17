import { useCallback, useState } from "react";
import { create } from "zustand";
import { api } from "./ipc";
import { friendlyError } from "./errors";

/** Shared across every Guide button, so all of them show the busy state. */
const useGuideStore = create<{ opening: boolean }>(() => ({ opening: false }));

/**
 * Opens the video guide window. `opening` covers the backend's reachability
 * check and is shared by every caller; `error` is a sentence for this caller
 * to show (each button reports its own), cleared on the next attempt.
 */
export function useGuide() {
  const opening = useGuideStore((s) => s.opening);
  const [error, setError] = useState<string | null>(null);

  const open = useCallback(async () => {
    if (useGuideStore.getState().opening) return;
    setError(null);
    useGuideStore.setState({ opening: true });
    try {
      await api.openGuide();
    } catch (e) {
      setError(`Couldn't open the video guide. ${friendlyError(e)}`);
    } finally {
      useGuideStore.setState({ opening: false });
    }
  }, []);

  const clearError = useCallback(() => setError(null), []);

  return { open, opening, error, clearError };
}
