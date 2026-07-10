//! Живая статистика трафика через Clash API sing-box.
//!
//! Пока подключение активно — раз в секунду опрашиваем `/connections`,
//! берём суммарные байты, считаем скорость как разницу и шлём событие `traffic`.

use crate::config::CLASH_API_PORT;
use crate::connection;
use serde::Serialize;
use serde_json::Value;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

const EVENT: &str = "traffic";

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Traffic {
    /// Скорость отдачи, байт/с.
    pub up: u64,
    /// Скорость загрузки, байт/с.
    pub down: u64,
    /// Суммарно отдано за сессию, байт.
    pub up_total: u64,
    /// Суммарно загружено за сессию, байт.
    pub down_total: u64,
}

/// Запускает фоновый опрос статистики; живёт, пока состояние == Connected.
pub fn start_polling(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let client = reqwest::Client::new();
        let url = format!("http://127.0.0.1:{CLASH_API_PORT}/connections");
        let mut last: Option<(u64, u64)> = None;

        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;
            if !connection::is_connected(&app) {
                break;
            }

            let value: Option<Value> = match client.get(&url).send().await {
                Ok(resp) => resp.json().await.ok(),
                Err(_) => None,
            };
            let Some(v) = value else { continue };

            let up_total = v.get("uploadTotal").and_then(Value::as_u64).unwrap_or(0);
            let down_total = v.get("downloadTotal").and_then(Value::as_u64).unwrap_or(0);
            let (up, down) = match last {
                Some((lu, ld)) => (up_total.saturating_sub(lu), down_total.saturating_sub(ld)),
                None => (0, 0),
            };
            last = Some((up_total, down_total));

            let _ = app.emit(
                EVENT,
                Traffic {
                    up,
                    down,
                    up_total,
                    down_total,
                },
            );
        }

        // обнулить индикатор после остановки
        let _ = app.emit(EVENT, Traffic::default());
    });
}
