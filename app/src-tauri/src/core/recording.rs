use crate::audio;
use crate::audio_mute::AudioMuteManager;
use crate::commands;
use crate::events;
use crate::get_setting_from_store;
use crate::pipeline;
use crate::recording_request_initialization::{
    start_request_log_with_seed, LogLlmSeedMode, RecordingRequestSeed,
};
use crate::request_log::RequestLogStore;
use crate::state::AppState;
use crate::OverlayAudioLevelPayload;
use crate::PipelineStateEvent;
use tauri::{AppHandle, Emitter, Manager};

use std::sync::atomic::Ordering;

// ============================================================================
// Playing audio handling during recording
// ============================================================================

#[cfg(desktop)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PlayingAudioHandling {
    None,
    Mute,
    Pause,
    MuteAndPause,
}

#[cfg(desktop)]
impl PlayingAudioHandling {
    fn from_str(s: &str) -> Self {
        match s {
            "none" => Self::None,
            "mute" => Self::Mute,
            "pause" => Self::Pause,
            "mute_and_pause" => Self::MuteAndPause,
            // Unknown values: fall back to the default.
            _ => Self::None,
        }
    }

    pub fn wants_mute(self) -> bool {
        matches!(self, Self::Mute | Self::MuteAndPause)
    }

    pub fn wants_pause(self) -> bool {
        matches!(self, Self::Pause | Self::MuteAndPause)
    }
}

#[cfg(desktop)]
pub(crate) fn get_playing_audio_handling(app: &AppHandle) -> PlayingAudioHandling {
    // Prefer the new enum setting.
    let raw: serde_json::Value =
        get_setting_from_store(app, "playing_audio_handling", serde_json::Value::Null);

    if let serde_json::Value::String(s) = raw {
        return PlayingAudioHandling::from_str(&s);
    }

    // Legacy fallback: auto_mute_audio boolean.
    // If the legacy key is missing entirely, default to None.
    let legacy_raw: serde_json::Value =
        get_setting_from_store(app, "auto_mute_audio", serde_json::Value::Null);

    match legacy_raw {
        serde_json::Value::Bool(true) => PlayingAudioHandling::Mute,
        serde_json::Value::Bool(false) => PlayingAudioHandling::None,
        _ => PlayingAudioHandling::None,
    }
}

/// Start recording with sound and audio mute handling
#[cfg(desktop)]
pub(crate) fn start_recording(
    app: &AppHandle,
    state: &AppState,
    sound_enabled: bool,
    audio_cue: audio::AudioCue,
    audio_mute_manager: &Option<tauri::State<'_, AudioMuteManager>>,
    playing_audio_handling: PlayingAudioHandling,
    source: &str,
) {
    // Log current pipeline state before attempting to start
    let current_state = app
        .try_state::<pipeline::SharedPipeline>()
        .map(|p| p.state());
    log::info!(
        "{}: starting recording (current pipeline state: {:?})",
        source,
        current_state
    );
    crate::emit_system_event(
        app,
        "shortcut",
        &format!("{}: starting recording", source),
        Some(&format!("Pipeline state: {:?}", current_state)),
    );

    // Start pipeline recording FIRST - if it fails, don't do anything else
    if let Some(pipeline) = app.try_state::<pipeline::SharedPipeline>() {
        // Pin the per-program profile *before* we show any overlay windows.
        // The overlay is always-on-top and can briefly become the foreground window on Windows,
        // which would otherwise cause per-program profile detection to degrade to Default.
        let config = pipeline.config();
        let foreground = crate::windows_apps::get_foreground_process_path();
        let matched_profile =
            crate::pipeline::select_profile_for_foreground_app(&config.llm_config);

        if let Some(p) = matched_profile.as_ref() {
            let _ = pipeline.set_session_profile_override(Some(p.id.clone()));
        } else {
            let _ = pipeline.set_session_profile_override(None);
        }

        log::info!(
            "[profile] start_recording source={} foreground={:?} profiles={} matched={}",
            source,
            foreground,
            config.llm_config.program_prompt_profiles.len(),
            matched_profile
                .as_ref()
                .map(|p| format!("{} ({})", p.name, p.id))
                .unwrap_or_else(|| "<none>".to_string())
        );

        let (profile_id, profile_name) = if foreground.is_none() {
            (None, None)
        } else if let Some(p) = matched_profile.as_ref() {
            (Some(p.id.clone()), Some(p.name.clone()))
        } else {
            (Some("default".to_string()), Some("Default".to_string()))
        };

        // Keep a per-session copy so stop_recording can use the same profile resolution
        // even if the overlay briefly becomes the foreground window.
        if let Ok(mut slot) = state.recording_session_profile_id.lock() {
            *slot = profile_id.clone();
        }

        // Seed a request-log entry *before* asking the audio layer to start.
        // That way "failed to start" attempts still show up on the Logs page instead of
        // disappearing into the Rust log stream with no user-visible breadcrumb.
        let request_seed = RecordingRequestSeed::from_config(&config)
            .with_profile(profile_id.clone(), profile_name.clone());
        let request_log_store = app.try_state::<RequestLogStore>();
        let request_id = request_log_store.as_ref().map(|state| {
            start_request_log_with_seed(
                state.inner(),
                &request_seed,
                LogLlmSeedMode::PreserveConfigured,
                |log| {
                    log.info(format!("Recording started ({})", source));
                },
            )
        });

        if let Err(e) = pipeline.start_recording() {
            log::error!(
                "{}: Failed to start pipeline recording: {} (state was: {:?})",
                source,
                e,
                current_state
            );
            let _ = pipeline.set_session_profile_override(None);

            // We took a best-effort snapshot at start time; clear it when start never actually
            // happened so the next recording can't inherit stale profile metadata.
            if let Ok(mut slot) = state.recording_session_profile_id.lock() {
                *slot = None;
            }

            if let Some(log_store) = app.try_state::<RequestLogStore>() {
                log_store.with_current(|log| {
                    log.error_with_details(
                        format!("Failed to start recording: {}", e),
                        crate::request_log::format_error_chain(&e),
                    );
                    log.complete_error(e.to_string());
                });
            }

            crate::sessions::recording_finalization::complete_current_request_without_cost(
                app,
                pipeline.inner(),
                request_id.as_deref(),
            );

            let error_msg = format!("{} (pipeline state: {:?})", e, current_state);
            crate::emit_system_event(
                app,
                "error",
                &format!("{}: Failed to start recording", source),
                Some(&error_msg),
            );

            // Keep the overlay payload concise. The detailed causal chain lives in request logs.
            crate::recording_completion::emit_pipeline_error(app, &e.to_string(), None);

            // Match the later-stage transcription failure behavior: even when the user normally
            // keeps overlays hidden, force-show the overlay so a startup failure isn't invisible.
            if let Err(show_err) = commands::overlay::show_overlay_with_reset_if_not_always(app) {
                log::warn!(
                    "Failed to force-show overlay after start error: {}",
                    show_err
                );
            }
            return;
        }

        // Begin an OCR session tied to this request id so OCR can survive internal
        // pipeline transitions (e.g. reset-to-idle) and still be consumable later.
        if let Some(request_id) = request_id {
            pipeline.begin_ocr_session(request_id);
        }

        // Attempt to start concurrent streaming STT in the background.
        // This is best-effort: if the provider doesn't support it, the
        // pipeline will fall back to batch transcription at stop time.
        // NOTE: We use `tauri::async_runtime::spawn` instead of `tokio::spawn`
        // because this function is called from the global-shortcut handler which
        // runs on the main OS thread — outside the Tokio runtime context.
        let app_handle = app.clone();
        tauri::async_runtime::spawn(async move {
            if let Some(pipeline) = app_handle.try_state::<pipeline::SharedPipeline>() {
                pipeline.try_start_concurrent_streaming(&app_handle).await;
            }
        });
    }

    // While recording/transcribing, allow Escape to cancel without triggering transcription.
    crate::set_escape_cancel_shortcut_enabled(app, true);

    // Pipeline started successfully - now update state and do side effects
    state.is_recording.store(true, Ordering::SeqCst);
    let _ = app.emit(
        events::EVENT_PIPELINE_STATE_CHANGED,
        PipelineStateEvent::Recording,
    );

    // Start the recording chime ASAP.
    // Showing/snapping the overlay window can be a bit slow on some systems (monitor queries,
    // position math, window show), so we kick off audio playback *before* that work.
    //
    // If we're about to mute system audio, defer the mute until the cue has finished playing,
    // but do so off-thread so the overlay can appear immediately.
    // If playing-audio handling includes mute, we intentionally suppress the cue entirely.
    // (User expectation: mute modes are "quiet" and should not play chimes.)
    let should_play_start_cue = sound_enabled && !playing_audio_handling.wants_mute();
    if should_play_start_cue {
        // No immediate mute: play asynchronously to keep the UI responsive.
        audio::play_sound(audio::SoundType::RecordingStart, audio_cue);
    }

    // Show overlay if in "recording_only" mode
    let overlay_mode: String =
        get_setting_from_store(app, "overlay_mode", "recording_only".to_string());
    if overlay_mode == "recording_only" {
        if let Err(e) = commands::overlay::show_overlay_with_reset_if_not_always(app) {
            log::warn!("Failed to show overlay on recording start: {}", e);
        }
    }

    // Prime the overlay waveform immediately. The background publisher loop will follow up
    // with real levels as soon as the first CPAL callback arrives.
    //
    // This also helps when the overlay is shown at start: the listener may not yet be
    // registered when the very first publisher tick runs.
    {
        let payload = OverlayAudioLevelPayload {
            seq: 0,
            rms: 0.0,
            peak: 0.0,
            wave_seq: Some(0),
            mins: Some(Vec::<f32>::new()),
            maxes: Some(Vec::<f32>::new()),
        };
        if let Some(overlay) = app.get_webview_window("overlay") {
            let _ = overlay.emit(events::EVENT_OVERLAY_AUDIO_LEVEL, payload);
        } else {
            let _ = app.emit(events::EVENT_OVERLAY_AUDIO_LEVEL, payload);
        }
    }

    // Notify frontend ASAP so the overlay can update/animate without waiting for
    // audio side-effects (which may block, e.g. when we ensure the cue finishes
    // before muting system audio).
    let _ = app.emit(events::EVENT_RECORDING_START, ());

    // Mute system audio if enabled.
    // If we played a cue, muting used to be deferred until after the cue finishes.
    // We no longer play cues in mute modes, so we can always mute immediately.
    if playing_audio_handling.wants_mute() {
        if let Some(manager) = audio_mute_manager {
            if let Err(e) = manager.mute() {
                log::warn!("Failed to mute audio: {}", e);
            }
        }
    }

    // Pause playing audio (best-effort).
    if playing_audio_handling.wants_pause() {
        match crate::adapters::media_controls::is_non_system_audio_session_active() {
            Ok(true) => match crate::adapters::media_controls::toggle_media_play_pause(app) {
                Ok(()) => {
                    state.play_pause_toggled.store(true, Ordering::SeqCst);
                }
                Err(e) => {
                    log::warn!("Failed to toggle media play/pause: {}", e);
                    state.play_pause_toggled.store(false, Ordering::SeqCst);
                }
            },
            Ok(false) => {
                // Nothing appears to be playing: don't send play/pause,
                // otherwise we might accidentally start playback.
                state.play_pause_toggled.store(false, Ordering::SeqCst);
            }
            Err(e) => {
                // Detection failed: be conservative and avoid toggling.
                log::warn!(
                    "Failed to detect active audio session; skipping pause: {}",
                    e
                );
                state.play_pause_toggled.store(false, Ordering::SeqCst);
            }
        }
    } else {
        state.play_pause_toggled.store(false, Ordering::SeqCst);
    }
}
