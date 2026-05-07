//! Tauri commands for request logging.

use crate::request_log::{
    strip_request_log_text_and_payloads, RequestLog, RequestLogStore, RequestLogsRetentionConfig,
};
use std::path::Path;
use tauri::{AppHandle, Manager};

#[cfg(desktop)]
use crate::settings::store::SettingsReadMode;
#[cfg(desktop)]
use crate::settings_view;

#[cfg(desktop)]
fn read_request_logs_retention(app: &AppHandle) -> RequestLogsRetentionConfig {
    settings_view::read_request_logs_retention(app, SettingsReadMode::Fresh)
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

/// Allow the frontend to write structured log entries into the Rust log stream.
///
/// This is the primary way to get frontend logs into the rolling log files
/// (DevTools is unavailable in production builds). The level parameter accepts
/// "error", "warn", "info", "debug" (default: "debug").
#[tauri::command]
pub fn frontend_log(level: Option<String>, scope: String, message: String) {
    let lvl = level.as_deref().unwrap_or("debug");
    match lvl {
        "error" => log::error!("[ui:{}] {}", scope, message),
        "warn" => log::warn!("[ui:{}] {}", scope, message),
        "info" => log::info!("[ui:{}] {}", scope, message),
        _ => log::debug!("[ui:{}] {}", scope, message),
    }
}

// ---------------------------------------------------------------------------
// App trace logs (file-based rolling logs written by tracing)
// ---------------------------------------------------------------------------

/// Return the directory containing rolling app log files, or `null` if file
/// logging couldn't be initialised (missing `%APPDATA%`, etc.).
#[tauri::command]
pub fn get_app_logs_dir() -> Option<String> {
    crate::tracing_init::log_dir().map(|p| p.to_string_lossy().into_owned())
}

/// Open the app logs directory in the system file explorer.
#[tauri::command]
pub fn open_app_logs_folder() -> Result<(), String> {
    let dir = crate::tracing_init::log_dir()
        .ok_or_else(|| "File logging is not active; log directory unavailable.".to_string())?;

    open::that(dir).map_err(|e| format!("Failed to open logs folder: {e}"))
}

/// Send a deterministic backend smoke event to Sentry.
///
/// Returns `true` when an event was queued (i.e., backend Sentry is configured).
#[tauri::command]
pub fn sentry_backend_smoke_test(surface: Option<String>) -> bool {
    crate::sentry_init::capture_backend_smoke(surface.as_deref().unwrap_or("tauri-command"))
}
