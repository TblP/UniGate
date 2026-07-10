//! Персист и операции над профилями: `<app_config_dir>/profiles.json`.

use crate::models::Profile;
use crate::storage;
use tauri::AppHandle;

const FILE: &str = "profiles.json";

/// Все профили (пустой список, если файла нет).
pub fn list(app: &AppHandle) -> Result<Vec<Profile>, String> {
    storage::read_json(app, FILE)
}

/// Перезаписывает весь список профилей на диск.
pub fn save_all(app: &AppHandle, profiles: &[Profile]) -> Result<(), String> {
    storage::write_json(app, FILE, profiles)
}
