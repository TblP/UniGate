//! Доменные модели UniGate.
//!
//! Сериализуются в camelCase, чтобы фронтенд получал идиоматичный для TS JSON.
//! Зеркальные типы — в `src/lib/types.ts` (держать синхронно).

use serde::{Deserialize, Serialize};

/// Поддерживаемый протокол исходящего соединения.
///
/// Теговый enum (`{ "type": "socks", ... }`) → на фронте это discriminated union.
/// На Phase 1 заведены протоколы для первого подключения (Phase 3);
/// остальные (Hysteria2, AmneziaWG, Shadowsocks, VMess, VLESS, Trojan, TUIC)
/// добавляются в свои фазы.
/// Общие TLS-параметры для протоколов поверх TLS (trojan/vless/vmess/tuic).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct TlsOpts {
    #[serde(default)]
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sni: Option<String>,
    #[serde(default)]
    pub insecure: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub alpn: Vec<String>,
    /// uTLS fingerprint (например, "chrome").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<String>,
    /// Reality public key (если используется Reality).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reality_public_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reality_short_id: Option<String>,
}

/// Транспорт. Отсутствие (None у outbound) = обычный TCP.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case", rename_all_fields = "camelCase")]
pub enum Transport {
    Ws {
        #[serde(skip_serializing_if = "Option::is_none")]
        path: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        host: Option<String>,
    },
    Grpc {
        #[serde(skip_serializing_if = "Option::is_none")]
        service_name: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case", rename_all_fields = "camelCase")]
pub enum Outbound {
    Socks {
        server: String,
        port: u16,
        #[serde(skip_serializing_if = "Option::is_none")]
        username: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        password: Option<String>,
    },
    Http {
        server: String,
        port: u16,
        #[serde(skip_serializing_if = "Option::is_none")]
        username: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        password: Option<String>,
        #[serde(default)]
        tls: bool,
    },
    Hysteria2 {
        server: String,
        port: u16,
        password: String,
        /// TLS SNI (`tls.server_name`). Если пусто — sing-box возьмёт адрес сервера.
        #[serde(skip_serializing_if = "Option::is_none")]
        sni: Option<String>,
        /// Не проверять сертификат (`tls.insecure`).
        #[serde(default)]
        insecure: bool,
        /// Пароль обфускации Salamander (`obfs.password`), если используется.
        #[serde(skip_serializing_if = "Option::is_none")]
        obfs_password: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        up_mbps: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        down_mbps: Option<u32>,
    },
    Shadowsocks {
        server: String,
        port: u16,
        /// Шифр (например, "aes-256-gcm", "chacha20-ietf-poly1305", "2022-blake3-aes-256-gcm").
        method: String,
        password: String,
    },
    Trojan {
        server: String,
        port: u16,
        password: String,
        #[serde(default)]
        tls: TlsOpts,
        #[serde(skip_serializing_if = "Option::is_none")]
        transport: Option<Transport>,
    },
    Vless {
        server: String,
        port: u16,
        uuid: String,
        /// flow (например, "xtls-rprx-vision"); пусто — без flow.
        #[serde(skip_serializing_if = "Option::is_none")]
        flow: Option<String>,
        #[serde(default)]
        tls: TlsOpts,
        #[serde(skip_serializing_if = "Option::is_none")]
        transport: Option<Transport>,
    },
    Vmess {
        server: String,
        port: u16,
        uuid: String,
        #[serde(default)]
        alter_id: u32,
        /// Шифрование (auto/aes-128-gcm/chacha20-poly1305/none).
        #[serde(skip_serializing_if = "Option::is_none")]
        security: Option<String>,
        #[serde(default)]
        tls: TlsOpts,
        #[serde(skip_serializing_if = "Option::is_none")]
        transport: Option<Transport>,
    },
    Tuic {
        server: String,
        port: u16,
        uuid: String,
        password: String,
        /// Контроль перегрузки (cubic/new_reno/bbr).
        #[serde(skip_serializing_if = "Option::is_none")]
        congestion_control: Option<String>,
        #[serde(default)]
        tls: TlsOpts,
    },
    /// AmneziaWG — обрабатывается отдельным движком (amneziawg.exe), НЕ sing-box.
    /// `config` — готовый `.conf` (формат wg-quick + Jc/Jmin/S/H/I).
    AmneziaWg {
        config: String,
        /// Адрес/порт сервера — только для отображения.
        server: String,
        port: u16,
    },
}

/// Профиль подключения: именованная обёртка над одним outbound.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub outbound: Outbound,
    /// Id подписки, если профиль создан из неё (для обновления/удаления).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subscription_id: Option<String>,
}

/// Подписка: URL со списком серверов, обновляемый по требованию.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Subscription {
    pub id: String,
    pub name: String,
    pub url: String,
    /// Сколько профилей создано из подписки при последнем обновлении.
    #[serde(default)]
    pub count: usize,
    /// Метка последнего обновления (UNIX-время, секунды).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<u64>,
}

/// Состояние подключения. Теговый enum, чтобы фронт мог матчить по `state`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum ConnectionState {
    #[default]
    Disconnected,
    Connecting,
    Connected,
    Disconnecting,
    Error {
        message: String,
    },
}

/// Тема оформления.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

/// Режим работы: локальный прокси (без админа) или TUN (полный VPN, нужен админ).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    #[default]
    Proxy,
    Tun,
}

/// Сетевой стек TUN-инбаунда — как sing-box обрабатывает перехваченный трафик.
/// Значения совпадают со строками sing-box (`inbounds[].stack`).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum TunStack {
    /// Userspace TCP/IP (gVisor) внутри процесса sing-box. Совместим с
    /// виртуальными адаптерами (Docker/Hyper-V/VirtualBox), но под высоким
    /// packet-per-second (игры) дороже по CPU. По умолчанию.
    #[default]
    Gvisor,
    /// Стек ядра ОС — быстрее и без userspace-оверхеда, но рядом с виртуальными
    /// адаптерами может не поймать часть трафика.
    System,
    /// system для TCP + gvisor для UDP.
    Mixed,
}

impl TunStack {
    /// Строка для `inbounds[].stack` sing-box.
    pub fn as_singbox(self) -> &'static str {
        match self {
            TunStack::Gvisor => "gvisor",
            TunStack::System => "system",
            TunStack::Mixed => "mixed",
        }
    }
}

/// Режим раздельного туннелирования по приложениям (только TUN).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum AppMode {
    /// Все приложения через туннель (кроме обходов LAN/RU).
    #[default]
    Off,
    /// Только выбранные приложения через туннель, остальные — напрямую.
    Only,
    /// Выбранные приложения напрямую, остальные — через туннель.
    Except,
}

/// Настройки маршрутизации (split-tunneling). Действуют в TUN-режиме, для
/// sing-box-протоколов (не AmneziaWG).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Routing {
    /// LAN (приватные IP) — напрямую, мимо туннеля.
    pub bypass_lan: bool,
    /// RU-трафик (geoip-ru + домены .ru/.рф/.su) — напрямую.
    pub bypass_ru: bool,
    /// Режим по приложениям.
    pub app_mode: AppMode,
    /// Приложения, идущие через VPN (режим Only). Имена процессов (напр. "telegram.exe").
    #[serde(default)]
    pub only_apps: Vec<String>,
    /// Приложения, идущие напрямую (режим Except).
    #[serde(default)]
    pub except_apps: Vec<String>,
}

/// Пользовательские настройки приложения. Персистятся на диск.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub theme: Theme,
    /// Язык интерфейса: "ru" | "en".
    pub language: String,
    /// Автоподключение к активному профилю при старте.
    pub auto_connect: bool,
    /// Id профиля, выбранного активным (если есть).
    pub active_profile_id: Option<String>,
    /// Режим работы (proxy/tun).
    pub mode: Mode,
    /// Сетевой стек TUN (см. `TunStack`). По умолчанию gvisor.
    #[serde(default)]
    pub tun_stack: TunStack,
    /// Маршрутизация / split-tunneling.
    #[serde(default)]
    pub routing: Routing,
    /// Закрытие окна сворачивает в трей (а не выходит).
    #[serde(default = "default_true")]
    pub minimize_to_tray: bool,
    /// Windows: запускаться с правами администратора без UAC-запроса — через
    /// задачу Планировщика с RunLevel=Highest (создаётся один раз под UAC).
    /// На других ОС игнорируется.
    #[serde(default)]
    pub admin_launch: bool,
}

fn default_true() -> bool {
    true
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: Theme::default(),
            language: "ru".to_string(),
            auto_connect: false,
            active_profile_id: None,
            mode: Mode::default(),
            tun_stack: TunStack::default(),
            routing: Routing::default(),
            minimize_to_tray: true,
            admin_launch: false,
        }
    }
}
