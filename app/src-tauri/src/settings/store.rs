//! Helpers for reading `settings.json` consistently.
//!
//! Tauri's store plugin keeps an in-memory cache per store instance.
//! Some call sites need "fresh read" semantics (reload from disk) to avoid
//! stale values after settings are changed by the UI or other windows.

#[cfg(desktop)]
use std::sync::Arc;

#[cfg(desktop)]
use tauri::AppHandle;

#[cfg(desktop)]
use tauri::Wry;

#[cfg(desktop)]
use tauri_plugin_store::{Store, StoreExt};

/// Whether to use cached store values or reload from disk before reading.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsReadMode {
    /// Use the cached store instance as-is.
    Cached,
    /// Best-effort reload the store from disk before reading.
    Fresh,
}

#[cfg(desktop)]
pub type SettingsStore = Store<Wry>;

/// Get the settings store.
///
/// - Returns `None` if the store cannot be loaded.
/// - If `mode` is `Fresh`, this will best-effort reload from disk to avoid
///   stale reads (the store is cached across calls).
#[cfg(desktop)]
pub fn get_settings_store(app: &AppHandle, mode: SettingsReadMode) -> Option<Arc<SettingsStore>> {
    let store = app.store("settings.json").ok()?;
    if mode == SettingsReadMode::Fresh {
        let _ = store.reload();
    }
    Some(store)
}

/// Same as `get_settings_store`, but returns a string error (handy for commands).
#[cfg(desktop)]
pub fn get_settings_store_or_err(
    app: &AppHandle,
    mode: SettingsReadMode,
) -> Result<Arc<SettingsStore>, String> {
    get_settings_store(app, mode).ok_or_else(|| "Failed to load settings store".to_string())
}

#[cfg(not(desktop))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsReadMode {
    Cached,
    Fresh,
}

#[cfg(not(desktop))]
pub fn get_settings_store(_app: &tauri::AppHandle, _mode: SettingsReadMode) -> Option<()> {
    None
}

#[cfg(not(desktop))]
pub fn get_settings_store_or_err(
    _app: &tauri::AppHandle,
    _mode: SettingsReadMode,
) -> Result<(), String> {
    Err("settings store not available".to_string())
}
