// Типобезопасные обёртки над Tauri-командами.
// Единая точка вызова бэкенда — компоненты не дёргают invoke напрямую.

import { invoke } from "@tauri-apps/api/core";
import type {
  ConnectionState,
  Outbound,
  Profile,
  Settings,
  Subscription,
} from "./types";

export const ipc = {
  /** Версия sidecar sing-box (первая строка вывода `sing-box version`). */
  singboxVersion: () => invoke<string>("singbox_version"),

  /** Текущие настройки (или значения по умолчанию, если файла нет). */
  getSettings: () => invoke<Settings>("get_settings"),

  /** Сохраняет настройки на диск, возвращает сохранённое значение. */
  saveSettings: (settings: Settings) =>
    invoke<Settings>("save_settings", { settings }),

  /** Все профили. */
  listProfiles: () => invoke<Profile[]>("list_profiles"),

  /** Создаёт профиль (id назначает бэкенд). */
  createProfile: (name: string, outbound: Outbound) =>
    invoke<Profile>("create_profile", { name, outbound }),

  /** Обновляет существующий профиль по id. */
  updateProfile: (profile: Profile) =>
    invoke<Profile>("update_profile", { profile }),

  /** Удаляет профиль по id. */
  deleteProfile: (id: string) => invoke<void>("delete_profile", { id }),

  /** Дублирует профиль по id. */
  duplicateProfile: (id: string) =>
    invoke<Profile>("duplicate_profile", { id }),

  /** Импортирует профиль из ссылки (hysteria2://) или JSON sing-box. */
  importProfile: (input: string, name?: string) =>
    invoke<Profile>("import_profile", { input, name: name ?? null }),

  /** Экспортирует профиль: format="link" — share-ссылка, "json" — sing-box JSON. */
  exportProfile: (id: string, format: "link" | "json") =>
    invoke<string>("export_profile", { id, format }),

  /** Все подписки. */
  listSubscriptions: () => invoke<Subscription[]>("list_subscriptions"),

  /** Добавляет подписку (скачивает список серверов → профили). */
  addSubscription: (name: string, url: string) =>
    invoke<Subscription>("add_subscription", { name, url }),

  /** Обновляет подписку (перекачивает список). */
  updateSubscription: (id: string) =>
    invoke<Subscription>("update_subscription", { id }),

  /** Удаляет подписку вместе с её профилями. */
  deleteSubscription: (id: string) =>
    invoke<void>("delete_subscription", { id }),

  /** Подключается через профиль (proxy mode). */
  connect: (profileId: string) =>
    invoke<ConnectionState>("connect", { profileId }),

  /** Отключается. */
  disconnect: () => invoke<ConnectionState>("disconnect"),

  /** Текущее состояние подключения. */
  getConnectionState: () => invoke<ConnectionState>("get_connection_state"),

  /** Адрес локального прокси (для ручной проверки). */
  localProxyAddr: () => invoke<string>("local_proxy_addr"),

  /** Запущено ли приложение с правами администратора. */
  isElevated: () => invoke<boolean>("is_elevated_cmd"),

  /** Доступен ли awg-shim (AmneziaWG через sing-box: split + статистика). */
  awgShimAvailable: () => invoke<boolean>("awg_shim_available"),

  /** Перезапустить приложение от имени администратора (UAC). */
  relaunchElevated: () => invoke<void>("relaunch_elevated_cmd"),

  /** Применить настройку «запуск от администратора» (Windows: задача Планировщика). */
  applyAdminLaunch: () => invoke<void>("apply_admin_launch"),
};
