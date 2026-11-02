import { create } from "zustand";
import { toast } from "sonner";
import { ipc, errMsg } from "@/lib/ipc";
import type { AppPrefs, Settings } from "@/types";
import { on } from "@/app/events";

interface SettingsState {
  settings: Settings | null;
  prefs: AppPrefs | null;
  load: () => Promise<void>;
  /** Applique tout de suite, enregistre après une courte attente. */
  patch: (p: Partial<Settings>) => void;
  patchPrefs: (p: Partial<AppPrefs>) => void;
}

let settingsTimer: ReturnType<typeof setTimeout> | undefined;
let prefsTimer: ReturnType<typeof setTimeout> | undefined;
let listening = false;

export const useSettings = create<SettingsState>((set, get) => ({
  settings: null,
  prefs: null,
  load: async () => {
    const [settings, prefs] = await Promise.all([ipc.getSettings(), ipc.getAppPrefs()]);
    set({ settings, prefs });
    if (!listening) {
      listening = true;
      on<Settings>("settings://changed", (s) => set({ settings: s }));
      on<AppPrefs>("prefs://changed", (p) => set({ prefs: p }));
    }
  },
  patch: (p) => {
    const cur = get().settings;
    if (!cur) return;
    const next = { ...cur, ...p };
    set({ settings: next });
    clearTimeout(settingsTimer);
    settingsTimer = setTimeout(async () => {
      try {
        const saved = await ipc.setSettings(get().settings!);
        set({ settings: saved });
      } catch (e) {
        toast.error(errMsg(e));
        set({ settings: await ipc.getSettings() });
      }
    }, 350);
  },
  patchPrefs: (p) => {
    const cur = get().prefs;
    if (!cur) return;
    set({ prefs: { ...cur, ...p } });
    clearTimeout(prefsTimer);
    prefsTimer = setTimeout(async () => {
      try {
        set({ prefs: await ipc.setAppPrefs(get().prefs!) });
      } catch (e) {
        toast.error(errMsg(e));
      }
    }, 150);
  },
}));
