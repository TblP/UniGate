import { useState } from "react";
import { useAppStore } from "../../store/useAppStore";

function formatTime(unix?: number): string {
  if (!unix) return "—";
  return new Date(unix * 1000).toLocaleString();
}

export function SubscriptionsPanel() {
  const subscriptions = useAppStore((s) => s.subscriptions);
  const addSubscription = useAppStore((s) => s.addSubscription);
  const updateSubscription = useAppStore((s) => s.updateSubscription);
  const deleteSubscription = useAppStore((s) => s.deleteSubscription);

  const [name, setName] = useState("");
  const [url, setUrl] = useState("");
  const [adding, setAdding] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [updatingId, setUpdatingId] = useState<string | null>(null);

  const add = async () => {
    if (!url.trim()) return setError("Укажите URL подписки");
    setBusy(true);
    setError(null);
    try {
      await addSubscription(name.trim() || "Подписка", url.trim());
      setName("");
      setUrl("");
      setAdding(false);
    } catch (e) {
      setError(String(e));
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
      setError(String(e));
    } finally {
      setUpdatingId(null);
    }
  };

  return (
    <section className="core-card">
      <div className="panel-head">
        <h2>Подписки</h2>
        {!adding && <button onClick={() => setAdding(true)}>+ Добавить</button>}
      </div>

      {adding && (
        <div className="profile-form">
          <label>
            Имя (необязательно)
            <input
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="Мой провайдер"
            />
          </label>
          <label>
            URL подписки
            <input
              value={url}
              onChange={(e) => setUrl(e.target.value)}
              placeholder="https://…/sub"
            />
          </label>
          {error && <p className="error">{error}</p>}
          <div className="profile-actions">
            <button onClick={add} disabled={busy}>
              {busy ? "Загрузка…" : "Добавить"}
            </button>
            <button
              onClick={() => {
                setAdding(false);
                setError(null);
              }}
              disabled={busy}
            >
              Отмена
            </button>
          </div>
        </div>
      )}

      {!adding && error && <p className="error">{error}</p>}

      {subscriptions.length === 0
        ? !adding && (
            <p className="hint">Нет подписок. Вставь URL — создастся список серверов.</p>
          )
        : (
            <ul className="profile-list">
              {subscriptions.map((s) => (
                <li key={s.id} className="profile">
                  <div className="profile-info">
                    <span className="profile-name">{s.name}</span>
                    <span className="profile-sub">
                      {s.count} серв. · обновлено {formatTime(s.updatedAt)}
                    </span>
                  </div>
                  <div className="profile-actions">
                    <button onClick={() => update(s.id)} disabled={updatingId === s.id}>
                      {updatingId === s.id ? "Обновление…" : "Обновить"}
                    </button>
                    <button className="danger" onClick={() => deleteSubscription(s.id)}>
                      Удалить
                    </button>
                  </div>
                </li>
              ))}
            </ul>
          )}
    </section>
  );
}
