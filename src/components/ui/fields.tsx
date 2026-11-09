import type {
  InputHTMLAttributes,
  ReactNode,
  SelectHTMLAttributes,
} from "react";
import { cn } from "../../lib/cn";

const fieldBase =
  "w-full rounded-lg border border-slate-700 bg-slate-950/60 px-3 py-2 text-sm text-slate-100 " +
  "placeholder:text-slate-500 focus:border-emerald-500 focus:outline-none disabled:opacity-50";

export function Label({ children, hint }: { children: ReactNode; hint?: string | undefined }) {
  return (
    <label className="mb-1.5 flex items-baseline justify-between gap-2 text-xs font-medium text-slate-300">
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

export interface NumberFieldProps extends InputHTMLAttributes<HTMLInputElement> {
  label?: string;
  hint?: string | undefined;
}

export function NumberField({ label, hint, className, ...rest }: NumberFieldProps) {
  return (
    <div className="w-full">
      {label && <Label hint={hint}>{label}</Label>}
      <input type="number" className={cn(fieldBase, className)} {...rest} />
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
      <select className={cn(fieldBase, "appearance-none pr-8", className)} {...rest}>
        {children}
      </select>
    </div>
  );
}

export interface SliderFieldProps
  extends InputHTMLAttributes<HTMLInputElement> {
  label?: string;
}

export function SliderField({ label, className, ...rest }: SliderFieldProps) {
  return (
    <div className="w-full">
      {label && <Label>{label}</Label>}
      <input
        type="range"
        min={0}
        max={100}
        className={cn(
          "h-2 w-full cursor-pointer appearance-none rounded-full bg-slate-700 accent-emerald-500",
          className,
        )}
        {...rest}
      />
    </div>
  );
}
