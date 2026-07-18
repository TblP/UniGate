//! Tauri-команды — мост между фронтендом и бэкендом.
//! Все команды возвращают `Result<_, String>`; строка ошибки приходит на фронт как reject.

use crate::models::{Outbound, Profile, Settings, Subscription};
use crate::{import, profiles, settings, subscriptions};
use tauri_plugin_shell::ShellExt;
use uuid::Uuid;

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Запускает sidecar `sing-box version` и возвращает первую строку вывода.
#[tauri::command]
pub async fn singbox_version(app: tauri::AppHandle) -> Result<String, String> {
    let output = app
        .shell()
        .sidecar("sing-box")
        .map_err(|e| format!("не удалось найти sidecar sing-box: {e}"))?
        .arg("version")
        .output()
        .await
        .map_err(|e| format!("не удалось запустить sing-box: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "sing-box завершился с ошибкой: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout.lines().next().unwrap_or("").trim().to_string())
}

/// Возвращает текущие настройки (или значения по умолчанию, если файла нет).
#[tauri::command]
pub fn get_settings(app: tauri::AppHandle) -> Result<Settings, String> {
    settings::load(&app)
}

/// Сохраняет настройки на диск и возвращает сохранённое значение.
#[tauri::command]
pub fn save_settings(app: tauri::AppHandle, settings: Settings) -> Result<Settings, String> {
    settings::save(&app, &settings)?;
    Ok(settings)
}

/// Возвращает все профили.
#[tauri::command]
pub fn list_profiles(app: tauri::AppHandle) -> Result<Vec<Profile>, String> {
    profiles::list(&app)
}

/// Создаёт профиль с новым id, сохраняет и возвращает его.
#[tauri::command]
pub fn create_profile(
    app: tauri::AppHandle,
    name: String,
    outbound: Outbound,
) -> Result<Profile, String> {
    let mut all = profiles::list(&app)?;
    let profile = Profile {
        id: Uuid::new_v4().to_string(),
        name,
        outbound,
        subscription_id: None,
    };
    all.push(profile.clone());
    profiles::save_all(&app, &all)?;
    Ok(profile)
}

/// Обновляет существующий профиль (по id). Ошибка, если не найден.
#[tauri::command]
pub fn update_profile(app: tauri::AppHandle, profile: Profile) -> Result<Profile, String> {
    let mut all = profiles::list(&app)?;
    let idx = all
        .iter()
        .position(|p| p.id == profile.id)
        .ok_or_else(|| format!("профиль {} не найден", profile.id))?;
    all[idx] = profile.clone();
    profiles::save_all(&app, &all)?;
    Ok(profile)
}

/// Удаляет профиль по id. Ошибка, если не найден.
#[tauri::command]
pub fn delete_profile(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let mut all = profiles::list(&app)?;
    let before = all.len();
    all.retain(|p| p.id != id);
    if all.len() == before {
        return Err(format!("профиль {id} не найден"));
    }
    profiles::save_all(&app, &all)
}

/// Импортирует профиль из ссылки (`hysteria2://…`) или JSON sing-box.
/// `name` переопределяет имя; если пусто — берётся из ссылки/тега.
#[tauri::command]
pub fn import_profile(
    app: tauri::AppHandle,
    input: String,
    name: Option<String>,
) -> Result<Profile, String> {
    let (parsed_name, outbound) = import::parse(&input)?;
    let final_name = name
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty())
        .unwrap_or(parsed_name);

    let mut all = profiles::list(&app)?;
    let profile = Profile {
        id: Uuid::new_v4().to_string(),
        name: final_name,
        outbound,
        subscription_id: None,
    };
    all.push(profile.clone());
    profiles::save_all(&app, &all)?;
    Ok(profile)
}

/// Все подписки.
#[tauri::command]
pub fn list_subscriptions(app: tauri::AppHandle) -> Result<Vec<Subscription>, String> {
    subscriptions::list(&app)
}

/// Добавляет подписку: скачивает URL, создаёт профили из списка серверов.
#[tauri::command]
pub async fn add_subscription(
    app: tauri::AppHandle,
    name: String,
    url: String,
) -> Result<Subscription, String> {
    let body = subscriptions::fetch(&url).await?;
    let parsed = subscriptions::parse_list(&body);
    if parsed.is_empty() {
        return Err("в подписке не найдено серверов".into());
    }
    let sub_id = Uuid::new_v4().to_string();

    let mut profs = profiles::list(&app)?;
    let count = parsed.len();
    for (pname, outbound) in parsed {
        profs.push(Profile {
            id: Uuid::new_v4().to_string(),
            name: pname,
            outbound,
            subscription_id: Some(sub_id.clone()),
        });
    }
    profiles::save_all(&app, &profs)?;

    let sub = Subscription {
        id: sub_id,
        name,
        url,
        count,
        updated_at: Some(now_unix()),
    };
    let mut subs = subscriptions::list(&app)?;
    subs.push(sub.clone());
    subscriptions::save_all(&app, &subs)?;
    Ok(sub)
}

/// Обновляет подписку: перекачивает список, заменяет её профили.
#[tauri::command]
pub async fn update_subscription(
    app: tauri::AppHandle,
    id: String,
) -> Result<Subscription, String> {
    let mut subs = subscriptions::list(&app)?;
    let idx = subs
        .iter()
        .position(|s| s.id == id)
        .ok_or_else(|| format!("подписка {id} не найдена"))?;
    let body = subscriptions::fetch(&subs[idx].url).await?;
    let parsed = subscriptions::parse_list(&body);
    if parsed.is_empty() {
        return Err("в подписке не найдено серверов".into());
    }

    // выкидываем старые профили этой подписки, добавляем новые
    let mut profs: Vec<Profile> = profiles::list(&app)?
        .into_iter()
        .filter(|p| p.subscription_id.as_deref() != Some(id.as_str()))
        .collect();
    let count = parsed.len();
    for (pname, outbound) in parsed {
        profs.push(Profile {
            id: Uuid::new_v4().to_string(),
            name: pname,
            outbound,
            subscription_id: Some(id.clone()),
        });
    }
    profiles::save_all(&app, &profs)?;

    subs[idx].count = count;
    subs[idx].updated_at = Some(now_unix());
    subscriptions::save_all(&app, &subs)?;
    Ok(subs[idx].clone())
}

/// Удаляет подписку вместе с её профилями.
#[tauri::command]
pub fn delete_subscription(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let mut subs = subscriptions::list(&app)?;
    subs.retain(|s| s.id != id);
    subscriptions::save_all(&app, &subs)?;

    let profs: Vec<Profile> = profiles::list(&app)?
        .into_iter()
        .filter(|p| p.subscription_id.as_deref() != Some(id.as_str()))
        .collect();
    profiles::save_all(&app, &profs)
}

/// Экспортирует профиль в выбранном формате: `"link"` — share-ссылка
/// (vless://…, для socks/http ссылки нет → JSON-фолбэк), `"json"` — sing-box
/// outbound JSON. AmneziaWG в формате `link` отдаёт упакованную `vpn://`-ссылку.
#[tauri::command]
pub fn export_profile(
    app: tauri::AppHandle,
    id: String,
    format: String,
) -> Result<String, String> {
    let profile = profiles::list(&app)?
        .into_iter()
        .find(|p| p.id == id)
        .ok_or_else(|| format!("профиль {id} не найден"))?;
    let out = match format.as_str() {
        "json" => crate::export::to_json(&profile.outbound),
        _ => crate::export::to_share(&profile.name, &profile.outbound),
    };
    Ok(out)
}

/// Дублирует профиль по id (новый id, имя «… (копия)»).
#[tauri::command]
pub fn duplicate_profile(app: tauri::AppHandle, id: String) -> Result<Profile, String> {
    let mut all = profiles::list(&app)?;
    let src = all
        .iter()
        .find(|p| p.id == id)
        .ok_or_else(|| format!("профиль {id} не найден"))?
        .clone();
    let copy = Profile {
        id: Uuid::new_v4().to_string(),
        name: format!("{} (копия)", src.name),
        outbound: src.outbound,
        subscription_id: None,
    };
    all.push(copy.clone());
    profiles::save_all(&app, &all)?;
    Ok(copy)
}
