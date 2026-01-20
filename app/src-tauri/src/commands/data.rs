//! Tauri commands for destructive data operations (danger zone).

use std::fs;

use tauri::{AppHandle, Emitter, Manager};

use schemars::JsonSchema;
use serde::Serialize;

use crate::events;
use crate::history::HistoryStorage;
use crate::recordings::RecordingStore;
use crate::request_log::RequestLogStore;
use crate::stats::StatsStore;

#[cfg(desktop)]
use tauri_plugin_store::StoreExt;

#[cfg(desktop)]
use crate::secrets::API_KEY_SETTING_KEYS;

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct DataStorageSummary {
    pub recordings_count: u64,
    pub recordings_bytes: u64,

    pub history_count: u64,
    pub history_bytes: u64,

    pub request_logs_count: u64,

    pub stats_files_count: u64,
    pub stats_bytes: u64,

    pub settings_bytes: u64,
    pub api_keys_set_count: u64,
}

#[cfg(desktop)]
fn file_size_bytes(path: &std::path::Path) -> u64 {
    std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

/// Get a lightweight summary of what's currently stored on disk/in memory.
///
/// Useful for the Settings -> Data "Danger zone" UI so users can see what would be deleted.
#[cfg(desktop)]
#[tauri::command]
pub fn get_data_storage_summary(app: AppHandle) -> Result<DataStorageSummary, String> {
    // Recordings
    let (recordings_count, recordings_bytes) = if let Some(recs) = app.try_state::<RecordingStore>()
    {
        match recs.stats() {
            Ok(s) => (s.count, s.bytes),
            Err(_) => (0, 0),
        }
    } else {
        (0, 0)
    };

    // History
    let (history_count, history_bytes) = if let Some(history) = app.try_state::<HistoryStorage>() {
        let count = history.count().unwrap_or(0) as u64;
        let bytes = history.file_size_bytes();
        (count, bytes)
    } else {
        (0, 0)
    };

    // Request logs (in-memory)
    let request_logs_count = app
        .try_state::<RequestLogStore>()
        .map(|logs| logs.count() as u64)
        .unwrap_or(0);

    // Persisted stats
    let (stats_files_count, stats_bytes) = if let Some(stats) = app.try_state::<StatsStore>() {
        let mut files: u64 = 0;
        let mut bytes: u64 = 0;
        if let Ok(entries) = std::fs::read_dir(stats.dir()) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }

                let name = entry.file_name();
                let name = name.to_string_lossy();
                if !name.starts_with("cost-events-") || !name.ends_with(".jsonl") {
                    continue;
                }

                files = files.saturating_add(1);
                bytes = bytes.saturating_add(entry.metadata().map(|m| m.len()).unwrap_or(0));
            }
        }
        (files, bytes)
    } else {
        (0, 0)
    };

    // Settings + API keys (store)
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to resolve app data dir: {e}"))?;
    let settings_path = app_data_dir.join("settings.json");
    let settings_bytes = file_size_bytes(&settings_path);

    let store = app.store("settings.json").map_err(|e| e.to_string())?;
    let mut api_keys_set_count: u64 = 0;
    let mut unique_keys: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for key in API_KEY_SETTING_KEYS {
        if !unique_keys.insert(key) {
            continue;
        }

        // Prefer secure storage, but allow legacy store fallback for users who
        // haven't successfully migrated yet.
        if crate::secrets::has_api_key(&app, key) {
            api_keys_set_count = api_keys_set_count.saturating_add(1);
            continue;
        }

        // If secure storage isn't available (or migration failed), fall back to
        // checking the raw store to keep the UI summary accurate.
        let Some(v) = store.get(key) else { continue };
        if let Ok(s) = serde_json::from_value::<String>(v) {
            if !s.trim().is_empty() {
                api_keys_set_count = api_keys_set_count.saturating_add(1);
            }
        }
    }

    Ok(DataStorageSummary {
        recordings_count,
        recordings_bytes,
        history_count,
        history_bytes,
        request_logs_count,
        stats_files_count,
        stats_bytes,
        settings_bytes,
        api_keys_set_count,
    })
}

#[cfg(not(desktop))]
#[tauri::command]
pub fn get_data_storage_summary(_app: AppHandle) -> Result<DataStorageSummary, String> {
    Ok(DataStorageSummary {
        recordings_count: 0,
        recordings_bytes: 0,
        history_count: 0,
        history_bytes: 0,
        request_logs_count: 0,
        stats_files_count: 0,
        stats_bytes: 0,
        settings_bytes: 0,
        api_keys_set_count: 0,
    })
}

/// Delete all configured API keys from the settings store.
///
/// This removes known `*_api_key` keys.
#[cfg(desktop)]
#[tauri::command]
pub fn delete_all_api_keys(app: AppHandle) -> Result<(), String> {
    // Delete from secure storage (and any legacy store copies).
    let mut unique_keys: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for key in API_KEY_SETTING_KEYS {
        if !unique_keys.insert(key) {
            continue;
        }
        let _ = crate::secrets::clear_api_key(&app, key);
    }

    // Best-effort: sync pipeline so provider availability updates immediately.
    let _ = crate::commands::config::sync_pipeline_config(app);

    Ok(())
}

#[cfg(not(desktop))]
#[tauri::command]
pub fn delete_all_api_keys(_app: AppHandle) -> Result<(), String> {
    Ok(())
}

/// Delete all settings by removing the store file and re-seeding defaults.
///
/// NOTE: This does not delete recordings/history/stats on disk.
#[cfg(desktop)]
#[tauri::command]
pub fn delete_all_settings(app: AppHandle) -> Result<(), String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to resolve app data dir: {e}"))?;

    let settings_path = app_data_dir.join("settings.json");
    if settings_path.exists() {
        fs::remove_file(&settings_path).map_err(|e| {
            format!(
                "Failed to delete settings file {}: {}",
                settings_path.display(),
                e
            )
        })?;
    }

    // Recreate the store + seed defaults so UI/backend agree.
    crate::settings::defaults::ensure_default_settings(&app)
        .map_err(|e: Box<dyn std::error::Error>| e.to_string())?;

    // Best-effort: sync pipeline so it uses the new defaults.
    let _ = crate::commands::config::sync_pipeline_config(app);

    Ok(())
}

#[cfg(not(desktop))]
#[tauri::command]
pub fn delete_all_settings(_app: AppHandle) -> Result<(), String> {
    Ok(())
}

/// Delete all persisted usage/cost stats (JSONL shards) from disk.
#[cfg(desktop)]
#[tauri::command]
pub fn delete_all_stats(app: AppHandle) -> Result<(), String> {
    if let Some(stats) = app.try_state::<StatsStore>() {
        let dir = stats.dir().to_path_buf();
        if dir.exists() {
            fs::remove_dir_all(&dir)
                .map_err(|e| format!("Failed to delete stats dir {}: {}", dir.display(), e))?;
        }
        fs::create_dir_all(&dir)
            .map_err(|e| format!("Failed to recreate stats dir {}: {}", dir.display(), e))?;
    }

    Ok(())
}

#[cfg(not(desktop))]
#[tauri::command]
pub fn delete_all_stats(_app: AppHandle) -> Result<(), String> {
    Ok(())
}

/// Delete *all data* (superset): history, recordings, request logs, persisted stats, and settings.
#[cfg(desktop)]
#[tauri::command]
pub fn delete_all_data(app: AppHandle) -> Result<(), String> {
    // 1) History
    if let Some(history) = app.try_state::<HistoryStorage>() {
        let _ = history.clear();
        let _ = app.emit(events::EVENT_HISTORY_CHANGED, ());
    }

    // 2) Request logs
    if let Some(logs) = app.try_state::<RequestLogStore>() {
        logs.clear();
    }

    // 3) Recordings
    if let Some(recs) = app.try_state::<RecordingStore>() {
        let _ = recs.delete_all_wavs();
    }

    // 4) Persisted stats (usage/cost)
    if let Some(stats) = app.try_state::<StatsStore>() {
        let dir = stats.dir().to_path_buf();
        if dir.exists() {
            let _ = fs::remove_dir_all(&dir);
        }
        let _ = fs::create_dir_all(&dir);
    }

    // 5) Settings (includes API keys)
    delete_all_settings(app)?;

    Ok(())
}

#[cfg(not(desktop))]
#[tauri::command]
pub fn delete_all_data(_app: AppHandle) -> Result<(), String> {
    Ok(())
}
