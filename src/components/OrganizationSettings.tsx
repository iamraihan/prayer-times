import { useEffect, useState, type FormEvent } from "react";
import { Building2, Loader2, Lock, RefreshCw, TriangleAlert } from "lucide-react";
import { Button } from "./ui/Button";
import { Section, SettingRow } from "./ui/Section";
import { Toast, type ToastData } from "./ui/Toast";
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

/** The daily refresh time (`organization::DAILY_REFRESH_AT`). */
const DAILY_CHECK = "13:00";

/**
 * "Masjid / Organization": follow a feed of jamaat times. The prayers it
 * lists get the Organization's time (locked); the others stay the user's.
 * The feed is fetched on Follow/Save and then once a day at 13:00.
 */
export function OrganizationSettings({
  link,
  status,
  hasLocation,
  today,
  mode,
}: {
  link: OrganizationLink | null;
  status: OrganizationStatus | null;
  hasLocation: boolean;
  /** Today's date in Location Time ("YYYY-MM-DD"), to date the last check. */
  today: string | undefined;
  mode: TimeFormat;
}) {
  const [url, setUrl] = useState(link?.feedUrl ?? "");
  const [key, setKey] = useState(link?.accessKey ?? "");
  const [preview, setPreview] = useState<OrganizationPreview | null>(null);
  const [editing, setEditing] = useState(false);
  const [busy, setBusy] = useState<Busy>(null);
  const [error, setError] = useState<string | null>(null);
  const [toast, setToast] = useState<ToastData | null>(null);

  // Follow the backend when it changes the link (follow, leave, a restore).
  useEffect(() => {
    setUrl(link?.feedUrl ?? "");
    setKey(link?.accessKey ?? "");
    setEditing(false);
  }, [link?.feedUrl, link?.accessKey]);

  const keyOrNull = key.trim() === "" ? null : key.trim();

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

  function checkLink(e?: FormEvent) {
    e?.preventDefault();
    if (busy || !hasLocation || url.trim() === "") return;
    void run("preview", async () => setPreview(await previewOrganization(url, keyOrNull)));
  }

  function follow(name: string | null, e?: FormEvent) {
    e?.preventDefault();
    if (busy) return;
    void run("follow", async () => {
      await followOrganization(url, keyOrNull);
      setPreview(null);
      setEditing(false);
      setToast({
        kind: "success",
        message: link ? "Link saved and times updated." : `Following ${name ?? "the organization"}.`,
      });
    });
  }

  function leave() {
    const name = status?.name ?? "the organization";
    void run("leave", async () => {
      await leaveOrganization();
      setToast({ kind: "success", message: `Stopped following ${name}. Your own times are back.` });
    });
  }

  function cancelEdit() {
    setUrl(link?.feedUrl ?? "");
    setKey(link?.accessKey ?? "");
    setEditing(false);
    setError(null);
  }

  const spinner = <Loader2 className="h-3.5 w-3.5 animate-spin" aria-hidden />;

  const fields = (
    <div className="grid gap-3 sm:grid-cols-[1fr_14rem]">
      <TextField
        label="API link"
        hint="from your masjid or organization"
        placeholder="https://example.org/prayer-times.json"
        value={url}
        type="url"
        inputMode="url"
        spellCheck={false}
        autoComplete="off"
        autoCapitalize="off"
        aria-invalid={error !== null ? true : undefined}
        onChange={(e) => {
          setUrl(e.target.value);
          setPreview(null);
          setError(null);
        }}
      />
      <TextField
        label="Access key"
        hint="optional"
        placeholder="Only if they gave you one"
        value={key}
        spellCheck={false}
        autoComplete="off"
        autoCapitalize="off"
        onChange={(e) => {
          setKey(e.target.value);
          setPreview(null);
          setError(null);
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

  let body;
  if (link && status) {
    // Following.
    const name = status.name ?? status.host;
    const edited = url.trim() !== link.feedUrl || keyOrNull !== link.accessKey;
    body = (
      <>
        <SettingRow
          label={
            <span className="flex items-center gap-2">
              <Building2 className="h-4 w-4 shrink-0 text-emerald-400" aria-hidden />
              {name}
            </span>
          }
          hint={
            status.lastFetchedAt
              ? `${status.host} · updated ${whenLabel(status.lastFetchedAt, today, mode)}`
              : status.host
          }
        >
          <Button size="sm" onClick={() => void run("refresh", refreshOrganization)} disabled={busy !== null}>
            {busy === "refresh" ? spinner : <RefreshCw className="h-3.5 w-3.5" aria-hidden />}
            Refresh now
          </Button>
          <Button size="sm" variant="ghost" onClick={leave} disabled={busy !== null}>
            {busy === "leave" && spinner}
            Leave
          </Button>
        </SettingRow>
        <div className="space-y-2.5 px-4 py-3.5">
          <JamaatTimes prayers={status.prayers} mode={mode} />
          <p className="text-[12px] leading-relaxed text-slate-500">
            Checks for new times every day at {fmtHM(DAILY_CHECK, mode)}.{" "}
            {!editing && (
              <button
                type="button"
                className="rounded text-slate-400 underline decoration-slate-600 underline-offset-2 hover:text-slate-200 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-emerald-500/40"
                onClick={() => setEditing(true)}
              >
                Change link or access key
              </button>
            )}
          </p>
        </div>
        {status.warning && (
          <p role="status" className="flex items-start gap-2.5 px-4 py-3 text-[13px] leading-relaxed text-amber-200">
            <TriangleAlert className="mt-0.5 h-4 w-4 shrink-0 text-amber-400" aria-hidden />
            {status.warning}
          </p>
        )}
        {editing && (
          <form className="space-y-3 px-4 py-4" onSubmit={(e) => follow(status.name, e)}>
            {fields}
            <div className="flex gap-2">
              <Button type="submit" size="sm" variant="primary" disabled={busy !== null || !edited || url.trim() === ""}>
                {busy === "follow" && spinner}
                Save
              </Button>
              <Button size="sm" variant="ghost" onClick={cancelEdit} disabled={busy !== null}>
                Cancel
              </Button>
            </div>
          </form>
        )}
        {errorRow}
      </>
    );
  } else if (preview) {
    // Checked, waiting for confirmation.
    const covered = preview.prayers.map((p) => PRAYER_LABELS[p.prayer]);
    const yours = PRAYERS.filter((p) => !preview.prayers.some((x) => x.prayer === p)).map(
      (p) => PRAYER_LABELS[p],
    );
    body = (
      <>
        <div className="space-y-3.5 px-4 py-4">
          <div>
            <p className="flex items-center gap-2 text-sm font-medium text-slate-100">
              <Building2 className="h-4 w-4 shrink-0 text-emerald-400" aria-hidden />
              {preview.name}
            </p>
            <p className="mt-0.5 text-[13px] text-slate-500">
              {preview.host} · {preview.timezone}
            </p>
          </div>
          <JamaatTimes prayers={preview.prayers} mode={mode} />
          <p className="text-[13px] leading-relaxed text-slate-400">
            {listOf(covered)} will follow its times and can't be changed here.
            {yours.length > 0 && ` ${listOf(yours)} stay${yours.length === 1 ? "s" : ""} yours.`}
          </p>
          <div className="flex gap-2">
            <Button size="sm" variant="primary" onClick={() => follow(preview.name)} disabled={busy !== null}>
              {busy === "follow" && spinner}
              Follow
            </Button>
            <Button size="sm" variant="ghost" onClick={() => setPreview(null)} disabled={busy !== null}>
              Cancel
            </Button>
          </div>
        </div>
        {errorRow}
      </>
    );
  } else {
    // Not following.
    body = (
      <>
        <form className="space-y-3 px-4 py-4" onSubmit={checkLink}>
          {fields}
          <div className="flex flex-wrap items-center justify-between gap-x-4 gap-y-2">
            <p className="text-[13px] text-slate-500">
              {hasLocation
                ? "Paste the link they shared. You'll see their times before following."
                : "Set your location first."}
            </p>
            <Button type="submit" size="sm" disabled={busy !== null || !hasLocation || url.trim() === ""}>
              {busy === "preview" && spinner}
              Check link
            </Button>
          </div>
        </form>
        {errorRow}
      </>
    );
  }

  return (
    <>
      <Section
        title="Masjid / Organization"
        description="Follow the jamaat times of your masjid or organization. The prayers it sets are locked to its times and update by themselves; the others stay yours."
      >
        {body}
      </Section>
      {toast && <Toast toast={toast} onDismiss={() => setToast(null)} />}
    </>
  );
}

/** All five prayers in one row: the Organization's time, or "Yours". */
function JamaatTimes({ prayers, mode }: { prayers: OrganizationPrayer[]; mode: TimeFormat }) {
  return (
    <dl className="grid grid-cols-5 gap-x-3" aria-label="Jamaat times">
      {PRAYERS.map((p) => {
        const covered = prayers.find((x) => x.prayer === p);
        return (
          <div key={p} className="min-w-0">
            <dt className="text-[12px] text-slate-500">{PRAYER_LABELS[p]}</dt>
            <dd
              className={
                covered
                  ? "tnum mt-0.5 flex items-center gap-1 text-sm font-medium text-emerald-200"
                  : "mt-0.5 text-sm text-slate-500"
              }
            >
              {covered ? (
                <>
                  <Lock className="h-3 w-3 shrink-0 text-emerald-400/80" aria-hidden />
                  {fmtHM(covered.time, mode)}
                </>
              ) : (
                "Yours"
              )}
            </dd>
          </div>
        );
      })}
    </dl>
  );
}

/** "today 1:00 PM" / "yesterday 1:00 PM" / "Nov 24, 1:00 PM", from an
 * RFC3339 string carrying the Location's offset. */
function whenLabel(iso: string, today: string | undefined, mode: TimeFormat): string {
  const day = iso.slice(0, 10);
  const time = fmtInstant(iso, mode);
  if (!today) return time;
  if (day === today) return `today ${time}`;
  const t = Date.parse(`${today}T00:00:00Z`);
  if (Date.parse(`${day}T00:00:00Z`) === t - 86_400_000) return `yesterday ${time}`;
  const date = new Intl.DateTimeFormat("en-US", { month: "short", day: "numeric", timeZone: "UTC" }).format(
    new Date(`${day}T00:00:00Z`),
  );
  return `${date}, ${time}`;
}

/** "Fajr", "Fajr and Asr", "Fajr, Dhuhr and Asr". */
function listOf(items: string[]): string {
  if (items.length <= 1) return items.join("");
  return `${items.slice(0, -1).join(", ")} and ${items[items.length - 1]}`;
}
