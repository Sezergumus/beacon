import React, { useState } from "react";
import {
  Activity,
  Check,
  CloudSun,
  Copy,
  Crown,
  Heart,
  MessageSquare,
  Package,
  Play,
  Save,
  Square,
  Sun,
  Users,
} from "lucide-react";
import { useServerStore } from "../../stores/useServerStore";
import { useNetworkStore } from "../../stores/useNetworkStore";
import { ServerCore } from "../../lib/tauri";
import { useStartWorld } from "../../lib/useStartWorld";
import { NewWorldForm } from "../worlds/NewWorldForm";
import { BeaconArt } from "../BeaconArt";
import type { ActiveTab } from "../../App";
import {
  Button,
  Card,
  Disclosure,
  EmptyState,
  PlayerHead,
  ProgressBar,
  Segmented,
  TextInput,
} from "../ui";

interface DashboardViewProps {
  onNavigate?: (tab: ActiveTab) => void;
}

const VERSION_NOTES: Record<string, string> = {
  "1.21.1": "most popular",
  "1.20.1": "best for mods",
  "1.16.5": "classic modpacks",
  "1.12.2": "classic modpacks",
  "1.8.8": "PvP classic",
};

/** `1.21.4` and `1.21` belong to the group "1.21"; year-based `26.1.2` to "26.1". */
const versionGroup = (v: string) => v.split(".").slice(0, 2).join(".");


export const DashboardView: React.FC<DashboardViewProps> = ({ onNavigate }) => {
  const {
    status,
    selectedCore,
    selectedVersion,
    customRamMb,
    telemetry,
    jvmProgress,
    setSelectedCore,
    setSelectedVersion,
    stopServer,
    sendCommand,
    executePlayerAction,
    openPlayerInventory,
    worlds,
    selectedWorldId,
    versions,
  } = useServerStore();
  const versionList = versions[selectedCore];
  const newestVersion = versionList?.[0];
  const versionGroups = React.useMemo(() => {
    const groups = new Map<string, string[]>();
    for (const v of versionList ?? []) {
      const g = versionGroup(v);
      groups.set(g, [...(groups.get(g) ?? []), v]);
    }
    return [...groups.entries()];
  }, [versionList]);
  const worldName = worlds?.find((w) => w.kind === "world" && w.id === selectedWorldId)?.name;

  const { isBusy: isNetBusy, startHostingRoom, joinFriendRoom, leaveSession } = useNetworkStore();

  const isRunning = status === "running";
  const isStopped = status === "stopped";
  const isTransitioning = !isRunning && !isStopped;

  const tps = telemetry?.tps ?? 20;
  const ramUsed = telemetry?.ram_used_mb ?? 0;
  const ramMax = telemetry?.ram_allocated_mb ?? customRamMb;
  const players = telemetry?.online_players ?? [];

  const handleStart = useStartWorld();

  const title = isRunning
    ? "Your world is open"
    : status === "stopping"
    ? "Saving your world…"
    : isTransitioning
    ? "Getting your world ready…"
    : "Ready when you are";

  const subtitle = isRunning
    ? `${worldName ? `“${worldName}” · ` : ""}Friends can join with your code below.`
    : isTransitioning
    ? "This can take a minute the first time."
    : "Pick a version and press Start. Beacon handles the rest.";

  return (
    <div className="h-full overflow-y-auto">
      <div className="max-w-4xl mx-auto px-8 py-8 flex flex-col gap-5">
        {/* 1. Start / stop */}
        <section className="rounded-3xl bg-surface p-7">
          <div className="flex items-center gap-6">
            <BeaconArt variant="card" status={status} />
            <div className="flex-1 min-w-0">
              <h1 className="text-3xl font-extrabold tracking-tight">{title}</h1>
              <p className="text-soft mt-1.5 text-base">{subtitle}</p>
            </div>
            {isStopped ? (
              <Button
                variant="primary"
                size="lg"
                onClick={handleStart}
                icon={<Play className="w-5 h-5 fill-current" />}
              >
                Start
              </Button>
            ) : (
              <Button
                variant="secondary"
                size="lg"
                onClick={stopServer}
                disabled={!isRunning}
                icon={<Square className="w-4 h-4 fill-current" />}
              >
                Stop
              </Button>
            )}
          </div>

          {jvmProgress && (
            <div className="mt-6">
              <ProgressBar
                percent={jvmProgress.percent}
                label={jvmProgress.stage}
                detail={`${jvmProgress.percent.toFixed(0)}%`}
              />
            </div>
          )}

          {isStopped && (
            <div className="mt-6 pt-6 border-t border-line/60 flex flex-wrap items-center gap-x-6 gap-y-3">
              <WorldPicker />
              <label className="flex items-center gap-3">
                <span className="font-bold text-soft">Version</span>
                <select
                  value={selectedVersion}
                  onChange={(e) => setSelectedVersion(e.target.value)}
                  disabled={!versionList}
                  className="h-11 rounded-xl bg-raised px-3 font-bold text-ink cursor-pointer focus:outline-none focus:ring-2 focus:ring-brand/60 disabled:opacity-50"
                >
                  {!versionList && <option value={selectedVersion}>Loading versions…</option>}
                  {versionGroups.map(([group, items]) => (
                    <optgroup key={group} label={`${group}.x`}>
                      {items.map((v) => (
                        <option key={v} value={v}>
                          {v}
                          {v === newestVersion ? " (newest)" : VERSION_NOTES[v] ? ` (${VERSION_NOTES[v]})` : ""}
                        </option>
                      ))}
                    </optgroup>
                  ))}
                </select>
              </label>
              <div className="flex items-center gap-3">
                <span className="font-bold text-soft">Type</span>
                <Segmented<ServerCore>
                  value={selectedCore}
                  onChange={setSelectedCore}
                  options={[
                    { id: "paper", label: "Paper" },
                    { id: "fabric", label: "Fabric" },
                    { id: "neoforge", label: "NeoForge" },
                  ]}
                />
              </div>
              <span className="text-sm text-faint">
                {selectedCore === "paper"
                  ? "Classic Minecraft with plugin support."
                  : "Pick this if you want to add mods."}{" "}
                Version, type, mods and plugins belong to this world only.
              </span>
            </div>
          )}

          {isRunning && (
            <div className="mt-6 pt-6 border-t border-line/60 flex flex-wrap items-center gap-2">
              <span className="text-sm text-soft mr-auto">
                {tps >= 18 ? "Running smoothly" : tps >= 14 ? "A bit busy" : "Lagging"} ·{" "}
                {(ramUsed / 1024).toFixed(1)} of {(ramMax / 1024).toFixed(1)} GB memory used
              </span>
              <Button size="sm" onClick={() => sendCommand("time set day")} icon={<Sun className="w-4 h-4" />}>
                Make it day
              </Button>
              <Button size="sm" onClick={() => sendCommand("weather clear")} icon={<CloudSun className="w-4 h-4" />}>
                Clear weather
              </Button>
              <Button size="sm" onClick={() => sendCommand("save-all flush")} icon={<Save className="w-4 h-4" />}>
                Save now
              </Button>
            </div>
          )}
        </section>

        {/* 2. Friends */}
        <InviteCard
          busy={isNetBusy}
          onHost={() => startHostingRoom()}
          onJoin={joinFriendRoom}
          onLeave={leaveSession}
          version={selectedVersion}
        />

        {/* 3. Who's playing */}
        <Card
          title="Who's playing"
          action={
            players.length > 0 && (
              <Button variant="ghost" size="sm" onClick={() => onNavigate?.("players")}>
                See all
              </Button>
            )
          }
        >
          {players.length === 0 ? (
            <EmptyState
              icon={Users}
              title="Nobody's here yet"
              text={
                isRunning
                  ? "Send your code to a friend and they'll show up here."
                  : "Start your world, then invite some friends."
              }
            />
          ) : (
            <ul className="flex flex-col gap-2">
              {players.map((p) => (
                <li key={p.username} className="flex items-center gap-3 rounded-2xl bg-raised/60 p-3">
                  <PlayerHead name={p.username} />
                  <div className="flex-1 min-w-0">
                    <div className="font-bold truncate">
                      {p.username} {p.is_op && <Crown className="inline w-4 h-4 text-sun -mt-0.5" aria-label="Admin" />}
                    </div>
                    <div className="text-sm text-soft capitalize">{p.gamemode}</div>
                  </div>
                  <Button
                    size="sm"
                    variant="ghost"
                    onClick={() => executePlayerAction(p.username, "heal_feed")}
                    icon={<Heart className="w-4 h-4" />}
                  >
                    Heal
                  </Button>
                  <Button
                    size="sm"
                    onClick={() => {
                      openPlayerInventory(p);
                      onNavigate?.("players");
                    }}
                    icon={<Package className="w-4 h-4" />}
                  >
                    Inventory
                  </Button>
                </li>
              ))}
            </ul>
          )}
        </Card>

        {/* 4. For the curious */}
        <div className="px-2 pb-4">
          <Button
            variant="ghost"
            size="sm"
            onClick={() => onNavigate?.("monitor")}
            icon={<Activity className="w-4 h-4" />}
          >
            Open console &amp; performance
          </Button>
        </div>
      </div>
    </div>
  );
};

/* ---------- World picker ---------- */

const NEW_WORLD = "__new__";

const WorldPicker: React.FC = () => {
  const { worlds, selectedWorldId, selectWorld } = useServerStore();
  const live = (worlds ?? []).filter((w) => w.kind === "world");
  const [creating, setCreating] = useState(false);

  if (worlds === null) return null;

  // With no world yet the only sensible thing to do is make one.
  if (creating || live.length === 0) {
    return (
      <NewWorldForm
        className="basis-full"
        label="World"
        placeholder={live.length === 0 ? "Name your first world, e.g. Survival" : "Name your new world"}
        autoFocus={creating}
        onCancel={live.length > 0 ? () => setCreating(false) : undefined}
        onCreated={() => setCreating(false)}
      />
    );
  }

  return (
    <label className="flex items-center gap-3">
      <span className="font-bold text-soft">World</span>
      <select
        value={selectedWorldId ?? ""}
        onChange={(e) => (e.target.value === NEW_WORLD ? setCreating(true) : selectWorld(e.target.value))}
        className="h-11 max-w-[14rem] rounded-xl bg-raised px-3 font-bold text-ink cursor-pointer focus:outline-none focus:ring-2 focus:ring-brand/60"
      >
        {live.map((w) => (
          <option key={w.id} value={w.id}>
            {w.name}
          </option>
        ))}
        <option value={NEW_WORLD}>＋ New world…</option>
      </select>
    </label>
  );
};

/* ---------- Invite friends ---------- */

const InviteCard: React.FC<{
  busy: boolean;
  version: string;
  onHost: () => void;
  onJoin: (code: string, name: string) => void;
  onLeave: () => void;
}> = ({ busy, version, onHost, onJoin, onLeave }) => {
  const { network, modSyncStatus, startHostingRoom } = useNetworkStore();
  const [code, setCode] = useState("");
  const [ownCode, setOwnCode] = useState("");
  const [name, setName] = useState("");
  const [copied, setCopied] = useState<"code" | "invite" | null>(null);

  const copy = (what: "code" | "invite") => {
    if (!network.room_code) return;
    navigator.clipboard.writeText(
      what === "code"
        ? network.room_code
        : `Come play Minecraft ${version} with me! Open Beacon and join with code: ${network.room_code}`
    );
    setCopied(what);
    setTimeout(() => setCopied(null), 1800);
  };

  if (network.role === "host" && network.room_code) {
    const friends = network.connected_peers.length;
    return (
      <Card
        title="Invite friends"
        hint="Send this code to your friends. No port forwarding needed."
      >
        <div className="flex flex-wrap items-center gap-3">
          <div className="flex-1 min-w-[220px] h-14 rounded-2xl bg-raised px-5 flex items-center font-mono text-xl font-bold tracking-wide select-text">
            {network.room_code}
          </div>
          <Button
            variant="primary"
            size="lg"
            onClick={() => copy("code")}
            icon={copied === "code" ? <Check className="w-5 h-5" /> : <Copy className="w-5 h-5" />}
          >
            {copied === "code" ? "Copied!" : "Copy"}
          </Button>
        </div>
        <div className="flex flex-wrap items-center gap-2 mt-4">
          <span className="text-sm text-soft mr-auto">
            {friends === 0
              ? "Waiting for friends to connect…"
              : `${friends} ${friends === 1 ? "friend" : "friends"} connected`}
          </span>
          <Button
            size="sm"
            variant="ghost"
            onClick={() => copy("invite")}
            icon={copied === "invite" ? <Check className="w-4 h-4" /> : <MessageSquare className="w-4 h-4" />}
          >
            {copied === "invite" ? "Message copied" : "Copy invite message"}
          </Button>
          <Button size="sm" variant="ghost" onClick={onLeave}>
            Stop sharing
          </Button>
        </div>
      </Card>
    );
  }

  if (network.role === "client") {
    const syncing = modSyncStatus && !modSyncStatus.completed;
    return (
      <Card title={`You're in ${network.room_code ?? "a friend's world"}`}>
        {syncing && (
          <div className="mb-5">
            <ProgressBar
              percent={(modSyncStatus.synced_mods / Math.max(1, modSyncStatus.total_mods)) * 100}
              label="Getting your friend's mods…"
              detail={`${modSyncStatus.synced_mods} of ${modSyncStatus.total_mods}`}
            />
          </div>
        )}
        <ol className="flex flex-col gap-2 text-base">
          <li>
            <span className="font-bold">1.</span> Open Minecraft and go to{" "}
            <span className="font-bold">Multiplayer</span>.
          </li>
          <li>
            <span className="font-bold">2.</span> Your friend's world shows up at the bottom under{" "}
            <span className="font-bold">LAN worlds</span>. Join it!
          </li>
        </ol>
        <div className="flex justify-end mt-4">
          <Button size="sm" variant="ghost" onClick={onLeave}>
            Leave
          </Button>
        </div>
      </Card>
    );
  }

  return (
    <div className="grid grid-cols-1 md:grid-cols-2 gap-5">
      <Card title="Invite friends" hint="Get a short code to share with friends.">
        <Button variant="primary" onClick={onHost} disabled={busy} className="w-full">
          {busy ? "Making a code…" : "Get my code"}
        </Button>
        <div className="mt-4">
          <Disclosure label="Choose my own code">
            <form
              className="flex gap-2"
              onSubmit={(e) => {
                e.preventDefault();
                if (ownCode.trim()) startHostingRoom(ownCode.trim());
              }}
            >
              <TextInput
                value={ownCode}
                onChange={(e) => setOwnCode(e.target.value)}
                placeholder="e.g. cozy-cabin-7"
                className="font-mono"
                aria-label="Your own code"
              />
              <Button type="submit" disabled={busy || !ownCode.trim()}>
                Use
              </Button>
            </form>
          </Disclosure>
        </div>
      </Card>
      <Card title="Join a friend" hint="Got a code from a friend? Type it here.">
        <form
          className="flex flex-col gap-2"
          onSubmit={(e) => {
            e.preventDefault();
            onJoin(code, name);
          }}
        >
          <TextInput
            value={code}
            onChange={(e) => setCode(e.target.value)}
            placeholder="e.g. crimson-dragon-42"
            className="font-mono"
            aria-label="Friend's code"
          />
          <div className="flex gap-2">
            <TextInput
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="Your name"
              aria-label="Your name"
            />
            <Button type="submit" disabled={busy || !code.trim()}>
              {busy ? "Joining…" : "Join"}
            </Button>
          </div>
        </form>
      </Card>
    </div>
  );
};
