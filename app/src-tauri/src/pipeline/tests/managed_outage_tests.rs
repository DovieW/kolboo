use crate::pipeline::config::PipelineConfig;
use crate::pipeline::PipelineError;
use crate::stt::SttError;

#[test]
fn managed_gateway_ready_requires_non_empty_gateway_and_token() {
    let mut config = PipelineConfig {
        managed_inference_enabled: true,
        ..Default::default()
    };

    assert!(!super::managed_gateway_ready(&config));

    config.managed_inference_gateway_url = Some("https://gateway.example".to_string());
    assert!(!super::managed_gateway_ready(&config));

    config.managed_inference_access_token = Some("token".to_string());
    assert!(super::managed_gateway_ready(&config));
}

#[test]
fn stt_provider_falls_back_when_managed_gateway_unavailable() {
    let config = PipelineConfig {
        managed_inference_enabled: true,
        managed_inference_fallback_stt_provider: Some("assemblyai".to_string()),
        ..Default::default()
    };

    let provider =
        super::resolve_stt_provider_for_runtime(&config, "groq", Some("whisper-large-v3-turbo"));
    assert_eq!(provider, "assemblyai");
}

#[test]
fn stt_provider_does_not_fall_back_when_byok_is_selected() {
    let config = PipelineConfig {
        managed_inference_enabled: true,
        managed_stt_preferred: false,
        managed_inference_fallback_stt_provider: Some("groq".to_string()),
        ..Default::default()
    };

    let provider = super::resolve_stt_provider_for_runtime(&config, "openai", Some("whisper-1"));
    assert_eq!(provider, "openai");
}

#[test]
fn stt_provider_does_not_fall_back_for_a_non_managed_model() {
    let config = PipelineConfig {
        managed_inference_enabled: true,
        managed_stt_preferred: true,
        managed_inference_fallback_stt_provider: Some("groq".to_string()),
        ..Default::default()
    };

    let provider = super::resolve_stt_provider_for_runtime(
        &config,
        "groq",
        Some("distil-whisper-large-v3-en"),
    );
    assert_eq!(provider, "groq");
}

#[test]
fn llm_provider_falls_back_when_managed_gateway_unavailable() {
    let config = PipelineConfig {
        managed_inference_enabled: true,
        managed_inference_fallback_llm_provider: Some("anthropic".to_string()),
        ..Default::default()
    };

    let provider = super::resolve_llm_provider_for_runtime(&config, "managed");
    assert_eq!(provider, "anthropic");
}

#[test]
fn byok_llm_provider_does_not_change_when_managed_gateway_is_unavailable() {
    let config = PipelineConfig {
        managed_inference_enabled: true,
        managed_inference_fallback_llm_provider: Some("anthropic".to_string()),
        ..Default::default()
    };

    let provider = super::resolve_llm_provider_for_runtime(&config, "openai");
    assert_eq!(provider, "openai");
}

#[test]
fn managed_auth_error_detection_matches_token_rejection_messages() {
    let err = PipelineError::Stt(SttError::Api(
        "Groq API error (401 Unauthorized): {\"error\":{\"code\":\"AUTH_INVALID_TOKEN\",\"message\":\"Supabase auth user lookup rejected token\"}}".to_string(),
    ));

    assert!(super::is_managed_auth_token_error(&err));
}

#[test]
fn managed_auth_error_detection_ignores_non_auth_stt_errors() {
    let err = PipelineError::Stt(SttError::Api("rate limit exceeded".to_string()));

    assert!(!super::is_managed_auth_token_error(&err));
}
