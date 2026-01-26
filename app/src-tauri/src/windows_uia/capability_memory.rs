use serde::{Deserialize, Serialize};

#[cfg(desktop)]
use crate::settings::store::{get_settings_store, SettingsReadMode};
use crate::windows_uia::types::WindowsInsertMethod;

const SETTINGS_KEY: &str = "windows_app_capability_memory";

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct WindowsAppCapabilityStats {
    pub uia_value_pattern_success: u64,
    pub uia_value_pattern_fail: u64,
    pub paste_success: u64,
    pub paste_fail: u64,
    pub typing_success: u64,
    pub typing_fail: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct WindowsAppCapabilityEntry {
    pub last_seen_at_ms: u64,
    pub prefer_method: Option<String>,
    pub stats: WindowsAppCapabilityStats,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct WindowsAppCapabilityMemory {
    pub version: u32,
    pub apps: std::collections::HashMap<String, WindowsAppCapabilityEntry>,
}

pub fn record_insertion_result(
    memory: &mut WindowsAppCapabilityMemory,
    app_key: &str,
    method: WindowsInsertMethod,
    success: bool,
    now_ms: u64,
) {
    let entry = memory
        .apps
        .entry(app_key.to_string())
        .or_insert_with(WindowsAppCapabilityEntry::default);

    entry.last_seen_at_ms = now_ms;
    entry.prefer_method = Some(format!("{:?}", method).to_lowercase());

    match (method, success) {
        (WindowsInsertMethod::UiaValuePattern, true) => {
            entry.stats.uia_value_pattern_success += 1;
        }
        (WindowsInsertMethod::UiaValuePattern, false) => {
            entry.stats.uia_value_pattern_fail += 1;
        }
        (WindowsInsertMethod::Paste, true) => {
            entry.stats.paste_success += 1;
        }
        (WindowsInsertMethod::Paste, false) => {
            entry.stats.paste_fail += 1;
        }
        (WindowsInsertMethod::Typing, true) => {
            entry.stats.typing_success += 1;
        }
        (WindowsInsertMethod::Typing, false) => {
            entry.stats.typing_fail += 1;
        }
        (WindowsInsertMethod::None, _) => {}
    }
}

#[cfg(desktop)]
pub fn load_capability_memory(
    app: &tauri::AppHandle,
) -> Result<WindowsAppCapabilityMemory, String> {
    let Some(store) = get_settings_store(app, SettingsReadMode::Cached) else {
        return Ok(WindowsAppCapabilityMemory::default());
    };

    let Some(raw) = store.get(SETTINGS_KEY) else {
        return Ok(WindowsAppCapabilityMemory::default());
    };
    if raw.is_null() {
        return Ok(WindowsAppCapabilityMemory::default());
    }

    serde_json::from_value(raw).map_err(|err| format!("Failed to parse {SETTINGS_KEY}: {err}"))
}

#[cfg(desktop)]
pub fn save_capability_memory(
    app: &tauri::AppHandle,
    memory: &WindowsAppCapabilityMemory,
) -> Result<(), String> {
    let Some(store) = get_settings_store(app, SettingsReadMode::Cached) else {
        return Err("Failed to load settings store".to_string());
    };

    let value = serde_json::to_value(memory)
        .map_err(|err| format!("Failed to serialize {SETTINGS_KEY}: {err}"))?;
    store.set(SETTINGS_KEY, value);
    store
        .save()
        .map_err(|err| format!("Failed to save {SETTINGS_KEY}: {err}"))
}

#[cfg(not(desktop))]
pub fn load_capability_memory(
    _app: &tauri::AppHandle,
) -> Result<WindowsAppCapabilityMemory, String> {
    Ok(WindowsAppCapabilityMemory::default())
}

#[cfg(not(desktop))]
pub fn save_capability_memory(
    _app: &tauri::AppHandle,
    _memory: &WindowsAppCapabilityMemory,
) -> Result<(), String> {
    Ok(())
}
