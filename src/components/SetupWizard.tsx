import { useEffect, useState } from "react";
import { ArrowLeft, ArrowRight, Check, MapPin, Sunrise } from "lucide-react";
import logo from "../assets/logo.png";
import { Button } from "./ui/Button";
import { Card } from "./ui/Card";
import { SelectField } from "./ui/fields";
import { CountryPicker } from "./CountryPicker";
import { CityPicker, cityLabel, type CityChoice } from "./CityPicker";
import type { Country } from "../lib/countries";
import { METHODS, METHOD_BY_COUNTRY } from "../lib/methods";
import { previewLocation, saveLocation } from "../lib/actions";
import type { AsrSchool, DayTimings, LocationPreview } from "../lib/types";
import { PRAYERS, PRAYER_LABELS } from "../lib/types";
import { PrayerIcon } from "./PrayerIcon";
import { fmtHM, fmtLongDate } from "../lib/time";
import { cn } from "../lib/cn";
import { friendlyError as sharedFriendlyError } from "../lib/errors";

export interface SetupWizardProps {
  /** Called after the location was saved successfully. */
  onFinished?: () => void;
  /** When set, the wizard is changing an existing location: it can be
   * cancelled and isn't titled as a first-run welcome. */
  onCancel?: () => void;
}

const STEPS = ["Location", "Calculation", "Confirm"] as const;

/**
 * First-run wizard: country + city, calculation method, then a live preview
 * fetched from the Aladhan API before anything is persisted.
 */
export function SetupWizard({ onFinished, onCancel }: SetupWizardProps) {
  const [step, setStep] = useState(0);

  const [country, setCountry] = useState<Country | null>(null);
  const [city, setCity] = useState<CityChoice | null>(null);
  const [method, setMethod] = useState<number>(3);
  const [school, setSchool] = useState<AsrSchool>(0);

  const [preview, setPreview] = useState<LocationPreview | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  // Auto-select the typical method for the chosen country.
  function handleCountryChange(c: Country | null) {
    setCountry(c);
    if (c) setMethod(METHOD_BY_COUNTRY[c.code] ?? 3);
  }

  const locationReady = Boolean(country) && city !== null;

  // Fetch the preview whenever the confirm step is entered.
  useEffect(() => {
    if (step !== 2 || !country || !city) return;
    let cancelled = false;
    setLoading(true);
    setError(null);
    setPreview(null);
    previewLocation({
      city: city.kind === "picked" ? city.match.name : city.name,
      country: country.name,
      countryCode: country.code,
      method,
      school,
      picked: city.kind === "picked" ? city.match : null,
    })
      .then((p) => {
        if (!cancelled) setPreview(p);
      })
      .catch((e) => {
        if (!cancelled) setError(String(e));
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [step, country, method, school, city]);

  async function finish() {
    if (!preview) return;
    setSaving(true);
    setError(null);
    try {
      await saveLocation(preview.location, method, school);
      onFinished?.();
    } catch (e) {
      setError(String(e));
      setSaving(false);
    }
  }

  return (
    <div className="flex min-h-full items-center justify-center p-6">
      <div className="w-full max-w-lg">
        <div className="mb-7 flex flex-col items-center text-center">
          <img src={logo} alt="" className="h-14 w-14 rounded-2xl" />
          <h1 className="mt-4 text-[22px] font-semibold tracking-[-0.015em] text-slate-50">
            {onCancel ? "Change location" : "Welcome to Prayer Times"}
          </h1>
          <p className="mt-1 text-sm text-slate-400">
            {onCancel
              ? "Pick the city to calculate prayer times for."
              : "Set your location to get accurate prayer times and alarms."}
          </p>
        </div>

        <ol className="mb-5 flex items-center justify-center gap-2 text-[13px]">
          {STEPS.map((label, i) => (
            <li key={label} className="flex items-center gap-2" aria-current={i === step ? "step" : undefined}>
              <span
                className={cn(
                  "flex h-6 w-6 items-center justify-center rounded-full border text-[11px] font-semibold",
                  i < step && "border-emerald-500 bg-emerald-600 text-white",
                  i === step && "border-emerald-500 text-emerald-400",
                  i > step && "border-slate-700 text-slate-500",
                )}
              >
                {i < step ? <Check className="h-3.5 w-3.5" /> : i + 1}
              </span>
              <span className={i === step ? "text-slate-200" : "text-slate-500"}>
                {label}
              </span>
              {i < STEPS.length - 1 && (
                <span className="mx-1 h-px w-8 bg-slate-700" aria-hidden />
              )}
            </li>
          ))}
        </ol>

        <Card>
          {step === 0 && (
            <div className="fade-in space-y-4">
              <div>
                <p id="setup-country-label" className="mb-1.5 text-[13px] font-medium text-slate-300">
                  Country
                </p>
                <CountryPicker
                  value={country}
                  onChange={handleCountryChange}
                  labelId="setup-country-label"
                />
              </div>
              <CityPicker
                countryCode={country?.code ?? null}
                countryName={country?.name ?? null}
                value={city}
                onChange={setCity}
              />
              <p className="text-[13px] leading-relaxed text-slate-500">
                Pick your city from the list so times are calculated for its
                exact position. If your town isn't listed, pick the nearest
                one.
              </p>
            </div>
          )}

          {step === 1 && (
            <div className="fade-in space-y-4">
              <SelectField
                label="Calculation method"
                value={method}
                onChange={(e) => setMethod(Number(e.target.value))}
              >
                {METHODS.map((m) => (
                  <option key={m.id} value={m.id}>
                    {m.name}
                  </option>
                ))}
              </SelectField>
              <SelectField
                label="Asr calculation"
                hint="Hanafi uses the later Asr time"
                value={school}
                onChange={(e) => setSchool(Number(e.target.value) === 1 ? 1 : 0)}
              >
                <option value={0}>Standard (Shafi, Maliki, Hanbali)</option>
                <option value={1}>Hanafi</option>
              </SelectField>
              <p className="text-[13px] leading-relaxed text-slate-500">
                Not sure? Keep the default — it was pre-selected for your
                country.
              </p>
            </div>
          )}

          {step === 2 && (
            <div className="fade-in space-y-4">
              {loading && (
                <p className="py-6 text-center text-sm text-slate-400">
                  Fetching today's times for{" "}
                  {city?.kind === "picked" ? cityLabel(city.match) : city?.name}…
                </p>
              )}
              {!loading && preview && (
                <PreviewResult preview={preview} />
              )}
              {!loading && !preview && error && (
                <div className="space-y-3">
                  <p className="rounded-lg border border-red-900/60 bg-red-950/40 px-3 py-2 text-sm text-red-300">
                    {friendlyError(error)}
                  </p>
                  <p className="text-[13px] text-slate-500">
                    Go back and check the city spelling, or try the nearest
                    larger city.
                  </p>
                </div>
              )}
            </div>
          )}

          <div className="mt-6 flex items-center justify-between">
            {step === 0 && onCancel ? (
              <Button variant="ghost" onClick={onCancel}>
                Cancel
              </Button>
            ) : (
              <Button
                variant="ghost"
                onClick={() => setStep((s) => Math.max(0, s - 1))}
                disabled={step === 0}
              >
                <ArrowLeft className="h-4 w-4" aria-hidden /> Back
              </Button>
            )}
            {step < 2 ? (
              <Button
                variant="primary"
                onClick={() => setStep((s) => s + 1)}
                disabled={!locationReady}
              >
                Continue <ArrowRight className="h-4 w-4" />
              </Button>
            ) : (
              <Button
                variant="primary"
                onClick={() => void finish()}
                disabled={!preview || saving}
              >
                <Check className="h-4 w-4" />
                {saving ? "Saving…" : "Finish setup"}
              </Button>
            )}
          </div>
        </Card>
      </div>
    </div>
  );
}

function PreviewResult({ preview }: { preview: LocationPreview }) {
  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center gap-x-2 gap-y-0.5 text-sm text-slate-200">
        <MapPin className="h-4 w-4 text-emerald-400" aria-hidden />
        {preview.location.region
          ? `${preview.location.city}, ${preview.location.region}, ${preview.location.country}`
          : `${preview.location.city}, ${preview.location.country}`}
        <span className="text-[13px] text-slate-500">{preview.location.timezone}</span>
      </div>
      <TimesTable day={preview.day} />
      <p className="text-[13px] text-slate-500">
        {fmtLongDate(preview.day.date)} · {preview.day.hijri}
      </p>
    </div>
  );
}

export function TimesTable({ day }: { day: DayTimings }) {
  return (
    <ul className="divide-y divide-slate-800/70 overflow-hidden rounded-lg border border-slate-800/80">
      {PRAYERS.flatMap((p) => {
        const t = day.timings.find((x) => x.prayer === p);
        if (!t) return [];
        const row = (
          <li key={p} className="flex items-center gap-3 px-3.5 py-2.5 text-sm">
            <PrayerIcon prayer={p} className="h-4 w-4 text-slate-500" />
            <span className="flex-1 text-slate-100">{PRAYER_LABELS[p]}</span>
            <span className="tnum text-slate-100">{fmtHM(t.start)}</span>
          </li>
        );
        if (p !== "fajr") return [row];
        return [
          row,
          <li key="sunrise" className="flex items-center gap-3 px-3.5 py-2 text-[13px] text-slate-500">
            <Sunrise className="h-4 w-4" strokeWidth={1.75} aria-hidden />
            <span className="flex-1">Sunrise</span>
            <span className="tnum">{fmtHM(day.sunrise)}</span>
          </li>,
        ];
      })}
    </ul>
  );
}

function friendlyError(raw: string): string {
  // Our own location checks already explain the problem and the fix.
  if (raw.startsWith("Couldn't find") || raw.includes(" isn't in ")) return raw;
  if (/400|404|not found|geocode/i.test(raw)) {
    return "Couldn't find prayer times for that city. Please check the spelling.";
  }
  if (/network|timeout|dns|connect/i.test(raw)) {
    return "Network error — please check your internet connection and try again.";
  }
  return sharedFriendlyError(raw);
}
