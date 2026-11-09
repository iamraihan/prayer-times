import { useEffect, useRef, useState } from "react";
import { Check, Loader2, MapPin, Search } from "lucide-react";
import { searchCities } from "../lib/actions";
import type { CityMatch } from "../lib/types";
import { cn } from "../lib/cn";

/** What the user chose: a real place from search (exact coordinates), or —
 * only when search can't find their town — the name as typed. */
export type CityChoice =
  | { kind: "picked"; match: CityMatch }
  | { kind: "typed"; name: string };

export interface CityPickerProps {
  /** ISO code of the selected country; search is disabled until it is set. */
  countryCode: string | null;
  countryName: string | null;
  value: CityChoice | null;
  onChange: (choice: CityChoice | null) => void;
}

const DEBOUNCE_MS = 300;

export function cityLabel(match: CityMatch): string {
  return match.region ? `${match.name}, ${match.region}` : match.name;
}

/** Search-as-you-type city field backed by the geocoding search. */
export function CityPicker({ countryCode, countryName, value, onChange }: CityPickerProps) {
  const [query, setQuery] = useState(() =>
    value?.kind === "picked" ? cityLabel(value.match) : (value?.name ?? ""),
  );
  const [results, setResults] = useState<CityMatch[]>([]);
  const [searching, setSearching] = useState(false);
  const [searchError, setSearchError] = useState<string | null>(null);
  const [searched, setSearched] = useState<string | null>(null);
  const [open, setOpen] = useState(false);
  const [active, setActive] = useState(0);
  const seq = useRef(0);

  // A new country invalidates the text and any choice.
  const firstCountry = useRef(countryCode);
  useEffect(() => {
    if (firstCountry.current === countryCode) return;
    firstCountry.current = countryCode;
    setQuery("");
    setResults([]);
    setSearched(null);
    onChange(null);
  }, [countryCode, onChange]);

  const term = query.trim();
  const showingChoice =
    value?.kind === "picked" ? cityLabel(value.match) === query : value?.name === query;

  useEffect(() => {
    if (!countryCode || term.length < 2 || showingChoice) {
      setResults([]);
      setSearching(false);
      return;
    }
    const id = ++seq.current;
    setSearching(true);
    const timer = setTimeout(() => {
      searchCities(term, countryCode)
        .then((found) => {
          if (id !== seq.current) return;
          setResults(found);
          setSearchError(null);
          setActive(0);
        })
        .catch((e: unknown) => {
          if (id !== seq.current) return;
          setResults([]);
          setSearchError(String(e));
        })
        .finally(() => {
          if (id !== seq.current) return;
          setSearched(term);
          setSearching(false);
        });
    }, DEBOUNCE_MS);
    return () => clearTimeout(timer);
  }, [term, countryCode, showingChoice]);

  function pick(match: CityMatch) {
    onChange({ kind: "picked", match });
    setQuery(cityLabel(match));
    setOpen(false);
  }

  function useTyped() {
    onChange({ kind: "typed", name: term });
    setQuery(term);
    setOpen(false);
  }

  const noMatch = !searching && searched === term && results.length === 0 && term.length >= 2;

  return (
    <div className="relative">
      <label className="mb-1.5 block text-xs font-medium text-slate-300">City</label>
      <div
        className={cn(
          "flex h-10 items-center gap-2 rounded-lg border border-slate-700 bg-slate-950/60 px-3",
          "focus-within:border-emerald-500",
          !countryCode && "opacity-50",
        )}
      >
        {value?.kind === "picked" && showingChoice ? (
          <Check className="h-4 w-4 shrink-0 text-emerald-400" />
        ) : (
          <Search className="h-4 w-4 shrink-0 text-slate-500" />
        )}
        <input
          value={query}
          disabled={!countryCode}
          placeholder={countryCode ? "Start typing your city…" : "Select a country first"}
          onChange={(e) => {
            setQuery(e.target.value);
            setOpen(true);
            if (value) onChange(null);
          }}
          onFocus={() => setOpen(true)}
          onKeyDown={(e) => {
            if (!open || results.length === 0) return;
            if (e.key === "ArrowDown") {
              e.preventDefault();
              setActive((i) => Math.min(i + 1, results.length - 1));
            } else if (e.key === "ArrowUp") {
              e.preventDefault();
              setActive((i) => Math.max(i - 1, 0));
            } else if (e.key === "Enter") {
              e.preventDefault();
              const match = results[active];
              if (match) pick(match);
            } else if (e.key === "Escape") {
              setOpen(false);
            }
          }}
          role="combobox"
          aria-expanded={open && results.length > 0}
          aria-autocomplete="list"
          className="w-full bg-transparent text-sm text-slate-100 placeholder:text-slate-500 focus:outline-none"
        />
        {searching && <Loader2 className="h-4 w-4 shrink-0 animate-spin text-slate-500" />}
      </div>

      {open && results.length > 0 && !showingChoice && (
        <>
          <div className="fixed inset-0 z-10" onClick={() => setOpen(false)} />
          <ul
            role="listbox"
            className="absolute z-20 mt-1 max-h-64 w-full overflow-y-auto rounded-lg border border-slate-700 bg-slate-900 py-1 shadow-xl shadow-black/40"
          >
            {results.map((m, i) => (
              <li key={`${m.latitude},${m.longitude}`} role="option" aria-selected={i === active}>
                <button
                  type="button"
                  onMouseEnter={() => setActive(i)}
                  onClick={() => pick(m)}
                  className={cn(
                    "flex w-full items-center gap-2 px-3 py-1.5 text-left text-sm text-slate-200",
                    i === active && "bg-slate-800",
                  )}
                >
                  <MapPin className="h-3.5 w-3.5 shrink-0 text-slate-500" />
                  <span>{m.name}</span>
                  {m.region && <span className="text-xs text-slate-500">{m.region}</span>}
                </button>
              </li>
            ))}
          </ul>
        </>
      )}

      {value?.kind === "picked" && showingChoice && (
        <p className="mt-1.5 text-xs text-slate-500">
          {Math.abs(value.match.latitude).toFixed(2)}°{value.match.latitude >= 0 ? "N" : "S"},{" "}
          {Math.abs(value.match.longitude).toFixed(2)}°{value.match.longitude >= 0 ? "E" : "W"} ·{" "}
          {value.match.timezone}
        </p>
      )}

      {value?.kind === "typed" && showingChoice && (
        <p className="mt-1.5 text-xs text-amber-400/90">
          Using “{value.name}” as typed. The times service will look it up by name, so check the
          preview carefully.
        </p>
      )}

      {!value && (noMatch || searchError) && (
        <div className="mt-1.5 space-y-1 text-xs">
          <p className="text-amber-400/90">
            {searchError
              ? "City search is unavailable right now."
              : `No town called “${term}” found in ${countryName ?? "this country"}. Check the spelling.`}
          </p>
          <button
            type="button"
            onClick={useTyped}
            className="text-slate-400 underline underline-offset-2 hover:text-slate-200"
          >
            Use “{term}” as typed anyway
          </button>
        </div>
      )}
    </div>
  );
}
