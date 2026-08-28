//! Android VPN connection manager.
//!
//! Android does not launch the desktop sing-box sidecar. Rust generates the
//! configuration, while a Kotlin `VpnService` owns the system VPN interface
//! and runs the embedded sing-box libbox library.

use crate::models::{AppMode, ConnectionState, InstalledApp, Mode, Outbound, TunStack};
use crate::{config, profiles, settings};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::plugin::{Builder, PluginHandle, TauriPlugin};
use tauri::{AppHandle, Emitter, Manager, Runtime};

const EVENT: &str = "connection-state";
const AWG_SHIM_PORT: u16 = 2081;

pub struct Connection(pub Mutex<ConnectionState>);

impl Connection {
    pub fn new() -> Self {
        Self(Mutex::new(ConnectionState::Disconnected))
    }
}

struct VpnPlugin<R: Runtime>(PluginHandle<R>);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StartRequest {
    config: String,
    awg_config: Option<String>,
    profile_name: String,
    include_packages: Vec<String>,
    exclude_packages: Vec<String>,
}

#[derive(Deserialize)]
struct StartResponse {
    started: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AssetsResponse {
    geoip_path: String,
}

#[derive(Deserialize)]
struct AppsResponse {
    apps: Vec<InstalledApp>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StatusResponse {
    state: String,
}

#[derive(Deserialize)]
struct QuickTileResponse {
    added: bool,
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("unigate-vpn")
        .setup(|app, api| {
            let handle = api.register_android_plugin("com.unigate.app.vpn", "VpnPlugin")?;
            app.manage(VpnPlugin(handle));
            Ok(())
        })
        .build()
}

fn set_state(app: &AppHandle, state: ConnectionState) {
    if let Ok(mut current) = app.state::<Connection>().0.lock() {
        *current = state.clone();
    }
    let _ = app.emit(EVENT, state);
}

#[tauri::command]
pub async fn get_connection_state(app: AppHandle) -> ConnectionState {
    let was_connected = is_connected(&app);
    let native_state = app
        .state::<VpnPlugin<tauri::Wry>>()
        .0
        .run_mobile_plugin_async::<StatusResponse>("status", ())
        .await
        .map(|response| response.state)
        .unwrap_or_else(|_| {
            if was_connected {
                "on".into()
            } else {
                "off".into()
            }
        });
    let state = match native_state.as_str() {
        "on" => ConnectionState::Connected,
        "connecting" => ConnectionState::Connecting,
        _ => ConnectionState::Disconnected,
    };
    set_state(&app, state.clone());
    if matches!(&state, ConnectionState::Connected) && !was_connected {
        crate::stats::start_polling(app);
    }
    state
}

pub fn is_connected(app: &AppHandle) -> bool {
    matches!(
        app.state::<Connection>().0.lock().as_deref(),
        Ok(ConnectionState::Connected)
    )
}

#[tauri::command]
pub fn local_proxy_addr() -> String {
    String::new()
}

#[tauri::command]
pub fn awg_shim_available() -> bool {
    true
}

pub async fn list_installed_apps(app: AppHandle) -> Result<Vec<InstalledApp>, String> {
    app.state::<VpnPlugin<tauri::Wry>>()
        .0
        .run_mobile_plugin_async::<AppsResponse>("listApps", ())
        .await
        .map(|response| response.apps)
        .map_err(|error| format!("Не удалось получить приложения Android: {error}"))
}

pub async fn request_quick_tile(app: AppHandle) -> Result<bool, String> {
    app.state::<VpnPlugin<tauri::Wry>>()
        .0
        .run_mobile_plugin_async::<QuickTileResponse>("requestQuickTile", ())
        .await
        .map(|response| response.added)
        .map_err(|error| format!("Не удалось добавить плитку UniGate: {error}"))
}

pub async fn request_widget(app: AppHandle) -> Result<bool, String> {
    app.state::<VpnPlugin<tauri::Wry>>()
        .0
        .run_mobile_plugin_async::<QuickTileResponse>("requestWidget", ())
        .await
        .map(|response| response.added)
        .map_err(|error| format!("Не удалось добавить виджет UniGate: {error}"))
}

fn looks_like_android_package(value: &str) -> bool {
    value.contains('.') && !value.to_ascii_lowercase().ends_with(".exe")
}

#[tauri::command]
pub async fn connect(app: AppHandle, profile_id: String) -> Result<ConnectionState, String> {
    let profile = profiles::list(&app)?
        .into_iter()
        .find(|profile| profile.id == profile_id)
        .ok_or_else(|| "Профиль не найден".to_string())?;

    set_state(&app, ConnectionState::Connecting);
    let app_settings = settings::load(&app)?;

    // Android performs per-app split at VpnService.Builder level using package
    // identifiers. Disable desktop process_name rules in the sing-box config.
    let mut routing = app_settings.routing.clone();
    let app_mode = routing.app_mode;
    routing.app_mode = AppMode::Off;
    let geoip_path = if routing.bypass_ru {
        match app
            .state::<VpnPlugin<tauri::Wry>>()
            .0
            .run_mobile_plugin_async::<AssetsResponse>("prepareAssets", ())
            .await
        {
            Ok(response) => Some(response.geoip_path),
            Err(error) => {
                let message = format!("Не удалось подготовить RU-маршрутизацию: {error}");
                set_state(
                    &app,
                    ConnectionState::Error {
                        message: message.clone(),
                    },
                );
                return Err(message);
            }
        }
    } else {
        None
    };

    // Android embeds the same AWG 3.1 userspace shim as desktop. sing-box sees
    // it as a local SOCKS5 outbound and therefore keeps RU/LAN/per-app split
    // and Clash API statistics for AmneziaWG profiles too.
    let awg_config = match &profile.outbound {
        Outbound::AmneziaWg { config, .. } => Some(config.clone()),
        _ => None,
    };
    let mut generated_profile = profile.clone();
    if awg_config.is_some() {
        generated_profile.outbound = Outbound::Socks {
            server: "127.0.0.1".into(),
            port: AWG_SHIM_PORT,
            username: None,
            password: None,
        };
    }
    let config = config::generate(
        &generated_profile,
        Mode::Tun,
        0,
        &routing,
        geoip_path.as_deref(),
        &[],
        false,
        TunStack::Gvisor,
    );
    let config = serde_json::to_string(&config)
        .map_err(|error| format!("Не удалось создать Android-конфиг: {error}"))?;

    let (include_packages, exclude_packages) = match app_mode {
        AppMode::Only => (
            app_settings
                .routing
                .only_apps
                .iter()
                .filter(|value| looks_like_android_package(value))
                .cloned()
                .collect(),
            Vec::new(),
        ),
        AppMode::Except => (
            Vec::new(),
            app_settings
                .routing
                .except_apps
                .iter()
                .filter(|value| looks_like_android_package(value))
                .cloned()
                .collect(),
        ),
        AppMode::Off => (Vec::new(), Vec::new()),
    };
    if matches!(app_mode, AppMode::Only) && include_packages.is_empty() {
        let message = "Выберите хотя бы одно приложение для режима «Только выбранные»"
            .to_string();
        set_state(
            &app,
            ConnectionState::Error {
                message: message.clone(),
            },
        );
        return Err(message);
    }

    let request = StartRequest {
        config,
        awg_config,
        profile_name: profile.name,
        include_packages,
        exclude_packages,
    };
    let response = match app
        .state::<VpnPlugin<tauri::Wry>>()
        .0
        .run_mobile_plugin_async::<StartResponse>("start", request)
        .await
    {
        Ok(response) => response,
        Err(error) => {
            let message = format!("Не удалось запустить Android VPN: {error}");
            set_state(
                &app,
                ConnectionState::Error {
                    message: message.clone(),
                },
            );
            return Err(message);
        }
    };

    if !response.started {
        let message = "Android VPN-сервис не подтвердил запуск".to_string();
        set_state(
            &app,
            ConnectionState::Error {
                message: message.clone(),
            },
        );
        return Err(message);
    }

    set_state(&app, ConnectionState::Connected);
    crate::stats::start_polling(app.clone());
    Ok(ConnectionState::Connected)
}

#[tauri::command]
pub async fn disconnect(app: AppHandle) -> Result<ConnectionState, String> {
    set_state(&app, ConnectionState::Disconnecting);
    app.state::<VpnPlugin<tauri::Wry>>()
        .0
        .run_mobile_plugin_async::<()>("stop", ())
        .await
        .map_err(|error| format!("Не удалось остановить Android VPN: {error}"))?;
    set_state(&app, ConnectionState::Disconnected);
    Ok(ConnectionState::Disconnected)
}
