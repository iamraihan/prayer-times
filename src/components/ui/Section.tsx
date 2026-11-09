import type { ReactNode } from "react";
import { cn } from "../../lib/cn";

/** A settings group: heading + optional description above one grouped list
 * of rows. The list is the only container — rows inside are separated by
 * hairlines, never boxed again. */
export function Section({
  title,
  description,
  children,
  className,
}: {
  title: string;
  description?: ReactNode;
  children: ReactNode;
  className?: string;
}) {
  return (
    <section className={cn("space-y-2.5", className)}>
      <header className="px-1">
        <h2 className="text-[13px] font-semibold text-slate-200">{title}</h2>
        {description && (
          <p className="mt-0.5 max-w-prose text-[13px] leading-relaxed text-slate-500">
            {description}
          </p>
        )}
      </header>
      <div className="divide-y divide-slate-800/80 rounded-xl border border-slate-800/80 bg-slate-900/50">
        {children}
      </div>
    </section>
  );
}

/** One row of a Section: label (+ hint) on the left, control on the right. */
export function SettingRow({
  label,
  hint,
  children,
  disabled,
}: {
  label: ReactNode;
  hint?: ReactNode;
  children?: ReactNode;
  disabled?: boolean;
}) {
  return (
    <div className="flex min-h-14 flex-wrap items-center justify-between gap-x-6 gap-y-2 px-4 py-3">
      <div className={cn("min-w-0", disabled && "opacity-50")}>
        <div className="text-sm text-slate-100">{label}</div>
        {hint && <div className="mt-0.5 text-[13px] text-slate-500">{hint}</div>}
      </div>
      {children && <div className="flex items-center gap-2">{children}</div>}
    </div>
  );
}
