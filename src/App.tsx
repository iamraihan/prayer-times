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
import { useTour } from "./lib/useTour";

type Tab = "today" | "history" | "settings";

export default function App() {
  const ready = useAppStore((s) => s.ready);
  const initError = useAppStore((s) => s.initError);
  const retry = useAppStore((s) => s.retry);
  const hasLocation = useAppStore((s) => s.state?.hasLocation ?? false);

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
    // After setup the Today Tour explains what will ring (see useTour).
    return <SetupWizard />;
  }

  return <MainLayout />;
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

function MainLayout() {
  const [tab, setTab] = useState<Tab>("today");
  useTour(tab);
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
