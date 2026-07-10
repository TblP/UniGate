//! Общий слой персиста: атомарное чтение/запись JSON в каталоге конфигурации.
//! Используется и настройками, и профилями.

use serde::{de::DeserializeOwned, Serialize};
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

/// Полный путь к файлу в каталоге конфигурации (каталог гарантированно существует).
pub fn path(app: &AppHandle, file: &str) -> Result<PathBuf, String> {
    Ok(config_dir(app)?.join(file))
}

/// Каталог конфигурации приложения, гарантируя его существование.
/// На Windows — `%APPDATA%/com.unigate.app`.
fn config_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("не удалось определить каталог конфигурации: {e}"))?;
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("не удалось создать каталог конфигурации: {e}"))?;
    Ok(dir)
}

/// Читает JSON-файл. Если файла нет — `T::default()`.
/// Битый файл не валит приложение: логируем и откатываемся на дефолт.
pub fn read_json<T: DeserializeOwned + Default>(app: &AppHandle, file: &str) -> Result<T, String> {
    let path = config_dir(app)?.join(file);
    if !path.exists() {
        return Ok(T::default());
    }
    let raw = std::fs::read_to_string(&path)
        .map_err(|e| format!("не удалось прочитать {}: {e}", path.display()))?;
    match serde_json::from_str::<T>(&raw) {
        Ok(value) => Ok(value),
        Err(e) => {
            eprintln!("{file} повреждён ({e}), откат на значения по умолчанию");
            Ok(T::default())
        }
    }
}

/// Атомарно (temp + rename) пишет значение в JSON-файл.
pub fn write_json<T: Serialize + ?Sized>(
    app: &AppHandle,
    file: &str,
    value: &T,
) -> Result<(), String> {
    let path = config_dir(app)?.join(file);
    let json = serde_json::to_string_pretty(value)
        .map_err(|e| format!("не удалось сериализовать {file}: {e}"))?;
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, json).map_err(|e| format!("не удалось записать {}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, &path)
        .map_err(|e| format!("не удалось заменить {}: {e}", path.display()))?;
    Ok(())
}
