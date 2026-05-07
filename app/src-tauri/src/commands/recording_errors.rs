//! Recording-command error mapping.
//!
//! The pipeline owns rich domain errors; Tauri commands need stable, UI-friendly
//! `CommandError` codes and retryability flags. Keeping that translation here
//! prevents the large recording command module from accumulating another concern.

use crate::commands::CommandError;
use crate::pipeline::PipelineError;

fn looks_like_auth_error(s: &str) -> bool {
    let lower = s.to_lowercase();
    lower.contains("401")
        || lower.contains("403")
        || lower.contains("unauthorized")
        || lower.contains("forbidden")
        || lower.contains("invalid api key")
        || lower.contains("invalid_api_key")
}

fn looks_like_rate_limit(s: &str) -> bool {
    let lower = s.to_lowercase();
    lower.contains("429")
        || lower.contains("rate limit")
        || lower.contains("rate_limit")
        || lower.contains("too many requests")
}

fn classify_pipeline_error(err: &PipelineError) -> (&'static str, &'static str, bool) {
    use crate::llm::LlmError;
    use crate::stt::SttError;

    match err {
        PipelineError::AudioCapture(_) => ("audio", "audio_capture", true),

        PipelineError::Stt(e) => match e {
            SttError::Timeout => ("stt", "stt_timeout", true),
            SttError::Network(re) => {
                if re.is_timeout() {
                    ("stt", "stt_timeout", true)
                } else {
                    ("stt", "stt_network", true)
                }
            }
            SttError::NetworkMessage(_) => ("stt", "stt_network", true),
            SttError::Config(_) => ("stt", "stt_config", false),
            SttError::Audio(_) => ("stt", "stt_audio", false),
            SttError::Api(msg) => {
                if looks_like_auth_error(msg) {
                    ("stt", "stt_auth", false)
                } else if looks_like_rate_limit(msg) {
                    ("stt", "stt_rate_limited", true)
                } else {
                    ("stt", "stt_api", true)
                }
            }
        },

        PipelineError::Llm(e) => match e {
            LlmError::Timeout(_) => ("llm", "llm_timeout", true),
            LlmError::Network(re) => {
                if re.is_timeout() {
                    ("llm", "llm_timeout", true)
                } else {
                    ("llm", "llm_network", true)
                }
            }
            LlmError::NoApiKey(_) => ("llm", "llm_no_api_key", false),
            LlmError::ProviderNotAvailable(_) => ("llm", "llm_provider_unavailable", false),
            LlmError::InvalidResponse(_) => ("llm", "llm_invalid_response", false),
            LlmError::Api(msg) => {
                if looks_like_auth_error(msg) {
                    ("llm", "llm_auth", false)
                } else if looks_like_rate_limit(msg) {
                    ("llm", "llm_rate_limited", true)
                } else {
                    ("llm", "llm_api", true)
                }
            }
        },

        PipelineError::NoProvider => ("config", "no_provider", false),
        PipelineError::AlreadyRecording => ("state", "already_recording", false),
        PipelineError::NotRecording => ("state", "not_recording", false),
        PipelineError::Config(_) => ("config", "config_error", false),
        PipelineError::Lock(_) => ("internal", "lock_error", false),
        PipelineError::Cancelled => ("cancelled", "cancelled", false),
        PipelineError::Timeout(_) => ("timeout", "timeout", true),
        PipelineError::RecordingTooLarge(_, _) => ("size", "recording_too_large", false),
    }
}

impl From<PipelineError> for CommandError {
    fn from(err: PipelineError) -> Self {
        let (error_type, code, retryable) = classify_pipeline_error(&err);
        let details = crate::request_log::format_error_chain(&err);
        CommandError::new(err.to_string(), error_type)
            .with_code(code)
            .with_retryable(retryable)
            .with_details(details)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stt::SttError;

    #[test]
    fn stt_auth_errors_are_not_retryable() {
        let (_kind, code, retryable) = classify_pipeline_error(&PipelineError::Stt(SttError::Api(
            "401 invalid api key".to_string(),
        )));

        assert_eq!(code, "stt_auth");
        assert!(!retryable);
    }

    #[test]
    fn stt_rate_limits_are_retryable() {
        let (_kind, code, retryable) = classify_pipeline_error(&PipelineError::Stt(SttError::Api(
            "429 too many requests".to_string(),
        )));

        assert_eq!(code, "stt_rate_limited");
        assert!(retryable);
    }

    #[test]
    fn state_errors_keep_stable_codes() {
        let (kind, code, retryable) = classify_pipeline_error(&PipelineError::AlreadyRecording);
        assert_eq!(
            (kind, code, retryable),
            ("state", "already_recording", false)
        );
    }
}
