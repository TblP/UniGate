import { useState } from "react";
import { useAppStore } from "../../store/useAppStore";
import { ipc } from "../../lib/ipc";
import type { Outbound, Profile } from "../../lib/types";
import { ProfileForm } from "./ProfileForm";
import { ImportDialog } from "./ImportDialog";
import { displayError, useI18n } from "../../lib/i18n";

function outboundSummary(o: Outbound): string {
  const addr = `${o.server}:${o.port}`;
  switch (o.type) {
    case "socks":
      return `SOCKS5 · ${addr}`;
    case "http":
      return `HTTP${o.tls ? "S" : ""} · ${addr}`;
    case "hysteria2":
      return `Hysteria2 · ${addr}`;
    case "shadowsocks":
      return `Shadowsocks · ${addr}`;
    case "trojan":
      return `Trojan · ${addr}`;
    case "vless":
      return `VLESS · ${addr}`;
    case "vmess":
      return `VMess · ${addr}`;
    case "tuic":
      return `TUIC · ${addr}`;
    case "amnezia_wg":
      return `AmneziaWG · ${addr}`;
  }
}

/** У профиля заданы учётные данные (логин/пароль/uuid/ключ). */
function hasAuth(o: Outbound): boolean {
  switch (o.type) {
    case "socks":
    case "http":
      return !!o.username || !!o.password;
    case "vless":
    case "vmess":
      return !!o.uuid;
    case "amnezia_wg":
      return true;
    default:
      return !!o.password;
  }
}

/** Протоколы, которые умеет ручная форма (остальные — только импорт). */
function isFormEditable(o: Outbound): boolean {
  return o.type === "socks" || o.type === "http" || o.type === "hysteria2";
}

type ShareFormat = "link" | "json";

/** Доступные форматы шаринга для протокола. У socks/http нет стандартной
 *  share-ссылки, у AmneziaWG — только .conf (отдаётся веткой json). */
function shareFormats(o: Outbound): ShareFormat[] {
  switch (o.type) {
    case "socks":
    case "http":
    case "amnezia_wg":
      return ["json"];
    default:
      return ["link", "json"];
  }
}

const FORMAT_LABEL: Record<ShareFormat, string> = { link: "connection.link", json: "JSON" };

type Editor =
  | { mode: "create" }
  | { mode: "edit"; profile: Profile }
  | { mode: "import" }
  | null;

export function ProfilesPanel() {
  const { language, t } = useI18n();
  const profiles = useAppStore((s) => s.profiles);
  const activeId = useAppStore((s) => s.settings?.activeProfileId ?? null);
  const createProfile = useAppStore((s) => s.createProfile);
  const updateProfile = useAppStore((s) => s.updateProfile);
  const deleteProfile = useAppStore((s) => s.deleteProfile);
  const duplicateProfile = useAppStore((s) => s.duplicateProfile);
  const setActiveProfile = useAppStore((s) => s.setActiveProfile);

  const [editor, setEditor] = useState<Editor>(null);
  const [shared, setShared] = useState<{
    id: string;
    name: string;
    formats: ShareFormat[];
    format: ShareFormat;
    content: string;
  } | null>(null);

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

  const share = (p: Profile) => {
    const formats = shareFormats(p.outbound);
    void loadShare(p.id, p.name, formats, formats[0]);
  };

  const handleSubmit = async (name: string, outbound: Outbound) => {
    if (editor?.mode === "edit") {
      await updateProfile({ ...editor.profile, name, outbound });
    } else {
      await createProfile(name, outbound);
    }
    setEditor(null);
  };

  return (
    <section className="core-card">
      <div className="panel-head">
        <h2>{t("profiles.title")}</h2>
        {!editor && (
          <div className="panel-head-actions">
            <button onClick={() => setEditor({ mode: "import" })}>{t("common.import")}</button>
            <button onClick={() => setEditor({ mode: "create" })}>+ {t("common.add")}</button>
          </div>
        )}
      </div>

      {shared && (
        <div className="share-box">
          <div className="share-head">
            <span>{t("connection.shareName", { name: shared.name })}</span>
            <button onClick={() => setShared(null)}>✕</button>
          </div>
          {shared.formats.length > 1 && (
            <div className="share-formats">
              {shared.formats.map((f) => (
                <button
                  key={f}
                  type="button"
                  className={f === shared.format ? "active" : ""}
                  onClick={() =>
                    void loadShare(shared.id, shared.name, shared.formats, f)
                  }
                >
                  {t(FORMAT_LABEL[f])}
                </button>
              ))}
            </div>
          )}
          <textarea
            className="import-input"
            readOnly
            rows={shared.format === "json" ? 8 : 3}
            value={shared.content}
          />
          <button
            type="button"
            onClick={() => navigator.clipboard.writeText(shared.content)}
          >
            {t("common.copy")}
          </button>
        </div>
      )}

      {editor?.mode === "import" ? (
        <ImportDialog onDone={() => setEditor(null)} onCancel={() => setEditor(null)} />
      ) : editor ? (
        <ProfileForm
          initial={editor.mode === "edit" ? editor.profile : undefined}
          onSubmit={handleSubmit}
          onCancel={() => setEditor(null)}
        />
      ) : profiles.length === 0 ? (
        <p className="hint">{t("profiles.empty")}</p>
      ) : (
        <ul className="profile-list">
          {profiles.map((p) => (
            <li key={p.id} className={p.id === activeId ? "profile active" : "profile"}>
              <div className="profile-info">
                <span className="profile-name">
                  {p.name}
                  {p.id === activeId && <span className="badge">{t("profiles.active")}</span>}
                </span>
                <span className="profile-sub">
                  {outboundSummary(p.outbound)}
                  {hasAuth(p.outbound) && (
                    <span className="auth-badge" title={t("connection.authProfile")}>
                      🔑
                    </span>
                  )}
                </span>
              </div>
              <div className="profile-actions">
                {p.id === activeId ? (
                  <button onClick={() => setActiveProfile(null)}>{t("profiles.deselect")}</button>
                ) : (
                  <button onClick={() => setActiveProfile(p.id)}>{t("profiles.select")}</button>
                )}
                {isFormEditable(p.outbound) && (
                  <button onClick={() => setEditor({ mode: "edit", profile: p })}>
                    {t("profiles.edit")}
                  </button>
                )}
                <button onClick={() => duplicateProfile(p.id)}>{t("profiles.duplicate")}</button>
                <button onClick={() => share(p)}>{t("common.share")}</button>
                <button className="danger" onClick={() => deleteProfile(p.id)}>
                  {t("common.delete")}
                </button>
              </div>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
