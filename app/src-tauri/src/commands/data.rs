//! Tauri commands for destructive data operations (danger zone).

use std::fs;

use tauri::{AppHandle, Emitter, Manager};

use schemars::JsonSchema;
use serde::Serialize;

use crate::commands::CommandResult;
use crate::events;
use crate::history::HistoryStorage;
use crate::recordings::RecordingStore;
use crate::request_log::RequestLogStore;
use crate::stats::StatsStore;

#[cfg(desktop)]
use crate::settings::store::{get_settings_store_or_err, SettingsReadMode};

#[cfg(desktop)]
use crate::secrets::API_KEY_SETTING_KEYS;

#[cfg(desktop)]
fn emit_settings_changed(app: &AppHandle, payload: crate::SettingsChangedPayload) {
    let _ = app.emit(events::EVENT_SETTINGS_CHANGED, payload);
}

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
pub fn get_data_storage_summary(app: AppHandle) -> CommandResult<DataStorageSummary> {
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

    let store = get_settings_store_or_err(&app, SettingsReadMode::Fresh)?;
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
pub fn get_data_storage_summary(_app: AppHandle) -> CommandResult<DataStorageSummary> {
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
pub fn delete_all_api_keys(app: AppHandle) -> CommandResult<()> {
    // Delete from secure storage (and any legacy store copies).
    let mut unique_keys: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for key in API_KEY_SETTING_KEYS {
        if !unique_keys.insert(key) {
            continue;
        }
        let _ = crate::secrets::clear_api_key(&app, key);
    }

    // Notify other windows/UI to refresh derived provider availability.
    let mut payload = crate::SettingsChangedPayload::new();
    payload.insert(
        "api_keys_changed".to_string(),
        serde_json::Value::Bool(true),
    );
    emit_settings_changed(&app, payload);

    // Best-effort: sync pipeline so provider availability updates immediately.
    let _ = crate::commands::config::sync_pipeline_config(app);

    Ok(())
}

#[cfg(not(desktop))]
#[tauri::command]
pub fn delete_all_api_keys(_app: AppHandle) -> CommandResult<()> {
    Ok(())
}

/// Delete all settings by removing the store file and re-seeding defaults.
///
/// NOTE: This does not delete recordings/history/stats on disk.
#[cfg(desktop)]
#[tauri::command]
pub fn delete_all_settings(app: AppHandle) -> CommandResult<()> {
    // Settings reset should also clear secrets so the UI message "including API keys"
    // is actually true even when secure storage is enabled.
    let _ = delete_all_api_keys(app.clone());

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

    // Let UI/other windows know settings were reset (best-effort).
    let mut payload = crate::SettingsChangedPayload::new();
    payload.insert("settings_reset".to_string(), serde_json::Value::Bool(true));
    emit_settings_changed(&app, payload);

    // Best-effort: sync pipeline so it uses the new defaults.
    let _ = crate::commands::config::sync_pipeline_config(app);

    Ok(())
}

#[cfg(not(desktop))]
#[tauri::command]
pub fn delete_all_settings(_app: AppHandle) -> CommandResult<()> {
    Ok(())
}

/// Delete all persisted usage/cost stats (JSONL shards) from disk.
#[cfg(desktop)]
#[tauri::command]
pub fn delete_all_stats(app: AppHandle) -> CommandResult<()> {
    if let Some(stats) = app.try_state::<StatsStore>() {
        let dir = stats.dir().to_path_buf();
        let mut did_delete_any = false;
        if dir.exists() {
            fs::remove_dir_all(&dir)
                .map_err(|e| format!("Failed to delete stats dir {}: {}", dir.display(), e))?;
            did_delete_any = true;
        }
        fs::create_dir_all(&dir)
            .map_err(|e| format!("Failed to recreate stats dir {}: {}", dir.display(), e))?;

        if did_delete_any {
            let _ = app.emit(events::EVENT_STATS_CHANGED, ());
        }
    }

    Ok(())
}

/// Delete all transcript text from history while keeping recording links.
#[cfg(desktop)]
#[tauri::command]
pub fn delete_all_transcripts_keep_recordings(app: AppHandle) -> CommandResult<u64> {
    let mut updated: u64 = 0;
    if let Some(history) = app.try_state::<HistoryStorage>() {
        updated = history
            .clear_all_transcript_text_keep_recordings()
            .unwrap_or(0) as u64;
        if updated > 0 {
            let _ = app.emit(events::EVENT_HISTORY_CHANGED, ());
        }
    }

    Ok(updated)
}

#[cfg(not(desktop))]
#[tauri::command]
pub fn delete_all_transcripts_keep_recordings(_app: AppHandle) -> CommandResult<u64> {
    Ok(0)
}

#[cfg(not(desktop))]
#[tauri::command]
pub fn delete_all_stats(_app: AppHandle) -> CommandResult<()> {
    Ok(())
}

/// Delete *all data* (superset): history, recordings, request logs, persisted stats, and settings.
#[cfg(desktop)]
#[tauri::command]
pub fn delete_all_data(app: AppHandle) -> CommandResult<()> {
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
        let _ = app.emit(events::EVENT_STATS_CHANGED, ());
    }

    // 5) Settings (includes API keys)
    delete_all_settings(app)?;

    Ok(())
}

#[cfg(not(desktop))]
#[tauri::command]
pub fn delete_all_data(_app: AppHandle) -> CommandResult<()> {
    Ok(())
}
