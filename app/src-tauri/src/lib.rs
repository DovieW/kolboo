use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, WindowEvent,
};
use tauri_utils::config::BackgroundThrottlingPolicy;

mod audio;
mod audio_capture;
mod audio_mute;
mod commands;
mod cost;
mod embeddings;
mod history;
mod llm;
mod network;
mod pipeline;
mod recordings;
mod request_log;
mod router_embeddings_cache;
mod settings;
mod shortcuts_lock;
mod stats;
mod state;
mod stt;
mod vad;
mod windows_apps;

#[cfg(target_os = "windows")]
mod windows_modifier_hotkeys;

#[cfg(test)]
mod tests;

use audio_mute::AudioMuteManager;
use history::{HistoryStorage, RequestModelInfo};
use recordings::RecordingStore;
use request_log::{RequestKind, RequestLogStore, RequestLogsRetentionConfig, RequestLogsRetentionMode};
use settings::HotkeyConfig;
use state::{AppState, MicTestMeterState, TrayKeepAlive};

#[cfg(desktop)]
use enigo::{Direction, Enigo, Key, Keyboard, Settings};

#[cfg(desktop)]
use tauri_plugin_store::StoreExt;

#[cfg(desktop)]
use tauri_plugin_global_shortcut::{Shortcut, ShortcutEvent, ShortcutState};

#[cfg(desktop)]
use tauri_plugin_global_shortcut::GlobalShortcutExt;

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

/// Normalize a shortcut string for comparison (handles "ctrl" vs "control" differences)
#[cfg(desktop)]
pub(crate) fn normalize_shortcut_string(s: &str) -> String {
    // Canonicalize for comparison across:
    // - different modifier aliases (ctrl vs control)
    // - different output ordering (e.g. "ctrl+shift+f3" vs "shift+control+f3")
    let mut parts: Vec<String> = s
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

    parts.sort();
    parts.join("+")
}

/// Helper to read a setting from the store with a default fallback
#[cfg(desktop)]
fn get_setting_from_store<T: serde::de::DeserializeOwned>(
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

/// Ensure settings shown in the UI match what the backend will use.
///
/// The frontend often treats missing keys as "unset" and shows fallback defaults.
/// If the backend uses different fallbacks, this can cause confusing mismatches.
///
/// To prevent that, we eagerly seed `settings.json` with defaults for missing/null keys
/// (without overwriting any existing values).
#[cfg(desktop)]
pub(crate) fn ensure_default_settings(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    use serde_json::{json, Value};
    use tauri_plugin_store::StoreExt;

    let store = app.store("settings.json")?;

    // Keep these defaults aligned with pipeline defaults / expected backend behavior.
    // We intentionally seed these so a brand new install has the same effective
    // settings that the pipeline will use at runtime (and what the UI shows).
    let default_pipeline_config = crate::pipeline::PipelineConfig::default();

    let is_missing = |v: Option<Value>| -> bool {
        matches!(v, None | Some(Value::Null))
    };

    let mut dirty = false;
    // Some settings intentionally use explicit null as a meaningful value.
    // For those keys, we only seed defaults when the key is truly absent.
    //
    // IMPORTANT: this closure must *not* capture `dirty`, otherwise `dirty` becomes
    // mutably borrowed for the lifetime of the closure and we can't update it
    // elsewhere (Rust E0506).
    let set_default = |key: &str, value: Value, only_if_absent: bool| -> bool {
        let should_set = if only_if_absent {
            store.get(key).is_none()
        } else {
            is_missing(store.get(key))
        };

        if should_set {
            store.set(key.to_string(), value);
            return true;
        }

        false
    };

    dirty |= set_default("stt_provider", json!("groq"), false);
    // Cerebras free-tier toggle (used by stats filtering).
    dirty |= set_default("cerebras_free_tier", json!(true), false);
    // Groq-specific toggle used by the UI (and potentially future backend pricing logic).
    dirty |= set_default("groq_free_tier", json!(true), false);
    // ElevenLabs free-tier toggle (used by stats filtering).
    dirty |= set_default("elevenlabs_free_tier", json!(true), false);
    // Cohere toggle (used by stats filtering).
    dirty |= set_default("cohere_free_tier", json!(true), false);
    // AssemblyAI and Speechmatics toggles (used by stats filtering).
    dirty |= set_default("assemblyai_free_tier", json!(true), false);
    dirty |= set_default("speechmatics_free_tier", json!(true), false);
    dirty |= set_default("stt_transcription_prompt", json!(null), false);
    dirty |= set_default("whisper_server_base_url", json!(null), false);

    // Local Whisper model selection (only meaningful when compiled with the feature).
    // This is the *model file* used by whisper.cpp (not the remote STT model dropdown).
    #[cfg(feature = "local-whisper")]
    {
        dirty |= set_default("local_whisper_model_id", json!("base"), false);
        // When to load the local whisper.cpp model file.
        // - manual: only load when user clicks Load
        // - on_transcribe: lazily load the first time transcription needs it
        // - on_launch: best-effort preload shortly after app launch
        dirty |= set_default("local_whisper_load_mode", json!("manual"), false);
    }

    dirty |= set_default("stt_timeout_seconds", json!(10.0), false);

    // Network / proxy settings.
    dirty |= set_default(
        "proxy_settings",
        serde_json::to_value(crate::settings::ProxySettings::default())?,
        false,
    );
    // How many recordings/history items to retain (impacts disk usage).
    // Keep this aligned with the UI default.
    dirty |= set_default("max_saved_recordings", json!(1000), false);

    // Request logs retention (in-memory request log history).
    // Keep this aligned with the UI default.
    dirty |= set_default("request_logs_retention_mode", json!("amount"), false);
    dirty |= set_default("request_logs_retention_amount", json!(50), false);
    // Only used when mode == "time" (days; 0 = forever)
    dirty |= set_default("request_logs_retention_days", json!(7), false);
    // Time-based retention for history/transcriptions. 0 = keep forever.
    dirty |= set_default("transcription_retention_days", json!(0), false);
    // New retention keys (unit+value) used by newer UI.
    // Keep legacy days key as well for backward compatibility.
    dirty |= set_default("transcription_retention_unit", json!("days"), false);
    dirty |= set_default("transcription_retention_value", json!(0.0), false);
    // When deleting old transcriptions, optionally also delete their .wav recordings.
    dirty |= set_default("transcription_retention_delete_recordings", json!(false), false);

    // Persisted stats retention (usage/cost events).
    // These are stored on disk (unlike request logs which are in-memory).
    // 0 = keep forever.
    dirty |= set_default("stats_retention_unit", json!("days"), false);
    dirty |= set_default("stats_retention_value", json!(30.0), false);
    // Defensive cap (bytes). The pruning logic enforces this regardless of time settings.
    dirty |= set_default("stats_retention_max_bytes", json!(50_000_000u64), false);
    dirty |= set_default("overlay_mode", json!("recording_only"), false);
    // Whether the overlay shows detailed phase text while processing
    // (e.g. "transcribing…", "routing…", "rewriting…"). When false, the overlay
    // uses a waveform animation instead.
    dirty |= set_default("overlay_show_detailed_loading", json!(false), false);
    dirty |= set_default("widget_position", json!("bottom-center"), false);
    // Whether clicking the window X exits the app or closes the main window to the tray.
    // - "exit_program": exit the application process
    // - "minimize_to_tray": close (destroy) the main window but keep the tray app running
    //   (the tray can recreate the main window on demand)
    // Legacy (migrated by the frontend normalizer):
    // - "close_window": previously meant "destroy the main window (tray can recreate it)".
    dirty |= set_default("main_window_close_behavior", json!("minimize_to_tray"), false);
    dirty |= set_default("output_mode", json!("paste"), false);
    dirty |= set_default("output_hit_enter", json!(false), false);
    dirty |= set_default("playing_audio_handling", json!("none"), false);
    dirty |= set_default("sound_enabled", json!(true), false);
    dirty |= set_default("rewrite_llm_enabled", json!(false), false);

    // Rewrite profiles: historically this was an empty array with the Default profile
    // represented implicitly by global settings. We now want Default to be a real,
    // persisted profile (id="default") so it can own presets/router config.
    let default_rewrite_profile = json!({
        "id": "default",
        "name": "Default",
        "program_paths": [],
        "cleanup_prompt_sections": null,
        "presets": [],
        "default_preset_id": null,
        "default_preset_description": null,
        "active_preset_id": null,
        "router": null,
        // Default profile inherits the global rewrite toggle.
        "rewrite_llm_enabled": null,
    });
    dirty |= set_default(
        "rewrite_program_prompt_profiles",
        json!([default_rewrite_profile.clone()]),
        false,
    );

    // Migration: if the key exists but Default isn't present yet (e.g. was seeded as []),
    // insert Default at the front without overwriting other profiles.
    match store.get("rewrite_program_prompt_profiles") {
        Some(Value::Array(mut arr)) => {
            let has_default = arr.iter().any(|v| {
                v.as_object()
                    .and_then(|o| o.get("id"))
                    .and_then(|id| id.as_str())
                    .map(|id| id == "default")
                    .unwrap_or(false)
            });

            if !has_default {
                arr.insert(0, default_rewrite_profile);
                store.set("rewrite_program_prompt_profiles".to_string(), Value::Array(arr));
                dirty = true;
            }
        }
        Some(Value::Null) | None => {
            // Already handled by set_default above.
        }
        Some(_) => {
            // Malformed value: replace with a minimal sane default.
            store.set(
                "rewrite_program_prompt_profiles".to_string(),
                json!([default_rewrite_profile]),
            );
            dirty = true;
        }
    }

    // Hotkeys: allow explicit null to mean "disabled"; only seed when key is absent.
    dirty |= set_default(
        "toggle_hotkey",
        serde_json::to_value(HotkeyConfig::default_toggle())?,
        true,
    );
    // Hotkeys: allow explicit null to mean "disabled"; only seed when key is absent.
    // Hold-to-record and paste-last are disabled by default.
    dirty |= set_default("hold_hotkey", json!(null), true);
    dirty |= set_default("paste_last_hotkey", json!(null), true);
    dirty |= set_default("retry_hotkey", json!(null), true);
    dirty |= set_default("quick_ask_hotkey", json!(null), true);
    dirty |= set_default("quick_ask_hold_hotkey", json!(null), true);
    dirty |= set_default("quick_ask_toggle_hotkey", json!(null), true);

    // Migration: legacy `quick_ask_hotkey` (hold-to-record) -> `quick_ask_hold_hotkey`.
    // Only migrate when the new key is truly absent (not when explicitly null).
    if store.get("quick_ask_hold_hotkey").is_none() {
        if let Some(v) = store.get("quick_ask_hotkey") {
            if !matches!(v, Value::Null) {
                store.set("quick_ask_hold_hotkey".to_string(), v);
                dirty = true;
            }
        }
    }

    // VAD settings are used by the pipeline.
    dirty |= set_default(
        "vad_settings",
        serde_json::to_value(settings::VadSettings::default())?,
        false,
    );

    // Capture behavior (Hot Mic + recovery)
    // - hot_mic_enabled: keep the input stream open while idle and maintain a rolling pre-roll
    // - hot_mic_pre_roll_ms: pre-roll duration (ms) to prepend at record start
    // - mic_auto_recover_enabled: watchdog the stream and attempt restart on hangs/disconnects
    dirty |= set_default("hot_mic_enabled", json!(false), false);
    dirty |= set_default("hot_mic_pre_roll_ms", json!(1500u32), false);
    dirty |= set_default("mic_auto_recover_enabled", json!(false), false);

    // Audio + quiet-recording gating.
    dirty |= set_default(
        "quiet_audio_gate_enabled",
        json!(default_pipeline_config.quiet_audio_gate_enabled),
        false,
    );
    dirty |= set_default(
        "quiet_audio_min_duration_secs",
        json!(default_pipeline_config.quiet_audio_min_duration_secs),
        false,
    );
    set_default(
        "quiet_audio_rms_dbfs_threshold",
        json!(default_pipeline_config.quiet_audio_rms_dbfs_threshold),
        false,
    );
    set_default(
        "quiet_audio_peak_dbfs_threshold",
        json!(default_pipeline_config.quiet_audio_peak_dbfs_threshold),
        false,
    );
    set_default(
        "quiet_audio_require_speech",
        json!(default_pipeline_config.quiet_audio_require_speech),
        false,
    );

    // Stop-time preprocessing defaults.
    set_default(
        "noise_gate_threshold_dbfs",
        json!(default_pipeline_config.noise_gate_threshold_dbfs),
        false,
    );
    set_default(
        "audio_downmix_to_mono",
        json!(default_pipeline_config.audio_downmix_to_mono),
        false,
    );
    set_default(
        "audio_resample_to_16khz",
        json!(default_pipeline_config.audio_resample_to_16khz),
        false,
    );
    set_default(
        "audio_highpass_enabled",
        json!(default_pipeline_config.audio_highpass_enabled),
        false,
    );
    set_default(
        "audio_agc_enabled",
        json!(default_pipeline_config.audio_agc_enabled),
        false,
    );
    set_default(
        "audio_noise_suppression_enabled",
        json!(default_pipeline_config.audio_noise_suppression_enabled),
        false,
    );

    if dirty {
        // Persist seeded defaults.
        // If saving fails, we don't want to crash the app; the runtime fallbacks will still work.
        if let Err(e) = store.save() {
            log::warn!("Failed to save seeded default settings: {}", e);
        }
    }

    Ok(())
}

/// Emit a system event to the frontend for debugging
#[cfg(desktop)]
fn emit_system_event(app: &AppHandle, event_type: &str, message: &str, details: Option<&str>) {
    #[derive(serde::Serialize, Clone)]
    struct SystemEvent {
        timestamp: String,
        event_type: String,
        message: String,
        details: Option<String>,
    }

    let event = SystemEvent {
        timestamp: chrono::Utc::now().to_rfc3339(),
        event_type: event_type.to_string(),
        message: message.to_string(),
        details: details.map(|s| s.to_string()),
    };

    let _ = app.emit("system-event", event);
}

/// Normalize transcript text for output.
///
/// We intentionally keep this conservative: the pipeline now performs a
/// quiet-audio gate before STT to avoid "silent audio" hallucinations.
fn sanitize_transcript(transcript: &str) -> Option<String> {
    let trimmed = transcript.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Resolve the most recent history entry id that has a persisted recording available.
///
/// This is used by the Retry hotkey to pick "the last recording".
#[cfg(desktop)]
fn resolve_last_recording_history_entry_id(app: &AppHandle) -> Option<String> {
    let Some(history) = app.try_state::<HistoryStorage>() else {
        return None;
    };

    let Some(store) = app.try_state::<RecordingStore>() else {
        return None;
    };

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
fn spawn_retry_last_recording_and_output(app: &AppHandle, source: &str) {
    let app = app.clone();
    let source = source.to_string();

    tauri::async_runtime::spawn(async move {
        let Some(pipeline) = app.try_state::<pipeline::SharedPipeline>() else {
            log::warn!("{source}: pipeline not available; cannot retry");
            return;
        };
        let pipeline = (*pipeline).clone();

        let pipeline_state = pipeline.state();
        if !matches!(pipeline_state, pipeline::PipelineState::Idle | pipeline::PipelineState::Error)
        {
            log::info!("{source}: retry ignored (pipeline busy: {:?})", pipeline_state);
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

        let Some(text) = sanitize_transcript(&transcript) else {
            log::info!("{source}: retry returned empty transcript; nothing to output");
            return;
        };

        let output_mode_str: String = get_setting_from_store(&app, "output_mode", "paste".to_string());
        let output_mode = commands::text::OutputMode::from_str(&output_mode_str);
        let output_hit_enter: bool = get_setting_from_store(&app, "output_hit_enter", false);

        if let Err(e) = commands::text::output_text_with_mode(&text, output_mode, output_hit_enter)
        {
            log::error!("{source}: failed to output retry transcript: {}", e);
        }
    });
}

// ============================================================================
// Playing audio handling during recording
// ============================================================================

#[cfg(desktop)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlayingAudioHandling {
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

    fn wants_mute(self) -> bool {
        matches!(self, Self::Mute | Self::MuteAndPause)
    }

    fn wants_pause(self) -> bool {
        matches!(self, Self::Pause | Self::MuteAndPause)
    }
}

#[cfg(desktop)]
fn get_playing_audio_handling(app: &AppHandle) -> PlayingAudioHandling {
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

#[cfg(desktop)]
fn toggle_media_play_pause(app: &AppHandle) -> Result<(), String> {
    // On macOS, enigo requires running on the main thread.
    #[cfg(target_os = "macos")]
    {
        let (tx, rx) = std::sync::mpsc::channel::<Result<(), String>>();
        app.run_on_main_thread(move || {
            let mut enigo = match Enigo::new(&Settings::default()) {
                Ok(e) => e,
                Err(e) => {
                    let _ = tx.send(Err(e.to_string()));
                    return;
                }
            };

            let result = enigo
                .key(Key::MediaPlayPause, Direction::Click)
                .map_err(|e| e.to_string());
            let _ = tx.send(result);
        })
        .map_err(|e| e.to_string())?;
        return rx.recv().map_err(|e| e.to_string())?;
    }

    #[cfg(not(target_os = "macos"))]
    {
        // `app` is only needed on macOS (main-thread requirement). Silence the
        // unused-parameter warning on other platforms without changing behavior.
        let _ = app;
        let mut enigo = Enigo::new(&Settings::default()).map_err(|e| e.to_string())?;
        enigo
            .key(Key::MediaPlayPause, Direction::Click)
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

#[cfg(target_os = "windows")]
fn is_non_system_audio_session_active() -> Result<bool, String> {
    use windows::Win32::Media::Audio::{
        eConsole, eRender, AudioSessionStateActive, IAudioSessionManager2, IMMDevice,
        IMMDeviceEnumerator, MMDeviceEnumerator,
    };
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED,
    };

    unsafe {
        // Initialize COM (ignore error if already initialized)
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);

        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
                .map_err(|e| format!("Failed to create device enumerator: {}", e))?;

        let device: IMMDevice = enumerator
            .GetDefaultAudioEndpoint(eRender, eConsole)
            .map_err(|e| format!("Failed to get default audio endpoint: {}", e))?;

        // Enumerate sessions on the default render endpoint.
        let session_manager = device
            .Activate::<IAudioSessionManager2>(CLSCTX_ALL, None)
            .map_err(|e| format!("Failed to activate session manager: {}", e))?;

        let sessions = session_manager
            .GetSessionEnumerator()
            .map_err(|e| format!("Failed to get session enumerator: {}", e))?;

        let count = sessions
            .GetCount()
            .map_err(|e| format!("Failed to get session count: {}", e))?;

        for i in 0..count {
            let session = sessions
                .GetSession(i)
                .map_err(|e| format!("Failed to get session {}: {}", i, e))?;

            let state = session
                .GetState()
                .map_err(|e| format!("Failed to get session state: {}", e))?;
            if state == AudioSessionStateActive {
                return Ok(true);
            }
        }

        Ok(false)
    }
}

#[cfg(not(target_os = "windows"))]
fn is_non_system_audio_session_active() -> Result<bool, String> {
    // Best-effort on non-Windows platforms: we don't currently have a reliable
    // cross-platform way to detect whether audio is actively playing.
    Ok(true)
}

/// Start recording with sound and audio mute handling
#[cfg(desktop)]
fn start_recording(
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
    log::info!("{}: starting recording (current pipeline state: {:?})", source, current_state);
    emit_system_event(app, "shortcut", &format!("{}: starting recording", source), Some(&format!("Pipeline state: {:?}", current_state)));

    // Start pipeline recording FIRST - if it fails, don't do anything else
    if let Some(pipeline) = app.try_state::<pipeline::SharedPipeline>() {
        // Pin the per-program profile *before* we show any overlay windows.
        // The overlay is always-on-top and can briefly become the foreground window on Windows,
        // which would otherwise cause per-program profile detection to degrade to Default.
        let config = pipeline.config();
        let foreground = crate::windows_apps::get_foreground_process_path();
        let matched_profile = crate::pipeline::select_profile_for_foreground_app(&config.llm_config);

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

        if let Err(e) = pipeline.start_recording() {
            log::error!("{}: Failed to start pipeline recording: {} (state was: {:?})", source, e, current_state);
            let _ = pipeline.set_session_profile_override(None);
            let error_msg = format!("{} (pipeline state: {:?})", e, current_state);
            emit_system_event(app, "error", &format!("{}: Failed to start recording", source), Some(&error_msg));
            let payload = serde_json::json!({
                "message": error_msg,
                "request_id": null,
            });
            let _ = app.emit("pipeline-error", payload);
            return;
        }

        // Pipeline started successfully - now start request logging.
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
                // Avoid confusing logs: if LLM rewrite is disabled, do not record an LLM model.
                // (The settings store may still contain a previously-selected model.)
                log.llm_model = if config.llm_config.enabled {
                    config.llm_config.model.clone()
                } else {
                    None
                };
                log.info(format!("Recording started ({})", source));
            });
        }
    }

    // While recording/transcribing, allow Escape to cancel without triggering transcription.
    set_escape_cancel_shortcut_enabled(app, true);

    // Pipeline started successfully - now update state and do side effects
    state.is_recording.store(true, Ordering::SeqCst);

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
        let payload = serde_json::json!({
            "seq": 0,
            "rms": 0.0,
            "peak": 0.0,
            "wave_seq": 0,
            "mins": Vec::<f32>::new(),
            "maxes": Vec::<f32>::new(),
        });
        if let Some(overlay) = app.get_webview_window("overlay") {
            let _ = overlay.emit("overlay-audio-level", payload);
        } else {
            let _ = app.emit("overlay-audio-level", payload);
        }
    }

    // Notify frontend ASAP so the overlay can update/animate without waiting for
    // audio side-effects (which may block, e.g. when we ensure the cue finishes
    // before muting system audio).
    let _ = app.emit("recording-start", ());

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
        match is_non_system_audio_session_active() {
            Ok(true) => match toggle_media_play_pause(app) {
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
                log::warn!("Failed to detect active audio session; skipping pause: {}", e);
                state.play_pause_toggled.store(false, Ordering::SeqCst);
            }
        }
    } else {
        state.play_pause_toggled.store(false, Ordering::SeqCst);
    }

}

/// Stop recording with sound and audio unmute handling
#[cfg(desktop)]
fn stop_recording(
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
    emit_system_event(app, "shortcut", &format!("{}: stopping recording", source), None);

    // If this recording was started via the Quick Ask hotkey, branch the post-transcription
    // flow into an LLM answer overlay (instead of output/paste).
    let is_quick_ask_session = state
        .quick_ask_session_active
        .swap(false, Ordering::SeqCst);

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

    // Stop pipeline and trigger transcription in background
    if let Some(pipeline) = app.try_state::<pipeline::SharedPipeline>() {
        let pipeline_clone = (*pipeline).clone();
        let app_clone = app.clone();
        let overlay_mode_clone = overlay_mode.clone();

        // Capture model info from pipeline config for persistence in history.
        let config = pipeline.config();
        let profile = pipeline::select_profile_for_foreground_app(&config.llm_config);

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
        }

        let quick_ask_profile_cfg: QuickAskProfileConfig = profile
            .as_ref()
            .map(|p| QuickAskProfileConfig {
                provider: p.quick_ask_provider.clone(),
                model: p.quick_ask_model.clone(),
                system_prompt: p.quick_ask_system_prompt.clone(),
                openai_reasoning_effort: p.quick_ask_openai_reasoning_effort.clone(),
                gemini_thinking_budget: p.quick_ask_gemini_thinking_budget,
                gemini_thinking_level: p.quick_ask_gemini_thinking_level.clone(),
                anthropic_thinking_budget: p.quick_ask_anthropic_thinking_budget,
            })
            .unwrap_or_default();

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
                let id = log_store.start_request(config.stt_provider.clone(), config.stt_model.clone());
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
                tauri::async_runtime::spawn(async move {
                    let start = std::time::Instant::now();
                    loop {
                        match pipeline_for_evt.state() {
                            pipeline::PipelineState::Transcribing
                            | pipeline::PipelineState::Rewriting => {
                                let _ = app_for_evt.emit("pipeline-transcription-started", ());

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
                            pipeline::PipelineState::Recording | pipeline::PipelineState::Routing => {}
                        }

                        if start.elapsed() > std::time::Duration::from_secs(2) {
                            break;
                        }

                        tokio::time::sleep(std::time::Duration::from_millis(15)).await;
                    }
                });
            }

            // Emit routing started once the pipeline transitions into the Routing phase.
            {
                let app_for_evt = app_clone.clone();
                let pipeline_for_evt = pipeline_clone.clone();
                tauri::async_runtime::spawn(async move {
                    let start = std::time::Instant::now();
                    loop {
                        match pipeline_for_evt.state() {
                            pipeline::PipelineState::Routing => {
                                let _ = app_for_evt.emit("pipeline-routing-started", ());
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
                });
            }

            // Emit rewriting started once the pipeline actually enters the optional LLM phase.
            //
            // This keeps the overlay UI accurate even if state polling is delayed.
            {
                let app_for_evt = app_clone.clone();
                let pipeline_for_evt = pipeline_clone.clone();
                tauri::async_runtime::spawn(async move {
                    let start = std::time::Instant::now();
                    loop {
                        match pipeline_for_evt.state() {
                            pipeline::PipelineState::Rewriting => {
                                let _ = app_for_evt.emit("pipeline-rewriting-started", ());
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
                });
            }

            // Create an in-progress history entry while we transcribe.
            // Quick Ask uses a separate UI surface and should not pollute the main dictation history.
            if !is_quick_ask_session {
                if let Some(ref req_id) = request_id {
                    if let Some(history) = app_clone.try_state::<HistoryStorage>() {
                        let max_saved_recordings: usize = (get_setting_from_store(
                            &app_clone,
                            "max_saved_recordings",
                            1000u64,
                        ))
                        .clamp(1, 100_000) as usize;

                        let _ = history.add_request_entry(
                            req_id.clone(),
                            model_info,
                            max_saved_recordings,
                        );
                        let _ = app_clone.emit("history-changed", ());
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
                        let should_complete_now = !is_quick_ask_session;

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
                                    let _ = history.set_request_preset(req_id, preset_id, preset_name);
                                    let _ = app_clone.emit("history-changed", ());
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
                        }

                        if should_complete_now {
                            log_store.complete_current();
                        }
                    }

                    // Persist audio for retry (best-effort)
                    if let (Some(ref req_id), Some(store)) = (
                        request_id.as_ref(),
                        app_clone.try_state::<RecordingStore>(),
                    ) {
                        if let Some(wav) = pipeline_clone.clone_last_wav_bytes() {
                            if store.save_wav(req_id, &wav).is_ok() {
                                let max_saved_recordings: usize = (get_setting_from_store(
                                    &app_clone,
                                    "max_saved_recordings",
                                    1000u64,
                                ))
                                .clamp(1, 100_000) as usize;

                                let _ = store.prune_to_max_files(max_saved_recordings);
                            }
                        }
                    }

                    if let Some(ref text) = filtered_transcript {
                        let _ = app_clone.emit("pipeline-transcript-ready", text);

                        // Quick Ask: instead of outputting/pasting the transcript, send it to an LLM
                        // for an answer and show it in a dedicated overlay.
                        if is_quick_ask_session {
                            let question = sanitize_transcript(&result.stt_text)
                                .unwrap_or_else(|| text.clone())
                                .trim()
                                .to_string();

                            let emit_to_quick_ask = |app: &AppHandle, event: &str, payload: serde_json::Value| {
                                if let Some(win) = app.get_webview_window("quick_ask") {
                                    let _ = win.emit(event, payload);
                                } else {
                                    let _ = app.emit(event, payload);
                                }
                            };

                            // Ensure the answer window is visible before we start the LLM call.
                            if let Some(win) = app_clone.get_webview_window("quick_ask") {
                                let _ = win.set_always_on_top(true);
                                let _ = win.show();
                                let _ = win.set_focus();
                            }

                            if question.is_empty() {
                                // Quick Ask is considered the "request" here, so mark the request log
                                // accordingly and finalize it.
                                if let Some(log_store) = app_clone.try_state::<RequestLogStore>() {
                                    log_store.with_current(|log| {
                                        log.kind = RequestKind::QuickAsk;
                                        log.quick_ask_question = Some(String::new());
                                        log.quick_ask_answer = None;
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

                                emit_to_quick_ask(
                                    &app_clone,
                                    "quick-ask-answer",
                                    serde_json::json!({
                                        "ok": false,
                                        "error": "No transcript to answer (empty)"
                                    }),
                                );
                            } else {
                                // Resolve effective Quick Ask configuration:
                                // per-profile override -> global Quick Ask defaults -> global rewrite provider -> fallback.
                                let global_quick_ask_provider: Option<String> =
                                    get_setting_from_store(&app_clone, "quick_ask_provider", Option::<String>::None);
                                let global_quick_ask_model: Option<String> =
                                    get_setting_from_store(&app_clone, "quick_ask_model", Option::<String>::None);
                                let global_quick_ask_system_prompt: Option<String> = get_setting_from_store(
                                    &app_clone,
                                    "quick_ask_system_prompt",
                                    Option::<String>::None,
                                );

                                let global_qa_openai_reasoning_effort: Option<String> = get_setting_from_store(
                                    &app_clone,
                                    "quick_ask_openai_reasoning_effort",
                                    Option::<String>::None,
                                );
                                let global_qa_gemini_thinking_budget: Option<i64> = get_setting_from_store(
                                    &app_clone,
                                    "quick_ask_gemini_thinking_budget",
                                    Option::<i64>::None,
                                );
                                let global_qa_gemini_thinking_level: Option<String> = get_setting_from_store(
                                    &app_clone,
                                    "quick_ask_gemini_thinking_level",
                                    Option::<String>::None,
                                );
                                let global_qa_anthropic_thinking_budget: Option<i64> = get_setting_from_store(
                                    &app_clone,
                                    "quick_ask_anthropic_thinking_budget",
                                    Option::<i64>::None,
                                );

                                let fallback_provider: Option<String> =
                                    get_setting_from_store(&app_clone, "llm_provider", Option::<String>::None);

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

                                emit_to_quick_ask(
                                    &app_clone,
                                    "quick-ask-started",
                                    serde_json::json!({
                                        "question": question.clone(),
                                        "provider": provider.clone(),
                                        "model": model.clone(),
                                    }),
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
                                    let err = format!("No API key configured for provider: {}", provider);
                                    if let Some(log_store) = app_clone.try_state::<RequestLogStore>() {
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

                                    emit_to_quick_ask(
                                        &app_clone,
                                        "quick-ask-answer",
                                        serde_json::json!({
                                            "ok": false,
                                            "error": err,
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
                                        crate::commands::llm::create_llm_provider_unstructured(&provider_cfg);
                                    let t0 = std::time::Instant::now();
                                    match provider_impl.complete(system_prompt.as_str(), question.as_str()).await {
                                        Ok(answer) => {
                                            let answer = answer.trim().to_string();
                                            let duration_ms = t0.elapsed().as_millis() as u64;

                                            if let Some(log_store) = app_clone.try_state::<RequestLogStore>() {
                                                log_store.with_current(|log| {
                                                    log.kind = RequestKind::QuickAsk;
                                                    log.quick_ask_answer = Some(answer.clone());
                                                    log.quick_ask_provider = Some(provider_impl.name().to_string());
                                                    log.quick_ask_model = Some(provider_impl.model().to_string());
                                                    log.quick_ask_duration_ms = Some(duration_ms);
                                                    log.quick_ask_response_json = Some(serde_json::json!({
                                                        "ok": true,
                                                        "answer": answer.clone(),
                                                        "provider_used": provider_impl.name(),
                                                        "model_used": provider_impl.model(),
                                                        "duration_ms": duration_ms,
                                                    }));
                                                    log.complete_success();
                                                });

                                                if let Some(wav) = pipeline_clone.clone_last_wav_bytes() {
                                                    stats::emit_cost_events_for_current_request(
                                                        &app_clone,
                                                        stats::EventStatus::Success,
                                                        Some(&wav),
                                                    );
                                                }

                                                log_store.complete_current();
                                            }

                                            emit_to_quick_ask(
                                                &app_clone,
                                                "quick-ask-answer",
                                                serde_json::json!({
                                                    "ok": true,
                                                    "answer": answer,
                                                    "provider_used": provider_impl.name(),
                                                    "model_used": provider_impl.model(),
                                                    "duration_ms": duration_ms,
                                                }),
                                            );
                                        }
                                        Err(e) => {
                                            let err = e.to_string();
                                            if let Some(log_store) = app_clone.try_state::<RequestLogStore>() {
                                                log_store.with_current(|log| {
                                                    log.kind = RequestKind::QuickAsk;
                                                    log.quick_ask_answer = None;
                                                    log.quick_ask_response_json = Some(serde_json::json!({
                                                        "ok": false,
                                                        "error": err.clone(),
                                                    }));
                                                    log.error(format!("Quick Ask failed: {}", err.clone()));
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

                                            emit_to_quick_ask(
                                                &app_clone,
                                                "quick-ask-answer",
                                                serde_json::json!({
                                                    "ok": false,
                                                    "error": err,
                                                }),
                                            );
                                        }
                                    }
                                }
                            }
                        } else {
                            // Output the transcript based on mode
                            if let Err(e) = commands::text::output_text_with_mode(text, output_mode, output_hit_enter) {
                                log::error!("Failed to output transcript: {}", e);

                                if let Some(log_store) = app_clone.try_state::<RequestLogStore>() {
                                    log_store.with_current(|log| {
                                        log.warn(format!("Output failed: {}", e));
                                    });
                                }
                            }
                        }

                        // Save to history
                        if !is_quick_ask_session {
                            if let Some(ref req_id) = request_id {
                                if let Some(history) = app_clone.try_state::<HistoryStorage>() {
                                    if let Err(e) = history.complete_request_success(req_id, text.clone()) {
                                        log::warn!("Failed to update history: {}", e);
                                    }

                                    let (provider, model) = if result.llm_attempted() {
                                        (result.llm_provider_used.clone(), result.llm_model_used.clone())
                                    } else {
                                        (None, None)
                                    };
                                    let _ = history.set_request_llm_model(req_id, provider, model);
                                    let _ = app_clone.emit("history-changed", ());
                                }
                            }
                        }

                        // Time-based retention (best-effort). This path is used by global shortcuts.
                        commands::recording::apply_transcription_retention(&app_clone);
                    } else {
                        // Emit empty transcript event so UI can update appropriately
                        let _ = app_clone.emit("pipeline-transcript-ready", "");
                        log::info!("No transcript output (empty/whitespace), not outputting");

                        if is_quick_ask_session {
                            let emit_to_quick_ask = |app: &AppHandle,
                                                     event: &str,
                                                     payload: serde_json::Value| {
                                if let Some(win) = app.get_webview_window("quick_ask") {
                                    let _ = win.emit(event, payload);
                                } else {
                                    let _ = app.emit(event, payload);
                                }
                            };

                            // Ensure the answer window is visible so the error is actually seen.
                            if let Some(win) = app_clone.get_webview_window("quick_ask") {
                                let _ = win.set_always_on_top(true);
                                let _ = win.show();
                                let _ = win.set_focus();
                            }

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

                            emit_to_quick_ask(
                                &app_clone,
                                "quick-ask-answer",
                                serde_json::json!({
                                    "ok": false,
                                    "error": "No transcript to answer (empty)",
                                }),
                            );
                        }

                        // Mark history entry as success with empty text (keeps timeline consistent)
                        if !is_quick_ask_session {
                            if let Some(ref req_id) = request_id {
                                if let Some(history) = app_clone.try_state::<HistoryStorage>() {
                                    let _ = history.complete_request_success(req_id, String::new());

                                    let (provider, model) = if result.llm_attempted() {
                                        (result.llm_provider_used.clone(), result.llm_model_used.clone())
                                    } else {
                                        (None, None)
                                    };
                                    let _ = history.set_request_llm_model(req_id, provider, model);
                                    let _ = app_clone.emit("history-changed", ());
                                }
                            }
                        }

                        // Time-based retention (best-effort). This path is used by global shortcuts.
                        commands::recording::apply_transcription_retention(&app_clone);
                    }

                    // Hide overlay after transcription completes if in "recording_only" mode.
                    // We request a hide so the frontend can animate (zoom-out) before the webview hides.
                    if overlay_mode_clone == "recording_only" {
                        let _ = app_clone.emit("overlay-hide-requested", ());

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
                                if current_mode == "recording_only" && current_epoch == expected_epoch {
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
                        let _ = app_clone.emit("pipeline-cancelled", ());

                        if overlay_mode_clone == "recording_only" {
                            let _ = app_clone.emit("overlay-hide-requested", ());
                            if let Some(window) = app_clone.get_webview_window("overlay") {
                                let _ = window.hide();
                            }
                        }

                        // Done - stop stealing Escape.
                        crate::set_escape_cancel_shortcut_enabled(&app_clone, false);
                        return;
                    }

                    log::error!("Transcription failed: {}", e);
                    let payload = serde_json::json!({
                        "message": e.to_string(),
                        "request_id": request_id.clone(),
                    });
                    let _ = app_clone.emit("pipeline-error", payload);

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
                    if let (Some(ref req_id), Some(store)) = (
                        request_id.as_ref(),
                        app_clone.try_state::<RecordingStore>(),
                    ) {
                        if let Some(wav) = pipeline_clone.clone_last_wav_bytes() {
                            if store.save_wav(req_id, &wav).is_ok() {
                                let max_saved_recordings: usize = (get_setting_from_store(
                                    &app_clone,
                                    "max_saved_recordings",
                                    1000u64,
                                ))
                                .clamp(1, 100_000) as usize;

                                let _ = store.prune_to_max_files(max_saved_recordings);
                            }
                        }
                    }

                    // Mark history entry as error and keep it
                    if let Some(ref req_id) = request_id {
                        if let Some(history) = app_clone.try_state::<HistoryStorage>() {
                            let _ = history.complete_request_error(req_id, e.to_string());
                            let _ = app_clone.emit("history-changed", ());
                        }
                    }

                    // Time-based retention (best-effort). Still apply even on failures.
                    commands::recording::apply_transcription_retention(&app_clone);

                    // Force-show overlay for retry UI regardless of overlay_mode.
                    // If the user is not in always-visible mode, also snap back to the saved preset.
                    if let Err(e) = commands::overlay::show_overlay_with_reset_if_not_always(&app_clone) {
                        log::warn!("Failed to force-show overlay after error: {}", e);
                    }

                }
            }

            // Transcription finished (success or error) - stop stealing Escape.
            crate::set_escape_cancel_shortcut_enabled(&app_clone, false);
        });
    }

    let _ = app.emit("recording-stop", ());
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

        if let Err(e) = shortcut_manager.on_shortcut(ESCAPE_CANCEL_SHORTCUT, |app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                cancel_pipeline_session(app, "Escape");
            }
        }) {
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
    state.quick_ask_toggle_key_held.store(false, Ordering::SeqCst);
    state.quick_ask_session_active.store(false, Ordering::SeqCst);

    // Restore audio side effects (unmute + resume playback if we paused).
    let sound_enabled: bool = get_setting_from_store(app, "sound_enabled", true);
    let playing_audio_handling: PlayingAudioHandling = get_playing_audio_handling(app);
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
        let audio_cue_raw: String =
            get_setting_from_store(app, "audio_cue", "kolboo".to_string());
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
            let _ = app.emit("history-changed", ());
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
        let _ = app.emit("overlay-hide-requested", ());

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
    let _ = app.emit("pipeline-cancelled", ());

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
    let playing_audio_handling: PlayingAudioHandling = get_playing_audio_handling(app);

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
                .or_else(|| HotkeyConfig::default_quick_ask()),
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
                    emit_system_event(app, "shortcut", "Toggle key released", Some(&format!("Pipeline state: {:?}", pipeline_state)));

                    // Do not allow starting a new capture while we are processing a previous one.
                    // This avoids a brief error UI flash if the user taps the toggle again.
                    if matches!(
                        pipeline_state,
                        Some(pipeline::PipelineState::Transcribing | pipeline::PipelineState::Rewriting)
                    ) {
                        log::info!(
                            "Toggle ignored (pipeline busy: {:?})",
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
                        log::info!(
                            "Toggle ignored (pipeline state: {:?})",
                            pipeline_state
                        );
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
                    emit_system_event(app, "shortcut", "Hold key pressed", Some(&format!("Pipeline state: {:?}", pipeline_state)));

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
                    let output_mode_str: String = get_setting_from_store(app, "output_mode", "paste".to_string());
                    let output_mode = commands::text::OutputMode::from_str(&output_mode_str);

                    let output_hit_enter: bool = get_setting_from_store(app, "output_hit_enter", false);

                    let history_storage = app.state::<HistoryStorage>();

                    if let Ok(entries) = history_storage.get_all(Some(1)) {
                        if let Some(entry) = entries.first() {
                            if let Err(e) = commands::text::output_text_with_mode(&entry.text, output_mode, output_hit_enter) {
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

                    log::info!("QuickAskHold pressed: pipeline state = {:?}", pipeline_state);
                    emit_system_event(
                        app,
                        "shortcut",
                        "Quick Ask Hold pressed",
                        Some(&format!("Pipeline state: {:?}", pipeline_state)),
                    );

                    // Do not allow starting while we are processing a previous capture.
                    if matches!(
                        pipeline_state,
                        Some(pipeline::PipelineState::Transcribing | pipeline::PipelineState::Rewriting)
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
                            state.quick_ask_session_active.store(false, Ordering::SeqCst);
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
                        state.quick_ask_session_active.store(false, Ordering::SeqCst);
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
                if state.quick_ask_toggle_key_held.swap(false, Ordering::SeqCst) {
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
                        Some(pipeline::PipelineState::Transcribing | pipeline::PipelineState::Rewriting)
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
                            state.quick_ask_session_active.store(false, Ordering::SeqCst);
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

/// Check if audio mute is supported on this platform
#[tauri::command]
fn is_audio_mute_supported() -> bool {
    audio_mute::is_supported()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize logger
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let mut builder = tauri::Builder::default();

    #[cfg(desktop)]
    {
        builder = builder.plugin(build_global_shortcut_plugin());
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
            commands::data::delete_all_data,
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
            // Fireworks helpers
            commands::fireworks::fireworks_list_models,
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
                ensure_default_settings(app.handle())?;
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
            let app_data_dir = app
                .path()
                .app_data_dir()
                .expect("Failed to get app data directory");

            // Initialize recording store (saved WAVs for retry)
            let recording_store = RecordingStore::new(app_data_dir.clone());
            app.manage(recording_store);

            let history_storage = HistoryStorage::new(app_data_dir.clone());
            app.manage(history_storage);

            // Initialize persisted stats store (usage/cost ledger)
            let stats_store = stats::StatsStore::new(app_data_dir);
            app.manage(stats_store);

            // Apply stats retention immediately on startup.
            // This keeps disk usage bounded even if the app is updated after long gaps.
            {
                let cfg = stats::read_stats_retention_config(app.handle());
                if let Some(store) = app.try_state::<stats::StatsStore>() {
                    let _ = store.prune(cfg);
                }
            }

            // Apply the configured history retention limit immediately so existing installs
            // don't keep more entries than the UI/backend intend.
            #[cfg(desktop)]
            {
                let max_saved_recordings: u64 =
                    get_setting_from_store(app.handle(), "max_saved_recordings", 1000u64);
                if let Some(history) = app.try_state::<HistoryStorage>() {
                    let _ = history.trim_to(max_saved_recordings as usize);
                }
            }

            // Initialize request log store
            #[cfg(desktop)]
            {
                use chrono::Duration as ChronoDuration;

                let mode: String = get_setting_from_store(
                    app.handle(),
                    "request_logs_retention_mode",
                    "amount".to_string(),
                );
                let amount: u64 =
                    get_setting_from_store(app.handle(), "request_logs_retention_amount", 50u64);
                let days: u64 =
                    get_setting_from_store(app.handle(), "request_logs_retention_days", 7u64);

                let mode = if mode == "time" {
                    RequestLogsRetentionMode::Time
                } else {
                    RequestLogsRetentionMode::Amount
                };

                let retention = RequestLogsRetentionConfig {
                    mode,
                    amount: amount.max(1).min(1000) as usize,
                    time_retention: if days == 0 {
                        None
                    } else {
                        Some(ChronoDuration::days(days as i64))
                    },
                };

                let request_log_store = request_log::RequestLogStore::new_with_retention(retention);
                app.manage(request_log_store);
            }

            #[cfg(not(desktop))]
            {
                let request_log_store = request_log::RequestLogStore::new();
                app.manage(request_log_store);
            }

            // Initialize audio mute manager (may be None on unsupported platforms)
            if let Some(audio_mute_manager) = AudioMuteManager::new() {
                app.manage(audio_mute_manager);
            }

            // Initialize pipeline with settings from store
            #[cfg(desktop)]
            {
                let pipeline = initialize_pipeline_from_settings(app.handle());

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
                let app_handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    let mut last_seq: u64 = 0;
                    let mut last_emit = Instant::now();
                    let mut last_priming_emit: Option<Instant> = None;

                    loop {
                        // 60Hz-ish. If this is too chatty we can reduce to 30Hz later.
                        tokio::time::sleep(Duration::from_millis(16)).await;

                        let Some(pipeline) = app_handle.try_state::<pipeline::SharedPipeline>() else {
                            continue;
                        };

                        // Prefer a non-blocking state check: during pipeline start-up the
                        // mutex may be held while CPAL capture already begins.
                        //
                        // If we can prove we're NOT recording, don't publish. Otherwise,
                        // allow publishing once the meter seq starts moving.
                        if let Some(state) = pipeline.try_state() {
                            if state != pipeline::PipelineState::Recording {
                                last_seq = 0;
                                last_priming_emit = None;
                                continue;
                            }
                        }

                        // Read the latest snapshots without locking the pipeline.
                        // Drive emission from the level meter so the overlay stays alive
                        // even if waveform buckets are temporarily unavailable.
                        let levels = pipeline.audio_level_snapshot_fast();

                        // If the capture stream has started but we haven't seen a callback yet,
                        // send a one-time "priming" event so the overlay can render immediately
                        // (baseline waveform) instead of waiting for the first buffer.
                        if levels.seq == 0 {
                            // Haven't observed any callbacks yet.
                            // Keep sending priming frames for a short while so the overlay
                            // doesn't miss the first event if its listener registers late.
                            let should_emit = match last_priming_emit {
                                None => true,
                                Some(t) => t.elapsed() >= Duration::from_millis(50),
                            };
                            if should_emit {
                                last_priming_emit = Some(Instant::now());
                                let payload = serde_json::json!({
                                    "seq": 0,
                                    "rms": 0.0,
                                    "peak": 0.0,
                                    "wave_seq": 0,
                                    "mins": Vec::<f32>::new(),
                                    "maxes": Vec::<f32>::new(),
                                });
                                if let Some(overlay) = app_handle.get_webview_window("overlay") {
                                    let _ = overlay.emit("overlay-audio-level", payload);
                                } else {
                                    let _ = app_handle.emit("overlay-audio-level", payload);
                                }
                            }
                            continue;
                        }

                        if levels.seq == last_seq {
                            continue;
                        }
                        last_seq = levels.seq;

                        // Waveform buckets (may be all-zeros early or on some devices).
                        let wave = pipeline.audio_waveform_snapshot_fast();

                        // Throttle slightly if needed (defensive).
                        if last_emit.elapsed() < Duration::from_millis(8) {
                            continue;
                        }
                        last_emit = Instant::now();

                        // Emit directly to the overlay window when available.
                        // This avoids any ambiguity around app-wide vs window event targets.
                        let payload = serde_json::json!({
                            "seq": levels.seq,
                            "rms": levels.rms,
                            "peak": levels.peak,
                            "wave_seq": wave.seq,
                            "mins": wave.mins,
                            "maxes": wave.maxes,
                        });
                        if let Some(overlay) = app_handle.get_webview_window("overlay") {
                            let _ = overlay.emit("overlay-audio-level", payload);
                        } else {
                            let _ = app_handle.emit("overlay-audio-level", payload);
                        }
                    }
                });
            }

            // Register shortcuts from store (now that store plugin is available)
            #[cfg(desktop)]
            {
                register_initial_shortcuts(app.handle())?;
            }

            // Create overlay window
            let overlay = tauri::WebviewWindowBuilder::new(
                app,
                "overlay",
                tauri::WebviewUrl::App("overlay.html".into()),
            )
            .title("Kolboo Overlay")
            .inner_size(48.0, 48.0)
            .decorations(false)
            .transparent(true)
            .shadow(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .resizable(false)
            .focused(false)
            .focusable(false)
            .accept_first_mouse(true)
            .visible(true)
            .visible_on_all_workspaces(true)
            .background_throttling(BackgroundThrottlingPolicy::Disabled)
            .build()?;

            // Create hover panel window (hidden by default).
            // This avoids resizing the main overlay window on hover, which can cause
            // cursor flicker and position drift on Windows.
            let _overlay_hover = tauri::WebviewWindowBuilder::new(
                app,
                "overlay_hover",
                tauri::WebviewUrl::App("overlay-hover.html".into()),
            )
            .title("Kolboo Presets")
            .inner_size(320.0, 220.0)
            .decorations(false)
            .transparent(true)
            .shadow(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .resizable(false)
            .focused(false)
            .focusable(false)
            .accept_first_mouse(true)
            .visible(false)
            .visible_on_all_workspaces(true)
            .background_throttling(BackgroundThrottlingPolicy::Disabled)
            .build()?;

            // Create Quick Ask answer window (hidden by default).
            // This is a separate transparent webview that renders an answer + copy button.
            let quick_ask = tauri::WebviewWindowBuilder::new(
                app,
                "quick_ask",
                tauri::WebviewUrl::App("quick-ask.html".into()),
            )
            .title("Kolboo Quick Ask")
            .inner_size(520.0, 340.0)
            .decorations(false)
            .transparent(true)
            .shadow(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .resizable(false)
            .focused(false)
            .accept_first_mouse(true)
            .visible(false)
            .visible_on_all_workspaces(true)
            .background_throttling(BackgroundThrottlingPolicy::Disabled)
            .build()?;

            // On macOS, convert to NSPanel for better fullscreen app behavior
            #[cfg(target_os = "macos")]
            {
                use tauri_nspanel::{CollectionBehavior, PanelLevel, WebviewWindowExt};
                match overlay.to_panel::<OverlayPanel>() {
                    Ok(panel) => {
                        // Configure panel to float above fullscreen apps
                        panel.set_level(PanelLevel::ScreenSaver.value());
                        panel.set_floating_panel(true);

                        // Set collection behavior to appear on all spaces including fullscreen
                        let behavior = CollectionBehavior::new()
                            .can_join_all_spaces()
                            .full_screen_auxiliary();
                        panel.set_collection_behavior(behavior.value());

                        // Set style mask to non-activating panel
                        let style = tauri_nspanel::StyleMask::empty().nonactivating_panel();
                        panel.set_style_mask(style.value());

                        log::info!("[NSPanel] Successfully converted overlay to NSPanel");
                    }
                    Err(e) => {
                        log::error!("[NSPanel] Failed to convert overlay to NSPanel: {:?}", e);
                    }
                }
            }

            // Position overlay based on saved setting
            if let Ok(Some(monitor)) = overlay.current_monitor() {
                let size = monitor.size();
                let pos = monitor.position();
                let scale = monitor.scale_factor();
                // Use PHYSICAL coordinates for initial placement to avoid DPI conversion
                // edge cases on Windows.
                let screen_width_px = size.width as f64;
                let screen_height_px = size.height as f64;
                let origin_x_px = pos.x as f64;
                let origin_y_px = pos.y as f64;

                // Estimate initial widget size (before content loads). The frontend will
                // auto-resize after mount, but using a closer estimate prevents off-screen drift.
                let window_width_px = (224.0 * scale).round();
                let window_height_px = (56.0 * scale).round();
                let margin_px = (50.0 * scale).round();

                let widget_position: String = get_setting_from_store(
                    app.handle(),
                    "widget_position",
                    "bottom-center".to_string(),
                );

                let (x_px, y_px) = match widget_position.as_str() {
                    "top-left" => (origin_x_px + margin_px, origin_y_px + margin_px),
                    "top-center" => (
                        origin_x_px + (screen_width_px - window_width_px) / 2.0,
                        origin_y_px + margin_px,
                    ),
                    "top-right" => (
                        origin_x_px + screen_width_px - window_width_px - margin_px,
                        origin_y_px + margin_px,
                    ),
                    "center" => (
                        origin_x_px + (screen_width_px - window_width_px) / 2.0,
                        origin_y_px + (screen_height_px - window_height_px) / 2.0,
                    ),
                    "bottom-left" => (
                        origin_x_px + margin_px,
                        origin_y_px + screen_height_px - window_height_px - margin_px,
                    ),
                    "bottom-center" => (
                        origin_x_px + (screen_width_px - window_width_px) / 2.0,
                        origin_y_px + screen_height_px - window_height_px - margin_px,
                    ),
                    _ => (
                        // "bottom-right" or unknown
                        origin_x_px + screen_width_px - window_width_px - margin_px,
                        origin_y_px + screen_height_px - window_height_px - margin_px,
                    ),
                };

                let _ = overlay.set_position(tauri::Position::Physical(tauri::PhysicalPosition {
                    x: x_px.round() as i32,
                    y: y_px.round() as i32,
                }));

                // Quick Ask should behave like an overlay: cover the current monitor so the
                // panel can sit bottom-center and the user can dismiss by clicking anywhere.
                // Use PHYSICAL coordinates to avoid DPI conversion edge cases on Windows.
                let _ = quick_ask.set_size(tauri::Size::Physical(tauri::PhysicalSize {
                    width: size.width,
                    height: size.height,
                }));
                let _ = quick_ask.set_position(tauri::Position::Physical(tauri::PhysicalPosition {
                    x: pos.x,
                    y: pos.y,
                }));
            }

            // Set initial overlay visibility based on saved settings
            #[cfg(desktop)]
            {
                let overlay_mode: String = get_setting_from_store(
                    app.handle(),
                    "overlay_mode",
                    "recording_only".to_string(),
                );
                match overlay_mode.as_str() {
                    "never" | "recording_only" => {
                        let _ = overlay.hide();
                    }
                    _ => {} // "always" - keep visible (default)
                }
            }

            // Setup system tray
            setup_tray(app.handle())?;

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Handle modifier-only key events (Windows-only).
///
/// This is used for hotkeys like "AltRight" with no modifiers.
#[cfg(all(desktop, target_os = "windows"))]
pub(crate) fn handle_modifier_key_event(app: &AppHandle, key: &str, is_down: bool) {
    let state = app.state::<AppState>();

    // Determine which (if any) configured hotkey uses this modifier-only key.
    let toggle_hotkey = get_hotkey_from_store(app, "toggle_hotkey", HotkeyConfig::default_toggle_opt);
    let hold_hotkey = get_hotkey_from_store(app, "hold_hotkey", HotkeyConfig::default_hold);
    let paste_last_hotkey = get_hotkey_from_store(app, "paste_last_hotkey", HotkeyConfig::default_paste_last);
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
                .or_else(|| HotkeyConfig::default_quick_ask()),
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
    let is_paste_last = paste_last_hotkey.as_ref().is_some_and(matches_modifier_only);
    let is_retry = retry_hotkey.as_ref().is_some_and(matches_modifier_only);
    let is_quick_ask_hold = quick_ask_hold_hotkey.as_ref().is_some_and(matches_modifier_only);
    let is_quick_ask_toggle = quick_ask_toggle_hotkey.as_ref().is_some_and(matches_modifier_only);

    if !(is_toggle || is_hold || is_paste_last || is_retry || is_quick_ask_hold || is_quick_ask_toggle) {
        return;
    }

    // Get current settings from store (mirrors handle_shortcut_event behavior)
    let sound_enabled: bool = get_setting_from_store(app, "sound_enabled", true);
    let audio_cue_raw: String = get_setting_from_store(app, "audio_cue", "kolboo".to_string());
    let audio_cue = audio::AudioCue::from_str(&audio_cue_raw);
    let playing_audio_handling: PlayingAudioHandling = get_playing_audio_handling(app);
    let audio_mute_manager = app.try_state::<AudioMuteManager>();

    if is_toggle {
        // Toggle mode: action happens on key release (debounced)
        if is_down {
            state.toggle_key_held.swap(true, Ordering::SeqCst);
        } else {
            if state.toggle_key_held.swap(false, Ordering::SeqCst) {
                let pipeline_state = app
                    .try_state::<pipeline::SharedPipeline>()
                    .map(|p| p.state());

                // Do not allow starting a new capture while we are processing a previous one.
                if matches!(
                    pipeline_state,
                    Some(pipeline::PipelineState::Transcribing | pipeline::PipelineState::Rewriting)
                ) {
                    log::info!(
                        "Toggle(AltRight) ignored (pipeline busy: {:?})",
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
                    stop_recording(
                        app,
                        &state,
                        sound_enabled,
                        audio_cue,
                        &audio_mute_manager,
                        playing_audio_handling,
                        "Toggle(AltRight)",
                    );
                } else if can_start {
                    start_recording(
                        app,
                        &state,
                        sound_enabled,
                        audio_cue,
                        &audio_mute_manager,
                        playing_audio_handling,
                        "Toggle(AltRight)",
                    );
                } else {
                    log::info!(
                        "Toggle(AltRight) ignored (pipeline state: {:?})",
                        pipeline_state
                    );
                }
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
                        "Hold(AltRight)",
                    );
                }
            }
        } else {
            if state.ptt_key_held.swap(false, Ordering::SeqCst) {
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
                        "Hold(AltRight)",
                    );
                }
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

        if !state.paste_key_held.swap(false, Ordering::SeqCst) {
            return;
        }

        // Key released - output based on configured mode
        log::info!("OutputLast(AltRight): outputting last transcription");

        let output_mode_str: String =
            get_setting_from_store(app, "output_mode", "paste".to_string());
        let output_mode = commands::text::OutputMode::from_str(&output_mode_str);
        let output_hit_enter: bool = get_setting_from_store(app, "output_hit_enter", false);

        let history_storage = app.state::<HistoryStorage>();

        if let Ok(entries) = history_storage.get_all(Some(1)) {
            if let Some(entry) = entries.first() {
                if let Err(e) = commands::text::output_text_with_mode(
                    &entry.text,
                    output_mode,
                    output_hit_enter,
                ) {
                    log::error!("Failed to output last transcription: {}", e);
                }
            } else {
                log::info!("OutputLast(AltRight): no history entries available");
            }
        }

        return;
    }

    if is_retry {
        // Retry-last-recording: action on release (debounced)
        if is_down {
            state.retry_key_held.swap(true, Ordering::SeqCst);
            return;
        }
        if !state.retry_key_held.swap(false, Ordering::SeqCst) {
            return;
        }

        log::info!("Retry(AltRight): retrying last recording");
        spawn_retry_last_recording_and_output(app, "Retry(AltRight)");

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
                    Some(pipeline::PipelineState::Transcribing | pipeline::PipelineState::Rewriting)
                ) {
                    log::info!(
                        "QuickAskHold(AltRight) ignored (pipeline busy: {:?})",
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
                        "QuickAskHold(AltRight)",
                    );

                    let is_recording = app
                        .try_state::<pipeline::SharedPipeline>()
                        .map(|p| p.state() == pipeline::PipelineState::Recording)
                        .unwrap_or(false);
                    if !is_recording {
                        state.quick_ask_session_active.store(false, Ordering::SeqCst);
                    }
                }
            }
        } else {
            if state.quick_ask_key_held.swap(false, Ordering::SeqCst) {
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
                        "QuickAskHold(AltRight)",
                    );
                } else {
                    state.quick_ask_session_active.store(false, Ordering::SeqCst);
                }
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

        if !state.quick_ask_toggle_key_held.swap(false, Ordering::SeqCst) {
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
                "QuickAskToggle(AltRight) ignored (pipeline busy: {:?})",
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
                    "QuickAskToggle(AltRight)",
                );
            } else {
                log::info!(
                    "QuickAskToggle(AltRight) stop ignored (active session is not Quick Ask)"
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
                "QuickAskToggle(AltRight)",
            );

            let is_recording = app
                .try_state::<pipeline::SharedPipeline>()
                .map(|p| p.state() == pipeline::PipelineState::Recording)
                .unwrap_or(false);
            if !is_recording {
                state.quick_ask_session_active.store(false, Ordering::SeqCst);
            }
        }

        return;
    }
}

fn setup_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    fn show_main_window(app: &AppHandle, source: &str) {
        let Some(window) = app.get_webview_window("main") else {
            log::error!("{source}: main window not found (was it closed?) - recreating");

            // Recreate main window if it was previously closed/destroyed.
            // NOTE: Creating windows from synchronous event handlers can deadlock on Windows,
            // so we do it on a separate thread.
            let app_handle = app.clone();
            let source = source.to_string();
            std::thread::spawn(move || {
                log::info!("{source}: creating main window");
                match tauri::WebviewWindowBuilder::new(
                    &app_handle,
                    "main",
                    tauri::WebviewUrl::App("index.html".into()),
                )
                .title("Kolboo")
                .inner_size(1280.0, 720.0)
                .resizable(true)
                .center()
                .build()
                {
                    Ok(w) => {
                        let _ = w.unminimize();
                        let _ = w.show();
                        let _ = w.set_focus();
                        log::info!("{source}: main window created and shown");
                    }
                    Err(e) => {
                        log::error!("{source}: failed to create main window: {e}");
                    }
                }
            });
            return;
        };

        let visible_before = window.is_visible().ok();
        log::info!("{source}: attempting to show main window (visible_before={visible_before:?})");

        if let Err(e) = window.unminimize() {
            log::warn!("{source}: window.unminimize() failed: {e}");
        }
        if let Err(e) = window.show() {
            log::warn!("{source}: window.show() failed: {e}");
        }
        // If the window was previously on a disconnected monitor, showing/focusing may succeed
        // but the window can still be effectively invisible. Centering is a good recovery.
        if let Err(e) = window.center() {
            log::warn!("{source}: window.center() failed: {e}");
        }
        // A brief always-on-top toggle can help bring the window above other windows on Windows.
        let _ = window.set_always_on_top(true);
        if let Err(e) = window.set_focus() {
            log::warn!("{source}: window.set_focus() failed: {e}");
        }
        let _ = window.set_always_on_top(false);

        let visible_after = window.is_visible().ok();
        log::info!("{source}: done (visible_after={visible_after:?})");
    }

    let show_item = MenuItem::with_id(app, "show", "Show Window", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show_item, &quit_item])?;

    // Use the same tray icon everywhere (full-color, brand-consistent).
    // NOTE: Some platforms (notably macOS) have UI conventions around template icons,
    // but we intentionally keep it consistent with the rest of the app branding.
    let icon_bytes = include_bytes!("../icons/32x32.png");
    let icon = tauri::image::Image::from_bytes(icon_bytes)?;

    let tray = TrayIconBuilder::new()
        .icon(icon)
        .icon_as_template(false)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => {
                log::info!("Tray menu: show");
                show_main_window(app, "tray-menu-show");
            }
            "quit" => {
                log::info!("Tray menu: quit");
                // Emit disconnect request to frontend before exiting
                if let Some(window) = app.get_webview_window("overlay") {
                    let _ = window.emit("request-disconnect", ());
                }
                // Give frontend time to disconnect gracefully
                std::thread::sleep(std::time::Duration::from_millis(500));
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            match event {
                // On Windows, a double click triggers two `Click` events plus one `DoubleClick`.
                // If we "toggle" visibility on click, the two clicks cancel each other out and it
                // looks like double click does nothing.
                TrayIconEvent::DoubleClick {
                    button: MouseButton::Left,
                    ..
                }
                | TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } => {
                    log::info!("Tray icon: activate (left click/double-click)");
                    let app = tray.app_handle();
                    show_main_window(app, "tray-icon-activate");
                }
                _ => {}
            }
        })
        .build(app)?;

    // Keep the tray handle alive for the lifetime of the app.
    // This helps ensure click + menu callbacks keep firing reliably.
    #[cfg(desktop)]
    {
        let keepalive = app.state::<TrayKeepAlive>();
        keepalive.set(tray);
    }

    Ok(())
}

#[cfg(desktop)]
fn build_global_shortcut_plugin() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    // Just initialize the plugin - shortcuts will be registered in setup() after store is available
    tauri_plugin_global_shortcut::Builder::new().build()
}

/// Initialize the recording pipeline from settings in the store
#[cfg(desktop)]
fn initialize_pipeline_from_settings(app: &AppHandle) -> pipeline::SharedPipeline {
    use std::time::Duration;
    use std::collections::HashMap;

    // Read STT settings from store
    let stt_provider: String = get_setting_from_store(app, "stt_provider", "groq".to_string());

    // Read STT model from store
    let stt_model: Option<String> = get_setting_from_store(app, "stt_model", None);

    // Read global STT transcription prompt from store
    let stt_transcription_prompt: Option<String> =
        get_setting_from_store(app, "stt_transcription_prompt", None);

    // Read STT timeout from store (seconds)
    let stt_timeout_seconds_raw: f64 = get_setting_from_store(app, "stt_timeout_seconds", 10.0);
    let stt_timeout_seconds: f64 = if stt_timeout_seconds_raw.is_finite() && stt_timeout_seconds_raw > 0.0 {
        stt_timeout_seconds_raw
    } else {
        log::warn!(
            "Invalid stt_timeout_seconds value in store ({}); falling back to 10s",
            stt_timeout_seconds_raw
        );
        10.0
    };

    // Read all available STT API keys (for per-profile provider overrides at runtime)
    let mut stt_api_keys: HashMap<String, String> = HashMap::new();
    for provider in [
        "openai",
        "fireworks",
        "aquavoice",
        "groq",
        "elevenlabs",
        "assemblyai",
        "speechmatics",
        "deepgram",
    ] {
        let key_name = format!("{}_api_key", provider);
        let key: String = get_setting_from_store(app, &key_name, String::new());
        if !key.is_empty() {
            stt_api_keys.insert(provider.to_string(), key);
        }
    }

    // Get the appropriate API key based on provider
    let stt_api_key: String = match stt_provider.as_str() {
        "openai" => get_setting_from_store(app, "openai_api_key", String::new()),
        "fireworks" => get_setting_from_store(app, "fireworks_api_key", String::new()),
        "aquavoice" => get_setting_from_store(app, "aquavoice_api_key", String::new()),
        "groq" => get_setting_from_store(app, "groq_api_key", String::new()),
        "elevenlabs" => get_setting_from_store(app, "elevenlabs_api_key", String::new()),
        "assemblyai" => get_setting_from_store(app, "assemblyai_api_key", String::new()),
        "speechmatics" => get_setting_from_store(app, "speechmatics_api_key", String::new()),
        "deepgram" => get_setting_from_store(app, "deepgram_api_key", String::new()),
        _ => String::new(),
    };

    // Read VAD settings from store
    let vad_settings: settings::VadSettings =
        get_setting_from_store(app, "vad_settings", settings::VadSettings::default());

    // Read quiet-audio gate settings from store
    let default_pipeline_config = pipeline::PipelineConfig::default();

    let sanitize_quiet_duration_secs = |v: f32, fallback: f32| -> f32 {
        if !v.is_finite() {
            return fallback;
        }
        // UI clamps to a small range; keep this defensive in case the store was edited.
        if v < 0.0 {
            return fallback;
        }
        v.min(30.0)
    };

    let sanitize_quiet_dbfs_threshold = |v: f32, fallback: f32| -> f32 {
        if !v.is_finite() {
            return fallback;
        }
        // dBFS thresholds should be negative. If the store contains 0 or a positive number,
        // the quiet gate would (almost) always trigger.
        if v > -1.0 {
            return fallback;
        }
        v.clamp(-120.0, -1.0)
    };
    let quiet_audio_gate_enabled: bool = get_setting_from_store(
        app,
        "quiet_audio_gate_enabled",
        default_pipeline_config.quiet_audio_gate_enabled,
    );
    let quiet_audio_min_duration_secs: f32 = get_setting_from_store(
        app,
        "quiet_audio_min_duration_secs",
        default_pipeline_config.quiet_audio_min_duration_secs,
    );
    let quiet_audio_rms_dbfs_threshold: f32 = get_setting_from_store(
        app,
        "quiet_audio_rms_dbfs_threshold",
        default_pipeline_config.quiet_audio_rms_dbfs_threshold,
    );
    let quiet_audio_peak_dbfs_threshold: f32 = get_setting_from_store(
        app,
        "quiet_audio_peak_dbfs_threshold",
        default_pipeline_config.quiet_audio_peak_dbfs_threshold,
    );

    let quiet_audio_min_duration_secs =
        sanitize_quiet_duration_secs(quiet_audio_min_duration_secs, default_pipeline_config.quiet_audio_min_duration_secs);
    let quiet_audio_rms_dbfs_threshold = sanitize_quiet_dbfs_threshold(
        quiet_audio_rms_dbfs_threshold,
        default_pipeline_config.quiet_audio_rms_dbfs_threshold,
    );
    let quiet_audio_peak_dbfs_threshold = sanitize_quiet_dbfs_threshold(
        quiet_audio_peak_dbfs_threshold,
        default_pipeline_config.quiet_audio_peak_dbfs_threshold,
    );

    // Read experimental noise gate settings from store.
    // New key is `noise_gate_threshold_dbfs` (Option<f32>), with legacy fallback to
    // `noise_gate_strength` (0..=100 mapped to -75..-30 dBFS).
    let sanitize_noise_gate_threshold_dbfs = |v: f32| -> Option<f32> {
        if !v.is_finite() {
            return None;
        }
        Some(v.clamp(-75.0, -30.0))
    };

    let noise_gate_threshold_dbfs: Option<f32> = {
        let raw: Option<f32> = get_setting_from_store(app, "noise_gate_threshold_dbfs", None);
        if let Some(v) = raw.and_then(sanitize_noise_gate_threshold_dbfs) {
            Some(v)
        } else {
            // Legacy fallback
            let strength_raw: u64 = get_setting_from_store(app, "noise_gate_strength", 0u64);
            let strength = (strength_raw.min(100) as u8) as f32;
            if strength <= 0.0 {
                None
            } else {
                let t = strength / 100.0;
                Some((-75.0 + (-30.0 + 75.0) * t).clamp(-75.0, -30.0))
            }
        }
    };

    // Read voice-pickup preprocessing toggles from store.
    let audio_downmix_to_mono: bool = get_setting_from_store(
        app,
        "audio_downmix_to_mono",
        default_pipeline_config.audio_downmix_to_mono,
    );
    let audio_resample_to_16khz: bool = get_setting_from_store(
        app,
        "audio_resample_to_16khz",
        default_pipeline_config.audio_resample_to_16khz,
    );
    let audio_highpass_enabled: bool = get_setting_from_store(
        app,
        "audio_highpass_enabled",
        default_pipeline_config.audio_highpass_enabled,
    );
    let audio_agc_enabled: bool = get_setting_from_store(
        app,
        "audio_agc_enabled",
        default_pipeline_config.audio_agc_enabled,
    );
    let audio_noise_suppression_enabled: bool = get_setting_from_store(
        app,
        "audio_noise_suppression_enabled",
        default_pipeline_config.audio_noise_suppression_enabled,
    );

    let quiet_audio_require_speech: bool = get_setting_from_store(
        app,
        "quiet_audio_require_speech",
        default_pipeline_config.quiet_audio_require_speech,
    );

    // Read LLM settings from store
    let rewrite_llm_enabled: bool = get_setting_from_store(app, "rewrite_llm_enabled", false);
    let llm_provider_setting: Option<String> = get_setting_from_store(app, "llm_provider", None);
    let llm_model_setting: Option<String> = get_setting_from_store(app, "llm_model", None);

    // Optional provider-specific reasoning/thinking knobs.
    // These are ignored unless the selected provider/model supports them.
    let openai_reasoning_effort: Option<String> =
        get_setting_from_store(app, "openai_reasoning_effort", None);
    let gemini_thinking_budget: Option<i64> =
        get_setting_from_store(app, "gemini_thinking_budget", None);
    let gemini_thinking_level: Option<String> =
        get_setting_from_store(app, "gemini_thinking_level", None);
    let anthropic_thinking_budget: Option<i64> =
        get_setting_from_store(app, "anthropic_thinking_budget", None);

    // If the user never explicitly selected a model, treat "default" as the provider's
    // concrete default model so request logs can display the exact model used.
    let llm_provider_effective = llm_provider_setting
        .clone()
        .unwrap_or_else(|| "openai".to_string());
    let llm_model_effective: Option<String> = llm_model_setting.or_else(|| {
        if rewrite_llm_enabled {
            llm::default_llm_model_for_provider(llm_provider_effective.as_str())
                .map(|m| m.to_string())
        } else {
            None
        }
    });

    let llm_api_key: String = llm_provider_setting
        .as_deref()
        .map(|provider| {
            let key_name = format!("{}_api_key", provider);
            get_setting_from_store(app, &key_name, String::new())
        })
        .unwrap_or_default();

    // IMPORTANT: `enabled` is only the global toggle. The effective provider/key is resolved
    // per transcription based on the active profile.
    let llm_enabled = rewrite_llm_enabled;

    // Read all available LLM API keys (for per-profile provider overrides at runtime)
    let mut llm_api_keys: HashMap<String, String> = HashMap::new();
    for provider in [
        "openai",
        "fireworks",
        "anthropic",
        "groq",
        "gemini",
        "cohere",
        "cerebras",
    ] {
        let key_name = format!("{}_api_key", provider);
        let key: String = get_setting_from_store(app, &key_name, String::new());
        if !key.is_empty() {
            llm_api_keys.insert(provider.to_string(), key);
        }
    }

    // Read rewrite prompt sections + per-program profiles from store
    //
    // `cleanup_prompt_sections` is treated as overrides on top of the built-in defaults.
    // Each program profile can further override individual sections.
    let cleanup_prompt_sections: Option<settings::CleanupPromptSectionsSetting> =
        get_setting_from_store(app, "cleanup_prompt_sections", None);
    let base_prompts: llm::PromptSections = cleanup_prompt_sections
        .as_ref()
        .map(|o| o.apply_to(&llm::PromptSections::default()))
        .unwrap_or_else(llm::PromptSections::default);

    let rewrite_program_prompt_profiles: Vec<settings::RewriteProgramPromptProfile> =
        get_setting_from_store(app, "rewrite_program_prompt_profiles", Vec::new());

    let program_prompt_profiles: Vec<llm::ProgramPromptProfile> = rewrite_program_prompt_profiles
        .into_iter()
        .map(|p| {
            let profile_prompts = p
                .cleanup_prompt_sections
                .as_ref()
                .map(|o| o.apply_to(&base_prompts))
                .unwrap_or_else(|| base_prompts.clone());

            let presets = p
                .presets
                .into_iter()
                .map(|preset| {
                    let preset_prompts = preset
                        .cleanup_prompt_sections
                        .as_ref()
                        .map(|o| o.apply_to(&profile_prompts))
                        .unwrap_or_else(|| profile_prompts.clone());

                    llm::ProgramPreset {
                        id: preset.id,
                        name: preset.name,
                        description: preset.description,
                        routing_hints: preset.routing_hints,
                        prompts: preset_prompts,
                        rewrite_llm_enabled: preset.rewrite_llm_enabled,
                        stt_provider: preset.stt_provider,
                        stt_model: preset.stt_model,
                        stt_timeout_seconds: preset.stt_timeout_seconds,
                        llm_provider: preset.llm_provider,
                        llm_model: preset.llm_model,
                        openai_reasoning_effort: preset.openai_reasoning_effort,
                        gemini_thinking_budget: preset.gemini_thinking_budget,
                        gemini_thinking_level: preset.gemini_thinking_level,
                        anthropic_thinking_budget: preset.anthropic_thinking_budget,
                    }
                })
                .collect();

            llm::ProgramPromptProfile {
                id: p.id,
                name: p.name,
                program_paths: p.program_paths,
                prompts: profile_prompts,

                presets,
                default_preset_id: p.default_preset_id,
                default_preset_description: p.default_preset_description,
                active_preset_id: p.active_preset_id,
                router: p.router,

                rewrite_llm_enabled: p.rewrite_llm_enabled,
                stt_provider: p.stt_provider,
                stt_model: p.stt_model,
                stt_timeout_seconds: p.stt_timeout_seconds,
                llm_provider: p.llm_provider,
                llm_model: p.llm_model,
                openai_reasoning_effort: p.openai_reasoning_effort,
                gemini_thinking_budget: p.gemini_thinking_budget,
                gemini_thinking_level: p.gemini_thinking_level,
                anthropic_thinking_budget: p.anthropic_thinking_budget,

                quick_ask_provider: p.quick_ask_provider,
                quick_ask_model: p.quick_ask_model,
                quick_ask_system_prompt: p.quick_ask_system_prompt,
                quick_ask_openai_reasoning_effort: p.quick_ask_openai_reasoning_effort,
                quick_ask_gemini_thinking_budget: p.quick_ask_gemini_thinking_budget,
                quick_ask_gemini_thinking_level: p.quick_ask_gemini_thinking_level,
                quick_ask_anthropic_thinking_budget: p.quick_ask_anthropic_thinking_budget,
            }
        })
        .collect();

    // Microphone selection (backend / CPAL).
    // Historical key name is `selected_mic_id` (originally from browser deviceId).
    // Newer builds store a backend-generated selection token (unique per enumerated device).
    // Older builds stored a CPAL device *name*.
    // The audio capture layer accepts both.
    let input_device_name: Option<String> = {
        let raw: Option<String> = get_setting_from_store(app, "selected_mic_id", None);
        raw.and_then(|s| {
            let t = s.trim().to_string();
            if t.is_empty() || t == "default" {
                None
            } else {
                Some(t)
            }
        })
    };

    // Microphone capture behavior.
    // - hot_mic_enabled: keep the input stream open while idle and maintain a rolling pre-roll
    // - hot_mic_pre_roll_ms: pre-roll duration (ms) to prepend at record start
    // - mic_auto_recover_enabled: watchdog the stream and attempt restart on hangs/disconnects
    let hot_mic_enabled: bool = get_setting_from_store(app, "hot_mic_enabled", false);
    let hot_mic_pre_roll_ms: u32 =
        get_setting_from_store(app, "hot_mic_pre_roll_ms", 1500u32).min(5000);
    let mic_auto_recover_enabled: bool = get_setting_from_store(app, "mic_auto_recover_enabled", false);

    let proxy_settings: settings::ProxySettings =
        get_setting_from_store(app, "proxy_settings", settings::ProxySettings::default());

    let whisper_server_base_url: Option<String> = {
        let raw: Option<String> = get_setting_from_store(app, "whisper_server_base_url", None);
        raw.and_then(|s| {
            let t = s.trim().to_string();
            if t.is_empty() { None } else { Some(t) }
        })
    };

    #[cfg(feature = "local-whisper")]
    let whisper_model_path: Option<std::path::PathBuf> = {
        use crate::stt::WhisperModel;

        let model_id: String = get_setting_from_store(app, "local_whisper_model_id", "base".to_string());
        let model = match model_id.trim().to_lowercase().as_str() {
            "tiny" => WhisperModel::Tiny,
            "tinyen" | "tiny_en" | "tiny-en" => WhisperModel::TinyEn,
            "base" => WhisperModel::Base,
            "baseen" | "base_en" | "base-en" => WhisperModel::BaseEn,
            "small" => WhisperModel::Small,
            "smallen" | "small_en" | "small-en" => WhisperModel::SmallEn,
            "medium" => WhisperModel::Medium,
            "mediumen" | "medium_en" | "medium-en" => WhisperModel::MediumEn,
            "largev1" | "large_v1" | "large-v1" => WhisperModel::LargeV1,
            "largev2" | "large_v2" | "large-v2" => WhisperModel::LargeV2,
            "largev3" | "large_v3" | "large-v3" => WhisperModel::LargeV3,
            "largev3turbo" | "large_v3_turbo" | "large-v3-turbo" => WhisperModel::LargeV3Turbo,
            _ => WhisperModel::Base,
        };

        app.path().app_data_dir().ok().map(|app_data_dir| {
            let models_dir = app_data_dir.join("whisper-models");
            // Best-effort; if it fails we'll still return a path and LocalWhisperProvider
            // will error clearly if the model doesn't exist.
            let _ = std::fs::create_dir_all(&models_dir);
            models_dir.join(model.filename())
        })
    };

    let local_whisper_load_mode: String = get_setting_from_store(
        app,
        "local_whisper_load_mode",
        "manual".to_string(),
    );

    let config = pipeline::PipelineConfig {
        input_device_name,

        hot_mic_enabled,
        hot_mic_pre_roll_ms,
        mic_auto_recover_enabled,

        stt_provider,
        stt_api_key,
        stt_api_keys,
        stt_model,
        stt_transcription_prompt,
        whisper_server_base_url,
        max_duration_secs: 300.0,
        retry_config: stt::RetryConfig::default(),
        vad_config: vad_settings.to_vad_auto_stop_config(),
        transcription_timeout: Duration::from_secs_f64(stt_timeout_seconds),
        max_recording_bytes: 50 * 1024 * 1024, // 50MB

        proxy_settings,

        quiet_audio_gate_enabled,
        quiet_audio_min_duration_secs,
        quiet_audio_rms_dbfs_threshold,
        quiet_audio_peak_dbfs_threshold,

        noise_gate_threshold_dbfs,

        audio_downmix_to_mono,
        audio_resample_to_16khz,
        audio_highpass_enabled,
        audio_agc_enabled,
        audio_noise_suppression_enabled,

        quiet_audio_require_speech,

        llm_config: llm::LlmConfig {
            enabled: llm_enabled,
            provider: llm_provider_effective,
            api_key: llm_api_key,
            model: llm_model_effective,
            openai_reasoning_effort,
            gemini_thinking_budget,
            gemini_thinking_level,
            anthropic_thinking_budget,
            prompts: base_prompts,
            program_prompt_profiles,
            ..Default::default()
        },
        llm_api_keys,

        // Allow providers to enrich the active RequestLog with request/response payloads.
        request_log_store: app.try_state::<RequestLogStore>().map(|s| s.inner().clone()),

        #[cfg(feature = "local-whisper")]
        whisper_model_path,

        local_whisper_load_mode,
    };

    log::info!(
        "Initializing pipeline with STT provider: {}, VAD enabled: {}",
        config.stt_provider,
        config.vad_config.enabled
    );

    pipeline::SharedPipeline::new(config)
}

/// Register shortcuts from store settings (called from setup() after store plugin is available)
#[cfg(desktop)]
fn register_initial_shortcuts(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    use tauri_plugin_global_shortcut::GlobalShortcutExt;

    #[cfg(all(desktop, target_os = "windows"))]
    fn is_windows_modifier_only_hotkey(hk: &HotkeyConfig) -> bool {
        hk.modifiers.is_empty() && matches!(hk.key.as_str(), "AltRight")
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
                .or_else(|| HotkeyConfig::default_quick_ask()),
        };

        let toggle = get_hotkey_from_store(
            app,
            "quick_ask_toggle_hotkey",
            HotkeyConfig::default_quick_ask,
        );

        (hold, toggle)
    };

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
        if is_windows_modifier_only_hotkey(&hk) {
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
        if is_windows_modifier_only_hotkey(&hk) {
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
        if is_windows_modifier_only_hotkey(&hk) {
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
        if is_windows_modifier_only_hotkey(&hk) {
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
        if is_windows_modifier_only_hotkey(&hk) {
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
        if is_windows_modifier_only_hotkey(&hk) {
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
        let toggle_shortcut =
            <Shortcut as std::str::FromStr>::from_str(toggle_shortcut_str).map_err(|e| {
                failures.push(format!(
                    "Toggle ({}) => failed to parse shortcut: {:?}",
                    toggle_shortcut_str, e
                ));
            });
        if let Ok(toggle_shortcut) = toggle_shortcut {
            if let Err(e) =
                shortcut_manager.on_shortcut(toggle_shortcut, |app, shortcut, event| {
                    handle_shortcut_event(app, shortcut, &event);
                })
            {
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
            if let Err(e) = shortcut_manager.on_shortcut(
                paste_last_shortcut,
                |app, shortcut, event| {
                    handle_shortcut_event(app, shortcut, &event);
                },
            ) {
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
            if let Err(e) = shortcut_manager.on_shortcut(quick_ask_hold_shortcut, |app, shortcut, event| {
                handle_shortcut_event(app, shortcut, &event);
            }) {
                failures.push(format!("QuickAskHold ({}) => {}", quick_ask_hold_shortcut_str, e));
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
            if let Err(e) = shortcut_manager.on_shortcut(quick_ask_toggle_shortcut, |app, shortcut, event| {
                handle_shortcut_event(app, shortcut, &event);
            }) {
                failures.push(format!("QuickAskToggle ({}) => {}", quick_ask_toggle_shortcut_str, e));
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
