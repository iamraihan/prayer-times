import { useEffect, useState } from "react";
import { Building2, Loader2, Lock, RefreshCw, TriangleAlert } from "lucide-react";
import { Button } from "./ui/Button";
import { Section, SettingRow } from "./ui/Section";
import { TextField } from "./ui/fields";
import {
  followOrganization,
  leaveOrganization,
  previewOrganization,
  refreshOrganization,
} from "../lib/actions";
import { friendlyError } from "../lib/errors";
import { fmtHM, fmtInstant, type TimeFormat } from "../lib/time";
import {
  PRAYERS,
  PRAYER_LABELS,
  type OrganizationLink,
  type OrganizationPrayer,
  type OrganizationPreview,
  type OrganizationStatus,
} from "../lib/types";

type Busy = "preview" | "follow" | "refresh" | "leave" | null;

/**
 * "Masjid / Organization": follow a feed of jamaat times. The prayers it
 * lists get the Organization's time (locked); the others stay the user's.
 * The feed is fetched on Follow/Save and then once a day at 13:00.
 */
export function OrganizationSettings({
  link,
  status,
  hasLocation,
  mode,
}: {
  link: OrganizationLink | null;
  status: OrganizationStatus | null;
  hasLocation: boolean;
  mode: TimeFormat;
}) {
  const [url, setUrl] = useState(link?.feedUrl ?? "");
  const [key, setKey] = useState(link?.accessKey ?? "");
  const [preview, setPreview] = useState<OrganizationPreview | null>(null);
  const [busy, setBusy] = useState<Busy>(null);
  const [error, setError] = useState<string | null>(null);

  // Follow the backend when it changes the link (follow, leave, a restore).
  useEffect(() => {
    setUrl(link?.feedUrl ?? "");
    setKey(link?.accessKey ?? "");
  }, [link?.feedUrl, link?.accessKey]);

  const keyOrNull = key.trim() === "" ? null : key.trim();
  const edited = link !== null && (url.trim() !== link.feedUrl || keyOrNull !== link.accessKey);

  async function run(kind: Exclude<Busy, null>, action: () => Promise<void>) {
    setBusy(kind);
    setError(null);
    try {
      await action();
    } catch (e) {
      setError(friendlyError(e));
    } finally {
      setBusy(null);
    }
  }

  const doPreview = () =>
    run("preview", async () => setPreview(await previewOrganization(url, keyOrNull)));
  const doFollow = () =>
    run("follow", async () => {
      await followOrganization(url, keyOrNull);
      setPreview(null);
    });
  const doRefresh = () => run("refresh", refreshOrganization);
  const doLeave = () => run("leave", leaveOrganization);

  const fields = (
    <div className="grid gap-3 px-4 py-3 sm:grid-cols-[1fr_14rem]">
      <TextField
        label="API link"
        hint="from your masjid or organization"
        placeholder="https://example.org/prayer-times.json"
        value={url}
        inputMode="url"
        spellCheck={false}
        autoComplete="off"
        onChange={(e) => {
          setUrl(e.target.value);
          setPreview(null);
        }}
      />
      <TextField
        label="Access key"
        hint="optional"
        placeholder="Only if they gave you one"
        value={key}
        spellCheck={false}
        autoComplete="off"
        onChange={(e) => {
          setKey(e.target.value);
          setPreview(null);
        }}
      />
    </div>
  );

  const errorRow = error && (
    <p role="alert" className="flex items-start gap-2.5 px-4 py-3 text-[13px] leading-relaxed text-red-300">
      <TriangleAlert className="mt-0.5 h-4 w-4 shrink-0 text-red-400" aria-hidden />
      {error}
    </p>
  );

  return (
    <Section
      title="Masjid / Organization"
      description="Follow the jamaat times of your masjid or organization. The prayers it sets are locked to its times and update by themselves; the others stay yours."
    >
      {link && status ? (
        <>
          <SettingRow
            label={
              <span className="flex items-center gap-2">
                <Building2 className="h-4 w-4 shrink-0 text-emerald-400" aria-hidden />
                {status.name ?? status.host}
              </span>
            }
            hint={
              status.lastFetchedAt
                ? `${status.host} · checked ${fmtInstant(status.lastFetchedAt, mode)}, again daily at ${fmtHM("13:00", mode)}`
                : status.host
            }
          >
            <Button size="sm" onClick={() => void doRefresh()} disabled={busy !== null}>
              {busy === "refresh" ? (
                <Loader2 className="h-3.5 w-3.5 animate-spin" aria-hidden />
              ) : (
                <RefreshCw className="h-3.5 w-3.5" aria-hidden />
              )}
              Refresh now
            </Button>
            <Button size="sm" variant="ghost" onClick={() => void doLeave()} disabled={busy !== null}>
              Leave
            </Button>
          </SettingRow>
          <CoveredPrayers prayers={status.prayers} mode={mode} />
          {status.warning && (
            <p role="alert" className="flex items-start gap-2.5 px-4 py-3 text-[13px] leading-relaxed text-amber-200">
              <TriangleAlert className="mt-0.5 h-4 w-4 shrink-0 text-amber-400" aria-hidden />
              {status.warning}
            </p>
          )}
          {fields}
          {edited && (
            <div className="flex justify-end gap-2 px-4 py-3">
              <Button
                size="sm"
                variant="ghost"
                onClick={() => {
                  setUrl(link.feedUrl);
                  setKey(link.accessKey ?? "");
                }}
              >
                Cancel
              </Button>
              <Button size="sm" variant="primary" onClick={() => void doFollow()} disabled={busy !== null}>
                {busy === "follow" && <Loader2 className="h-3.5 w-3.5 animate-spin" aria-hidden />}
                Save
              </Button>
            </div>
          )}
          {errorRow}
        </>
      ) : (
        <>
          {fields}
          {preview ? (
            <div className="space-y-3 px-4 py-3">
              <p className="text-sm text-slate-100">
                Follow <span className="font-medium">{preview.name}</span>{" "}
                <span className="text-slate-500">({preview.host})</span>?
              </p>
              <CoveredPrayers prayers={preview.prayers} mode={mode} bare />
              <div className="flex justify-end gap-2">
                <Button size="sm" variant="ghost" onClick={() => setPreview(null)}>
                  Cancel
                </Button>
                <Button size="sm" variant="primary" onClick={() => void doFollow()} disabled={busy !== null}>
                  {busy === "follow" && <Loader2 className="h-3.5 w-3.5 animate-spin" aria-hidden />}
                  Follow
                </Button>
              </div>
            </div>
          ) : (
            <div className="flex items-center justify-between gap-4 px-4 py-3">
              <p className="text-[13px] text-slate-500">
                {hasLocation
                  ? "Paste the API link your masjid or organization shared, then check its times before following."
                  : "Set your location first."}
              </p>
              <Button
                size="sm"
                onClick={() => void doPreview()}
                disabled={busy !== null || !hasLocation || url.trim() === ""}
              >
                {busy === "preview" && <Loader2 className="h-3.5 w-3.5 animate-spin" aria-hidden />}
                Check link
              </Button>
            </div>
          )}
          {errorRow}
        </>
      )}
    </Section>
  );
}

/** Each prayer with the Organization's time, or "yours" when it doesn't set it. */
function CoveredPrayers({
  prayers,
  mode,
  bare,
}: {
  prayers: OrganizationPrayer[];
  mode: TimeFormat;
  bare?: boolean | undefined;
}) {
  return (
    <ul className={bare ? "grid grid-cols-5 gap-2" : "grid grid-cols-5 gap-2 px-4 py-3"} aria-label="Jamaat times">
      {PRAYERS.map((p) => {
        const covered = prayers.find((x) => x.prayer === p);
        return (
          <li key={p} className="rounded-lg border border-slate-800 px-2.5 py-2 text-center">
            <div className="text-[12px] text-slate-400">{PRAYER_LABELS[p]}</div>
            {covered ? (
              <div className="tnum mt-0.5 flex items-center justify-center gap-1 text-sm font-medium text-emerald-200">
                <Lock className="h-3 w-3" aria-label="set by the organization" />
                {fmtHM(covered.time, mode)}
              </div>
            ) : (
              <div className="mt-0.5 text-sm text-slate-500">yours</div>
            )}
          </li>
        );
      })}
    </ul>
  );
}
