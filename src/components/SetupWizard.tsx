import { useEffect, useState } from "react";
import { ArrowLeft, ArrowRight, Check, MapPin } from "lucide-react";
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
import { PRAYERS, PRAYER_EMOJIS, PRAYER_LABELS } from "../lib/types";
import { fmtHM } from "../lib/time";
import { cn } from "../lib/cn";

export interface SetupWizardProps {
  /** Called after the location was saved successfully. */
  onFinished?: () => void;
}

const STEPS = ["Location", "Calculation", "Confirm"] as const;

/**
 * First-run wizard: country + city, calculation method, then a live preview
 * fetched from the Aladhan API before anything is persisted.
 */
export function SetupWizard({ onFinished }: SetupWizardProps) {
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
    <div className="flex h-full items-center justify-center p-6">
      <div className="w-full max-w-xl">
        <div className="mb-6 flex flex-col items-center gap-2 text-center">
          <img src={logo} alt="Prayer Times" className="h-16 w-16 rounded-2xl" />
          <h1 className="text-2xl font-semibold">Welcome to Prayer Times</h1>
          <p className="text-sm text-slate-400">
            Set your location to get accurate prayer times and alarms.
          </p>
        </div>

        <ol className="mb-4 flex items-center justify-center gap-2 text-xs">
          {STEPS.map((label, i) => (
            <li key={label} className="flex items-center gap-2">
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
                <label className="mb-1.5 block text-xs font-medium text-slate-300">
                  Country
                </label>
                <CountryPicker value={country} onChange={handleCountryChange} />
              </div>
              <CityPicker
                countryCode={country?.code ?? null}
                countryName={country?.name ?? null}
                value={city}
                onChange={setCity}
              />
              <p className="text-xs leading-relaxed text-slate-500">
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
              <p className="text-xs leading-relaxed text-slate-500">
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
                  <p className="text-xs text-slate-500">
                    Go back and check the city spelling, or try the nearest
                    larger city.
                  </p>
                </div>
              )}
            </div>
          )}

          <div className="mt-6 flex items-center justify-between">
            <Button
              variant="ghost"
              onClick={() => setStep((s) => Math.max(0, s - 1))}
              disabled={step === 0}
            >
              <ArrowLeft className="h-4 w-4" /> Back
            </Button>
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
      <div className="flex items-center gap-2 text-sm text-slate-300">
        <MapPin className="h-4 w-4 text-emerald-400" />
        {preview.location.region
          ? `${preview.location.city}, ${preview.location.region}, ${preview.location.country}`
          : `${preview.location.city}, ${preview.location.country}`}
        <span className="text-xs text-slate-500">
          ({preview.location.timezone})
        </span>
      </div>
      <TimesTable day={preview.day} />
      <p className="text-xs text-slate-500">
        {preview.day.gregorian} · {preview.day.hijri}
      </p>
    </div>
  );
}

export function TimesTable({ day }: { day: DayTimings }) {
  return (
    <table className="w-full text-sm">
      <tbody>
        <tr className="text-slate-400">
          <td className="py-1.5">Sunrise</td>
          <td className="py-1.5 text-right font-mono">{fmtHM(day.sunrise)}</td>
        </tr>
        {PRAYERS.map((p) => {
          const t = day.timings.find((x) => x.prayer === p);
          if (!t) return null;
          return (
            <tr key={p} className="text-slate-100">
              <td className="py-1.5">
                <span className="mr-2">{PRAYER_EMOJIS[p]}</span>
                {PRAYER_LABELS[p]}
              </td>
              <td className="py-1.5 text-right font-mono">{fmtHM(t.start)}</td>
            </tr>
          );
        })}
      </tbody>
    </table>
  );
}

function friendlyError(raw: string): string {
  if (/404|not found|city/i.test(raw)) {
    return "Couldn't find prayer times for that city. Please check the spelling.";
  }
  if (/network|timeout|dns|connect/i.test(raw)) {
    return "Network error — please check your internet connection and try again.";
  }
  return raw;
}
