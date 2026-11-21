import { useState } from "react";
import { useAppStore } from "../../store/useAppStore";
import { displayError, useI18n } from "../../lib/i18n";

interface Props {
  onDone: () => void;
  onCancel: () => void;
  scanQr?: () => Promise<string | null>;
}

export function ImportDialog({ onDone, onCancel, scanQr }: Props) {
  const { language, t } = useI18n();
  const importProfile = useAppStore((s) => s.importProfile);
  const [input, setInput] = useState("");
  const [name, setName] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [scanning, setScanning] = useState(false);

  const readQr = async () => {
    if (!scanQr || scanning || busy) return;
    setScanning(true);
    setError(null);
    try {
      const content = (await scanQr())?.trim();
      if (content) setInput(content);
    } catch {
      setError(t("profiles.qrScanError"));
    } finally {
      setScanning(false);
    }
  };

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (busy || scanning) return;
    if (!input.trim()) return setError(t("profiles.requireLink"));
    setBusy(true);
    setError(null);
    try {
      await importProfile(input, name.trim() || undefined);
      onDone();
    } catch (err) {
      setError(displayError(language, err, "errors.import"));
    } finally {
      setBusy(false);
    }
  };

  return (
    <form className="profile-form" onSubmit={submit}>
      <label>
        {t("profiles.linkOrJson")}
        <textarea
          className="import-input"
          value={input}
          onChange={(e) => setInput(e.target.value)}
          rows={6}
          placeholder={`naive+https://...\nhysteria2://...\n${t("profiles.or")}\n{ "type": "hysteria2", "server": ... }`}
        />
      </label>

      <label>
        {t("profiles.importName")}
        <input value={name} onChange={(e) => setName(e.target.value)} />
      </label>

      {error && <p className="error">{error}</p>}

      <div className="profile-actions">
        {scanQr && (
          <button type="button" disabled={busy || scanning} onClick={() => void readQr()}>
            {t("profiles.scanQr")}
          </button>
        )}
        <button type="submit" disabled={busy || scanning}>
          {busy ? t("profiles.importing") : t("profiles.importAction")}
        </button>
        <button type="button" onClick={onCancel} disabled={busy || scanning}>
          {t("common.cancel")}
        </button>
      </div>
    </form>
  );
}
