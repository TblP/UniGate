import { useState } from "react";
import { useAppStore } from "../../store/useAppStore";
import { formatBytes, formatSpeed } from "../../lib/format";
import { ipc } from "../../lib/ipc";
import type { ConnectionState, Outbound, Profile } from "../../lib/types";
import { ProfileForm } from "../profiles/ProfileForm";
import { VpnImportDialog } from "../profiles/VpnImportDialog";
import { displayError, useI18n } from "../../lib/i18n";
import { ProfileQr } from "../../components/ProfileQr";
import { ProfileEditorModal } from "../../components/ProfileEditorModal";

type ShareFormat = "link" | "json";

function shareFormats(outbound: Outbound): ShareFormat[] {
  if (outbound.type === "amnezia_wg") return ["link"];
  return outbound.type === "socks" || outbound.type === "http" ? ["json"] : ["link", "json"];
}

function serverAddress(profile: Profile): string {
  return `${profile.outbound.server}:${profile.outbound.port}`;
}

const FORMAT_LABEL: Record<ShareFormat, string> = { link: "connection.link", json: "JSON" };

const STATUS_LABEL: Record<ConnectionState["state"], string> = {
  disconnected: "status.disconnected",
  connecting: "status.connecting",
  connected: "status.connected",
  disconnecting: "status.disconnecting",
  error: "status.error",
};

export function ConnectionPanel() {
  const { language, t } = useI18n();
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
  const errorMsg = connection.state === "error" ? displayError(language, connection.message, "errors.connection") : null;
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
      setShared({ id, name, formats, format, content: displayError(language, e, "errors.export") });
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
        <label htmlFor="active-profile">{t("common.server")}</label>
        <select
          id="active-profile"
          value={activeId ?? ""}
          disabled={isBusy || isConnected}
          onChange={(event) => void setActiveProfile(event.target.value || null)}
        >
          <option value="">{t("connection.noServer")}</option>
          {profiles.map((profile) => (
            <option key={profile.id} value={profile.id}>
              {profile.name} · {serverAddress(profile)}
            </option>
          ))}
        </select>
        <div className="profile-picker-actions">
          <button onClick={() => setEditor("import")}>+ VPN</button>
          <button onClick={() => setEditor("create")}>+ {t("vpn.proxy")}</button>
        </div>
      </section>

      {editor && (
        <ProfileEditorModal title={editor === "import" ? "+ VPN" : `+ ${t("vpn.proxy")}`} onClose={() => setEditor(null)}>
          {editor === "import" ? (
            <VpnImportDialog onDone={() => setEditor(null)} onCancel={() => setEditor(null)} />
          ) : (
            <ProfileForm onSubmit={handleCreate} onCancel={() => setEditor(null)} />
          )}
        </ProfileEditorModal>
      )}

      {shared && (
        <div className="share-box connection-share">
          <div className="share-head">
            <span>{t("connection.shareName", { name: shared.name })}</span>
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
                  {t(FORMAT_LABEL[format])}
                </button>
              ))}
            </div>
          )}
          <textarea className="import-input" readOnly rows={shared.format === "json" ? 8 : 3} value={shared.content} />
          {shared.format === "link" && <ProfileQr key={shared.id} value={shared.content} t={t} />}
          <button type="button" onClick={() => navigator.clipboard.writeText(shared.content)}>
            {t("common.copy")}
          </button>
        </div>
      )}

      <section className="core-card">
      <h2>{t("nav.connection")}</h2>
      {!active ? (
        <p className="hint">{t("connection.pickHint")}</p>
      ) : (
        <div className="conn-layout">
          <div className="conn-info">
            <p className="conn-profile">{active.name}</p>
            <p className="conn-address">{serverAddress(active)}</p>
            <div className="conn-profile-actions">
              <button onClick={() => share(active)}>{t("common.share")}</button>
              <button
                className="danger"
                disabled={isBusy || isConnected}
                onClick={() => void deleteProfile(active.id)}
              >
                {t("common.delete")}
              </button>
            </div>
            <p className={`conn-status ${state}`}>
              {t(STATUS_LABEL[state])}
              {errorMsg ? `: ${errorMsg}` : ""}
            </p>

            {isConnected ? (
              <button className="big" onClick={disconnect}>
                {t("connection.disconnect")}
              </button>
            ) : (
              <button className="big primary" onClick={connect} disabled={isBusy}>
                {state === "connecting" ? t("status.connecting") : t("connection.connect")}
              </button>
            )}

            {isConnected && !isAwgLegacy && (
              <div className="traffic">
                <div className="traffic-speeds">
                  <span className="speed down">↓ {formatSpeed(traffic?.down ?? 0)}</span>
                  <span className="speed up">↑ {formatSpeed(traffic?.up ?? 0)}</span>
                </div>
                <div className="traffic-totals">
                  {t("connection.session")}: ↓ {formatBytes(traffic?.downTotal ?? 0)} · ↑{" "}
                  {formatBytes(traffic?.upTotal ?? 0)}
                </div>
              </div>
            )}

            {isConnected && (
              <p className="hint">
                {isAwgLegacy
                  ? t("connection.legacy")
                  : mode === "tun"
                    ? t("connection.tunMode")
                    : localProxy
                      ? t("connection.localProxy", { address: localProxy })
                      : ""}
              </p>
            )}
          </div>

          <img
            className={`conn-logo${isConnected ? " on" : ""}`}
            src={isConnected ? "/logo_on.png" : "/logo_off.png"}
            alt={isConnected ? t("status.connected") : t("status.disconnected")}
          />
        </div>
      )}
      </section>
    </div>
  );
}
