use crate::events;
use crate::settings::HotkeyConfig;
use crate::SystemEvent;
use tauri::{AppHandle, Emitter, Manager};

#[cfg(desktop)]
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

#[cfg(desktop)]
use tauri_plugin_store::StoreExt;

use crate::settings::doctor::SettingsDoctorReport;
#[cfg(desktop)]
use crate::settings::doctor::{self, SETTINGS_DOCTOR_KEYS};
#[cfg(desktop)]
use std::collections::HashSet;

/// Update the backend runtime flag for hotkey debug events.
///
/// Why this exists:
/// The Windows modifier-only hotkey hook runs on a background thread and needs
/// a cheap, reliable way to know whether it should emit debug `system-event`s.
/// Reading from the store on every keypress is both expensive and can be stale
/// (multi-window store instances can lag behind the JS side).
#[cfg(desktop)]
#[tauri::command]
pub async fn set_hotkey_debug_enabled_runtime(app: AppHandle, enabled: bool) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        crate::windows_modifier_hotkeys::set_hotkey_debug_enabled(enabled);
    }

    let event = SystemEvent {
        timestamp: chrono::Utc::now().to_rfc3339(),
        event_type: "debug".to_string(),
        message: format!("Hotkey debug runtime enabled={}", enabled),
        details: Some("(confirmation event from backend)".to_string()),
    };

    let _ = app.emit(events::EVENT_SYSTEM_EVENT, event);
    Ok(())
}

// Stub for non-desktop platforms
#[cfg(not(desktop))]
#[tauri::command]
pub async fn set_hotkey_debug_enabled_runtime(
    _app: AppHandle,
    _enabled: bool,
) -> Result<(), String> {
    Ok(())
}

#[cfg(all(desktop, target_os = "windows"))]
fn is_windows_hook_handled_hotkey(hk: &HotkeyConfig) -> bool {
    // These are handled by a low-level Windows keyboard hook, not by
    // tauri-plugin-global-shortcut.
    hk.modifiers.is_empty() && matches!(hk.key.as_str(), "AltRight" | "Copilot")
}

/// Temporarily unregister all global shortcuts.
/// Call this before capturing a new hotkey to prevent the shortcuts from intercepting key presses.
#[cfg(desktop)]
#[tauri::command]
pub async fn unregister_shortcuts(app: AppHandle) -> Result<(), String> {
    let _guard = crate::shortcuts_lock::global_shortcut_lock().lock().await;
    log::info!("Temporarily unregistering all shortcuts for hotkey capture");

    // Prevent modifier-only hotkeys (Windows hook) from firing while the UI is capturing.
    #[cfg(target_os = "windows")]
    {
        crate::windows_modifier_hotkeys::set_enabled(false);
    }

    let shortcut_manager = app.global_shortcut();
    shortcut_manager
        .unregister_all()
        .map_err(|e| format!("Failed to unregister shortcuts: {}", e))?;
    Ok(())
}

// Stub for non-desktop platforms
#[cfg(not(desktop))]
#[tauri::command]
pub async fn unregister_shortcuts(_app: AppHandle) -> Result<(), String> {
    Ok(())
}

/// Read a hotkey setting from the store.
///
/// Semantics:
/// - missing key => use default
/// - explicit null => disabled (None)
/// - invalid value => use default
#[cfg(desktop)]
fn get_hotkey_from_store(
    app: &AppHandle,
    key: &str,
    default_fn: fn() -> Option<HotkeyConfig>,
) -> Option<HotkeyConfig> {
    use serde_json::Value;

    let raw = app
        .store("settings.json")
        .ok()
        .and_then(|store| store.get(key));

    match raw {
        None => default_fn(),
        Some(Value::Null) => None,
        Some(v) => serde_json::from_value::<HotkeyConfig>(v)
            .ok()
            .or_else(default_fn),
    }
}

/// Re-register global shortcuts with the current settings from the store.
/// Called from frontend after hotkey settings are changed.
/// Falls back to defaults if stored values are invalid.
#[cfg(desktop)]
#[tauri::command]
pub async fn register_shortcuts(app: AppHandle) -> Result<(), String> {
    let _guard = crate::shortcuts_lock::global_shortcut_lock().lock().await;

    // Hotkey capture ended; allow modifier-only hook hotkeys again.
    #[cfg(target_os = "windows")]
    {
        crate::windows_modifier_hotkeys::set_enabled(true);
    }

    // Read hotkeys from store.
    // - missing => default
    // - null => disabled
    // - invalid => default
    let toggle_hotkey =
        get_hotkey_from_store(&app, "toggle_hotkey", HotkeyConfig::default_toggle_opt);
    let hold_hotkey = get_hotkey_from_store(&app, "hold_hotkey", HotkeyConfig::default_hold);
    let paste_last_hotkey =
        get_hotkey_from_store(&app, "paste_last_hotkey", HotkeyConfig::default_paste_last);
    let retry_hotkey = get_hotkey_from_store(&app, "retry_hotkey", HotkeyConfig::default_retry);

    // Quick Ask hotkeys:
    // - Legacy key: quick_ask_hotkey (hold-to-record)
    // - New keys: quick_ask_hold_hotkey + quick_ask_toggle_hotkey
    // For backward compatibility, Quick Ask Hold falls back to the legacy key only
    // when the new key is absent (not when explicitly null).
    let (quick_ask_hold_hotkey, quick_ask_toggle_hotkey) = {
        use serde_json::Value;

        let store = app.store("settings.json").ok();
        let raw_hold = store.as_ref().and_then(|s| s.get("quick_ask_hold_hotkey"));

        let hold = match raw_hold {
            None => {
                get_hotkey_from_store(&app, "quick_ask_hotkey", HotkeyConfig::default_quick_ask)
            }
            Some(Value::Null) => None,
            Some(v) => serde_json::from_value::<HotkeyConfig>(v)
                .ok()
                .or_else(HotkeyConfig::default_quick_ask),
        };

        let toggle = get_hotkey_from_store(
            &app,
            "quick_ask_toggle_hotkey",
            HotkeyConfig::default_quick_ask,
        );

        (hold, toggle)
    };

    // Keep Windows hook behavior in sync with settings.
    #[cfg(target_os = "windows")]
    {
        let matches_copilot = |hk: &HotkeyConfig| hk.modifiers.is_empty() && hk.key == "Copilot";
        let matches_alt_right = |hk: &HotkeyConfig| hk.modifiers.is_empty() && hk.key == "AltRight";

        let copilot_enabled = toggle_hotkey.as_ref().is_some_and(matches_copilot)
            || hold_hotkey.as_ref().is_some_and(matches_copilot)
            || paste_last_hotkey.as_ref().is_some_and(matches_copilot)
            || retry_hotkey.as_ref().is_some_and(matches_copilot)
            || quick_ask_hold_hotkey.as_ref().is_some_and(matches_copilot)
            || quick_ask_toggle_hotkey
                .as_ref()
                .is_some_and(matches_copilot);

        let alt_right_enabled = toggle_hotkey.as_ref().is_some_and(matches_alt_right)
            || hold_hotkey.as_ref().is_some_and(matches_alt_right)
            || paste_last_hotkey.as_ref().is_some_and(matches_alt_right)
            || retry_hotkey.as_ref().is_some_and(matches_alt_right)
            || quick_ask_hold_hotkey
                .as_ref()
                .is_some_and(matches_alt_right)
            || quick_ask_toggle_hotkey
                .as_ref()
                .is_some_and(matches_alt_right);

        crate::windows_modifier_hotkeys::set_copilot_hotkey_enabled(copilot_enabled);
        crate::windows_modifier_hotkeys::set_alt_right_hotkey_enabled(alt_right_enabled);
    }

    log::info!(
        "Re-registering shortcuts - Toggle: {}, Hold: {}, PasteLast: {}, Retry: {}, QuickAskHold: {}, QuickAskToggle: {}",
        toggle_hotkey
            .as_ref()
            .map(|h| h.to_shortcut_string())
            .unwrap_or_else(|| "<disabled>".to_string()),
        hold_hotkey
            .as_ref()
            .map(|h| h.to_shortcut_string())
            .unwrap_or_else(|| "<disabled>".to_string()),
        paste_last_hotkey
            .as_ref()
            .map(|h| h.to_shortcut_string())
            .unwrap_or_else(|| "<disabled>".to_string()),
        retry_hotkey
            .as_ref()
            .map(|h| h.to_shortcut_string())
            .unwrap_or_else(|| "<disabled>".to_string()),
        quick_ask_hold_hotkey
            .as_ref()
            .map(|h| h.to_shortcut_string())
            .unwrap_or_else(|| "<disabled>".to_string())
        ,
        quick_ask_toggle_hotkey
            .as_ref()
            .map(|h| h.to_shortcut_string())
            .unwrap_or_else(|| "<disabled>".to_string())
    );

    // Get the global shortcut manager
    let shortcut_manager = app.global_shortcut();

    // Unregister all existing shortcuts
    shortcut_manager
        .unregister_all()
        .map_err(|e| format!("Failed to unregister shortcuts: {}", e))?;

    // Collect shortcuts to register
    let mut shortcuts: Vec<Shortcut> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    fn push_unique(shortcuts: &mut Vec<Shortcut>, seen: &mut HashSet<String>, sc: Shortcut) {
        let k = sc.to_string();
        if seen.insert(k) {
            shortcuts.push(sc);
        } else {
            log::warn!("Duplicate hotkey detected; skipping duplicate registration");
        }
    }
    if let Some(hk) = toggle_hotkey {
        #[cfg(all(desktop, target_os = "windows"))]
        if is_windows_hook_handled_hotkey(&hk) {
            // handled by Windows hook (not tauri-plugin-global-shortcut)
        } else {
            match hk.to_shortcut() {
                Ok(sc) => push_unique(&mut shortcuts, &mut seen, sc),
                Err(e) => log::warn!(
                    "Invalid toggle hotkey in settings store ({}); treating as disabled",
                    e
                ),
            }
        }

        #[cfg(not(all(desktop, target_os = "windows")))]
        {
            match hk.to_shortcut() {
                Ok(sc) => shortcuts.push(sc),
                Err(e) => log::warn!(
                    "Invalid toggle hotkey in settings store ({}); treating as disabled",
                    e
                ),
            }
        }
    }
    if let Some(hk) = hold_hotkey {
        #[cfg(all(desktop, target_os = "windows"))]
        if is_windows_hook_handled_hotkey(&hk) {
            // handled by Windows hook
        } else {
            match hk.to_shortcut() {
                Ok(sc) => push_unique(&mut shortcuts, &mut seen, sc),
                Err(e) => log::warn!(
                    "Invalid hold hotkey in settings store ({}); treating as disabled",
                    e
                ),
            }
        }

        #[cfg(not(all(desktop, target_os = "windows")))]
        match hk.to_shortcut() {
            Ok(sc) => shortcuts.push(sc),
            Err(e) => log::warn!(
                "Invalid hold hotkey in settings store ({}); treating as disabled",
                e
            ),
        }
    }
    if let Some(hk) = paste_last_hotkey {
        #[cfg(all(desktop, target_os = "windows"))]
        if is_windows_hook_handled_hotkey(&hk) {
            // handled by Windows hook
        } else {
            match hk.to_shortcut() {
                Ok(sc) => push_unique(&mut shortcuts, &mut seen, sc),
                Err(e) => log::warn!(
                    "Invalid paste-last hotkey in settings store ({}); treating as disabled",
                    e
                ),
            }
        }

        #[cfg(not(all(desktop, target_os = "windows")))]
        match hk.to_shortcut() {
            Ok(sc) => shortcuts.push(sc),
            Err(e) => log::warn!(
                "Invalid paste-last hotkey in settings store ({}); treating as disabled",
                e
            ),
        }
    }

    if let Some(hk) = retry_hotkey {
        #[cfg(all(desktop, target_os = "windows"))]
        if is_windows_hook_handled_hotkey(&hk) {
            // handled by Windows hook
        } else {
            match hk.to_shortcut() {
                Ok(sc) => push_unique(&mut shortcuts, &mut seen, sc),
                Err(e) => log::warn!(
                    "Invalid retry hotkey in settings store ({}); treating as disabled",
                    e
                ),
            }
        }

        #[cfg(not(all(desktop, target_os = "windows")))]
        match hk.to_shortcut() {
            Ok(sc) => shortcuts.push(sc),
            Err(e) => log::warn!(
                "Invalid retry hotkey in settings store ({}); treating as disabled",
                e
            ),
        }
    }

    if let Some(hk) = quick_ask_hold_hotkey {
        #[cfg(all(desktop, target_os = "windows"))]
        if is_windows_hook_handled_hotkey(&hk) {
            // handled by Windows hook
        } else {
            match hk.to_shortcut() {
                Ok(sc) => push_unique(&mut shortcuts, &mut seen, sc),
                Err(e) => log::warn!(
                    "Invalid quick ask hold hotkey in settings store ({}); treating as disabled",
                    e
                ),
            }
        }

        #[cfg(not(all(desktop, target_os = "windows")))]
        match hk.to_shortcut() {
            Ok(sc) => push_unique(&mut shortcuts, &mut seen, sc),
            Err(e) => log::warn!(
                "Invalid quick ask hold hotkey in settings store ({}); treating as disabled",
                e
            ),
        }
    }

    if let Some(hk) = quick_ask_toggle_hotkey {
        #[cfg(all(desktop, target_os = "windows"))]
        if is_windows_hook_handled_hotkey(&hk) {
            // handled by Windows hook
        } else {
            match hk.to_shortcut() {
                Ok(sc) => push_unique(&mut shortcuts, &mut seen, sc),
                Err(e) => log::warn!(
                    "Invalid quick ask toggle hotkey in settings store ({}); treating as disabled",
                    e
                ),
            }
        }

        #[cfg(not(all(desktop, target_os = "windows")))]
        match hk.to_shortcut() {
            Ok(sc) => push_unique(&mut shortcuts, &mut seen, sc),
            Err(e) => log::warn!(
                "Invalid quick ask toggle hotkey in settings store ({}); treating as disabled",
                e
            ),
        }
    }

    // Register new shortcuts with handler (skip if all are disabled)
    if !shortcuts.is_empty() {
        shortcut_manager
            .on_shortcuts(shortcuts, |app, shortcut, event| {
                crate::handle_shortcut_event(app, shortcut, &event);
            })
            .map_err(|e| format!("Failed to register shortcuts: {}", e))?;
    }

    // If we're currently recording/transcribing, re-enable Escape-to-cancel.
    // (Registering hotkeys unregisters all shortcuts, which would otherwise drop Escape.)
    let should_enable_escape = app
        .try_state::<crate::pipeline::SharedPipeline>()
        .map(|p| p.state().can_cancel())
        .unwrap_or(false);
    crate::set_escape_cancel_shortcut_enabled(&app, should_enable_escape);

    log::info!("Shortcuts re-registered successfully");
    Ok(())
}

// Stub for non-desktop platforms
#[cfg(not(desktop))]
#[tauri::command]
pub async fn register_shortcuts(_app: AppHandle) -> Result<(), String> {
    Ok(())
}

/// Validate settings.json after applying backend defaults.
#[cfg(desktop)]
#[tauri::command]
pub async fn settings_doctor(app: AppHandle) -> Result<SettingsDoctorReport, String> {
    crate::settings::defaults::ensure_default_settings(&app).map_err(|e| e.to_string())?;

    let store = app.store("settings.json").map_err(|e| e.to_string())?;
    let mut values = serde_json::Map::new();

    for key in SETTINGS_DOCTOR_KEYS {
        if let Some(value) = store.get(*key) {
            values.insert((*key).to_string(), value);
        }
    }

    Ok(doctor::validate_settings_map(&values))
}

// Stub for non-desktop platforms
#[cfg(not(desktop))]
#[tauri::command]
pub async fn settings_doctor(_app: AppHandle) -> Result<SettingsDoctorReport, String> {
    Ok(SettingsDoctorReport::default())
}
