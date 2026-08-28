mod commands;
mod config;
#[cfg(desktop)]
mod connection;
#[cfg(mobile)]
#[path = "android/connection.rs"]
mod connection;
mod elevation;
mod export;
mod import;
mod models;
mod profiles;
mod settings;
mod stats;
mod storage;
mod subscriptions;
mod sysproxy;

use connection::Connection;
#[cfg(desktop)]
use tauri::menu::{Menu, MenuItem};
#[cfg(desktop)]
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};
#[cfg(desktop)]
use tauri_plugin_autostart::ManagerExt;

/// Аргумент, с которым автозапуск системы стартует приложение.
pub const AUTOSTART_ARG: &str = "--autostart";

/// Маркер «этот запуск пришёл из автозапуска» для редиректа через задачу
/// Планировщика (adminLaunch): задача запускает exe без аргументов, поэтому
/// обычный процесс перед редиректом оставляет файл-маркер
/// (см. elevation::admin_launch_startup).
pub const AUTOSTART_MARKER: &str = "autostart-redirect.flag";

/// Запущены ли мы автозапуском системы: по аргументу `--autostart` либо по
/// свежему маркеру от процесса, средиректившего нас через Планировщик.
/// Маркер одноразовый — удаляется при любом исходе.
#[cfg(desktop)]
fn launched_via_autostart(app: &AppHandle) -> bool {
    let by_arg = std::env::args().any(|a| a == AUTOSTART_ARG);
    let mut by_marker = false;
    if let Ok(marker) = storage::path(app, AUTOSTART_MARKER) {
        if marker.exists() {
            by_marker = std::fs::metadata(&marker)
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.elapsed().ok())
                .is_some_and(|age| age < std::time::Duration::from_secs(120));
            let _ = std::fs::remove_file(&marker);
        }
    }
    by_arg || by_marker
}

/// Перерегистрирует включённый автозапуск: записи, сделанные до появления
/// `--autostart`, не содержат аргумента — enable() идемпотентно докидывает его.
#[cfg(desktop)]
fn refresh_autostart(app: &AppHandle) {
    let autolaunch = app.autolaunch();
    if autolaunch.is_enabled().unwrap_or(false) {
        let _ = autolaunch.enable();
    }
}

/// Показывает и фокусирует главное окно (из трея).
#[cfg(desktop)]
fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

/// Иконка в системном трее с меню (Показать / Скрыть / Выход).
#[cfg(desktop)]
fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "Показать", true, None::<&str>)?;
    let hide = MenuItem::with_id(app, "hide", "Скрыть в трей", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Выход", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &hide, &quit])?;

    TrayIconBuilder::with_id("main")
        .icon(app.default_window_icon().unwrap().clone())
        .tooltip("UniGate")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => show_main(app),
            "hide" => {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.hide();
                }
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

/// Закрытие окна: если включено «сворачивать в трей» — прячем, а не выходим.
#[cfg(desktop)]
fn on_window_event(window: &tauri::Window, event: &tauri::WindowEvent) {
    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
        let to_tray = settings::load(window.app_handle())
            .map(|s| s.minimize_to_tray)
            .unwrap_or(true);
        if to_tray {
            api.prevent_close();
            let _ = window.hide();
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init());
    #[cfg(windows)]
    let builder = builder.plugin(tauri_plugin_updater::Builder::new().build());
    #[cfg(desktop)]
    let builder = builder.plugin(tauri_plugin_autostart::init(
        tauri_plugin_autostart::MacosLauncher::LaunchAgent,
        Some(vec![AUTOSTART_ARG]),
    ));
    #[cfg(mobile)]
    let builder = builder.plugin(connection::init());
    let builder = builder.manage(Connection::new()).setup(|app| {
        // Windows + «запуск от администратора»: обычный запуск редиректим
        // через задачу планировщика (элевация без UAC) и выходим
        #[cfg(windows)]
        if elevation::admin_launch_startup(app.handle()) {
            app.handle().exit(0);
            return Ok(());
        }
        #[cfg(desktop)]
        {
            // окно создаётся скрытым (tauri.conf.json): при запуске
            // автозапуском системы остаёмся в трее, иначе показываем
            if !launched_via_autostart(app.handle()) {
                show_main(app.handle());
            }
            refresh_autostart(app.handle());
            // снять «зависший» системный прокси после нештатного выхода
            connection::reconcile_startup(app.handle());
            build_tray(app.handle())?;
        }
        #[cfg(mobile)]
        let _ = app;
        Ok(())
    });
    #[cfg(desktop)]
    let builder = builder.on_window_event(on_window_event);
    builder
        .invoke_handler(tauri::generate_handler![
            commands::singbox_version,
            commands::get_settings,
            commands::save_settings,
            commands::list_profiles,
            commands::create_profile,
            commands::update_profile,
            commands::delete_profile,
            commands::duplicate_profile,
            commands::import_profile,
            commands::export_profile,
            commands::list_subscriptions,
            commands::add_subscription,
            commands::update_subscription,
            commands::delete_subscription,
            commands::list_installed_apps,
            commands::request_android_quick_tile,
            commands::request_android_widget,
            connection::connect,
            connection::disconnect,
            connection::get_connection_state,
            connection::local_proxy_addr,
            connection::awg_shim_available,
            elevation::is_elevated_cmd,
            elevation::relaunch_elevated_cmd,
            elevation::apply_admin_launch,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            // при выходе гарантированно убиваем sing-box и снимаем системный прокси
            #[cfg(desktop)]
            if let tauri::RunEvent::Exit = event {
                connection::cleanup(app);
            }
            #[cfg(mobile)]
            let _ = (app, event);
        });
}
