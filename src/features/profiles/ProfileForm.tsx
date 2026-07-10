import { useState } from "react";
import type { Outbound, Profile } from "../../lib/types";

type ProtocolType = Outbound["type"]; // "socks" | "http" | "hysteria2"

interface Props {
  /** Если задан — режим редактирования, поля предзаполнены. */
  initial?: Profile;
  onSubmit: (name: string, outbound: Outbound) => void | Promise<void>;
  onCancel: () => void;
}

export function ProfileForm({ initial, onSubmit, onCancel }: Props) {
  const o = initial?.outbound;
  const hasAuth = o?.type === "socks" || o?.type === "http";

  const [name, setName] = useState(initial?.name ?? "");
  const [type, setType] = useState<ProtocolType>(o?.type ?? "socks");
  const [server, setServer] = useState(o?.server ?? "");
  const [port, setPort] = useState(o ? String(o.port) : "1080");
  const [username, setUsername] = useState(hasAuth ? o!.username ?? "" : "");
  const [password, setPassword] = useState(
    o?.type === "hysteria2" ? o.password : hasAuth ? o!.password ?? "" : "",
  );
  const [tls, setTls] = useState(o?.type === "http" ? o.tls ?? false : false);
  const [sni, setSni] = useState(o?.type === "hysteria2" ? o.sni ?? "" : "");
  const [insecure, setInsecure] = useState(
    o?.type === "hysteria2" ? o.insecure ?? false : false,
  );
  const [obfsPassword, setObfsPassword] = useState(
    o?.type === "hysteria2" ? o.obfsPassword ?? "" : "",
  );
  const [up, setUp] = useState(
    o?.type === "hysteria2" && o.upMbps ? String(o.upMbps) : "",
  );
  const [down, setDown] = useState(
    o?.type === "hysteria2" && o.downMbps ? String(o.downMbps) : "",
  );
  const [error, setError] = useState<string | null>(null);

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    const portNum = Number.parseInt(port, 10);
    if (!name.trim()) return setError("Укажите имя профиля");
    if (!server.trim()) return setError("Укажите адрес сервера");
    if (!Number.isFinite(portNum) || portNum < 1 || portNum > 65535)
      return setError("Порт должен быть в диапазоне 1–65535");

    const srv = server.trim();
    let outbound: Outbound;
    if (type === "socks") {
      outbound = {
        type: "socks",
        server: srv,
        port: portNum,
        username: username.trim() || undefined,
        password: password.trim() || undefined,
      };
    } else if (type === "http") {
      outbound = {
        type: "http",
        server: srv,
        port: portNum,
        username: username.trim() || undefined,
        password: password.trim() || undefined,
        tls,
      };
    } else {
      outbound = {
        type: "hysteria2",
        server: srv,
        port: portNum,
        password: password,
        sni: sni.trim() || undefined,
        insecure: insecure || undefined,
        obfsPassword: obfsPassword.trim() || undefined,
        upMbps: up.trim() ? Number.parseInt(up, 10) : undefined,
        downMbps: down.trim() ? Number.parseInt(down, 10) : undefined,
      };
    }

    await onSubmit(name.trim(), outbound);
  };

  return (
    <form className="profile-form" onSubmit={submit}>
      <label>
        Имя
        <input value={name} onChange={(e) => setName(e.target.value)} placeholder="Мой профиль" />
      </label>

      <label>
        Протокол
        <select value={type} onChange={(e) => setType(e.target.value as ProtocolType)}>
          <option value="socks">SOCKS5</option>
          <option value="http">HTTP(S)</option>
          <option value="hysteria2">Hysteria 2</option>
        </select>
      </label>

      <label>
        Сервер
        <input value={server} onChange={(e) => setServer(e.target.value)} placeholder="example.com" />
      </label>

      <label>
        Порт
        <input
          type="number"
          value={port}
          onChange={(e) => setPort(e.target.value)}
          min={1}
          max={65535}
        />
      </label>

      {(type === "socks" || type === "http") && (
        <>
          <label>
            Логин (необязательно)
            <input value={username} onChange={(e) => setUsername(e.target.value)} />
          </label>
          <label>
            Пароль (необязательно)
            <input
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
            />
          </label>
        </>
      )}

      {type === "http" && (
        <label className="checkbox">
          <input type="checkbox" checked={tls} onChange={(e) => setTls(e.target.checked)} />
          TLS (HTTPS-прокси)
        </label>
      )}

      {type === "hysteria2" && (
        <>
          <label>
            Пароль
            <input
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
            />
          </label>

          <details className="advanced">
            <summary>
              Дополнительно — только если так настроен ваш сервер (обычно не нужно)
            </summary>

            <label>
              Домен для TLS (SNI)
              <input
                value={sni}
                onChange={(e) => setSni(e.target.value)}
                placeholder="оставьте пустым — возьмётся адрес сервера"
              />
              <span className="field-hint">
                Домен, на который выписан сертификат сервера. Если он совпадает с
                адресом сервера — оставьте пустым.
              </span>
            </label>

            <label>
              Пароль маскировки трафика (obfs)
              <input
                value={obfsPassword}
                onChange={(e) => setObfsPassword(e.target.value)}
                placeholder="оставьте пустым, если не уверены"
              />
              <span className="field-hint">
                Нужен, только если на сервере включена маскировка Salamander.
                Если подключение и так работает — оставьте пустым.
              </span>
            </label>

            <label>
              Скорость канала, Мбит/с (необязательно)
              <div className="row-2">
                <input
                  type="number"
                  value={up}
                  onChange={(e) => setUp(e.target.value)}
                  min={0}
                  placeholder="↑ отдача"
                />
                <input
                  type="number"
                  value={down}
                  onChange={(e) => setDown(e.target.value)}
                  min={0}
                  placeholder="↓ загрузка"
                />
              </div>
              <span className="field-hint">
                Подсказка скорости вашего канала для ускорения. Можно не заполнять.
              </span>
            </label>

            <label className="checkbox">
              <input
                type="checkbox"
                checked={insecure}
                onChange={(e) => setInsecure(e.target.checked)}
              />
              Разрешить недоверенный сертификат
            </label>
            <span className="field-hint">
              Включайте, только если у сервера самоподписанный сертификат (нет
              настоящего домена). У вас есть домен — не включайте.
            </span>
          </details>
        </>
      )}

      {error && <p className="error">{error}</p>}

      <div className="profile-actions">
        <button type="submit">{initial ? "Сохранить" : "Создать"}</button>
        <button type="button" onClick={onCancel}>
          Отмена
        </button>
      </div>
    </form>
  );
}
