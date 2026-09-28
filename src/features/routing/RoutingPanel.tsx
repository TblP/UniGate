import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { useAppStore } from "../../store/useAppStore";
import { isMacOS, isWindows } from "../../lib/platform";
import type { AppMode, Routing } from "../../lib/types";
import { useI18n } from "../../lib/i18n";

const APP_MODE_LABEL: Record<AppMode, string> = {
  off: "routing.off", only: "routing.only", except: "routing.except",
};

export function RoutingPanel() {
  const { t } = useI18n();
  const settings = useAppStore((s) => s.settings);
  const updateSettings = useAppStore((s) => s.updateSettings);
  const awgShim = useAppStore((s) => s.awgShim);
  const [appInput, setAppInput] = useState("");

  if (!settings) return null;
  const routing = settings.routing;
  const isTun = settings.mode === "tun";

  const patch = (p: Partial<Routing>) =>
    updateSettings({ routing: { ...routing, ...p } });

  // у каждого режима свой список приложений
  const currentApps =
    routing.appMode === "except" ? routing.exceptApps : routing.onlyApps;
  const setCurrentApps = (apps: string[]) =>
    patch(routing.appMode === "except" ? { exceptApps: apps } : { onlyApps: apps });

  const addAppName = (name: string) => {
    const n = name.trim();
    if (!n || currentApps.includes(n)) return;
    setCurrentApps([...currentApps, n]);
  };

  const addApp = () => {
    addAppName(appInput);
    setAppInput("");
  };

  const pickApp = async () => {
    const selected = await open({
      multiple: false,
      filters: [
        isMacOS
          ? { name: t("routing.programs"), extensions: ["app"] }
          : { name: t("routing.programs"), extensions: ["exe"] },
      ],
    });
    if (typeof selected === "string") {
      let name = selected.split(/[\\/]/).pop() ?? "";
      // macOS: из бандла Foo.app берём имя процесса Foo
      if (isMacOS) name = name.replace(/\.app$/i, "");
      if (name) addAppName(name);
    }
  };

  return (
    <section className="core-card">
      <h2>{t("routing.title")}</h2>
      {!isTun && (
        <p className="hint">{t("routing.tunOnly")}</p>
      )}

      <div className="settings">
        <label className="checkbox">
          <input
            type="checkbox"
            checked={routing.bypassLan}
            onChange={(e) =>
              patch(
                e.target.checked
                  ? { bypassLan: true }
                  : { bypassLan: false, vpnCompatibility: false }
              )
            }
          />
          {t("routing.lan")}
        </label>

        {isWindows && (
          <>
            <label className="checkbox">
              <input
                type="checkbox"
                checked={routing.vpnCompatibility}
                disabled={!routing.bypassLan}
                onChange={(e) => patch({ vpnCompatibility: e.target.checked })}
              />
              {t("routing.vpnCompatibility")}
            </label>
            <p className="hint">
              {t("routing.vpnCompatibilityHint")}
            </p>
          </>
        )}

        <label className="checkbox">
          <input
            type="checkbox"
            checked={routing.bypassRu}
            onChange={(e) => patch({ bypassRu: e.target.checked })}
          />
          {t("routing.ru")}
        </label>

        <label>
          {t("routing.apps")}
          <select
            value={routing.appMode}
            onChange={(e) => patch({ appMode: e.target.value as AppMode })}
          >
            {(["off", "only", "except"] as AppMode[]).map((m) => (
              <option key={m} value={m}>
                {t(APP_MODE_LABEL[m])}
              </option>
            ))}
          </select>
        </label>

        {routing.appMode !== "off" && (
          <div className="apps">
            <div className="app-add">
              <button type="button" onClick={pickApp}>
                {isMacOS ? t("routing.pickApp") : t("routing.pickExe")}
              </button>
              <input
                value={appInput}
                onChange={(e) => setAppInput(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") {
                    e.preventDefault();
                    addApp();
                  }
                }}
                placeholder={t("routing.processPlaceholder")}
              />
              <button type="button" onClick={addApp}>
                +
              </button>
            </div>
            {currentApps.length === 0 ? (
              <p className="hint">
                {routing.appMode === "only"
                  ? t("routing.onlyHint")
                  : t("routing.exceptHint")}
              </p>
            ) : (
              <ul className="app-list">
                {currentApps.map((a) => (
                  <li key={a}>
                    <span>{a}</span>
                    <button
                      type="button"
                      className="danger"
                      onClick={() => setCurrentApps(currentApps.filter((x) => x !== a))}
                    >
                      ✕
                    </button>
                  </li>
                ))}
              </ul>
            )}
          </div>
        )}

        {!awgShim && (
          <p className="hint">
            {t("routing.awgLegacy")}
          </p>
        )}
      </div>
    </section>
  );
}
