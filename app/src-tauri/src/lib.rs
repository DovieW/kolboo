use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use schemars::JsonSchema;
use tauri::{AppHandle, Emitter, Manager, WindowEvent};
use tracing::Instrument;

// Extraction buckets: settings defaults, overlay wiring, shortcuts lifecycle, bootstrap wiring.

// Re-export core/adapter types that lib.rs and commands need.
#[cfg(desktop)]
pub(crate) use crate::adapters::media_controls::toggle_media_play_pause;
#[cfg(desktop)]
pub(crate) use crate::core::recording::{
    get_playing_audio_handling, start_recording, PlayingAudioHandling,
};

mod app_paths;
mod audio;
mod audio_capture;
mod audio_mute;
mod bootstrap;
mod clipboard_context;
mod commands;
mod cost;
mod embeddings;
pub mod events;
mod fs;
mod history;
mod llm;
mod network;
mod overlay;
mod pipeline;
mod recordings;
mod request_log;
mod router_embeddings_cache;
pub mod schema_export;
mod secrets;
mod sessions;
#[path = "settings.rs"]
mod settings;
mod shortcuts;
mod shortcuts_lock;
mod state;
mod stats;
mod stt;
mod text;
mod tracing_init;
mod vad;
mod windows_apps;

mod adapters;
mod core;

#[derive(Debug, Clone, serde::Serialize, JsonSchema)]
pub struct SystemEvent {
    pub timestamp: String,
    pub event_type: String,
    pub message: String,
    pub details: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, JsonSchema)]
pub struct PipelineErrorPayload {
    pub message: String,
    pub request_id: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, JsonSchema)]
pub struct OverlayAudioLevelPayload {
    pub seq: u64,
    pub rms: f32,
    pub peak: f32,
    pub wave_seq: Option<u64>,
    pub mins: Option<Vec<f32>>,
    pub maxes: Option<Vec<f32>>,
}

#[derive(Debug, Clone, serde::Serialize, JsonSchema)]
pub struct QuickAskStartedPayload {
    pub question: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, JsonSchema)]
pub struct QuickAskAnswerOkPayload {
    pub ok: bool,
    pub answer: String,
    pub provider_used: Option<String>,
    pub model_used: Option<String>,
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Clone, serde::Serialize, JsonSchema)]
pub struct QuickAskAnswerErrorPayload {
    pub ok: bool,
    pub error: String,
}

#[derive(Debug, Clone, serde::Serialize, JsonSchema)]
#[serde(untagged)]
pub enum QuickAskAnswerPayload {
    Ok(QuickAskAnswerOkPayload),
    Err(QuickAskAnswerErrorPayload),
}

#[derive(Debug, Clone, Copy, serde::Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PipelineStateEvent {
    Idle,
    Recording,
    Transcribing,
    Routing,
    Rewriting,
    Error,
}

pub type PipelineTranscriptReadyPayload = String;
pub type EmptyEventPayload = ();
pub type SettingsChangedPayload = std::collections::BTreeMap<String, serde_json::Value>;

#[derive(Debug, Clone, Copy, serde::Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionStateEvent {
    Disconnected,
    Connecting,
    Idle,
    Recording,
    Processing,
}

#[derive(Debug, Clone, serde::Serialize, JsonSchema)]
pub struct ConnectionStateChangedPayload {
    pub state: ConnectionStateEvent,
}

pub use audio_capture::AudioCaptureDiagnostics;
pub use audio_capture::AudioLevelStats;
pub use commands::audio::MicTestAudioLevelPayload;
pub use commands::config::AvailableProvidersResponse;
pub use commands::config::DefaultSectionsResponse;
pub use commands::config::ProviderInfo;
pub use commands::data::DataStorageSummary;
pub use commands::fireworks::ModelOption;
pub use commands::history::HistoryDeleteMode;
pub use commands::history::HistoryDeleteOptions;
pub use commands::history::HistoryDeleteResult;
pub use commands::llm::IterateRewritePromptResponse;
pub use commands::llm::LlmCompleteResponse;
pub use commands::llm::LlmProviderInfo;
pub use commands::llm::TestLlmRewriteResponse;
pub use commands::llm::TestRewriteWithPromptResponse;
pub use commands::network::SystemProxyInfo;
pub use commands::network::WindowsInternetProxySettings;
pub use commands::pricing::LlmModelPricing;
pub use commands::pricing::ModelPricingResponse;
pub use commands::pricing::SttModelPricing;
pub use commands::recording::AudioSettingsTestWavs;
pub use commands::router::CacheRouterEmbeddingsResponse;
pub use commands::stats::CostByProviderResponse;
pub use commands::stats::CostSummaryResponse;
pub use commands::stats::ProviderCostTotal;
pub use commands::whisper::LocalWhisperBackendStatusResponse as LocalWhisperBackendStatus;
pub use commands::whisper::LocalWhisperComputeBackend;
pub use commands::whisper::LocalWhisperModelLoadEvent;
pub use commands::whisper::LocalWhisperModelLoadStatus;
pub use commands::whisper::WhisperModelDownloadProgress;
pub use commands::whisper::WhisperModelDownloadStatus;
pub use commands::whisper::WhisperModelInfo;
pub use history::HistoryPageQuery;
pub use history::HistoryPageResult;
pub use recordings::RecordingsStats;
pub use request_log::RequestLog;
pub use settings::HotkeyConfig;
pub use settings::IntentRouterSettings;
pub use settings::ProxySettings;
pub use settings::RewritePreset;
pub use settings::RewriteProgramPromptProfile;
pub use windows_apps::OpenWindowInfo;

#[cfg(target_os = "windows")]
mod windows_modifier_hotkeys;

#[cfg(test)]
mod tests;

use audio_mute::AudioMuteManager;
use history::{HistoryStorage, RequestModelInfo};
use recordings::RecordingStore;
use request_log::{RequestKind, RequestLogStore};
use state::{AppState, MicTestMeterState, QuickAskConversationMemory, TrayKeepAlive};

#[cfg(desktop)]
pub(crate) use shortcuts::{
    cancel_pipeline_session, handle_shortcut_event, set_escape_cancel_shortcut_enabled,
};

#[cfg(desktop)]
use tauri_plugin_store::StoreExt;

// Define NSPanel type for overlay on macOS
#[cfg(target_os = "macos")]
tauri_nspanel::tauri_panel! {
    panel!(OverlayPanel {
        config: {
            can_become_key_window: false,
            is_floating_panel: true
        }
    })
}

/// Helper to read a setting from the store with a default fallback
#[cfg(desktop)]
pub(crate) fn get_setting_from_store<T: serde::de::DeserializeOwned>(
    app: &AppHandle,
    key: &str,
    default: T,
) -> T {
    app.store("settings.json")
        .ok()
        .and_then(|store| store.get(key))
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or(default)
}
/// Emit a system event to the frontend for debugging
#[cfg(desktop)]
pub(crate) fn emit_system_event(
    app: &AppHandle,
    event_type: &str,
    message: &str,
    details: Option<&str>,
) {
    let event = SystemEvent {
        timestamp: chrono::Utc::now().to_rfc3339(),
        event_type: event_type.to_string(),
        message: message.to_string(),
        details: details.map(|s| s.to_string()),
    };

    let _ = app.emit(events::EVENT_SYSTEM_EVENT, event);
}

/// Normalize transcript text for output.
///
/// We intentionally keep this conservative: the pipeline now performs a
/// quiet-audio gate before STT to avoid "silent audio" hallucinations.
pub(crate) fn sanitize_transcript(transcript: &str) -> Option<String> {
    let trimmed = transcript.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Stop recording with sound and audio unmute handling
#[cfg(desktop)]
pub(crate) fn stop_recording(
    app: &AppHandle,
    state: &AppState,
    sound_enabled: bool,
    audio_cue: audio::AudioCue,
    audio_mute_manager: &Option<tauri::State<'_, AudioMuteManager>>,
    playing_audio_handling: PlayingAudioHandling,
    source: &str,
) {
    state.is_recording.store(false, Ordering::SeqCst);
    log::info!("{}: stopping recording", source);
    emit_system_event(
        app,
        "shortcut",
        &format!("{}: stopping recording", source),
        None,
    );

    // If this recording was started via the Quick Ask hotkey, branch the post-transcription
    // flow into an LLM answer overlay (instead of output/paste).
    let is_quick_ask_session = state.quick_ask_session_active.swap(false, Ordering::SeqCst);

    // If hallucination protection (quiet-audio gate) is enabled and the recording is considered
    // effectively quiet, the pipeline will skip STT and immediately return to Idle.
    // In that case, playing the stop sound is misleading, so we only play it if we actually
    // enter Transcribing/Rewriting.
    let quiet_audio_gate_enabled: bool =
        get_setting_from_store(app, "quiet_audio_gate_enabled", true);
    let play_stop_sound_when_transcribing =
        sound_enabled && quiet_audio_gate_enabled && !playing_audio_handling.wants_mute();

    // Keep Escape-to-cancel enabled during the transcription phase too.
    set_escape_cancel_shortcut_enabled(app, true);
    // Unmute system audio if it was muted
    if playing_audio_handling.wants_mute() {
        if let Some(manager) = audio_mute_manager {
            if let Err(e) = manager.unmute() {
                log::warn!("Failed to unmute audio: {}", e);
            }
        }
    }
    // If the quiet-audio gate is disabled, play the stop sound immediately as before.
    if sound_enabled && !quiet_audio_gate_enabled && !playing_audio_handling.wants_mute() {
        audio::play_sound(audio::SoundType::RecordingStop, audio_cue);
    }

    // Resume playing audio if we previously toggled it.
    if playing_audio_handling.wants_pause()
        && state.play_pause_toggled.swap(false, Ordering::SeqCst)
    {
        if let Err(e) = toggle_media_play_pause(app) {
            log::warn!("Failed to restore media play/pause: {}", e);
        }
    }

    // Get overlay mode for hiding after transcription
    let overlay_mode: String =
        get_setting_from_store(app, "overlay_mode", "recording_only".to_string());

    // Get output mode for how to output text
    let output_mode_str: String = get_setting_from_store(app, "output_mode", "paste".to_string());
    let output_mode = commands::text::OutputMode::from_str(&output_mode_str);

    // Optional: after pasting, press Enter.
    let output_hit_enter: bool = get_setting_from_store(app, "output_hit_enter", false);

    // Resolve the program profile id captured at recording start (before overlays can steal focus).
    // We "take" it (read and clear) so it can't leak across sessions.
    let session_profile_id: Option<String> = state
        .recording_session_profile_id
        .lock()
        .ok()
        .and_then(|mut g| g.take());

    // Stop pipeline and trigger transcription in background
    if let Some(pipeline) = app.try_state::<pipeline::SharedPipeline>() {
        let pipeline_clone = (*pipeline).clone();
        let app_clone = app.clone();
        let overlay_mode_clone = overlay_mode.clone();

        // Capture model info from pipeline config for persistence in history.
        let config = pipeline.config();
        let profile: Option<crate::llm::ProgramPromptProfile> = session_profile_id
            .as_deref()
            .and_then(|id| {
                config
                    .llm_config
                    .program_prompt_profiles
                    .iter()
                    .find(|p| p.id == id)
                    .cloned()
            })
            .or_else(|| pipeline::select_profile_for_foreground_app(&config.llm_config));

        let model_info = RequestModelInfo {
            stt_provider: Some(config.stt_provider.clone()),
            stt_model: config.stt_model.clone(),
            llm_provider: if config.llm_config.enabled {
                Some(config.llm_config.provider.clone())
            } else {
                None
            },
            llm_model: config.llm_config.model.clone(),
            profile_id: profile.as_ref().map(|p| p.id.clone()),
            profile_name: profile.as_ref().map(|p| p.name.clone()),
            preset_id: None,
            preset_name: None,
        };

        #[derive(Clone, Debug, Default)]
        struct QuickAskProfileConfig {
            provider: Option<String>,
            model: Option<String>,
            system_prompt: Option<String>,
            openai_reasoning_effort: Option<String>,
            gemini_thinking_budget: Option<i64>,
            gemini_thinking_level: Option<String>,
            anthropic_thinking_budget: Option<i64>,
            include_clipboard_context: bool,
        }

        let default_profile = config
            .llm_config
            .program_prompt_profiles
            .iter()
            .find(|p| p.id == "default");

        let quick_ask_profile_cfg: QuickAskProfileConfig = {
            let include_clipboard_context = profile
                .as_ref()
                .and_then(|p| p.quick_ask_include_clipboard_context)
                .or_else(|| default_profile.and_then(|p| p.quick_ask_include_clipboard_context))
                .unwrap_or(false);

            profile
                .as_ref()
                .map(|p| QuickAskProfileConfig {
                    provider: p.quick_ask_provider.clone(),
                    model: p.quick_ask_model.clone(),
                    system_prompt: p.quick_ask_system_prompt.clone(),
                    openai_reasoning_effort: p.quick_ask_openai_reasoning_effort.clone(),
                    gemini_thinking_budget: p.quick_ask_gemini_thinking_budget,
                    gemini_thinking_level: p.quick_ask_gemini_thinking_level.clone(),
                    anthropic_thinking_budget: p.quick_ask_anthropic_thinking_budget,
                    include_clipboard_context,
                })
                .unwrap_or(QuickAskProfileConfig {
                    include_clipboard_context,
                    ..Default::default()
                })
        };

        const DEFAULT_QUICK_REPLACE_SYSTEM_PROMPT: &str =
            "You are an expert editor. Apply the user's instructions to the provided text.\n\nRules:\n- Return ONLY the updated text (no commentary, no code fences).\n- Preserve the original language and formatting unless instructed otherwise.";

        #[derive(Clone, Debug, Default)]
        struct QuickReplaceProfileConfig {
            enabled: bool,
            provider: Option<String>,
            model: Option<String>,
            system_prompt: String,
            include_clipboard_context: bool,
        }

        // Context grabbing method (highlighted selection capture).
        // This is a per-profile setting; when unset, we default to Ctrl+C.
        let context_grab_method: crate::commands::text::ContextGrabMethod = {
            let method_str = profile
                .as_ref()
                .and_then(|p| p.context_grab_method.clone())
                .or_else(|| default_profile.and_then(|p| p.context_grab_method.clone()));

            match method_str.as_deref() {
                Some("none") => crate::commands::text::ContextGrabMethod::None,
                Some("ctrl_shift_c") => crate::commands::text::ContextGrabMethod::CtrlShiftC,
                Some("ctrl_insert") => crate::commands::text::ContextGrabMethod::CtrlInsert,
                _ => crate::commands::text::ContextGrabMethod::CtrlC,
            }
        };

        let quick_replace_cfg: QuickReplaceProfileConfig = {
            let enabled_opt = profile
                .as_ref()
                .and_then(|p| p.quick_replace_enabled)
                .or_else(|| default_profile.and_then(|p| p.quick_replace_enabled));

            // Backward-compatible fallback to the legacy global key (pre per-profile settings).
            let enabled_legacy: bool = get_setting_from_store(app, "quick_replace_enabled", false);

            let enabled = !is_quick_ask_session && enabled_opt.unwrap_or(enabled_legacy);

            let provider = profile
                .as_ref()
                .and_then(|p| p.quick_replace_provider.clone())
                .or_else(|| default_profile.and_then(|p| p.quick_replace_provider.clone()))
                .or_else(|| profile.as_ref().and_then(|p| p.llm_provider.clone()))
                .or_else(|| default_profile.and_then(|p| p.llm_provider.clone()))
                .or(Some(config.llm_config.provider.clone()));

            let provider_for_default_model = provider.as_deref().unwrap_or("openai");

            let model = profile
                .as_ref()
                .and_then(|p| p.quick_replace_model.clone())
                .or_else(|| default_profile.and_then(|p| p.quick_replace_model.clone()))
                .or_else(|| profile.as_ref().and_then(|p| p.llm_model.clone()))
                .or_else(|| default_profile.and_then(|p| p.llm_model.clone()))
                .or_else(|| config.llm_config.model.clone())
                .or_else(|| {
                    llm::default_llm_model_for_provider(provider_for_default_model)
                        .map(|m| m.to_string())
                });

            let system_prompt = profile
                .as_ref()
                .and_then(|p| p.quick_replace_system_prompt.clone())
                .or_else(|| default_profile.and_then(|p| p.quick_replace_system_prompt.clone()))
                .unwrap_or_else(|| DEFAULT_QUICK_REPLACE_SYSTEM_PROMPT.to_string());

            let include_clipboard_context = profile
                .as_ref()
                .and_then(|p| p.quick_replace_include_clipboard_context)
                .or_else(|| default_profile.and_then(|p| p.quick_replace_include_clipboard_context))
                .unwrap_or(false);

            QuickReplaceProfileConfig {
                enabled,
                provider,
                model,
                system_prompt,
                include_clipboard_context,
            }
        };

        // Quick Replace: probe for currently highlighted text while transcription runs.
        let quick_replace_epoch: u64 = if quick_replace_cfg.enabled {
            sessions::selection_probe::spawn_probe(
                app,
                sessions::selection_probe::ProbeKind::QuickReplace,
                context_grab_method,
            )
        } else {
            0
        };

        // Quick Ask: probe for currently highlighted text to use as additional context.
        let quick_ask_include_selected_text: bool =
            get_setting_from_store(app, "quick_ask_include_selected_text", false);

        let quick_ask_epoch: u64 = if is_quick_ask_session && quick_ask_include_selected_text {
            sessions::selection_probe::spawn_probe(
                app,
                sessions::selection_probe::ProbeKind::QuickAsk,
                context_grab_method,
            )
        } else {
            0
        };

        // Capture current request id (for history + retry audio).
        //
        // In some edge cases, request logging may not have been started at
        // recording-start (e.g., hotkey pressed during startup or other state
        // desync). If so, create a request log now so failures still show up in
        // Request Logs + History.
        let mut request_id: Option<String> = app
            .try_state::<RequestLogStore>()
            .and_then(|store| store.with_current(|log| log.id.clone()));

        if request_id.is_none() {
            if let Some(log_store) = app.try_state::<RequestLogStore>() {
                let id =
                    log_store.start_request(config.stt_provider.clone(), config.stt_model.clone());
                log_store.with_current(|log| {
                    log.profile_id = model_info.profile_id.clone();
                    log.profile_name = model_info.profile_name.clone();
                    log.llm_provider = model_info.llm_provider.clone();
                    // Avoid confusing logs: if LLM rewrite is disabled, do not record an LLM model.
                    log.llm_model = if config.llm_config.enabled {
                        model_info.llm_model.clone()
                    } else {
                        None
                    };
                    log.warn(format!(
                        "Request log was missing at stop; started a new request log entry ({})",
                        source
                    ));
                });
                request_id = Some(id);
            }
        }

        tauri::async_runtime::spawn(async move {
            // Emit transcription started only once the pipeline actually transitions
            // into Transcribing (quiet-audio gate skips should fade out without ever
            // showing "TRANSCRIBING...").
            {
                let app_for_evt = app_clone.clone();
                let pipeline_for_evt = pipeline_clone.clone();
                let audio_cue_for_stop = audio_cue;
                let should_play_stop_sound = play_stop_sound_when_transcribing;
                tauri::async_runtime::spawn(
                    async move {
                        let start = std::time::Instant::now();
                        loop {
                            match pipeline_for_evt.state() {
                                pipeline::PipelineState::Transcribing
                                | pipeline::PipelineState::Rewriting => {
                                    let _ = app_for_evt
                                        .emit(events::EVENT_PIPELINE_TRANSCRIPTION_STARTED, ());
                                    let _ = app_for_evt.emit(
                                        events::EVENT_PIPELINE_STATE_CHANGED,
                                        PipelineStateEvent::Transcribing,
                                    );

                                    if should_play_stop_sound {
                                        crate::audio::play_sound(
                                            crate::audio::SoundType::RecordingStop,
                                            audio_cue_for_stop,
                                        );
                                    }
                                    break;
                                }
                                pipeline::PipelineState::Idle | pipeline::PipelineState::Error => {
                                    // Idle can happen immediately due to quiet-audio skip.
                                    break;
                                }
                                pipeline::PipelineState::Recording
                                | pipeline::PipelineState::Routing => {}
                            }

                            if start.elapsed() > std::time::Duration::from_secs(2) {
                                break;
                            }

                            tokio::time::sleep(std::time::Duration::from_millis(15)).await;
                        }
                    }
                    .in_current_span(),
                );
            }

            // Emit routing started once the pipeline transitions into the Routing phase.
            {
                let app_for_evt = app_clone.clone();
                let pipeline_for_evt = pipeline_clone.clone();
                tauri::async_runtime::spawn(
                    async move {
                        let start = std::time::Instant::now();
                        loop {
                            match pipeline_for_evt.state() {
                                pipeline::PipelineState::Routing => {
                                    let _ = app_for_evt
                                        .emit(events::EVENT_PIPELINE_ROUTING_STARTED, ());
                                    let _ = app_for_evt.emit(
                                        events::EVENT_PIPELINE_STATE_CHANGED,
                                        PipelineStateEvent::Routing,
                                    );
                                    break;
                                }
                                pipeline::PipelineState::Idle | pipeline::PipelineState::Error => {
                                    break;
                                }
                                pipeline::PipelineState::Recording
                                | pipeline::PipelineState::Transcribing
                                | pipeline::PipelineState::Rewriting => {}
                            }

                            if start.elapsed() > std::time::Duration::from_secs(15 * 60) {
                                break;
                            }

                            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
                        }
                    }
                    .in_current_span(),
                );
            }

            // Emit rewriting started once the pipeline actually enters the optional LLM phase.
            //
            // This keeps the overlay UI accurate even if state polling is delayed.
            {
                let app_for_evt = app_clone.clone();
                let pipeline_for_evt = pipeline_clone.clone();
                tauri::async_runtime::spawn(
                    async move {
                        let start = std::time::Instant::now();
                        loop {
                            match pipeline_for_evt.state() {
                                pipeline::PipelineState::Rewriting => {
                                    let _ = app_for_evt
                                        .emit(events::EVENT_PIPELINE_REWRITING_STARTED, ());
                                    let _ = app_for_evt.emit(
                                        events::EVENT_PIPELINE_STATE_CHANGED,
                                        PipelineStateEvent::Rewriting,
                                    );
                                    break;
                                }
                                pipeline::PipelineState::Idle | pipeline::PipelineState::Error => {
                                    // No rewrite (disabled/failed early) or pipeline exited.
                                    break;
                                }
                                pipeline::PipelineState::Recording
                                | pipeline::PipelineState::Transcribing
                                | pipeline::PipelineState::Routing => {}
                            }

                            if start.elapsed() > std::time::Duration::from_secs(15 * 60) {
                                break;
                            }

                            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                        }
                    }
                    .in_current_span(),
                );
            }

            // Create an in-progress history entry while we transcribe.
            // Quick Ask uses a separate UI surface and should not pollute the main dictation history.
            if !is_quick_ask_session {
                if let Some(req_id) = request_id.as_ref() {
                    if let Some(history) = app_clone.try_state::<HistoryStorage>() {
                        let _ = history.add_request_entry(
                            req_id.clone(),
                            model_info,
                            commands::history::get_history_max_entries(&app_clone),
                        );
                        let _ = app_clone.emit(events::EVENT_HISTORY_CHANGED, ());
                    }
                }
            }

            // Log transcription start for this request
            if let Some(log_store) = app_clone.try_state::<RequestLogStore>() {
                log_store.with_current(|log| {
                    // Do not include recording time in the request "Total" duration.
                    log.mark_processing_started();
                    log.info("Recording stopped, starting transcription");
                });
            }

            match pipeline_clone.stop_and_transcribe_detailed().await {
                Ok(result) => {
                    log::info!("Transcription complete: {} chars", result.final_text.len());

                    // Final output after pipeline (STT + optional LLM) normalization.
                    // Quiet recordings should already have been skipped in the pipeline.
                    let filtered_transcript = sanitize_transcript(&result.final_text);

                    // Update request log store
                    if let Some(log_store) = app_clone.try_state::<RequestLogStore>() {
                        // Like Quick Ask, Quick Replace needs to keep the request log open
                        // until the extra LLM step completes, otherwise the UI won't capture
                        // the additional diagnostics.
                        let should_complete_now = !(is_quick_ask_session
                            || (quick_replace_cfg.enabled && quick_replace_epoch != 0));

                        log_store.with_current(|log| {
                            // Raw STT output (pre-LLM)
                            log.raw_transcript = Some(result.stt_text.clone());

                            // Final output after pipeline + hallucination filtering (if any)
                            if let Some(ref text) = filtered_transcript {
                                log.formatted_transcript = Some(text.clone());
                            }

                            log.stt_duration_ms = Some(result.stt_duration_ms);
                            log.llm_duration_ms = result.llm_duration_ms;

                            // Persist a structured outcome so the UI can surface
                            // why rewrite didn't run (common confusion when STT succeeds).
                            log.llm_outcome = Some(result.llm_outcome.code().to_string());
                            log.llm_not_attempted_reason = None;
                            log.llm_error_message = None;

                            // Use the provider instance's model (includes provider defaults) so
                            // the UI can show the real model used. If we didn't attempt LLM
                            // formatting, clear any pre-populated provider/model values.
                            if result.llm_attempted() {
                                log.llm_provider = result.llm_provider_used.clone();
                                log.llm_model = result.llm_model_used.clone();
                            } else {
                                log.llm_provider = None;
                                log.llm_model = None;
                            }

                            log.info(format!(
                                "STT completed in {}ms ({} chars)",
                                result.stt_duration_ms,
                                result.stt_text.len()
                            ));

                            match &result.llm_outcome {
                                pipeline::LlmOutcome::NotAttempted(reason) => {
                                    log.llm_not_attempted_reason =
                                        Some(reason.code().to_string());
                                    if let pipeline::LlmNotAttemptedReason::ProviderUnavailable { .. } = reason {
                                        log.llm_error_message = Some(reason.to_log_details());
                                    }
                                    log.info_with_details(
                                        "LLM formatting not attempted",
                                        reason.to_log_details(),
                                    );
                                }
                                pipeline::LlmOutcome::Succeeded => {
                                    if let Some(ms) = result.llm_duration_ms {
                                        log.info(format!(
                                            "LLM formatting succeeded in {}ms ({} -> {} chars)",
                                            ms,
                                            result.stt_text.len(),
                                            result.final_text.len()
                                        ));
                                    } else {
                                        log.info("LLM formatting succeeded");
                                    }
                                }
                                pipeline::LlmOutcome::TimedOut => {
                                    if let Some(ms) = result.llm_duration_ms {
                                        log.warn(format!(
                                            "LLM formatting timed out after {}ms; fell back to STT transcript",
                                            ms
                                        ));
                                    } else {
                                        log.warn("LLM formatting timed out; fell back to STT transcript");
                                    }
                                }
                                pipeline::LlmOutcome::Failed(err) => {
                                    log.llm_error_message = Some(err.clone());
                                    log.warn(format!(
                                        "LLM formatting failed; fell back to STT transcript ({})",
                                        err
                                    ));
                                }
                            }

                            if filtered_transcript.is_none() {
                                log.warn("No transcript output (empty/whitespace)");
                            }

                            if should_complete_now {
                                log.complete_success();
                            } else {
                                log.info("Transcription completed; Quick Ask answer pending");
                            }
                        });

                        // Persist preset metadata into History (best-effort).
                        // The pipeline decides preset selection during routing and stores it
                        // into the current RequestLog; we mirror that into History so the
                        // History badge matches Request Logs.
                        if let Some(req_id) = request_id.as_deref() {
                            let preset_meta = log_store.with_current(|log| {
                                (log.preset_id.clone(), log.preset_name.clone())
                            });
                            if let Some((preset_id, preset_name)) = preset_meta {
                                if let Some(history) = app_clone.try_state::<HistoryStorage>() {
                                    let _ =
                                        history.set_request_preset(req_id, preset_id, preset_name);
                                    let _ = app_clone.emit(events::EVENT_HISTORY_CHANGED, ());
                                }
                            }
                        }

                        // Persist cost/usage stats (best-effort).
                        // For Quick Ask sessions we emit stats after the answer step so we can
                        // include the answer LLM details.
                        if should_complete_now {
                            if let Some(wav) = pipeline_clone.clone_last_wav_bytes() {
                                stats::emit_cost_events_for_current_request(
                                    &app_clone,
                                    stats::EventStatus::Success,
                                    Some(&wav),
                                );
                            }
                            log_store.complete_current();
                        }
                    }

                    // Persist audio for retry (best-effort)
                    if let (Some(req_id), Some(store)) = (
                        request_id.as_deref(),
                        app_clone.try_state::<RecordingStore>(),
                    ) {
                        if let Some(wav) = pipeline_clone.clone_last_wav_bytes() {
                            if store.save_wav(req_id, &wav).is_ok() {
                                let max_saved_recordings: usize = (get_setting_from_store(
                                    &app_clone,
                                    "max_saved_recordings",
                                    1000u64,
                                ))
                                .clamp(1, 100_000)
                                    as usize;

                                let _ = store.prune_to_max_files(max_saved_recordings);
                            }
                        }
                    }

                    if let Some(ref text) = filtered_transcript {
                        let _ = app_clone.emit(events::EVENT_PIPELINE_TRANSCRIPT_READY, text);
                        let _ = app_clone.emit(
                            events::EVENT_PIPELINE_STATE_CHANGED,
                            PipelineStateEvent::Idle,
                        );

                        // Default output is the (possibly rewritten) pipeline transcript.
                        // Quick Replace may overwrite this when a selection is present.
                        let mut output_value = text.clone();

                        // If Quick Replace was intended (selection present) but the rewrite failed,
                        // capture the error here so we can:
                        // - mark the request log + history as error
                        // - show the overlay error state with retry
                        // - avoid pasting the plain transcript into the selection
                        let mut quick_replace_failure: Option<String> = None;

                        // Quick Ask: instead of outputting/pasting the transcript, send it to an LLM
                        // for an answer and show it in a dedicated overlay.
                        if is_quick_ask_session {
                            let question = sanitize_transcript(&result.stt_text)
                                .unwrap_or_else(|| text.clone())
                                .trim()
                                .to_string();

                            // Ensure the answer window is visible before we start the LLM call.
                            crate::sessions::quick_ask::ensure_quick_ask_window_visible(&app_clone);

                            if question.is_empty() {
                                // Quick Ask is considered the "request" here, so mark the request log
                                // accordingly and finalize it.
                                if let Some(log_store) = app_clone.try_state::<RequestLogStore>() {
                                    log_store.with_current(|log| {
                                        log.kind = RequestKind::QuickAsk;
                                        log.quick_ask_question = Some(String::new());
                                        log.quick_ask_answer = None;
                                        log.error(
                                            "Quick Ask failed: no transcript to answer (empty)",
                                        );
                                        log.complete_error("No transcript to answer (empty)");
                                    });

                                    if let Some(wav) = pipeline_clone.clone_last_wav_bytes() {
                                        stats::emit_cost_events_for_current_request(
                                            &app_clone,
                                            stats::EventStatus::Error,
                                            Some(&wav),
                                        );
                                    }

                                    log_store.complete_current();
                                }

                                crate::sessions::quick_ask::emit_to_quick_ask(
                                    &app_clone,
                                    crate::sessions::quick_ask::EVENT_QUICK_ASK_ANSWER,
                                    QuickAskAnswerPayload::Err(QuickAskAnswerErrorPayload {
                                        ok: false,
                                        error: "No transcript to answer (empty)".to_string(),
                                    }),
                                );
                            } else {
                                // Resolve effective Quick Ask configuration:
                                // per-profile override -> global Quick Ask defaults -> global rewrite provider -> fallback.
                                let global_quick_ask_provider: Option<String> =
                                    get_setting_from_store(
                                        &app_clone,
                                        "quick_ask_provider",
                                        Option::<String>::None,
                                    );
                                let global_quick_ask_model: Option<String> = get_setting_from_store(
                                    &app_clone,
                                    "quick_ask_model",
                                    Option::<String>::None,
                                );
                                let global_quick_ask_system_prompt: Option<String> =
                                    get_setting_from_store(
                                        &app_clone,
                                        "quick_ask_system_prompt",
                                        Option::<String>::None,
                                    );

                                // Quick Ask conversation history (in-memory only).
                                let quick_ask_conversation_history_enabled: bool =
                                    get_setting_from_store(
                                        &app_clone,
                                        "quick_ask_conversation_history_enabled",
                                        true,
                                    );
                                let quick_ask_conversation_history_count_raw: u64 =
                                    get_setting_from_store(
                                        &app_clone,
                                        "quick_ask_conversation_history_count",
                                        3u64,
                                    );
                                let quick_ask_conversation_history_count: usize =
                                    quick_ask_conversation_history_count_raw.clamp(1, 20) as usize;

                                let global_qa_openai_reasoning_effort: Option<String> =
                                    get_setting_from_store(
                                        &app_clone,
                                        "quick_ask_openai_reasoning_effort",
                                        Option::<String>::None,
                                    );
                                let global_qa_gemini_thinking_budget: Option<i64> =
                                    get_setting_from_store(
                                        &app_clone,
                                        "quick_ask_gemini_thinking_budget",
                                        Option::<i64>::None,
                                    );
                                let global_qa_gemini_thinking_level: Option<String> =
                                    get_setting_from_store(
                                        &app_clone,
                                        "quick_ask_gemini_thinking_level",
                                        Option::<String>::None,
                                    );
                                let global_qa_anthropic_thinking_budget: Option<i64> =
                                    get_setting_from_store(
                                        &app_clone,
                                        "quick_ask_anthropic_thinking_budget",
                                        Option::<i64>::None,
                                    );

                                let fallback_provider: Option<String> = get_setting_from_store(
                                    &app_clone,
                                    "llm_provider",
                                    Option::<String>::None,
                                );

                                let provider = quick_ask_profile_cfg
                                    .provider
                                    .clone()
                                    .or(global_quick_ask_provider)
                                    .or(fallback_provider)
                                    .unwrap_or_else(|| "openai".to_string());
                                let model = quick_ask_profile_cfg
                                    .model
                                    .clone()
                                    .or(global_quick_ask_model);

                                let system_prompt = quick_ask_profile_cfg
                                    .system_prompt
                                    .clone()
                                    .or(global_quick_ask_system_prompt)
                                    .unwrap_or_else(|| {
                                        "You are a helpful assistant. Answer the user's question based on the transcript.".to_string()
                                    });

                                let openai_reasoning_effort = quick_ask_profile_cfg
                                    .openai_reasoning_effort
                                    .clone()
                                    .or(global_qa_openai_reasoning_effort);
                                let gemini_thinking_budget = quick_ask_profile_cfg
                                    .gemini_thinking_budget
                                    .or(global_qa_gemini_thinking_budget);
                                let gemini_thinking_level = quick_ask_profile_cfg
                                    .gemini_thinking_level
                                    .clone()
                                    .or(global_qa_gemini_thinking_level);
                                let anthropic_thinking_budget = quick_ask_profile_cfg
                                    .anthropic_thinking_budget
                                    .or(global_qa_anthropic_thinking_budget);

                                // Attach Quick Ask metadata to the current request log so it shows up
                                // as a distinct type in the UI.
                                if let Some(log_store) = app_clone.try_state::<RequestLogStore>() {
                                    log_store.with_current(|log| {
                                        log.kind = RequestKind::QuickAsk;
                                        log.quick_ask_question = Some(question.clone());
                                        log.quick_ask_provider = Some(provider.clone());
                                        log.quick_ask_model = model.clone();
                                        log.quick_ask_request_json = Some(serde_json::json!({
                                                "system_prompt": system_prompt.clone(),
                                                "question": question.clone(),
                                                "provider": provider.clone(),
                                                "model": model.clone(),
                                        }));
                                        log.info("Quick Ask: starting answer generation");
                                    });
                                }

                                crate::sessions::quick_ask::emit_to_quick_ask(
                                    &app_clone,
                                    crate::sessions::quick_ask::EVENT_QUICK_ASK_STARTED,
                                    QuickAskStartedPayload {
                                        question: Some(question.clone()),
                                        provider: Some(provider.clone()),
                                        model: model.clone(),
                                    },
                                );

                                let cfg = pipeline_clone.config();
                                let api_key = if provider == "ollama" {
                                    String::new()
                                } else {
                                    cfg.llm_api_keys
                                        .get(provider.as_str())
                                        .cloned()
                                        .unwrap_or_default()
                                };

                                if provider != "ollama" && api_key.trim().is_empty() {
                                    let err =
                                        format!("No API key configured for provider: {}", provider);
                                    if let Some(log_store) =
                                        app_clone.try_state::<RequestLogStore>()
                                    {
                                        log_store.with_current(|log| {
                                            log.kind = RequestKind::QuickAsk;
                                            log.error(format!("Quick Ask failed: {}", err));
                                            log.complete_error(err.clone());
                                        });

                                        if let Some(wav) = pipeline_clone.clone_last_wav_bytes() {
                                            stats::emit_cost_events_for_current_request(
                                                &app_clone,
                                                stats::EventStatus::Error,
                                                Some(&wav),
                                            );
                                        }

                                        log_store.complete_current();
                                    }

                                    crate::sessions::quick_ask::emit_to_quick_ask(
                                        &app_clone,
                                        crate::sessions::quick_ask::EVENT_QUICK_ASK_ANSWER,
                                        QuickAskAnswerPayload::Err(QuickAskAnswerErrorPayload {
                                            ok: false,
                                            error: err,
                                        }),
                                    );
                                } else {
                                    let provider_cfg = crate::llm::LlmConfig {
                                        enabled: true,
                                        provider: provider.clone(),
                                        api_key,
                                        model: model.clone(),
                                        ollama_url: cfg.llm_config.ollama_url.clone(),
                                        openai_reasoning_effort,
                                        gemini_thinking_budget,
                                        gemini_thinking_level,
                                        anthropic_thinking_budget,
                                        prompts: crate::llm::PromptSections::default(),
                                        program_prompt_profiles: Vec::new(),
                                        timeout: cfg.llm_config.timeout,
                                    };

                                    let provider_impl =
                                        crate::commands::llm::create_llm_provider_unstructured(
                                            &provider_cfg,
                                        );

                                    // Best-effort: attach any highlighted text captured at recording stop.
                                    // We keep this bounded so we don't blow up token usage.
                                    let selected_context: Option<String> =
                                        sessions::selection_probe::await_probe_result(
                                            &app_clone,
                                            sessions::selection_probe::ProbeKind::QuickAsk,
                                            quick_ask_epoch,
                                            700,
                                        )
                                        .await;

                                    let selected_context_trimmed = selected_context
                                        .as_deref()
                                        .map(str::trim)
                                        .filter(|s| !s.is_empty());

                                    // Read clipboard context if enabled for this profile.
                                    let clipboard_text: Option<String> = if quick_ask_profile_cfg
                                        .include_clipboard_context
                                    {
                                        // The selection probe may temporarily write a sentinel token into the clipboard
                                        // while it waits for the target app to copy the selection.
                                        // If we read clipboard context during that window, request logs can end up with
                                        // "__kolboo_selection_probe__..." as the clipboard context.
                                        // Wait for probe to finish first.
                                        let _ = sessions::selection_probe::await_probe_result(
                                            &app_clone,
                                            sessions::selection_probe::ProbeKind::QuickAsk,
                                            quick_ask_epoch,
                                            350,
                                        )
                                        .await;

                                        let is_probe_sentinel = |s: &str| {
                                            s.trim_start().starts_with("__kolboo_selection_probe__")
                                        };

                                        let v_first =
                                            clipboard_context::read_clipboard_text_best_effort_async(8000).await;

                                        // If we still see the sentinel, wait a beat and retry once.
                                        let v = if v_first.as_deref().is_some_and(is_probe_sentinel)
                                        {
                                            tokio::time::sleep(Duration::from_millis(120)).await;
                                            clipboard_context::read_clipboard_text_best_effort_async(8000).await
                                        } else {
                                            v_first
                                        };

                                        if v.as_deref().is_some_and(is_probe_sentinel) {
                                            None
                                        } else {
                                            v
                                        }
                                    } else {
                                        None
                                    };
                                    let clipboard_trimmed = clipboard_text
                                        .as_deref()
                                        .map(str::trim)
                                        .filter(|s| !s.is_empty());

                                    // Keep the two context sources distinct:
                                    // - highlighted selection (grab shortcut)
                                    // - clipboard text (optional)
                                    // Both are independently optional.
                                    let cap = 8_000usize;

                                    let selected_context_capped: Option<String> =
                                        selected_context_trimmed.map(|ctx| {
                                            if ctx.len() > cap {
                                                format!("{}\n\n… (truncated)", &ctx[..cap])
                                            } else {
                                                ctx.to_string()
                                            }
                                        });

                                    let clipboard_context_capped: Option<String> =
                                        clipboard_trimmed.map(|cb| {
                                            if cb.len() > cap {
                                                format!("{}\n\n… (truncated)", &cb[..cap])
                                            } else {
                                                cb.to_string()
                                            }
                                        });

                                    // This is the *exact* context text (if any) we attached to the question.
                                    // Stored for request logs/UI.
                                    let quick_ask_context_text_for_log: Option<String> =
                                        selected_context_capped.clone();
                                    let quick_ask_clipboard_context_for_log: Option<String> =
                                        clipboard_context_capped.clone();

                                    let question_with_context = crate::clipboard_context::build_quick_ask_user_message_with_context(
                                        question.as_str(),
                                        selected_context_capped.as_deref(),
                                        clipboard_context_capped.as_deref(),
                                    );

                                    // Best-effort: include last N Quick Ask turns from in-memory history.
                                    // We intentionally do NOT persist or log this conversation content.
                                    let question_with_context =
                                        if quick_ask_conversation_history_enabled {
                                            let turns: Vec<crate::state::QuickAskConversationTurn> =
                                                app_clone
                                                    .try_state::<QuickAskConversationMemory>()
                                                    .map(|m| {
                                                        m.snapshot_last(
                                                            quick_ask_conversation_history_count,
                                                        )
                                                    })
                                                    .unwrap_or_default();

                                            if turns.is_empty() {
                                                question_with_context
                                            } else {
                                                let mut s = String::new();
                                                s.push_str("Previous Quick Ask conversation (most recent last):\n\n");

                                                for t in turns.iter() {
                                                    let q = t.question.trim();
                                                    let a = t.answer.trim();
                                                    if q.is_empty() && a.is_empty() {
                                                        continue;
                                                    }

                                                    // Keep each turn reasonably bounded.
                                                    let cap = 1_500usize;
                                                    let q_capped = if q.len() > cap {
                                                        format!("{}…", &q[..cap])
                                                    } else {
                                                        q.to_string()
                                                    };
                                                    let a_capped = if a.len() > cap {
                                                        format!("{}…", &a[..cap])
                                                    } else {
                                                        a.to_string()
                                                    };

                                                    s.push_str("User: ");
                                                    s.push_str(&q_capped);
                                                    s.push_str("\nAssistant: ");
                                                    s.push_str(&a_capped);
                                                    s.push_str("\n\n");
                                                }

                                                s.push_str("---\n\n");
                                                s.push_str(&question_with_context);
                                                s
                                            }
                                        } else {
                                            question_with_context
                                        };

                                    // Update the logical request payload to indicate context was used.
                                    // We avoid logging the raw context string by default (it may contain secrets).
                                    if let Some(log_store) =
                                        app_clone.try_state::<RequestLogStore>()
                                    {
                                        let context_chars =
                                            selected_context_trimmed.map(|s| s.len());
                                        let clipboard_chars = clipboard_trimmed.map(|s| s.len());
                                        let quick_ask_context_text_for_log =
                                            quick_ask_context_text_for_log.clone();
                                        let quick_ask_clipboard_context_for_log =
                                            quick_ask_clipboard_context_for_log.clone();
                                        log_store.with_current(|log| {
                                            log.quick_ask_context_text =
                                                quick_ask_context_text_for_log;
                                            log.quick_ask_clipboard_context =
                                                quick_ask_clipboard_context_for_log;

                                            if let Some(serde_json::Value::Object(map)) =
                                                log.quick_ask_request_json.as_mut()
                                            {
                                                map.insert(
                                                    "context_present".to_string(),
                                                    serde_json::Value::Bool(
                                                        context_chars.is_some(),
                                                    ),
                                                );
                                                map.insert(
                                                    "context_chars".to_string(),
                                                    context_chars
                                                        .map(|n| {
                                                            serde_json::Value::Number(
                                                                serde_json::Number::from(n as u64),
                                                            )
                                                        })
                                                        .unwrap_or(serde_json::Value::Null),
                                                );
                                                map.insert(
                                                    "clipboard_context_present".to_string(),
                                                    serde_json::Value::Bool(
                                                        clipboard_chars.is_some(),
                                                    ),
                                                );
                                                map.insert(
                                                    "clipboard_context_chars".to_string(),
                                                    clipboard_chars
                                                        .map(|n| {
                                                            serde_json::Value::Number(
                                                                serde_json::Number::from(n as u64),
                                                            )
                                                        })
                                                        .unwrap_or(serde_json::Value::Null),
                                                );
                                            }
                                        });
                                    }

                                    let t0 = std::time::Instant::now();
                                    match provider_impl
                                        .complete(
                                            system_prompt.as_str(),
                                            question_with_context.as_str(),
                                        )
                                        .await
                                    {
                                        Ok(answer) => {
                                            let answer = answer.trim().to_string();
                                            let duration_ms = t0.elapsed().as_millis() as u64;

                                            // Record the successful Q/A turn into in-memory Quick Ask history.
                                            if let Some(mem) =
                                                app_clone.try_state::<QuickAskConversationMemory>()
                                            {
                                                mem.push_turn(question.clone(), answer.clone());
                                            }

                                            if let Some(log_store) =
                                                app_clone.try_state::<RequestLogStore>()
                                            {
                                                log_store.with_current(|log| {
                                                    log.kind = RequestKind::QuickAsk;
                                                    log.quick_ask_answer = Some(answer.clone());
                                                    log.quick_ask_provider =
                                                        Some(provider_impl.name().to_string());
                                                    log.quick_ask_model =
                                                        Some(provider_impl.model().to_string());
                                                    log.quick_ask_duration_ms = Some(duration_ms);
                                                    log.quick_ask_response_json =
                                                        Some(serde_json::json!({
                                                            "ok": true,
                                                            "answer": answer.clone(),
                                                            "provider_used": provider_impl.name(),
                                                            "model_used": provider_impl.model(),
                                                            "duration_ms": duration_ms,
                                                        }));
                                                    log.complete_success();
                                                });

                                                if let Some(wav) =
                                                    pipeline_clone.clone_last_wav_bytes()
                                                {
                                                    stats::emit_cost_events_for_current_request(
                                                        &app_clone,
                                                        stats::EventStatus::Success,
                                                        Some(&wav),
                                                    );
                                                }

                                                log_store.complete_current();
                                            }

                                            crate::sessions::quick_ask::emit_to_quick_ask(
                                                &app_clone,
                                                crate::sessions::quick_ask::EVENT_QUICK_ASK_ANSWER,
                                                QuickAskAnswerPayload::Ok(
                                                    QuickAskAnswerOkPayload {
                                                        ok: true,
                                                        answer,
                                                        provider_used: Some(
                                                            provider_impl.name().to_string(),
                                                        ),
                                                        model_used: Some(
                                                            provider_impl.model().to_string(),
                                                        ),
                                                        duration_ms: Some(duration_ms),
                                                    },
                                                ),
                                            );
                                        }
                                        Err(e) => {
                                            let err = e.to_string();
                                            if let Some(log_store) =
                                                app_clone.try_state::<RequestLogStore>()
                                            {
                                                log_store.with_current(|log| {
                                                    log.kind = RequestKind::QuickAsk;
                                                    log.quick_ask_answer = None;
                                                    log.quick_ask_response_json =
                                                        Some(serde_json::json!({
                                                            "ok": false,
                                                            "error": err.clone(),
                                                        }));
                                                    log.error(format!(
                                                        "Quick Ask failed: {}",
                                                        err.clone()
                                                    ));
                                                    log.complete_error(err.clone());
                                                });

                                                if let Some(wav) =
                                                    pipeline_clone.clone_last_wav_bytes()
                                                {
                                                    stats::emit_cost_events_for_current_request(
                                                        &app_clone,
                                                        stats::EventStatus::Error,
                                                        Some(&wav),
                                                    );
                                                }

                                                log_store.complete_current();
                                            }

                                            crate::sessions::quick_ask::emit_to_quick_ask(
                                                &app_clone,
                                                crate::sessions::quick_ask::EVENT_QUICK_ASK_ANSWER,
                                                QuickAskAnswerPayload::Err(
                                                    QuickAskAnswerErrorPayload {
                                                        ok: false,
                                                        error: err,
                                                    },
                                                ),
                                            );
                                        }
                                    }
                                }
                            }
                        } else {
                            // Output the transcript based on mode.
                            // If Quick Replace is enabled and we captured highlighted text when transcription started,
                            // rewrite the selection using the transcript as an instruction.
                            if quick_replace_cfg.enabled && quick_replace_epoch != 0 {
                                // Wait briefly for the selection probe to finish (best-effort).
                                let selected_text = sessions::selection_probe::await_probe_result(
                                    &app_clone,
                                    sessions::selection_probe::ProbeKind::QuickReplace,
                                    quick_replace_epoch,
                                    700,
                                )
                                .await;

                                if let Some(selected) = selected_text
                                    .as_ref()
                                    .map(|s| s.trim())
                                    .filter(|s| !s.is_empty())
                                {
                                    if let Some(log_store) =
                                        app_clone.try_state::<RequestLogStore>()
                                    {
                                        log_store.with_current(|log| {
                                            log.kind = RequestKind::QuickReplace;
                                            log.info(format!(
                                                "Quick replace: rewriting selection ({} chars)",
                                                selected.len()
                                            ));

                                            // Best-effort: keep these bounded so request logs stay usable.
                                            let cap = 8_000usize;
                                            let selected_capped = if selected.len() > cap {
                                                format!("{}\n\n… (truncated)", &selected[..cap])
                                            } else {
                                                selected.to_string()
                                            };

                                            let instructions = output_value.trim().to_string();
                                            let instructions_capped = if instructions.len() > cap {
                                                format!("{}\n\n… (truncated)", &instructions[..cap])
                                            } else {
                                                instructions
                                            };

                                            log.quick_replace_selected_text = Some(selected_capped);
                                            log.quick_replace_instructions =
                                                Some(instructions_capped);
                                        });
                                    }

                                    // Resolve effective LLM config from the Quick Replace profile settings.
                                    let cfg = pipeline_clone.config();
                                    let provider = quick_replace_cfg
                                        .provider
                                        .clone()
                                        .unwrap_or_else(|| cfg.llm_config.provider.clone());
                                    let model = quick_replace_cfg
                                        .model
                                        .clone()
                                        .or_else(|| cfg.llm_config.model.clone())
                                        .or_else(|| {
                                            llm::default_llm_model_for_provider(provider.as_str())
                                                .map(|m| m.to_string())
                                        });
                                    let api_key = if provider == "ollama" {
                                        String::new()
                                    } else {
                                        cfg.llm_api_keys
                                            .get(provider.as_str())
                                            .cloned()
                                            .unwrap_or_default()
                                    };

                                    if provider != "ollama" && api_key.trim().is_empty() {
                                        let err = format!(
                                            "Quick Replace failed: no API key configured for provider: {}",
                                            provider
                                        );
                                        if let Some(log_store) =
                                            app_clone.try_state::<RequestLogStore>()
                                        {
                                            log_store.with_current(|log| {
                                                log.warn(format!(
                                                    "Quick replace: skipped (no API key configured for provider: {})",
                                                    provider
                                                ));
                                                log.quick_replace_provider = Some(provider.clone());
                                                log.quick_replace_model = quick_replace_cfg.model.clone();
                                                log.quick_replace_response_json = Some(serde_json::json!({
                                                    "ok": false,
                                                    "error": err.clone(),
                                                }));
                                                log.error(err.clone());
                                                log.complete_error(err.clone());
                                            });
                                        }

                                        quick_replace_failure = Some(err);
                                    } else {
                                        // NOTE: Keep prompts minimal and instruct the model to output only the rewritten text.
                                        let system_prompt = quick_replace_cfg.system_prompt.clone();
                                        let instructions_text = output_value.trim().to_string();

                                        // Read clipboard context if enabled for this profile.
                                        let clipboard_text: Option<String> = if quick_replace_cfg
                                            .include_clipboard_context
                                        {
                                            clipboard_context::read_clipboard_text_best_effort_async(8000).await
                                        } else {
                                            None
                                        };

                                        let user_prompt = if let Some(ref cb) = clipboard_text {
                                            format!(
                                                "INSTRUCTIONS:\n{}\n\nSELECTED TEXT:\n{}\n\nCLIPBOARD CONTEXT:\n{}\n\nReturn only the updated text.",
                                                instructions_text,
                                                selected,
                                                cb.trim()
                                            )
                                        } else {
                                            format!(
                                                "INSTRUCTIONS:\n{}\n\nSELECTED TEXT:\n{}\n\nReturn only the updated text.",
                                                instructions_text,
                                                selected
                                            )
                                        };

                                        // Store clipboard context in request log if present.
                                        if let Some(ref cb_text) = clipboard_text {
                                            if let Some(log_store) =
                                                app_clone.try_state::<RequestLogStore>()
                                            {
                                                log_store.with_current(|log| {
                                                    log.quick_replace_clipboard_context =
                                                        Some(cb_text.clone());
                                                });
                                            }
                                        }

                                        let provider_cfg = crate::llm::LlmConfig {
                                            enabled: true,
                                            provider: provider.clone(),
                                            api_key,
                                            model,
                                            ollama_url: cfg.llm_config.ollama_url.clone(),
                                            openai_reasoning_effort: cfg
                                                .llm_config
                                                .openai_reasoning_effort
                                                .clone(),
                                            gemini_thinking_budget: cfg
                                                .llm_config
                                                .gemini_thinking_budget,
                                            gemini_thinking_level: cfg
                                                .llm_config
                                                .gemini_thinking_level
                                                .clone(),
                                            anthropic_thinking_budget: cfg
                                                .llm_config
                                                .anthropic_thinking_budget,
                                            prompts: crate::llm::PromptSections::default(),
                                            program_prompt_profiles: Vec::new(),
                                            timeout: cfg.llm_config.timeout,
                                        };

                                        let provider_impl =
                                            crate::commands::llm::create_llm_provider_unstructured(
                                                &provider_cfg,
                                            );
                                        let t0 = Instant::now();
                                        match provider_impl
                                            .complete(&system_prompt, &user_prompt)
                                            .await
                                        {
                                            Ok(rewritten) => {
                                                let rewritten = rewritten.trim().to_string();
                                                if rewritten.is_empty() {
                                                    let err = "Quick Replace failed: model returned empty output".to_string();
                                                    if let Some(log_store) =
                                                        app_clone.try_state::<RequestLogStore>()
                                                    {
                                                        log_store.with_current(|log| {
                                                            log.kind = RequestKind::QuickReplace;
                                                            log.quick_replace_provider = Some(
                                                                provider_impl.name().to_string(),
                                                            );
                                                            log.quick_replace_model = Some(
                                                                provider_impl.model().to_string(),
                                                            );
                                                            log.quick_replace_response_json =
                                                                Some(serde_json::json!({
                                                                    "ok": false,
                                                                    "error": err.clone(),
                                                                }));
                                                            log.error(err.clone());
                                                            log.complete_error(err.clone());
                                                        });
                                                    }
                                                    quick_replace_failure = Some(err);
                                                } else {
                                                    output_value = rewritten;
                                                    if let Some(log_store) =
                                                        app_clone.try_state::<RequestLogStore>()
                                                    {
                                                        let ms = t0.elapsed().as_millis() as u64;
                                                        log_store.with_current(|log| {
                                                            log.kind = RequestKind::QuickReplace;
                                                            log.info(format!(
                                                                "Quick replace: rewrite succeeded in {}ms ({} chars)",
                                                                ms,
                                                                output_value.len()
                                                            ));

                                                            log.quick_replace_provider =
                                                                Some(provider_impl.name().to_string());
                                                            log.quick_replace_model =
                                                                Some(provider_impl.model().to_string());
                                                            log.quick_replace_duration_ms = Some(ms);
                                                            log.quick_replace_output_text = Some(output_value.clone());
                                                            log.quick_replace_request_json = Some(serde_json::json!({
                                                                "provider": provider.clone(),
                                                                "model": provider_impl.model(),
                                                                "system_prompt": system_prompt.clone(),
                                                                "instructions_chars": instructions_text.len(),
                                                                "selected_text_chars": selected.len(),
                                                            }));
                                                            log.quick_replace_response_json = Some(serde_json::json!({
                                                                "ok": true,
                                                                "provider_used": provider_impl.name(),
                                                                "model_used": provider_impl.model(),
                                                                "duration_ms": ms,
                                                                "output_chars": output_value.len(),
                                                            }));

                                                            // The effective final output for this request.
                                                            log.formatted_transcript = Some(output_value.clone());
                                                        });
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                let err = e.to_string();
                                                if let Some(log_store) =
                                                    app_clone.try_state::<RequestLogStore>()
                                                {
                                                    log_store.with_current(|log| {
                                                        log.kind = RequestKind::QuickReplace;
                                                        log.quick_replace_provider = Some(provider.clone());
                                                        log.quick_replace_model = quick_replace_cfg.model.clone();
                                                        log.quick_replace_request_json = Some(serde_json::json!({
                                                            "provider": provider.clone(),
                                                            "system_prompt": system_prompt.clone(),
                                                            "selected_text_chars": selected.len(),
                                                        }));
                                                        log.quick_replace_response_json = Some(serde_json::json!({
                                                            "ok": false,
                                                            "error": err.clone(),
                                                        }));
                                                        log.warn(format!(
                                                            "Quick replace: rewrite failed ({})",
                                                            err
                                                        ));

                                                        // Treat rewrite failure as a request error; do not pretend success.
                                                        log.error(format!("Quick Replace failed: {}", err.clone()));
                                                        log.complete_error(err.clone());
                                                    });
                                                }

                                                quick_replace_failure = Some(err);
                                            }
                                        }
                                    }
                                }
                            }

                            // If we deferred request-log completion for Quick Replace, finalize it now.
                            // (Quick Ask handles its own completion in its branch.)
                            if quick_replace_cfg.enabled && quick_replace_epoch != 0 {
                                if let Some(log_store) = app_clone.try_state::<RequestLogStore>() {
                                    log_store.with_current(|log| {
                                        if log.status
                                            == crate::request_log::RequestStatus::InProgress
                                        {
                                            if quick_replace_failure.is_some() {
                                                // Preserve error status set earlier.
                                            } else {
                                                log.complete_success();
                                            }
                                        }
                                    });

                                    if let Some(wav) = pipeline_clone.clone_last_wav_bytes() {
                                        stats::emit_cost_events_for_current_request(
                                            &app_clone,
                                            if quick_replace_failure.is_some() {
                                                stats::EventStatus::Error
                                            } else {
                                                stats::EventStatus::Success
                                            },
                                            Some(&wav),
                                        );
                                    }

                                    log_store.complete_current();
                                }
                            }

                            // If Quick Replace was intended but failed, surface an error instead of
                            // pasting the transcript into the selection.
                            if let Some(err) = quick_replace_failure.as_deref() {
                                // In hotkey-triggered flows, the overlay webview may not be created
                                // (or visible) yet. Show it so the error state + Retry UI is actually seen.
                                let _ = commands::overlay::show_overlay(app_clone.clone()).await;

                                // Emit pipeline-error so overlay shows error state + retry affordance.
                                let payload = PipelineErrorPayload {
                                    message: err.to_string(),
                                    request_id: request_id.clone(),
                                };
                                let _ = app_clone.emit(events::EVENT_PIPELINE_ERROR, payload);
                                let _ = app_clone.emit(
                                    events::EVENT_PIPELINE_STATE_CHANGED,
                                    PipelineStateEvent::Error,
                                );
                            } else {
                                // Output using the selected output mode.
                                let output_clipboard_privacy_mode: bool = get_setting_from_store(
                                    &app_clone,
                                    "output_clipboard_privacy_mode",
                                    false,
                                );

                                if let Err(e) = commands::text::output_text_with_mode_options(
                                    &output_value,
                                    output_mode,
                                    output_hit_enter,
                                    !output_clipboard_privacy_mode,
                                ) {
                                    log::error!("Failed to output transcript: {}", e);

                                    if let Some(log_store) =
                                        app_clone.try_state::<RequestLogStore>()
                                    {
                                        log_store.with_current(|log| {
                                            log.warn(format!("Output failed: {}", e));
                                        });
                                    }
                                }
                            }
                        }

                        // Save to history
                        if !is_quick_ask_session {
                            if let Some(req_id) = request_id.as_ref() {
                                if let Some(history) = app_clone.try_state::<HistoryStorage>() {
                                    // Store the actual output (Quick Replace may have changed it).
                                    if let Some(err) = quick_replace_failure.as_deref() {
                                        let _ =
                                            history.complete_request_error(req_id, err.to_string());
                                    } else if let Err(e) = history
                                        .complete_request_success(req_id, output_value.clone())
                                    {
                                        log::warn!("Failed to update history: {}", e);
                                    }

                                    let (provider, model) = if result.llm_attempted() {
                                        (
                                            result.llm_provider_used.clone(),
                                            result.llm_model_used.clone(),
                                        )
                                    } else {
                                        (None, None)
                                    };
                                    let _ = history.set_request_llm_model(req_id, provider, model);
                                    let _ = app_clone.emit(events::EVENT_HISTORY_CHANGED, ());
                                }
                            }
                        }

                        // Time-based retention (best-effort). This path is used by global shortcuts.
                        commands::recording::apply_transcription_retention(&app_clone);
                    } else {
                        // Emit empty transcript event so UI can update appropriately
                        let _ = app_clone.emit(events::EVENT_PIPELINE_TRANSCRIPT_READY, "");
                        let _ = app_clone.emit(
                            events::EVENT_PIPELINE_STATE_CHANGED,
                            PipelineStateEvent::Idle,
                        );
                        log::info!("No transcript output (empty/whitespace), not outputting");

                        if is_quick_ask_session {
                            // Ensure the answer window is visible so the error is actually seen.
                            crate::sessions::quick_ask::ensure_quick_ask_window_visible(&app_clone);

                            // Finalize the deferred Quick Ask request log.
                            if let Some(log_store) = app_clone.try_state::<RequestLogStore>() {
                                log_store.with_current(|log| {
                                    log.kind = RequestKind::QuickAsk;
                                    log.quick_ask_question = Some(String::new());
                                    log.error("Quick Ask failed: no transcript to answer (empty)");
                                    log.complete_error("No transcript to answer (empty)");
                                });

                                if let Some(wav) = pipeline_clone.clone_last_wav_bytes() {
                                    stats::emit_cost_events_for_current_request(
                                        &app_clone,
                                        stats::EventStatus::Error,
                                        Some(&wav),
                                    );
                                }

                                log_store.complete_current();
                            }

                            crate::sessions::quick_ask::emit_to_quick_ask(
                                &app_clone,
                                crate::sessions::quick_ask::EVENT_QUICK_ASK_ANSWER,
                                QuickAskAnswerPayload::Err(QuickAskAnswerErrorPayload {
                                    ok: false,
                                    error: "No transcript to answer (empty)".to_string(),
                                }),
                            );
                        }

                        // Mark history entry as success with empty text (keeps timeline consistent)
                        if !is_quick_ask_session {
                            if let Some(req_id) = request_id.as_ref() {
                                if let Some(history) = app_clone.try_state::<HistoryStorage>() {
                                    let _ = history.complete_request_success(req_id, String::new());

                                    let (provider, model) = if result.llm_attempted() {
                                        (
                                            result.llm_provider_used.clone(),
                                            result.llm_model_used.clone(),
                                        )
                                    } else {
                                        (None, None)
                                    };
                                    let _ = history.set_request_llm_model(req_id, provider, model);
                                    let _ = app_clone.emit(events::EVENT_HISTORY_CHANGED, ());
                                }
                            }
                        }

                        // Time-based retention (best-effort). This path is used by global shortcuts.
                        commands::recording::apply_transcription_retention(&app_clone);
                    }

                    // Hide overlay after transcription completes if in "recording_only" mode.
                    // We request a hide so the frontend can animate (zoom-out) before the webview hides.
                    if overlay_mode_clone == "recording_only" {
                        let _ = app_clone.emit(events::EVENT_OVERLAY_HIDE_REQUESTED, ());

                        // Fallback: if the overlay frontend isn't running/listening, hide anyway.
                        // Re-check the current overlay_mode before hiding to avoid races with settings changes.
                        if let Some(window) = app_clone.get_webview_window("overlay") {
                            let window_clone = window.clone();
                            let app_check = app_clone.clone();
                            let expected_epoch = app_clone
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
                                if current_mode == "recording_only"
                                    && current_epoch == expected_epoch
                                {
                                    let _ = window_clone.hide();
                                }
                            });
                        }
                    }
                }
                Err(e) => {
                    if matches!(e, pipeline::PipelineError::Cancelled) {
                        log::info!("Transcription cancelled");

                        // Mark request as cancelled (best-effort)
                        if let Some(log_store) = app_clone.try_state::<RequestLogStore>() {
                            log_store.with_current(|log| {
                                log.warn("Recording cancelled by user");
                                log.complete_cancelled();
                            });

                            // Persist cost/usage stats (best-effort).
                            if let Some(wav) = pipeline_clone.clone_last_wav_bytes() {
                                stats::emit_cost_events_for_current_request(
                                    &app_clone,
                                    stats::EventStatus::Cancelled,
                                    Some(&wav),
                                );
                            }

                            log_store.complete_current();
                        }

                        // Notify frontend and hide overlay if needed.
                        let _ = app_clone.emit(events::EVENT_PIPELINE_CANCELLED, ());
                        let _ = app_clone.emit(
                            events::EVENT_PIPELINE_STATE_CHANGED,
                            PipelineStateEvent::Idle,
                        );

                        // Best-effort: remove any in-progress history entry for this request
                        // so it doesn't remain stuck in "in_progress".
                        if let Some(req_id) = request_id.as_ref() {
                            if let Some(history) = app_clone.try_state::<HistoryStorage>() {
                                let _ = history.delete(req_id);
                                let _ = app_clone.emit(events::EVENT_HISTORY_CHANGED, ());
                            }
                        }

                        if overlay_mode_clone == "recording_only" {
                            let _ = app_clone.emit(events::EVENT_OVERLAY_HIDE_REQUESTED, ());
                            if let Some(window) = app_clone.get_webview_window("overlay") {
                                let _ = window.hide();
                            }
                        }

                        // Done - stop stealing Escape.
                        set_escape_cancel_shortcut_enabled(&app_clone, false);
                        return;
                    }

                    log::error!("Transcription failed: {}", e);
                    let payload = PipelineErrorPayload {
                        message: e.to_string(),
                        request_id: request_id.clone(),
                    };
                    let _ = app_clone.emit(events::EVENT_PIPELINE_ERROR, payload);
                    let _ = app_clone.emit(
                        events::EVENT_PIPELINE_STATE_CHANGED,
                        PipelineStateEvent::Error,
                    );

                    if let Some(log_store) = app_clone.try_state::<RequestLogStore>() {
                        log_store.with_current(|log| {
                            log.error_with_details(
                                format!("Transcription failed: {}", e),
                                crate::request_log::format_error_chain(&e),
                            );
                            log.complete_error(e.to_string());
                        });

                        // Persist cost/usage stats (best-effort).
                        if let Some(wav) = pipeline_clone.clone_last_wav_bytes() {
                            stats::emit_cost_events_for_current_request(
                                &app_clone,
                                stats::EventStatus::Error,
                                Some(&wav),
                            );
                        }

                        log_store.complete_current();
                    }

                    // Persist audio for retry (best-effort)
                    if let (Some(req_id), Some(store)) = (
                        request_id.as_deref(),
                        app_clone.try_state::<RecordingStore>(),
                    ) {
                        if let Some(wav) = pipeline_clone.clone_last_wav_bytes() {
                            if store.save_wav(req_id, &wav).is_ok() {
                                let max_saved_recordings: usize = (get_setting_from_store(
                                    &app_clone,
                                    "max_saved_recordings",
                                    1000u64,
                                ))
                                .clamp(1, 100_000)
                                    as usize;

                                let _ = store.prune_to_max_files(max_saved_recordings);
                            }
                        }
                    }

                    // Mark history entry as error and keep it
                    if let Some(req_id) = request_id.as_ref() {
                        if let Some(history) = app_clone.try_state::<HistoryStorage>() {
                            let _ = history.complete_request_error(req_id, e.to_string());
                            let _ = app_clone.emit(events::EVENT_HISTORY_CHANGED, ());
                        }
                    }

                    // Time-based retention (best-effort). Still apply even on failures.
                    commands::recording::apply_transcription_retention(&app_clone);

                    // Force-show overlay for retry UI regardless of overlay_mode.
                    // If the user is not in always-visible mode, also snap back to the saved preset.
                    if let Err(e) =
                        commands::overlay::show_overlay_with_reset_if_not_always(&app_clone)
                    {
                        log::warn!("Failed to force-show overlay after error: {}", e);
                    }
                }
            }

            // Transcription finished (success or error) - stop stealing Escape.
            set_escape_cancel_shortcut_enabled(&app_clone, false);
        });
    }

    let _ = app.emit(events::EVENT_RECORDING_STOP, ());
}

/// Check if audio mute is supported on this platform
#[tauri::command]
fn is_audio_mute_supported() -> bool {
    audio_mute::is_supported()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize structured tracing (JSON logs + request spans).
    tracing_init::init();

    let mut builder = tauri::Builder::default();

    #[cfg(desktop)]
    {
        builder = builder.plugin(shortcuts::build_global_shortcut_plugin());
        builder = builder.plugin(tauri_plugin_dialog::init());
    }

    #[cfg(target_os = "macos")]
    {
        builder = builder.plugin(tauri_nspanel::init());
    }

    builder
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .manage(AppState::default())
        .manage(QuickAskConversationMemory::default())
        .manage(TrayKeepAlive::default())
        .manage(MicTestMeterState::default())
        .manage(commands::whisper::WhisperDownloadManager::default())
        .invoke_handler(tauri::generate_handler![
            commands::audio::play_audio_cue_preview,
            commands::audio::list_audio_input_devices,
            commands::audio::list_audio_input_devices_v2,
            commands::audio::get_default_audio_input_device_name,
            commands::audio::mic_test_start_meter,
            commands::audio::mic_test_stop_meter,
            commands::text::type_text,
            commands::text::get_server_url,
            commands::settings::register_shortcuts,
            commands::settings::unregister_shortcuts,
            commands::settings::set_hotkey_debug_enabled_runtime,
            commands::settings::settings_doctor,
            commands::settings::settings_apply_patch,
            is_audio_mute_supported,
            commands::history::add_history_entry,
            commands::history::get_history,
            commands::history::get_history_page,
            commands::history::delete_history_entry,
            commands::history::get_history_delete_options,
            commands::history::delete_history_entry_ex,
            commands::history::clear_history,
            commands::overlay::resize_overlay,
            commands::overlay::show_overlay,
            commands::overlay::hide_overlay,
            commands::overlay::show_overlay_hover,
            commands::overlay::hide_overlay_hover,
            commands::overlay::schedule_hide_overlay_hover,
            commands::overlay::set_overlay_mode,
            commands::overlay::set_widget_position,
            // Pipeline commands for all-in-app STT
            commands::recording::pipeline_start_recording,
            commands::recording::pipeline_get_active_profile_for_foreground_app,
            commands::recording::pipeline_get_session_preset_lock,
            commands::recording::pipeline_set_session_preset_lock,
            commands::recording::pipeline_stop_and_transcribe,
            commands::recording::pipeline_cancel,
            commands::recording::pipeline_get_state,
            commands::recording::pipeline_is_recording,
            commands::recording::pipeline_is_error,
            commands::recording::pipeline_update_config,
            commands::recording::pipeline_dictate,
            commands::recording::pipeline_toggle,
            commands::recording::pipeline_force_reset,
            commands::recording::pipeline_test_transcribe_last_audio,
            commands::recording::pipeline_has_last_audio,
            commands::recording::pipeline_get_last_recording_diagnostics,
            commands::recording::pipeline_test_audio_settings_start_recording,
            commands::recording::pipeline_test_audio_settings_stop_recording,
            commands::recording::pipeline_retry_transcription,
            // Recording file access (for playback)
            commands::recording::recording_get_wav_path,
            commands::recording::recording_get_wav_base64,
            // Recording folder helpers
            commands::recording::recordings_open_folder,
            commands::recording::recordings_get_storage_bytes,
            commands::recording::recordings_get_stats,
            commands::recording::recordings_delete_all,
            // Danger-zone data operations
            commands::data::delete_all_api_keys,
            commands::data::delete_all_settings,
            commands::data::delete_all_stats,
            commands::data::get_data_storage_summary,
            commands::data::delete_all_transcripts_keep_recordings,
            commands::data::delete_all_data,
            // Backups (export/import settings; exclude secrets)
            commands::backup::export_settings_backup_json,
            commands::backup::export_settings_backup_to_file,
            commands::backup::import_settings_backup_json,
            commands::backup::import_settings_backup_from_file,
            // Optional GitHub Gist backup
            commands::backup::github_backup_has_token,
            commands::backup::github_backup_set_token,
            commands::backup::github_backup_clear_token,
            commands::backup::github_backup_push_to_gist,
            commands::backup::github_backup_pull_from_gist,
            // Secure secrets (API keys)
            commands::secrets::secrets_has_api_key,
            commands::secrets::secrets_get_api_key,
            commands::secrets::secrets_set_api_key,
            commands::secrets::secrets_clear_api_key,
            // Config commands (replacing Python server)
            commands::config::get_default_sections,
            commands::config::get_available_providers,
            commands::config::sync_pipeline_config,
            // Network commands
            commands::network::get_system_proxy_info,
            commands::network::load_trusted_ca_certificate_from_file,
            // VAD settings commands
            commands::config::get_vad_settings,
            commands::config::set_vad_settings,
            // LLM formatting commands
            commands::llm::get_llm_default_prompts,
            commands::llm::get_llm_providers,
            commands::llm::update_llm_config,
            commands::llm::update_llm_prompts,
            commands::llm::get_llm_config,
            commands::llm::test_llm_rewrite,
            commands::llm::iterate_rewrite_prompt,
            commands::llm::test_rewrite_with_prompt,
            commands::llm::llm_complete,
            // Router helpers
            commands::router::cache_router_embeddings,
            // Local Whisper model management commands
            commands::whisper::is_local_whisper_available,
            commands::whisper::get_local_whisper_backend_status,
            commands::whisper::is_local_whisper_model_loaded,
            commands::whisper::load_local_whisper_model,
            commands::whisper::unload_local_whisper_model,
            commands::whisper::get_whisper_models,
            commands::whisper::get_whisper_models_dir,
            commands::whisper::is_whisper_model_downloaded,
            commands::whisper::get_whisper_model_url,
            commands::whisper::delete_whisper_model,
            commands::whisper::validate_whisper_model,
            commands::whisper::download_whisper_model,
            commands::whisper::cancel_whisper_model_download,
            // Request logging commands
            commands::logs::get_request_logs,
            commands::logs::clear_request_logs,
            commands::logs::export_request_logs_to_file,
            // Fireworks helpers
            commands::fireworks::fireworks_list_models,
            // Ollama helpers
            commands::ollama::ollama_list_models,
            // Usage/cost stats commands
            commands::stats::get_cost_summary,
            commands::stats::get_cost_summary_v2,
            commands::stats::get_cost_by_provider_v2,
            commands::pricing::get_model_pricing,
            // Window/process commands (used for per-program prompts)
            commands::windows::list_open_windows,
            commands::windows::get_foreground_process_path,
        ])
        .setup(|app| {
            // Seed defaults into settings.json so UI and backend agree on effective settings.
            // Must run before pipeline initialization and any settings reads.
            #[cfg(desktop)]
            {
                settings::defaults::ensure_default_settings(app.handle())?;
            }

            // Dev-only: validate settings shape at startup and print issues to the terminal.
            #[cfg(all(desktop, debug_assertions))]
            {
                use tauri_plugin_store::StoreExt;

                let store = app.store("settings.json")?;
                let mut values = serde_json::Map::new();

                for key in settings::doctor::SETTINGS_DOCTOR_KEYS {
                    if let Some(value) = store.get(*key) {
                        values.insert((*key).to_string(), value);
                    }
                }

                let report = settings::doctor::validate_settings_map(&values);
                if report.issues.is_empty() {
                    log::info!("Settings doctor: no issues found");
                } else {
                    log::warn!(
                        "Settings doctor: found {} issue(s)",
                        report.issues.len()
                    );
                    for issue in report.issues {
                        log::warn!("Settings doctor issue: {} -> {}", issue.key, issue.message);
                    }
                }
            }

            // Startup window visibility:
            // - Show the main window only on first-run (when the setup guide is pending).
            // - Otherwise, keep it hidden; the tray icon is the explicit entrypoint.
            #[cfg(desktop)]
            {
                let guide_state: String = get_setting_from_store(
                    app.handle(),
                    "settings_guide_state",
                    "pending".to_string(),
                );

                if let Some(main) = app.get_webview_window("main") {
                    if guide_state == "pending" {
                        log::info!(
                            "Startup: settings guide is pending -> showing main window"
                        );
                        let _ = main.show();
                        let _ = main.unminimize();
                        let _ = main.set_focus();
                    } else {
                        log::info!(
                            "Startup: settings guide state is '{}' -> keeping main window hidden",
                            guide_state
                        );
                        let _ = main.hide();
                    }
                } else {
                    // If the window isn't present (e.g. it was closed/destroyed or config changed),
                    // the tray will recreate it on demand.
                    log::warn!(
                        "Startup: main window not found; tray will recreate it on demand"
                    );
                }
            }

            // Windows-only: enable modifier-only hotkeys (e.g. Right Alt alone) via a low-level
            // keyboard hook. This is separate from tauri-plugin-global-shortcut.
            #[cfg(target_os = "windows")]
            {
                windows_modifier_hotkeys::init(app.handle().clone());
            }

            // Configure what happens when the user clicks the X on the main/settings window.
            // Default is to close-to-tray (destroy the window; tray can recreate it).
            #[cfg(desktop)]
            {
                if let Some(main) = app.get_webview_window("main") {
                    let app_handle = app.handle().clone();
                    main.on_window_event(move |event| {
                        if let WindowEvent::CloseRequested { api, .. } = event {
                            // Hotkey debug is meant to be temporary. If the user closes the
                            // main window (close-to-tray), disable it so it can't keep
                            // generating high-volume debug events in the background.
                            #[cfg(target_os = "windows")]
                            {
                                crate::windows_modifier_hotkeys::set_hotkey_debug_enabled(false);
                            }

                            // Best-effort: persist the flag off so it doesn't remain enabled
                            // across sessions.
                            {
                                use tauri_plugin_store::StoreExt;

                                if let Ok(store) = app_handle.store("settings.json") {
                                    store.set("hotkey_debug_enabled", serde_json::json!(false));
                                    let _ = store.save();
                                }
                            }

                            let behavior: String = get_setting_from_store(
                                &app_handle,
                                "main_window_close_behavior",
                                "minimize_to_tray".to_string(),
                            );

                            if behavior == "exit_program" {
                                log::info!("Main window close requested -> exiting (exit_program)");
                                api.prevent_close();
                                app_handle.exit(0);
                                return;
                            }

                            // RAM-saving behavior: allow the window to be destroyed.
                            // The tray's Show action will recreate the window if needed.
                            // Also covers the legacy value "close_window".
                            log::info!(
                                "Main window close requested -> closing window (close-to-tray via {behavior})"
                            );
                        }
                    });
                } else {
                    log::warn!("Main window not found during setup; tray will recreate it on demand");
                }
            }

            // Initialize history storage
            let app_data_dir = app_paths::app_data_dir(app.handle())
                .expect("Failed to get app data directory");

            // Initialize recording store (saved WAVs for retry)
            let recording_store = RecordingStore::new(app_data_dir.clone());
            app.manage(recording_store);

            let history_storage = HistoryStorage::new(app_data_dir.clone());
            app.manage(history_storage);

            // Initialize persisted stats store (usage/cost ledger)
            let stats_store = stats::StatsStore::new(app_data_dir);
            app.manage(stats_store);

            #[cfg(desktop)]
            {
                bootstrap::apply_startup_retention(app.handle());
            }

            // Initialize request log store
            bootstrap::initialize_request_log_store(app.handle());

            // Initialize audio mute manager (may be None on unsupported platforms)
            if let Some(audio_mute_manager) = AudioMuteManager::new() {
                app.manage(audio_mute_manager);
            }

            // Initialize pipeline with settings from store
            #[cfg(desktop)]
            {
                let pipeline = bootstrap::initialize_pipeline_from_settings(app.handle());

                // Best-effort: preload local-whisper model at launch.
                // Do this in the background so startup remains snappy.
                #[cfg(feature = "local-whisper")]
                {
                    let preload_pipeline = pipeline.clone();
                    if preload_pipeline.config().local_whisper_load_mode == "on_launch"
                        && !preload_pipeline.is_local_whisper_loaded()
                    {
                        // Emit load events so the UI can update (and so users can see
                        // that on-launch loading is actually happening).
                        let app_handle_for_emit = app.handle().clone();
                        let _ = app_handle_for_emit.emit(
                            commands::whisper::LOCAL_WHISPER_MODEL_LOAD_EVENT,
                            commands::whisper::LocalWhisperModelLoadEvent {
                                status: commands::whisper::LocalWhisperModelLoadStatus::Started,
                                message: None,
                            },
                        );

                        let app_handle_for_emit_done = app_handle_for_emit.clone();
                        tauri::async_runtime::spawn_blocking(move || {
                            let result = preload_pipeline.force_load_local_whisper();

                            let payload = match result {
                                Ok(()) => commands::whisper::LocalWhisperModelLoadEvent {
                                    status: commands::whisper::LocalWhisperModelLoadStatus::Completed,
                                    message: None,
                                },
                                Err(e) => commands::whisper::LocalWhisperModelLoadEvent {
                                    status: commands::whisper::LocalWhisperModelLoadStatus::Error,
                                    message: Some(e.to_string()),
                                },
                            };

                            let _ = app_handle_for_emit_done.emit(
                                commands::whisper::LOCAL_WHISPER_MODEL_LOAD_EVENT,
                                payload,
                            );
                        });
                    }
                }

                // Preload persisted router embeddings cache once at startup.
                // Routing uses the in-memory cache and does not read the store per request.
                let _ = router_embeddings_cache::migrate_router_embeddings_out_of_settings(app.handle());
                let persisted = router_embeddings_cache::load_router_embeddings_from_store(app.handle());
                if !persisted.is_empty() {
                    pipeline.preload_embedding_cache(persisted);
                }

                app.manage(pipeline);
            }

            // Backend-driven overlay waveform: publish realtime mic levels to the overlay.
            // This avoids browser getUserMedia startup latency and stays aligned with the
            // actual CPAL capture stream.
            #[cfg(desktop)]
            {
                overlay::spawn_overlay_waveform_publisher(app.handle());
            }

            // Register shortcuts from store (now that store plugin is available)
            #[cfg(desktop)]
            {
                shortcuts::register_initial_shortcuts(app.handle())?;
            }

            overlay::create_overlay_windows(app)?;

            // Setup system tray
            bootstrap::setup_tray(app.handle())?;

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
