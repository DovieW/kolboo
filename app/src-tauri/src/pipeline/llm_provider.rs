use crate::llm::{
    AnthropicLlmProvider, CerebrasLlmProvider, CohereLlmProvider, FireworksLlmProvider,
    GeminiLlmProvider, GroqLlmProvider, LlmConfig, LlmProvider, OllamaLlmProvider,
    OpenAiLlmProvider,
};
use crate::request_log::RequestLogStore;
use crate::settings::ProxySettings;
use std::sync::Arc;
use std::time::Duration;

fn managed_llm_api_base_url(provider: &str, gateway_url: &str) -> Option<String> {
    let gateway = gateway_url.trim().trim_end_matches('/');
    if gateway.is_empty() {
        return None;
    }

    match provider {
        // OpenAI provider appends /v1/responses internally.
        "openai" => Some(gateway.to_string()),
        // Provider adapters that expect a full endpoint URL.
        "groq" => Some(format!("{gateway}/groq/openai/v1/chat/completions")),
        "fireworks" => Some(format!("{gateway}/fireworks/inference/v1/chat/completions")),
        _ => None,
    }
}

pub(super) struct LlmProviderParams {
    pub model: Option<String>,
    pub timeout: Duration,
    pub ollama_url: Option<String>,
    pub openai_reasoning_effort: Option<String>,
    pub gemini_thinking_budget: Option<i64>,
    pub gemini_thinking_level: Option<String>,
    pub anthropic_thinking_budget: Option<i64>,
}

/// Create an LLM provider based on configuration
pub(super) fn create_llm_provider(
    config: &LlmConfig,
    request_log_store: Option<RequestLogStore>,
    proxy_settings: &ProxySettings,
) -> Result<Arc<dyn LlmProvider>, crate::pipeline::PipelineError> {
    let client = crate::network::build_http_client(proxy_settings).map_err(|e| {
        crate::pipeline::PipelineError::Config(format!("Failed to create HTTP client: {}", e))
    })?;

    let provider: Arc<dyn LlmProvider> = match config.provider.as_str() {
        "cerebras" => Arc::new(
            CerebrasLlmProvider::with_client(
                client.clone(),
                config.api_key.clone(),
                config.model.clone(),
            )
            .with_timeout(config.timeout)
            .with_request_log_store(request_log_store.clone())
            .with_reasoning_effort(config.openai_reasoning_effort.clone()),
        ),
        "anthropic" => {
            if config.managed_gateway_url.as_deref().is_some() {
                return Err(crate::pipeline::PipelineError::Config(
                    "Managed mode does not yet support Anthropic LLM provider routing".to_string(),
                ));
            }

            Arc::new(
                AnthropicLlmProvider::with_client(
                    client.clone(),
                    config.api_key.clone(),
                    config.model.clone(),
                )
                .with_timeout(config.timeout)
                .with_request_log_store(request_log_store.clone())
                .with_thinking_budget(config.anthropic_thinking_budget),
            )
        }
        "groq" => {
            let provider = GroqLlmProvider::with_client(
                client.clone(),
                config.api_key.clone(),
                config.model.clone(),
            )
            .with_timeout(config.timeout)
            .with_request_log_store(request_log_store.clone());

            let provider = if let Some(gateway) = config.managed_gateway_url.as_deref() {
                let url = managed_llm_api_base_url("groq", gateway).ok_or_else(|| {
                    crate::pipeline::PipelineError::Config(
                        "Managed mode could not resolve Groq gateway URL".to_string(),
                    )
                })?;
                provider.with_api_base_url(url)
            } else {
                provider
            };

            Arc::new(provider)
        }
        "gemini" => {
            if config.managed_gateway_url.as_deref().is_some() {
                return Err(crate::pipeline::PipelineError::Config(
                    "Managed mode does not yet support Gemini LLM provider routing".to_string(),
                ));
            }

            Arc::new(
                GeminiLlmProvider::with_client(
                    client.clone(),
                    config.api_key.clone(),
                    config.model.clone(),
                )
                .with_timeout(config.timeout)
                .with_request_log_store(request_log_store.clone())
                .with_thinking_budget(config.gemini_thinking_budget)
                .with_thinking_level(config.gemini_thinking_level.clone()),
            )
        }
        "ollama" => Arc::new(
            OllamaLlmProvider::with_client(
                client.clone(),
                config.ollama_url.clone(),
                config.model.clone(),
            )
            .with_timeout(config.timeout)
            .with_request_log_store(request_log_store.clone()),
        ),
        "cohere" => {
            if config.managed_gateway_url.as_deref().is_some() {
                return Err(crate::pipeline::PipelineError::Config(
                    "Managed mode does not yet support Cohere LLM provider routing".to_string(),
                ));
            }

            Arc::new(
                CohereLlmProvider::with_client(
                    client.clone(),
                    config.api_key.clone(),
                    config.model.clone(),
                )
                .with_timeout(config.timeout)
                .with_request_log_store(request_log_store.clone()),
            )
        }
        "fireworks" => {
            let provider = FireworksLlmProvider::with_client(
                client.clone(),
                config.api_key.clone(),
                config.model.clone(),
            )
            .with_timeout(config.timeout)
            .with_request_log_store(request_log_store.clone());

            let provider = if let Some(gateway) = config.managed_gateway_url.as_deref() {
                let url = managed_llm_api_base_url("fireworks", gateway).ok_or_else(|| {
                    crate::pipeline::PipelineError::Config(
                        "Managed mode could not resolve Fireworks gateway URL".to_string(),
                    )
                })?;
                provider.with_api_base_url(url)
            } else {
                provider
            };

            Arc::new(provider)
        }
        _ => {
            // Default to OpenAI
            let provider = OpenAiLlmProvider::with_client(
                client,
                config.api_key.clone(),
                config.model.clone(),
            )
            .with_timeout(config.timeout)
            .with_request_log_store(request_log_store.clone())
            .with_reasoning_effort(config.openai_reasoning_effort.clone());

            let provider = if let Some(gateway) = config.managed_gateway_url.as_deref() {
                let url = managed_llm_api_base_url("openai", gateway).ok_or_else(|| {
                    crate::pipeline::PipelineError::Config(
                        "Managed mode could not resolve OpenAI gateway URL".to_string(),
                    )
                })?;
                provider.with_api_base_url(url)
            } else {
                provider
            };

            Arc::new(provider)
        }
    };

    Ok(provider)
}
