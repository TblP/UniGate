import { useEffect, useState } from "react";
import { disable, enable, isEnabled } from "@tauri-apps/plugin-autostart";
import { useAppStore } from "../../store/useAppStore";
import { isMacOS, isWindows } from "../../lib/platform";
import { ipc } from "../../lib/ipc";
import type { Mode, Theme, TunStack } from "../../lib/types";

const THEME_LABEL: Record<Theme, string> = {
  system: "Системная",
  light: "Светлая",
  dark: "Тёмная",
};

export function SettingsPanel() {
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

  if (!settings) return <section className="card">Загрузка…</section>;

  return (
    <section className="card">
      <h2>Настройки</h2>
      <div className="form-grid">
        <label>
          Режим
          <select
            value={settings.mode}
            onChange={(e) => updateSettings({ mode: e.target.value as Mode })}
          >
            <option value="proxy">Прокси (без прав)</option>
            <option value="tun">TUN — весь трафик ОС</option>
          </select>
        </label>

        {settings.mode === "tun" &&
          (isMacOS ? (
            <div className="warn">
              ℹ️ TUN-режим перенаправляет весь трафик ОС. При подключении macOS
              запросит пароль администратора (для создания VPN-туннеля).
            </div>
          ) : (
            !isElevated && (
              <div className="warn">
                ⚠️ TUN-режим перенаправляет весь трафик ОС и требует прав
                администратора.
                <button className="btn warn-action" onClick={relaunchElevated}>
                  Перезапустить от администратора
                </button>
              </div>
            )
          ))}

        {settings.mode === "tun" && isWindows && (
          <>
            <label>
              Сетевой стек TUN
              <select
                value={settings.tunStack}
                onChange={(e) =>
                  updateSettings({ tunStack: e.target.value as TunStack })
                }
              >
                <option value="gvisor">gVisor (по умолчанию, совместимый)</option>
                <option value="system">System (быстрее, под нагрузкой)</option>
                <option value="mixed">Mixed (system TCP + gvisor UDP)</option>
              </select>
            </label>
            {settings.tunStack !== "gvisor" && (
              <div className="warn">
                ℹ️ System-стек быстрее и меньше грузит CPU под высокой нагрузкой
                (игры), но рядом с виртуальными адаптерами (VirtualBox, Docker,
                Hyper-V) может не поймать часть трафика. Если после подключения
                пропал интернет — верните gVisor. Стек применяется при следующем
                подключении.
              </div>
            )}
          </>
        )}

        <label>
          Тема
          <select
            value={settings.theme}
            onChange={(e) => updateSettings({ theme: e.target.value as Theme })}
          >
            {(["system", "light", "dark"] as Theme[]).map((t) => (
              <option key={t} value={t}>
                {THEME_LABEL[t]}
              </option>
            ))}
          </select>
        </label>

        <label>
          Язык
          <select
            value={settings.language}
            onChange={(e) => updateSettings({ language: e.target.value })}
          >
            <option value="ru">Русский</option>
            <option value="en">English</option>
          </select>
        </label>

        <label className="checkbox">
          <input
            type="checkbox"
            checked={settings.autoConnect}
            onChange={(e) => updateSettings({ autoConnect: e.target.checked })}
          />
          Автоподключение при старте
        </label>

        <label className="checkbox">
          <input
            type="checkbox"
            checked={settings.minimizeToTray}
            onChange={(e) => updateSettings({ minimizeToTray: e.target.checked })}
          />
          Закрытие окна сворачивает в трей
        </label>

        <label className="checkbox">
          <input
            type="checkbox"
            checked={autostart}
            onChange={(e) => toggleAutostart(e.target.checked)}
          />
          Запускать при старте системы (свёрнутым в трей)
        </label>

        {isWindows && (
          <>
            <label className="checkbox">
              <input
                type="checkbox"
                checked={settings.adminLaunch}
                onChange={(e) => toggleAdminLaunch(e.target.checked)}
              />
              Запускаться от администратора без запроса UAC
            </label>
            {settings.adminLaunch && !isElevated && (
              <div className="warn">
                ℹ️ Один раз подтвердите UAC — приложение создаст задачу в
                Планировщике и дальше будет запускаться с правами администратора
                автоматически (и при автозапуске, и вручную).
              </div>
            )}
          </>
        )}
      </div>
    </section>
  );
}
