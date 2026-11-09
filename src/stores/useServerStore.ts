import { create } from "zustand";
import {
  formatAppError,
  HardwareProfile,
  InstalledContentItem,
  JvmProgressEvent,
  ModpackProgressEvent,
  ModrinthSearchHit,
  NewWorldOptions,
  OnlinePlayer,
  PlayerInventoryData,
  ServerCore,
  ServerLogEntry,
  ServerProperties,
  ServerStatus,
  ServerTelemetry,
  tauriApi,
  WorldEntry,
} from "../lib/tauri";

const ACTION_LABELS: Record<string, string> = {
  heal_feed: "Healed",
  op: "Made admin:",
  deop: "Removed admin from",
  kill: "Respawned",
  clear: "Emptied the inventory of",
  gamemode: "Changed game mode for",
  tp: "Teleported",
  kick: "Kicked",
  ban: "Banned",
  give_item: "Gave an item to",
  delete_slot: "Took an item from",
};

export interface ToastNotice {
  id: string;
  type: "success" | "error" | "info";
  title: string;
  message: string;
}

export interface TelemetryPoint {
  time: string;
  tps: number;
  ramUsedMb: number;
  cpuPercent: number;
}

interface ServerStoreState {
  hardware: HardwareProfile | null;
  status: ServerStatus;
  selectedCore: ServerCore;
  selectedVersion: string;
  /** Minecraft versions per server type, newest first (loaded from the web). */
  versions: Partial<Record<ServerCore, string[]>>;
  customRamMb: number;
  /** Minutes between automatic saves of a running world. 0 = off. */
  autosaveMins: number;
  telemetry: ServerTelemetry | null;
  telemetryHistory: TelemetryPoint[];
  logs: ServerLogEntry[];
  properties: ServerProperties | null;
  jvmProgress: JvmProgressEvent | null;
  modpackProgress: ModpackProgressEvent | null;
  activeInventory: PlayerInventoryData | null;
  installedContent: InstalledContentItem[];
  /** Modrinth projects that are installed in the selected world (recognised by file hash). */
  installedProjectIds: string[];
  searchResults: ModrinthSearchHit[];
  isSearchingMods: boolean;
  installingSlug: string | null;
  /** Modrinth project whose files are being deleted right now. */
  removingProjectId: string | null;
  toasts: ToastNotice[];
  /** Every world and backup, as listed by the backend (null until first load). */
  worlds: WorldEntry[] | null;
  /** The world the next Start will open. */
  selectedWorldId: string | null;

  refreshWorlds: () => Promise<void>;
  /** Switch to another world: its own version, type, mods and settings load. */
  selectWorld: (id: string) => Promise<void>;
  /** Makes a world, selects it and returns its id (null if it failed). */
  createWorld: (name: string, options?: NewWorldOptions) => Promise<string | null>;
  renameWorld: (id: string, name: string) => Promise<boolean>;
  pushToast: (type: ToastNotice["type"], title: string, message: string) => void;
  dismissToast: (id: string) => void;
  initializeServerStore: () => Promise<void>;
  loadVersions: (core: ServerCore) => Promise<void>;
  setSelectedCore: (core: ServerCore) => void;
  setSelectedVersion: (version: string) => void;
  setCustomRamMb: (ramMb: number) => Promise<void>;
  setAutosaveMins: (mins: number) => void;
  startServer: () => Promise<void>;
  stopServer: () => Promise<void>;
  sendCommand: (cmd: string) => Promise<void>;
  clearLogs: () => void;
  updateProperties: (next: ServerProperties) => Promise<void>;
  executePlayerAction: (player: string, action: string, payload?: string) => Promise<void>;
  openPlayerInventory: (player: OnlinePlayer) => Promise<void>;
  closePlayerInventory: () => void;
  searchModrinth: (query: string, projectType: string) => Promise<void>;
  installModrinthItem: (hit: ModrinthSearchHit) => Promise<void>;
  installLocalMrpack: (path: string) => Promise<void>;
  refreshInstalledContent: () => Promise<void>;
  /** Remove every installed file that belongs to this Modrinth project. */
  removeProject: (projectId: string) => Promise<void>;
  removeContentItem: (filename: string, kind: string) => Promise<void>;
}

const AUTOSAVE_KEY = "beacon.autosaveMins";
const DEFAULT_AUTOSAVE_MINS = 10;

const loadAutosaveMins = (): number => {
  try {
    const raw = localStorage.getItem(AUTOSAVE_KEY);
    const n = raw === null ? NaN : Number(raw);
    return Number.isFinite(n) && n >= 0 ? n : DEFAULT_AUTOSAVE_MINS;
  } catch {
    return DEFAULT_AUTOSAVE_MINS;
  }
};

const FALLBACK_VERSIONS: Record<ServerCore, string[]> = {
  paper: ["1.21.4", "1.21.1", "1.20.4", "1.20.1", "1.19.4", "1.18.2", "1.16.5", "1.12.2", "1.8.8"],
  fabric: ["1.21.4", "1.21.1", "1.20.4", "1.20.1", "1.19.4", "1.18.2", "1.16.5"],
  neoforge: ["1.21.4", "1.21.1", "1.20.4"],
};

/** Set once the store has loaded its data and bound the backend event listeners. */
let storeInitialized = false;

/** File hash (SHA-512) -> Modrinth project it belongs to, or null if Modrinth doesn't know the file. */
const projectByHash = new Map<string, string | null>();

const projectIdsOf = (items: InstalledContentItem[]): string[] => [
  ...new Set(
    items.map((i) => projectByHash.get(i.sha512.toLowerCase())).filter((id): id is string => !!id),
  ),
];

/** If the chosen version isn't offered for this server type, move to a sensible one. */
const keepVersionValid = (core: ServerCore) => {
  const { versions, selectedVersion, selectedCore, pushToast } = useServerStore.getState();
  const list = versions[core];
  if (core !== selectedCore || !list || list.includes(selectedVersion)) return;
  const next = list.includes("1.21.1") ? "1.21.1" : list[0];
  useServerStore.setState({ selectedVersion: next });
  pushToast("info", `Switched to ${next}`, `${selectedVersion} isn't available for this server type.`);
};

/** Show a world's own server type and version in the controls. */
const applyWorldConfig = (world: WorldEntry) => {
  const patch: Partial<Pick<ServerStoreState, "selectedCore" | "selectedVersion">> = {};
  if (world.core) patch.selectedCore = world.core;
  if (world.mc_version) patch.selectedVersion = world.mc_version;
  if (Object.keys(patch).length) {
    useServerStore.setState(patch);
    if (patch.selectedCore) useServerStore.getState().loadVersions(patch.selectedCore);
  }
};

/** Point the backend (mods, settings, players) at this world and reload what depends on it. */
const activateWorld = async (id: string) => {
  const { pushToast } = useServerStore.getState();
  try {
    await tauriApi.setActiveWorld(id);
    const properties = await tauriApi.getServerProperties();
    useServerStore.setState({ properties });
    await useServerStore.getState().refreshInstalledContent();
  } catch (err) {
    pushToast("error", "Couldn't switch worlds", formatAppError(err));
  }
};

/** Save the version/type shown in the controls onto the selected world only. */
const saveWorldConfig = async () => {
  const { selectedWorldId, selectedCore, selectedVersion, pushToast, refreshWorlds } =
    useServerStore.getState();
  if (!selectedWorldId) return; // no world yet: the first one is created with these values
  try {
    await tauriApi.setWorldConfig(selectedWorldId, selectedCore, selectedVersion);
    await refreshWorlds();
  } catch (err) {
    pushToast("error", "Couldn't change the version", formatAppError(err));
    await refreshWorlds();
  }
};

export const useServerStore = create<ServerStoreState>((set, get) => ({
  hardware: null,
  status: "stopped",
  selectedCore: "paper",
  selectedVersion: "1.21.1",
  versions: {},
  customRamMb: 4096,
  autosaveMins: loadAutosaveMins(),
  telemetry: null,
  telemetryHistory: [],
  logs: [],
  properties: null,
  jvmProgress: null,
  modpackProgress: null,
  activeInventory: null,
  installedContent: [],
  installedProjectIds: [],
  searchResults: [],
  isSearchingMods: false,
  installingSlug: null,
  removingProjectId: null,
  toasts: [],
  worlds: null,
  selectedWorldId: null,

  refreshWorlds: async () => {
    try {
      const worlds = await tauriApi.listWorlds();
      const live = worlds.filter((w) => w.kind === "world");
      const { selectedWorldId } = get();
      // Keep the player's pick if it still exists, otherwise follow the backend's last-used world.
      const pick =
        live.find((w) => w.id === selectedWorldId) ?? live.find((w) => w.active) ?? live[0];
      set({ worlds, selectedWorldId: pick?.id ?? null });

      if (pick) {
        // Version and type belong to the world, so the controls always mirror it.
        applyWorldConfig(pick);
        // The backend may still be on another world (e.g. the first one ever picked).
        if (!pick.active && get().status === "stopped") {
          await activateWorld(pick.id);
        }
      }
    } catch (err) {
      set((s) => ({ worlds: s.worlds ?? [] }));
      get().pushToast("error", "Couldn't load your worlds", formatAppError(err));
    }
  },

  selectWorld: async (id) => {
    if (get().status !== "stopped" || get().selectedWorldId === id) return;
    set({ selectedWorldId: id });
    const world = get().worlds?.find((w) => w.kind === "world" && w.id === id);
    if (world) applyWorldConfig(world);
    await activateWorld(id);
    get().refreshWorlds();
  },

  createWorld: async (name, options) => {
    const { selectedCore, selectedVersion } = get();
    try {
      const meta = await tauriApi.createWorld(name, selectedCore, selectedVersion, options);
      set({ selectedWorldId: meta.id });
      await activateWorld(meta.id);
      await get().refreshWorlds();
      return meta.id;
    } catch (err) {
      get().pushToast("error", "Couldn't make that world", formatAppError(err));
      return null;
    }
  },

  renameWorld: async (id, name) => {
    try {
      await tauriApi.renameWorld(id, name);
      await get().refreshWorlds();
      return true;
    } catch (err) {
      get().pushToast("error", "Couldn't rename that world", formatAppError(err));
      return false;
    }
  },

  pushToast: (type, title, message) => {
    const id = `${Date.now()}-${Math.random().toString(36).slice(2, 7)}`;
    set((s) => ({
      toasts: [...s.toasts.slice(-4), { id, type, title, message }],
    }));
    setTimeout(() => {
      get().dismissToast(id);
    }, 5500);
  },

  dismissToast: (id) =>
    set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) })),

  initializeServerStore: async () => {
    // React StrictMode mounts twice in dev; a second run would add every listener again
    // and each log line would show up twice.
    if (storeInitialized) return;
    storeInitialized = true;
    try {
      const [hw, props, telemetry, logs] = await Promise.all([
        tauriApi.getHardwareProfile(),
        tauriApi.getServerProperties(),
        tauriApi.getServerTelemetry(),
        tauriApi.getServerLogs(),
      ]);

      tauriApi.setAutosaveInterval(get().autosaveMins).catch(() => {});
      set({
        hardware: hw,
        customRamMb: hw.recommended_ram_mb,
        properties: props,
        telemetry,
        status: telemetry.status,
        logs,
      });
    } catch (err) {
      get().pushToast("error", "Couldn't load everything", formatAppError(err));
    }

    get().refreshInstalledContent();
    get().refreshWorlds();
    get().loadVersions(get().selectedCore);

    // Subscribe to real-time Tauri backend events
    tauriApi.onEvent<ServerStatus>("server:status", (status) => {
      set({ status });
      if (status === "provisioning") set({ telemetryHistory: [] });
      if (status === "stopped") set({ jvmProgress: null });
      if (status === "running") {
        set({ jvmProgress: null });
        get().pushToast(
          "success",
          "Your world is open!",
          "Share your code so friends can join."
        );
      }
    });

    tauriApi.onEvent<ServerLogEntry>("server:log", (entry) => {
      set((s) => ({
        logs: [...s.logs.slice(-600), entry],
      }));
    });

    tauriApi.onEvent<ServerTelemetry>("server:telemetry", (telemetry) => {
      const point: TelemetryPoint = {
        time: new Date().toLocaleTimeString([], {
          hour: "2-digit",
          minute: "2-digit",
          second: "2-digit",
        }),
        tps: telemetry.tps,
        ramUsedMb: telemetry.ram_used_mb,
        cpuPercent: telemetry.cpu_usage_percent,
      };
      set((s) => ({
        telemetry,
        status: telemetry.status,
        telemetryHistory: [...s.telemetryHistory.slice(-89), point],
      }));
    });

    tauriApi.onEvent<JvmProgressEvent>("jvm:download-progress", (jvmProgress) => {
      set({ jvmProgress });
    });

    tauriApi.onEvent<ModpackProgressEvent>(
      "content:modpack-progress",
      (modpackProgress) => {
        set({ modpackProgress });
      }
    );

    tauriApi.onEvent<PlayerInventoryData>(
      "player:inventory-updated",
      (invData) => {
        const current = get().activeInventory;
        if (current && current.uuid === invData.uuid) {
          set({
            activeInventory: {
              ...invData,
              username: current.username ?? invData.username,
            },
          });
        }
      }
    );
  },

  loadVersions: async (core) => {
    if (get().versions[core]) return;
    try {
      const list = await tauriApi.listMinecraftVersions(core);
      set((s) => ({ versions: { ...s.versions, [core]: list } }));
    } catch (err) {
      // Offline: a short list of well-known versions keeps Start usable.
      set((s) => ({ versions: { ...s.versions, [core]: FALLBACK_VERSIONS[core] } }));
      get().pushToast(
        "info",
        "Couldn't load all versions",
        `Showing the popular ones for now. ${formatAppError(err)}`
      );
    }
    keepVersionValid(core);
  },

  setSelectedCore: (selectedCore) => {
    set({ selectedCore });
    // Each server type supports different versions; load them, then fix the pick if needed.
    get()
      .loadVersions(selectedCore)
      .then(() => keepVersionValid(selectedCore))
      .then(saveWorldConfig);
  },
  setSelectedVersion: (selectedVersion) => {
    set({ selectedVersion });
    saveWorldConfig();
  },

  setAutosaveMins: (mins) => {
    set({ autosaveMins: mins });
    try {
      localStorage.setItem(AUTOSAVE_KEY, String(mins));
    } catch {
      // private mode etc. - the value just won't survive a restart
    }
    tauriApi.setAutosaveInterval(mins).catch(() => {});
  },

  setCustomRamMb: async (ramMb) => {
    set({ customRamMb: ramMb });
    try {
      const hw = await tauriApi.getHardwareProfile(ramMb, 21);
      set({ hardware: hw });
    } catch {
      // ignore
    }
  },

  startServer: async () => {
    const { selectedCore, selectedVersion, customRamMb, selectedWorldId } = get();
    try {
      await tauriApi.startMinecraftServer({
        core: selectedCore,
        mc_version: selectedVersion,
        allocated_ram_mb: customRamMb,
        world_id: selectedWorldId ?? undefined,
      });
      // A first start creates "My world"; pick it up in the lists.
      get().refreshWorlds();
    } catch (err) {
      get().pushToast("error", "Your world couldn't start", formatAppError(err));
    }
  },

  stopServer: async () => {
    try {
      await tauriApi.stopMinecraftServer();
      get().pushToast(
        "info",
        "World closed",
        "Everything was saved. See you next time!"
      );
    } catch (err) {
      get().pushToast("error", "Couldn't close the world", formatAppError(err));
    }
  },

  sendCommand: async (cmd) => {
    if (!cmd.trim()) return;
    try {
      await tauriApi.sendServerCommand(cmd);
    } catch (err) {
      get().pushToast("error", "Command didn't work", formatAppError(err));
    }
  },

  clearLogs: () => set({ logs: [] }),

  updateProperties: async (next) => {
    set({ properties: next });
    try {
      const saved = await tauriApi.saveServerProperties(next);
      set({ properties: saved });
      get().pushToast(
        "success",
        "Saved",
        "Your settings are updated."
      );
    } catch (err) {
      get().pushToast("error", "Couldn't save settings", formatAppError(err));
    }
  },

  executePlayerAction: async (player, action, payload) => {
    try {
      await tauriApi.runPlayerAction(player, action, payload);
      get().pushToast(
        "success",
        "Done!",
        `${ACTION_LABELS[action] ?? "Updated"} ${player}.`
      );
      const active = get().activeInventory;
      if (
        active &&
        active.username === player &&
        (action === "delete_slot" || action === "give_item" || action === "clear")
      ) {
        const refreshed = await tauriApi.inspectPlayerInventory(
          active.uuid,
          player
        );
        set({ activeInventory: { ...refreshed, username: player } });
      }
    } catch (err) {
      get().pushToast("error", "That didn't work", formatAppError(err));
    }
  },

  openPlayerInventory: async (player) => {
    try {
      const inv = await tauriApi.inspectPlayerInventory(
        player.uuid,
        player.username
      );
      set({
        activeInventory: {
          ...inv,
          username: player.username,
        },
      });
    } catch (err) {
      get().pushToast(
        "error",
        "Couldn't open the inventory",
        formatAppError(err)
      );
    }
  },

  closePlayerInventory: () => set({ activeInventory: null }),

  searchModrinth: async (query, projectType) => {
    const { selectedVersion, selectedCore } = get();
    set({ isSearchingMods: true });
    try {
      const hits = await tauriApi.searchModrinthCatalog(
        query,
        projectType,
        selectedVersion,
        selectedCore
      );
      set({ searchResults: hits, isSearchingMods: false });
    } catch (err) {
      set({ isSearchingMods: false });
      get().pushToast("error", "Search didn't work", formatAppError(err));
    }
  },

  installModrinthItem: async (hit) => {
    const { selectedVersion, selectedCore, selectedWorldId } = get();
    if (!selectedWorldId) {
      get().pushToast("info", "Make a world first", "Mods and plugins are added to a specific world.");
      return;
    }
    set({ installingSlug: hit.slug });
    try {
      const filename = await tauriApi.installModrinthItem(
        hit.project_id,
        selectedVersion,
        selectedCore,
        hit.project_type
      );
      await get().refreshInstalledContent();
      set({ installingSlug: null, modpackProgress: null });
      get().pushToast(
        "success",
        "Added to this world",
        `${hit.title} is ready to use.`
      );
    } catch (err) {
      set({ installingSlug: null, modpackProgress: null });
      get().pushToast("error", "Couldn't add that", formatAppError(err));
    }
  },

  installLocalMrpack: async (path) => {
    if (!path.trim()) return;
    if (!get().selectedWorldId) {
      get().pushToast("info", "Make a world first", "Modpacks are installed into a specific world.");
      return;
    }
    set({ installingSlug: "local-mrpack" });
    try {
      const packName = await tauriApi.installLocalMrpack(path.trim());
      await get().refreshInstalledContent();
      set({ installingSlug: null, modpackProgress: null });
      get().pushToast(
        "success",
        "Modpack installed",
        `${packName} is ready to play.`
      );
    } catch (err) {
      set({ installingSlug: null, modpackProgress: null });
      get().pushToast("error", "Couldn't install the modpack", formatAppError(err));
    }
  },

  refreshInstalledContent: async () => {
    try {
      const installedContent = await tauriApi.getInstalledContent();
      set({ installedContent, installedProjectIds: projectIdsOf(installedContent) });

      // Ask Modrinth which projects the files belong to (only for files not seen before).
      const unknown = [
        ...new Set(
          installedContent.map((i) => i.sha512.toLowerCase()).filter((h) => h && !projectByHash.has(h)),
        ),
      ];
      if (unknown.length === 0) return;
      const found = await tauriApi.identifyInstalledContent(unknown);
      for (const h of unknown) projectByHash.set(h, found[h] ?? null);
      // The list may have changed while waiting, so work from the current one.
      set((s) => ({ installedProjectIds: projectIdsOf(s.installedContent) }));
    } catch {
      // Offline or Modrinth is down: the list still shows, only the "Installed" marks lag behind.
    }
  },

  removeProject: async (projectId) => {
    const files = get().installedContent.filter(
      (i) => projectByHash.get(i.sha512.toLowerCase()) === projectId,
    );
    if (get().removingProjectId) return;
    set({ removingProjectId: projectId });
    try {
      for (const f of files) await get().removeContentItem(f.filename, f.kind);
    } finally {
      set({ removingProjectId: null });
    }
  },

  removeContentItem: async (filename, kind) => {
    try {
      await tauriApi.deleteInstalledContent(filename, kind);
      await get().refreshInstalledContent();
      get().pushToast("info", "Removed", `${filename} was removed from your world.`);
    } catch (err) {
      get().pushToast("error", "Couldn't remove that", formatAppError(err));
    }
  },
}));
