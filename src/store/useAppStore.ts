// Глобальный стейт приложения на Zustand.
// Phase 1: версия ядра, настройки (с персистом).
// Phase 2: профили (CRUD) + выбор активного.
// Phase 3: состояние подключения + connect/disconnect (proxy mode).

import { create } from "zustand";
import { listen } from "@tauri-apps/api/event";
import { ipc } from "../lib/ipc";
import type {
  ConnectionState,
  Outbound,
  Profile,
  Settings,
  Subscription,
  Traffic,
} from "../lib/types";

interface AppState {
  coreVersion: string | null;
  coreError: string | null;
  settings: Settings | null;
  connection: ConnectionState;
  profiles: Profile[];
  subscriptions: Subscription[];
  localProxy: string | null;
  traffic: Traffic | null;
  isElevated: boolean;
  /** AmneziaWG идёт через awg-shim + sing-box (split и статистика работают). */
  awgShim: boolean;

  /** Первичная загрузка + подписка на события подключения. */
  init: () => Promise<void>;
  relaunchElevated: () => Promise<void>;

  addSubscription: (name: string, url: string) => Promise<void>;
  updateSubscription: (id: string) => Promise<void>;
  deleteSubscription: (id: string) => Promise<void>;
  updateSettings: (patch: Partial<Settings>) => Promise<void>;

  createProfile: (name: string, outbound: Outbound) => Promise<void>;
  updateProfile: (profile: Profile) => Promise<void>;
  deleteProfile: (id: string) => Promise<void>;
  duplicateProfile: (id: string) => Promise<void>;
  /** Импорт из ссылки/JSON. Бросает ошибку при неудаче (показывает диалог). */
  importProfile: (input: string, name?: string) => Promise<void>;
  setActiveProfile: (id: string | null) => Promise<void>;

  connect: () => Promise<void>;
  disconnect: () => Promise<void>;
}

let unlisten: (() => void) | null = null;

export const useAppStore = create<AppState>((set, get) => ({
  coreVersion: null,
  coreError: null,
  settings: null,
  connection: { state: "disconnected" },
  profiles: [],
  subscriptions: [],
  localProxy: null,
  traffic: null,
  isElevated: true,
  awgShim: false,

  init: async () => {
    try {
      const version = await ipc.singboxVersion();
      set({ coreVersion: version, coreError: null });
    } catch (e) {
      set({ coreError: String(e) });
    }

    try {
      const [settings, profiles, subscriptions, connection, localProxy, isElevated, awgShim] =
        await Promise.all([
          ipc.getSettings(),
          ipc.listProfiles(),
          ipc.listSubscriptions(),
          ipc.getConnectionState(),
          ipc.localProxyAddr(),
          ipc.isElevated(),
          ipc.awgShimAvailable(),
        ]);
      set({ settings, profiles, subscriptions, connection, localProxy, isElevated, awgShim });
    } catch (e) {
      console.error("init load failed:", e);
    }

    // события из бэкенда: состояние подключения + статистика трафика
    if (!unlisten) {
      const offState = await listen<ConnectionState>("connection-state", (e) => {
        const next = e.payload;
        // при выходе из Connected сбрасываем индикатор трафика
        if (next.state !== "connected") set({ connection: next, traffic: null });
        else set({ connection: next });
      });
      const offTraffic = await listen<Traffic>("traffic", (e) => {
        set({ traffic: e.payload });
      });
      unlisten = () => {
        offState();
        offTraffic();
      };
    }

    // автоподключение при старте (если включено и есть активный профиль)
    const st = get().settings;
    if (
      st?.autoConnect &&
      st.activeProfileId &&
      get().connection.state === "disconnected"
    ) {
      void get().connect();
    }
  },

  updateSettings: async (patch) => {
    const current = get().settings;
    if (!current) return;
    const next = { ...current, ...patch };
    set({ settings: next });
    try {
      const saved = await ipc.saveSettings(next);
      set({ settings: saved });
    } catch (e) {
      console.error("save_settings failed:", e);
      set({ settings: current });
    }
  },

  createProfile: async (name, outbound) => {
    const profile = await ipc.createProfile(name, outbound);
    set({ profiles: [...get().profiles, profile] });
  },

  updateProfile: async (profile) => {
    const saved = await ipc.updateProfile(profile);
    set({
      profiles: get().profiles.map((p) => (p.id === saved.id ? saved : p)),
    });
  },

  deleteProfile: async (id) => {
    await ipc.deleteProfile(id);
    set({ profiles: get().profiles.filter((p) => p.id !== id) });
    if (get().settings?.activeProfileId === id) {
      await get().setActiveProfile(null);
    }
  },

  duplicateProfile: async (id) => {
    const copy = await ipc.duplicateProfile(id);
    set({ profiles: [...get().profiles, copy] });
  },

  importProfile: async (input, name) => {
    const profile = await ipc.importProfile(input, name);
    set({ profiles: [...get().profiles, profile] });
  },

  setActiveProfile: async (id) => {
    await get().updateSettings({ activeProfileId: id });
  },

  connect: async () => {
    const id = get().settings?.activeProfileId;
    if (!id) return;
    set({ connection: { state: "connecting" }, traffic: null });
    try {
      const state = await ipc.connect(id);
      set({ connection: state });
    } catch (e) {
      set({ connection: { state: "error", message: String(e) } });
    }
  },

  disconnect: async () => {
    try {
      const state = await ipc.disconnect();
      set({ connection: state });
    } catch (e) {
      console.error("disconnect failed:", e);
    }
  },

  relaunchElevated: async () => {
    try {
      await ipc.relaunchElevated();
    } catch (e) {
      console.error("relaunch elevated failed:", e);
    }
  },

  // после операций с подпиской обновляем и подписки, и профили
  addSubscription: async (name, url) => {
    await ipc.addSubscription(name, url);
    const [subscriptions, profiles] = await Promise.all([
      ipc.listSubscriptions(),
      ipc.listProfiles(),
    ]);
    set({ subscriptions, profiles });
  },

  updateSubscription: async (id) => {
    await ipc.updateSubscription(id);
    const [subscriptions, profiles] = await Promise.all([
      ipc.listSubscriptions(),
      ipc.listProfiles(),
    ]);
    set({ subscriptions, profiles });
  },

  deleteSubscription: async (id) => {
    await ipc.deleteSubscription(id);
    const [subscriptions, profiles] = await Promise.all([
      ipc.listSubscriptions(),
      ipc.listProfiles(),
    ]);
    set({ subscriptions, profiles });
  },
}));
