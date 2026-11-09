import { useEffect, useState, type ReactNode } from "react";
import { CalendarDays, Clock, RefreshCw, Settings } from "lucide-react";
import logo from "./assets/logo.png";
import { TodayPage } from "./pages/TodayPage";
import { SettingsPage } from "./pages/SettingsPage";
import { HistoryPage } from "./pages/HistoryPage";
import { SetupWizard } from "./components/SetupWizard";
import { UpdateBanner } from "./components/UpdateBanner";
import { startClock, useAppStore } from "./store/useAppStore";
import { useUpdateStore } from "./store/useUpdateStore";
import { cn } from "./lib/cn";
import { Button } from "./components/ui/Button";

type Tab = "today" | "history" | "settings";

export default function App() {
  const ready = useAppStore((s) => s.ready);
  const initError = useAppStore((s) => s.initError);
  const retry = useAppStore((s) => s.retry);
  const hasLocation = useAppStore((s) => s.state?.hasLocation ?? false);
  // First run: after the wizard completes, land on Today with a one-time
  // card saying what will ring and where to change it.
  const [justSetup, setJustSetup] = useState(false);

  useEffect(() => {
    startClock();
    void useAppStore.getState().init();
    void useUpdateStore.getState().init();
  }, []);

  if (!ready) {
    return <Splash />;
  }

  if (initError) {
    return (
      <div className="flex h-full items-center justify-center p-6">
        <div className="max-w-sm space-y-4 text-center">
          <p className="text-sm text-red-300">Couldn't start: {initError}</p>
          <button
            onClick={retry}
            className="mx-auto flex items-center gap-2 rounded-lg border border-slate-700 px-4 py-2 text-sm text-slate-200 hover:bg-slate-800"
          >
            <RefreshCw className="h-4 w-4" /> Retry
          </button>
        </div>
      </div>
    );
  }

  if (!hasLocation) {
    return <SetupWizard onFinished={() => setJustSetup(true)} />;
  }

  return <MainLayout welcome={justSetup} />;
}

function Splash() {
  return (
    <div className="flex h-full items-center justify-center">
      <div className="flex flex-col items-center gap-3 text-slate-500">
        <img src={logo} alt="" className="h-14 w-14 animate-pulse rounded-2xl" />
        <p className="text-sm">Loading…</p>
      </div>
    </div>
  );
}

function MainLayout({ welcome = false }: { welcome?: boolean }) {
  const [tab, setTab] = useState<Tab>("today");
  const [showWelcome, setShowWelcome] = useState(welcome);
  const location = useAppStore((s) => s.state?.settings.location ?? null);

  return (
    <div className="flex h-full flex-col">
      <header className="flex h-14 shrink-0 items-center justify-between border-b border-slate-800/80 px-5">
        <div className="flex min-w-0 items-center gap-3">
          <img src={logo} alt="" className="h-8 w-8 rounded-lg" />
          <div className="min-w-0 leading-tight">
            <p className="text-sm font-semibold text-slate-50">Prayer Times</p>
            {location && (
              <p className="truncate text-[12px] text-slate-400">
                {location.city}, {location.country}
              </p>
            )}
          </div>
        </div>
        <nav aria-label="Main" className="flex items-center gap-0.5 rounded-lg bg-slate-900 p-0.5 ring-1 ring-slate-800">
          <TabButton active={tab === "today"} onClick={() => setTab("today")}>
            <Clock className="h-3.5 w-3.5" aria-hidden /> Today
          </TabButton>
          <TabButton active={tab === "history"} onClick={() => setTab("history")}>
            <CalendarDays className="h-3.5 w-3.5" aria-hidden /> History
          </TabButton>
          <TabButton active={tab === "settings"} onClick={() => setTab("settings")}>
            <Settings className="h-3.5 w-3.5" aria-hidden /> Settings
          </TabButton>
        </nav>
      </header>

      <UpdateBanner />

      <main className="flex-1 overflow-y-auto px-5 pt-7 pb-5">
        <div className="mx-auto max-w-2xl">
          {/* Settings stays mounted so an unsaved draft survives tab switches. */}
          {tab === "today" && showWelcome && (
            <WelcomeCard
              onReview={() => {
                setShowWelcome(false);
                setTab("settings");
              }}
              onDismiss={() => setShowWelcome(false)}
            />
          )}
          {tab === "today" && <TodayPage />}
          {tab === "history" && <HistoryPage />}
          <div hidden={tab !== "settings"}>
            <SettingsPage />
          </div>
        </div>
      </main>
    </div>
  );
}

/** Shown once, right after setup: what will ring, in the user's own settings,
 * and where to change it. */
function WelcomeCard({ onReview, onDismiss }: { onReview: () => void; onDismiss: () => void }) {
  const settings = useAppStore((s) => s.state?.settings ?? null);
  if (!settings) return null;
  const parts: string[] = [];
  if (settings.getReadyEnabled) parts.push(`${settings.getReadyMinutes} minutes before each prayer`);
  if (settings.startAlarmEnabled) parts.push("when each prayer starts");
  if (settings.preEndWarningEnabled) {
    parts.push(`${settings.preEndWarningMinutes} minutes before its time ends, if you haven't marked it done`);
  }
  const when =
    parts.length === 0
      ? "All alarms are off right now."
      : `Alarms will ring ${parts.length > 1 ? `${parts.slice(0, -1).join(", ")} and ${parts[parts.length - 1] ?? ""}` : (parts[0] ?? "")}.`;
  return (
    <section
      aria-label="You're set up"
      className="fade-in mb-6 rounded-xl border border-emerald-400/25 bg-emerald-400/[0.04] p-5"
    >
      <h2 className="font-semibold text-slate-50">You're set up for {settings.location?.city ?? "your city"}</h2>
      <p className="mt-1 text-sm leading-relaxed text-slate-300">
        {when} The app keeps running in the tray when you close this window.
      </p>
      <div className="mt-4 flex flex-wrap gap-2">
        <Button size="sm" variant="primary" onClick={onDismiss}>
          Got it
        </Button>
        <Button size="sm" variant="ghost" onClick={onReview}>
          Change alarms
        </Button>
      </div>
    </section>
  );
}

function TabButton({
  active,
  onClick,
  children,
}: {
  active: boolean;
  onClick: () => void;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      aria-current={active ? "page" : undefined}
      onClick={onClick}
      className={cn(
        "flex h-7 items-center gap-1.5 rounded-md px-3 text-[13px] font-medium transition-colors",
        active ? "bg-slate-700/70 text-slate-50" : "text-slate-400 hover:text-slate-100",
      )}
    >
      {children}
    </button>
  );
}
