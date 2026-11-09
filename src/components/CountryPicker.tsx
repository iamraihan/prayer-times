import { useEffect, useId, useMemo, useRef, useState } from "react";
import { ChevronDown, Search } from "lucide-react";
import { COUNTRIES, type Country } from "../lib/countries";
import { cn } from "../lib/cn";

export interface CountryPickerProps {
  value: Country | null;
  onChange: (country: Country | null) => void;
  /** Id of the visible label, for screen readers. */
  labelId?: string | undefined;
}

/**
 * Searchable country dropdown. The trigger opens a search box with a list:
 * type to filter, ↑/↓ to move, Enter to pick, Esc to close (focus returns to
 * the trigger). The Aladhan API receives the ISO alpha-2 code.
 */
export function CountryPicker({ value, onChange, labelId }: CountryPickerProps) {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const trigger = useRef<HTMLButtonElement>(null);
  const list = useRef<HTMLUListElement>(null);
  const listId = useId();
  const optionId = (i: number) => `${listId}-opt-${i}`;

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return COUNTRIES;
    return COUNTRIES.filter((c) => c.name.toLowerCase().includes(q) || c.code.toLowerCase() === q);
  }, [query]);

  // Start on the current choice when opening; keep the active row visible.
  useEffect(() => {
    if (!open) return;
    const i = value ? filtered.findIndex((c) => c.code === value.code) : 0;
    setActive(Math.max(0, i));
  }, [open]);
  useEffect(() => {
    list.current?.querySelector(`#${CSS.escape(optionId(active))}`)?.scrollIntoView({ block: "nearest" });
  }, [active, open]);

  function close(refocus: boolean) {
    setOpen(false);
    setQuery("");
    if (refocus) trigger.current?.focus();
  }

  function pick(c: Country) {
    onChange(c);
    close(true);
  }

  return (
    <div className="relative">
      <button
        ref={trigger}
        type="button"
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-labelledby={labelId}
        onClick={() => (open ? close(false) : setOpen(true))}
        onKeyDown={(e) => {
          if (e.key === "ArrowDown" || e.key === "ArrowUp") {
            e.preventDefault();
            setOpen(true);
          }
        }}
        className={cn(
          "flex h-9 w-full items-center justify-between rounded-lg border border-slate-700 bg-slate-950/50 px-3 text-sm",
          "transition-colors hover:border-slate-600 focus-visible:border-emerald-500",
        )}
      >
        <span className={value ? "text-slate-100" : "text-slate-500"}>
          {value ? value.name : "Select a country…"}
        </span>
        <ChevronDown className="h-4 w-4 text-slate-400" aria-hidden />
      </button>

      {open && (
        <>
          <div className="fixed inset-0 z-10" onClick={() => close(false)} aria-hidden />
          <div className="absolute z-20 mt-1 w-full rounded-lg border border-slate-700 bg-slate-900 shadow-xl shadow-black/40">
            <div className="flex items-center gap-2 border-b border-slate-800 px-3 py-2">
              <Search className="h-4 w-4 shrink-0 text-slate-500" aria-hidden />
              <input
                autoFocus
                value={query}
                onChange={(e) => {
                  setQuery(e.target.value);
                  setActive(0);
                }}
                onKeyDown={(e) => {
                  if (e.key === "ArrowDown") {
                    e.preventDefault();
                    setActive((i) => Math.min(i + 1, filtered.length - 1));
                  } else if (e.key === "ArrowUp") {
                    e.preventDefault();
                    setActive((i) => Math.max(i - 1, 0));
                  } else if (e.key === "Enter") {
                    e.preventDefault();
                    const c = filtered[active];
                    if (c) pick(c);
                  } else if (e.key === "Escape") {
                    e.preventDefault();
                    close(true);
                  } else if (e.key === "Tab") {
                    close(false);
                  }
                }}
                placeholder="Search country…"
                role="combobox"
                aria-label="Search countries"
                aria-expanded="true"
                aria-controls={listId}
                aria-autocomplete="list"
                aria-activedescendant={filtered[active] ? optionId(active) : undefined}
                className="w-full bg-transparent text-sm text-slate-100 placeholder:text-slate-500 focus:outline-none"
              />
            </div>
            <ul ref={list} id={listId} role="listbox" aria-labelledby={labelId} className="max-h-56 overflow-y-auto py-1">
              {filtered.length === 0 && (
                <li className="px-3 py-2 text-sm text-slate-500" role="presentation">
                  No country matches “{query.trim()}”
                </li>
              )}
              {filtered.map((c, i) => (
                <li
                  key={c.code}
                  id={optionId(i)}
                  role="option"
                  aria-selected={c.code === value?.code}
                  onMouseEnter={() => setActive(i)}
                  onClick={() => pick(c)}
                  className={cn(
                    "flex cursor-pointer items-center px-3 py-1.5 text-sm",
                    i === active && "bg-slate-800",
                    c.code === value?.code ? "text-emerald-300" : "text-slate-200",
                  )}
                >
                  {c.name}
                  <span className="ml-2 text-xs text-slate-500">{c.code}</span>
                </li>
              ))}
            </ul>
          </div>
        </>
      )}
    </div>
  );
}
