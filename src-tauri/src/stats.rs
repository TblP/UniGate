//! Живая статистика трафика через локальные API sing-box и Xray.
//!
//! Пока подключение активно — раз в секунду опрашиваем `/connections`,
//! берём суммарные байты, считаем скорость как разницу и шлём событие `traffic`.

use crate::config::{CLASH_API_PORT, XRAY_TRAFFIC_TAG};
use crate::connection;
use serde::Serialize;
use serde_json::Value;
use std::time::Duration;
use std::sync::atomic::{AtomicU64, Ordering};
use tauri::{AppHandle, Emitter};

const EVENT: &str = "traffic";
static POLL_SESSION: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy)]
pub enum Source {
    SingBox,
    Xray,
}

impl Source {
    fn path(self) -> &'static str {
        match self {
            Self::SingBox => "/connections",
            Self::Xray => "/debug/vars",
        }
    }

    fn totals(self, value: &Value) -> Option<(u64, u64)> {
        match self {
            Self::SingBox => Some((
                value.get("uploadTotal")?.as_u64()?,
                value.get("downloadTotal")?.as_u64()?,
            )),
            Self::Xray => {
                let outbound = value.get("stats")?.get("outbound")?;
                let counter = outbound.get(XRAY_TRAFFIC_TAG);
                Some((
                    counter.and_then(|v| v.get("uplink")).and_then(Value::as_u64).unwrap_or(0),
                    counter.and_then(|v| v.get("downlink")).and_then(Value::as_u64).unwrap_or(0),
                ))
            }
        }
    }
}

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
pub fn start_polling(
    app: AppHandle,
    #[cfg(not(target_os = "android"))] secret: String,
) {
    start_polling_for(
        app,
        Source::SingBox,
        #[cfg(not(target_os = "android"))]
        Some(secret),
    );
}

pub fn start_polling_for(
    app: AppHandle,
    source: Source,
    #[cfg(not(target_os = "android"))] secret: Option<String>,
) {
    let session = POLL_SESSION.fetch_add(1, Ordering::Relaxed).wrapping_add(1);
    tauri::async_runtime::spawn(async move {
        let Ok(client) = reqwest::Client::builder().no_proxy().timeout(Duration::from_secs(2)).build() else { return };
        let url = format!("http://127.0.0.1:{CLASH_API_PORT}{}", source.path());
        let mut last: Option<(u64, u64, std::time::Instant)> = None;

        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;
            if POLL_SESSION.load(Ordering::Relaxed) != session || !connection::is_connected(&app) {
                break;
            }

            let request = client.get(&url);
            #[cfg(target_os = "android")]
            let request = {
                let Some(secret) = connection::clash_api_secret(&app).await else {
                    continue;
                };
                request.bearer_auth(secret)
            };
            #[cfg(not(target_os = "android"))]
            let request = match &secret {
                Some(secret) => request.bearer_auth(secret),
                None => request,
            };
            let value: Option<Value> = match request.send().await {
                Ok(resp) if resp.status().is_success() => resp.json().await.ok(),
                _ => None,
            };
            if POLL_SESSION.load(Ordering::Relaxed) != session || !connection::is_connected(&app) {
                break;
            }
            let Some((up_total, down_total)) = value.as_ref().and_then(|v| source.totals(v)) else { continue };

            let now = std::time::Instant::now();
            let (up, down) = match last {
                Some((lu, ld, then)) => {
                    let seconds = now.duration_since(then).as_secs_f64().max(0.001);
                    ((up_total.saturating_sub(lu) as f64 / seconds) as u64,
                     (down_total.saturating_sub(ld) as f64 / seconds) as u64)
                }
                None => (0, 0),
            };
            last = Some((up_total, down_total, now));

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
        if POLL_SESSION.load(Ordering::Relaxed) == session {
            let _ = app.emit(EVENT, Traffic::default());
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn totals_from_both_cores() {
        assert_eq!(Source::SingBox.totals(&json!({"uploadTotal": 5, "downloadTotal": 40})), Some((5, 40)));
        let mut payload = json!({"stats": {"outbound": {"direct": {"uplink": 900, "downlink": 900}}}});
        assert_eq!(Source::Xray.totals(&payload), Some((0, 0)));
        payload["stats"]["outbound"][XRAY_TRAFFIC_TAG] = json!({"uplink": 7, "downlink": 80});
        assert_eq!(Source::Xray.totals(&payload), Some((7, 80)));
        assert_eq!(Source::Xray.totals(&json!({})), None);
    }
}
