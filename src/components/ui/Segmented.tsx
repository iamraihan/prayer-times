import type { ReactNode } from "react";
import { cn } from "../../lib/cn";

export interface SegmentItem {
  key: string;
  label: ReactNode;
  onClick: () => void;
  /** Set for a choice group: marks the selected segment. Leave out for a
   * row of actions (snooze presets). */
  pressed?: boolean | undefined;
  /** Tint of the selected segment. */
  tone?: "accent" | "neutral" | undefined;
  ariaLabel?: string | undefined;
}

/** A joined row of small buttons: a choice group (with `pressed`) or a row
 * of quick actions. One look for both, 32px tall. */
export function Segmented({
  items,
  ariaLabel,
  disabled,
}: {
  items: SegmentItem[];
  ariaLabel: string;
  disabled?: boolean | undefined;
}) {
  return (
    <div
      role="group"
      aria-label={ariaLabel}
      className="flex overflow-hidden rounded-lg border border-slate-700 bg-slate-950/40"
    >
      {items.map((item, i) => (
        <button
          key={item.key}
          type="button"
          disabled={disabled}
          aria-pressed={item.pressed}
          aria-label={item.ariaLabel}
          onClick={item.onClick}
          className={cn(
            "tnum h-8 px-3 text-[13px] transition-colors disabled:opacity-45",
            "focus-visible:relative focus-visible:z-10",
            i > 0 && "border-l border-slate-700",
            item.pressed
              ? item.tone === "accent"
                ? "bg-emerald-500/20 text-emerald-200"
                : "bg-slate-700/60 text-slate-100"
              : item.pressed === false
                ? "text-slate-400 hover:bg-slate-800 hover:text-slate-200"
                : "text-slate-200 hover:bg-slate-800",
          )}
        >
          {item.label}
        </button>
      ))}
    </div>
  );
}
