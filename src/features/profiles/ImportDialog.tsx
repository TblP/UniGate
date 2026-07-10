import { useState } from "react";
import { useAppStore } from "../../store/useAppStore";

interface Props {
  onDone: () => void;
  onCancel: () => void;
}

export function ImportDialog({ onDone, onCancel }: Props) {
  const importProfile = useAppStore((s) => s.importProfile);
  const [input, setInput] = useState("");
  const [name, setName] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!input.trim()) return setError("Вставьте ссылку или JSON");
    setBusy(true);
    setError(null);
    try {
      await importProfile(input, name.trim() || undefined);
      onDone();
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <form className="profile-form" onSubmit={submit}>
      <label>
        Ссылка или JSON
        <textarea
          className="import-input"
          value={input}
          onChange={(e) => setInput(e.target.value)}
          rows={6}
          placeholder={"hysteria2://...\nили\n{ \"type\": \"hysteria2\", \"server\": ... }"}
        />
      </label>

      <label>
        Имя (необязательно — иначе из ссылки)
        <input value={name} onChange={(e) => setName(e.target.value)} />
      </label>

      {error && <p className="error">{error}</p>}

      <div className="profile-actions">
        <button type="submit" disabled={busy}>
          {busy ? "Импорт…" : "Импортировать"}
        </button>
        <button type="button" onClick={onCancel} disabled={busy}>
          Отмена
        </button>
      </div>
    </form>
  );
}
