import { create } from "zustand";
import {
  formatAppError,
  MigrationEvent,
  ModSyncStatusEvent,
  NetworkStateSnapshot,
  SnapshotInfo,
  tauriApi,
} from "../lib/tauri";
import { useServerStore } from "./useServerStore";

interface NetworkStoreState {
  network: NetworkStateSnapshot;
  isBusy: boolean;
  modSyncStatus: ModSyncStatusEvent | null;
  migrationBanner: MigrationEvent | null;

  initializeNetworkStore: () => Promise<void>;
  startHostingRoom: (customRoomCode?: string) => Promise<void>;
  joinFriendRoom: (roomCode: string, displayName: string) => Promise<void>;
  leaveSession: () => Promise<void>;
  refreshNetworkState: () => Promise<void>;
  toggleTrustedCohost: (nodeId: string, trusted: boolean) => Promise<void>;
  createWorldSnapshotNow: () => Promise<void>;
  transferHostToPeer: (targetNodeId: string) => Promise<void>;
}

const DEFAULT_NETWORK_STATE: NetworkStateSnapshot = {
  role: "idle",
  node_id: "initializing...",
  room_code: null,
  relay_url: null,
  local_lan_port: null,
  upnp_external_addr: null,
  connected_peers: [],
  latest_snapshot: null,
  host_ping_ms: null,
  is_trusted_cohost: true,
  auto_failover_enabled: true,
};

export const useNetworkStore = create<NetworkStoreState>((set, get) => ({
  network: DEFAULT_NETWORK_STATE,
  isBusy: false,
  modSyncStatus: null,
  migrationBanner: null,

  initializeNetworkStore: async () => {
    try {
      const state = await tauriApi.getNetworkState();
      set({ network: state });
    } catch {
      // ignore initial error
    }

    tauriApi.onEvent<NetworkStateSnapshot>("network:state", (network) => {
      set({ network });
    });

    tauriApi.onEvent<string>("network:peer-updated", () => {
      get().refreshNetworkState();
    });

    tauriApi.onEvent<ModSyncStatusEvent>("network:mod-sync", (modSyncStatus) => {
      set({ modSyncStatus });
    });

    tauriApi.onEvent<SnapshotInfo>("migration:snapshot-created", (snap) => {
      set((s) => ({
        network: { ...s.network, latest_snapshot: snap },
      }));
      useServerStore
        .getState()
        .pushToast(
          "success",
          "World backed up",
          "A fresh copy of your world was saved."
        );
    });

    tauriApi.onEvent<SnapshotInfo>("migration:snapshot-synced", (snap) => {
      set((s) => ({
        network: { ...s.network, latest_snapshot: snap },
      }));
      useServerStore
        .getState()
        .pushToast(
          "info",
          "Backup received",
          "You now have the latest copy of your friend's world."
        );
    });

    tauriApi.onEvent<MigrationEvent>("migration:event", (migrationBanner) => {
      set({ migrationBanner });
      if (migrationBanner.stage === "completed") {
        get().refreshNetworkState();
        useServerStore
          .getState()
          .pushToast("success", "New host is ready", migrationBanner.message);
      }
    });
  },

  startHostingRoom: async (customRoomCode) => {
    set({ isBusy: true });
    try {
      const state = await tauriApi.startP2pHost(customRoomCode);
      set({ network: state, isBusy: false });
      useServerStore
        .getState()
        .pushToast(
          "success",
          "Your code is ready!",
          `Send ${state.room_code} to your friends.`
        );
    } catch (err) {
      set({ isBusy: false });
      useServerStore
        .getState()
        .pushToast("error", "Couldn't make a code", formatAppError(err));
    }
  },

  joinFriendRoom: async (roomCode, displayName) => {
    if (!roomCode.trim()) return;
    set({ isBusy: true, modSyncStatus: null });
    try {
      const state = await tauriApi.joinP2pRoom(
        roomCode.trim(),
        displayName.trim() || "BeaconPlayer"
      );
      set({ network: state, isBusy: false });
      useServerStore
        .getState()
        .pushToast(
          "success",
          "You're in!",
          "Open Minecraft → Multiplayer. The world is under LAN worlds."
        );
    } catch (err) {
      set({ isBusy: false });
      useServerStore
        .getState()
        .pushToast("error", "Couldn't join", formatAppError(err));
    }
  },

  leaveSession: async () => {
    try {
      const state = await tauriApi.leaveP2pSession();
      set({ network: state, modSyncStatus: null, migrationBanner: null });
      useServerStore
        .getState()
        .pushToast("info", "Disconnected", "You stopped sharing.");
    } catch (err) {
      useServerStore
        .getState()
        .pushToast("error", "Couldn't disconnect", formatAppError(err));
    }
  },

  refreshNetworkState: async () => {
    try {
      const state = await tauriApi.getNetworkState();
      set({ network: state });
    } catch {
      // ignore
    }
  },

  toggleTrustedCohost: async (nodeId, trusted) => {
    try {
      const state = await tauriApi.setTrustedCohost(nodeId, trusted);
      set({ network: state });
    } catch (err) {
      useServerStore
        .getState()
        .pushToast("error", "Couldn't change that", formatAppError(err));
    }
  },

  createWorldSnapshotNow: async () => {
    set({ isBusy: true });
    try {
      const snap = await tauriApi.createManualSnapshot();
      set((s) => ({
        isBusy: false,
        network: { ...s.network, latest_snapshot: snap },
      }));
    } catch (err) {
      set({ isBusy: false });
      useServerStore
        .getState()
        .pushToast("error", "Backup didn't work", formatAppError(err));
    }
  },

  transferHostToPeer: async (targetNodeId) => {
    set({ isBusy: true });
    try {
      await tauriApi.triggerHostMigration(targetNodeId);
      set({ isBusy: false });
      useServerStore
        .getState()
        .pushToast(
          "info",
          "Handing over…",
          "Your friend is taking over hosting."
        );
    } catch (err) {
      set({ isBusy: false });
      useServerStore
        .getState()
        .pushToast("error", "Couldn't hand over hosting", formatAppError(err));
    }
  },
}));
