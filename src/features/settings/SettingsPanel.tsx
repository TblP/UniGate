import { useEffect, useState } from "react";
import { disable, enable, isEnabled } from "@tauri-apps/plugin-autostart";
import { useAppStore } from "../../store/useAppStore";
import { isMacOS, isWindows } from "../../lib/platform";
import { ipc } from "../../lib/ipc";
import type { Mode, Theme, TunStack } from "../../lib/types";
import { useI18n } from "../../lib/i18n";

const THEME_LABEL: Record<Theme, string> = {
  system: "settings.systemTheme",
  light: "settings.lightTheme",
  dark: "settings.darkTheme",
};

export function SettingsPanel() {
  const { t } = useI18n();
  const settings = useAppStore((s) => s.settings);
  const isElevated = useAppStore((s) => s.isElevated);
  const updateSettings = useAppStore((s) => s.updateSettings);
  const relaunchElevated = useAppStore((s) => s.relaunchElevated);

  const [autostart, setAutostart] = useState(false);
  useEffect(() => {
    isEnabled().then(setAutostart).catch(() => {});
  }, []);

  const toggleAutostart = async (on: boolean) => {
    try {
      if (on) await enable();
      else await disable();
      setAutostart(on);
    } catch (e) {
      console.error("autostart toggle failed:", e);
    }
  };

  // Windows: включение из обычного процесса перезапустит приложение с одним
  // запросом UAC — дальше запуски идут через задачу Планировщика без запросов.
  const toggleAdminLaunch = async (on: boolean) => {
    await updateSettings({ adminLaunch: on });
    try {
      await ipc.applyAdminLaunch();
    } catch (e) {
      console.error("admin launch toggle failed:", e);
    }
  };

  if (!settings) return <section className="card">{t("common.loading")}</section>;

  return (
    <section className="card">
      <h2>{t("nav.settings")}</h2>
      <div className="form-grid">
        <label>
          {t("common.mode")}
          <select
            value={settings.mode}
            onChange={(e) => updateSettings({ mode: e.target.value as Mode })}
          >
            <option value="proxy">{t("settings.proxy")}</option>
            <option value="tun">{t("settings.tun")}</option>
          </select>
        </label>

        {settings.mode === "tun" &&
          (isMacOS ? (
            <div className="warn">
              {t("settings.macAdmin")}
            </div>
          ) : (
            !isElevated && (
              <div className="warn">
                {t("settings.winAdmin")}
                <button className="btn warn-action" onClick={relaunchElevated}>
                  {t("settings.restartAdmin")}
                </button>
              </div>
            )
          ))}

        {settings.mode === "tun" && isWindows && (
          <>
            <label>
              {t("settings.tunStack")}
              <select
                value={settings.tunStack}
                onChange={(e) =>
                  updateSettings({ tunStack: e.target.value as TunStack })
                }
              >
                <option value="gvisor">{t("settings.gvisor")}</option>
                <option value="system">{t("settings.systemStack")}</option>
                <option value="mixed">{t("settings.mixedStack")}</option>
              </select>
            </label>
            {settings.tunStack !== "gvisor" && (
              <div className="warn">
                {t("settings.stackHint")}
              </div>
            )}
          </>
        )}

        <label>
          {t("settings.theme")}
          <select
            value={settings.theme}
            onChange={(e) => updateSettings({ theme: e.target.value as Theme })}
          >
            {(["system", "light", "dark"] as Theme[]).map((theme) => (
              <option key={theme} value={theme}>
                {t(THEME_LABEL[theme])}
              </option>
            ))}
          </select>
        </label>

        <label>
          {t("settings.language")}
          <select
            value={settings.language}
            onChange={(e) => updateSettings({ language: e.target.value })}
          >
            <option value="ru">{t("settings.russian")}</option>
            <option value="en">{t("settings.english")}</option>
          </select>
        </label>

        <label className="checkbox">
          <input
            type="checkbox"
            checked={settings.autoConnect}
            onChange={(e) => updateSettings({ autoConnect: e.target.checked })}
          />
          {t("settings.autoConnect")}
        </label>

        <label className="checkbox">
          <input
            type="checkbox"
            checked={settings.minimizeToTray}
            onChange={(e) => updateSettings({ minimizeToTray: e.target.checked })}
          />
          {t("settings.minimizeTray")}
        </label>

        <label className="checkbox">
          <input
            type="checkbox"
            checked={autostart}
            onChange={(e) => toggleAutostart(e.target.checked)}
          />
          {t("settings.autostart")}
        </label>

        {isWindows && (
          <>
            <label className="checkbox">
              <input
                type="checkbox"
                checked={settings.adminLaunch}
                onChange={(e) => toggleAdminLaunch(e.target.checked)}
              />
              {t("settings.adminLaunch")}
            </label>
            {settings.adminLaunch && !isElevated && (
              <div className="warn">
                {t("settings.adminLaunchHint")}
              </div>
            )}
          </>
        )}
      </div>
    </section>
  );
}
