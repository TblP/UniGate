//! Персист пользовательских настроек: `<app_config_dir>/settings.json`.

use crate::models::Settings;
use crate::storage;
use tauri::AppHandle;

const FILE: &str = "settings.json";

/// Читает настройки с диска (или значения по умолчанию, если файла нет).
pub fn load(app: &AppHandle) -> Result<Settings, String> {
    storage::read_json(app, FILE)
}

/// Сохраняет настройки на диск.
pub fn save(app: &AppHandle, settings: &Settings) -> Result<(), String> {
    storage::write_json(app, FILE, settings)
}
