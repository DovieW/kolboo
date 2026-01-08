//! Tauri commands for the recording pipeline.
//!
//! These commands expose the recording pipeline functionality to the frontend,
//! enabling voice dictation directly from the Tauri app.

use crate::audio_capture::{AudioCaptureDiagnostics, VadAutoStopConfig};
use crate::history::{HistoryStorage, RequestModelInfo};
use crate::pipeline::{LlmOutcome, PipelineConfig, PipelineError, PipelineState, SharedPipeline};
use crate::recordings::{RecordingStore, RecordingsStats};
use crate::request_log::RequestLogStore;
use crate::stats::{self, EventStatus};
use crate::commands::history::get_max_saved_recordings;
use tauri::{AppHandle, Manager, State, Emitter};
use chrono::{Duration as ChronoDuration, Utc};
use std::time::{Duration, Instant};

#[cfg(desktop)]
use tauri_plugin_store::StoreExt;

#[derive(Debug, Clone, Copy)]
enum TranscriptionRetentionUnit {
    Days,
    Hours,
}

fn resolve_profile_for_foreground_app(cfg: &PipelineConfig) -> (Option<String>, Option<String>) {
    // Distinguish between:
    // - Unknown foreground app (can't determine): return no profile -> no chip in UI.
    // - Known foreground app but no profile match: return explicit "default".
    #[cfg(desktop)]
    {
        if crate::windows_apps::get_foreground_process_path().is_none() {
            return (None, None);
        }
    }

    #[cfg(not(desktop))]
    {
        // Non-desktop targets cannot reliably resolve a foreground app.
        return (None, None);
    }

    let profile = crate::pipeline::select_profile_for_foreground_app(&cfg.llm_config);

    if let Some(p) = profile {
        return (Some(p.id), Some(p.name));
    }

    // Keep "default" explicit so the UI can always show a chip when desired.
    (Some("default".to_string()), Some("Default".to_string()))
}

fn resolve_profile_by_id(cfg: &PipelineConfig, profile_id: Option<&str>) -> (Option<String>, Option<String>) {
    let Some(profile_id) = profile_id else {
        return (Some("default".to_string()), Some("Default".to_string()));
    };

    if profile_id == "default" {
        return (Some("default".to_string()), Some("Default".to_string()));
    }

    let name = cfg
        .llm_config
        .program_prompt_profiles
        .iter()
        .find(|p| p.id == profile_id)
        .map(|p| p.name.clone());

    (Some(profile_id.to_string()), name)
}

fn get_transcription_retention_duration(app: &AppHandle) -> Option<ChronoDuration> {
    #[cfg(desktop)]
    {
        let store = app.store("settings.json").ok();

        // Ensure we see the latest persisted settings (the store is cached across calls).
        // Best-effort: if load fails, continue with whatever is already in memory.
        if let Some(s) = store.as_ref() {
            let _ = s.reload();
        }

        // New keys: unit + value
        let unit = store
            .as_ref()
            .and_then(|s| s.get("transcription_retention_unit"))
            .and_then(|v| {
                v.as_str().map(|s| match s {
                    "hours" => TranscriptionRetentionUnit::Hours,
                    _ => TranscriptionRetentionUnit::Days,
                })
            });

        let value = store
            .as_ref()
            .and_then(|s| s.get("transcription_retention_value"))
            .and_then(|v| {
                v.as_f64()
                    .or_else(|| v.as_u64().map(|x| x as f64))
                    .or_else(|| v.as_str().and_then(|s| s.parse::<f64>().ok()))
            });

        if let (Some(unit), Some(value)) = (unit, value) {
            // 0 means keep forever.
            if !(value > 0.0) {
                return None;
            }

            match unit {
                TranscriptionRetentionUnit::Days => {
                    // Defensive cap: 0..36500 days (~100 years)
                    let days = value.round().clamp(0.0, 36_500.0) as i64;
                    if days <= 0 {
                        None
                    } else {
                        Some(ChronoDuration::days(days))
                    }
                }
                TranscriptionRetentionUnit::Hours => {
                    // Allow fractional hours (e.g. 0.5)
                    // Defensive cap: ~100 years in hours
                    let hours = value.clamp(0.0, 36_500.0 * 24.0);
                    if !(hours > 0.0) {
                        None
                    } else {
                        let millis = (hours * 3_600_000.0).round() as i64;
                        Some(ChronoDuration::milliseconds(millis))
                    }
                }
            }
        } else {
            // Legacy key: days
            let days = get_transcription_retention_days(app);
            if days == 0 {
                None
            } else {
                Some(ChronoDuration::days(days as i64))
            }
        }
    }

    #[cfg(not(desktop))]
    {
        None
    }
}

fn get_transcription_retention_delete_recordings(app: &AppHandle) -> bool {
    #[cfg(desktop)]
    {
        return app
            .store("settings.json")
            .ok()
            .and_then(|store| store.get("transcription_retention_delete_recordings"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
    }

    #[cfg(not(desktop))]
    {
        false
    }
}

fn get_transcription_retention_days(app: &AppHandle) -> u64 {
    #[cfg(desktop)]
    {
        return app
            .store("settings.json")
            .ok()
            .and_then(|store| store.get("transcription_retention_days"))
            .and_then(|v| v.as_u64().or_else(|| v.as_f64().map(|f| f as u64)))
            .unwrap_or(0u64);
    }

    #[cfg(not(desktop))]
    {
        0u64
    }
}

pub(crate) fn apply_transcription_retention(app: &AppHandle) {
    let Some(retention) = get_transcription_retention_duration(app) else {
        return;
    };

    let cutoff = Utc::now() - retention;
    let delete_recordings = get_transcription_retention_delete_recordings(app);

    let Some(history) = app.try_state::<HistoryStorage>() else {
        return;
    };

    let removed = match history.prune_older_than(cutoff) {
        Ok(ids) => ids,
        Err(e) => {
            log::warn!("Failed to prune history by time retention: {}", e);
            return;
        }
    };

    if removed.is_empty() {
        return;
    }

    if delete_recordings {
        if let Some(store) = app.try_state::<RecordingStore>() {
            for id in removed.iter() {
                // Best-effort: ignore errors and non-existent files.
                let _ = store.delete_wav_if_exists(id);
            }
        }
    }

    let _ = app.emit("history-changed", ());
}

/// Tauri-compatible error type for commands
#[derive(Debug, serde::Serialize)]
pub struct CommandError {
    pub message: String,
    pub error_type: String,
}

impl From<PipelineError> for CommandError {
    fn from(err: PipelineError) -> Self {
        let error_type = match &err {
            PipelineError::AudioCapture(_) => "audio",
            PipelineError::Stt(_) => "stt",
            PipelineError::Llm(_) => "llm",
            PipelineError::NoProvider => "config",
            PipelineError::AlreadyRecording => "state",
            PipelineError::NotRecording => "state",
            PipelineError::Config(_) => "config",
            PipelineError::Lock(_) => "internal",
            PipelineError::Cancelled => "cancelled",
            PipelineError::Timeout(_) => "timeout",
            PipelineError::RecordingTooLarge(_, _) => "size",
        };
        Self {
            message: err.to_string(),
            error_type: error_type.to_string(),
        }
    }
}

impl From<String> for CommandError {
    fn from(message: String) -> Self {
        Self {
            message,
            error_type: "unknown".to_string(),
        }
    }
}

/// Get the absolute path to a saved WAV recording for a given request id.
///
/// Returns `null` when the recording doesn't exist.
#[tauri::command]
pub fn recording_get_wav_path(
    app: AppHandle,
    request_id: String,
) -> Result<Option<String>, CommandError> {
    let store = app
        .try_state::<RecordingStore>()
        .ok_or_else(|| CommandError::from("Recording store not available".to_string()))?;

    let path = store.wav_path_if_exists(&request_id).map_err(CommandError::from)?;
    Ok(path.map(|p| p.to_string_lossy().to_string()))
}

/// Get saved WAV bytes for a given request id as base64.
///
/// Some webviews can fail to play `convertFileSrc` URLs for WAVs if the asset protocol
/// serves an unexpected content-type; base64+Blob playback is a reliable fallback.
///
/// Returns `null` when the recording doesn't exist.
#[tauri::command]
pub fn recording_get_wav_base64(
    app: AppHandle,
    request_id: String,
) -> Result<Option<String>, CommandError> {
    use base64::Engine;

    let store = app
        .try_state::<RecordingStore>()
        .ok_or_else(|| CommandError::from("Recording store not available".to_string()))?;

    // Reuse the same validation / existence semantics.
    let path = store.wav_path_if_exists(&request_id).map_err(CommandError::from)?;
    let Some(_) = path else {
        return Ok(None);
    };

    let wav = store.load_wav(&request_id).map_err(CommandError::from)?;
    let encoded = base64::engine::general_purpose::STANDARD.encode(wav);
    Ok(Some(encoded))
}

/// Delete all saved recordings from disk.
///
/// Returns the number of `.wav` files deleted.
#[tauri::command]
pub fn recordings_delete_all(app: AppHandle) -> Result<u64, CommandError> {
    let store = app
        .try_state::<RecordingStore>()
        .ok_or_else(|| CommandError::from("Recording store not available".to_string()))?;

    let deleted = store.delete_all_wavs().map_err(CommandError::from)?;
    Ok(deleted)
}

/// Open the recordings folder in the OS file manager.
#[tauri::command]
pub fn recordings_open_folder(app: AppHandle) -> Result<(), CommandError> {
    let store = app
        .try_state::<RecordingStore>()
        .ok_or_else(|| CommandError::from("Recording store not available".to_string()))?;

    open::that(store.directory())
        .map_err(|e| CommandError::from(format!("Failed to open recordings folder: {}", e)))
}

/// Total bytes used by saved recordings on disk.
#[tauri::command]
pub fn recordings_get_storage_bytes(app: AppHandle) -> Result<u64, CommandError> {
    let store = app
        .try_state::<RecordingStore>()
        .ok_or_else(|| CommandError::from("Recording store not available".to_string()))?;

    store.total_size_bytes().map_err(CommandError::from)
}

/// Stats about saved recordings (count + total bytes).
#[tauri::command]
pub fn recordings_get_stats(app: AppHandle) -> Result<RecordingsStats, CommandError> {
    let store = app
        .try_state::<RecordingStore>()
        .ok_or_else(|| CommandError::from("Recording store not available".to_string()))?;

    store.stats().map_err(CommandError::from)
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ActiveProfileInfo {
    pub profile_id: Option<String>,
    pub profile_name: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SessionPresetLockInfo {
    pub profile_id: Option<String>,
    pub preset_id: Option<String>,
}

/// Resolve the currently active program profile based on the foreground application.
///
/// This is used by the overlay to show per-program UX (e.g. a one-shot preset lock)
/// without re-implementing OS-specific process detection in the frontend.
#[tauri::command]
pub fn pipeline_get_active_profile_for_foreground_app(
    pipeline: State<'_, SharedPipeline>,
) -> Result<ActiveProfileInfo, CommandError> {
    let config = pipeline.config();
    let (profile_id, profile_name) = resolve_profile_for_foreground_app(&config);
    Ok(ActiveProfileInfo {
        profile_id,
        profile_name,
    })
}

/// Set (or clear) a one-shot, non-persisted preset override.
///
/// The next transcription will prefer this preset over any persisted manual override
/// and over intent routing.
#[tauri::command]
pub fn pipeline_set_session_preset_lock(
    pipeline: State<'_, SharedPipeline>,
    profile_id: Option<String>,
    preset_id: Option<String>,
) -> Result<(), CommandError> {
    pipeline
        .set_session_preset_lock(profile_id, preset_id)
        .map_err(CommandError::from)
}

/// Read the current in-memory session preset lock (without clearing it).
#[tauri::command]
pub fn pipeline_get_session_preset_lock(
    pipeline: State<'_, SharedPipeline>,
) -> Result<SessionPresetLockInfo, CommandError> {
    let lock = pipeline.peek_session_preset_lock();
    Ok(match lock {
        Some((profile_id, preset_id)) => SessionPresetLockInfo {
            profile_id,
            preset_id: Some(preset_id),
        },
        None => SessionPresetLockInfo {
            profile_id: None,
            preset_id: None,
        },
    })
}

/// Start recording audio using the pipeline
#[tauri::command]
pub fn pipeline_start_recording(
    app: AppHandle,
    pipeline: State<'_, SharedPipeline>,
) -> Result<(), CommandError> {
    // Resolve the profile immediately (best-effort) while the user is likely still
    // in the target app, then pin it for this recording session so stop/transcribe
    // isn't impacted by focus stealing from our always-on-top windows.
    //
    // IMPORTANT: Only pin a *real* program profile id. Do NOT pin explicit "default",
    // because that would force the whole request to Default even if matching could
    // have succeeded later.
    let config = pipeline.config();

    #[cfg(desktop)]
    let foreground = crate::windows_apps::get_foreground_process_path();
    #[cfg(not(desktop))]
    let foreground: Option<String> = None;

    let matched_profile = crate::pipeline::select_profile_for_foreground_app(&config.llm_config);
    if let Some(p) = matched_profile.as_ref() {
        let _ = pipeline.set_session_profile_override(Some(p.id.clone()));
    } else {
        let _ = pipeline.set_session_profile_override(None);
    }

    // One-time per recording: log what we saw so we can debug "always Default" reports.
    log::info!(
        "[profile] start_recording foreground={:?} profiles={} matched={}",
        foreground,
        config.llm_config.program_prompt_profiles.len(),
        matched_profile
            .as_ref()
            .map(|p| format!("{} ({})", p.name, p.id))
            .unwrap_or_else(|| "<none>".to_string())
    );

    // Preserve existing semantics for UI chips/logs:
    // - foreground unknown -> None
    // - foreground known but no match -> Default
    // - match -> profile
    let (profile_id, profile_name) = if foreground.is_none() {
        (None, None)
    } else if let Some(p) = matched_profile.as_ref() {
        (Some(p.id.clone()), Some(p.name.clone()))
    } else {
        (Some("default".to_string()), Some("Default".to_string()))
    };

    // Start request logging
    if let Some(log_store) = app.try_state::<RequestLogStore>() {
        log_store.start_request(config.stt_provider.clone(), config.stt_model.clone());
        log_store.with_current(|log| {
            log.profile_id = profile_id;
            log.profile_name = profile_name;
            log.llm_provider = if config.llm_config.enabled {
                Some(config.llm_config.provider.clone())
            } else {
                None
            };
            log.llm_model = config.llm_config.model.clone();
            log.info("Recording started");
        });
    }

    pipeline.start_recording().map_err(|e| {
        // If we fail to start, clear any pinned session profile so it doesn't leak.
        let _ = pipeline.set_session_profile_override(None);
        if let Some(log_store) = app.try_state::<RequestLogStore>() {
            log_store.with_current(|log| {
                log.error(format!("Failed to start recording: {}", e));
                log.complete_error(e.to_string());
            });
            log_store.complete_current();
        }
        CommandError::from(e)
    })?;

    // While recording/transcribing, allow Escape to cancel without triggering transcription.
    #[cfg(desktop)]
    crate::set_escape_cancel_shortcut_enabled(&app, true);

    // Emit event to frontend
    let _ = app.emit("pipeline-recording-started", ());

    Ok(())
}

/// Stop recording and transcribe the audio
#[tauri::command]
pub async fn pipeline_stop_and_transcribe(
    app: AppHandle,
    pipeline: State<'_, SharedPipeline>,
) -> Result<String, CommandError> {
    let max_saved_recordings = get_max_saved_recordings(&app);

    // Ensure Escape-to-cancel is available during the transcription phase.
    #[cfg(desktop)]
    crate::set_escape_cancel_shortcut_enabled(&app, true);

    let config = pipeline.config();
    let (profile_id, profile_name) = resolve_profile_for_foreground_app(&config);

    // Try to capture the active request id for history + persistent audio.
    //
    // In some edge cases (e.g., backend-initiated recordings or unexpected state
    // resets), request logging may not have been started at recording-start.
    // For UX consistency, ensure we still create a request log + history entry.
    let mut active_request_id: Option<String> = app
        .try_state::<RequestLogStore>()
        .and_then(|store| store.with_current(|log| log.id.clone()));

    if active_request_id.is_none() {
        if let Some(log_store) = app.try_state::<RequestLogStore>() {
            let id = log_store.start_request(config.stt_provider.clone(), config.stt_model.clone());
            log_store.with_current(|log| {
                log.profile_id = profile_id.clone();
                log.profile_name = profile_name.clone();
                log.llm_provider = if config.llm_config.enabled {
                    Some(config.llm_config.provider.clone())
                } else {
                    None
                };
                log.llm_model = if config.llm_config.enabled {
                    config.llm_config.model.clone()
                } else {
                    None
                };
                log.warn("Request log was missing at stop; started a new request log entry");
            });
            active_request_id = Some(id);
        }
    }

    // Capture model info for persistence in history.
    // Note: we intentionally start with no profile metadata in history.
    // The overlay window can steal focus during stop, which can cause an incorrect
    // "Default" profile to be recorded here. We'll update the entry once the
    // pipeline actually transitions into Transcribing/Rewriting.
    let model_info = RequestModelInfo {
        stt_provider: Some(config.stt_provider.clone()),
        stt_model: config.stt_model.clone(),
        llm_provider: if config.llm_config.enabled {
            Some(config.llm_config.provider.clone())
        } else {
            None
        },
        llm_model: config.llm_config.model.clone(),
        profile_id: None,
        profile_name: None,
        preset_id: None,
        preset_name: None,
    };

    // Create an in-progress history entry so the History view shows a running request.
    if let Some(req_id) = active_request_id.as_deref() {
        if let Some(history) = app.try_state::<HistoryStorage>() {
            let _ = history.add_request_entry(
                req_id.to_string(),
                model_info,
                max_saved_recordings,
            );
            let _ = app.emit("history-changed", ());
        }
    }

    // Emit transcription started *only if* the pipeline actually enters the
    // Transcribing state.
    //
    // This prevents the overlay from briefly showing "TRANSCRIBING..." when the
    // quiet-audio gate (hallucination protection) decides to skip STT.
    {
        let app_clone = app.clone();
        let pipeline_clone = pipeline.inner().clone();
        let request_id_for_history = active_request_id.clone();
        tauri::async_runtime::spawn(async move {
            let start = Instant::now();
            loop {
                match pipeline_clone.state() {
                    PipelineState::Transcribing | PipelineState::Routing | PipelineState::Rewriting => {
                        // Now that transcription has begun, copy the *actual* profile metadata
                        // from the request log into History.
                        //
                        // Why not resolve from the foreground app here?
                        // On Windows, our always-on-top overlay windows can briefly become the
                        // foreground window during stop/transcribe, which can incorrectly record
                        // the profile as Default. The pipeline writes the chosen profile into the
                        // request log as part of starting transcription.
                        if let Some(req_id) = request_id_for_history.as_deref() {
                            let profile_meta = app_clone
                                .try_state::<RequestLogStore>()
                                .and_then(|store| {
                                    store.with_current(|log| {
                                        (log.profile_id.clone(), log.profile_name.clone())
                                    })
                                });

                            let (pid, pname) = profile_meta.unwrap_or_else(|| {
                                let cfg = pipeline_clone.config();
                                resolve_profile_for_foreground_app(&cfg)
                            });

                            if let Some(history) = app_clone.try_state::<HistoryStorage>() {
                                let _ = history.set_request_profile(req_id, pid, pname);
                                let _ = app_clone.emit("history-changed", ());
                            }
                        }

                        let _ = app_clone.emit("pipeline-transcription-started", ());
                        break;
                    }
                    PipelineState::Idle | PipelineState::Error => {
                        // Quiet-audio skip resets to Idle; errors also shouldn't show
                        // a "transcribing" phase.
                        break;
                    }
                    PipelineState::Recording => {
                        // Still finalizing stop.
                    }
                }

                if start.elapsed() > Duration::from_secs(2) {
                    break;
                }

                tokio::time::sleep(Duration::from_millis(15)).await;
            }
        });
    }

    // Emit routing started once the pipeline actually enters the Routing phase.
    {
        let app_clone = app.clone();
        let pipeline_clone = pipeline.inner().clone();
        tauri::async_runtime::spawn(async move {
            let start = Instant::now();
            loop {
                match pipeline_clone.state() {
                    PipelineState::Routing => {
                        let _ = app_clone.emit("pipeline-routing-started", ());
                        break;
                    }
                    PipelineState::Idle | PipelineState::Error => {
                        break;
                    }
                    PipelineState::Recording | PipelineState::Transcribing | PipelineState::Rewriting => {}
                }

                // Hard stop to avoid a runaway task in pathological cases.
                if start.elapsed() > Duration::from_secs(15 * 60) {
                    break;
                }

                tokio::time::sleep(Duration::from_millis(25)).await;
            }
        });
    }

    // Emit rewriting started once the pipeline actually enters the optional LLM phase.
    //
    // Why not rely on the overlay's `pipeline_get_state` polling?
    // The overlay may be awaiting a long-running `invoke("pipeline_stop_and_transcribe")`,
    // which can prevent intermediate polling updates from being observed. A dedicated event
    // keeps the UI honest about the rewrite duration.
    {
        let app_clone = app.clone();
        let pipeline_clone = pipeline.inner().clone();
        tauri::async_runtime::spawn(async move {
            let start = Instant::now();
            loop {
                match pipeline_clone.state() {
                    PipelineState::Rewriting => {
                        let _ = app_clone.emit("pipeline-rewriting-started", ());
                        break;
                    }
                    PipelineState::Idle | PipelineState::Error => {
                        // No rewrite (disabled/failed early) or pipeline exited.
                        break;
                    }
                    PipelineState::Recording | PipelineState::Transcribing | PipelineState::Routing => {
                        // Not yet.
                    }
                }

                // Hard stop to avoid a runaway task in pathological cases.
                if start.elapsed() > Duration::from_secs(15 * 60) {
                    break;
                }

                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        });
    }

    // Log transcription start
    if let Some(log_store) = app.try_state::<RequestLogStore>() {
        log_store.with_current(|log| {
            log.profile_id = profile_id.clone();
            log.profile_name = profile_name.clone();
            // Important: do not include recording time in the request "Total" duration.
            // The request log is created at recording-start, so we explicitly mark the
            // processing start when transcription begins.
            log.mark_processing_started();
            log.info("Recording stopped, starting transcription");
        });
    }

    let result = match pipeline.stop_and_transcribe_detailed().await {
        Ok(r) => r,
        Err(PipelineError::Cancelled) => {
            // User cancelled (Escape / cancel button). Treat as a normal outcome.
            #[cfg(desktop)]
            crate::set_escape_cancel_shortcut_enabled(&app, false);

            // Best-effort: complete current request as cancelled.
            if let Some(log_store) = app.try_state::<RequestLogStore>() {
                log_store.with_current(|log| {
                    log.warn("Recording cancelled by user");
                    log.complete_cancelled();
                });
                log_store.complete_current();
            }

            let _ = app.emit("pipeline-cancelled", ());
            return Ok(String::new());
        }
        Err(e) => {
            #[cfg(desktop)]
            crate::set_escape_cancel_shortcut_enabled(&app, false);

            let wav_bytes = pipeline.clone_last_wav_bytes();

            if let Some(log_store) = app.try_state::<RequestLogStore>() {
                log_store.with_current(|log| {
                    log.error_with_details(
                        format!("Transcription failed: {}", e),
                        crate::request_log::format_error_chain(&e),
                    );
                    log.complete_error(e.to_string());
                });

                // Persist cost/usage stats (best-effort).
                crate::stats::emit_cost_events_for_current_request(
                    &app,
                    EventStatus::Error,
                    wav_bytes.as_deref(),
                );

                log_store.complete_current();
            }

            // Update history entry with error (keep it visible for retry)
            if let Some(req_id) = active_request_id.as_deref() {
                if let Some(history) = app.try_state::<HistoryStorage>() {
                    let _ = history.complete_request_error(req_id, e.to_string());
                    let _ = app.emit("history-changed", ());
                }
            }

            // Time-based retention (best-effort). Runs only after a transcription attempt.
            apply_transcription_retention(&app);

            // Persist audio for retry (best-effort)
            if let (Some(req_id), Some(store)) = (
                active_request_id.as_deref(),
                app.try_state::<RecordingStore>(),
            ) {
                if let Some(wav) = wav_bytes {
                    if store.save_wav(req_id, &wav).is_ok() {
                        let _ = store.prune_to_max_files(max_saved_recordings);
                    }
                }
            }

            // Emit pipeline-error event with request_id so the overlay can show a retry button.
            let payload = serde_json::json!({
                "message": e.to_string(),
                "request_id": active_request_id.clone(),
            });
            let _ = app.emit("pipeline-error", payload);

            return Err(CommandError::from(e));
        }
    };

    let final_text = result.final_text.clone();

    // Capture WAV bytes once (used for duration + retry persistence + cost).
    let wav_bytes = pipeline.clone_last_wav_bytes();
    let audio_secs_from_wav = wav_bytes.as_deref().and_then(stats::wav_duration_secs);
    let audio_size_bytes = wav_bytes.as_ref().map(|v| v.len());

    // Log success
    if let Some(log_store) = app.try_state::<RequestLogStore>() {
        log_store.with_current(|log| {
            log.raw_transcript = Some(result.stt_text.clone());
            log.formatted_transcript = Some(result.final_text.clone());
            log.stt_duration_ms = Some(result.stt_duration_ms);
            log.llm_duration_ms = result.llm_duration_ms;

            log.llm_outcome = Some(result.llm_outcome.code().to_string());
            log.llm_not_attempted_reason = None;
            log.llm_error_message = None;

            // Useful for stats (and for later UI display).
            let audio_secs = audio_secs_from_wav.or_else(|| {
                if log.stt_provider == "openai" {
                    log.stt_response_json
                        .as_ref()
                        .and_then(stats::parse_openai_stt_duration_secs_from_response_json)
                } else {
                    None
                }
            });

            log.audio_duration_secs = audio_secs.map(|s| s as f32);
            log.audio_size_bytes = audio_size_bytes;

            // Use the provider instance's model (includes provider defaults) so the UI can show
            // the real model used even if no explicit model override was configured.
            if result.llm_attempted() {
                log.llm_provider = result.llm_provider_used.clone();
                log.llm_model = result.llm_model_used.clone();
            } else {
                // Avoid misleading UI chips: if we didn't attempt LLM formatting,
                // clear any pre-populated provider/model values.
                log.llm_provider = None;
                log.llm_model = None;
            }

            log.info(format!(
                "STT completed in {}ms ({} chars)",
                result.stt_duration_ms,
                result.stt_text.len()
            ));

            match &result.llm_outcome {
                LlmOutcome::NotAttempted(reason) => {
                    log.llm_not_attempted_reason = Some(reason.code().to_string());
                    if let crate::pipeline::LlmNotAttemptedReason::ProviderUnavailable { .. } = reason {
                        log.llm_error_message = Some(reason.to_log_details());
                    }
                    log.info_with_details("LLM formatting not attempted", reason.to_log_details());
                }
                LlmOutcome::Succeeded => {
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
                LlmOutcome::TimedOut => {
                    if let Some(ms) = result.llm_duration_ms {
                        log.warn(format!(
                            "LLM formatting timed out after {}ms; fell back to STT transcript",
                            ms
                        ));
                    } else {
                        log.warn("LLM formatting timed out; fell back to STT transcript");
                    }
                }
                LlmOutcome::Failed(err) => {
                    log.llm_error_message = Some(err.clone());
                    log.warn(format!(
                        "LLM formatting failed; fell back to STT transcript ({})",
                        err
                    ));
                }
            }

            log.complete_success();
        });

        // Persist cost/usage stats (best-effort).
        crate::stats::emit_cost_events_for_current_request(
            &app,
            EventStatus::Success,
            wav_bytes.as_deref(),
        );

        // Persist preset metadata into History (best-effort).
        if let Some(req_id) = active_request_id.as_deref() {
            let preset_meta = log_store.with_current(|log| {
                (log.preset_id.clone(), log.preset_name.clone())
            });
            if let Some((preset_id, preset_name)) = preset_meta {
                if let Some(history) = app.try_state::<HistoryStorage>() {
                    let _ = history.set_request_preset(
                        req_id,
                        preset_id,
                        preset_name,
                    );
                    let _ = app.emit("history-changed", ());
                }
            }
        }

        // Persist the *actual* LLM provider/model used (or clear it if not attempted).
        if let Some(req_id) = active_request_id.as_deref() {
            if let Some(history) = app.try_state::<HistoryStorage>() {
                let (provider, model) = if result.llm_attempted() {
                    (result.llm_provider_used.clone(), result.llm_model_used.clone())
                } else {
                    (None, None)
                };
                let _ = history.set_request_llm_model(req_id, provider, model);
                let _ = app.emit("history-changed", ());
            }
        }

        log_store.complete_current();
    }

    // Persist audio for retry (best-effort)
    if let (Some(req_id), Some(store)) = (
        active_request_id.as_deref(),
        app.try_state::<RecordingStore>(),
    ) {
        if let Some(wav) = wav_bytes {
            if store.save_wav(req_id, &wav).is_ok() {
                let _ = store.prune_to_max_files(max_saved_recordings);

                // Mark that this history entry has a recording available (it is stored under req_id).
                if let Some(history) = app.try_state::<HistoryStorage>() {
                    let _ = history.set_request_recording_id(req_id, Some(req_id.to_string()));
                    let _ = app.emit("history-changed", ());
                }
            }
        }
    }

    // Update history entry with success text
    if let Some(req_id) = active_request_id.as_deref() {
        if let Some(history) = app.try_state::<HistoryStorage>() {
            let _ = history.complete_request_success(req_id, final_text.clone());

            let (provider, model) = if result.llm_attempted() {
                (result.llm_provider_used.clone(), result.llm_model_used.clone())
            } else {
                (None, None)
            };
            let _ = history.set_request_llm_model(req_id, provider, model);
            let _ = app.emit("history-changed", ());
        }
    }

    // Time-based retention (best-effort). Runs only after a transcription attempt.
    apply_transcription_retention(&app);

    // Emit transcript ready event
    let _ = app.emit("pipeline-transcript-ready", &final_text);

    // Done transcribing - stop stealing Escape.
    #[cfg(desktop)]
    crate::set_escape_cancel_shortcut_enabled(&app, false);

    Ok(final_text)
}

/// Retry transcription for a prior request id.
///
/// Loads the saved WAV (if available), creates a new request log + history entry,
/// and re-runs STT + optional LLM formatting.
#[tauri::command]
pub async fn pipeline_retry_transcription(
    app: AppHandle,
    pipeline: State<'_, SharedPipeline>,
    request_id: String,
) -> Result<String, CommandError> {
    pipeline_retry_transcription_impl(app, pipeline.inner().clone(), request_id).await
}

/// Implementation for retry transcription that can be called both from the Tauri command
/// and from internal shortcut handlers.
pub(crate) async fn pipeline_retry_transcription_impl(
    app: AppHandle,
    pipeline: SharedPipeline,
    request_id: String,
) -> Result<String, CommandError> {
    let max_saved_recordings = get_max_saved_recordings(&app);

    // Allow Escape-to-cancel while the retry transcription is running.
    #[cfg(desktop)]
    crate::set_escape_cancel_shortcut_enabled(&app, true);

    let recording_store = app
        .try_state::<RecordingStore>()
        .ok_or_else(|| CommandError::from("Recording store not available".to_string()))?;

    let original_entry = app
        .try_state::<HistoryStorage>()
        .and_then(|history| history.get_by_id(&request_id).ok().flatten());

    // Resolve which request id actually owns the recording.
    // - For normal requests, this is the same as `request_id`.
    // - For reruns (including failed reruns), the entry should point back to the original.
    let recording_source_id: String = original_entry
        .as_ref()
        .and_then(|entry| entry.recording_request_id.clone())
        .unwrap_or_else(|| request_id.clone());

    // Preserve the preset used by the original request (if we can find it).
    // This was added after presets existed, but retry transcription historically did not
    // carry preset selection forward.
    let original_preset_id: Option<String> = original_entry.as_ref().and_then(|e| e.preset_id.clone());
    let original_preset_name: Option<String> = original_entry
        .as_ref()
        .and_then(|e| e.preset_name.clone());

    let wav = recording_store
        .load_wav(&recording_source_id)
        .map_err(CommandError::from)?;

    // Start a *new* request log for the retry attempt.
    let config = pipeline.config();
    // Use the same profile as the original request (if we can find it).
    // This avoids retry accidentally using Default just because the foreground app changed.
    let original_profile_id: Option<String> = original_entry.as_ref().and_then(|e| e.profile_id.clone());

    // Preserve "unknown" as None so the UI doesn't show a Default chip unless it was
    // explicitly recorded as default on the original entry.
    let (profile_id, profile_name) = if original_profile_id.is_none() {
        (None, None)
    } else {
        resolve_profile_by_id(&config, original_profile_id.as_deref())
    };

    let new_request_id: Option<String> = app.try_state::<RequestLogStore>().map(|log_store| {
        log_store.start_request(config.stt_provider.clone(), config.stt_model.clone())
    });

    if let Some(log_store) = app.try_state::<RequestLogStore>() {
        log_store.with_current(|log| {
            log.profile_id = profile_id.clone();
            log.profile_name = profile_name.clone();
            // Seed preset metadata up-front so UI/logs can reflect intent immediately.
            // The pipeline will still persist the *effective* preset selected during routing.
            log.preset_id = original_preset_id.clone();
            log.preset_name = original_preset_name.clone();
        });
    }

    // Capture model info for persistence in history.
    let model_info = RequestModelInfo {
        stt_provider: Some(config.stt_provider.clone()),
        stt_model: config.stt_model.clone(),
        llm_provider: if config.llm_config.enabled {
            Some(config.llm_config.provider.clone())
        } else {
            None
        },
        llm_model: config.llm_config.model.clone(),
        profile_id: profile_id.clone(),
        profile_name: profile_name.clone(),
        preset_id: original_preset_id.clone(),
        preset_name: original_preset_name.clone(),
    };

    // Create a history entry for the retry attempt.
    if let Some(req_id) = new_request_id.as_deref() {
        if let Some(history) = app.try_state::<HistoryStorage>() {
            let _ = history.add_request_entry(req_id.to_string(), model_info, max_saved_recordings);

            // Ensure play/rerun for this new entry points at the original recording.
            let _ = history.set_request_recording_id(req_id, Some(recording_source_id.clone()));

            let _ = app.emit("history-changed", ());
        }
    }

    let _ = app.emit("pipeline-transcription-started", ());

    // If we know the original preset, set a one-shot session lock so the retry uses the
    // same preset as the entry being rerun (instead of whatever the profile/router would
    // pick right now).
    //
    // This lock is normally consumed (take+clear) by the pipeline once it reaches routing,
    // but we also defensively clear it on drop so early STT failures don't leak the lock
    // into the next transcription attempt.
    struct ClearPresetLockOnDrop {
        pipeline: SharedPipeline,
    }
    impl Drop for ClearPresetLockOnDrop {
        fn drop(&mut self) {
            let _ = self.pipeline.set_session_preset_lock(None, None);
        }
    }

    let _preset_lock_guard = if original_preset_id.is_some() {
        let _ = pipeline.set_session_preset_lock(profile_id.clone(), original_preset_id.clone());
        Some(ClearPresetLockOnDrop {
            pipeline: pipeline.clone(),
        })
    } else {
        None
    };

    // Emit rewriting started once we enter the optional LLM phase.
    {
        let app_clone = app.clone();
        let pipeline_clone = pipeline.clone();
        tauri::async_runtime::spawn(async move {
            let start = Instant::now();
            loop {
                match pipeline_clone.state() {
                    PipelineState::Rewriting => {
                        let _ = app_clone.emit("pipeline-rewriting-started", ());
                        break;
                    }
                    PipelineState::Idle | PipelineState::Error => {
                        break;
                    }
                    PipelineState::Recording | PipelineState::Transcribing | PipelineState::Routing => {}
                }

                if start.elapsed() > Duration::from_secs(15 * 60) {
                    break;
                }

                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        });
    }

    // Emit routing started once we enter the Routing phase.
    {
        let app_clone = app.clone();
        let pipeline_clone = pipeline.clone();
        tauri::async_runtime::spawn(async move {
            let start = Instant::now();
            loop {
                match pipeline_clone.state() {
                    PipelineState::Routing => {
                        let _ = app_clone.emit("pipeline-routing-started", ());
                        break;
                    }
                    PipelineState::Idle | PipelineState::Error => {
                        break;
                    }
                    PipelineState::Recording | PipelineState::Transcribing | PipelineState::Rewriting => {}
                }

                if start.elapsed() > Duration::from_secs(15 * 60) {
                    break;
                }

                tokio::time::sleep(Duration::from_millis(25)).await;
            }
        });
    }

    // Run the retry transcription (STT + optional LLM)
    let result = match pipeline
        .transcribe_wav_bytes_detailed_for_profile(wav.clone(), profile_id.as_deref())
        .await
    {
        Ok(r) => r,
        Err(PipelineError::Cancelled) => {
            #[cfg(desktop)]
            crate::set_escape_cancel_shortcut_enabled(&app, false);
            let _ = app.emit("pipeline-cancelled", ());
            return Ok(String::new());
        }
        Err(e) => {
            #[cfg(desktop)]
            crate::set_escape_cancel_shortcut_enabled(&app, false);

            if let Some(log_store) = app.try_state::<RequestLogStore>() {
                log_store.with_current(|log| {
                    log.error_with_details(
                        format!("Retry transcription failed: {}", e),
                        crate::request_log::format_error_chain(&e),
                    );
                    log.complete_error(e.to_string());
                });

                // Persist cost/usage stats (best-effort).
                crate::stats::emit_cost_events_for_current_request(
                    &app,
                    EventStatus::Error,
                    Some(wav.as_slice()),
                );

                log_store.complete_current();
            }

            if let Some(req_id) = new_request_id.as_deref() {
                if let Some(history) = app.try_state::<HistoryStorage>() {
                    let _ = history.complete_request_error(req_id, e.to_string());
                    let _ = app.emit("history-changed", ());
                }
            }

            // Also emit pipeline-error so the overlay can present the always-on-top retry UI.
            let payload = serde_json::json!({
                "message": e.to_string(),
                "request_id": new_request_id,
            });
            let _ = app.emit("pipeline-error", payload);

            return Err(CommandError::from(e));
        }
    };

    // IMPORTANT: Do NOT copy the WAV under the new request id.
    // Reruns always point back to the original recording via `recording_request_id`.

    let final_text = result.final_text.clone();

    // Update log store on success
    if let Some(log_store) = app.try_state::<RequestLogStore>() {
        log_store.with_current(|log| {
            log.raw_transcript = Some(result.stt_text.clone());
            log.formatted_transcript = Some(result.final_text.clone());
            log.stt_duration_ms = Some(result.stt_duration_ms);
            log.llm_duration_ms = result.llm_duration_ms;

            log.llm_outcome = Some(result.llm_outcome.code().to_string());
            log.llm_not_attempted_reason = None;
            log.llm_error_message = None;

            // Useful for stats (and for later UI display).
            log.audio_duration_secs = stats::wav_duration_secs(wav.as_slice()).map(|s| s as f32);
            log.audio_size_bytes = Some(wav.len());

            if result.llm_attempted() {
                log.llm_provider = result.llm_provider_used.clone();
                log.llm_model = result.llm_model_used.clone();
            } else {
                // Avoid misleading UI chips: clear any pre-populated provider/model values.
                log.llm_provider = None;
                log.llm_model = None;
            }

            log.info(format!(
                "Retry STT completed in {}ms ({} chars)",
                result.stt_duration_ms,
                result.stt_text.len()
            ));
            log.complete_success();
        });

        // Persist cost/usage stats (best-effort).
        crate::stats::emit_cost_events_for_current_request(
            &app,
            EventStatus::Success,
            Some(wav.as_slice()),
        );

        // Persist preset metadata into History (best-effort).
        if let Some(req_id) = new_request_id.as_deref() {
            let preset_meta =
                log_store.with_current(|log| (log.preset_id.clone(), log.preset_name.clone()));
            if let Some((preset_id, preset_name)) = preset_meta {
                if let Some(history) = app.try_state::<HistoryStorage>() {
                    let _ = history.set_request_preset(req_id, preset_id, preset_name);
                    let _ = app.emit("history-changed", ());
                }
            }
        }

        log_store.complete_current();
    }

    // Update history on success
    if let Some(req_id) = new_request_id.as_deref() {
        if let Some(history) = app.try_state::<HistoryStorage>() {
            let _ = history.complete_request_success(req_id, final_text.clone());

            let (provider, model) = if result.llm_attempted() {
                (result.llm_provider_used.clone(), result.llm_model_used.clone())
            } else {
                (None, None)
            };
            let _ = history.set_request_llm_model(req_id, provider, model);
            let _ = app.emit("history-changed", ());
        }
    }

    // Emit transcript ready event
    let _ = app.emit("pipeline-transcript-ready", &final_text);

    #[cfg(desktop)]
    crate::set_escape_cancel_shortcut_enabled(&app, false);

    Ok(final_text)
}

/// Cancel the current recording/transcription
#[tauri::command]
pub fn pipeline_cancel(
    app: AppHandle,
    pipeline: State<'_, SharedPipeline>,
) -> Result<(), CommandError> {
    // `pipeline` is kept for API stability; on desktop we delegate to a helper that
    // re-acquires the shared pipeline from app state.
    let _ = pipeline;

    #[cfg(desktop)]
    {
        // Reuse the centralized cancel logic so audio mute/pause state is restored too.
        crate::cancel_pipeline_session(&app, "Command");
        return Ok(());
    }

    #[cfg(not(desktop))]
    {
        // Log cancellation
        if let Some(log_store) = app.try_state::<RequestLogStore>() {
            log_store.with_current(|log| {
                log.warn("Recording cancelled by user");
                log.complete_cancelled();
            });
            log_store.complete_current();
        }

        pipeline.cancel();

        // Emit cancelled event
        let _ = app.emit("pipeline-cancelled", ());

        Ok(())
    }
}

/// Get the current pipeline state
#[tauri::command]
pub fn pipeline_get_state(
    pipeline: State<'_, SharedPipeline>,
) -> Result<String, CommandError> {
    let state = pipeline.state();
    let state_str = match state {
        PipelineState::Idle => "idle",
        PipelineState::Recording => "recording",
        PipelineState::Routing => "routing",
        PipelineState::Transcribing => "transcribing",
        PipelineState::Rewriting => "rewriting",
        PipelineState::Error => "error",
    };
    Ok(state_str.to_string())
}

/// Check if the pipeline is currently recording
#[tauri::command]
pub fn pipeline_is_recording(
    pipeline: State<'_, SharedPipeline>,
) -> Result<bool, CommandError> {
    Ok(pipeline.is_recording())
}

/// Configuration payload for updating the pipeline
#[derive(Debug, serde::Deserialize)]
pub struct PipelineConfigPayload {
    pub stt_provider: Option<String>,
    pub stt_api_key: Option<String>,
    pub stt_model: Option<String>,
    pub max_duration_secs: Option<f32>,
    pub max_retries: Option<u32>,
    pub vad_enabled: Option<bool>,
    pub vad_auto_stop: Option<bool>,
    /// Timeout in seconds for transcription requests
    pub transcription_timeout_secs: Option<u64>,
    /// Maximum recording size in bytes
    pub max_recording_bytes: Option<usize>,
}

/// Update the pipeline configuration
#[tauri::command]
pub fn pipeline_update_config(
    pipeline: State<'_, SharedPipeline>,
    config: PipelineConfigPayload,
) -> Result<(), CommandError> {
    use std::collections::HashMap;

    let mut retry_config = crate::stt::RetryConfig::default();
    if let Some(max_retries) = config.max_retries {
        retry_config.max_retries = max_retries;
    }

    // Build VAD config from payload if any VAD settings provided
    let vad_config = VadAutoStopConfig {
        enabled: config.vad_enabled.unwrap_or(false),
        auto_stop: config.vad_auto_stop.unwrap_or(false),
        ..VadAutoStopConfig::default()
    };

    let mut new_config = PipelineConfig::default();
    new_config.stt_provider = config.stt_provider.unwrap_or_else(|| "groq".to_string());
    new_config.stt_api_key = config.stt_api_key.unwrap_or_default();
    new_config.stt_api_keys = HashMap::new();
    new_config.stt_model = config.stt_model;
    new_config.max_duration_secs = config.max_duration_secs.unwrap_or(300.0);
    new_config.retry_config = retry_config;
    new_config.vad_config = vad_config;
    new_config.transcription_timeout = config
        .transcription_timeout_secs
        .map(Duration::from_secs)
        .unwrap_or(Duration::from_secs(60));
    new_config.max_recording_bytes = config.max_recording_bytes.unwrap_or(50 * 1024 * 1024);
    new_config.llm_config = crate::llm::LlmConfig::default();
    new_config.llm_api_keys = HashMap::new();

    pipeline.update_config(new_config).map_err(CommandError::from)?;
    log::info!("Pipeline configuration updated");

    Ok(())
}

/// Stop recording, transcribe, and type the result
/// This is the main end-to-end command for voice dictation
#[tauri::command]
pub async fn pipeline_dictate(
    app: AppHandle,
    pipeline: State<'_, SharedPipeline>,
) -> Result<String, CommandError> {
    let max_saved_recordings = get_max_saved_recordings(&app);

    // Ensure Escape-to-cancel remains available while we transcribe.
    #[cfg(desktop)]
    crate::set_escape_cancel_shortcut_enabled(&app, true);

    let cfg = pipeline.config();
    let (profile_id, profile_name) = resolve_profile_for_foreground_app(&cfg);

    // Ensure there is a request log (pipeline_toggle starts one on recording-start,
    // but other flows can reach here without an active log).
    let mut active_request_id: Option<String> = app
        .try_state::<RequestLogStore>()
        .and_then(|store| store.with_current(|log| log.id.clone()));

    if active_request_id.is_none() {
        if let Some(log_store) = app.try_state::<RequestLogStore>() {
            let id = log_store.start_request(cfg.stt_provider.clone(), cfg.stt_model.clone());
            log_store.with_current(|log| {
                log.profile_id = profile_id.clone();
                log.profile_name = profile_name.clone();
                log.llm_provider = if cfg.llm_config.enabled {
                    Some(cfg.llm_config.provider.clone())
                } else {
                    None
                };
                log.llm_model = if cfg.llm_config.enabled {
                    cfg.llm_config.model.clone()
                } else {
                    None
                };
                log.warn("Request log was missing at dictate; started a new request log entry");
            });
            active_request_id = Some(id);
        }
    }

    // Capture model info for persistence in history.
    let model_info = RequestModelInfo {
        stt_provider: Some(cfg.stt_provider.clone()),
        stt_model: cfg.stt_model.clone(),
        llm_provider: if cfg.llm_config.enabled {
            Some(cfg.llm_config.provider.clone())
        } else {
            None
        },
        llm_model: cfg.llm_config.model.clone(),
        profile_id: profile_id.clone(),
        profile_name: profile_name.clone(),
        preset_id: None,
        preset_name: None,
    };

    // Create an in-progress history entry so the History view shows a running request.
    if let Some(req_id) = active_request_id.as_deref() {
        if let Some(history) = app.try_state::<HistoryStorage>() {
            let _ = history.add_request_entry(req_id.to_string(), model_info, max_saved_recordings);
            let _ = app.emit("history-changed", ());
        }
    }

    // Log transcription start
    if let Some(log_store) = app.try_state::<RequestLogStore>() {
        log_store.with_current(|log| {
            log.profile_id = profile_id.clone();
            log.profile_name = profile_name.clone();
            // Do not include recording time in the request "Total" duration.
            log.mark_processing_started();
            log.info("Recording stopped, starting transcription");
        });
    }

    // Emit transcription started *only if* the pipeline actually enters the
    // Transcribing state (avoid flashing "TRANSCRIBING..." on quiet-audio skips).
    {
        let app_clone = app.clone();
        let pipeline_clone = pipeline.inner().clone();
        tauri::async_runtime::spawn(async move {
            let start = Instant::now();
            loop {
                match pipeline_clone.state() {
                    PipelineState::Transcribing | PipelineState::Routing | PipelineState::Rewriting => {
                        let _ = app_clone.emit("pipeline-transcription-started", ());
                        break;
                    }
                    PipelineState::Idle | PipelineState::Error => {
                        break;
                    }
                    PipelineState::Recording => {}
                }

                if start.elapsed() > Duration::from_secs(2) {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(15)).await;
            }
        });
    }

    // Emit routing started once we enter the Routing phase.
    {
        let app_clone = app.clone();
        let pipeline_clone = pipeline.inner().clone();
        tauri::async_runtime::spawn(async move {
            let start = Instant::now();
            loop {
                match pipeline_clone.state() {
                    PipelineState::Routing => {
                        let _ = app_clone.emit("pipeline-routing-started", ());
                        break;
                    }
                    PipelineState::Idle | PipelineState::Error => {
                        break;
                    }
                    PipelineState::Recording | PipelineState::Transcribing | PipelineState::Rewriting => {}
                }

                if start.elapsed() > Duration::from_secs(15 * 60) {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
        });
    }

    let result = match pipeline.stop_and_transcribe_detailed().await {
        Ok(r) => r,
        Err(PipelineError::Cancelled) => {
            #[cfg(desktop)]
            crate::set_escape_cancel_shortcut_enabled(&app, false);
            let _ = app.emit("pipeline-cancelled", ());

            // Best-effort: mark request as cancelled in logs + history.
            if let Some(log_store) = app.try_state::<RequestLogStore>() {
                log_store.with_current(|log| {
                    log.warn("Recording cancelled by user");
                    log.complete_cancelled();
                });
                log_store.complete_current();
            }

            if let Some(req_id) = active_request_id.as_deref() {
                if let Some(history) = app.try_state::<HistoryStorage>() {
                    let _ = history.complete_request_error(req_id, "Cancelled".to_string());
                    let _ = app.emit("history-changed", ());
                }
            }

            return Ok(String::new());
        }
        Err(e) => {
            #[cfg(desktop)]
            crate::set_escape_cancel_shortcut_enabled(&app, false);

            // Best-effort: persist cost/usage stats even when dictate fails.
            // (This flow is commonly used by the global hotkey / toggle path.)
            let wav_bytes = pipeline.clone_last_wav_bytes();
            crate::stats::emit_cost_events_for_current_request(&app, EventStatus::Error, wav_bytes.as_deref());

            if let Some(log_store) = app.try_state::<RequestLogStore>() {
                log_store.with_current(|log| {
                    log.error_with_details(
                        format!("Transcription failed: {}", e),
                        crate::request_log::format_error_chain(&e),
                    );
                    log.complete_error(e.to_string());
                });
                log_store.complete_current();
            }

            // Persist audio for retry (best-effort)
            if let (Some(req_id), Some(store)) = (
                active_request_id.as_deref(),
                app.try_state::<RecordingStore>(),
            ) {
                if let Some(wav) = wav_bytes {
                    if store.save_wav(req_id, &wav).is_ok() {
                        let _ = store.prune_to_max_files(max_saved_recordings);
                    } else if let Some(log_store) = app.try_state::<RequestLogStore>() {
                        log_store.with_current(|log| {
                            log.warn("Failed to persist audio for retry");
                        });
                    }
                }
            }

            // Update history entry with error (keep it visible for retry)
            if let Some(req_id) = active_request_id.as_deref() {
                if let Some(history) = app.try_state::<HistoryStorage>() {
                    let _ = history.complete_request_error(req_id, e.to_string());
                    let _ = app.emit("history-changed", ());
                }
            }

            // Time-based retention (best-effort). Still apply even on failures.
            apply_transcription_retention(&app);

            // Emit pipeline-error event with request_id so the overlay can show a retry button.
            let payload = serde_json::json!({
                "message": e.to_string(),
                "request_id": active_request_id.clone(),
            });
            let _ = app.emit("pipeline-error", payload);

            return Err(CommandError::from(e));
        }
    };

    let final_text = result.final_text.clone();

    // Capture WAV bytes once (used for duration + cost).
    let wav_bytes = pipeline.clone_last_wav_bytes();

    // Emit transcript ready event
    let _ = app.emit("pipeline-transcript-ready", &final_text);

    // Type the transcript
    if !final_text.is_empty() {
        if let Some(log_store) = app.try_state::<RequestLogStore>() {
            log_store.with_current(|log| {
                log.info("Typing transcript...");
            });
        }

        crate::commands::text::type_text(app.clone(), final_text.clone())
            .await
            .map_err(|e| {
                if let Some(log_store) = app.try_state::<RequestLogStore>() {
                    log_store.with_current(|log| {
                        log.error(format!("Failed to type text: {}", e));
                    });
                }
                CommandError::from(e)
            })?;
    }

    // Log success
    if let Some(log_store) = app.try_state::<RequestLogStore>() {
        log_store.with_current(|log| {
            log.raw_transcript = Some(result.stt_text.clone());
            log.formatted_transcript = Some(result.final_text.clone());
            log.stt_duration_ms = Some(result.stt_duration_ms);
            log.llm_duration_ms = result.llm_duration_ms;

            if result.llm_attempted() {
                log.llm_provider = result.llm_provider_used.clone();
                log.llm_model = result.llm_model_used.clone();
            } else {
                // Avoid misleading UI chips: clear any pre-populated provider/model values.
                log.llm_provider = None;
                log.llm_model = None;
            }

            log.info(format!(
                "STT completed in {}ms ({} chars)",
                result.stt_duration_ms,
                result.stt_text.len()
            ));

            match &result.llm_outcome {
                LlmOutcome::NotAttempted(reason) => {
                    log.llm_not_attempted_reason = Some(reason.code().to_string());
                    if let crate::pipeline::LlmNotAttemptedReason::ProviderUnavailable { .. } = reason {
                        log.llm_error_message = Some(reason.to_log_details());
                    }
                    log.info_with_details("LLM formatting not attempted", reason.to_log_details());
                }
                LlmOutcome::Succeeded => {
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
                LlmOutcome::TimedOut => {
                    if let Some(ms) = result.llm_duration_ms {
                        log.warn(format!(
                            "LLM formatting timed out after {}ms; fell back to STT transcript",
                            ms
                        ));
                    } else {
                        log.warn("LLM formatting timed out; fell back to STT transcript");
                    }
                }
                LlmOutcome::Failed(err) => {
                    log.llm_error_message = Some(err.clone());
                    log.warn(format!(
                        "LLM formatting failed; fell back to STT transcript ({})",
                        err
                    ));
                }
            }

            log.complete_success();
        });

        // Persist cost/usage stats (best-effort).
        // NOTE: `pipeline_stop_and_transcribe` and `pipeline_retry_transcription` already do this,
        // but `pipeline_dictate` is a separate flow used by hotkeys and should also be tracked.
        crate::stats::emit_cost_events_for_current_request(&app, EventStatus::Success, wav_bytes.as_deref());

        // Persist preset metadata into History (best-effort).
        if let Some(req_id) = active_request_id.as_deref() {
            let preset_meta = log_store.with_current(|log| {
                (log.preset_id.clone(), log.preset_name.clone())
            });
            if let Some((preset_id, preset_name)) = preset_meta {
                if let Some(history) = app.try_state::<HistoryStorage>() {
                    let _ = history.set_request_preset(req_id, preset_id, preset_name);
                    let _ = app.emit("history-changed", ());
                }
            }
        }

        log_store.complete_current();
    }

    // Persist audio for retry/playback (best-effort)
    if let (Some(req_id), Some(store)) = (
        active_request_id.as_deref(),
        app.try_state::<RecordingStore>(),
    ) {
        if let Some(wav) = wav_bytes.clone() {
            if store.save_wav(req_id, &wav).is_ok() {
                let _ = store.prune_to_max_files(max_saved_recordings);
            }
        }
    }

    // Update history entry with success text
    if let Some(req_id) = active_request_id.as_deref() {
        if let Some(history) = app.try_state::<HistoryStorage>() {
            let _ = history.complete_request_success(req_id, final_text.clone());

            let (provider, model) = if result.llm_attempted() {
                (result.llm_provider_used.clone(), result.llm_model_used.clone())
            } else {
                (None, None)
            };
            let _ = history.set_request_llm_model(req_id, provider, model);
            let _ = app.emit("history-changed", ());
        }
    }

    // Time-based retention (best-effort). Runs only after a transcription attempt.
    apply_transcription_retention(&app);

    #[cfg(desktop)]
    crate::set_escape_cancel_shortcut_enabled(&app, false);

    Ok(final_text)
}

/// Test transcription using the last captured audio (WAV bytes).
///
/// This is primarily used by the settings UI to validate STT provider/model.
#[tauri::command]
pub async fn pipeline_test_transcribe_last_audio(
    app: AppHandle,
    pipeline: State<'_, SharedPipeline>,
    profile_id: Option<String>,
) -> Result<String, CommandError> {
    // Create a dedicated request-log entry for this test action.
    // This is important because it is a standalone STT call (no recording/start step).
    let stt_started_at = Instant::now();

    if let Some(log_store) = app.try_state::<RequestLogStore>() {
        let cfg = pipeline.config();

        let (profile_id_used, profile_name_used) =
            resolve_profile_by_id(&cfg, profile_id.as_deref());

        // Best-effort: pick the *desired* provider/model based on profile overrides.
        // The pipeline may still fall back to global provider/model if overrides are invalid.
        let (desired_provider, desired_model) = profile_id
            .as_deref()
            .and_then(|id| {
                if id == "default" {
                    None
                } else {
                    cfg.llm_config
                        .program_prompt_profiles
                        .iter()
                        .find(|p| p.id == id)
                }
            })
            .map(|p| {
                (
                    p.stt_provider
                        .clone()
                        .unwrap_or_else(|| cfg.stt_provider.clone()),
                    p.stt_model.clone().or_else(|| cfg.stt_model.clone()),
                )
            })
            .unwrap_or_else(|| (cfg.stt_provider.clone(), cfg.stt_model.clone()));

        log_store.start_request(desired_provider, desired_model);
        log_store.with_current(|log| {
            log.profile_id = profile_id_used;
            log.profile_name = profile_name_used;
            log.llm_provider = None;
            log.llm_model = None;
            log.info("Test transcription started");
        });
    }

    // Attempt transcription and persist cost events centrally.
    let res = pipeline
        .transcribe_last_audio_for_profile(profile_id.as_deref())
        .await;

    match res {
        Ok(s) => {
            let wav = pipeline.clone_last_wav_bytes();

            if let Some(log_store) = app.try_state::<RequestLogStore>() {
                let stt_duration_ms = stt_started_at.elapsed().as_millis() as u64;
                log_store.with_current(|log| {
                    log.audio_size_bytes = wav.as_ref().map(|b| b.len());
                    log.raw_transcript = Some(s.clone());
                    log.formatted_transcript = Some(s.clone());
                    log.stt_duration_ms = Some(stt_duration_ms);
                    log.info(format!(
                        "Test transcription completed in {}ms ({} chars)",
                        stt_duration_ms,
                        s.len()
                    ));
                    log.complete_success();
                });

                // Best-effort: emit cost events using the last WAV bytes.
                crate::stats::emit_cost_events_for_current_request(
                    &app,
                    EventStatus::Success,
                    wav.as_deref(),
                );

                log_store.complete_current();
            } else {
                crate::stats::emit_cost_events_for_current_request(
                    &app,
                    EventStatus::Success,
                    wav.as_deref(),
                );
            }

            Ok(s)
        }
        Err(e) => {
            let wav = pipeline.clone_last_wav_bytes();

            if let Some(log_store) = app.try_state::<RequestLogStore>() {
                let stt_duration_ms = stt_started_at.elapsed().as_millis() as u64;
                log_store.with_current(|log| {
                    log.audio_size_bytes = wav.as_ref().map(|b| b.len());
                    log.stt_duration_ms = Some(stt_duration_ms);
                    log.error_with_details(
                        format!("Test transcription failed: {}", e),
                        crate::request_log::format_error_chain(&e),
                    );
                    log.complete_error(e.to_string());
                });

                crate::stats::emit_cost_events_for_current_request(
                    &app,
                    EventStatus::Error,
                    wav.as_deref(),
                );

                log_store.complete_current();
            } else {
                crate::stats::emit_cost_events_for_current_request(
                    &app,
                    EventStatus::Error,
                    wav.as_deref(),
                );
            }

            Err(CommandError::from(e))
        }
    }
}

/// Whether there is a previously captured audio buffer available for STT testing.
#[tauri::command]
pub fn pipeline_has_last_audio(pipeline: State<'_, SharedPipeline>) -> Result<bool, CommandError> {
    Ok(pipeline.has_last_audio())
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct AudioSettingsTestWavs {
    pub raw_wav_base64: String,
    pub processed_wav_base64: String,
}

/// Start a recording intended for audio settings A/B testing.
///
/// This records raw audio (no filters applied during capture). The stop command will
/// return both the raw encode and the processed encode using current settings.
#[tauri::command]
pub fn pipeline_test_audio_settings_start_recording(
    pipeline: State<'_, SharedPipeline>,
) -> Result<(), CommandError> {
    pipeline.start_recording().map_err(CommandError::from)
}

/// Stop the audio settings A/B test recording and return before/after audio.
#[tauri::command]
pub fn pipeline_test_audio_settings_stop_recording(
    pipeline: State<'_, SharedPipeline>,
) -> Result<AudioSettingsTestWavs, CommandError> {
    use base64::Engine;

    let (raw_wav, processed_wav) = pipeline
        .stop_recording_before_after()
        .map_err(CommandError::from)?;

    Ok(AudioSettingsTestWavs {
        raw_wav_base64: base64::engine::general_purpose::STANDARD.encode(raw_wav),
        processed_wav_base64: base64::engine::general_purpose::STANDARD.encode(processed_wav),
    })
}

/// Get a copy of the most recent recording diagnostics (duration/RMS/peak + optional speech flag).
#[tauri::command]
pub fn pipeline_get_last_recording_diagnostics(
    pipeline: State<'_, SharedPipeline>,
) -> Result<Option<AudioCaptureDiagnostics>, CommandError> {
    Ok(pipeline.last_recording_diagnostics())
}

/// Full pipeline helper: Start recording if not recording, or stop and transcribe if recording
#[tauri::command]
pub async fn pipeline_toggle(
    app: AppHandle,
    pipeline: State<'_, SharedPipeline>,
) -> Result<String, CommandError> {
    if pipeline.is_recording() {
        pipeline_dictate(app, pipeline).await
    } else {
        // Try to start the pipeline FIRST - don't create a log if it fails
        pipeline.start_recording().map_err(|e| {
            log::warn!("Toggle: Failed to start recording: {}", e);
            CommandError::from(e)
        })?;

        #[cfg(desktop)]
        crate::set_escape_cancel_shortcut_enabled(&app, true);

        // Pipeline started successfully - now create the request log
        if let Some(log_store) = app.try_state::<RequestLogStore>() {
            let config = pipeline.config();
            log_store.start_request(
                config.stt_provider.clone(),
                config.stt_model.clone(),
            );
            log_store.with_current(|log| {
                log.llm_provider = if config.llm_config.enabled {
                    Some(config.llm_config.provider.clone())
                } else {
                    None
                };
                log.llm_model = config.llm_config.model.clone();
                log.info("Recording started (toggle)");
            });
        }

        let _ = app.emit("pipeline-recording-started", ());
        Ok(String::new())
    }
}

/// Check if the pipeline is in an error state
#[tauri::command]
pub fn pipeline_is_error(
    pipeline: State<'_, SharedPipeline>,
) -> Result<bool, CommandError> {
    Ok(pipeline.is_error())
}

/// Force reset the pipeline state to Idle
/// Use this to recover from stuck states
#[tauri::command]
pub fn pipeline_force_reset(
    app: AppHandle,
    pipeline: State<'_, SharedPipeline>,
) -> Result<(), CommandError> {
    pipeline.force_reset();
    log::info!("Pipeline force reset to Idle state");

    #[cfg(desktop)]
    crate::set_escape_cancel_shortcut_enabled(&app, false);

    // Emit reset event
    let _ = app.emit("pipeline-reset", ());

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_error_from_string() {
        let error = CommandError::from("test error".to_string());
        assert_eq!(error.message, "test error");
    }
}
