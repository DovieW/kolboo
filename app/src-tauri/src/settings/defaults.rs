#[cfg(desktop)]
use crate::pipeline;
#[cfg(desktop)]
use crate::secrets;
#[cfg(desktop)]
use serde_json::{json, Value};
#[cfg(desktop)]
use tauri::AppHandle;
#[cfg(desktop)]
use tauri_plugin_store::StoreExt;

#[cfg(desktop)]
use super::{migrations, HotkeyConfig, ProxySettings, VadSettings};

/// Ensure settings shown in the UI match what the backend will use.
///
/// The frontend often treats missing keys as "unset" and shows fallback defaults.
/// If the backend uses different fallbacks, this can cause confusing mismatches.
///
/// To prevent that, we eagerly seed `settings.json` with defaults for missing/null keys
/// (without overwriting any existing values).
#[cfg(desktop)]
pub(crate) fn ensure_default_settings(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let store = app.store("settings.json")?;

    let mut dirty = migrations::run_settings_migrations(&store)?;

    // Keep these defaults aligned with pipeline defaults / expected backend behavior.
    // We intentionally seed these so a brand new install has the same effective
    // settings that the pipeline will use at runtime (and what the UI shows).
    let default_pipeline_config = pipeline::PipelineConfig::default();

    let is_missing = |v: Option<Value>| -> bool { matches!(v, None | Some(Value::Null)) };

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

    // Settings schema version (for forward migrations).
    // Bump when adding migrations; keep TS/Rust/tests in sync.
    // Start at version 1 for new installs.
    dirty |= set_default(
        "settings_version",
        json!(migrations::SETTINGS_VERSION_LATEST),
        false,
    );

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
    dirty |= set_default("ollama_url", json!(null), false);
    dirty |= set_default("ocr_base_url", json!(null), false);
    dirty |= set_default("ocr_model", json!("lightonai/LightOnOCR-1B-1025"), false);
    dirty |= set_default("ocr_auth_mode", json!("none"), false);
    dirty |= set_default(
        "ocr_prompt",
        json!(default_pipeline_config.ocr_config.prompt),
        false,
    );
    dirty |= set_default(
        "ocr_max_tokens",
        json!(default_pipeline_config.ocr_config.max_tokens),
        false,
    );
    dirty |= set_default(
        "ocr_temperature",
        json!(default_pipeline_config.ocr_config.temperature),
        false,
    );
    dirty |= set_default(
        "ocr_top_p",
        json!(default_pipeline_config.ocr_config.top_p),
        false,
    );
    dirty |= set_default("ocr_request_timeout_ms", json!(2000u64), false);
    dirty |= set_default("ocr_context_max_chars", json!(8000u64), false);
    dirty |= set_default("ocr_auto_capture_timing", json!("on_start"), false);
    dirty |= set_default("ocr_hallucination_protection", json!(true), false);
    dirty |= set_default("ocr_hallucination_threshold", json!(2500u64), false);
    dirty |= set_default("ocr_resize_max_dimension", json!(0u32), false);
    dirty |= set_default("ocr_resize_filter", json!("nearest"), false);
    dirty |= set_default("rewrite_active_window_ocr_mode", json!("off"), false);
    dirty |= set_default("quick_replace_active_window_ocr_mode", json!("off"), false);
    dirty |= set_default("quick_ask_active_window_ocr_mode", json!("off"), false);

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
        serde_json::to_value(ProxySettings::default())?,
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
    // Transcription retention mode (amount or time). Used by history capping.
    dirty |= set_default("transcription_retention_mode", json!("time"), false);
    dirty |= set_default("transcription_retention_amount", json!(1000), false);
    // Recordings retention (amount or time). Time-based retention is UI-only today.
    dirty |= set_default("recordings_retention_mode", json!("amount"), false);
    dirty |= set_default("recordings_retention_amount", json!(1000), false);
    dirty |= set_default("recordings_retention_unit", json!("days"), false);
    dirty |= set_default("recordings_retention_value", json!(0.0), false);
    // Time-based retention for history/transcriptions. 0 = keep forever.
    dirty |= set_default("transcription_retention_days", json!(0), false);
    // New retention keys (unit+value) used by newer UI.
    // Keep legacy days key as well for backward compatibility.
    dirty |= set_default("transcription_retention_unit", json!("days"), false);
    dirty |= set_default("transcription_retention_value", json!(0.0), false);
    // When deleting old transcriptions, optionally also delete their .wav recordings.
    dirty |= set_default(
        "transcription_retention_delete_recordings",
        json!(false),
        false,
    );

    // Optional diagnostics: when true, emit additional hotkey debug events to the
    // in-app "System Events" panel (useful in release builds).
    dirty |= set_default("hotkey_debug_enabled", json!(false), false);

    // Persisted stats retention (usage/cost events).
    // These are stored on disk (unlike request logs which are in-memory).
    // 0 = keep forever.
    dirty |= set_default("stats_retention_unit", json!("days"), false);
    dirty |= set_default("stats_retention_value", json!(30.0), false);
    // Defensive cap (bytes). The pruning logic enforces this regardless of time settings.
    dirty |= set_default("stats_retention_max_bytes", json!(50_000_000u64), false);

    // Backups
    // Optional GitHub Gist id for push/pull backups. Null/absent means "not configured".
    dirty |= set_default("github_backup_gist_id", json!(null), true);
    dirty |= set_default("overlay_mode", json!("recording_only"), false);
    // Whether the overlay shows detailed phase text while processing
    // (e.g. "transcribing…", "routing…", "rewriting…"). When false, the overlay
    // uses a waveform animation instead.
    dirty |= set_default("overlay_show_detailed_loading", json!(false), false);
    // Which monitor overlay windows should appear on.
    // - main: primary monitor
    // - cursor: monitor containing cursor
    // - active_window: monitor containing the active/foreground window
    dirty |= set_default("overlay_monitor_target", json!("main"), false);
    dirty |= set_default("widget_position", json!("bottom-center"), false);
    // Whether clicking the window X exits the app or closes the main window to the tray.
    // - "exit_program": exit the application process
    // - "minimize_to_tray": close (destroy) the main window but keep the tray app running
    //   (the tray can recreate the main window on demand)
    // Legacy (migrated by the frontend normalizer):
    // - "close_window": previously meant "destroy the main window (tray can recreate it)".
    dirty |= set_default(
        "main_window_close_behavior",
        json!("minimize_to_tray"),
        false,
    );
    dirty |= set_default("output_mode", json!("paste"), false);
    dirty |= set_default("output_hit_enter", json!(false), false);
    // When true, output injection will not read the clipboard and will not attempt to restore it.
    // This reduces accidental exposure of clipboard contents at the cost of leaving output text
    // on the clipboard after paste.
    dirty |= set_default("output_clipboard_privacy_mode", json!(false), false);
    // When true, avoid pasting into sensitive targets (e.g., password fields).
    dirty |= set_default("output_smart_paste_protection", json!(false), false);
    dirty |= set_default("playing_audio_handling", json!("none"), false);
    dirty |= set_default("sound_enabled", json!(true), false);
    dirty |= set_default("rewrite_llm_enabled", json!(false), false);
    // When true, if there is highlighted text when transcription starts, treat the transcript
    // as an instruction to rewrite the selected text (Quick replace).
    dirty |= set_default("quick_replace_enabled", json!(false), false);

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
        // Implicit Default (no preset) should rewrite by default.
        "default_target_rewrite_llm_enabled": true,
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
                store.set(
                    "rewrite_program_prompt_profiles".to_string(),
                    Value::Array(arr),
                );
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

    if store.get("hotkey_shortcuts").is_none() {
        let mut cards: Vec<Value> = Vec::new();

        let push_card = |cards: &mut Vec<Value>, action: &str, value: Option<Value>| {
            let Some(Value::Object(_)) = value else {
                return;
            };

            cards.push(json!({
                "id": format!("seed-{}", action),
                "type": action,
                "hotkey": value,
            }));
        };

        push_card(cards.as_mut(), "toggle", store.get("toggle_hotkey"));
        push_card(cards.as_mut(), "hold", store.get("hold_hotkey"));
        push_card(cards.as_mut(), "paste_last", store.get("paste_last_hotkey"));
        push_card(cards.as_mut(), "retry", store.get("retry_hotkey"));

        let raw_quick_ask_hold = match store.get("quick_ask_hold_hotkey") {
            None => store.get("quick_ask_hotkey"),
            other => other,
        };
        push_card(cards.as_mut(), "quick_ask_hold", raw_quick_ask_hold);
        push_card(
            cards.as_mut(),
            "quick_ask_toggle",
            store.get("quick_ask_toggle_hotkey"),
        );

        store.set("hotkey_shortcuts".to_string(), Value::Array(cards));
        dirty = true;
    }

    // Quick Ask system prompt:
    // - missing key => seed the default
    // - explicit null => user disabled it, don't overwrite
    dirty |= set_default(
        "quick_ask_system_prompt",
        json!("Try to answer the question in a single word, sentence or paragraph when possible. Use markdown for formatting when necessary."),
        true,
    );

    // Quick Ask dismiss mode (manual or auto).
    dirty |= set_default("quick_ask_dismiss_mode", json!("manual"), false);

    // Quick Ask conversation history (ephemeral; stored in memory only).
    // These keys only control whether/how much in-memory history to attach to prompts.
    dirty |= set_default("quick_ask_conversation_history_enabled", json!(true), false);
    // How many previous Q/A turns to include when enabled.
    dirty |= set_default("quick_ask_conversation_history_count", json!(3), false);

    // Quick Ask highlighted selection context (disabled by default).
    // When false, we won't probe/capture the currently highlighted text for Quick Ask.
    dirty |= set_default("quick_ask_include_selected_text", json!(false), false);

    // VAD settings are used by the pipeline.
    dirty |= set_default(
        "vad_settings",
        serde_json::to_value(VadSettings::default())?,
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
    dirty |= set_default(
        "quiet_audio_rms_dbfs_threshold",
        json!(default_pipeline_config.quiet_audio_rms_dbfs_threshold),
        false,
    );
    dirty |= set_default(
        "quiet_audio_peak_dbfs_threshold",
        json!(default_pipeline_config.quiet_audio_peak_dbfs_threshold),
        false,
    );
    dirty |= set_default(
        "quiet_audio_require_speech",
        json!(default_pipeline_config.quiet_audio_require_speech),
        false,
    );

    // Stop-time preprocessing defaults.
    dirty |= set_default(
        "noise_gate_threshold_dbfs",
        json!(default_pipeline_config.noise_gate_threshold_dbfs),
        false,
    );
    dirty |= set_default(
        "audio_downmix_to_mono",
        json!(default_pipeline_config.audio_downmix_to_mono),
        false,
    );
    dirty |= set_default(
        "audio_resample_to_16khz",
        json!(default_pipeline_config.audio_resample_to_16khz),
        false,
    );
    dirty |= set_default(
        "audio_highpass_enabled",
        json!(default_pipeline_config.audio_highpass_enabled),
        false,
    );
    dirty |= set_default(
        "audio_agc_enabled",
        json!(default_pipeline_config.audio_agc_enabled),
        false,
    );
    dirty |= set_default(
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

    // Best-effort: migrate legacy plaintext API keys out of `settings.json`.
    // This runs on startup (after the store exists), and deletes the store copy
    // only after the key was written to secure storage.
    let _ = secrets::migrate_api_keys_from_store(app);

    Ok(())
}
