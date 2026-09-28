import { useEffect, useMemo, useState } from "react";
import { ipc } from "../lib/ipc";
import { formatBytes, formatSpeed } from "../lib/format";
import type {
  AndroidInstalledApp,
  AppMode,
  ConnectionState,
  Outbound,
  Profile,
  Routing,
  Settings,
  Theme,
} from "../lib/types";
import { useAppStore } from "../store/useAppStore";
import { ImportDialog } from "../features/profiles/ImportDialog";
import { ProfileForm } from "../features/profiles/ProfileForm";
import { UpdateButton } from "../components/UpdateButton";
import { displayError, useI18n } from "../lib/i18n";

type MobileTab = "connection" | "routing" | "settings";
type Editor = "create" | "import" | null;
type IconName =
  | "menu"
  | "close"
  | "bolt"
  | "route"
  | "settings"
  | "server"
  | "download"
  | "plus"
  | "share"
  | "trash"
  | "shield"
  | "info"
  | "chevron";

const NAV: Array<{ id: MobileTab; icon: IconName }> = [
  { id: "connection", icon: "bolt" },
  { id: "routing", icon: "route" },
  { id: "settings", icon: "settings" },
];

const STATUS_LABEL: Record<ConnectionState["state"], string> = {
  disconnected: "status.disconnected", connecting: "status.connecting", connected: "status.connected", disconnecting: "status.disconnecting", error: "status.error",
};

const PROTOCOL_LABEL: Record<Outbound["type"], string> = {
  socks: "SOCKS5",
  http: "HTTP(S)",
  hysteria2: "Hysteria 2",
  shadowsocks: "Shadowsocks",
  trojan: "Trojan",
  vless: "VLESS",
  vmess: "VMess",
  tuic: "TUIC",
  amnezia_wg: "AmneziaWG",
};

const APP_MODE_LABEL: Record<AppMode, string> = {
  off: "routing.allApps", only: "routing.mobileOnly", except: "routing.mobileExcept",
};

const IS_ANDROID_PREVIEW =
  import.meta.env.DEV &&
  new URLSearchParams(window.location.search).get("platform") === "android";

const PREVIEW_SETTINGS: Settings = {
  theme: "dark",
  language: "ru",
  autoConnect: false,
  activeProfileId: "android-preview",
  mode: "tun",
  tunStack: "gvisor",
  routing: {
    bypassLan: true,
    vpnCompatibility: false,
    bypassRu: false,
    appMode: "off",
    onlyApps: [],
    exceptApps: [],
  },
  minimizeToTray: false,
  adminLaunch: false,
};

const PREVIEW_PROFILE: Profile = {
  id: "android-preview",
  name: "Amsterdam · Mobile",
  outbound: {
    type: "hysteria2",
    server: "nl-1.unigate.example",
    port: 443,
    password: "preview",
  },
};

function Icon({ name, size = 22 }: { name: IconName; size?: number }) {
  const paths: Record<IconName, React.ReactNode> = {
    menu: <path d="M4 7h16M4 12h16M4 17h16" />,
    close: <path d="m6 6 12 12M18 6 6 18" />,
    bolt: <path d="m13 2-8 12h7l-1 8 8-12h-7l1-8Z" />,
    route: (
      <>
        <circle cx="6" cy="18" r="2" />
        <circle cx="18" cy="6" r="2" />
        <path d="M8 18h3a3 3 0 0 0 3-3V9a3 3 0 0 1 3-3" />
      </>
    ),
    settings: (
      <>
        <circle cx="12" cy="12" r="3" />
        <path d="M19.4 15a1.7 1.7 0 0 0 .34 1.88l.06.06-2.83 2.83-.06-.06a1.7 1.7 0 0 0-1.88-.34 1.7 1.7 0 0 0-1.03 1.56V21h-4v-.09A1.7 1.7 0 0 0 9 19.36a1.7 1.7 0 0 0-1.88.34l-.06.06-2.83-2.83.06-.06A1.7 1.7 0 0 0 4.63 15a1.7 1.7 0 0 0-1.56-1.03H3v-4h.09A1.7 1.7 0 0 0 4.64 9a1.7 1.7 0 0 0-.34-1.88l-.06-.06 2.83-2.83.06.06A1.7 1.7 0 0 0 9 4.63a1.7 1.7 0 0 0 1.03-1.56V3h4v.09A1.7 1.7 0 0 0 15 4.64a1.7 1.7 0 0 0 1.88-.34l.06-.06 2.83 2.83-.06.06A1.7 1.7 0 0 0 19.37 9a1.7 1.7 0 0 0 1.56 1.03H21v4h-.09A1.7 1.7 0 0 0 19.4 15Z" />
      </>
    ),
    server: (
      <>
        <rect x="4" y="4" width="16" height="6" rx="2" />
        <rect x="4" y="14" width="16" height="6" rx="2" />
        <path d="M8 7h.01M8 17h.01" />
      </>
    ),
    download: <path d="M12 3v12m0 0 4-4m-4 4-4-4M5 21h14" />,
    plus: <path d="M12 5v14M5 12h14" />,
    share: (
      <>
        <circle cx="18" cy="5" r="2.5" />
        <circle cx="6" cy="12" r="2.5" />
        <circle cx="18" cy="19" r="2.5" />
        <path d="m8.2 10.8 7.5-4.3m-7.5 6.7 7.5 4.3" />
      </>
    ),
    trash: <path d="M4 7h16m-10 4v6m4-6v6M9 7l1-3h4l1 3m3 0-1 14H7L6 7" />,
    shield: <path d="M12 3 5 6v5c0 4.6 2.9 8 7 10 4.1-2 7-5.4 7-10V6l-7-3Zm-3 9 2 2 4-4" />,
    info: (
      <>
        <circle cx="12" cy="12" r="9" />
        <path d="M12 11v5m0-8h.01" />
      </>
    ),
    chevron: <path d="m9 6 6 6-6 6" />,
  };

  return (
    <svg
      className="mobile-icon"
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.8"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      {paths[name]}
    </svg>
  );
}

function applyTheme(theme: Theme) {
  const resolved =
    theme === "system"
      ? window.matchMedia("(prefers-color-scheme: dark)").matches
        ? "dark"
        : "light"
      : theme;
  document.documentElement.dataset.theme = resolved;
}

function serverAddress(profile: Profile) {
  return `${profile.outbound.server}:${profile.outbound.port}`;
}

function Modal({
  title,
  children,
  onClose,
}: {
  title: string;
  children: React.ReactNode;
  onClose: () => void;
}) {
  const { t } = useI18n();
  return (
    <div className="mobile-modal-backdrop" onMouseDown={onClose}>
      <section
        className="mobile-sheet"
        role="dialog"
        aria-modal="true"
        aria-label={title}
        onMouseDown={(event) => event.stopPropagation()}
      >
        <div className="mobile-sheet-handle" />
        <header className="mobile-sheet-head">
          <h2>{title}</h2>
          <button className="mobile-icon-button" onClick={onClose} aria-label={t("common.close")}>
            <Icon name="close" />
          </button>
        </header>
        <div className="mobile-sheet-content">{children}</div>
      </section>
    </div>
  );
}

function MobileConnection() {
  const { language, t } = useI18n();
  const profiles = useAppStore((state) => state.profiles);
  const activeId = useAppStore((state) => state.settings?.activeProfileId ?? null);
  const connection = useAppStore((state) => state.connection);
  const traffic = useAppStore((state) => state.traffic);
  const setActiveProfile = useAppStore((state) => state.setActiveProfile);
  const createProfile = useAppStore((state) => state.createProfile);
  const deleteProfile = useAppStore((state) => state.deleteProfile);
  const connect = useAppStore((state) => state.connect);
  const disconnect = useAppStore((state) => state.disconnect);
  const [editor, setEditor] = useState<Editor>(null);
  const [shareText, setShareText] = useState<string | null>(null);

  const active = profiles.find((profile) => profile.id === activeId) ?? null;
  const busy = connection.state === "connecting" || connection.state === "disconnecting";
  const connected = connection.state === "connected";
  const error = connection.state === "error" ? displayError(language, connection.message, "errors.connection") : null;

  const create = async (name: string, outbound: Outbound) => {
    await createProfile(name, outbound);
    setEditor(null);
  };

  const share = async () => {
    if (!active) return;
    const format =
      active.outbound.type === "socks" || active.outbound.type === "http" ? "json" : "link";
    try {
      setShareText(await ipc.exportProfile(active.id, format));
    } catch (exportError) {
      setShareText(displayError(language, exportError, "errors.export"));
    }
  };

  return (
    <div className="mobile-page mobile-connection">
      <section className="mobile-card mobile-server-card">
        <div className="mobile-section-label">
          <Icon name="server" size={18} />
          <span>{t("common.server")}</span>
        </div>
        <label className="mobile-select-wrap">
          <select
            aria-label={t("connection.activeProfile")}
            value={activeId ?? ""}
            disabled={busy || connected}
            onChange={(event) => void setActiveProfile(event.target.value || null)}
          >
            <option value="">{t("connection.selectProfile")}</option>
            {profiles.map((profile) => (
                <option key={profile.id} value={profile.id}>
                  {profile.name} · {serverAddress(profile)}
                </option>
              ))}
          </select>
          <Icon name="chevron" size={18} />
        </label>
        <div className="mobile-row mobile-server-actions">
          <button className="mobile-secondary-button" onClick={() => setEditor("import")}>
            <Icon name="download" size={19} />
            {t("common.import")}
          </button>
          <button className="mobile-secondary-button" onClick={() => setEditor("create")}>
            <Icon name="plus" size={19} />
            {t("common.add")}
          </button>
        </div>
      </section>

      <section className={`mobile-card mobile-connect-card ${connected ? "is-connected" : ""}`}>
        <div className="mobile-profile-head">
          <div className="mobile-profile-copy">
            <span className="mobile-eyebrow">{t("connection.activeProfile")}</span>
            <h1>{active?.name ?? t("connection.noActiveServer")}</h1>
            <p>
              {active
                ? `${PROTOCOL_LABEL[active.outbound.type]} · ${serverAddress(active)}`
                : t("connection.addOrImport")}
            </p>
          </div>
          <img
            className="mobile-mascot"
            src={connected ? "/logo_on.png" : "/logo_off.png"}
            alt=""
          />
        </div>

        {active && (
          <div className="mobile-row mobile-profile-actions">
            <button className="mobile-text-button" onClick={() => void share()}>
              <Icon name="share" size={18} />
              {t("common.share")}
            </button>
            <button
              className="mobile-text-button danger"
              disabled={busy || connected}
              onClick={() => void deleteProfile(active.id)}
            >
              <Icon name="trash" size={18} />
              {t("common.delete")}
            </button>
          </div>
        )}

        <div className={`mobile-status mobile-status-${connection.state}`}>
          <span className="mobile-status-dot" />
          <span>{t(STATUS_LABEL[connection.state])}</span>
        </div>
        {error && <p className="mobile-error">{error}</p>}

        <button
          className={`mobile-connect-button ${connected ? "disconnect" : ""}`}
          disabled={!active || busy}
          onClick={() => void (connected ? disconnect() : connect())}
        >
          <Icon name="shield" />
          {connected
            ? t("connection.disconnect")
            : connection.state === "connecting"
              ? t("status.connecting")
              : t("connection.connect")}
        </button>

        <div className="mobile-traffic">
          <div>
            <span className="mobile-traffic-label">{t("connection.download")}</span>
            <strong>↓ {formatSpeed(traffic?.down ?? 0)}</strong>
            <small>{formatBytes(traffic?.downTotal ?? 0)} {t("connection.session")}</small>
          </div>
          <div>
            <span className="mobile-traffic-label">{t("connection.upload")}</span>
            <strong>↑ {formatSpeed(traffic?.up ?? 0)}</strong>
            <small>{formatBytes(traffic?.upTotal ?? 0)} {t("connection.session")}</small>
          </div>
        </div>

        <div className="mobile-info-line">
          <Icon name="info" size={18} />
          <span>{t("connection.androidTun")}</span>
        </div>
      </section>

      {editor && (
        <Modal
          title={editor === "import" ? t("connection.importProfile") : t("connection.newProfile")}
          onClose={() => setEditor(null)}
        >
          {editor === "import" ? (
            <ImportDialog onDone={() => setEditor(null)} onCancel={() => setEditor(null)} />
          ) : (
            <ProfileForm onSubmit={create} onCancel={() => setEditor(null)} />
          )}
        </Modal>
      )}

      {shareText !== null && (
        <Modal title={t("connection.shareProfile")} onClose={() => setShareText(null)}>
          <textarea className="mobile-share-text" readOnly value={shareText} rows={7} />
          <button
            className="mobile-connect-button"
            onClick={() => void navigator.clipboard.writeText(shareText)}
          >
            {t("common.copy")}
          </button>
        </Modal>
      )}
    </div>
  );
}

function MobileSwitch({
  checked,
  onChange,
  title,
  description,
}: {
  checked: boolean;
  onChange: (checked: boolean) => void;
  title: string;
  description: string;
}) {
  return (
    <label className="mobile-setting-row mobile-switch-row">
      <span>
        <strong>{title}</strong>
        <small>{description}</small>
      </span>
      <input type="checkbox" checked={checked} onChange={(event) => onChange(event.target.checked)} />
      <span className="mobile-switch" aria-hidden="true" />
    </label>
  );
}

function MobileRouting() {
  const { language, t } = useI18n();
  const settings = useAppStore((state) => state.settings);
  const updateSettings = useAppStore((state) => state.updateSettings);
  const [pickerOpen, setPickerOpen] = useState(false);
  const [installedApps, setInstalledApps] = useState<AndroidInstalledApp[]>([]);
  const [appsLoading, setAppsLoading] = useState(false);
  const [appsError, setAppsError] = useState<string | null>(null);
  const [appSearch, setAppSearch] = useState("");
  const [draftApps, setDraftApps] = useState<string[]>([]);

  if (!settings) return <div className="mobile-loading">{t("common.loading")}</div>;
  const routing = settings.routing;
  const patch = (value: Partial<Routing>) =>
    void updateSettings({ routing: { ...routing, ...value } });
  const apps = routing.appMode === "except" ? routing.exceptApps : routing.onlyApps;
  const setApps = (next: string[]) =>
    patch(routing.appMode === "except" ? { exceptApps: next } : { onlyApps: next });
  const openAppPicker = async () => {
    setPickerOpen(true);
    setDraftApps([...apps]);
    setAppSearch("");
    setAppsError(null);
    if (installedApps.length > 0) return;
    setAppsLoading(true);
    try {
      setInstalledApps(await ipc.listInstalledApps());
    } catch (error) {
      setAppsError(displayError(language, error, "errors.apps"));
    } finally {
      setAppsLoading(false);
    }
  };
  const filteredApps = installedApps.filter((app) => {
    const query = appSearch.trim().toLocaleLowerCase();
    return (
      !query ||
      app.name.toLocaleLowerCase().includes(query) ||
      app.packageName.toLocaleLowerCase().includes(query)
    );
  });
  const visibleApps = filteredApps.slice(0, 50);
  const toggleDraftApp = (packageName: string) =>
    setDraftApps((current) =>
      current.includes(packageName)
        ? current.filter((item) => item !== packageName)
        : [...current, packageName],
    );
  const appDetails = (packageName: string) =>
    installedApps.find((app) => app.packageName === packageName);

  return (
    <div className="mobile-page">
      <header className="mobile-page-heading">
        <h1>{t("nav.routing")}</h1>
        <p>{t("routing.mobileHint")}</p>
      </header>

      <section className="mobile-card mobile-settings-card">
        <MobileSwitch
          checked={routing.bypassLan}
          onChange={(checked) => patch({ bypassLan: checked })}
          title={t("routing.lanMobile")}
          description={t("routing.lanDescription")}
        />
        <MobileSwitch
          checked={routing.bypassRu}
          onChange={(checked) => patch({ bypassRu: checked })}
          title={t("routing.ruMobile")}
          description={t("routing.ruDescription")}
        />
      </section>

      <section className="mobile-card">
        <div className="mobile-section-label">
          <Icon name="route" size={18} />
          <span>{t("routing.apps")}</span>
        </div>
        <label className="mobile-field">
          <span>{t("common.mode")}</span>
          <div className="mobile-select-wrap">
            <select
              value={routing.appMode}
              onChange={(event) => patch({ appMode: event.target.value as AppMode })}
            >
              {(["off", "only", "except"] as AppMode[]).map((mode) => (
                <option key={mode} value={mode}>
                  {t(APP_MODE_LABEL[mode])}
                </option>
              ))}
            </select>
            <Icon name="chevron" size={18} />
          </div>
        </label>

        {routing.appMode !== "off" && (
          <div className="mobile-apps">
            <p className="mobile-field-hint">
              {routing.appMode === "only"
                ? t("routing.mobileOnlyHint")
                : t("routing.mobileExceptHint")}
            </p>
            <button className="mobile-app-picker-button" onClick={() => void openAppPicker()}>
              <span className="mobile-app-picker-plus">
                <Icon name="plus" />
              </span>
              <span>
                <strong>{t("routing.selectApps")}</strong>
                <small>{apps.length ? t("routing.selected", { count: apps.length }) : t("routing.openApps")}</small>
              </span>
              <Icon name="chevron" size={18} />
            </button>
            {routing.appMode === "only" && apps.length === 0 && (
              <p className="mobile-app-warning">{t("routing.selectOne")}</p>
            )}
            {apps.length > 0 && (
              <div className="mobile-selected-apps">
                {apps.map((packageName) => {
                  const details = appDetails(packageName);
                  return (
                    <div className="mobile-selected-app" key={packageName}>
                      {details?.icon ? (
                        <img src={details.icon} alt="" />
                      ) : (
                        <span className="mobile-app-fallback">
                          {(details?.name ?? packageName).slice(0, 1).toLocaleUpperCase()}
                        </span>
                      )}
                      <span>
                        <strong>{details?.name ?? packageName}</strong>
                        {details && <small>{packageName}</small>}
                      </span>
                      <button
                        onClick={() => setApps(apps.filter((item) => item !== packageName))}
                        aria-label={t("routing.removeApp", { name: details?.name ?? packageName })}
                      >
                        <Icon name="close" size={18} />
                      </button>
                    </div>
                  );
                })}
              </div>
            )}
          </div>
        )}
      </section>

      <div className="mobile-note">
        <Icon name="info" size={20} />
        <span>{t("routing.nextConnection")}</span>
      </div>

      {pickerOpen && (
        <Modal title={t("routing.appPicker")} onClose={() => setPickerOpen(false)}>
          <div className="mobile-app-picker">
            <div className="mobile-app-search">
              <input
                value={appSearch}
                onChange={(event) => setAppSearch(event.target.value)}
                placeholder={t("routing.search")}
                autoFocus
              />
            </div>
            {appsLoading && <div className="mobile-loading">{t("routing.loadingApps")}</div>}
            {appsError && <div className="mobile-error">{appsError}</div>}
            {!appsLoading && !appsError && filteredApps.length === 0 && (
              <div className="mobile-loading">
                {appSearch ? t("routing.nothingFound") : t("routing.noApps")}
              </div>
            )}
            <div className="mobile-app-list">
              {visibleApps.map((app) => {
                const selected = draftApps.includes(app.packageName);
                return (
                  <button
                    type="button"
                    className={`mobile-app-choice ${selected ? "selected" : ""}`}
                    key={app.packageName}
                    onClick={() => toggleDraftApp(app.packageName)}
                  >
                    {app.icon ? (
                      <img src={app.icon} alt="" />
                    ) : (
                      <span className="mobile-app-fallback">
                        {app.name.slice(0, 1).toLocaleUpperCase()}
                      </span>
                    )}
                    <span className="mobile-app-copy">
                      <strong>{app.name}</strong>
                      <small>{app.packageName}</small>
                    </span>
                    <span className="mobile-app-check">{selected ? "✓" : ""}</span>
                  </button>
                );
              })}
            </div>
            {filteredApps.length > visibleApps.length && (
              <p className="mobile-app-list-hint">
                {t("routing.firstApps", { count: visibleApps.length })}
              </p>
            )}
            <button
              className="mobile-connect-button mobile-app-picker-done"
              onClick={() => {
                setApps(draftApps);
                setPickerOpen(false);
              }}
              disabled={routing.appMode === "only" && draftApps.length === 0}
            >
              {t("routing.done")}{draftApps.length ? ` · ${draftApps.length}` : ""}
            </button>
          </div>
        </Modal>
      )}
    </div>
  );
}

function MobileSettings() {
  const { language, t } = useI18n();
  const settings = useAppStore((state) => state.settings);
  const coreVersion = useAppStore((state) => state.coreVersion);
  const coreError = useAppStore((state) => state.coreError);
  const updateSettings = useAppStore((state) => state.updateSettings);
  const [integrationMessage, setIntegrationMessage] = useState<string | null>(null);

  if (!settings) return <div className="mobile-loading">{t("common.loading")}</div>;

  const requestWidget = async () => {
    setIntegrationMessage(null);
    try {
      const requested = await ipc.requestAndroidWidget();
      setIntegrationMessage(
        requested
          ? t("settings.widgetRequested")
          : t("settings.widgetManual"),
      );
    } catch (error) {
      setIntegrationMessage(displayError(language, error, "errors.integration"));
    }
  };

  const requestQuickTile = async () => {
    setIntegrationMessage(null);
    try {
      const added = await ipc.requestAndroidQuickTile();
      setIntegrationMessage(
        added
          ? t("settings.tileAdded")
          : t("settings.tileManual"),
      );
    } catch (error) {
      setIntegrationMessage(displayError(language, error, "errors.integration"));
    }
  };

  return (
    <div className="mobile-page">
      <header className="mobile-page-heading">
        <h1>{t("nav.settings")}</h1>
        <p>{t("settings.androidDescription")}</p>
      </header>

      <section className="mobile-card mobile-settings-card">
        <label className="mobile-setting-row">
          <span>
            <strong>{t("settings.theme")}</strong>
            <small>{t("settings.appearance")}</small>
          </span>
          <select
            value={settings.theme}
            onChange={(event) => void updateSettings({ theme: event.target.value as Theme })}
          >
            <option value="system">{t("settings.systemTheme")}</option>
            <option value="dark">{t("settings.darkTheme")}</option>
            <option value="light">{t("settings.lightTheme")}</option>
          </select>
        </label>
        <label className="mobile-setting-row">
          <span>
            <strong>{t("settings.language")}</strong>
            <small>{t("settings.appLanguage")}</small>
          </span>
          <select
            value={settings.language}
            onChange={(event) => void updateSettings({ language: event.target.value })}
          >
            <option value="ru">{t("settings.russian")}</option>
            <option value="en">{t("settings.english")}</option>
          </select>
        </label>
        <MobileSwitch
          checked={settings.autoConnect}
          onChange={(checked) => void updateSettings({ autoConnect: checked })}
          title={t("settings.autoConnectMobile")}
          description={t("settings.autoConnectDescription")}
        />
      </section>

      <section className="mobile-card">
        <div className="mobile-section-label">
          <Icon name="bolt" size={18} />
          <span>{t("settings.androidIntegration")}</span>
        </div>
        <div className="mobile-native-feature">
          <span>
            <strong>{t("settings.homeWidget")}</strong>
            <small>{t("settings.widgetDescription")}</small>
          </span>
          <button onClick={() => void requestWidget()}>{t("common.add")}</button>
        </div>
        <div className="mobile-native-feature">
          <span>
            <strong>{t("settings.quickTile")}</strong>
            <small>{t("settings.quickTileDescription")}</small>
          </span>
          <button onClick={() => void requestQuickTile()}>{t("common.add")}</button>
        </div>
        {integrationMessage && (
          <p className="mobile-native-message">{integrationMessage}</p>
        )}
      </section>

      <section className="mobile-card mobile-about-card">
        <div className="mobile-about-logo">
          <img src="/logo.png" alt="" />
          <div>
            <strong>{t("settings.androidName")}</strong>
            <span>{t("common.version", { version: __APP_VERSION__ })}</span>
          </div>
        </div>
        <div className="mobile-about-row">
          <span>{t("settings.vpnCore")}</span>
          <strong>{coreError ? t("common.unavailable") : coreVersion ?? t("common.checking")}</strong>
        </div>
        <div className="mobile-about-row">
          <span>{t("common.mode")}</span>
          <strong>TUN · gVisor</strong>
        </div>
      </section>

      <div className="mobile-note">
        <Icon name="shield" size={20} />
        <span>
          {t("settings.androidPermission")}
        </span>
      </div>
    </div>
  );
}

export function AndroidApp() {
  const { language, t } = useI18n();
  const init = useAppStore((state) => state.init);
  const theme = useAppStore((state) => state.settings?.theme ?? "system");
  const connection = useAppStore((state) => state.connection);
  const [tab, setTab] = useState<MobileTab>("connection");
  const [drawerOpen, setDrawerOpen] = useState(false);

  useEffect(() => {
    document.documentElement.dataset.platform = "android";
    if (IS_ANDROID_PREVIEW && !useAppStore.getState().settings) {
      useAppStore.setState({
        settings: PREVIEW_SETTINGS,
        profiles: [PREVIEW_PROFILE],
      });
    } else {
      void init();
    }
    return () => {
      delete document.documentElement.dataset.platform;
    };
  }, [init]);

  useEffect(() => {
    applyTheme(theme);
    if (theme !== "system") return;
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const listener = () => applyTheme("system");
    media.addEventListener("change", listener);
    return () => media.removeEventListener("change", listener);
  }, [theme]);

  useEffect(() => {
    document.documentElement.lang = language;
  }, [language]);

  useEffect(() => {
    const syncNativeState = async () => {
      if (document.visibilityState !== "visible" || IS_ANDROID_PREVIEW) return;
      try {
        const next = await ipc.getConnectionState();
        useAppStore.setState({
          connection: next,
          ...(next.state === "connected" ? {} : { traffic: null }),
        });
      } catch {
        // Основная инициализация уже показывает ошибки; resume-sync не мешает UI.
      }
    };
    const onVisibility = () => void syncNativeState();
    document.addEventListener("visibilitychange", onVisibility);
    window.addEventListener("focus", onVisibility);
    return () => {
      document.removeEventListener("visibilitychange", onVisibility);
      window.removeEventListener("focus", onVisibility);
    };
  }, []);

  const currentTitle = useMemo(
    () => t(`nav.${tab}`),
    [tab, language],
  );

  const navigate = (next: MobileTab) => {
    setTab(next);
    setDrawerOpen(false);
  };

  return (
    <div className="android-root">
      <header className="mobile-appbar">
        <button
          className="mobile-icon-button"
          onClick={() => setDrawerOpen(true)}
          aria-label={t("mobile.openMenu")}
        >
          <Icon name="menu" />
        </button>
        <div className="mobile-appbar-title">
          <strong>UniGate</strong>
          <span>{currentTitle}</span>
        </div>
        <span className={`mobile-appbar-status ${connection.state}`} />
      </header>

      <main className="mobile-content">
        {tab === "connection" && <MobileConnection />}
        {tab === "routing" && <MobileRouting />}
        {tab === "settings" && <MobileSettings />}
      </main>

      <div
        className={`mobile-drawer-backdrop ${drawerOpen ? "open" : ""}`}
        onClick={() => setDrawerOpen(false)}
      />
      <aside className={`mobile-drawer ${drawerOpen ? "open" : ""}`} aria-hidden={!drawerOpen}>
        <div className="mobile-drawer-brand">
          <img src="/logo.png" alt="" />
          <div>
            <strong>UniGate</strong>
            <span>{t("mobile.vpnForAndroid")}</span>
          </div>
          <button
            className="mobile-icon-button"
            onClick={() => setDrawerOpen(false)}
            aria-label={t("mobile.closeMenu")}
          >
            <Icon name="close" />
          </button>
        </div>
        <nav className="mobile-nav">
          {NAV.map((item) => (
            <button
              key={item.id}
              className={tab === item.id ? "active" : ""}
              onClick={() => navigate(item.id)}
            >
              <Icon name={item.icon} />
              <span>{t(`nav.${item.id}`)}</span>
              {item.id === "connection" && (
                <span className={`mobile-nav-dot ${connection.state}`} />
              )}
            </button>
          ))}
        </nav>
        <footer>
          <UpdateButton />
          <span>UniGate {__APP_VERSION__}</span>
          <span>Android · ARM64</span>
        </footer>
      </aside>
    </div>
  );
}
