import { useEffect, useMemo, useState } from "react";
import { useAppStore } from "./store/useAppStore";
import { TitleBar } from "./components/TitleBar";
import { ConnectionPanel } from "./features/connection/ConnectionPanel";
import { SubscriptionsPanel } from "./features/subscriptions/SubscriptionsPanel";
import { RoutingPanel } from "./features/routing/RoutingPanel";
import { SettingsPanel } from "./features/settings/SettingsPanel";
import type { Theme } from "./lib/types";
import "./App.css";

type TabId = "connection" | "subscriptions" | "routing" | "settings";

const NAV: { id: TabId; label: string; icon: string }[] = [
  { id: "connection", label: "Подключение", icon: "⚡" },
  // Вкладка «Подписки» скрыта (hidden) — панель остаётся в коде, но не в навигации.
  // { id: "subscriptions", label: "Подписки", icon: "🔗" },
  { id: "routing", label: "Маршрутизация", icon: "🧭" },
  { id: "settings", label: "Настройки", icon: "⚙️" },
];

function applyTheme(theme: Theme) {
  const resolved =
    theme === "system"
      ? window.matchMedia("(prefers-color-scheme: dark)").matches
        ? "dark"
        : "light"
      : theme;
  document.documentElement.dataset.theme = resolved;
}

function App() {
  const coreVersion = useAppStore((s) => s.coreVersion);
  const coreError = useAppStore((s) => s.coreError);
  const theme = useAppStore((s) => s.settings?.theme ?? "system");
  const connection = useAppStore((s) => s.connection);
  const init = useAppStore((s) => s.init);

  const [tab, setTab] = useState<TabId>("connection");

  useEffect(() => {
    init();
  }, [init]);

  useEffect(() => {
    applyTheme(theme);
    if (theme !== "system") return;
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const handler = () => applyTheme("system");
    mq.addEventListener("change", handler);
    return () => mq.removeEventListener("change", handler);
  }, [theme]);

  const statusDot = useMemo(() => {
    switch (connection.state) {
      case "connected":
        return "dot ok";
      case "connecting":
      case "disconnecting":
        return "dot busy";
      case "error":
        return "dot err";
      default:
        return "dot";
    }
  }, [connection.state]);

  return (
    <div className="window-root">
      <TitleBar />
      <div className="app">
      <aside className="sidebar">
        <div className="brand">
          <img className="brand-logo" src="/logo.png" alt="UniGate" />
          <span className="brand-name">UniGate</span>
        </div>

        <nav className="nav">
          {NAV.map((n) => (
            <button
              key={n.id}
              className={`nav-item${tab === n.id ? " active" : ""}`}
              onClick={() => setTab(n.id)}
            >
              <span className="nav-icon">{n.icon}</span>
              {n.label}
              {n.id === "connection" && <span className={statusDot} />}
            </button>
          ))}
        </nav>

        <div className="sidebar-foot">
          {coreError ? (
            <span className="core-badge err" title={coreError}>
              ❌ ядро недоступно
            </span>
          ) : (
            <span
              className="core-badge"
              title={coreVersion ? `Ядро: ${coreVersion}` : "проверка ядра…"}
            >
              v{__APP_VERSION__}
            </span>
          )}
        </div>
      </aside>

      <main className="content">
        {tab === "connection" && <ConnectionPanel />}
        {tab === "subscriptions" && <SubscriptionsPanel />}
        {tab === "routing" && <RoutingPanel />}
        {tab === "settings" && <SettingsPanel />}
      </main>
      </div>
    </div>
  );
}

export default App;
