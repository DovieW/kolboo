use crate::settings::HotkeyConfig;
use tauri::{AppHandle, Manager};

#[cfg(desktop)]
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

#[cfg(desktop)]
use tauri_plugin_store::StoreExt;

#[cfg(all(desktop, target_os = "windows"))]
fn is_windows_modifier_only_hotkey(hk: &HotkeyConfig) -> bool {
    hk.modifiers.is_empty() && matches!(hk.key.as_str(), "AltRight")
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
            .or_else(|| default_fn()),
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

    log::info!(
        "Re-registering shortcuts - Toggle: {}, Hold: {}, PasteLast: {}, Retry: {}",
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
    if let Some(hk) = toggle_hotkey {
        #[cfg(all(desktop, target_os = "windows"))]
        if is_windows_modifier_only_hotkey(&hk) {
            // handled by Windows hook (not tauri-plugin-global-shortcut)
        } else {
            match hk.to_shortcut() {
                Ok(sc) => shortcuts.push(sc),
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
        if is_windows_modifier_only_hotkey(&hk) {
            // handled by Windows hook
        } else {
            match hk.to_shortcut() {
                Ok(sc) => shortcuts.push(sc),
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
        if is_windows_modifier_only_hotkey(&hk) {
            // handled by Windows hook
        } else {
            match hk.to_shortcut() {
                Ok(sc) => shortcuts.push(sc),
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
        if is_windows_modifier_only_hotkey(&hk) {
            // handled by Windows hook
        } else {
            match hk.to_shortcut() {
                Ok(sc) => shortcuts.push(sc),
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
