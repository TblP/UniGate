// Зеркало доменных моделей из src-tauri/src/models.rs.
// Держать синхронно с Rust-стороной (camelCase, теговые union'ы).

export interface TlsOpts {
  enabled: boolean;
  sni?: string;
  insecure?: boolean;
  alpn?: string[];
  fingerprint?: string;
  realityPublicKey?: string;
  realityShortId?: string;
}

export type Transport =
  | { type: "ws"; path?: string; host?: string }
  | { type: "grpc"; serviceName?: string };

export type Outbound =
  | {
      type: "socks";
      server: string;
      port: number;
      username?: string;
      password?: string;
    }
  | {
      type: "http";
      server: string;
      port: number;
      username?: string;
      password?: string;
      tls?: boolean;
    }
  | {
      type: "hysteria2";
      server: string;
      port: number;
      password: string;
      sni?: string;
      insecure?: boolean;
      obfsPassword?: string;
      upMbps?: number;
      downMbps?: number;
    }
  | {
      type: "shadowsocks";
      server: string;
      port: number;
      method: string;
      password: string;
    }
  | {
      type: "trojan";
      server: string;
      port: number;
      password: string;
      tls: TlsOpts;
      transport?: Transport;
    }
  | {
      type: "vless";
      server: string;
      port: number;
      uuid: string;
      flow?: string;
      tls: TlsOpts;
      transport?: Transport;
    }
  | {
      type: "vmess";
      server: string;
      port: number;
      uuid: string;
      alterId: number;
      security?: string;
      tls: TlsOpts;
      transport?: Transport;
    }
  | {
      type: "tuic";
      server: string;
      port: number;
      uuid: string;
      password: string;
      congestionControl?: string;
      tls: TlsOpts;
    }
  | {
      // AmneziaWG — отдельный движок; config = готовый .conf
      type: "amnezia_wg";
      config: string;
      server: string;
      port: number;
    };

export interface Profile {
  id: string;
  name: string;
  outbound: Outbound;
  /** Id подписки, если профиль из неё. */
  subscriptionId?: string;
}

export interface Subscription {
  id: string;
  name: string;
  url: string;
  /** Число профилей из подписки. */
  count: number;
  /** UNIX-время последнего обновления (сек). */
  updatedAt?: number;
}

export type ConnectionState =
  | { state: "disconnected" }
  | { state: "connecting" }
  | { state: "connected" }
  | { state: "disconnecting" }
  | { state: "error"; message: string };

export interface Traffic {
  /** Скорость отдачи, байт/с. */
  up: number;
  /** Скорость загрузки, байт/с. */
  down: number;
  /** Суммарно отдано за сессию, байт. */
  upTotal: number;
  /** Суммарно загружено за сессию, байт. */
  downTotal: number;
}

export type Theme = "system" | "light" | "dark";

export type Mode = "proxy" | "tun";

export type TunStack = "gvisor" | "system" | "mixed";

export type AppMode = "off" | "only" | "except";

export interface Routing {
  /** LAN (приватные IP) — напрямую. */
  bypassLan: boolean;
  /** RU-трафик (geoip-ru + .ru/.рф/.su) — напрямую. */
  bypassRu: boolean;
  /** Режим split по приложениям. */
  appMode: AppMode;
  /** Приложения через VPN (режим Only). */
  onlyApps: string[];
  /** Приложения напрямую (режим Except). */
  exceptApps: string[];
}

export interface Settings {
  theme: Theme;
  language: string;
  autoConnect: boolean;
  activeProfileId: string | null;
  mode: Mode;
  /** Сетевой стек TUN (gvisor по умолчанию). */
  tunStack: TunStack;
  routing: Routing;
  /** Закрытие окна сворачивает в трей. */
  minimizeToTray: boolean;
  /** Windows: запуск с правами администратора без UAC (задача Планировщика). */
  adminLaunch: boolean;
}
