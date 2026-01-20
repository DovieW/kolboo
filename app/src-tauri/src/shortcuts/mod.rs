use std::sync::atomic::Ordering;

use tauri::{AppHandle, Emitter, Manager};

#[cfg(desktop)]
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutEvent, ShortcutState};

#[cfg(desktop)]
use tauri_plugin_store::StoreExt;

use crate::audio;
use crate::commands;
use crate::events;
use crate::history::HistoryStorage;
use crate::pipeline;
use crate::recordings::RecordingStore;
use crate::request_log::RequestLogStore;
use crate::settings::HotkeyConfig as InternalHotkeyConfig;
use crate::settings::HotkeyConfig;
use crate::shortcuts_lock;
use crate::state::AppState;
use crate::{
    emit_system_event, get_playing_audio_handling, get_setting_from_store, start_recording,
    stop_recording, toggle_media_play_pause, AudioMuteManager, PipelineStateEvent,
};

// ============================================================================
// Retry-last-recording hotkey support
// ============================================================================

/// Resolve the most recent history entry id that has a persisted recording available.
///
/// This is used by the Retry hotkey to pick "the last recording".
#[cfg(desktop)]
fn resolve_last_recording_history_entry_id(app: &AppHandle) -> Option<String> {
    let history = app.try_state::<HistoryStorage>()?;
    let store = app.try_state::<RecordingStore>()?;

    // Be conservative on work done inside shortcut-triggered paths.
    let entries = history.get_all(Some(50)).ok()?;
    for entry in entries.iter() {
        // Prefer an explicit recording pointer (covers reruns), but fall back to
        // legacy storage where the WAV is stored under the entry id.
        let candidate_ids = [
            entry.recording_request_id.as_deref(),
            Some(entry.id.as_str()),
        ];

        if candidate_ids
            .iter()
            .flatten()
            .any(|rid| store.has(rid.trim()))
        {
            return Some(entry.id.clone());
        }
    }

    None
}

/// Retry the last available recording and output the result.
///
/// Intended for use by the global Retry hotkey (so it shows the overlay loading state
/// even when the overlay is normally hidden).
#[cfg(desktop)]
pub(crate) fn spawn_retry_last_recording_and_output(app: &AppHandle, source: &str) {
    let app = app.clone();
    let source = source.to_string();

    tauri::async_runtime::spawn(async move {
        let Some(pipeline) = app.try_state::<pipeline::SharedPipeline>() else {
            log::warn!("{source}: pipeline not available; cannot retry");
            return;
        };
        let pipeline = (*pipeline).clone();

        let pipeline_state = pipeline.state();
        if !matches!(
            pipeline_state,
            pipeline::PipelineState::Idle | pipeline::PipelineState::Error
        ) {
            log::info!(
                "{source}: retry ignored (pipeline busy: {:?})",
                pipeline_state
            );
            return;
        }

        let Some(history_entry_id) = resolve_last_recording_history_entry_id(&app) else {
            log::info!("{source}: no recording available to retry");
            emit_system_event(&app, "shortcut", "Retry: no recording available", None);
            return;
        };

        // Force-show overlay so the user gets the loading state UX.
        if let Err(e) = commands::overlay::show_overlay_with_reset_if_not_always(&app) {
            log::warn!("{source}: failed to show overlay for retry: {}", e);
        }

        let transcript = match commands::recording::pipeline_retry_transcription_impl(
            app.clone(),
            pipeline.clone(),
            history_entry_id,
        )
        .await
        {
            Ok(t) => t,
            Err(e) => {
                log::warn!("{source}: retry failed: {}", e.message);
                return;
            }
        };

        let Some(text) = crate::sanitize_transcript(&transcript) else {
            log::info!("{source}: retry returned empty transcript; nothing to output");
            return;
        };

        let output_mode_str: String =
            get_setting_from_store(&app, "output_mode", "paste".to_string());
        let output_mode = commands::text::OutputMode::from_str(&output_mode_str);
        let output_hit_enter: bool = get_setting_from_store(&app, "output_hit_enter", false);
        let output_clipboard_privacy_mode: bool =
            get_setting_from_store(&app, "output_clipboard_privacy_mode", false);

        if let Err(e) = commands::text::output_text_with_mode_options(
            &text,
            output_mode,
            output_hit_enter,
            !output_clipboard_privacy_mode,
        ) {
            log::error!("{source}: failed to output retry transcript: {}", e);
        }
    });
}

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

// ============================================================================
// Escape-to-cancel support
// ============================================================================

#[cfg(desktop)]
const ESCAPE_CANCEL_SHORTCUT: &str = "Escape";

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
    let shortcut_manager = app.global_shortcut();

    let is_registered = shortcut_manager.is_registered(ESCAPE_CANCEL_SHORTCUT);
    log::debug!(
        "Escape shortcut toggle: enabled={} (currently registered={})",
        enabled,
        is_registered
    );

    if enabled {
        if is_registered {
            return;
        }

        if let Err(e) =
            shortcut_manager.on_shortcut(ESCAPE_CANCEL_SHORTCUT, |app, _shortcut, event| {
                if event.state == ShortcutState::Pressed {
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
                if current_mode == "recording_only" && current_epoch == expected_epoch {
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

    // Get configured hotkeys from store.
    // - missing => default
    // - null => disabled
    // - invalid => default
    let toggle_hotkey =
        get_hotkey_from_store(app, "toggle_hotkey", HotkeyConfig::default_toggle_opt);
    let hold_hotkey = get_hotkey_from_store(app, "hold_hotkey", HotkeyConfig::default_hold);
    let paste_last_hotkey =
        get_hotkey_from_store(app, "paste_last_hotkey", HotkeyConfig::default_paste_last);
    let retry_hotkey = get_hotkey_from_store(app, "retry_hotkey", HotkeyConfig::default_retry);

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

    // Convert to normalized shortcut strings.
    // For disabled hotkeys, we keep None so it can never match.
    let toggle_shortcut_str: Option<String> = toggle_hotkey.map(|hk| {
        let shortcut_str = hk
            .to_shortcut()
            .map(|_| hk.to_shortcut_string())
            .unwrap_or_else(|_| HotkeyConfig::default_toggle().to_shortcut_string());
        normalize_shortcut_string(&shortcut_str)
    });
    let hold_shortcut_str: Option<String> = hold_hotkey.and_then(|hk| {
        hk.to_shortcut()
            .map(|_| normalize_shortcut_string(&hk.to_shortcut_string()))
            .map_err(|e| {
                log::warn!(
                    "Invalid hold hotkey in settings store ({}); treating as disabled",
                    e
                )
            })
            .ok()
    });
    let paste_last_shortcut_str: Option<String> = paste_last_hotkey.and_then(|hk| {
        hk.to_shortcut()
            .map(|_| normalize_shortcut_string(&hk.to_shortcut_string()))
            .map_err(|e| {
                log::warn!(
                    "Invalid paste-last hotkey in settings store ({}); treating as disabled",
                    e
                )
            })
            .ok()
    });
    let retry_shortcut_str: Option<String> = retry_hotkey.and_then(|hk| {
        hk.to_shortcut()
            .map(|_| normalize_shortcut_string(&hk.to_shortcut_string()))
            .map_err(|e| {
                log::warn!(
                    "Invalid retry hotkey in settings store ({}); treating as disabled",
                    e
                )
            })
            .ok()
    });

    let quick_ask_hold_shortcut_str: Option<String> = quick_ask_hold_hotkey.and_then(|hk| {
        hk.to_shortcut()
            .map(|_| normalize_shortcut_string(&hk.to_shortcut_string()))
            .map_err(|e| {
                log::warn!(
                    "Invalid quick ask hold hotkey in settings store ({}); treating as disabled",
                    e
                )
            })
            .ok()
    });

    let quick_ask_toggle_shortcut_str: Option<String> = quick_ask_toggle_hotkey.and_then(|hk| {
        hk.to_shortcut()
            .map(|_| normalize_shortcut_string(&hk.to_shortcut_string()))
            .map_err(|e| {
                log::warn!(
                    "Invalid quick ask toggle hotkey in settings store ({}); treating as disabled",
                    e
                )
            })
            .ok()
    });

    // Get audio mute manager if available
    let audio_mute_manager = app.try_state::<AudioMuteManager>();

    // Compare normalized strings directly
    let is_toggle = toggle_shortcut_str.as_deref() == Some(shortcut_str.as_str());
    let is_hold = hold_shortcut_str.as_deref() == Some(shortcut_str.as_str());
    let is_paste_last = paste_last_shortcut_str.as_deref() == Some(shortcut_str.as_str());
    let is_retry = retry_shortcut_str.as_deref() == Some(shortcut_str.as_str());
    let is_quick_ask_hold = quick_ask_hold_shortcut_str.as_deref() == Some(shortcut_str.as_str());
    let is_quick_ask_toggle =
        quick_ask_toggle_shortcut_str.as_deref() == Some(shortcut_str.as_str());

    if is_toggle {
        // Toggle mode: action happens on key release (debounced)
        match event.state {
            ShortcutState::Pressed => {
                state.toggle_key_held.swap(true, Ordering::SeqCst);
            }
            ShortcutState::Released => {
                if state.toggle_key_held.swap(false, Ordering::SeqCst) {
                    // Check pipeline state directly instead of AppState
                    let pipeline_state = app
                        .try_state::<pipeline::SharedPipeline>()
                        .map(|p| p.state());

                    log::info!("Toggle released: pipeline state = {:?}", pipeline_state);
                    emit_system_event(
                        app,
                        "shortcut",
                        "Toggle key released",
                        Some(&format!("Pipeline state: {:?}", pipeline_state)),
                    );

                    // Do not allow starting a new capture while we are processing a previous one.
                    // This avoids a brief error UI flash if the user taps the toggle again.
                    if matches!(
                        pipeline_state,
                        Some(
                            pipeline::PipelineState::Transcribing
                                | pipeline::PipelineState::Rewriting
                        )
                    ) {
                        log::info!("Toggle ignored (pipeline busy: {:?})", pipeline_state);
                        return;
                    }

                    let can_stop = pipeline_state
                        .map(|s| s.can_stop_recording())
                        .unwrap_or(false);
                    let can_start = pipeline_state
                        .map(|s| s.can_start_recording())
                        .unwrap_or(false);

                    if can_stop {
                        stop_recording(
                            app,
                            &state,
                            sound_enabled,
                            audio_cue,
                            &audio_mute_manager,
                            playing_audio_handling,
                            "Toggle",
                        );
                    } else if can_start {
                        start_recording(
                            app,
                            &state,
                            sound_enabled,
                            audio_cue,
                            &audio_mute_manager,
                            playing_audio_handling,
                            "Toggle",
                        );
                    } else {
                        log::info!("Toggle ignored (pipeline state: {:?})", pipeline_state);
                    }
                }
            }
        }
    } else if is_hold {
        // Hold-to-Record: start on press, stop on release
        match event.state {
            ShortcutState::Pressed => {
                if !state.ptt_key_held.swap(true, Ordering::SeqCst) {
                    // Only start if pipeline is not already recording/transcribing
                    let pipeline_state = app
                        .try_state::<pipeline::SharedPipeline>()
                        .map(|p| p.state());

                    log::info!("Hold pressed: pipeline state = {:?}", pipeline_state);
                    emit_system_event(
                        app,
                        "shortcut",
                        "Hold key pressed",
                        Some(&format!("Pipeline state: {:?}", pipeline_state)),
                    );

                    let can_start = pipeline_state
                        .map(|s| s.can_start_recording())
                        .unwrap_or(false);

                    if can_start {
                        start_recording(
                            app,
                            &state,
                            sound_enabled,
                            audio_cue,
                            &audio_mute_manager,
                            playing_audio_handling,
                            "Hold",
                        );
                    }
                }
            }
            ShortcutState::Released => {
                if state.ptt_key_held.swap(false, Ordering::SeqCst) {
                    // Only stop if pipeline is actually recording
                    let is_recording = app
                        .try_state::<pipeline::SharedPipeline>()
                        .map(|p| p.state() == pipeline::PipelineState::Recording)
                        .unwrap_or(false);

                    if is_recording {
                        stop_recording(
                            app,
                            &state,
                            sound_enabled,
                            audio_cue,
                            &audio_mute_manager,
                            playing_audio_handling,
                            "Hold",
                        );
                    }
                }
            }
        }
    } else if is_paste_last {
        // Output last transcription: hold-to-output (output happens on release)
        match event.state {
            ShortcutState::Pressed => {
                // Mark key as held (ignore OS key repeat)
                state.paste_key_held.swap(true, Ordering::SeqCst);
            }
            ShortcutState::Released => {
                if state.paste_key_held.swap(false, Ordering::SeqCst) {
                    // Key released - output based on configured mode
                    log::info!("OutputLast: outputting last transcription");

                    // Get output mode from settings
                    let output_mode_str: String =
                        get_setting_from_store(app, "output_mode", "paste".to_string());
                    let output_mode = commands::text::OutputMode::from_str(&output_mode_str);

                    let output_hit_enter: bool =
                        get_setting_from_store(app, "output_hit_enter", false);
                    let output_clipboard_privacy_mode: bool =
                        get_setting_from_store(app, "output_clipboard_privacy_mode", false);

                    let history_storage = app.state::<HistoryStorage>();

                    if let Ok(entries) = history_storage.get_all(Some(1)) {
                        if let Some(entry) = entries.first() {
                            if let Err(e) = commands::text::output_text_with_mode_options(
                                &entry.text,
                                output_mode,
                                output_hit_enter,
                                !output_clipboard_privacy_mode,
                            ) {
                                log::error!("Failed to output last transcription: {}", e);
                            }
                        } else {
                            log::info!("OutputLast: no history entries available");
                        }
                    }
                }
            }
        }
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
        // Quick Ask Hold: hold-to-record.
        // Start capture on press; stop on release, then branch into the Quick Ask answer flow.
        match event.state {
            ShortcutState::Pressed => {
                if !state.quick_ask_key_held.swap(true, Ordering::SeqCst) {
                    // Only start if pipeline is not already recording/transcribing
                    let pipeline_state = app
                        .try_state::<pipeline::SharedPipeline>()
                        .map(|p| p.state());

                    log::info!(
                        "QuickAskHold pressed: pipeline state = {:?}",
                        pipeline_state
                    );
                    emit_system_event(
                        app,
                        "shortcut",
                        "Quick Ask Hold pressed",
                        Some(&format!("Pipeline state: {:?}", pipeline_state)),
                    );

                    // Do not allow starting while we are processing a previous capture.
                    if matches!(
                        pipeline_state,
                        Some(
                            pipeline::PipelineState::Transcribing
                                | pipeline::PipelineState::Rewriting
                        )
                    ) {
                        log::info!("QuickAskHold ignored (pipeline busy: {:?})", pipeline_state);
                        return;
                    }

                    let can_start = pipeline_state
                        .map(|s| s.can_start_recording())
                        .unwrap_or(false);

                    if can_start {
                        state.quick_ask_session_active.store(true, Ordering::SeqCst);
                        start_recording(
                            app,
                            &state,
                            sound_enabled,
                            audio_cue,
                            &audio_mute_manager,
                            playing_audio_handling,
                            "QuickAskHold",
                        );

                        // Defensive: if we failed to enter Recording, clear the intent.
                        let is_recording = app
                            .try_state::<pipeline::SharedPipeline>()
                            .map(|p| p.state() == pipeline::PipelineState::Recording)
                            .unwrap_or(false);
                        if !is_recording {
                            state
                                .quick_ask_session_active
                                .store(false, Ordering::SeqCst);
                        }
                    }
                }
            }
            ShortcutState::Released => {
                if state.quick_ask_key_held.swap(false, Ordering::SeqCst) {
                    // Only stop if pipeline is actually recording
                    let is_recording = app
                        .try_state::<pipeline::SharedPipeline>()
                        .map(|p| p.state() == pipeline::PipelineState::Recording)
                        .unwrap_or(false);

                    if is_recording {
                        stop_recording(
                            app,
                            &state,
                            sound_enabled,
                            audio_cue,
                            &audio_mute_manager,
                            playing_audio_handling,
                            "QuickAskHold",
                        );
                    } else {
                        // If we never actually started capture, clear the intent.
                        state
                            .quick_ask_session_active
                            .store(false, Ordering::SeqCst);
                    }
                }
            }
        }
    } else if is_quick_ask_toggle {
        // Quick Ask Toggle: press once to start, press again to stop.
        // Debounce: action happens on key release.
        match event.state {
            ShortcutState::Pressed => {
                state.quick_ask_toggle_key_held.swap(true, Ordering::SeqCst);
            }
            ShortcutState::Released => {
                if state
                    .quick_ask_toggle_key_held
                    .swap(false, Ordering::SeqCst)
                {
                    let pipeline_state = app
                        .try_state::<pipeline::SharedPipeline>()
                        .map(|p| p.state());

                    log::info!(
                        "QuickAskToggle released: pipeline state = {:?}",
                        pipeline_state
                    );
                    emit_system_event(
                        app,
                        "shortcut",
                        "Quick Ask Toggle released",
                        Some(&format!("Pipeline state: {:?}", pipeline_state)),
                    );

                    // Do not allow starting while we are processing a previous capture.
                    if matches!(
                        pipeline_state,
                        Some(
                            pipeline::PipelineState::Transcribing
                                | pipeline::PipelineState::Rewriting
                        )
                    ) {
                        log::info!(
                            "QuickAskToggle ignored (pipeline busy: {:?})",
                            pipeline_state
                        );
                        return;
                    }

                    let can_stop = pipeline_state
                        .map(|s| s.can_stop_recording())
                        .unwrap_or(false);
                    let can_start = pipeline_state
                        .map(|s| s.can_start_recording())
                        .unwrap_or(false);

                    if can_stop {
                        // Only stop if this recording session is actually a Quick Ask.
                        let is_quick_ask_session =
                            state.quick_ask_session_active.load(Ordering::SeqCst);
                        if is_quick_ask_session {
                            stop_recording(
                                app,
                                &state,
                                sound_enabled,
                                audio_cue,
                                &audio_mute_manager,
                                playing_audio_handling,
                                "QuickAskToggle",
                            );
                        } else {
                            log::info!(
                                "QuickAskToggle stop ignored (active session is not Quick Ask)"
                            );
                        }
                    } else if can_start {
                        state.quick_ask_session_active.store(true, Ordering::SeqCst);
                        start_recording(
                            app,
                            &state,
                            sound_enabled,
                            audio_cue,
                            &audio_mute_manager,
                            playing_audio_handling,
                            "QuickAskToggle",
                        );

                        // Defensive: if we failed to enter Recording, clear the intent.
                        let is_recording = app
                            .try_state::<pipeline::SharedPipeline>()
                            .map(|p| p.state() == pipeline::PipelineState::Recording)
                            .unwrap_or(false);
                        if !is_recording {
                            state
                                .quick_ask_session_active
                                .store(false, Ordering::SeqCst);
                        }
                    } else {
                        log::info!(
                            "QuickAskToggle ignored (pipeline state: {:?})",
                            pipeline_state
                        );
                    }
                }
            }
        }
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

    let toggle_label = format!("Toggle({key})");
    let hold_label = format!("Hold({key})");
    let paste_last_label = format!("OutputLast({key})");
    let quick_ask_hold_label = format!("QuickAskHold({key})");
    let quick_ask_toggle_label = format!("QuickAskToggle({key})");

    let hotkey_debug = crate::windows_modifier_hotkeys::hotkey_debug_runtime_enabled();

    // Determine which (if any) configured hotkey uses this modifier-only key.
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

    let matches_modifier_only = |hk: &HotkeyConfig| hk.modifiers.is_empty() && hk.key == key;
    let is_toggle = toggle_hotkey.as_ref().is_some_and(matches_modifier_only);
    let is_hold = hold_hotkey.as_ref().is_some_and(matches_modifier_only);
    let is_paste_last = paste_last_hotkey
        .as_ref()
        .is_some_and(matches_modifier_only);
    let is_retry = retry_hotkey.as_ref().is_some_and(matches_modifier_only);
    let is_quick_ask_hold = quick_ask_hold_hotkey
        .as_ref()
        .is_some_and(matches_modifier_only);
    let is_quick_ask_toggle = quick_ask_toggle_hotkey
        .as_ref()
        .is_some_and(matches_modifier_only);

    if !(is_toggle
        || is_hold
        || is_paste_last
        || is_retry
        || is_quick_ask_hold
        || is_quick_ask_toggle)
    {
        if hotkey_debug {
            let details = format!(
                "key={key} is_down={is_down} suppress_release_actions={suppress_release_actions} toggle_hotkey={} hold_hotkey={} paste_last_hotkey={} retry_hotkey={} quick_ask_hold_hotkey={} quick_ask_toggle_hotkey={} (no match)",
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
                    .unwrap_or_else(|| "<disabled>".to_string()),
                quick_ask_toggle_hotkey
                    .as_ref()
                    .map(|h| h.to_shortcut_string())
                    .unwrap_or_else(|| "<disabled>".to_string()),
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
    let audio_mute_manager = app.try_state::<AudioMuteManager>();

    if is_toggle {
        // Toggle mode: action happens on key release (debounced)
        if is_down {
            state.toggle_key_held.swap(true, Ordering::SeqCst);
        } else {
            let was_held = state.toggle_key_held.swap(false, Ordering::SeqCst);
            if suppress_release_actions {
                if hotkey_debug {
                    emit_system_event(
                        app,
                        "debug",
                        &format!("{toggle_label}: release suppressed"),
                        Some("AltGr/typing suppression triggered"),
                    );
                }
                return;
            }

            if was_held {
                let pipeline_state = app
                    .try_state::<pipeline::SharedPipeline>()
                    .map(|p| p.state());

                if hotkey_debug {
                    emit_system_event(
                        app,
                        "debug",
                        &format!("{toggle_label}: key released"),
                        Some(&format!("Pipeline state: {:?}", pipeline_state)),
                    );
                }

                // Do not allow starting a new capture while we are processing a previous one.
                if matches!(
                    pipeline_state,
                    Some(
                        pipeline::PipelineState::Transcribing | pipeline::PipelineState::Rewriting
                    )
                ) {
                    log::info!(
                        "{} ignored (pipeline busy: {:?})",
                        toggle_label,
                        pipeline_state
                    );

                    if hotkey_debug {
                        emit_system_event(
                            app,
                            "debug",
                            &format!("{toggle_label} ignored (pipeline busy)"),
                            Some(&format!("Pipeline state: {:?}", pipeline_state)),
                        );
                    }
                    return;
                }

                let can_stop = pipeline_state
                    .map(|s| s.can_stop_recording())
                    .unwrap_or(false);
                let can_start = pipeline_state
                    .map(|s| s.can_start_recording())
                    .unwrap_or(false);

                if can_stop {
                    stop_recording(
                        app,
                        &state,
                        sound_enabled,
                        audio_cue,
                        &audio_mute_manager,
                        playing_audio_handling,
                        &toggle_label,
                    );
                } else if can_start {
                    start_recording(
                        app,
                        &state,
                        sound_enabled,
                        audio_cue,
                        &audio_mute_manager,
                        playing_audio_handling,
                        &toggle_label,
                    );
                } else {
                    log::info!(
                        "{} ignored (pipeline state: {:?})",
                        toggle_label,
                        pipeline_state
                    );

                    if hotkey_debug {
                        emit_system_event(
                            app,
                            "debug",
                            &format!("{toggle_label} ignored (cannot start/stop)"),
                            Some(&format!("Pipeline state: {:?}", pipeline_state)),
                        );
                    }
                }
            } else if hotkey_debug {
                emit_system_event(
                    app,
                    "debug",
                    &format!("{toggle_label}: key released but was_held=false"),
                    Some("Down event was not observed/latched"),
                );
            }
        }

        return;
    }

    if is_hold {
        // Hold-to-Record: start on press, stop on release
        if is_down {
            if !state.ptt_key_held.swap(true, Ordering::SeqCst) {
                let pipeline_state = app
                    .try_state::<pipeline::SharedPipeline>()
                    .map(|p| p.state());
                let can_start = pipeline_state
                    .map(|s| s.can_start_recording())
                    .unwrap_or(false);
                if can_start {
                    start_recording(
                        app,
                        &state,
                        sound_enabled,
                        audio_cue,
                        &audio_mute_manager,
                        playing_audio_handling,
                        &hold_label,
                    );
                }
            }
        } else if state.ptt_key_held.swap(false, Ordering::SeqCst) {
            let is_recording = app
                .try_state::<pipeline::SharedPipeline>()
                .map(|p| p.state() == pipeline::PipelineState::Recording)
                .unwrap_or(false);
            if is_recording {
                stop_recording(
                    app,
                    &state,
                    sound_enabled,
                    audio_cue,
                    &audio_mute_manager,
                    playing_audio_handling,
                    &hold_label,
                );
            }
        }

        return;
    }

    if is_paste_last {
        // Paste-last: action on release (debounced)
        if is_down {
            state.paste_key_held.swap(true, Ordering::SeqCst);
            return;
        }

        let was_held = state.paste_key_held.swap(false, Ordering::SeqCst);
        if !was_held {
            return;
        }

        if suppress_release_actions {
            return;
        }

        // Key released - output based on configured mode
        log::info!("{}: outputting last transcription", paste_last_label);

        let output_mode_str: String =
            get_setting_from_store(app, "output_mode", "paste".to_string());
        let output_mode = commands::text::OutputMode::from_str(&output_mode_str);
        let output_hit_enter: bool = get_setting_from_store(app, "output_hit_enter", false);
        let output_clipboard_privacy_mode: bool =
            get_setting_from_store(app, "output_clipboard_privacy_mode", false);

        let history_storage = app.state::<HistoryStorage>();

        if let Ok(entries) = history_storage.get_all(Some(1)) {
            if let Some(entry) = entries.first() {
                if let Err(e) = commands::text::output_text_with_mode_options(
                    &entry.text,
                    output_mode,
                    output_hit_enter,
                    !output_clipboard_privacy_mode,
                ) {
                    log::error!("Failed to output last transcription: {}", e);
                }
            } else {
                log::info!("{}: no history entries available", paste_last_label);
            }
        }

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
        // Quick Ask Hold: start on press, stop on release.
        if is_down {
            if !state.quick_ask_key_held.swap(true, Ordering::SeqCst) {
                let pipeline_state = app
                    .try_state::<pipeline::SharedPipeline>()
                    .map(|p| p.state());

                // Do not allow starting while we are processing a previous capture.
                if matches!(
                    pipeline_state,
                    Some(
                        pipeline::PipelineState::Transcribing | pipeline::PipelineState::Rewriting
                    )
                ) {
                    log::info!(
                        "{} ignored (pipeline busy: {:?})",
                        quick_ask_hold_label,
                        pipeline_state
                    );
                    return;
                }

                let can_start = pipeline_state
                    .map(|s| s.can_start_recording())
                    .unwrap_or(false);
                if can_start {
                    state.quick_ask_session_active.store(true, Ordering::SeqCst);
                    start_recording(
                        app,
                        &state,
                        sound_enabled,
                        audio_cue,
                        &audio_mute_manager,
                        playing_audio_handling,
                        &quick_ask_hold_label,
                    );

                    let is_recording = app
                        .try_state::<pipeline::SharedPipeline>()
                        .map(|p| p.state() == pipeline::PipelineState::Recording)
                        .unwrap_or(false);
                    if !is_recording {
                        state
                            .quick_ask_session_active
                            .store(false, Ordering::SeqCst);
                    }
                }
            }
        } else if state.quick_ask_key_held.swap(false, Ordering::SeqCst) {
            let is_recording = app
                .try_state::<pipeline::SharedPipeline>()
                .map(|p| p.state() == pipeline::PipelineState::Recording)
                .unwrap_or(false);
            if is_recording {
                stop_recording(
                    app,
                    &state,
                    sound_enabled,
                    audio_cue,
                    &audio_mute_manager,
                    playing_audio_handling,
                    &quick_ask_hold_label,
                );
            } else {
                state
                    .quick_ask_session_active
                    .store(false, Ordering::SeqCst);
            }
        }

        return;
    }

    if is_quick_ask_toggle {
        // Quick Ask Toggle: action on release (debounced).
        if is_down {
            state.quick_ask_toggle_key_held.swap(true, Ordering::SeqCst);
            return;
        }

        let was_held = state
            .quick_ask_toggle_key_held
            .swap(false, Ordering::SeqCst);
        if !was_held {
            return;
        }

        if suppress_release_actions {
            return;
        }

        let pipeline_state = app
            .try_state::<pipeline::SharedPipeline>()
            .map(|p| p.state());

        // Do not allow starting while we are processing a previous capture.
        if matches!(
            pipeline_state,
            Some(pipeline::PipelineState::Transcribing | pipeline::PipelineState::Rewriting)
        ) {
            log::info!(
                "{} ignored (pipeline busy: {:?})",
                quick_ask_toggle_label,
                pipeline_state
            );
            return;
        }

        let can_stop = pipeline_state
            .map(|s| s.can_stop_recording())
            .unwrap_or(false);
        let can_start = pipeline_state
            .map(|s| s.can_start_recording())
            .unwrap_or(false);

        if can_stop {
            let is_quick_ask_session = state.quick_ask_session_active.load(Ordering::SeqCst);
            if is_quick_ask_session {
                stop_recording(
                    app,
                    &state,
                    sound_enabled,
                    audio_cue,
                    &audio_mute_manager,
                    playing_audio_handling,
                    &quick_ask_toggle_label,
                );
            } else {
                log::info!(
                    "{} stop ignored (active session is not Quick Ask)",
                    quick_ask_toggle_label
                );
            }
        } else if can_start {
            state.quick_ask_session_active.store(true, Ordering::SeqCst);
            start_recording(
                app,
                &state,
                sound_enabled,
                audio_cue,
                &audio_mute_manager,
                playing_audio_handling,
                &quick_ask_toggle_label,
            );

            let is_recording = app
                .try_state::<pipeline::SharedPipeline>()
                .map(|p| p.state() == pipeline::PipelineState::Recording)
                .unwrap_or(false);
            if !is_recording {
                state
                    .quick_ask_session_active
                    .store(false, Ordering::SeqCst);
            }
        }
    }
}

/// Register shortcuts from store settings (called from setup() after store plugin is available)
#[cfg(desktop)]
pub(crate) fn register_initial_shortcuts(
    app: &AppHandle,
) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(all(desktop, target_os = "windows"))]
    fn is_windows_hook_handled_hotkey(hk: &HotkeyConfig) -> bool {
        hk.modifiers.is_empty() && matches!(hk.key.as_str(), "AltRight" | "Copilot")
    }

    // Read hotkeys from store.
    // - missing => default
    // - null => disabled
    // - invalid => default
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

    // Keep Windows hook behavior in sync with settings at startup.
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

    // Convert to shortcut strings with validation.
    //
    // Windows-only note:
    // - Modifier-only hotkeys (e.g. AltRight) are handled by the low-level hook
    //   in windows_modifier_hotkeys.rs and are NOT registered with
    //   tauri-plugin-global-shortcut.
    //
    // We must not fall back to a different key (like the historical F3) here,
    // otherwise both keys can end up toggling recording.
    // NOTE: We intentionally register each shortcut individually so that a conflict
    // (e.g. another app already using Ctrl+F3) doesn't prevent the app from starting.
    let toggle_shortcut_str: Option<String> = toggle_hotkey.and_then(|hk| {
        #[cfg(all(desktop, target_os = "windows"))]
        if is_windows_hook_handled_hotkey(&hk) {
            return None;
        }

        match hk.to_shortcut() {
            Ok(_) => Some(hk.to_shortcut_string()),
            Err(e) => {
                log::warn!(
                    "Invalid toggle hotkey in settings store ({}); treating as disabled",
                    e
                );
                None
            }
        }
    });
    let hold_shortcut_str: Option<String> = hold_hotkey.and_then(|hk| {
        #[cfg(all(desktop, target_os = "windows"))]
        if is_windows_hook_handled_hotkey(&hk) {
            return None;
        }

        hk.to_shortcut()
            .map(|_| hk.to_shortcut_string())
            .map_err(|e| {
                log::warn!(
                    "Invalid hold hotkey in settings store ({}); treating as disabled",
                    e
                )
            })
            .ok()
    });
    let paste_last_shortcut_str: Option<String> = paste_last_hotkey.and_then(|hk| {
        #[cfg(all(desktop, target_os = "windows"))]
        if is_windows_hook_handled_hotkey(&hk) {
            return None;
        }

        hk.to_shortcut()
            .map(|_| hk.to_shortcut_string())
            .map_err(|e| {
                log::warn!(
                    "Invalid paste-last hotkey in settings store ({}); treating as disabled",
                    e
                )
            })
            .ok()
    });

    let retry_shortcut_str: Option<String> = retry_hotkey.and_then(|hk| {
        #[cfg(all(desktop, target_os = "windows"))]
        if is_windows_hook_handled_hotkey(&hk) {
            return None;
        }

        hk.to_shortcut()
            .map(|_| hk.to_shortcut_string())
            .map_err(|e| {
                log::warn!(
                    "Invalid retry hotkey in settings store ({}); treating as disabled",
                    e
                )
            })
            .ok()
    });

    let quick_ask_hold_shortcut_str: Option<String> = quick_ask_hold_hotkey.and_then(|hk| {
        #[cfg(all(desktop, target_os = "windows"))]
        if is_windows_hook_handled_hotkey(&hk) {
            return None;
        }

        hk.to_shortcut()
            .map(|_| hk.to_shortcut_string())
            .map_err(|e| {
                log::warn!(
                    "Invalid quick ask hold hotkey in settings store ({}); treating as disabled",
                    e
                )
            })
            .ok()
    });

    let quick_ask_toggle_shortcut_str: Option<String> = quick_ask_toggle_hotkey.and_then(|hk| {
        #[cfg(all(desktop, target_os = "windows"))]
        if is_windows_hook_handled_hotkey(&hk) {
            return None;
        }

        hk.to_shortcut()
            .map(|_| hk.to_shortcut_string())
            .map_err(|e| {
                log::warn!(
                    "Invalid quick ask toggle hotkey in settings store ({}); treating as disabled",
                    e
                )
            })
            .ok()
    });

    log::info!(
        "Registering shortcuts - Toggle: {}, Hold: {}, PasteLast: {}, Retry: {}, QuickAskHold: {}, QuickAskToggle: {}",
        toggle_shortcut_str.as_deref().unwrap_or("<disabled>"),
        hold_shortcut_str.as_deref().unwrap_or("<disabled>"),
        paste_last_shortcut_str.as_deref().unwrap_or("<disabled>"),
        retry_shortcut_str.as_deref().unwrap_or("<disabled>"),
        quick_ask_hold_shortcut_str.as_deref().unwrap_or("<disabled>"),
        quick_ask_toggle_shortcut_str.as_deref().unwrap_or("<disabled>")
    );

    let shortcut_manager = app.global_shortcut();

    // Register each shortcut independently; on failure we log + emit a warning event.
    let mut failures: Vec<String> = Vec::new();

    if let Some(toggle_shortcut_str) = &toggle_shortcut_str {
        let toggle_shortcut = <Shortcut as std::str::FromStr>::from_str(toggle_shortcut_str)
            .map_err(|e| {
                failures.push(format!(
                    "Toggle ({}) => failed to parse shortcut: {:?}",
                    toggle_shortcut_str, e
                ));
            });
        if let Ok(toggle_shortcut) = toggle_shortcut {
            if let Err(e) = shortcut_manager.on_shortcut(toggle_shortcut, |app, shortcut, event| {
                handle_shortcut_event(app, shortcut, &event);
            }) {
                failures.push(format!("Toggle ({}) => {}", toggle_shortcut_str, e));
            }
        }
    }

    if let Some(hold_shortcut_str) = &hold_shortcut_str {
        let hold_shortcut =
            <Shortcut as std::str::FromStr>::from_str(hold_shortcut_str).map_err(|e| {
                failures.push(format!(
                    "Hold ({}) => failed to parse shortcut: {:?}",
                    hold_shortcut_str, e
                ));
            });
        if let Ok(hold_shortcut) = hold_shortcut {
            if let Err(e) = shortcut_manager.on_shortcut(hold_shortcut, |app, shortcut, event| {
                handle_shortcut_event(app, shortcut, &event);
            }) {
                failures.push(format!("Hold ({}) => {}", hold_shortcut_str, e));
            }
        }
    }

    if let Some(paste_last_shortcut_str) = &paste_last_shortcut_str {
        let paste_last_shortcut =
            <Shortcut as std::str::FromStr>::from_str(paste_last_shortcut_str).map_err(|e| {
                failures.push(format!(
                    "PasteLast ({}) => failed to parse shortcut: {:?}",
                    paste_last_shortcut_str, e
                ));
            });
        if let Ok(paste_last_shortcut) = paste_last_shortcut {
            if let Err(e) =
                shortcut_manager.on_shortcut(paste_last_shortcut, |app, shortcut, event| {
                    handle_shortcut_event(app, shortcut, &event);
                })
            {
                failures.push(format!("PasteLast ({}) => {}", paste_last_shortcut_str, e));
            }
        }
    }

    if let Some(retry_shortcut_str) = &retry_shortcut_str {
        let retry_shortcut =
            <Shortcut as std::str::FromStr>::from_str(retry_shortcut_str).map_err(|e| {
                failures.push(format!(
                    "Retry ({}) => failed to parse shortcut: {:?}",
                    retry_shortcut_str, e
                ));
            });
        if let Ok(retry_shortcut) = retry_shortcut {
            if let Err(e) = shortcut_manager.on_shortcut(retry_shortcut, |app, shortcut, event| {
                handle_shortcut_event(app, shortcut, &event);
            }) {
                failures.push(format!("Retry ({}) => {}", retry_shortcut_str, e));
            }
        }
    }

    if let Some(quick_ask_hold_shortcut_str) = &quick_ask_hold_shortcut_str {
        let quick_ask_hold_shortcut =
            <Shortcut as std::str::FromStr>::from_str(quick_ask_hold_shortcut_str).map_err(|e| {
                failures.push(format!(
                    "QuickAskHold ({}) => failed to parse shortcut: {:?}",
                    quick_ask_hold_shortcut_str, e
                ));
            });
        if let Ok(quick_ask_hold_shortcut) = quick_ask_hold_shortcut {
            if let Err(e) =
                shortcut_manager.on_shortcut(quick_ask_hold_shortcut, |app, shortcut, event| {
                    handle_shortcut_event(app, shortcut, &event);
                })
            {
                failures.push(format!(
                    "QuickAskHold ({}) => {}",
                    quick_ask_hold_shortcut_str, e
                ));
            }
        }
    }

    if let Some(quick_ask_toggle_shortcut_str) = &quick_ask_toggle_shortcut_str {
        let quick_ask_toggle_shortcut =
            <Shortcut as std::str::FromStr>::from_str(quick_ask_toggle_shortcut_str).map_err(|e| {
                failures.push(format!(
                    "QuickAskToggle ({}) => failed to parse shortcut: {:?}",
                    quick_ask_toggle_shortcut_str, e
                ));
            });
        if let Ok(quick_ask_toggle_shortcut) = quick_ask_toggle_shortcut {
            if let Err(e) =
                shortcut_manager.on_shortcut(quick_ask_toggle_shortcut, |app, shortcut, event| {
                    handle_shortcut_event(app, shortcut, &event);
                })
            {
                failures.push(format!(
                    "QuickAskToggle ({}) => {}",
                    quick_ask_toggle_shortcut_str, e
                ));
            }
        }
    }

    if failures.is_empty() {
        log::info!("Shortcuts registered successfully");
    } else {
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

    // Never abort startup due to hotkey registration failures.
    Ok(())
}

#[cfg(desktop)]
pub(crate) fn build_global_shortcut_plugin() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    // Just initialize the plugin - shortcuts will be registered in setup() after store is available
    tauri_plugin_global_shortcut::Builder::new().build()
}
