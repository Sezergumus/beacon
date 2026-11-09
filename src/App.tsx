import React, { useEffect, useState } from "react";
import {
  Activity,
  Boxes,
  Check,
  CircleAlert,
  CircleCheck,
  Copy,
  Globe,
  Home,
  Info,
  Loader2,
  Play,
  Settings,
  Square,
  Users,
  X,
} from "lucide-react";
import { useServerStore } from "./stores/useServerStore";
import { useNetworkStore } from "./stores/useNetworkStore";
import { DashboardView } from "./components/dashboard/DashboardView";
import { PlayersView } from "./components/players/PlayersView";
import { ModsView } from "./components/mods/ModsView";
import { WorldsView } from "./components/worlds/WorldsView";
import { MonitorView } from "./components/monitor/MonitorView";
import { SettingsView } from "./components/settings/SettingsView";
import { Button, StatusDot } from "./components/ui";
import { useStartWorld } from "./lib/useStartWorld";
import { BeaconArt } from "./components/BeaconArt";

export type ActiveTab =
  | "dashboard"
  | "players"
  | "mods"
  | "monitor"
  | "worlds"
  | "settings";

export const App: React.FC = () => {
  const [activeTab, setActiveTab] = useState<ActiveTab>("dashboard");
  const [copied, setCopied] = useState(false);

  const {
    status,
    telemetry,
    toasts,
    dismissToast,
    stopServer,
    initializeServerStore,
  } = useServerStore();
  const { network, initializeNetworkStore } = useNetworkStore();
  const startWorld = useStartWorld();

  useEffect(() => {
    initializeServerStore();
    initializeNetworkStore();
  }, []);

  const playerCount = telemetry?.online_players.length ?? 0;
  const isOnline = status === "running";
  const isStopped = status === "stopped";
  const isBusy =
    status === "provisioning" || status === "starting" || status === "stopping";

  const copyCode = () => {
    if (!network.room_code) return;
    navigator.clipboard.writeText(network.room_code);
    setCopied(true);
    setTimeout(() => setCopied(false), 1800);
  };

  const navItems: {
    id: ActiveTab;
    label: string;
    icon: React.ReactNode;
    count?: number;
  }[] = [
    { id: "dashboard", label: "Home", icon: <Home className="w-5 h-5" /> },
    {
      id: "players",
      label: "Players",
      icon: <Users className="w-5 h-5" />,
      count: playerCount || undefined,
    },
    { id: "mods", label: "Mods", icon: <Boxes className="w-5 h-5" /> },
    { id: "monitor", label: "Monitor", icon: <Activity className="w-5 h-5" /> },
    { id: "worlds", label: "Worlds", icon: <Globe className="w-5 h-5" /> },
    { id: "settings", label: "Settings", icon: <Settings className="w-5 h-5" /> },
  ];

  return (
    <div className="h-screen w-screen bg-bg text-ink flex overflow-hidden select-none">
      <aside className="w-60 shrink-0 bg-surface/60 flex flex-col p-4">
        <div className="flex items-center gap-3 px-2 py-2 mb-6">
          <BeaconArt variant="sidebar" />
          <span className="text-xl font-extrabold tracking-tight">Beacon</span>
        </div>

        {/* Start / stop the selected world from anywhere. Stop stays disabled
            until the server is fully up. */}
        <div className="mb-4">
          {isStopped ? (
            <Button
              variant="primary"
              onClick={startWorld}
              className="w-full"
              icon={<Play className="w-5 h-5 fill-current" />}
            >
              Start server
            </Button>
          ) : (
            <Button
              variant="secondary"
              onClick={stopServer}
              disabled={!isOnline}
              className="w-full"
              icon={
                isBusy ? (
                  <Loader2 className="w-5 h-5 animate-spin" />
                ) : (
                  <Square className="w-4 h-4 fill-current" />
                )
              }
            >
              {status === "stopping" ? "Saving…" : isBusy ? "Starting…" : "Stop server"}
            </Button>
          )}
        </div>

        <nav className="flex flex-col gap-1">
          {navItems.map((item) => {
            const active = activeTab === item.id;
            return (
              <button
                key={item.id}
                onClick={() => setActiveTab(item.id)}
                aria-current={active ? "page" : undefined}
                className={`h-11 px-3 rounded-xl flex items-center gap-3 font-bold transition-colors cursor-pointer ${
                  active
                    ? "bg-raised text-ink"
                    : "text-soft hover:text-ink hover:bg-raised/60"
                }`}
              >
                <span className={active ? "text-brand" : ""}>{item.icon}</span>
                <span className="flex-1 text-left">{item.label}</span>
                {item.count !== undefined && (
                  <span className="min-w-6 h-6 px-1.5 rounded-full bg-brand text-brand-ink text-xs font-extrabold flex items-center justify-center">
                    {item.count}
                  </span>
                )}
              </button>
            );
          })}
        </nav>

        {/* Quiet status box */}
        <div className="mt-auto rounded-2xl bg-raised/70 p-4">
          <div className="flex items-center gap-2 font-bold">
            <StatusDot tone={isOnline ? "on" : isBusy ? "busy" : "off"} />
            {isOnline ? "World is open" : isBusy ? "Getting ready…" : "World is closed"}
          </div>
          {isOnline && (
            <div className="text-sm text-soft mt-1">
              {playerCount === 0
                ? "Nobody here yet"
                : `${playerCount} ${playerCount === 1 ? "player" : "players"} online`}
            </div>
          )}
          {network.room_code && (
            <button
              onClick={copyCode}
              title="Copy code"
              className="mt-3 w-full flex items-center justify-between gap-2 rounded-xl bg-surface px-3 py-2 text-left hover:bg-line transition-colors cursor-pointer"
            >
              <span className="font-mono text-sm truncate">{network.room_code}</span>
              {copied ? (
                <Check className="w-4 h-4 text-brand shrink-0" />
              ) : (
                <Copy className="w-4 h-4 text-soft shrink-0" />
              )}
            </button>
          )}
        </div>
      </aside>

      <main className="flex-1 h-full overflow-hidden">
        {activeTab === "dashboard" && <DashboardView onNavigate={setActiveTab} />}
        {activeTab === "players" && <PlayersView />}
        {activeTab === "mods" && <ModsView />}
        {activeTab === "monitor" && <MonitorView />}
        {activeTab === "worlds" && <WorldsView />}
        {activeTab === "settings" && <SettingsView onNavigate={setActiveTab} />}
      </main>

      {/* Toasts */}
      <div className="fixed bottom-5 right-5 z-50 flex flex-col gap-2 w-[360px] pointer-events-none">
        {toasts.map((t) => (
          <div
            key={t.id}
            role="status"
            className="animate-toast-in pointer-events-auto rounded-2xl bg-raised shadow-2xl shadow-black/40 p-4 flex items-start gap-3"
          >
            {t.type === "error" ? (
              <CircleAlert className="w-5 h-5 mt-0.5 shrink-0 text-danger" aria-hidden />
            ) : t.type === "success" ? (
              <CircleCheck className="w-5 h-5 mt-0.5 shrink-0 text-brand" aria-hidden />
            ) : (
              <Info className="w-5 h-5 mt-0.5 shrink-0 text-info" aria-hidden />
            )}
            <div className="flex-1 min-w-0">
              <div className={`font-bold ${t.type === "error" ? "text-danger" : ""}`}>
                {t.title}
              </div>
              <div className="text-sm text-soft mt-0.5 break-words">{t.message}</div>
            </div>
            <button
              onClick={() => dismissToast(t.id)}
              aria-label="Dismiss"
              className="p-1 rounded-lg text-faint hover:text-ink hover:bg-line cursor-pointer"
            >
              <X className="w-4 h-4" />
            </button>
          </div>
        ))}
      </div>
    </div>
  );
};

export default App;
