/**
 * Runs a tab's Tour with driver.js the first time the user opens that tab
 * (and again for Tour Steps added by an update). Which steps are pending
 * comes from `tour.ts`; what has been seen comes from Rust
 * (`AppState.seenTourSteps`) and is written back with `markTourSteps`.
 */
import { useEffect } from "react";
import { driver, type DriveStep, type Driver } from "driver.js";
import { markTourSteps } from "./actions";
import { STEPS, isUpdateTour, pendingSteps, type TourName } from "./tour";
import type { TourStep } from "./types";
import { useAppStore } from "../store/useAppStore";

/** How long to wait for a tab's controls to render (History loads async). */
const WAIT_MS = 2000;

/** The running Tour; at most one at a time across tabs. */
let active: Driver | null = null;
/** Steps started this session, so a failed save can't replay them on every
 * tab switch. Cleared by "Replay tours". */
const shownThisSession = new Set<TourStep>();

export function forgetShownTours(): void {
  shownThisSession.clear();
}

function selector(target: string): string {
  return `[data-tour="${target}"]`;
}

function present(id: TourStep): boolean {
  const target = STEPS[id].target;
  return target === null || document.querySelector(selector(target)) !== null;
}

/** Resolves once every step's control is in the page, or after `WAIT_MS`. */
function waitForTargets(ids: readonly TourStep[], signal: AbortSignal): Promise<void> {
  return new Promise((resolve) => {
    if (ids.every(present)) {
      resolve();
      return;
    }
    const finish = () => {
      observer.disconnect();
      clearTimeout(timer);
      signal.removeEventListener("abort", finish);
      resolve();
    };
    const observer = new MutationObserver(() => {
      if (ids.every(present)) finish();
    });
    observer.observe(document.body, { childList: true, subtree: true });
    const timer = setTimeout(finish, WAIT_MS);
    signal.addEventListener("abort", finish);
  });
}

function start(ids: readonly TourStep[], isUpdate: boolean): void {
  if (ids.length === 0) return;
  for (const id of ids) shownThisSession.add(id);
  const settings = useAppStore.getState().state?.settings ?? null;

  const steps: DriveStep[] = ids.map((id) => {
    const def = STEPS[id];
    const description =
      typeof def.description === "function" ? (settings ? def.description(settings) : "") : def.description;
    return {
      ...(def.target !== null ? { element: selector(def.target) } : {}),
      popover: {
        title: `${isUpdate ? "New · " : ""}${def.title}`,
        description,
        ...(def.side !== undefined ? { side: def.side } : {}),
      },
    };
  });

  // Highest step index the user reached; everything up to it was viewed.
  let reached = -1;
  const reduceMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;

  // driver.js only repositions on window resize/scroll; the app scrolls
  // inside <main> and its content can change height (alarm banner, ticks).
  const main = document.querySelector("main");
  const refresh = () => active?.refresh();
  const resizes = new ResizeObserver(refresh);
  if (main) {
    resizes.observe(main);
    if (main.firstElementChild) resizes.observe(main.firstElementChild);
    main.addEventListener("scroll", refresh, { passive: true });
  }

  active = driver({
    steps,
    showProgress: true,
    progressText: "{{current}} of {{total}}",
    allowClose: true,
    // A stray click on the dimmed page (say, on a tab) shouldn't skip the
    // rest of the Tour; ✕, Esc and Done still end it.
    overlayClickBehavior: "none",
    disableActiveInteraction: true,
    animate: !reduceMotion,
    // A smooth scroll can outlast the highlight transition and leave the
    // cut-out where the control used to be.
    smoothScroll: false,
    overlayColor: "#0d1418", // --color-slate-950
    popoverClass: "pt-tour",
    nextBtnText: "Next",
    prevBtnText: "Back",
    doneBtnText: "Done",
    stagePadding: 6,
    stageRadius: 12,
    onHighlighted: (_element, _step, { index }) => {
      if (index !== undefined) reached = Math.max(reached, index);
    },
    // Not onDestroyed: driver.js skips that hook when the tour is closed
    // before the first highlight transition ends, which would leave
    // `active` set for the whole session. onDestroyStarted always runs, and
    // the tour then has to be destroyed by hand.
    onDestroyStarted: (_element, _step, { driver: d }) => {
      reached = Math.max(reached, d.getActiveIndex() ?? 0);
      resizes.disconnect();
      main?.removeEventListener("scroll", refresh);
      active = null;
      d.destroy();
      void record(ids.slice(0, reached + 1), ids.slice(reached + 1));
    },
  });
  active.drive();
}

async function record(viewed: TourStep[], skipped: TourStep[]): Promise<void> {
  try {
    if (viewed.length > 0) await markTourSteps(viewed, "viewed");
    if (skipped.length > 0) await markTourSteps(skipped, "skipped");
  } catch (e) {
    console.error("couldn't save tour progress:", e);
  }
}

export function useTour(tab: TourName): void {
  // A string, so the 15 s state ticks (new arrays, same content) don't
  // re-run the effect.
  const seenKey = useAppStore((s) => s.state?.seenTourSteps?.join(",") ?? null);

  useEffect(() => {
    const seen = useAppStore.getState().state?.seenTourSteps ?? null;
    if (seen === null || active !== null) return;
    const pending = pendingSteps(tab, seen).filter((id) => !shownThisSession.has(id));
    if (pending.length === 0) return;

    // Leaving the tab while waiting cancels the start; a running Tour is
    // never torn down here.
    const controller = new AbortController();
    void waitForTargets(pending, controller.signal).then(() => {
      if (controller.signal.aborted || active !== null) return;
      start(pending.filter(present), isUpdateTour(tab, seen));
    });
    return () => controller.abort();
  }, [tab, seenKey]);
}
