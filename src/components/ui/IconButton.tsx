import type { ReactNode } from "react";

/** A square 32px button holding only an icon; `label` names it for screen
 * readers and as a tooltip. */
export function IconButton({
  label,
  onClick,
  disabled,
  children,
}: {
  label: string;
  onClick: () => void;
  disabled?: boolean | undefined;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      disabled={disabled}
      onClick={onClick}
      className="flex h-8 w-8 items-center justify-center rounded-lg text-slate-300 transition-colors hover:bg-slate-800 hover:text-slate-50 disabled:opacity-30 disabled:hover:bg-transparent"
    >
      {children}
    </button>
  );
}
