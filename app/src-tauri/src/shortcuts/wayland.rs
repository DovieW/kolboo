//! Wayland global-shortcut registration through the XDG Desktop Portal.
//!
//! Kolboo deliberately runs its GTK windows through XWayland when available so
//! the recording overlay can use absolute screen coordinates. That does not
//! turn the surrounding desktop session into X11: a native Wayland application
//! such as Chrome will still receive a key grabbed by Kolboo's X11 connection.
//! The compositor-owned portal is therefore the only reliable place to both
//! receive and consume global shortcuts in a Wayland session.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, OnceLock};

use ashpd::desktop::global_shortcuts::{BindShortcutsOptions, GlobalShortcuts, NewShortcut};
use ashpd::desktop::{CreateSessionOptions, Session};
use futures_util::StreamExt;
use tauri::AppHandle;
use tokio::sync::Mutex;

use crate::settings::{HotkeyAction, HotkeyConfig, HotkeyShortcutCard};

use super::HotkeyRegistrationMode;

struct PortalShortcutSession {
    session: Session<GlobalShortcuts>,
    activated_task: tauri::async_runtime::JoinHandle<()>,
    deactivated_task: tauri::async_runtime::JoinHandle<()>,
}

fn portal_session() -> &'static Mutex<Option<PortalShortcutSession>> {
    static SESSION: OnceLock<Mutex<Option<PortalShortcutSession>>> = OnceLock::new();
    SESSION.get_or_init(|| Mutex::new(None))
}

pub(crate) async fn unregister_hotkey_cards() -> Result<(), String> {
    let mut current = portal_session().lock().await;
    close_session(current.take()).await
}

pub(crate) async fn register_hotkey_cards(
    app: &AppHandle,
    cards: &[HotkeyShortcutCard],
    mode: HotkeyRegistrationMode,
) -> Result<(), String> {
    let registrations = collect_portal_shortcuts(cards);
    let mut current = portal_session().lock().await;

    // A portal session permits BindShortcuts exactly once. Replacing settings
    // therefore means replacing the complete session.
    close_session(current.take()).await?;

    if registrations.is_empty() {
        log::info!("No Wayland portal shortcuts are enabled");
        return Ok(());
    }

    match create_session(app, registrations).await {
        Ok(session) => {
            *current = Some(session);
            log::info!("Wayland portal shortcuts registered successfully");
            Ok(())
        }
        Err(error) if mode == HotkeyRegistrationMode::StartupBestEffort => {
            log::warn!("Wayland portal shortcut registration failed: {error}");
            Err(error)
        }
        Err(error) => Err(error),
    }
}

async fn close_session(session: Option<PortalShortcutSession>) -> Result<(), String> {
    let Some(session) = session else {
        return Ok(());
    };

    session.activated_task.abort();
    session.deactivated_task.abort();
    session
        .session
        .close()
        .await
        .map_err(|error| format!("Failed to close the Wayland shortcut session: {error}"))
}

struct PortalRegistration {
    id: String,
    action: HotkeyAction,
    shortcut: NewShortcut,
}

async fn create_session(
    app: &AppHandle,
    registrations: Vec<PortalRegistration>,
) -> Result<PortalShortcutSession, String> {
    let portal = GlobalShortcuts::new()
        .await
        .map_err(|error| format!("The Wayland global-shortcut portal is unavailable: {error}"))?;

    if portal.version() < 1 {
        return Err("The Wayland global-shortcut portal has an unsupported version".to_string());
    }

    let session = portal
        .create_session(CreateSessionOptions::default())
        .await
        .map_err(|error| format!("Failed to create a Wayland shortcut session: {error}"))?;

    let shortcuts: Vec<NewShortcut> = registrations
        .iter()
        .map(|registration| registration.shortcut.clone())
        .collect();
    let request = match portal
        .bind_shortcuts(&session, &shortcuts, None, BindShortcutsOptions::default())
        .await
    {
        Ok(request) => request,
        Err(error) => {
            let _ = session.close().await;
            return Err(format!(
                "Failed to request Wayland global shortcuts: {error}"
            ));
        }
    };
    let response = match request.response() {
        Ok(response) => response,
        Err(error) => {
            let _ = session.close().await;
            return Err(format!(
                "Wayland global-shortcut approval was not completed: {error}"
            ));
        }
    };

    let bound_ids: HashSet<&str> = response
        .shortcuts()
        .iter()
        .map(|shortcut| shortcut.id())
        .collect();
    let actions: HashMap<String, HotkeyAction> = registrations
        .into_iter()
        .filter(|registration| bound_ids.contains(registration.id.as_str()))
        .map(|registration| (registration.id, registration.action))
        .collect();

    if actions.is_empty() {
        let _ = session.close().await;
        return Err("No Wayland global shortcuts were approved".to_string());
    }

    for shortcut in response.shortcuts() {
        log::info!(
            "Wayland shortcut bound: id={} trigger={}",
            shortcut.id(),
            shortcut.trigger_description()
        );
    }

    let mut activated = match portal.receive_activated().await {
        Ok(stream) => stream,
        Err(error) => {
            let _ = session.close().await;
            return Err(format!(
                "Failed to listen for Wayland shortcut presses: {error}"
            ));
        }
    };
    let mut deactivated = match portal.receive_deactivated().await {
        Ok(stream) => stream,
        Err(error) => {
            let _ = session.close().await;
            return Err(format!(
                "Failed to listen for Wayland shortcut releases: {error}"
            ));
        }
    };
    let actions = Arc::new(actions);

    let pressed_app = app.clone();
    let pressed_actions = Arc::clone(&actions);
    let activated_task = tauri::async_runtime::spawn(async move {
        while let Some(event) = activated.next().await {
            if let Some(action) = pressed_actions.get(event.shortcut_id()).copied() {
                super::handle_hotkey_action_event(&pressed_app, action, true);
            }
        }
    });

    let released_app = app.clone();
    let deactivated_task = tauri::async_runtime::spawn(async move {
        while let Some(event) = deactivated.next().await {
            if let Some(action) = actions.get(event.shortcut_id()).copied() {
                super::handle_hotkey_action_event(&released_app, action, false);
            }
        }
    });

    Ok(PortalShortcutSession {
        session,
        activated_task,
        deactivated_task,
    })
}

fn collect_portal_shortcuts(cards: &[HotkeyShortcutCard]) -> Vec<PortalRegistration> {
    let mut registrations = Vec::new();
    let mut triggers = HashSet::new();

    for (index, card) in cards.iter().enumerate() {
        let Some(hotkey) = card.hotkey.as_ref() else {
            continue;
        };

        // Retain the same validation boundary used by the other desktop
        // adapters so unsupported settings do not reach the portal dialog.
        if let Err(error) = hotkey.to_shortcut() {
            log::warn!(
                "Invalid {} hotkey in settings store ({}); treating as disabled",
                action_label(card.kind),
                error
            );
            continue;
        }

        let trigger = match to_xdg_trigger(hotkey) {
            Ok(trigger) => trigger,
            Err(error) => {
                log::warn!(
                    "Unsupported {} Wayland hotkey ({}); treating as disabled",
                    action_label(card.kind),
                    error
                );
                continue;
            }
        };
        if !triggers.insert(trigger.to_ascii_lowercase()) {
            log::warn!("Duplicate Wayland hotkey detected; skipping duplicate registration");
            continue;
        }

        let id = portal_shortcut_id(card.kind, index, &trigger);
        let shortcut = NewShortcut::new(id.clone(), action_description(card.kind))
            .preferred_trigger(trigger.as_str());
        registrations.push(PortalRegistration {
            id,
            action: card.kind,
            shortcut,
        });
    }

    registrations
}

fn portal_shortcut_id(action: HotkeyAction, index: usize, trigger: &str) -> String {
    let trigger = trigger
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>();
    format!("{}-{index}-{trigger}", action_id(action))
}

fn action_id(action: HotkeyAction) -> &'static str {
    match action {
        HotkeyAction::Toggle => "toggle",
        HotkeyAction::Hold => "hold",
        HotkeyAction::PasteLast => "paste-last",
        HotkeyAction::Retry => "retry",
        HotkeyAction::QuickAskHold => "quick-ask-hold",
        HotkeyAction::QuickAskToggle => "quick-ask-toggle",
    }
}

fn action_label(action: HotkeyAction) -> &'static str {
    match action {
        HotkeyAction::Toggle => "Toggle",
        HotkeyAction::Hold => "Hold",
        HotkeyAction::PasteLast => "PasteLast",
        HotkeyAction::Retry => "Retry",
        HotkeyAction::QuickAskHold => "QuickAskHold",
        HotkeyAction::QuickAskToggle => "QuickAskToggle",
    }
}

fn action_description(action: HotkeyAction) -> &'static str {
    match action {
        HotkeyAction::Toggle => "Start or stop voice dictation",
        HotkeyAction::Hold => "Hold to dictate",
        HotkeyAction::PasteLast => "Paste the last dictation",
        HotkeyAction::Retry => "Retry the last dictation",
        HotkeyAction::QuickAskHold => "Hold to use Quick Ask",
        HotkeyAction::QuickAskToggle => "Start or stop Quick Ask",
    }
}

/// Convert Kolboo/Tauri key names into the XKB identifiers required by the
/// freedesktop shortcut specification.
fn to_xdg_trigger(hotkey: &HotkeyConfig) -> Result<String, String> {
    let mut parts = Vec::with_capacity(hotkey.modifiers.len() + 1);
    for modifier in &hotkey.modifiers {
        let modifier = match modifier.trim().to_ascii_lowercase().as_str() {
            "ctrl" | "control" => "CTRL",
            "alt" => "ALT",
            "shift" => "SHIFT",
            "cmd" | "logo" | "meta" | "super" | "win" => "LOGO",
            "num" => "NUM",
            other => return Err(format!("unknown modifier {other}")),
        };
        parts.push(modifier.to_string());
    }

    parts.push(to_xkb_key_name(&hotkey.key)?);
    Ok(parts.join("+"))
}

fn to_xkb_key_name(key: &str) -> Result<String, String> {
    if key.len() == 1 {
        let character = key.chars().next().expect("single-character key");
        if character.is_ascii_alphabetic() {
            return Ok(character.to_ascii_lowercase().to_string());
        }
        if character.is_ascii_digit() {
            return Ok(character.to_string());
        }
    }

    if let Some(letter) = key.strip_prefix("Key") {
        if letter.len() == 1 && letter.chars().all(|value| value.is_ascii_alphabetic()) {
            return Ok(letter.to_ascii_lowercase());
        }
    }
    if let Some(digit) = key.strip_prefix("Digit") {
        if digit.len() == 1 && digit.chars().all(|value| value.is_ascii_digit()) {
            return Ok(digit.to_string());
        }
    }
    if let Some(number) = key.strip_prefix('F') {
        if number
            .parse::<u8>()
            .is_ok_and(|number| (1..=35).contains(&number))
        {
            return Ok(key.to_ascii_uppercase());
        }
    }
    if let Some(number) = key.strip_prefix("Numpad") {
        if number.len() == 1 && number.chars().all(|value| value.is_ascii_digit()) {
            return Ok(format!("KP_{number}"));
        }
    }

    let xkb_name = match key {
        "Space" => "space",
        "Backspace" => "BackSpace",
        "Tab" => "Tab",
        "Enter" => "Return",
        "Escape" => "Escape",
        "Delete" => "Delete",
        "Insert" => "Insert",
        "Home" => "Home",
        "End" => "End",
        "PageUp" => "Prior",
        "PageDown" => "Next",
        "CapsLock" => "Caps_Lock",
        "NumLock" => "Num_Lock",
        "ScrollLock" => "Scroll_Lock",
        "PrintScreen" => "Print",
        "Pause" => "Pause",
        "ArrowUp" => "Up",
        "ArrowDown" => "Down",
        "ArrowLeft" => "Left",
        "ArrowRight" => "Right",
        "Period" => "period",
        "Comma" => "comma",
        "Slash" => "slash",
        "Backslash" => "backslash",
        "Semicolon" => "semicolon",
        "Quote" => "apostrophe",
        "BracketLeft" => "bracketleft",
        "BracketRight" => "bracketright",
        "Backquote" => "grave",
        "Minus" => "minus",
        "Equal" => "equal",
        "NumpadAdd" => "KP_Add",
        "NumpadSubtract" => "KP_Subtract",
        "NumpadMultiply" => "KP_Multiply",
        "NumpadDivide" => "KP_Divide",
        "NumpadDecimal" => "KP_Decimal",
        "NumpadEnter" => "KP_Enter",
        "MediaPlay" | "MediaPlayPause" => "XF86AudioPlay",
        "MediaPause" => "XF86AudioPause",
        "MediaTrackNext" => "XF86AudioNext",
        "MediaTrackPrevious" => "XF86AudioPrev",
        "MediaStop" => "XF86AudioStop",
        "VolumeUp" => "XF86AudioRaiseVolume",
        "VolumeDown" => "XF86AudioLowerVolume",
        "VolumeMute" => "XF86AudioMute",
        "BrowserBack" => "XF86Back",
        "BrowserForward" => "XF86Forward",
        "BrowserRefresh" => "XF86Refresh",
        "BrowserStop" => "XF86Stop",
        "BrowserSearch" => "XF86Search",
        "BrowserFavorites" => "XF86Favorites",
        "BrowserHome" => "XF86HomePage",
        "LaunchMail" => "XF86Mail",
        "LaunchMediaPlayer" => "XF86AudioMedia",
        "Calculator" => "XF86Calculator",
        "BrightnessUp" => "XF86MonBrightnessUp",
        "BrightnessDown" => "XF86MonBrightnessDown",
        "MicMute" => "XF86AudioMicMute",
        "Eject" => "XF86Eject",
        "Sleep" => "XF86Sleep",
        "WakeUp" => "XF86WakeUp",
        "Power" => "XF86PowerOff",
        unsupported => return Err(format!("unsupported key {unsupported}")),
    };

    Ok(xkb_name.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hotkey(modifiers: &[&str], key: &str) -> HotkeyConfig {
        HotkeyConfig {
            modifiers: modifiers
                .iter()
                .map(|modifier| modifier.to_string())
                .collect(),
            key: key.to_string(),
        }
    }

    #[test]
    fn converts_default_f3_to_xdg_syntax() {
        assert_eq!(to_xdg_trigger(&hotkey(&[], "F3")).unwrap(), "F3");
    }

    #[test]
    fn converts_modifiers_and_common_tauri_keys() {
        assert_eq!(
            to_xdg_trigger(&hotkey(&["ctrl", "shift"], "KeyR")).unwrap(),
            "CTRL+SHIFT+r"
        );
        assert_eq!(
            to_xdg_trigger(&hotkey(&["super"], "ArrowUp")).unwrap(),
            "LOGO+Up"
        );
        assert_eq!(
            to_xdg_trigger(&hotkey(&["alt"], "Numpad1")).unwrap(),
            "ALT+KP_1"
        );
    }

    #[test]
    fn rejects_keys_the_portal_cannot_describe() {
        assert!(to_xdg_trigger(&hotkey(&[], "Copilot")).is_err());
        assert!(to_xdg_trigger(&hotkey(&["hyper"], "F3")).is_err());
    }

    #[test]
    fn shortcut_ids_change_when_the_binding_changes() {
        assert_ne!(
            portal_shortcut_id(HotkeyAction::Toggle, 0, "F3"),
            portal_shortcut_id(HotkeyAction::Toggle, 0, "CTRL+F3")
        );
    }
}
