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

#[cfg(desktop)]
use serde_json::Value;

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

/// Convenience helper for the most common mode: best-effort fresh read.
#[cfg(desktop)]
pub fn get_fresh_settings_store(app: &AppHandle) -> Option<Arc<SettingsStore>> {
    get_settings_store(app, SettingsReadMode::Fresh)
}

/// Same as `get_settings_store`, but returns a string error (handy for commands).
#[cfg(desktop)]
pub fn get_settings_store_or_err(
    app: &AppHandle,
    mode: SettingsReadMode,
) -> Result<Arc<SettingsStore>, String> {
    get_settings_store(app, mode).ok_or_else(|| "Failed to load settings store".to_string())
}

// ---------------------------------------------------------------------------
// Typed settings helpers (defensive / tolerant)
// ---------------------------------------------------------------------------

#[cfg(desktop)]
fn coerce_u64(value: &Value) -> Option<u64> {
    match value {
        Value::Number(n) => n.as_u64().or_else(|| {
            n.as_f64().and_then(|f| {
                if f.is_finite() && f >= 0.0 {
                    Some(f as u64)
                } else {
                    None
                }
            })
        }),
        Value::String(s) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                return None;
            }

            // Prefer strict integer parse first (common case).
            if let Ok(v) = trimmed.parse::<u64>() {
                return Some(v);
            }

            // Fall back to float parse for values like "12.0".
            trimmed.parse::<f64>().ok().and_then(|f| {
                if f.is_finite() && f >= 0.0 {
                    Some(f as u64)
                } else {
                    None
                }
            })
        }
        _ => None,
    }
}

/// Read a setting as u64 (tolerant): accepts JSON numbers and strings.
///
/// - Missing/null/invalid values return `None`.
#[cfg(desktop)]
pub fn store_get_u64(store: &SettingsStore, key: &str) -> Option<u64> {
    store.get(key).as_ref().and_then(coerce_u64)
}

/// Read a u64 setting, apply a default, and clamp into `[min, max]`.
#[cfg(desktop)]
pub fn store_get_u64_clamped(
    store: &SettingsStore,
    key: &str,
    default: u64,
    min: u64,
    max: u64,
) -> u64 {
    store_get_u64(store, key).unwrap_or(default).clamp(min, max)
}

/// Convenience wrapper that reads from the settings store (cached or fresh).
#[cfg(desktop)]
pub fn get_u64_setting_clamped(
    app: &AppHandle,
    mode: SettingsReadMode,
    key: &str,
    default: u64,
    min: u64,
    max: u64,
) -> u64 {
    let Some(store) = get_settings_store(app, mode) else {
        return default.clamp(min, max);
    };
    store_get_u64_clamped(&store, key, default, min, max)
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
pub fn get_fresh_settings_store(_app: &tauri::AppHandle) -> Option<()> {
    None
}

#[cfg(not(desktop))]
pub fn get_settings_store_or_err(
    _app: &tauri::AppHandle,
    _mode: SettingsReadMode,
) -> Result<(), String> {
    Err("settings store not available".to_string())
}

#[cfg(all(test, desktop))]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn coerce_u64_accepts_numbers_and_strings() {
        assert_eq!(coerce_u64(&json!(123)), Some(123));
        assert_eq!(coerce_u64(&json!(12.0)), Some(12));
        assert_eq!(coerce_u64(&json!(12.9)), Some(12));
        assert_eq!(coerce_u64(&json!(" 42 ")), Some(42));
        assert_eq!(coerce_u64(&json!("12.0")), Some(12));
    }

    #[test]
    fn coerce_u64_rejects_negative_or_invalid() {
        assert_eq!(coerce_u64(&json!(-1)), None);
        assert_eq!(coerce_u64(&json!("")), None);
        assert_eq!(coerce_u64(&json!("nope")), None);
        assert_eq!(coerce_u64(&json!(null)), None);
    }
}
