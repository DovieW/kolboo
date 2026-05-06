//! Hotkey lifecycle registration helpers.
//!
//! Startup registration and runtime re-registration used to have separate copies of the same
//! decisions: which shortcut cards are Windows-hook Adapters, how duplicates are skipped, and how
//! global-shortcut plugin handlers are installed. This Module keeps those lifecycle decisions in
//! one place while leaving event dispatch in `shortcuts::mod` and Windows modifier-only mechanics
//! in `windows_modifier_hotkeys.rs`.

use std::collections::HashSet;
use std::str::FromStr;

use tauri::AppHandle;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

use crate::emit_system_event;
use crate::settings::{HotkeyAction, HotkeyConfig, HotkeyShortcutCard};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HotkeyRegistrationMode {
    /// Used at app startup. Individual shortcut conflicts are reported, but startup continues.
    StartupBestEffort,
    /// Used after settings changes. Existing global shortcuts are replaced atomically enough for UI
    /// rollback: a plugin registration error is returned to the caller.
    RuntimeReplaceAll,
}

#[cfg(all(desktop, target_os = "windows"))]
pub(crate) fn is_windows_hook_handled_hotkey(hk: &HotkeyConfig) -> bool {
    // These are handled by the low-level Windows hook Adapter, not by
    // tauri-plugin-global-shortcut. Keep this predicate shared by startup registration,
    // runtime registration, and validation so they cannot drift.
    hk.modifiers.is_empty() && matches!(hk.key.as_str(), "AltRight" | "Copilot")
}

#[cfg(not(all(desktop, target_os = "windows")))]
pub(crate) fn is_windows_hook_handled_hotkey(_hk: &HotkeyConfig) -> bool {
    false
}

#[cfg(desktop)]
pub(crate) fn sync_windows_modifier_hook_flags(cards: &[HotkeyShortcutCard]) {
    #[cfg(target_os = "windows")]
    {
        let copilot_enabled = cards
            .iter()
            .any(|card| card_matches_modifier_only(card, "Copilot"));
        let alt_right_enabled = cards
            .iter()
            .any(|card| card_matches_modifier_only(card, "AltRight"));

        crate::windows_modifier_hotkeys::set_copilot_hotkey_enabled(copilot_enabled);
        crate::windows_modifier_hotkeys::set_alt_right_hotkey_enabled(alt_right_enabled);
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = cards;
    }
}

#[cfg(desktop)]
pub(crate) fn register_hotkey_cards(
    app: &AppHandle,
    cards: &[HotkeyShortcutCard],
    mode: HotkeyRegistrationMode,
) -> Result<(), String> {
    log_hotkey_summary(cards, mode);

    let shortcut_manager = app.global_shortcut();

    if mode == HotkeyRegistrationMode::RuntimeReplaceAll {
        shortcut_manager
            .unregister_all()
            .map_err(|e| format!("Failed to unregister shortcuts: {e}"))?;
    }

    let registrations = collect_global_shortcuts(cards);

    match mode {
        HotkeyRegistrationMode::StartupBestEffort => {
            register_startup_best_effort(app, shortcut_manager, registrations);
            Ok(())
        }
        HotkeyRegistrationMode::RuntimeReplaceAll => {
            register_runtime_replace_all(shortcut_manager, registrations)
        }
    }
}

#[cfg(desktop)]
fn collect_global_shortcuts(cards: &[HotkeyShortcutCard]) -> Vec<ShortcutRegistration> {
    let mut registrations = Vec::new();
    let mut registered: HashSet<String> = HashSet::new();

    for card in cards {
        let Some(hotkey) = card.hotkey.as_ref() else {
            continue;
        };

        if is_windows_hook_handled_hotkey(hotkey) {
            continue;
        }

        let shortcut_str = match hotkey.to_shortcut() {
            Ok(_) => hotkey.to_shortcut_string(),
            Err(e) => {
                log::warn!(
                    "Invalid {} hotkey in settings store ({}); treating as disabled",
                    hotkey_action_label(card.kind),
                    e
                );
                continue;
            }
        };

        // Match the validation path's comparison key so legacy settings with
        // equivalent aliases/orderings (for example `ctrl+shift+Space` vs
        // `shift+control+Space`) do not make startup/runtime registration drift.
        let normalized_shortcut_str = super::normalize_shortcut_string(&shortcut_str);
        if !registered.insert(normalized_shortcut_str) {
            log::warn!("Duplicate hotkey detected; skipping duplicate registration");
            continue;
        }

        match Shortcut::from_str(shortcut_str.as_str()) {
            Ok(shortcut) => registrations.push(ShortcutRegistration {
                action: card.kind,
                shortcut_str,
                shortcut,
            }),
            Err(e) => log::warn!(
                "Invalid {} hotkey in settings store ({:?}); treating as disabled",
                hotkey_action_label(card.kind),
                e
            ),
        }
    }

    registrations
}

#[cfg(desktop)]
fn register_startup_best_effort(
    app: &AppHandle,
    shortcut_manager: &tauri_plugin_global_shortcut::GlobalShortcut<tauri::Wry>,
    registrations: Vec<ShortcutRegistration>,
) {
    let mut failures: Vec<String> = Vec::new();

    for registration in registrations {
        if let Err(e) =
            shortcut_manager.on_shortcut(registration.shortcut, |app, shortcut, event| {
                super::handle_shortcut_event(app, shortcut, &event);
            })
        {
            failures.push(format!(
                "{} ({}) => {}",
                hotkey_action_label(registration.action),
                registration.shortcut_str,
                e
            ));
        }
    }

    if failures.is_empty() {
        log::info!("Shortcuts registered successfully");
        return;
    }

    let details = failures.join("\n");
    log::warn!(
        "One or more shortcuts failed to register. The app will continue running, but some hotkeys may not work until you change them in Settings.\n{}",
        details
    );
    emit_system_event(
        app,
        "warning",
        "Some global hotkeys could not be registered",
        Some(&details),
    );
}

#[cfg(desktop)]
fn register_runtime_replace_all(
    shortcut_manager: &tauri_plugin_global_shortcut::GlobalShortcut<tauri::Wry>,
    registrations: Vec<ShortcutRegistration>,
) -> Result<(), String> {
    if registrations.is_empty() {
        log::info!("Shortcuts re-registered successfully");
        return Ok(());
    }

    let shortcuts: Vec<Shortcut> = registrations.into_iter().map(|r| r.shortcut).collect();
    shortcut_manager
        .on_shortcuts(shortcuts, |app, shortcut, event| {
            super::handle_shortcut_event(app, shortcut, &event);
        })
        .map_err(|e| format!("Failed to register shortcuts: {e}"))?;

    log::info!("Shortcuts re-registered successfully");
    Ok(())
}

#[cfg(desktop)]
fn log_hotkey_summary(cards: &[HotkeyShortcutCard], mode: HotkeyRegistrationMode) {
    let mut hotkey_summaries: Vec<String> = Vec::new();
    for card in cards {
        let Some(hotkey) = card.hotkey.as_ref() else {
            continue;
        };

        match mode {
            HotkeyRegistrationMode::StartupBestEffort => hotkey_summaries.push(format!(
                "{}: {}",
                hotkey_action_label(card.kind),
                hotkey.to_shortcut_string()
            )),
            HotkeyRegistrationMode::RuntimeReplaceAll => {
                hotkey_summaries.push(hotkey.to_shortcut_string())
            }
        }
    }

    let verb = match mode {
        HotkeyRegistrationMode::StartupBestEffort => "Registering",
        HotkeyRegistrationMode::RuntimeReplaceAll => "Re-registering",
    };

    log::info!(
        "{} {} shortcut cards: {}",
        verb,
        hotkey_summaries.len(),
        if hotkey_summaries.is_empty() {
            "<disabled>".to_string()
        } else {
            hotkey_summaries.join(", ")
        }
    );
}

#[cfg(desktop)]
fn card_matches_modifier_only(card: &HotkeyShortcutCard, key: &str) -> bool {
    card.hotkey
        .as_ref()
        .is_some_and(|hk| hk.modifiers.is_empty() && hk.key == key)
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
struct ShortcutRegistration {
    action: HotkeyAction,
    shortcut_str: String,
    shortcut: Shortcut,
}
