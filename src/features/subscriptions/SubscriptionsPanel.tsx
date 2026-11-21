import { useState } from "react";
import { useAppStore } from "../../store/useAppStore";
import { displayError, useI18n } from "../../lib/i18n";

function formatTime(unix: number | undefined, language: string): string {
  if (!unix) return "—";
  return new Date(unix * 1000).toLocaleString(language === "en" ? "en-US" : "ru-RU");
}

export function SubscriptionsPanel({ embedded = false, onClose, scanQr }: {
  embedded?: boolean; onClose?: () => void; scanQr?: () => Promise<string | null>;
}) {
  const { language, t } = useI18n();
  const subscriptions = useAppStore((s) => s.subscriptions);
  const addSubscription = useAppStore((s) => s.addSubscription);
  const updateSubscription = useAppStore((s) => s.updateSubscription);
  const deleteSubscription = useAppStore((s) => s.deleteSubscription);

  const [name, setName] = useState("");
  const [url, setUrl] = useState("");
  const [adding, setAdding] = useState(false);
  const [busy, setBusy] = useState(false);
  const [scanning, setScanning] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [updatingId, setUpdatingId] = useState<string | null>(null);
  const locked = busy || scanning || updatingId !== null;

  const readQr = async () => {
    if (!scanQr || locked) return;
    setScanning(true);
    setError(null);
    try {
      const content = (await scanQr())?.trim();
      if (!content) return;
      let parsed: URL;
      try { parsed = new URL(content); }
      catch { setError(t("subscriptions.qrHttpsRequired")); return; }
      if (!/^https:\/\//i.test(content) || parsed.protocol !== "https:" || !parsed.hostname ||
          parsed.username || parsed.password || /\s/.test(content)) {
        setError(t("subscriptions.qrHttpsRequired"));
        return;
      }
      setUrl(content);
    } catch {
      setError(t("profiles.qrScanError"));
    } finally {
      setScanning(false);
    }
  };

  const add = async () => {
    if (locked) return;
    if (!url.trim()) return setError(t("subscriptions.requireUrl"));
    setBusy(true);
    setError(null);
    try {
      await addSubscription(name.trim() || t("subscriptions.defaultName"), url.trim());
      setName("");
      setUrl("");
      setAdding(false);
    } catch (e) {
      setError(displayError(language, e, "errors.subscription"));
    } finally {
      setBusy(false);
    }
  };

  const update = async (id: string) => {
    setUpdatingId(id);
    setError(null);
    try {
      await updateSubscription(id);
    } catch (e) {
      setError(displayError(language, e, "errors.subscription"));
    } finally {
      setUpdatingId(null);
    }
  };

  return (
    <section className={embedded ? "embedded-subscriptions" : "core-card"}>
      {!embedded && <div className="panel-head">
        <h2>{t("nav.subscriptions")}</h2>
        {!adding && <button onClick={() => setAdding(true)}>+ {t("common.add")}</button>}
      </div>}

      {(adding || embedded) && (
        <div className="profile-form">
          <label>
            {t("subscriptions.optionalName")}
            <input
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder={t("subscriptions.provider")}
            />
          </label>
          <label>
            {t("subscriptions.url")}
            <input
              type="password"
              autoComplete="off"
              value={url}
              onChange={(e) => setUrl(e.target.value)}
              placeholder="https://…/sub"
            />
          </label>
          {error && <p className="error">{error}</p>}
          <div className="profile-actions">
            {scanQr && (
              <button type="button" disabled={locked} onClick={() => void readQr()}>
                {t("profiles.scanQr")}
              </button>
            )}
            <button onClick={add} disabled={locked}>
              {busy ? t("common.loading") : t("common.add")}
            </button>
            <button
              onClick={() => {
                if (embedded) return onClose?.();
                setAdding(false);
                setError(null);
              }}
              disabled={busy || scanning}
            >
              {t("common.cancel")}
            </button>
          </div>
        </div>
      )}

      {!adding && !embedded && error && <p className="error">{error}</p>}

      {subscriptions.length === 0
        ? !adding && (
            <p className="hint">{t("subscriptions.empty")}</p>
          )
        : (
            <ul className="profile-list">
              {subscriptions.map((s) => (
                <li key={s.id} className="profile">
                  <div className="profile-info">
                    <span className="profile-name">{s.name}</span>
                    <span className="profile-sub">
                      {t("subscriptions.count", { count: s.count, date: formatTime(s.updatedAt, language) })}
                      {!!s.skipped && ` · ${t("subscriptions.skipped", { count: s.skipped })}`}
                    </span>
                  </div>
                  <div className="profile-actions">
                    <button onClick={() => update(s.id)} disabled={locked}>
                      {updatingId === s.id ? t("subscriptions.updating") : t("subscriptions.update")}
                    </button>
                    <button className="danger" disabled={locked} onClick={async () => {
                      if (!window.confirm(t("subscriptions.confirmDelete", { name: s.name }))) return;
                      setBusy(true);
                      setError(null);
                      try { await deleteSubscription(s.id); }
                      catch (e) { setError(displayError(language, e, "errors.subscription")); }
                      finally { setBusy(false); }
                    }}>
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
