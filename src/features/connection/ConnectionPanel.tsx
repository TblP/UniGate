import { useState } from "react";
import { useAppStore } from "../../store/useAppStore";
import { formatBytes, formatSpeed } from "../../lib/format";
import { ipc } from "../../lib/ipc";
import type { ConnectionState, Outbound, Profile } from "../../lib/types";
import { ProfileForm } from "../profiles/ProfileForm";
import { ImportDialog } from "../profiles/ImportDialog";

type ShareFormat = "link" | "json";

function shareFormats(outbound: Outbound): ShareFormat[] {
  return outbound.type === "socks" || outbound.type === "http" || outbound.type === "amnezia_wg"
    ? ["json"]
    : ["link", "json"];
}

function serverAddress(profile: Profile): string {
  return `${profile.outbound.server}:${profile.outbound.port}`;
}

const FORMAT_LABEL: Record<ShareFormat, string> = { link: "Ссылка", json: "JSON" };

const STATUS_LABEL: Record<ConnectionState["state"], string> = {
  disconnected: "Отключено",
  connecting: "Подключение…",
  connected: "Подключено",
  disconnecting: "Отключение…",
  error: "Ошибка",
};

export function ConnectionPanel() {
  const connection = useAppStore((s) => s.connection);
  const profiles = useAppStore((s) => s.profiles);
  const activeId = useAppStore((s) => s.settings?.activeProfileId ?? null);
  const mode = useAppStore((s) => s.settings?.mode ?? "proxy");
  const localProxy = useAppStore((s) => s.localProxy);
  const traffic = useAppStore((s) => s.traffic);
  const connect = useAppStore((s) => s.connect);
  const disconnect = useAppStore((s) => s.disconnect);
  const awgShim = useAppStore((s) => s.awgShim);
  const setActiveProfile = useAppStore((s) => s.setActiveProfile);
  const createProfile = useAppStore((s) => s.createProfile);
  const deleteProfile = useAppStore((s) => s.deleteProfile);

  const [editor, setEditor] = useState<"create" | "import" | null>(null);
  const [shared, setShared] = useState<{
    id: string;
    name: string;
    formats: ShareFormat[];
    format: ShareFormat;
    content: string;
  } | null>(null);

  const active = profiles.find((p) => p.id === activeId) ?? null;
  const state = connection.state;
  const isConnected = state === "connected";
  const isBusy = state === "connecting" || state === "disconnecting";
  const errorMsg = connection.state === "error" ? connection.message : null;
  // AmneziaWG через awg-shim ведёт себя как остальные протоколы (sing-box в
  // пути: split + статистика); legacy-движок — полный туннель без статистики
  const isAwgLegacy = active?.outbound.type === "amnezia_wg" && !awgShim;

  const loadShare = async (
    id: string,
    name: string,
    formats: ShareFormat[],
    format: ShareFormat,
  ) => {
    try {
      const content = await ipc.exportProfile(id, format);
      setShared({ id, name, formats, format, content });
    } catch (e) {
      setShared({ id, name, formats, format, content: `Ошибка экспорта: ${e}` });
    }
  };

  const share = (profile: Profile) => {
    const formats = shareFormats(profile.outbound);
    void loadShare(profile.id, profile.name, formats, formats[0]);
  };

  const handleCreate = async (name: string, outbound: Outbound) => {
    await createProfile(name, outbound);
    setEditor(null);
  };

  return (
    <div className="connection-page">
      <section className="profile-picker-card">
        <label htmlFor="active-profile">Сервер</label>
        <select
          id="active-profile"
          value={activeId ?? ""}
          disabled={isBusy || isConnected}
          onChange={(event) => void setActiveProfile(event.target.value || null)}
        >
          <option value="">Не выбран</option>
          {profiles.map((profile) => (
            <option key={profile.id} value={profile.id}>
              {profile.name} · {serverAddress(profile)}
            </option>
          ))}
        </select>
        <div className="profile-picker-actions">
          <button onClick={() => setEditor("import")}>Импорт</button>
          <button onClick={() => setEditor("create")}>+ Добавить</button>
        </div>
      </section>

      {editor === "import" && (
        <section className="core-card compact-editor">
          <ImportDialog onDone={() => setEditor(null)} onCancel={() => setEditor(null)} />
        </section>
      )}
      {editor === "create" && (
        <section className="core-card compact-editor">
          <ProfileForm onSubmit={handleCreate} onCancel={() => setEditor(null)} />
        </section>
      )}

      {shared && (
        <div className="share-box connection-share">
          <div className="share-head">
            <span>Поделиться: {shared.name}</span>
            <button onClick={() => setShared(null)}>✕</button>
          </div>
          {shared.formats.length > 1 && (
            <div className="share-formats">
              {shared.formats.map((format) => (
                <button
                  key={format}
                  type="button"
                  className={format === shared.format ? "active" : ""}
                  onClick={() => void loadShare(shared.id, shared.name, shared.formats, format)}
                >
                  {FORMAT_LABEL[format]}
                </button>
              ))}
            </div>
          )}
          <textarea className="import-input" readOnly rows={shared.format === "json" ? 8 : 3} value={shared.content} />
          <button type="button" onClick={() => navigator.clipboard.writeText(shared.content)}>
            Копировать
          </button>
        </div>
      )}

      <section className="core-card">
      <h2>Подключение</h2>
      {!active ? (
        <p className="hint">Выбери сервер в списке выше или добавь новый профиль.</p>
      ) : (
        <div className="conn-layout">
          <div className="conn-info">
            <p className="conn-profile">{active.name}</p>
            <p className="conn-address">{serverAddress(active)}</p>
            <div className="conn-profile-actions">
              <button onClick={() => share(active)}>Поделиться</button>
              <button
                className="danger"
                disabled={isBusy || isConnected}
                onClick={() => void deleteProfile(active.id)}
              >
                Удалить
              </button>
            </div>
            <p className={`conn-status ${state}`}>
              {STATUS_LABEL[state]}
              {errorMsg ? `: ${errorMsg}` : ""}
            </p>

            {isConnected ? (
              <button className="big" onClick={disconnect}>
                Отключиться
              </button>
            ) : (
              <button className="big primary" onClick={connect} disabled={isBusy}>
                {state === "connecting" ? "Подключение…" : "Подключиться"}
              </button>
            )}

            {isConnected && !isAwgLegacy && (
              <div className="traffic">
                <div className="traffic-speeds">
                  <span className="speed down">↓ {formatSpeed(traffic?.down ?? 0)}</span>
                  <span className="speed up">↑ {formatSpeed(traffic?.up ?? 0)}</span>
                </div>
                <div className="traffic-totals">
                  за сессию: ↓ {formatBytes(traffic?.downTotal ?? 0)} · ↑{" "}
                  {formatBytes(traffic?.upTotal ?? 0)}
                </div>
              </div>
            )}

            {isConnected && (
              <p className="hint">
                {isAwgLegacy
                  ? "AmneziaWG — весь трафик ОС через туннель (статистика недоступна)"
                  : mode === "tun"
                    ? "Режим: TUN — весь трафик ОС через туннель"
                    : localProxy
                      ? `Локальный прокси: ${localProxy}`
                      : ""}
              </p>
            )}
          </div>

          <img
            className={`conn-logo${isConnected ? " on" : ""}`}
            src={isConnected ? "/logo_on.png" : "/logo_off.png"}
            alt={isConnected ? "Подключено" : "Отключено"}
          />
        </div>
      )}
      </section>
    </div>
  );
}
