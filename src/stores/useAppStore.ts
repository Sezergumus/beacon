import { create } from "zustand";
import { AppSettings, formatAppError, tauriApi } from "../lib/tauri";
import { useServerStore } from "./useServerStore";

interface AppStoreState {
  /** null until the first load finishes. */
  settings: AppSettings | null;
  load: () => Promise<void>;
  update: (patch: Partial<AppSettings>) => Promise<void>;
}

/** Settings of Beacon itself (tray behaviour, the welcome tour), not of any world. */
export const useAppStore = create<AppStoreState>((set, get) => ({
  settings: null,

  load: async () => {
    try {
      set({ settings: await tauriApi.getAppSettings() });
    } catch {
      // If this can't load, don't pester the user with a tour on every launch.
      set({ settings: { close_to_tray: true, onboarding_done: true } });
    }
  },

  update: async (patch) => {
    const before = get().settings;
    if (!before) return;
    set({ settings: { ...before, ...patch } });
    try {
      set({ settings: await tauriApi.saveAppSettings({ ...before, ...patch }) });
    } catch (err) {
      set({ settings: before });
      useServerStore.getState().pushToast("error", "Couldn't save settings", formatAppError(err));
    }
  },
}));
