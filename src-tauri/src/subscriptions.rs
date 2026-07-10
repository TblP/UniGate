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
    let client = reqwest::Client::builder()
        .user_agent("UniGate")
        .build()
        .map_err(|e| format!("http-клиент: {e}"))?;
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("не удалось загрузить подписку: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("подписка вернула статус {}", resp.status()));
    }
    resp.text()
        .await
        .map_err(|e| format!("не удалось прочитать подписку: {e}"))
}

/// Разбирает тело подписки в список `(имя, outbound)`; нераспознанные строки пропускает.
pub fn parse_list(body: &str) -> Vec<(String, Outbound)> {
    decode_maybe_base64(body)
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .filter_map(|l| import::parse(l).ok())
        .collect()
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
                    # комментарий (не ссылка)\n";
        let list = parse_list(body);
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].0, "A");
        assert_eq!(list[1].0, "B");
    }

    #[test]
    fn parse_base64_list() {
        let inner = "vless://11111111-1111-1111-1111-111111111111@1.2.3.4:443?security=tls&sni=ex.com&type=tcp#Node1\nss://YWVzLTI1Ni1nY206cGFzcw@1.2.3.4:8388#Node2";
        let b64 = general_purpose::STANDARD.encode(inner);
        let list = parse_list(&b64);
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].0, "Node1");
        assert_eq!(list[1].0, "Node2");
    }
}
