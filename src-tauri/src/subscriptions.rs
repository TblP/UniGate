//! Подписки: хранилище + загрузка (reqwest) и парсинг списка серверов.
//!
//! Тело подписки — обычно base64 со списком share-ссылок (по одной на строку),
//! либо тот же список открытым текстом. Каждую строку парсим через [`crate::import`].

use crate::models::{Outbound, Subscription};
use crate::{import, storage};
use base64::{engine::general_purpose, Engine};
use tauri::AppHandle;

const FILE: &str = "subscriptions.json";

pub fn list(app: &AppHandle) -> Result<Vec<Subscription>, String> {
    storage::read_json(app, FILE)
}

pub fn save_all(app: &AppHandle, subs: &[Subscription]) -> Result<(), String> {
    storage::write_json(app, FILE, subs)
}

/// Скачивает тело подписки по URL.
pub async fn fetch(url: &str) -> Result<String, String> {
    const MAX_BYTES: usize = 2 * 1024 * 1024;
    let url = reqwest::Url::parse(url).map_err(|_| "неверный URL подписки")?;
    if url.scheme() != "https" || url.host_str().is_none()
        || !url.username().is_empty() || url.password().is_some()
    {
        return Err("для подписки нужна HTTPS-ссылка без логина и пароля в адресе".into());
    }
    let client = reqwest::Client::builder()
        .user_agent("UniGate")
        .https_only(true)
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|_| "не удалось создать HTTP-клиент")?;
    let mut resp = client
        .get(url)
        .send()
        .await
        .map_err(|_| "не удалось загрузить подписку")?;
    if !resp.status().is_success() {
        return Err(format!("подписка вернула статус {}", resp.status()));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = resp.chunk().await.map_err(|_| "не удалось прочитать подписку")? {
        if bytes.len().saturating_add(chunk.len()) > MAX_BYTES {
            return Err("ответ подписки слишком большой".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    String::from_utf8(bytes).map_err(|_| "ответ подписки не в формате UTF-8".into())
}

/// Разбирает тело подписки в список `(имя, outbound)`; нераспознанные строки пропускает.
pub fn parse_list(body: &str) -> (Vec<(String, Outbound)>, usize) {
    let mut servers = Vec::new();
    let mut skipped = 0;
    for line in decode_maybe_base64(body).trim_start_matches('\u{feff}').lines().map(str::trim) {
        if line.is_empty() || line.starts_with('#') || line.starts_with("//") { continue; }
        if !line.split_once("://").is_some_and(|(scheme, _)| {
            !scheme.is_empty() && scheme.bytes().all(|c| c.is_ascii_alphanumeric() || b"+.-".contains(&c))
        }) {
            skipped += 1;
            continue;
        }
        match import::parse(line) {
            Ok(server) => servers.push(server),
            Err(_) => skipped += 1,
        }
    }
    (servers, skipped)
}

/// Если всё тело — base64 со ссылками, декодирует; иначе возвращает как есть.
fn decode_maybe_base64(body: &str) -> String {
    let compact: String = body.chars().filter(|c| !c.is_whitespace()).collect();
    let stripped = compact.trim_end_matches('=');
    let decoded = general_purpose::STANDARD_NO_PAD
        .decode(stripped)
        .or_else(|_| general_purpose::URL_SAFE_NO_PAD.decode(stripped))
        .ok()
        .and_then(|b| String::from_utf8(b).ok());
    match decoded {
        Some(text) if text.contains("://") => text,
        _ => body.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_plain_list() {
        let body = "hysteria2://pw@1.2.3.4:443?sni=ex.com#A\n\
                    trojan://secret@ex.com:443?sni=ex.com#B\n\
                    # комментарий (не ссылка)\n\
                    unsupported://one\n";
        let (list, skipped) = parse_list(body);
        assert_eq!(list.len(), 2);
        assert_eq!(skipped, 1);
        assert_eq!(list[0].0, "A");
        assert_eq!(list[1].0, "B");
    }

    #[test]
    fn parse_base64_list() {
        let inner = "vless://11111111-1111-1111-1111-111111111111@1.2.3.4:443?security=tls&sni=ex.com&type=tcp#Node1\nss://YWVzLTI1Ni1nY206cGFzcw@1.2.3.4:8388#Node2";
        let b64 = general_purpose::STANDARD.encode(inner);
        let (list, skipped) = parse_list(&b64);
        assert_eq!(list.len(), 2);
        assert_eq!(skipped, 0);
        assert_eq!(list[0].0, "Node1");
        assert_eq!(list[1].0, "Node2");
    }
}
