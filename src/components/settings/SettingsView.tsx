import React, { useEffect, useState } from "react";
import { Check, Download, Loader2 } from "lucide-react";
import { useServerStore } from "../../stores/useServerStore";
import {
  AppSettings,
  ServerProperties,
  tauriApi,
  formatAppError,
} from "../../lib/tauri";
import { useAppStore } from "../../stores/useAppStore";
import { Button, Card, Disclosure, Page, Row, Segmented, Toggle } from "../ui";
import { Select } from "../Select";
import { ServerAppearance } from "../ServerAppearance";
import type { ActiveTab } from "../../App";

const DEFAULTS: ServerProperties = {
  online_mode: false,
  keep_inventory: true,
  difficulty: "normal",
  pvp: true,
  view_distance: 10,
  simulation_distance: 8,
  spawn_protection: 0,
  white_list: false,
  motd: "A Beacon world",
  max_players: 16,
  gamemode: "survival",
  allow_flight: true,
  server_ip: "127.0.0.1",
  server_port: 25565,
};

const AUTOSAVE_OPTIONS = [
  { mins: 1, label: "Every minute" },
  { mins: 5, label: "Every 5 minutes" },
  { mins: 10, label: "Every 10 minutes" },
  { mins: 15, label: "Every 15 minutes" },
  { mins: 30, label: "Every 30 minutes" },
  { mins: 60, label: "Every hour" },
  { mins: 0, label: "Only when the game saves itself" },
];

export const SettingsView: React.FC<{
  onNavigate?: (tab: ActiveTab) => void;
}> = ({ onNavigate }) => {
  const {
    hardware,
    customRamMb,
    setCustomRamMb,
    autosaveMins,
    setAutosaveMins,
    pushToast,
  } = useServerStore();
  // These are the settings new worlds start with, not the ones of any one world.
  const [draft, setDraft] = useState<ServerProperties>(DEFAULTS);
  const [installingJava, setInstallingJava] = useState(false);
  const appSettings = useAppStore((s) => s.settings);
  const saveApp = useAppStore((s) => s.update);

  useEffect(() => {
    tauriApi
      .getDefaultProperties()
      .then(setDraft)
      .catch((err) =>
        pushToast("error", "Couldn't load your settings", formatAppError(err)),
      );
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  /** Update locally and save right away. */
  const save = async <K extends keyof ServerProperties>(
    key: K,
    value: ServerProperties[K],
  ) => {
    const next = { ...draft, [key]: value };
    setDraft(next);
    try {
      setDraft(await tauriApi.saveDefaultProperties(next));
      pushToast("success", "Saved", "New worlds will start with this.");
    } catch (err) {
      pushToast("error", "Couldn't save settings", formatAppError(err));
    }
  };

  /** Update locally only (used while dragging a slider or typing). */
  const edit = <K extends keyof ServerProperties>(
    key: K,
    value: ServerProperties[K],
  ) => setDraft((d) => ({ ...d, [key]: value }));

  const installJava = async () => {
    setInstallingJava(true);
    try {
      await tauriApi.provisionJavaRuntime(21);
      const hw = await tauriApi.getHardwareProfile(customRamMb, 21);
      useServerStore.setState({ hardware: hw });
      pushToast("success", "Java is ready", "Your world can start now.");
    } catch (err) {
      pushToast("error", "Couldn't install Java", formatAppError(err));
    } finally {
      setInstallingJava(false);
    }
  };

  const recommended = hardware?.recommended_ram_mb ?? 4096;

  const distanceSlider = ({
    k,
    title,
    hint,
    min,
    max,
    unit,
  }: {
    k: "view_distance" | "simulation_distance" | "spawn_protection";
    title: string;
    hint: string;
    min: number;
    max: number;
    unit: string;
  }) => (
    <Row title={title} hint={hint}>
      <div className="flex items-center gap-3 w-64">
        <input
          type="range"
          min={min}
          max={max}
          value={draft[k]}
          onChange={(e) => edit(k, Number(e.target.value))}
          onPointerUp={(e) =>
            save(k, Number((e.target as HTMLInputElement).value))
          }
          onKeyUp={(e) => save(k, Number((e.target as HTMLInputElement).value))}
          className="flex-1 accent-[rgb(var(--brand))] cursor-pointer"
          aria-label={title}
        />
        <span className="w-20 text-right font-bold text-sm">
          {draft[k]} {unit}
        </span>
      </div>
    </Row>
  );

  return (
    <Page title="Settings" subtitle="Changes save automatically.">
      <div className="rounded-2xl bg-brand/15 p-4 flex items-center gap-4">
        <p className="flex-1 text-sm font-bold">
          The game rules, welcome message, icon and advanced options below are
          the starting settings for{" "}
          <span className="text-brand">new worlds</span>. Every world keeps its
          own copy: to change an existing world, open it in Worlds and use its
          Settings.
        </p>
        {onNavigate && (
          <Button
            size="sm"
            onClick={() => onNavigate("worlds")}
            className="shrink-0"
          >
            Open Worlds
          </Button>
        )}
      </div>

      <Card title="Game rules">
        <Row title="Keep inventory" hint="Nobody loses their stuff on death.">
          <Toggle
            checked={draft.keep_inventory}
            onChange={(v) => save("keep_inventory", v)}
            label="Keep items"
          />
        </Row>
        <Row title="PvP" hint="Players can fight each other.">
          <Toggle
            checked={draft.pvp}
            onChange={(v) => save("pvp", v)}
            label="PvP"
          />
        </Row>
        <Row title="Online Mode" hint="Enables cracked accounts to join.">
          <Toggle
            checked={!draft.online_mode}
            onChange={(v) => save("online_mode", !v)}
            label="Allow non-premium accounts"
          />
        </Row>
        <Row
          title="Invite only"
          hint="Only players on your whitelist can get in."
        >
          <Toggle
            checked={draft.white_list}
            onChange={(v) => save("white_list", v)}
            label="Whitelist"
          />
        </Row>
        <Row title="Difficulty">
          <Segmented
            value={draft.difficulty}
            onChange={(v) => save("difficulty", v)}
            options={[
              { id: "peaceful", label: "Peaceful" },
              { id: "easy", label: "Easy" },
              { id: "normal", label: "Normal" },
              { id: "hard", label: "Hard" },
            ]}
          />
        </Row>
        <Row title="New players start in">
          <Segmented
            value={draft.gamemode}
            onChange={(v) => save("gamemode", v)}
            options={[
              { id: "survival", label: "Survival" },
              { id: "creative", label: "Creative" },
              { id: "adventure", label: "Adventure" },
            ]}
          />
        </Row>
      </Card>

      <Card title="Server list">
        <ServerAppearance
          worldId={null}
          motd={draft.motd}
          onMotdChange={(motd) => edit("motd", motd)}
          onMotdCommit={(motd) => save("motd", motd)}
          maxPlayers={draft.max_players}
        />
      </Card>

      <Card title="Performance">
        <Row
          title="Memory for your world"
          hint={`We suggest ${(recommended / 1024).toFixed(0)} GB for your PC. More memory helps with mods and lots of players.`}
        >
          <div className="flex items-center gap-3 w-64">
            <input
              type="range"
              min={2048}
              max={10240}
              step={512}
              value={customRamMb}
              onChange={(e) => setCustomRamMb(Number(e.target.value))}
              className="flex-1 accent-[rgb(var(--brand))] cursor-pointer"
              aria-label="Memory"
            />
            <span className="w-20 text-right font-bold text-sm">
              {(customRamMb / 1024).toFixed(1)} GB
            </span>
          </div>
        </Row>
        <Row
          title="Save while playing"
          hint="How often the world is saved to disk while friends are in it. The game also saves by itself now and then."
        >
          <Select<number>
            aria-label="Save while playing"
            size="sm"
            className="w-64"
            value={autosaveMins}
            onChange={setAutosaveMins}
            options={AUTOSAVE_OPTIONS.map((o) => ({
              value: o.mins,
              label: o.label,
            }))}
          />
        </Row>
        <Row
          title="Java"
          hint={
            hardware?.java_installed
              ? "Installed and ready."
              : "Downloaded for you the first time you start."
          }
        >
          {hardware?.java_installed ? (
            <span className="flex items-center gap-1.5 font-bold text-brand">
              <Check className="w-4 h-4" /> Ready
            </span>
          ) : (
            <Button
              size="sm"
              onClick={installJava}
              disabled={installingJava}
              icon={
                installingJava ? (
                  <Loader2 className="w-4 h-4 animate-spin" />
                ) : (
                  <Download className="w-4 h-4" />
                )
              }
            >
              {installingJava ? "Installing…" : "Install now"}
            </Button>
          )}
        </Row>
      </Card>

      <Card title="Beacon">
        <Row
          title="Minimize to tray"
          hint="Keeps your server running on the background."
        >
          <Select<AppSettings["close_action"]>
            aria-label="When you close the window"
            size="sm"
            className="w-64"
            value={appSettings?.close_action ?? "tray_when_running"}
            onChange={(close_action) => saveApp({ close_action })}
            options={[
              { value: "quit", label: "Disabled" },
              {
                value: "tray_when_running",
                label: "Minimize when world is open",
              },
              { value: "tray_always", label: "Always minimize" },
            ]}
          />
        </Row>
        <Row title="Welcome tour" hint="Show the quick introduction again.">
          <Button size="sm" onClick={() => saveApp({ onboarding_done: false })}>
            Show again
          </Button>
        </Row>
      </Card>

      <Card title="About">
        <p className="text-sm text-soft">
          Beacon is free software under the GNU GPL v3 (source on GitHub: Sezergumus/beacon). It is
          a fan project. It is not an official Minecraft product and is not approved
          by or associated with Mojang or Microsoft. Item pictures are Mojang's artwork; some come
          from the Minecraft Wiki (CC BY-NC-SA 3.0).
        </p>
      </Card>

      <div className="px-2 pb-4">
        <Disclosure label="Advanced">
          <Card>
            {distanceSlider({
              k: "view_distance",
              title: "View distance",
              hint: "How far players can see. Lower is faster.",
              min: 4,
              max: 32,
              unit: "chunks",
            })}
            {distanceSlider({
              k: "simulation_distance",
              title: "Active area",
              hint: "How far away crops grow and mobs move.",
              min: 4,
              max: 32,
              unit: "chunks",
            })}
            {distanceSlider({
              k: "spawn_protection",
              title: "Spawn protection",
              hint: "Blocks around spawn only admins can change.",
              min: 0,
              max: 64,
              unit: "blocks",
            })}
            {hardware?.jvm_flags && hardware.jvm_flags.length > 0 && (
              <div className="pt-4">
                <div className="font-bold mb-2">Java launch flags</div>
                <div className="rounded-xl bg-raised p-3 font-mono text-xs text-soft max-h-32 overflow-y-auto select-text">
                  {hardware.jvm_flags.join(" ")}
                </div>
              </div>
            )}
          </Card>
        </Disclosure>
      </div>
    </Page>
  );
};
