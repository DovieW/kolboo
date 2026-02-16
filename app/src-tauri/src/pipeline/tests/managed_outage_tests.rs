use crate::pipeline::config::PipelineConfig;

#[test]
fn managed_gateway_ready_requires_non_empty_gateway_and_token() {
    let mut config = PipelineConfig::default();
    config.managed_inference_enabled = true;

    assert!(!super::managed_gateway_ready(&config));

    config.managed_inference_gateway_url = Some("https://gateway.example".to_string());
    assert!(!super::managed_gateway_ready(&config));

    config.managed_inference_access_token = Some("token".to_string());
    assert!(super::managed_gateway_ready(&config));
}

#[test]
fn stt_provider_falls_back_when_managed_gateway_unavailable() {
    let mut config = PipelineConfig::default();
    config.managed_inference_enabled = true;
    config.managed_inference_fallback_stt_provider = Some("groq".to_string());

    let provider = super::resolve_stt_provider_for_runtime(&config, "kolboo_cloud");
    assert_eq!(provider, "groq");
}

#[test]
fn llm_provider_falls_back_when_managed_gateway_unavailable() {
    let mut config = PipelineConfig::default();
    config.managed_inference_enabled = true;
    config.managed_inference_fallback_llm_provider = Some("anthropic".to_string());

    let provider = super::resolve_llm_provider_for_runtime(&config, "kolboo_cloud");
    assert_eq!(provider, "anthropic");
}
