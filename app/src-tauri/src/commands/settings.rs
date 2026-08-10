use crate::settings::{HotkeyAction, HotkeyConfig, HotkeyShortcutCard};
use tauri::{AppHandle, Manager};

#[cfg(desktop)]
use crate::commands::CommandError;
use crate::commands::CommandResult;
#[cfg(desktop)]
use tauri_plugin_global_shortcut::GlobalShortcutExt;

#[cfg(desktop)]
use tauri_plugin_store::StoreExt;

use crate::settings::doctor::SettingsDoctorReport;
#[cfg(desktop)]
use crate::settings::doctor::{self, SETTINGS_DOCTOR_KEYS};
#[cfg(desktop)]
use std::collections::{HashMap, HashSet};

#[cfg(desktop)]
use serde_json::{Map, Value};

/// Update the backend runtime flag for hotkey debug events.
///
/// Why this exists:
/// The Windows modifier-only hotkey hook runs on a background thread and needs
/// a cheap, reliable way to know whether it should emit debug `system-event`s.
/// Reading from the store on every keypress is both expensive and can be stale
/// (multi-window store instances can lag behind the JS side).
#[cfg(desktop)]
#[tauri::command]
pub async fn set_hotkey_debug_enabled_runtime(app: AppHandle, enabled: bool) -> CommandResult<()> {
    #[cfg(target_os = "windows")]
    {
        crate::windows_modifier_hotkeys::set_hotkey_debug_enabled(enabled);
    }

    crate::app_shared::emit_system_event(
        &app,
        "debug",
        &format!("Hotkey debug runtime enabled={}", enabled),
        Some("(confirmation event from backend)"),
    );
    Ok(())
}

// Stub for non-desktop platforms
#[cfg(not(desktop))]
#[tauri::command]
pub async fn set_hotkey_debug_enabled_runtime(
    _app: AppHandle,
    _enabled: bool,
) -> CommandResult<()> {
    Ok(())
}

/// Forward a modifier-only key event (like AltRight) from the frontend.
///
/// Why this exists:
/// WebView2 (Chromium) intercepts Alt key events for menu accelerator handling
/// before they reach the Windows low-level keyboard hook. When the WebView has
/// focus, our WH_KEYBOARD_LL hook never sees AltRight events. This command
/// allows the frontend to detect AltRight via JavaScript and forward it.
#[cfg(all(desktop, target_os = "windows"))]
#[tauri::command]
pub fn forward_modifier_key_event(app: AppHandle, key: String, is_down: bool) {
    crate::shortcuts::handle_modifier_key_event(&app, &key, is_down, false);
}

#[cfg(not(all(desktop, target_os = "windows")))]
#[tauri::command]
pub fn forward_modifier_key_event(_app: AppHandle, _key: String, _is_down: bool) {
    // No-op on non-Windows platforms
}

/// Temporarily unregister all global shortcuts.
/// Call this before capturing a new hotkey to prevent the shortcuts from intercepting key presses.
#[cfg(desktop)]
#[tauri::command]
pub async fn unregister_shortcuts(app: AppHandle) -> CommandResult<()> {
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

    #[cfg(target_os = "linux")]
    if crate::platform_capabilities::current_linux_display_server()
        == crate::platform_capabilities::LinuxDisplayServer::Wayland
    {
        crate::shortcuts::unregister_wayland_hotkey_cards().await?;
    }
    Ok(())
}

// Stub for non-desktop platforms
#[cfg(not(desktop))]
#[tauri::command]
pub async fn unregister_shortcuts(_app: AppHandle) -> CommandResult<()> {
    Ok(())
}

/// Re-register global shortcuts with the current settings from the store.
/// Called from frontend after hotkey settings are changed.
/// Falls back to defaults if stored values are invalid.
#[cfg(desktop)]
#[tauri::command]
pub async fn register_shortcuts(app: AppHandle) -> CommandResult<()> {
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
    let cards = crate::shortcuts::get_hotkey_cards_from_store(&app);
    crate::shortcuts::sync_windows_modifier_hook_flags(&cards);

    #[cfg(target_os = "linux")]
    let registered_with_wayland_portal =
        crate::platform_capabilities::current_linux_display_server()
            == crate::platform_capabilities::LinuxDisplayServer::Wayland;
    #[cfg(not(target_os = "linux"))]
    let registered_with_wayland_portal = false;

    if registered_with_wayland_portal {
        #[cfg(target_os = "linux")]
        crate::shortcuts::register_wayland_hotkey_cards(
            &app,
            &cards,
            crate::shortcuts::HotkeyRegistrationMode::RuntimeReplaceAll,
        )
        .await?;
    } else {
        crate::shortcuts::register_hotkey_cards(
            &app,
            &cards,
            crate::shortcuts::HotkeyRegistrationMode::RuntimeReplaceAll,
        )?;
    }

    // If we're currently recording/transcribing, re-enable Escape-to-cancel.
    // (Registering hotkeys unregisters all shortcuts, which would otherwise drop Escape.)
    let should_enable_escape = app
        .try_state::<crate::pipeline::SharedPipeline>()
        .map(|p| p.state().can_cancel())
        .unwrap_or(false);
    crate::set_escape_cancel_shortcut_enabled(&app, should_enable_escape);

    Ok(())
}

#[cfg(desktop)]
fn hotkey_action_label(action: HotkeyAction) -> &'static str {
    match action {
        HotkeyAction::Toggle => "Toggle",
        HotkeyAction::Hold => "Hold",
        HotkeyAction::PasteLast => "PasteLast",
        HotkeyAction::Retry => "Retry",
        HotkeyAction::QuickAskHold => "QuickAskHold",
        HotkeyAction::QuickAskToggle => "QuickAskToggle",
    }
}

#[cfg(desktop)]
fn validate_hotkey_shortcut_cards(cards: &[HotkeyShortcutCard]) -> CommandResult<()> {
    let mut ids: HashSet<String> = HashSet::new();
    let mut seen: HashMap<String, String> = HashMap::new();

    for card in cards {
        if card.id.trim().is_empty() {
            return Err(
                CommandError::new("Hotkey card id is required", "invalid_input")
                    .with_code("hotkey_card_invalid"),
            );
        }
        if !ids.insert(card.id.clone()) {
            return Err(
                CommandError::new("Duplicate hotkey card id", "invalid_input")
                    .with_code("hotkey_card_duplicate_id"),
            );
        }

        let Some(hotkey) = card.hotkey.as_ref() else {
            continue;
        };

        let needs_global_shortcut = {
            #[cfg(all(desktop, target_os = "windows"))]
            {
                !crate::shortcuts::is_windows_hook_handled_hotkey(hotkey)
            }
            #[cfg(not(all(desktop, target_os = "windows")))]
            {
                true
            }
        };

        if needs_global_shortcut {
            hotkey.to_shortcut().map_err(|e| {
                CommandError::new(
                    format!("Invalid {} hotkey: {}", hotkey_action_label(card.kind), e),
                    "invalid_input",
                )
                .with_code("hotkey_invalid")
            })?;
        }

        let normalized = crate::shortcuts::normalize_shortcut_string(&hotkey.to_shortcut_string());
        if let Some(existing) = seen.insert(normalized.clone(), card.id.clone()) {
            return Err(CommandError::new(
                format!("Shortcut is already used by another card ({existing})",),
                "conflict",
            )
            .with_code("hotkey_conflict"));
        }
    }

    Ok(())
}

#[cfg(desktop)]
fn first_hotkey_for_action(
    cards: &[HotkeyShortcutCard],
    action: HotkeyAction,
) -> Option<HotkeyConfig> {
    for card in cards {
        if card.kind != action {
            continue;
        }
        if let Some(hotkey) = card.hotkey.as_ref() {
            return Some(hotkey.clone());
        }
    }

    None
}

#[cfg(desktop)]
fn build_hotkey_shortcuts_patch(
    cards: &[HotkeyShortcutCard],
) -> Result<Map<String, Value>, CommandError> {
    let mut patch: Map<String, Value> = Map::new();
    patch.insert(
        "hotkey_shortcuts".to_string(),
        serde_json::to_value(cards)
            .map_err(|e| CommandError::unknown(format!("Failed to encode hotkey cards: {e}")))?,
    );

    let toggle = first_hotkey_for_action(cards, HotkeyAction::Toggle);
    let hold = first_hotkey_for_action(cards, HotkeyAction::Hold);
    let paste_last = first_hotkey_for_action(cards, HotkeyAction::PasteLast);
    let retry = first_hotkey_for_action(cards, HotkeyAction::Retry);
    let quick_ask_hold = first_hotkey_for_action(cards, HotkeyAction::QuickAskHold);
    let quick_ask_toggle = first_hotkey_for_action(cards, HotkeyAction::QuickAskToggle);

    let insert_hotkey = |patch: &mut Map<String, Value>, key: &str, value: Option<HotkeyConfig>| {
        patch.insert(
            key.to_string(),
            serde_json::to_value(value).unwrap_or(Value::Null),
        );
    };

    insert_hotkey(&mut patch, "toggle_hotkey", toggle);
    insert_hotkey(&mut patch, "hold_hotkey", hold);
    insert_hotkey(&mut patch, "paste_last_hotkey", paste_last);
    insert_hotkey(&mut patch, "retry_hotkey", retry);
    insert_hotkey(&mut patch, "quick_ask_hold_hotkey", quick_ask_hold.clone());
    insert_hotkey(&mut patch, "quick_ask_toggle_hotkey", quick_ask_toggle);
    // Legacy alias: keep the pre-split quick_ask_hotkey in sync with hold.
    insert_hotkey(&mut patch, "quick_ask_hotkey", quick_ask_hold);

    Ok(patch)
}

#[cfg(desktop)]
async fn apply_hotkey_shortcuts_update(
    app: &AppHandle,
    cards: Vec<HotkeyShortcutCard>,
) -> CommandResult<Vec<HotkeyShortcutCard>> {
    validate_hotkey_shortcut_cards(&cards)?;
    let patch = build_hotkey_shortcuts_patch(&cards)?;

    let store = app
        .store("settings.json")
        .map_err(|e| CommandError::unknown(format!("Failed to open settings store: {e}")))?;
    let previous_cards = crate::shortcuts::get_hotkey_cards_from_store(app);

    let payload = crate::settings::patch::apply_settings_patch(&store, patch, vec![])?;
    store
        .save()
        .map_err(|e| CommandError::unknown(format!("Failed to save settings store: {e}")))?;
    crate::app_shared::emit_settings_changed(app, payload);

    if let Err(error) = register_shortcuts(app.clone()).await {
        log::warn!(
            "Hotkey re-registration failed after update; reverting cards: {}",
            error
        );
        let rollback_patch = build_hotkey_shortcuts_patch(&previous_cards)?;
        let rollback_payload =
            crate::settings::patch::apply_settings_patch(&store, rollback_patch, vec![])?;
        if let Err(save_error) = store.save() {
            log::warn!("Failed to save rollback hotkey settings: {save_error}");
        } else {
            crate::app_shared::emit_settings_changed(app, rollback_payload);
        }
        let _ = register_shortcuts(app.clone()).await;
        return Err(error);
    }

    Ok(cards)
}

/// Create a new hotkey shortcut card.
#[cfg(desktop)]
#[tauri::command]
pub async fn hotkey_shortcut_cards_create(
    app: AppHandle,
    card: HotkeyShortcutCard,
) -> CommandResult<Vec<HotkeyShortcutCard>> {
    let mut cards = crate::shortcuts::get_hotkey_cards_from_store(&app);
    if cards.iter().any(|existing| existing.id == card.id) {
        return Err(
            CommandError::new("Hotkey card id already exists", "conflict")
                .with_code("hotkey_card_duplicate_id"),
        );
    }
    cards.push(card);
    apply_hotkey_shortcuts_update(&app, cards).await
}

// Stub for non-desktop platforms
#[cfg(not(desktop))]
#[tauri::command]
pub async fn hotkey_shortcut_cards_create(
    _app: AppHandle,
    _card: HotkeyShortcutCard,
) -> CommandResult<Vec<HotkeyShortcutCard>> {
    Ok(Vec::new())
}

/// Update an existing hotkey shortcut card.
#[cfg(desktop)]
#[tauri::command]
pub async fn hotkey_shortcut_cards_update(
    app: AppHandle,
    card_id: String,
    hotkey: Option<HotkeyConfig>,
) -> CommandResult<Vec<HotkeyShortcutCard>> {
    let mut cards = crate::shortcuts::get_hotkey_cards_from_store(&app);
    let Some(existing) = cards.iter_mut().find(|card| card.id == card_id) else {
        return Err(CommandError::new("Hotkey card not found", "not_found")
            .with_code("hotkey_card_not_found"));
    };

    existing.hotkey = hotkey;
    apply_hotkey_shortcuts_update(&app, cards).await
}

// Stub for non-desktop platforms
#[cfg(not(desktop))]
#[tauri::command]
pub async fn hotkey_shortcut_cards_update(
    _app: AppHandle,
    _card_id: String,
    _hotkey: Option<HotkeyConfig>,
) -> CommandResult<Vec<HotkeyShortcutCard>> {
    Ok(Vec::new())
}

/// Delete a hotkey shortcut card.
#[cfg(desktop)]
#[tauri::command]
pub async fn hotkey_shortcut_cards_delete(
    app: AppHandle,
    card_id: String,
) -> CommandResult<Vec<HotkeyShortcutCard>> {
    let mut cards = crate::shortcuts::get_hotkey_cards_from_store(&app);
    let before = cards.len();
    cards.retain(|card| card.id != card_id);
    if cards.len() == before {
        return Err(CommandError::new("Hotkey card not found", "not_found")
            .with_code("hotkey_card_not_found"));
    }
    apply_hotkey_shortcuts_update(&app, cards).await
}

// Stub for non-desktop platforms
#[cfg(not(desktop))]
#[tauri::command]
pub async fn hotkey_shortcut_cards_delete(
    _app: AppHandle,
    _card_id: String,
) -> CommandResult<Vec<HotkeyShortcutCard>> {
    Ok(Vec::new())
}

// Stub for non-desktop platforms
#[cfg(not(desktop))]
#[tauri::command]
pub async fn register_shortcuts(_app: AppHandle) -> CommandResult<()> {
    Ok(())
}

/// Validate settings.json after applying backend defaults.
#[cfg(desktop)]
#[tauri::command]
pub async fn settings_doctor(app: AppHandle) -> CommandResult<SettingsDoctorReport> {
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

/// Apply a patch to settings.json, save it, and emit `settings-changed`.
///
/// This centralizes settings writes in the backend to avoid multi-window store
/// instances clobbering each other (a known footgun when each JS window has its
/// own in-memory Store instance).
#[cfg(desktop)]
#[tauri::command]
pub async fn settings_apply_patch(
    app: AppHandle,
    mut patch: Map<String, Value>,
    delete_keys: Vec<String>,
) -> CommandResult<()> {
    let sync_action = patch
        .remove("__cloud_sync_action")
        .and_then(|value| value.as_str().map(|s| s.to_string()));

    let sync_metadata_keys: [&str; 5] = [
        "cloud_sync_last_pushed_at",
        "cloud_sync_last_pulled_at",
        "cloud_sync_last_error",
        "cloud_sync_remote_revision",
        "__cloud_sync_action",
    ];

    let patch_touches_sync_metadata = patch
        .keys()
        .any(|key| sync_metadata_keys.iter().any(|k| *k == key));
    let has_patch_changes = !patch.is_empty() || !delete_keys.is_empty();

    let store = app
        .store("settings.json")
        .map_err(|e| CommandError::unknown(format!("Failed to open settings store: {}", e)))?;

    let payload = crate::settings::patch::apply_settings_patch(&store, patch, delete_keys)?;

    store
        .save()
        .map_err(|e| CommandError::unknown(format!("Failed to save settings store: {}", e)))?;

    crate::app_shared::emit_settings_changed(&app, payload);

    if let Some(action) = sync_action.as_deref() {
        match action {
            "push" => {
                crate::commands::sync::sync_push_settings_inner(&app).await?;
            }
            "pull" => {
                crate::commands::sync::sync_pull_settings_inner(&app).await?;
            }
            other => {
                return Err(CommandError::new(
                    format!("Unknown cloud sync action: {other}"),
                    "sync",
                )
                .with_code("sync_invalid_action"));
            }
        }
    } else {
        let cloud_sync_enabled = store
            .get("cloud_sync_enabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let cloud_sync_auto_push = store
            .get("cloud_sync_auto_push")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        if cloud_sync_enabled
            && cloud_sync_auto_push
            && has_patch_changes
            && !patch_touches_sync_metadata
        {
            if let Err(error) = crate::commands::sync::sync_push_settings_inner(&app).await {
                log::warn!("Cloud sync auto-push failed after settings patch: {error}");
            }
        }
    }

    Ok(())
}

// Stub for non-desktop platforms
#[cfg(not(desktop))]
#[tauri::command]
pub async fn settings_doctor(_app: AppHandle) -> CommandResult<SettingsDoctorReport> {
    Ok(SettingsDoctorReport::default())
}

// Stub for non-desktop platforms
#[cfg(not(desktop))]
#[tauri::command]
pub async fn settings_apply_patch(
    _app: AppHandle,
    _patch: serde_json::Map<String, serde_json::Value>,
    _delete_keys: Vec<String>,
) -> CommandResult<()> {
    Ok(())
}
