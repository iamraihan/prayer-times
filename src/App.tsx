import { useEffect, useState, type ReactNode } from "react";
import { RefreshCw, Settings } from "lucide-react";
import logo from "./assets/logo.png";
import { TodayPage } from "./pages/TodayPage";
import { SettingsPage } from "./pages/SettingsPage";
import { SetupWizard } from "./components/SetupWizard";
import { UpdateBanner } from "./components/UpdateBanner";
import { startClock, useAppStore } from "./store/useAppStore";
import { useUpdateStore } from "./store/useUpdateStore";
import { cn } from "./lib/cn";

type Tab = "today" | "settings";

export default function App() {
  const ready = useAppStore((s) => s.ready);
  const initError = useAppStore((s) => s.initError);
  const retry = useAppStore((s) => s.retry);
  const hasLocation = useAppStore((s) => s.state?.hasLocation ?? false);
  // First run: after the wizard completes, land on Settings so alarm tone /
  // reminders can be configured before anything else.
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
          <p className="text-sm text-red-300">{initError}</p>
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

  return <MainLayout initialTab={justSetup ? "settings" : "today"} />;
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

function MainLayout({ initialTab = "today" }: { initialTab?: Tab }) {
  const [tab, setTab] = useState<Tab>(initialTab);
  const location = useAppStore((s) => s.state?.settings.location ?? null);

  return (
    <div className="flex h-full flex-col">
      <header className="flex items-center justify-between border-b border-slate-800 bg-slate-900/40 px-5 py-3">
        <div className="flex items-center gap-3">
          <img src={logo} alt="Prayer Times" className="h-9 w-9 rounded-xl" />
          <div>
            <h1 className="text-sm font-semibold leading-tight">Prayer Times</h1>
            <p className="text-xs text-slate-500">
              {location ? `${location.city}, ${location.country}` : ""}
            </p>
          </div>
        </div>
        <nav className="flex items-center gap-1 rounded-xl bg-slate-900/70 p-1">
          <TabButton active={tab === "today"} onClick={() => setTab("today")}>
            Today
          </TabButton>
          <TabButton active={tab === "settings"} onClick={() => setTab("settings")}>
            <Settings className="h-3.5 w-3.5" /> Settings
          </TabButton>
        </nav>
      </header>

      <UpdateBanner />

      <main className="flex-1 overflow-y-auto px-5 py-5">
        <div className="mx-auto max-w-2xl">
          {/* Settings stays mounted so an unsaved draft survives tab switches. */}
          {tab === "today" && <TodayPage />}
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
      onClick={onClick}
      className={cn(
        "flex items-center gap-1.5 rounded-lg px-3 py-1.5 text-sm font-medium transition-colors",
        active ? "bg-slate-800 text-slate-100" : "text-slate-400 hover:text-slate-200",
      )}
    >
      {children}
    </button>
  );
}
