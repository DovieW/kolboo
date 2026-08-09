use serde_json::{json, Value};

use super::{migrations, ProxySettings};

pub const DEFAULT_STT_PROVIDER: &str = "groq";
pub const DEFAULT_STT_LANGUAGE: &str = "en";
pub const DEFAULT_STT_TIMEOUT_SECONDS: f64 = 10.0;
pub const DEFAULT_STT_LIVE_OUTPUT: bool = false;
pub const DEFAULT_STT_SIMULATED_STREAMING: bool = false;
#[cfg(feature = "local-whisper")]
pub const DEFAULT_LOCAL_WHISPER_MODEL_ID: &str = "base";
pub const DEFAULT_LOCAL_WHISPER_LOAD_MODE: &str = "manual";

pub const DEFAULT_OVERLAY_MODE: &str = "recording_only";
pub const DEFAULT_OVERLAY_SHOW_DETAILED_LOADING: bool = false;
pub const DEFAULT_OVERLAY_MONITOR_TARGET: &str = "main";
pub const DEFAULT_WIDGET_POSITION: &str = "bottom-center";
pub const DEFAULT_OUTPUT_MODE: &str = "paste";
pub const DEFAULT_OUTPUT_HIT_ENTER: bool = false;
pub const DEFAULT_OUTPUT_CLIPBOARD_PRIVACY_MODE: bool = false;
pub const DEFAULT_OUTPUT_SMART_PASTE_PROTECTION: bool = false;
pub const DEFAULT_MAIN_WINDOW_CLOSE_BEHAVIOR: &str = "minimize_to_tray";

pub const DEFAULT_PLAYING_AUDIO_HANDLING: &str = "none";
pub const DEFAULT_SOUND_ENABLED: bool = true;
pub const DEFAULT_REWRITE_LLM_ENABLED: bool = false;
pub const DEFAULT_QUICK_REPLACE_ENABLED: bool = false;

pub const DEFAULT_MAX_SAVED_RECORDINGS: u32 = 1000;
pub const DEFAULT_REQUEST_LOGS_RETENTION_MODE: &str = "amount";
pub const DEFAULT_REQUEST_LOGS_RETENTION_AMOUNT: u32 = 50;
pub const DEFAULT_REQUEST_LOGS_RETENTION_DAYS: u32 = 7;
pub const DEFAULT_REQUEST_LOGS_PRIVACY_MODE: bool = false;
pub const DEFAULT_POSTHOG_ANALYTICS_ENABLED: bool = true;
pub const DEFAULT_TELEMETRY_DISCLOSURE_ACKNOWLEDGED_AT: Option<&str> = None;
pub const DEFAULT_TELEMETRY_DISCLOSURE_VERSION: Option<&str> = None;
pub const DEFAULT_TRANSCRIPTION_RETENTION_MODE: &str = "time";
pub const DEFAULT_TRANSCRIPTION_RETENTION_AMOUNT: u32 = 1000;
pub const DEFAULT_TRANSCRIPTION_RETENTION_UNIT: &str = "days";
pub const DEFAULT_TRANSCRIPTION_RETENTION_VALUE: f64 = 0.0;
pub const DEFAULT_TRANSCRIPTION_RETENTION_DELETE_RECORDINGS: bool = false;
pub const DEFAULT_RECORDINGS_RETENTION_MODE: &str = "amount";
pub const DEFAULT_RECORDINGS_RETENTION_AMOUNT: u32 = 1000;
pub const DEFAULT_RECORDINGS_RETENTION_UNIT: &str = "days";
pub const DEFAULT_RECORDINGS_RETENTION_VALUE: f64 = 0.0;
pub const DEFAULT_STATS_RETENTION_UNIT: &str = "days";
pub const DEFAULT_STATS_RETENTION_VALUE: f64 = 30.0;
pub const DEFAULT_STATS_RETENTION_MAX_BYTES: u64 = 50_000_000;

pub const DEFAULT_HOTKEY_DEBUG_ENABLED: bool = false;

pub const DEFAULT_QUICK_ASK_DISMISS_MODE: &str = "manual";
pub const DEFAULT_QUICK_ASK_CONVERSATION_HISTORY_ENABLED: bool = true;
pub const DEFAULT_QUICK_ASK_CONVERSATION_HISTORY_COUNT: u32 = 3;
pub const DEFAULT_QUICK_ASK_INCLUDE_SELECTED_TEXT: bool = false;

pub const DEFAULT_OCR_MODEL: &str = "lightonai/LightOnOCR-1B-1025";
pub const DEFAULT_OCR_AUTH_MODE: &str = "none";
pub const DEFAULT_OCR_REQUEST_TIMEOUT_MS: u64 = 2000;
pub const DEFAULT_OCR_CONTEXT_MAX_CHARS: u64 = 8000;
pub const DEFAULT_OCR_AUTO_CAPTURE_TIMING: &str = "on_start";
pub const DEFAULT_OCR_HALLUCINATION_PROTECTION: bool = true;
pub const DEFAULT_OCR_HALLUCINATION_THRESHOLD: u64 = 2500;
pub const DEFAULT_OCR_RESIZE_MAX_DIMENSION: u32 = 0;
pub const DEFAULT_OCR_RESIZE_FILTER: &str = "nearest";
pub const DEFAULT_ACTIVE_WINDOW_OCR_MODE: &str = "off";

pub const DEFAULT_HOT_MIC_ENABLED: bool = false;
pub const DEFAULT_HOT_MIC_PRE_ROLL_MS: u32 = 1500;
pub const DEFAULT_MIC_AUTO_RECOVER_ENABLED: bool = false;

pub const DEFAULT_QUIET_AUDIO_GATE_ENABLED: bool = true;
pub const DEFAULT_QUIET_AUDIO_MIN_DURATION_SECS: f32 = 0.15;
pub const DEFAULT_QUIET_AUDIO_RMS_DBFS_THRESHOLD: f32 = -60.0;
pub const DEFAULT_QUIET_AUDIO_PEAK_DBFS_THRESHOLD: f32 = -50.0;
pub const DEFAULT_QUIET_AUDIO_REQUIRE_SPEECH: bool = false;

pub const DEFAULT_AUDIO_DOWNMIX_TO_MONO: bool = true;
pub const DEFAULT_AUDIO_RESAMPLE_TO_16KHZ: bool = false;
pub const DEFAULT_AUDIO_HIGHPASS_ENABLED: bool = true;
pub const DEFAULT_AUDIO_AGC_ENABLED: bool = false;
pub const DEFAULT_AUDIO_NOISE_SUPPRESSION_ENABLED: bool = false;

pub fn default_policy_state_value() -> Value {
    json!({
        "source": "none",
        "is_valid": true,
        "last_updated": null,
        "expires_at": null,
        "version": null,
        "enforced_fields": []
    })
}

pub fn default_license_state_value() -> Value {
    json!({
        "tier": "community",
        "status": "signed_out",
        "user_id": null,
        "email": null,
        "org": null,
        "expires_at": null,
        "cached_at": null,
        "last_validated_at": null,
        "usage": {
            "stt_seconds_used": 0,
            "llm_tokens_used": 0,
            "requests_today": 0
        },
        "limits": {
            "stt_seconds_monthly": 0,
            "llm_tokens_monthly": 0,
            "requests_per_day": 0
        },
        "portal_available": false
    })
}

pub fn default_token_exchange_trigger_set_value() -> Value {
    json!({
        "multi_idp_required": false,
        "kill_switch_required": false,
        "embedded_claims_required": false,
        "desktop_idp_agnostic_required": false,
        "reviewed_at": null,
        "decision": "direct_idp_token"
    })
}

pub fn default_rewrite_profile_value() -> Value {
    json!({
        "id": "default",
        "name": "Default",
        "program_paths": [],
        "cleanup_prompt_sections": null,
        "presets": [],
        "default_preset_id": null,
        "default_preset_description": null,
        "default_target_rewrite_llm_enabled": true,
        "active_preset_id": null,
        "router": null,
        "rewrite_llm_enabled": null,
    })
}

pub fn default_settings_version_value() -> Value {
    json!(migrations::SETTINGS_VERSION_LATEST)
}

pub fn default_proxy_settings_value() -> Result<Value, serde_json::Error> {
    serde_json::to_value(ProxySettings::default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::PipelineConfig;

    #[test]
    fn default_values_match_pipeline_defaults_for_overlapping_runtime_settings() {
        let pipeline = PipelineConfig::default();

        assert_eq!(pipeline.stt_provider, DEFAULT_STT_PROVIDER);
        assert_eq!(pipeline.stt_language.as_deref(), Some(DEFAULT_STT_LANGUAGE));
        assert_eq!(
            pipeline.local_whisper_load_mode,
            DEFAULT_LOCAL_WHISPER_LOAD_MODE
        );
        assert_eq!(pipeline.stt_live_output, DEFAULT_STT_LIVE_OUTPUT);
        assert_eq!(
            pipeline.stt_simulated_streaming,
            DEFAULT_STT_SIMULATED_STREAMING
        );
        assert_eq!(
            pipeline.quiet_audio_gate_enabled,
            DEFAULT_QUIET_AUDIO_GATE_ENABLED
        );
        assert_eq!(
            pipeline.quiet_audio_min_duration_secs,
            DEFAULT_QUIET_AUDIO_MIN_DURATION_SECS
        );
        assert_eq!(
            pipeline.quiet_audio_rms_dbfs_threshold,
            DEFAULT_QUIET_AUDIO_RMS_DBFS_THRESHOLD
        );
        assert_eq!(
            pipeline.quiet_audio_peak_dbfs_threshold,
            DEFAULT_QUIET_AUDIO_PEAK_DBFS_THRESHOLD
        );
        assert_eq!(
            pipeline.quiet_audio_require_speech,
            DEFAULT_QUIET_AUDIO_REQUIRE_SPEECH
        );
        assert_eq!(pipeline.hot_mic_enabled, DEFAULT_HOT_MIC_ENABLED);
        assert_eq!(pipeline.hot_mic_pre_roll_ms, DEFAULT_HOT_MIC_PRE_ROLL_MS);
        assert_eq!(
            pipeline.mic_auto_recover_enabled,
            DEFAULT_MIC_AUTO_RECOVER_ENABLED
        );
        assert_eq!(
            pipeline.audio_downmix_to_mono,
            DEFAULT_AUDIO_DOWNMIX_TO_MONO
        );
        assert_eq!(
            pipeline.audio_resample_to_16khz,
            DEFAULT_AUDIO_RESAMPLE_TO_16KHZ
        );
        assert_eq!(
            pipeline.audio_highpass_enabled,
            DEFAULT_AUDIO_HIGHPASS_ENABLED
        );
        assert_eq!(pipeline.audio_agc_enabled, DEFAULT_AUDIO_AGC_ENABLED);
        assert_eq!(
            pipeline.audio_noise_suppression_enabled,
            DEFAULT_AUDIO_NOISE_SUPPRESSION_ENABLED
        );
        assert_eq!(pipeline.ocr_config.model, DEFAULT_OCR_MODEL);
        assert_eq!(pipeline.ocr_config.auth_mode, DEFAULT_OCR_AUTH_MODE);
        assert_eq!(
            pipeline.ocr_config.request_timeout_ms,
            DEFAULT_OCR_REQUEST_TIMEOUT_MS
        );
        assert_eq!(
            pipeline.ocr_config.context_max_chars,
            DEFAULT_OCR_CONTEXT_MAX_CHARS as usize
        );
        assert_eq!(
            pipeline.ocr_config.resize_max_dimension,
            DEFAULT_OCR_RESIZE_MAX_DIMENSION
        );
        assert_eq!(pipeline.ocr_config.resize_filter, DEFAULT_OCR_RESIZE_FILTER);
        assert_eq!(
            pipeline.ocr_config.auto_capture_timing,
            DEFAULT_OCR_AUTO_CAPTURE_TIMING
        );
        assert_eq!(
            pipeline.ocr_config.hallucination_protection,
            DEFAULT_OCR_HALLUCINATION_PROTECTION
        );
        assert_eq!(
            pipeline.ocr_config.hallucination_threshold,
            DEFAULT_OCR_HALLUCINATION_THRESHOLD
        );
        assert_eq!(
            pipeline.ocr_config.resize_max_dimension,
            DEFAULT_OCR_RESIZE_MAX_DIMENSION
        );
        assert_eq!(
            pipeline.ocr_config.rewrite_mode,
            DEFAULT_ACTIVE_WINDOW_OCR_MODE
        );
        assert_eq!(
            pipeline.ocr_config.quick_replace_mode,
            DEFAULT_ACTIVE_WINDOW_OCR_MODE
        );
        assert_eq!(
            pipeline.ocr_config.quick_ask_mode,
            DEFAULT_ACTIVE_WINDOW_OCR_MODE
        );
    }

    #[test]
    fn default_json_shapes_preserve_explicit_null_semantics() {
        let profile = default_rewrite_profile_value();

        assert_eq!(profile["id"], "default");
        assert!(profile["rewrite_llm_enabled"].is_null());
        assert!(profile["cleanup_prompt_sections"].is_null());
        assert_eq!(default_policy_state_value()["source"], "none");
        assert_eq!(default_license_state_value()["tier"], "community");
        assert_eq!(
            default_token_exchange_trigger_set_value()["decision"],
            "direct_idp_token"
        );
        assert_eq!(default_settings_version_value(), json!(9));
        assert!(default_proxy_settings_value().is_ok());
    }
}
