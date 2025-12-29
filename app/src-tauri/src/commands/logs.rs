//! Tauri commands for request logging.

use crate::request_log::{
    RequestLog, RequestLogStore, RequestLogsRetentionConfig, RequestLogsRetentionMode,
};
use chrono::Duration as ChronoDuration;
use tauri::{AppHandle, Manager};

#[cfg(desktop)]
fn get_setting_from_store<T: serde::de::DeserializeOwned>(
    app: &AppHandle,
    key: &str,
    default: T,
) -> T {
    use tauri_plugin_store::StoreExt;
    app.store("settings.json")
        .ok()
        .and_then(|store| store.get(key))
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or(default)
}

#[cfg(desktop)]
fn read_request_logs_retention(app: &AppHandle) -> RequestLogsRetentionConfig {
    let mode: String = get_setting_from_store(app, "request_logs_retention_mode", "amount".into());
    let amount: u64 = get_setting_from_store(app, "request_logs_retention_amount", 50u64);
    let days: u64 = get_setting_from_store(app, "request_logs_retention_days", 7u64);

    let mode = if mode == "time" {
        RequestLogsRetentionMode::Time
    } else {
        RequestLogsRetentionMode::Amount
    };

    let time_retention = if days == 0 {
        None
    } else {
        Some(ChronoDuration::days(days as i64))
    };

    RequestLogsRetentionConfig {
        mode,
        amount: amount.max(1).min(1000) as usize,
        time_retention,
    }
}

#[cfg(not(desktop))]
fn read_request_logs_retention(_app: &AppHandle) -> RequestLogsRetentionConfig {
    RequestLogsRetentionConfig::default()
}

/// Get all request logs
#[tauri::command]
pub fn get_request_logs(app: AppHandle, limit: Option<usize>) -> Vec<RequestLog> {
    if let Some(store) = app.try_state::<RequestLogStore>() {
        store.set_retention(read_request_logs_retention(&app));
        store.get_logs(limit)
    } else {
        Vec::new()
    }
}

/// Clear all request logs
#[tauri::command]
pub fn clear_request_logs(app: AppHandle) {
    if let Some(store) = app.try_state::<RequestLogStore>() {
        store.clear();
    }
}
