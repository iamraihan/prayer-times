import type {
  InputHTMLAttributes,
  ReactNode,
  SelectHTMLAttributes,
} from "react";
import { ChevronDown } from "lucide-react";
import { cn } from "../../lib/cn";

const fieldBase =
  "h-9 w-full rounded-lg border border-slate-700 bg-slate-950/50 px-3 text-sm text-slate-100 " +
  "transition-colors placeholder:text-slate-500 hover:border-slate-600 " +
  "focus:border-emerald-500 focus:outline-none focus-visible:outline-none disabled:opacity-45 " +
  "aria-[invalid=true]:border-red-400/70";

export function Label({ children, hint }: { children: ReactNode; hint?: string | undefined }) {
  return (
    <label className="mb-1.5 flex items-baseline justify-between gap-2 text-[13px] font-medium text-slate-300">
      <span>{children}</span>
      {hint && <span className="font-normal text-slate-500">{hint}</span>}
    </label>
  );
}

export interface TextFieldProps extends InputHTMLAttributes<HTMLInputElement> {
  label?: string;
  hint?: string | undefined;
}

export function TextField({ label, hint, className, ...rest }: TextFieldProps) {
  return (
    <div className="w-full">
      {label && <Label hint={hint}>{label}</Label>}
      <input className={cn(fieldBase, className)} {...rest} />
    </div>
  );
}

export interface SelectFieldProps
  extends SelectHTMLAttributes<HTMLSelectElement> {
  label?: string;
  hint?: string | undefined;
  children: ReactNode;
}

export function SelectField({
  label,
  hint,
  className,
  children,
  ...rest
}: SelectFieldProps) {
  return (
    <div className="w-full">
      {label && <Label hint={hint}>{label}</Label>}
      <Select className={className} {...rest}>
        {children}
      </Select>
    </div>
  );
}

/** Native select (keeps the OS list and keyboard behaviour) with a drawn chevron. */
export function Select({ className, children, ...rest }: SelectHTMLAttributes<HTMLSelectElement>) {
  return (
    <div className={cn("relative", className)}>
      <select className={cn(fieldBase, "cursor-pointer appearance-none pr-9")} {...rest}>
        {children}
      </select>
      <ChevronDown
        className="pointer-events-none absolute top-1/2 right-3 h-4 w-4 -translate-y-1/2 text-slate-400"
        aria-hidden
      />
    </div>
  );
}

/** Small number box with a unit suffix, e.g. "30 min". */
export function UnitInput({
  unit,
  className,
  ...rest
}: InputHTMLAttributes<HTMLInputElement> & { unit: string }) {
  return (
    <div className={cn("relative w-24", className)}>
      <input type="number" className={cn(fieldBase, "tnum pr-11 text-right")} {...rest} />
      <span className="pointer-events-none absolute top-1/2 right-3 -translate-y-1/2 text-[13px] text-slate-500">
        {unit}
      </span>
    </div>
  );
}

export interface SliderFieldProps
  extends InputHTMLAttributes<HTMLInputElement> {
  label?: string;
}

export function SliderField({ label, className, style, ...rest }: SliderFieldProps) {
  const fill = `${Math.min(100, Math.max(0, Number(rest.value ?? 0)))}%`;
  return (
    <div className="w-full">
      {label && <Label>{label}</Label>}
      <input
        type="range"
        min={0}
        max={100}
        className={cn("range w-full cursor-pointer rounded-md", className)}
        style={{ ...style, ["--fill" as string]: fill }}
        {...rest}
      />
    </div>
  );
}
