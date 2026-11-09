import { Download, X } from "lucide-react";
import { Button } from "./ui/Button";
import { IS_MAC, useUpdateStore } from "../store/useUpdateStore";

/**
 * Thin strip under the header announcing a pending update. Dismissible,
 * except while a download is already in progress.
 */
export function UpdateBanner() {
  const phase = useUpdateStore((s) => s.phase);
  const newVersion = useUpdateStore((s) => s.newVersion);
  const dismissed = useUpdateStore((s) => s.dismissed);
  const downloadedBytes = useUpdateStore((s) => s.downloadedBytes);
  const totalBytes = useUpdateStore((s) => s.totalBytes);
  const install = useUpdateStore((s) => s.install);
  const openDownloadPage = useUpdateStore((s) => s.openDownloadPage);
  const dismiss = useUpdateStore((s) => s.dismiss);

  if (phase !== "available" && phase !== "downloading") return null;
  if (phase === "available" && dismissed) return null;

  const pct =
    totalBytes && totalBytes > 0
      ? Math.min(100, Math.round((downloadedBytes / totalBytes) * 100))
      : null;

  return (
    <div className="flex items-center justify-between gap-3 border-b border-emerald-900/50 bg-emerald-950/40 px-5 py-2 text-sm">
      <p className="flex items-center gap-2 text-emerald-200">
        <Download className="h-4 w-4 shrink-0" aria-hidden />
        {phase === "downloading"
          ? `Updating… ${pct !== null ? `${pct}% — ` : ""}the app restarts when ready`
          : `Version ${newVersion} is available`}
      </p>
      <div className="flex items-center gap-1">
        {phase === "available" &&
          (IS_MAC ? (
            <Button size="sm" variant="primary" onClick={openDownloadPage}>
              Get update
            </Button>
          ) : (
            <Button size="sm" variant="primary" onClick={() => void install()}>
              Update now
            </Button>
          ))}
        {phase === "available" && (
          <Button size="sm" variant="ghost" onClick={dismiss} aria-label="Dismiss update notice">
            <X className="h-4 w-4" />
          </Button>
        )}
      </div>
    </div>
  );
}
