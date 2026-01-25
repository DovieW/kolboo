//! Tauri commands for request logging.

use crate::request_log::{
    strip_request_log_text_and_payloads, RequestLog, RequestLogStore, RequestLogsRetentionConfig,
    RequestLogsRetentionMode,
};
use chrono::Duration as ChronoDuration;
use std::path::Path;
use tauri::{AppHandle, Manager};

#[cfg(desktop)]
use crate::settings::store::get_fresh_settings_store;

#[cfg(desktop)]
fn get_setting_from_store<T: serde::de::DeserializeOwned>(
    app: &AppHandle,
    key: &str,
    default: T,
) -> T {
    get_fresh_settings_store(app)
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
        amount: amount.clamp(1, 200) as usize,
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

/// Export request logs to a JSON file.
///
/// The store is in-memory; this is intended for debugging and bug reports.
#[tauri::command]
pub fn export_request_logs_to_file(
    app: AppHandle,
    path: String,
    limit: Option<usize>,
    strip_text_and_payloads: bool,
) -> Result<(), String> {
    let Some(store) = app.try_state::<RequestLogStore>() else {
        return Err("Request log store not available".to_string());
    };

    store.set_retention(read_request_logs_retention(&app));

    let mut logs = store.get_logs(limit);
    if strip_text_and_payloads {
        logs = logs
            .into_iter()
            .map(strip_request_log_text_and_payloads)
            .collect();
    }

    let json = serde_json::to_string_pretty(&logs)
        .map_err(|e| format!("Failed to serialize logs: {e}"))?;

    let p = Path::new(&path);
    if let Some(parent) = p.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create export directory: {e}"))?;
        }
    }

    std::fs::write(p, json).map_err(|e| format!("Failed to write export file: {e}"))?;
    Ok(())
}

/// Dev-only: allow the frontend to write structured debug notes into the Rust log stream.
///
/// This is useful when diagnosing issues like overlay flicker where the window stays visible
/// but the webview UI appears to blink (no Rust-side show/hide calls fire).
#[tauri::command]
pub fn ui_debug_log(scope: String, message: String) {
    if cfg!(debug_assertions) {
        log::debug!("[ui:{}] {}", scope, message);
    }
}
