use std::sync::atomic::Ordering;

use tauri::{AppHandle, Emitter, Manager};

#[cfg(desktop)]
mod hold_recording;
#[cfg(desktop)]
mod lifecycle;
#[cfg(desktop)]
mod paste_last;
#[cfg(desktop)]
mod quick_ask_hold;
#[cfg(desktop)]
mod quick_ask_toggle;
#[cfg(desktop)]
mod retry_last;
#[cfg(desktop)]
mod toggle_recording;

#[cfg(desktop)]
use hold_recording::{handle_hold_shortcut_event, HoldShortcutSource};
#[cfg(desktop)]
pub(crate) use lifecycle::{
    is_windows_hook_handled_hotkey, register_hotkey_cards, sync_windows_modifier_hook_flags,
    HotkeyRegistrationMode,
};
#[cfg(desktop)]
use paste_last::{handle_paste_last_shortcut_event, PasteLastShortcutSource};
#[cfg(desktop)]
use quick_ask_hold::{handle_quick_ask_hold_shortcut_event, QuickAskHoldShortcutSource};
#[cfg(desktop)]
use quick_ask_toggle::{handle_quick_ask_toggle_shortcut_event, QuickAskToggleShortcutSource};
#[cfg(desktop)]
pub(crate) use retry_last::spawn_retry_last_recording_and_output;
#[cfg(desktop)]
use toggle_recording::{handle_toggle_shortcut_event, ToggleShortcutSource};

#[cfg(desktop)]
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutEvent, ShortcutState};

#[cfg(desktop)]
use tauri_plugin_store::StoreExt;

use crate::audio;
use crate::events;
use crate::history::HistoryStorage;
use crate::pipeline;
use crate::request_log::RequestLogStore;
use crate::settings::HotkeyAction;
use crate::settings::HotkeyConfig as InternalHotkeyConfig;
use crate::settings::HotkeyConfig;
use crate::settings::HotkeyShortcutCard;
use crate::shortcuts_lock;
use crate::state::AppState;
use crate::{
    emit_system_event, get_playing_audio_handling, get_setting_from_store, toggle_media_play_pause,
    AudioMuteManager, PipelineStateEvent,
};

/// Normalize a shortcut string for comparison (handles "ctrl" vs "control" differences)
#[cfg(desktop)]
pub(crate) fn normalize_shortcut_string(s: &str) -> String {
    // Canonicalize for comparison across:
    // - different modifier aliases (ctrl vs control)
    // - different output ordering (e.g. "ctrl+shift+f3" vs "shift+control+f3")
    let parts: Vec<String> = s
        .split('+')
        .map(|p| p.trim().to_lowercase())
        .filter(|p| !p.is_empty())
        .map(|p| match p.as_str() {
            "ctrl" => "control".to_string(),
            "cmd" => "super".to_string(),
            "meta" => "super".to_string(),
            "win" => "super".to_string(),
            other => other.to_string(),
        })
        .collect();

    let mut modifiers: Vec<String> = Vec::new();
    let mut keys: Vec<String> = Vec::new();

    for part in parts {
        let is_modifier = matches!(part.as_str(), "control" | "alt" | "shift" | "super");
        if is_modifier {
            modifiers.push(part);
        } else {
            keys.push(part);
        }
    }

    modifiers.sort();
    keys.sort();
    modifiers.extend(keys);
    modifiers.join("+")
}

#[cfg(desktop)]
fn build_action_shortcut_strings(
    cards: &[HotkeyShortcutCard],
) -> std::collections::HashMap<HotkeyAction, Vec<String>> {
    let mut map: std::collections::HashMap<HotkeyAction, Vec<String>> =
        std::collections::HashMap::new();

    for card in cards {
        let Some(hotkey) = card.hotkey.as_ref() else {
            continue;
        };

        match hotkey.to_shortcut() {
            Ok(_) => {
                let normalized = normalize_shortcut_string(&hotkey.to_shortcut_string());
                map.entry(card.kind).or_default().push(normalized);
            }
            Err(e) => {
                log::warn!(
                    "Invalid {:?} hotkey in settings store ({}); treating as disabled",
                    card.kind,
                    e
                );
            }
        }
    }

    map
}

#[cfg(desktop)]
fn card_matches_modifier_only(card: &HotkeyShortcutCard, key: &str) -> bool {
    card.hotkey
        .as_ref()
        .is_some_and(|hk| hk.modifiers.is_empty() && hk.key == key)
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
    default_fn: fn() -> Option<InternalHotkeyConfig>,
) -> Option<HotkeyConfig> {
    use serde_json::Value;

    let raw = app
        .store("settings.json")
        .ok()
        .and_then(|store| store.get(key));

    match raw {
        None => default_fn(),
        Some(Value::Null) => None,
        Some(v) => serde_json::from_value::<InternalHotkeyConfig>(v)
            .ok()
            .or_else(default_fn),
    }
}

#[cfg(desktop)]
fn build_legacy_hotkey_cards(app: &AppHandle) -> Vec<HotkeyShortcutCard> {
    let toggle_hotkey =
        get_hotkey_from_store(app, "toggle_hotkey", HotkeyConfig::default_toggle_opt);
    let hold_hotkey = get_hotkey_from_store(app, "hold_hotkey", HotkeyConfig::default_hold);
    let paste_last_hotkey =
        get_hotkey_from_store(app, "paste_last_hotkey", HotkeyConfig::default_paste_last);
    let retry_hotkey = get_hotkey_from_store(app, "retry_hotkey", HotkeyConfig::default_retry);
    let (quick_ask_hold_hotkey, quick_ask_toggle_hotkey) = {
        use serde_json::Value;

        let store = app.store("settings.json").ok();
        let raw_hold = store.as_ref().and_then(|s| s.get("quick_ask_hold_hotkey"));
        let hold = match raw_hold {
            None => get_hotkey_from_store(app, "quick_ask_hotkey", HotkeyConfig::default_quick_ask),
            Some(Value::Null) => None,
            Some(v) => serde_json::from_value::<HotkeyConfig>(v)
                .ok()
                .or_else(HotkeyConfig::default_quick_ask),
        };

        let toggle = get_hotkey_from_store(
            app,
            "quick_ask_toggle_hotkey",
            HotkeyConfig::default_quick_ask,
        );

        (hold, toggle)
    };

    let mut cards: Vec<HotkeyShortcutCard> = Vec::new();
    let mut push_card = |kind: HotkeyAction, hotkey: Option<HotkeyConfig>, label: &str| {
        if let Some(hotkey) = hotkey {
            cards.push(HotkeyShortcutCard {
                id: format!("legacy-{}", label),
                kind,
                hotkey: Some(hotkey),
            });
        }
    };

    push_card(HotkeyAction::Toggle, toggle_hotkey, "toggle");
    push_card(HotkeyAction::Hold, hold_hotkey, "hold");
    push_card(HotkeyAction::PasteLast, paste_last_hotkey, "paste_last");
    push_card(HotkeyAction::Retry, retry_hotkey, "retry");
    push_card(
        HotkeyAction::QuickAskHold,
        quick_ask_hold_hotkey,
        "quick_ask_hold",
    );
    push_card(
        HotkeyAction::QuickAskToggle,
        quick_ask_toggle_hotkey,
        "quick_ask_toggle",
    );

    cards
}

#[cfg(desktop)]
pub(crate) fn get_hotkey_cards_from_store(app: &AppHandle) -> Vec<HotkeyShortcutCard> {
    let raw = app
        .store("settings.json")
        .ok()
        .and_then(|store| store.get("hotkey_shortcuts"));

    let Some(raw) = raw else {
        return build_legacy_hotkey_cards(app);
    };

    if raw.is_null() {
        return build_legacy_hotkey_cards(app);
    }

    match serde_json::from_value::<Vec<HotkeyShortcutCard>>(raw) {
        Ok(cards) => cards,
        Err(e) => {
            log::warn!(
                "Invalid hotkey_shortcuts in settings store ({}); falling back to legacy keys",
                e
            );
            build_legacy_hotkey_cards(app)
        }
    }
}

// ============================================================================
// Escape-to-cancel support
// ============================================================================

#[cfg(desktop)]
const ESCAPE_CANCEL_SHORTCUT: &str = "Escape";

#[cfg(desktop)]
fn is_quick_ask_visible(app: &AppHandle) -> bool {
    app.get_webview_window("quick_ask")
        .and_then(|win| win.is_visible().ok())
        .unwrap_or(false)
}

/// Enable/disable the Escape global shortcut that cancels the current pipeline session.
///
/// We register this shortcut only while the pipeline is Recording/Transcribing so we don't
/// steal Escape from other apps while idle.
#[cfg(desktop)]
pub(crate) fn set_escape_cancel_shortcut_enabled(app: &AppHandle, enabled: bool) {
    // IMPORTANT: this function can be called from within a global-shortcut callback.
    // Registering/unregistering shortcuts re-entrantly can crash/deadlock on some platforms.
    // Schedule the actual work onto the async runtime to avoid re-entrancy.
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        set_escape_cancel_shortcut_enabled_inner(&app, enabled).await;
    });
}

#[cfg(desktop)]
async fn set_escape_cancel_shortcut_enabled_inner(app: &AppHandle, enabled: bool) {
    let _guard = shortcuts_lock::global_shortcut_lock().lock().await;
    // CLI invocations (and other headless-ish contexts) may build a minimal Tauri app
    // without managing the global-shortcut plugin. In that case, calling
    // `app.global_shortcut()` panics (tauri state() called before manage()).
    //
    // Escape-to-cancel is purely a UX affordance; if shortcuts aren't available, we can
    // safely no-op.
    if app
        .try_state::<tauri_plugin_global_shortcut::GlobalShortcut<tauri::Wry>>()
        .is_none()
    {
        log::debug!(
            "Global shortcut plugin not managed; skipping Escape shortcut toggle (enabled={})",
            enabled
        );
        return;
    }

    let shortcut_manager = app.global_shortcut();
    let quick_ask_visible = is_quick_ask_visible(app);
    let pipeline_can_cancel = app
        .try_state::<pipeline::SharedPipeline>()
        .map(|p| p.state().can_cancel())
        .unwrap_or(false);
    let should_enable = enabled || pipeline_can_cancel || quick_ask_visible;

    let is_registered = shortcut_manager.is_registered(ESCAPE_CANCEL_SHORTCUT);
    log::trace!(
        "Escape shortcut toggle: enabled={} (currently registered={})",
        should_enable,
        is_registered
    );

    if should_enable {
        if is_registered {
            return;
        }

        if let Err(e) =
            shortcut_manager.on_shortcut(ESCAPE_CANCEL_SHORTCUT, |app, _shortcut, event| {
                if event.state == ShortcutState::Pressed {
                    if let Some(win) = app.get_webview_window("quick_ask") {
                        if win.is_visible().unwrap_or(false) {
                            let _ = win.emit(crate::events::EVENT_QUICK_ASK_DISMISS_REQUESTED, ());
                            return;
                        }
                    }
                    cancel_pipeline_session(app, "Escape");
                }
            })
        {
            log::warn!(
                "Failed to register Escape cancel shortcut ({}): {}",
                ESCAPE_CANCEL_SHORTCUT,
                e
            );
        }
    } else if is_registered {
        if let Err(e) = shortcut_manager.unregister(ESCAPE_CANCEL_SHORTCUT) {
            log::warn!(
                "Failed to unregister Escape cancel shortcut ({}): {}",
                ESCAPE_CANCEL_SHORTCUT,
                e
            );
        }
    }
}

/// Cancel current recording/transcription without triggering transcription output.
///
/// This is used by Escape-to-cancel and can also be reused by commands.
#[cfg(desktop)]
pub(crate) fn cancel_pipeline_session(app: &AppHandle, source: &str) {
    let state = app.state::<AppState>();

    // Best-effort: capture the active request id so we can clean up history.
    let active_request_id: Option<String> = app
        .try_state::<RequestLogStore>()
        .and_then(|store| store.with_current(|log| log.id.clone()));

    // If pipeline isn't in a cancellable state, ignore.
    // Also capture the pipeline state so we can tailor UX (e.g. avoid double "stop" cues
    // when cancelling during transcription).
    let pipeline = app.try_state::<pipeline::SharedPipeline>();
    let pipeline_state = pipeline.as_ref().map(|p| p.state());
    let can_cancel = pipeline_state.map(|s| s.can_cancel()).unwrap_or(false);

    if !can_cancel {
        // Defensive: if we somehow still have the shortcut registered while idle, disable it.
        set_escape_cancel_shortcut_enabled(app, false);
        return;
    }

    log::info!("{}: cancelling recording/transcription", source);
    emit_system_event(app, "shortcut", &format!("{}: cancelling", source), None);

    // Clear recording state flags.
    state.is_recording.store(false, Ordering::SeqCst);
    state.toggle_key_held.store(false, Ordering::SeqCst);
    state.ptt_key_held.store(false, Ordering::SeqCst);
    state.paste_key_held.store(false, Ordering::SeqCst);
    state.retry_key_held.store(false, Ordering::SeqCst);
    state.quick_ask_key_held.store(false, Ordering::SeqCst);
    state
        .quick_ask_toggle_key_held
        .store(false, Ordering::SeqCst);
    state
        .quick_ask_session_active
        .store(false, Ordering::SeqCst);

    // Restore audio side effects (unmute + resume playback if we paused).
    let sound_enabled: bool = get_setting_from_store(app, "sound_enabled", true);
    let playing_audio_handling = get_playing_audio_handling(app);
    let audio_mute_manager = app.try_state::<AudioMuteManager>();

    if playing_audio_handling.wants_mute() {
        if let Some(manager) = audio_mute_manager.as_ref() {
            if let Err(e) = manager.unmute() {
                log::warn!("Failed to unmute audio after cancel: {}", e);
            }
        }
    }

    if playing_audio_handling.wants_pause()
        && state.play_pause_toggled.swap(false, Ordering::SeqCst)
    {
        if let Err(e) = toggle_media_play_pause(app) {
            log::warn!("Failed to restore media play/pause after cancel: {}", e);
        }
    }

    // Never play the stop cue for Escape-to-cancel.
    // (User expectation: Escape is a silent abort, not a "stop recording" confirmation.)
    // For other cancel sources, we keep the existing behavior and only play the stop cue
    // if we're cancelling *during recording*.
    let should_play_stop_cue = source != "Escape"
        && !playing_audio_handling.wants_mute()
        && matches!(pipeline_state, Some(pipeline::PipelineState::Recording));
    if sound_enabled && should_play_stop_cue {
        let audio_cue_raw: String = get_setting_from_store(app, "audio_cue", "kolboo".to_string());
        let audio_cue = audio::AudioCue::from_str(&audio_cue_raw);
        audio::play_sound(audio::SoundType::RecordingStop, audio_cue);
    }

    // Cancel request log
    if let Some(log_store) = app.try_state::<RequestLogStore>() {
        log_store.with_current(|log| {
            log.warn("Recording cancelled by user");
            log.complete_cancelled();
        });
        log_store.complete_current();
    }

    // End OCR session (best-effort) so any in-flight OCR work is cancelled and does not
    // leak into later requests.
    if let (Some(pipeline), Some(req_id)) = (pipeline.as_ref(), active_request_id.as_deref()) {
        pipeline.end_ocr_session_if_matches(req_id);
    }

    // Best-effort: remove any in-progress history entry for this request.
    if let Some(req_id) = active_request_id.as_deref() {
        if let Some(history) = app.try_state::<HistoryStorage>() {
            let _ = history.delete(req_id);
            let _ = app.emit(events::EVENT_HISTORY_CHANGED, ());
        }
    }

    // Cancel pipeline
    if let Some(pipeline) = pipeline {
        pipeline.cancel();
    }

    // Hide overlay if in recording-only mode.
    let overlay_mode: String =
        get_setting_from_store(app, "overlay_mode", "recording_only".to_string());
    if overlay_mode == "recording_only" {
        let _ = app.emit(events::EVENT_OVERLAY_HIDE_REQUESTED, ());

        if let Some(window) = app.get_webview_window("overlay") {
            let window_clone = window.clone();
            let app_check = app.clone();
            let expected_epoch = app
                .state::<AppState>()
                .overlay_visibility_epoch
                .load(Ordering::SeqCst);
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(220)).await;
                let current_mode: String = get_setting_from_store(
                    &app_check,
                    "overlay_mode",
                    "recording_only".to_string(),
                );
                let current_epoch = app_check
                    .state::<AppState>()
                    .overlay_visibility_epoch
                    .load(Ordering::SeqCst);
                let pipeline_state = app_check
                    .try_state::<pipeline::SharedPipeline>()
                    .map(|p| p.state());
                log::debug!(
                    "[overlay] shortcut-cancel fallback hide check (current_mode={}, expected_epoch={}, current_epoch={}, pipeline_state={:?})",
                    current_mode,
                    expected_epoch,
                    current_epoch,
                    pipeline_state
                );
                if current_mode == "recording_only" && current_epoch == expected_epoch {
                    let visible_before = window_clone.is_visible().ok();
                    log::debug!(
                        "[overlay] shortcut-cancel fallback hide firing (visible_before={:?})",
                        visible_before
                    );
                    let _ = window_clone.hide();
                }
            });
        }
    }

    // Notify frontend
    let _ = app.emit(events::EVENT_PIPELINE_CANCELLED, ());
    let _ = app.emit(
        events::EVENT_PIPELINE_STATE_CHANGED,
        PipelineStateEvent::Idle,
    );

    // Disable Escape shortcut now that we're idle.
    set_escape_cancel_shortcut_enabled(app, false);
}

/// Handle a shortcut event - public so it can be called from commands/settings.rs
#[cfg(desktop)]
pub fn handle_shortcut_event(app: &AppHandle, shortcut: &Shortcut, event: &ShortcutEvent) {
    let state = app.state::<AppState>();

    // Get current settings from store
    let sound_enabled: bool = get_setting_from_store(app, "sound_enabled", true);
    let audio_cue_raw: String = get_setting_from_store(app, "audio_cue", "kolboo".to_string());
    let audio_cue = audio::AudioCue::from_str(&audio_cue_raw);
    let playing_audio_handling = get_playing_audio_handling(app);

    // Get shortcut string for comparison (normalized to handle "ctrl" vs "control" differences)
    let shortcut_str = normalize_shortcut_string(&shortcut.to_string());

    let cards = get_hotkey_cards_from_store(app);
    let action_shortcuts = build_action_shortcut_strings(&cards);

    // Compare normalized strings directly
    let matches_action = |action: HotkeyAction| {
        action_shortcuts
            .get(&action)
            .is_some_and(|list| list.iter().any(|s| s == &shortcut_str))
    };

    let is_toggle = matches_action(HotkeyAction::Toggle);
    let is_hold = matches_action(HotkeyAction::Hold);
    let is_paste_last = matches_action(HotkeyAction::PasteLast);
    let is_retry = matches_action(HotkeyAction::Retry);
    let is_quick_ask_hold = matches_action(HotkeyAction::QuickAskHold);
    let is_quick_ask_toggle = matches_action(HotkeyAction::QuickAskToggle);

    if is_toggle {
        handle_toggle_shortcut_event(
            app,
            &state,
            matches!(event.state, ShortcutState::Pressed),
            ToggleShortcutSource::Global,
            sound_enabled,
            audio_cue,
            playing_audio_handling,
        );
    } else if is_hold {
        handle_hold_shortcut_event(
            app,
            &state,
            matches!(event.state, ShortcutState::Pressed),
            HoldShortcutSource::Global,
            sound_enabled,
            audio_cue,
            playing_audio_handling,
        );
    } else if is_paste_last {
        handle_paste_last_shortcut_event(
            app,
            &state,
            matches!(event.state, ShortcutState::Pressed),
            PasteLastShortcutSource::Global,
        );
    } else if is_retry {
        // Retry last recording: action on release (debounced)
        match event.state {
            ShortcutState::Pressed => {
                state.retry_key_held.swap(true, Ordering::SeqCst);
            }
            ShortcutState::Released => {
                if state.retry_key_held.swap(false, Ordering::SeqCst) {
                    log::info!("Retry: retrying last recording");
                    spawn_retry_last_recording_and_output(app, "Retry");
                }
            }
        }
    } else if is_quick_ask_hold {
        handle_quick_ask_hold_shortcut_event(
            app,
            &state,
            matches!(event.state, ShortcutState::Pressed),
            QuickAskHoldShortcutSource::Global,
            sound_enabled,
            audio_cue,
            playing_audio_handling,
        );
    } else if is_quick_ask_toggle {
        handle_quick_ask_toggle_shortcut_event(
            app,
            &state,
            matches!(event.state, ShortcutState::Pressed),
            QuickAskToggleShortcutSource::Global,
            sound_enabled,
            audio_cue,
            playing_audio_handling,
        );
    } else {
        log::warn!("Unknown shortcut: {}", shortcut_str);
    }
}

/// Handle modifier-only key events (Windows-only).
///
/// This is used for hotkeys like "AltRight" with no modifiers.
#[cfg(all(desktop, target_os = "windows"))]
pub(crate) fn handle_modifier_key_event(
    app: &AppHandle,
    key: &str,
    is_down: bool,
    suppress_release_actions: bool,
) {
    let state = app.state::<AppState>();

    let hotkey_debug = crate::windows_modifier_hotkeys::hotkey_debug_runtime_enabled();

    let cards = get_hotkey_cards_from_store(app);
    let is_toggle = cards
        .iter()
        .any(|card| card.kind == HotkeyAction::Toggle && card_matches_modifier_only(card, key));
    let is_hold = cards
        .iter()
        .any(|card| card.kind == HotkeyAction::Hold && card_matches_modifier_only(card, key));
    let is_paste_last = cards
        .iter()
        .any(|card| card.kind == HotkeyAction::PasteLast && card_matches_modifier_only(card, key));
    let is_retry = cards
        .iter()
        .any(|card| card.kind == HotkeyAction::Retry && card_matches_modifier_only(card, key));
    let is_quick_ask_hold = cards.iter().any(|card| {
        card.kind == HotkeyAction::QuickAskHold && card_matches_modifier_only(card, key)
    });
    let is_quick_ask_toggle = cards.iter().any(|card| {
        card.kind == HotkeyAction::QuickAskToggle && card_matches_modifier_only(card, key)
    });

    if !(is_toggle
        || is_hold
        || is_paste_last
        || is_retry
        || is_quick_ask_hold
        || is_quick_ask_toggle)
    {
        if hotkey_debug {
            let action_hotkeys = |action: HotkeyAction| {
                let list: Vec<String> = cards
                    .iter()
                    .filter_map(|card| {
                        if card.kind != action {
                            return None;
                        }
                        card.hotkey.as_ref().map(|hk| hk.to_shortcut_string())
                    })
                    .collect();
                if list.is_empty() {
                    "<disabled>".to_string()
                } else {
                    list.join(", ")
                }
            };
            let details = format!(
                "key={key} is_down={is_down} suppress_release_actions={suppress_release_actions} toggle_hotkey={} hold_hotkey={} paste_last_hotkey={} retry_hotkey={} quick_ask_hold_hotkey={} quick_ask_toggle_hotkey={} (no match)",
                action_hotkeys(HotkeyAction::Toggle),
                action_hotkeys(HotkeyAction::Hold),
                action_hotkeys(HotkeyAction::PasteLast),
                action_hotkeys(HotkeyAction::Retry),
                action_hotkeys(HotkeyAction::QuickAskHold),
                action_hotkeys(HotkeyAction::QuickAskToggle),
            );
            emit_system_event(
                app,
                "debug",
                "Modifier-only hotkey event (ignored)",
                Some(&details),
            );
        }
        return;
    }

    if hotkey_debug {
        let details = format!(
            "key={key} is_down={is_down} suppress_release_actions={suppress_release_actions} match: toggle={is_toggle} hold={is_hold} paste_last={is_paste_last} retry={is_retry} quick_ask_hold={is_quick_ask_hold} quick_ask_toggle={is_quick_ask_toggle}",
        );
        emit_system_event(
            app,
            "debug",
            "Modifier-only hotkey event (matched)",
            Some(&details),
        );
    }

    // Get current settings from store (mirrors handle_shortcut_event behavior)
    let sound_enabled: bool = get_setting_from_store(app, "sound_enabled", true);
    let audio_cue_raw: String = get_setting_from_store(app, "audio_cue", "kolboo".to_string());
    let audio_cue = audio::AudioCue::from_str(&audio_cue_raw);
    let playing_audio_handling = get_playing_audio_handling(app);

    if is_toggle {
        handle_toggle_shortcut_event(
            app,
            &state,
            is_down,
            ToggleShortcutSource::ModifierOnly {
                key,
                suppress_release_actions,
                hotkey_debug,
            },
            sound_enabled,
            audio_cue,
            playing_audio_handling,
        );

        return;
    }

    if is_hold {
        handle_hold_shortcut_event(
            app,
            &state,
            is_down,
            HoldShortcutSource::ModifierOnly { key },
            sound_enabled,
            audio_cue,
            playing_audio_handling,
        );

        return;
    }

    if is_paste_last {
        handle_paste_last_shortcut_event(
            app,
            &state,
            is_down,
            PasteLastShortcutSource::ModifierOnly {
                key,
                suppress_release_actions,
            },
        );

        return;
    }

    if is_retry {
        let retry_label = format!("RetryLast({key})");

        // Retry-last-recording: action on release (debounced)
        if is_down {
            state.retry_key_held.swap(true, Ordering::SeqCst);
            return;
        }

        let was_held = state.retry_key_held.swap(false, Ordering::SeqCst);
        if !was_held {
            return;
        }

        if suppress_release_actions {
            return;
        }

        log::info!("{}: retrying last recording", retry_label);
        spawn_retry_last_recording_and_output(app, &retry_label);

        return;
    }

    if is_quick_ask_hold {
        handle_quick_ask_hold_shortcut_event(
            app,
            &state,
            is_down,
            QuickAskHoldShortcutSource::ModifierOnly { key, hotkey_debug },
            sound_enabled,
            audio_cue,
            playing_audio_handling,
        );

        return;
    }

    if is_quick_ask_toggle {
        handle_quick_ask_toggle_shortcut_event(
            app,
            &state,
            is_down,
            QuickAskToggleShortcutSource::ModifierOnly {
                key,
                suppress_release_actions,
                hotkey_debug,
            },
            sound_enabled,
            audio_cue,
            playing_audio_handling,
        );
    }
}

/// Register shortcuts from store settings (called from setup() after store plugin is available)
#[cfg(desktop)]
pub(crate) fn register_initial_shortcuts(
    app: &AppHandle,
) -> Result<(), Box<dyn std::error::Error>> {
    let cards = get_hotkey_cards_from_store(app);

    // Startup and runtime registration intentionally share lifecycle decisions so adding a
    // hotkey action or Windows-hook Adapter cannot accidentally diverge after restart.
    sync_windows_modifier_hook_flags(&cards);
    register_hotkey_cards(app, &cards, HotkeyRegistrationMode::StartupBestEffort)
        .map_err(|e| Box::<dyn std::error::Error>::from(std::io::Error::other(e)))?;

    // Never abort startup due to hotkey registration failures.
    Ok(())
}

#[cfg(desktop)]
pub(crate) fn build_global_shortcut_plugin() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    // Just initialize the plugin - shortcuts will be registered in setup() after store is available
    tauri_plugin_global_shortcut::Builder::new().build()
}
