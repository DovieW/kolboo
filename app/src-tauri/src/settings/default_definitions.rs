use serde_json::{json, Value};

use super::{default_values, HotkeyConfig, VadSettings};
use crate::pipeline::PipelineConfig;

/// Controls when startup seeding may write a default into `settings.json`.
///
/// Keep this next to the default value definitions so future settings additions
/// must make the null-vs-missing decision explicitly instead of burying it in
/// `ensure_default_settings(...)` call-site trivia.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SeedRule {
    /// Seed when the key is absent or explicitly `null`.
    MissingOrNull,
    /// Seed only when the key is absent. Explicit `null` is meaningful.
    MissingOnly,
}

impl SeedRule {
    pub(crate) fn only_if_absent(self) -> bool {
        matches!(self, Self::MissingOnly)
    }
}

/// A canonical persisted setting default.
///
/// This is deliberately a small data record: `defaults.rs` owns the store write
/// mechanics, while this Module owns the key/default/null-semantics contract.
#[derive(Debug, Clone)]
pub(crate) struct SettingDefaultDefinition {
    pub(crate) key: &'static str,
    pub(crate) value: Value,
    pub(crate) seed_rule: SeedRule,
}

impl SettingDefaultDefinition {
    fn missing_or_null(key: &'static str, value: Value) -> Self {
        Self {
            key,
            value,
            seed_rule: SeedRule::MissingOrNull,
        }
    }

    fn missing_only(key: &'static str, value: Value) -> Self {
        Self {
            key,
            value,
            seed_rule: SeedRule::MissingOnly,
        }
    }
}

/// Return the startup defaults that can be seeded independently.
///
/// Defaults that require reading existing store state (for example inserting the
/// Default rewrite profile into a malformed/legacy profile array, or deriving
/// hotkey shortcut cards from hotkey settings) intentionally stay in
/// `defaults.rs`; they are migrations with behavior, not static definitions.
pub(crate) fn seedable_settings(
    default_pipeline_config: &PipelineConfig,
) -> Result<Vec<SettingDefaultDefinition>, serde_json::Error> {
    let definitions = vec![
        SettingDefaultDefinition::missing_or_null(
            "settings_version",
            default_values::default_settings_version_value(),
        ),
        SettingDefaultDefinition::missing_or_null(
            "policy_state",
            default_values::default_policy_state_value(),
        ),
        SettingDefaultDefinition::missing_or_null(
            "license_state",
            default_values::default_license_state_value(),
        ),
        SettingDefaultDefinition::missing_or_null(
            "token_exchange_trigger_set",
            default_values::default_token_exchange_trigger_set_value(),
        ),
        SettingDefaultDefinition::missing_or_null(
            "stt_provider",
            json!(default_values::DEFAULT_STT_PROVIDER),
        ),
        SettingDefaultDefinition::missing_or_null(
            "stt_language",
            json!(default_values::DEFAULT_STT_LANGUAGE),
        ),
        // Free-tier toggles are consumed by stats filtering. Keep them seedable so
        // existing installs gain the toggle without changing cost history rows.
        SettingDefaultDefinition::missing_or_null("cerebras_free_tier", json!(true)),
        SettingDefaultDefinition::missing_or_null("groq_free_tier", json!(true)),
        SettingDefaultDefinition::missing_or_null("cohere_free_tier", json!(true)),
        SettingDefaultDefinition::missing_or_null("assemblyai_free_tier", json!(true)),
        SettingDefaultDefinition::missing_or_null("speechmatics_free_tier", json!(true)),
        SettingDefaultDefinition::missing_or_null("stt_transcription_prompt", json!(null)),
        SettingDefaultDefinition::missing_or_null("whisper_server_base_url", json!(null)),
        SettingDefaultDefinition::missing_or_null("ollama_url", json!(null)),
        SettingDefaultDefinition::missing_or_null("ocr_base_url", json!(null)),
        SettingDefaultDefinition::missing_or_null(
            "ocr_model",
            json!(default_values::DEFAULT_OCR_MODEL),
        ),
        SettingDefaultDefinition::missing_or_null(
            "ocr_auth_mode",
            json!(default_values::DEFAULT_OCR_AUTH_MODE),
        ),
        SettingDefaultDefinition::missing_or_null(
            "ocr_prompt",
            json!(default_pipeline_config.ocr_config.prompt.clone()),
        ),
        SettingDefaultDefinition::missing_or_null(
            "ocr_max_tokens",
            json!(default_pipeline_config.ocr_config.max_tokens),
        ),
        SettingDefaultDefinition::missing_or_null(
            "ocr_temperature",
            json!(default_pipeline_config.ocr_config.temperature),
        ),
        SettingDefaultDefinition::missing_or_null(
            "ocr_top_p",
            json!(default_pipeline_config.ocr_config.top_p),
        ),
        SettingDefaultDefinition::missing_or_null(
            "ocr_request_timeout_ms",
            json!(default_values::DEFAULT_OCR_REQUEST_TIMEOUT_MS),
        ),
        SettingDefaultDefinition::missing_or_null(
            "ocr_context_max_chars",
            json!(default_values::DEFAULT_OCR_CONTEXT_MAX_CHARS),
        ),
        SettingDefaultDefinition::missing_or_null(
            "ocr_auto_capture_timing",
            json!(default_values::DEFAULT_OCR_AUTO_CAPTURE_TIMING),
        ),
        SettingDefaultDefinition::missing_or_null(
            "ocr_hallucination_protection",
            json!(default_values::DEFAULT_OCR_HALLUCINATION_PROTECTION),
        ),
        SettingDefaultDefinition::missing_or_null(
            "ocr_hallucination_threshold",
            json!(default_values::DEFAULT_OCR_HALLUCINATION_THRESHOLD),
        ),
        SettingDefaultDefinition::missing_or_null(
            "ocr_resize_max_dimension",
            json!(default_values::DEFAULT_OCR_RESIZE_MAX_DIMENSION),
        ),
        SettingDefaultDefinition::missing_or_null(
            "ocr_resize_filter",
            json!(default_values::DEFAULT_OCR_RESIZE_FILTER),
        ),
        SettingDefaultDefinition::missing_or_null(
            "rewrite_active_window_ocr_mode",
            json!(default_values::DEFAULT_ACTIVE_WINDOW_OCR_MODE),
        ),
        SettingDefaultDefinition::missing_or_null(
            "quick_replace_active_window_ocr_mode",
            json!(default_values::DEFAULT_ACTIVE_WINDOW_OCR_MODE),
        ),
        SettingDefaultDefinition::missing_or_null(
            "quick_ask_active_window_ocr_mode",
            json!(default_values::DEFAULT_ACTIVE_WINDOW_OCR_MODE),
        ),
        SettingDefaultDefinition::missing_or_null(
            "stt_timeout_seconds",
            json!(default_values::DEFAULT_STT_TIMEOUT_SECONDS),
        ),
        SettingDefaultDefinition::missing_or_null(
            "proxy_settings",
            default_values::default_proxy_settings_value()?,
        ),
        SettingDefaultDefinition::missing_or_null(
            "max_saved_recordings",
            json!(default_values::DEFAULT_MAX_SAVED_RECORDINGS),
        ),
        SettingDefaultDefinition::missing_or_null(
            "request_logs_retention_mode",
            json!(default_values::DEFAULT_REQUEST_LOGS_RETENTION_MODE),
        ),
        SettingDefaultDefinition::missing_or_null(
            "request_logs_retention_amount",
            json!(default_values::DEFAULT_REQUEST_LOGS_RETENTION_AMOUNT),
        ),
        SettingDefaultDefinition::missing_or_null(
            "request_logs_retention_days",
            json!(default_values::DEFAULT_REQUEST_LOGS_RETENTION_DAYS),
        ),
        SettingDefaultDefinition::missing_or_null(
            "request_logs_privacy_mode",
            json!(default_values::DEFAULT_REQUEST_LOGS_PRIVACY_MODE),
        ),
        SettingDefaultDefinition::missing_or_null(
            "transcription_retention_mode",
            json!(default_values::DEFAULT_TRANSCRIPTION_RETENTION_MODE),
        ),
        SettingDefaultDefinition::missing_or_null(
            "transcription_retention_amount",
            json!(default_values::DEFAULT_TRANSCRIPTION_RETENTION_AMOUNT),
        ),
        SettingDefaultDefinition::missing_or_null(
            "recordings_retention_mode",
            json!(default_values::DEFAULT_RECORDINGS_RETENTION_MODE),
        ),
        SettingDefaultDefinition::missing_or_null(
            "recordings_retention_amount",
            json!(default_values::DEFAULT_RECORDINGS_RETENTION_AMOUNT),
        ),
        SettingDefaultDefinition::missing_or_null(
            "recordings_retention_unit",
            json!(default_values::DEFAULT_RECORDINGS_RETENTION_UNIT),
        ),
        SettingDefaultDefinition::missing_or_null(
            "recordings_retention_value",
            json!(default_values::DEFAULT_RECORDINGS_RETENTION_VALUE),
        ),
        // Legacy days key remains seeded for backwards compatibility with older
        // retention readers even though newer UI prefers unit/value.
        SettingDefaultDefinition::missing_or_null("transcription_retention_days", json!(0)),
        SettingDefaultDefinition::missing_or_null(
            "transcription_retention_unit",
            json!(default_values::DEFAULT_TRANSCRIPTION_RETENTION_UNIT),
        ),
        SettingDefaultDefinition::missing_or_null(
            "transcription_retention_value",
            json!(default_values::DEFAULT_TRANSCRIPTION_RETENTION_VALUE),
        ),
        SettingDefaultDefinition::missing_or_null(
            "transcription_retention_delete_recordings",
            json!(default_values::DEFAULT_TRANSCRIPTION_RETENTION_DELETE_RECORDINGS),
        ),
        SettingDefaultDefinition::missing_or_null(
            "hotkey_debug_enabled",
            json!(default_values::DEFAULT_HOTKEY_DEBUG_ENABLED),
        ),
        SettingDefaultDefinition::missing_or_null(
            "stats_retention_unit",
            json!(default_values::DEFAULT_STATS_RETENTION_UNIT),
        ),
        SettingDefaultDefinition::missing_or_null(
            "stats_retention_value",
            json!(default_values::DEFAULT_STATS_RETENTION_VALUE),
        ),
        SettingDefaultDefinition::missing_or_null(
            "stats_retention_max_bytes",
            json!(default_values::DEFAULT_STATS_RETENTION_MAX_BYTES),
        ),
        SettingDefaultDefinition::missing_only("github_backup_gist_id", json!(null)),
        SettingDefaultDefinition::missing_or_null(
            "overlay_mode",
            json!(default_values::DEFAULT_OVERLAY_MODE),
        ),
        SettingDefaultDefinition::missing_or_null(
            "overlay_show_detailed_loading",
            json!(default_values::DEFAULT_OVERLAY_SHOW_DETAILED_LOADING),
        ),
        SettingDefaultDefinition::missing_or_null(
            "overlay_monitor_target",
            json!(default_values::DEFAULT_OVERLAY_MONITOR_TARGET),
        ),
        SettingDefaultDefinition::missing_or_null(
            "widget_position",
            json!(default_values::DEFAULT_WIDGET_POSITION),
        ),
        SettingDefaultDefinition::missing_or_null(
            "main_window_close_behavior",
            json!(default_values::DEFAULT_MAIN_WINDOW_CLOSE_BEHAVIOR),
        ),
        SettingDefaultDefinition::missing_or_null(
            "output_mode",
            json!(default_values::DEFAULT_OUTPUT_MODE),
        ),
        SettingDefaultDefinition::missing_or_null(
            "output_hit_enter",
            json!(default_values::DEFAULT_OUTPUT_HIT_ENTER),
        ),
        SettingDefaultDefinition::missing_or_null(
            "stt_live_output",
            json!(default_values::DEFAULT_STT_LIVE_OUTPUT),
        ),
        SettingDefaultDefinition::missing_or_null(
            "stt_simulated_streaming",
            json!(default_values::DEFAULT_STT_SIMULATED_STREAMING),
        ),
        SettingDefaultDefinition::missing_or_null(
            "output_clipboard_privacy_mode",
            json!(default_values::DEFAULT_OUTPUT_CLIPBOARD_PRIVACY_MODE),
        ),
        SettingDefaultDefinition::missing_or_null(
            "output_smart_paste_protection",
            json!(default_values::DEFAULT_OUTPUT_SMART_PASTE_PROTECTION),
        ),
        SettingDefaultDefinition::missing_or_null(
            "playing_audio_handling",
            json!(default_values::DEFAULT_PLAYING_AUDIO_HANDLING),
        ),
        SettingDefaultDefinition::missing_or_null(
            "sound_enabled",
            json!(default_values::DEFAULT_SOUND_ENABLED),
        ),
        SettingDefaultDefinition::missing_or_null(
            "rewrite_llm_enabled",
            json!(default_values::DEFAULT_REWRITE_LLM_ENABLED),
        ),
        SettingDefaultDefinition::missing_or_null(
            "quick_replace_enabled",
            json!(default_values::DEFAULT_QUICK_REPLACE_ENABLED),
        ),
        // Hotkeys: explicit null means disabled, so only seed absent keys.
        SettingDefaultDefinition::missing_only(
            "toggle_hotkey",
            serde_json::to_value(HotkeyConfig::default_toggle())?,
        ),
        SettingDefaultDefinition::missing_only("hold_hotkey", json!(null)),
        SettingDefaultDefinition::missing_only("paste_last_hotkey", json!(null)),
        SettingDefaultDefinition::missing_only("retry_hotkey", json!(null)),
        SettingDefaultDefinition::missing_only("quick_ask_hotkey", json!(null)),
        SettingDefaultDefinition::missing_only("quick_ask_hold_hotkey", json!(null)),
        SettingDefaultDefinition::missing_only("quick_ask_toggle_hotkey", json!(null)),
        // Quick Ask system prompt: explicit null disables the default prompt.
        SettingDefaultDefinition::missing_only(
            "quick_ask_system_prompt",
            json!("Try to answer the question in a single word, sentence or paragraph when possible. Use markdown for formatting when necessary."),
        ),
        SettingDefaultDefinition::missing_or_null(
            "quick_ask_dismiss_mode",
            json!(default_values::DEFAULT_QUICK_ASK_DISMISS_MODE),
        ),
        SettingDefaultDefinition::missing_or_null(
            "quick_ask_conversation_history_enabled",
            json!(default_values::DEFAULT_QUICK_ASK_CONVERSATION_HISTORY_ENABLED),
        ),
        SettingDefaultDefinition::missing_or_null(
            "quick_ask_conversation_history_count",
            json!(default_values::DEFAULT_QUICK_ASK_CONVERSATION_HISTORY_COUNT),
        ),
        SettingDefaultDefinition::missing_or_null(
            "quick_ask_include_selected_text",
            json!(default_values::DEFAULT_QUICK_ASK_INCLUDE_SELECTED_TEXT),
        ),
        SettingDefaultDefinition::missing_or_null(
            "vad_settings",
            serde_json::to_value(VadSettings::default())?,
        ),
        SettingDefaultDefinition::missing_or_null(
            "hot_mic_enabled",
            json!(default_values::DEFAULT_HOT_MIC_ENABLED),
        ),
        SettingDefaultDefinition::missing_or_null(
            "hot_mic_pre_roll_ms",
            json!(default_values::DEFAULT_HOT_MIC_PRE_ROLL_MS),
        ),
        SettingDefaultDefinition::missing_or_null(
            "mic_auto_recover_enabled",
            json!(default_values::DEFAULT_MIC_AUTO_RECOVER_ENABLED),
        ),
        SettingDefaultDefinition::missing_or_null(
            "quiet_audio_gate_enabled",
            json!(default_pipeline_config.quiet_audio_gate_enabled),
        ),
        SettingDefaultDefinition::missing_or_null(
            "quiet_audio_min_duration_secs",
            json!(default_pipeline_config.quiet_audio_min_duration_secs),
        ),
        SettingDefaultDefinition::missing_or_null(
            "quiet_audio_rms_dbfs_threshold",
            json!(default_pipeline_config.quiet_audio_rms_dbfs_threshold),
        ),
        SettingDefaultDefinition::missing_or_null(
            "quiet_audio_peak_dbfs_threshold",
            json!(default_pipeline_config.quiet_audio_peak_dbfs_threshold),
        ),
        SettingDefaultDefinition::missing_or_null(
            "quiet_audio_require_speech",
            json!(default_pipeline_config.quiet_audio_require_speech),
        ),
        SettingDefaultDefinition::missing_or_null(
            "noise_gate_threshold_dbfs",
            json!(default_pipeline_config.noise_gate_threshold_dbfs),
        ),
        SettingDefaultDefinition::missing_or_null(
            "audio_downmix_to_mono",
            json!(default_pipeline_config.audio_downmix_to_mono),
        ),
        SettingDefaultDefinition::missing_or_null(
            "audio_resample_to_16khz",
            json!(default_pipeline_config.audio_resample_to_16khz),
        ),
        SettingDefaultDefinition::missing_or_null(
            "audio_highpass_enabled",
            json!(default_pipeline_config.audio_highpass_enabled),
        ),
        SettingDefaultDefinition::missing_or_null(
            "audio_agc_enabled",
            json!(default_pipeline_config.audio_agc_enabled),
        ),
        SettingDefaultDefinition::missing_or_null(
            "audio_noise_suppression_enabled",
            json!(default_pipeline_config.audio_noise_suppression_enabled),
        ),
    ];

    #[cfg(feature = "local-whisper")]
    let definitions = {
        let mut definitions = definitions;
        definitions.push(SettingDefaultDefinition::missing_or_null(
            "local_whisper_model_id",
            json!(default_values::DEFAULT_LOCAL_WHISPER_MODEL_ID),
        ));
        definitions.push(SettingDefaultDefinition::missing_or_null(
            "local_whisper_load_mode",
            json!(default_values::DEFAULT_LOCAL_WHISPER_LOAD_MODE),
        ));
        definitions
    };

    Ok(definitions)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn definition_for<'a>(
        definitions: &'a [SettingDefaultDefinition],
        key: &str,
    ) -> &'a SettingDefaultDefinition {
        definitions
            .iter()
            .find(|definition| definition.key == key)
            .unwrap_or_else(|| panic!("missing default definition for {key}"))
    }

    #[test]
    fn seedable_settings_have_unique_keys() {
        let definitions = seedable_settings(&PipelineConfig::default()).expect("defaults");
        let mut seen = HashSet::new();

        for definition in definitions {
            assert!(
                seen.insert(definition.key),
                "duplicate key {}",
                definition.key
            );
        }
    }

    #[test]
    fn seed_rules_preserve_explicit_null_contracts() {
        let definitions = seedable_settings(&PipelineConfig::default()).expect("defaults");

        assert_eq!(
            definition_for(&definitions, "stt_provider").seed_rule,
            SeedRule::MissingOrNull
        );
        assert_eq!(
            definition_for(&definitions, "toggle_hotkey").seed_rule,
            SeedRule::MissingOnly
        );
        assert_eq!(
            definition_for(&definitions, "github_backup_gist_id").seed_rule,
            SeedRule::MissingOnly
        );
        assert_eq!(
            definition_for(&definitions, "quick_ask_system_prompt").seed_rule,
            SeedRule::MissingOnly
        );
    }

    #[test]
    fn seedable_settings_expose_runtime_defaults() {
        let pipeline_config = PipelineConfig::default();
        let definitions = seedable_settings(&pipeline_config).expect("defaults");

        assert_eq!(
            definition_for(&definitions, "stt_provider").value,
            json!(default_values::DEFAULT_STT_PROVIDER)
        );
        assert_eq!(
            definition_for(&definitions, "ocr_model").value,
            json!(default_values::DEFAULT_OCR_MODEL)
        );
        assert_eq!(
            definition_for(&definitions, "quiet_audio_gate_enabled").value,
            json!(pipeline_config.quiet_audio_gate_enabled)
        );

        // OCR defaults are consumed both when seeding settings.json and when
        // building an in-memory PipelineConfig. Keep this table intentionally
        // explicit so a future OCR setting cannot drift silently between the
        // persisted Settings View and the runtime pipeline fallback.
        assert_eq!(
            definition_for(&definitions, "ocr_model").value,
            json!(pipeline_config.ocr_config.model)
        );
        assert_eq!(
            definition_for(&definitions, "ocr_auth_mode").value,
            json!(pipeline_config.ocr_config.auth_mode)
        );
        assert_eq!(
            definition_for(&definitions, "ocr_prompt").value,
            json!(pipeline_config.ocr_config.prompt)
        );
        assert_eq!(
            definition_for(&definitions, "ocr_max_tokens").value,
            json!(pipeline_config.ocr_config.max_tokens)
        );
        assert_eq!(
            definition_for(&definitions, "ocr_temperature").value,
            json!(pipeline_config.ocr_config.temperature)
        );
        assert_eq!(
            definition_for(&definitions, "ocr_top_p").value,
            json!(pipeline_config.ocr_config.top_p)
        );
        assert_eq!(
            definition_for(&definitions, "ocr_request_timeout_ms").value,
            json!(pipeline_config.ocr_config.request_timeout_ms)
        );
        assert_eq!(
            definition_for(&definitions, "ocr_context_max_chars").value,
            json!(pipeline_config.ocr_config.context_max_chars)
        );
        assert_eq!(
            definition_for(&definitions, "ocr_auto_capture_timing").value,
            json!(pipeline_config.ocr_config.auto_capture_timing)
        );
        assert_eq!(
            definition_for(&definitions, "ocr_hallucination_protection").value,
            json!(pipeline_config.ocr_config.hallucination_protection)
        );
        assert_eq!(
            definition_for(&definitions, "ocr_hallucination_threshold").value,
            json!(pipeline_config.ocr_config.hallucination_threshold)
        );
        assert_eq!(
            definition_for(&definitions, "ocr_resize_max_dimension").value,
            json!(pipeline_config.ocr_config.resize_max_dimension)
        );
        assert_eq!(
            definition_for(&definitions, "ocr_resize_filter").value,
            json!(pipeline_config.ocr_config.resize_filter)
        );
    }
}
