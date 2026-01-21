use crate::llm::{
    AnthropicLlmProvider, CerebrasLlmProvider, CohereLlmProvider, FireworksLlmProvider,
    GeminiLlmProvider, GroqLlmProvider, LlmConfig, LlmProvider, OllamaLlmProvider,
    OpenAiLlmProvider,
};
use crate::request_log::RequestLogStore;
use crate::settings::ProxySettings;
use std::sync::Arc;
use std::time::Duration;

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
        "anthropic" => Arc::new(
            AnthropicLlmProvider::with_client(
                client.clone(),
                config.api_key.clone(),
                config.model.clone(),
            )
            .with_timeout(config.timeout)
            .with_request_log_store(request_log_store.clone())
            .with_thinking_budget(config.anthropic_thinking_budget),
        ),
        "groq" => Arc::new(
            GroqLlmProvider::with_client(
                client.clone(),
                config.api_key.clone(),
                config.model.clone(),
            )
            .with_timeout(config.timeout)
            .with_request_log_store(request_log_store.clone()),
        ),
        "gemini" => Arc::new(
            GeminiLlmProvider::with_client(
                client.clone(),
                config.api_key.clone(),
                config.model.clone(),
            )
            .with_timeout(config.timeout)
            .with_request_log_store(request_log_store.clone())
            .with_thinking_budget(config.gemini_thinking_budget)
            .with_thinking_level(config.gemini_thinking_level.clone()),
        ),
        "ollama" => Arc::new(
            OllamaLlmProvider::with_client(
                client.clone(),
                config.ollama_url.clone(),
                config.model.clone(),
            )
            .with_timeout(config.timeout)
            .with_request_log_store(request_log_store.clone()),
        ),
        "cohere" => Arc::new(
            CohereLlmProvider::with_client(
                client.clone(),
                config.api_key.clone(),
                config.model.clone(),
            )
            .with_timeout(config.timeout)
            .with_request_log_store(request_log_store.clone()),
        ),
        "fireworks" => Arc::new(
            FireworksLlmProvider::with_client(
                client.clone(),
                config.api_key.clone(),
                config.model.clone(),
            )
            .with_timeout(config.timeout)
            .with_request_log_store(request_log_store.clone()),
        ),
        _ => {
            // Default to OpenAI
            Arc::new(
                OpenAiLlmProvider::with_client(
                    client,
                    config.api_key.clone(),
                    config.model.clone(),
                )
                .with_timeout(config.timeout)
                .with_request_log_store(request_log_store.clone())
                .with_reasoning_effort(config.openai_reasoning_effort.clone()),
            )
        }
    };

    Ok(provider)
}
