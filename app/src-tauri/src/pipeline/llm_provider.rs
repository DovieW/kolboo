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

/// Create an LLM provider for one-off ad-hoc completions where callers expect
/// free-form text instead of rewrite-oriented structured outputs.
///
/// Keeping this constructor in the LLM Provider Resolution Module prevents
/// command handlers and Quick Actions from growing provider-specific match
/// statements whenever a provider knob is added.
pub(crate) fn create_llm_provider_unstructured(config: &LlmConfig) -> Arc<dyn LlmProvider> {
    match config.provider.as_str() {
        "cerebras" => {
            let provider = if let Some(model) = &config.model {
                CerebrasLlmProvider::with_model(config.api_key.clone(), model.clone())
            } else {
                CerebrasLlmProvider::new(config.api_key.clone())
            };
            Arc::new(
                provider
                    .with_timeout(config.timeout)
                    .with_reasoning_effort(config.openai_reasoning_effort.clone()),
            )
        }
        "anthropic" => {
            let provider = if let Some(model) = &config.model {
                AnthropicLlmProvider::with_model(config.api_key.clone(), model.clone())
            } else {
                AnthropicLlmProvider::new(config.api_key.clone())
            };
            Arc::new(
                provider
                    .with_timeout(config.timeout)
                    .with_thinking_budget(config.anthropic_thinking_budget),
            )
        }
        "groq" => {
            let provider = if let Some(model) = &config.model {
                GroqLlmProvider::with_model(config.api_key.clone(), model.clone())
            } else {
                GroqLlmProvider::new(config.api_key.clone())
            };
            Arc::new(provider.with_timeout(config.timeout))
        }
        "gemini" => {
            let provider = if let Some(model) = &config.model {
                GeminiLlmProvider::with_model(config.api_key.clone(), model.clone())
            } else {
                GeminiLlmProvider::new(config.api_key.clone())
            };

            Arc::new(
                provider
                    .with_timeout(config.timeout)
                    .with_thinking_budget(config.gemini_thinking_budget)
                    .with_thinking_level(config.gemini_thinking_level.clone())
                    .with_structured_outputs(false),
            )
        }
        "cohere" => {
            let provider = if let Some(model) = &config.model {
                CohereLlmProvider::with_model(config.api_key.clone(), model.clone())
            } else {
                CohereLlmProvider::new(config.api_key.clone())
            };
            Arc::new(provider.with_timeout(config.timeout))
        }
        "fireworks" => {
            let provider = if let Some(model) = &config.model {
                FireworksLlmProvider::with_model(config.api_key.clone(), model.clone())
            } else {
                FireworksLlmProvider::new(config.api_key.clone())
            };
            Arc::new(provider.with_timeout(config.timeout))
        }
        "ollama" => {
            let provider = OllamaLlmProvider::with_url(
                config
                    .ollama_url
                    .clone()
                    .unwrap_or_else(|| "http://localhost:11434".to_string()),
                config.model.clone(),
            );
            Arc::new(provider.with_timeout(config.timeout))
        }
        _ => {
            // Default to OpenAI
            let provider = if let Some(model) = &config.model {
                OpenAiLlmProvider::with_model(config.api_key.clone(), model.clone())
            } else {
                OpenAiLlmProvider::new(config.api_key.clone())
            };
            Arc::new(
                provider
                    .with_timeout(config.timeout)
                    .with_reasoning_effort(config.openai_reasoning_effort.clone())
                    .with_structured_outputs(false),
            )
        }
    }
}

/// Create an LLM provider for Settings test commands that intentionally avoid
/// request timeouts while still attaching request-log capture.
pub(crate) fn create_llm_provider_without_timeout(
    config: &LlmConfig,
    request_log_store: Option<RequestLogStore>,
) -> Arc<dyn LlmProvider> {
    match config.provider.as_str() {
        "cerebras" => {
            let provider = if let Some(model) = &config.model {
                CerebrasLlmProvider::with_model(config.api_key.clone(), model.clone())
            } else {
                CerebrasLlmProvider::new(config.api_key.clone())
            };
            Arc::new(
                provider
                    .without_timeout()
                    .with_request_log_store(request_log_store.clone())
                    .with_reasoning_effort(config.openai_reasoning_effort.clone()),
            )
        }
        "anthropic" => {
            let provider = if let Some(model) = &config.model {
                AnthropicLlmProvider::with_model(config.api_key.clone(), model.clone())
            } else {
                AnthropicLlmProvider::new(config.api_key.clone())
            };
            Arc::new(
                provider
                    .without_timeout()
                    .with_request_log_store(request_log_store.clone())
                    .with_thinking_budget(config.anthropic_thinking_budget),
            )
        }
        "groq" => {
            let provider = if let Some(model) = &config.model {
                GroqLlmProvider::with_model(config.api_key.clone(), model.clone())
            } else {
                GroqLlmProvider::new(config.api_key.clone())
            };
            Arc::new(
                provider
                    .without_timeout()
                    .with_request_log_store(request_log_store.clone()),
            )
        }
        "gemini" => {
            let provider = if let Some(model) = &config.model {
                GeminiLlmProvider::with_model(config.api_key.clone(), model.clone())
            } else {
                GeminiLlmProvider::new(config.api_key.clone())
            };

            Arc::new(
                provider
                    .without_timeout()
                    .with_request_log_store(request_log_store.clone())
                    .with_thinking_budget(config.gemini_thinking_budget)
                    .with_thinking_level(config.gemini_thinking_level.clone()),
            )
        }
        "cohere" => {
            let provider = if let Some(model) = &config.model {
                CohereLlmProvider::with_model(config.api_key.clone(), model.clone())
            } else {
                CohereLlmProvider::new(config.api_key.clone())
            };
            Arc::new(
                provider
                    .without_timeout()
                    .with_request_log_store(request_log_store.clone()),
            )
        }
        "fireworks" => {
            let provider = if let Some(model) = &config.model {
                FireworksLlmProvider::with_model(config.api_key.clone(), model.clone())
            } else {
                FireworksLlmProvider::new(config.api_key.clone())
            };
            Arc::new(
                provider
                    .without_timeout()
                    .with_request_log_store(request_log_store.clone()),
            )
        }
        "ollama" => {
            let provider = OllamaLlmProvider::with_url(
                config
                    .ollama_url
                    .clone()
                    .unwrap_or_else(|| "http://localhost:11434".to_string()),
                config.model.clone(),
            );
            Arc::new(
                provider
                    .without_timeout()
                    .with_request_log_store(request_log_store.clone()),
            )
        }
        _ => {
            // Default to OpenAI
            let provider = if let Some(model) = &config.model {
                OpenAiLlmProvider::with_model(config.api_key.clone(), model.clone())
            } else {
                OpenAiLlmProvider::new(config.api_key.clone())
            };
            Arc::new(
                provider
                    .without_timeout()
                    .with_request_log_store(request_log_store.clone())
                    .with_reasoning_effort(config.openai_reasoning_effort.clone()),
            )
        }
    }
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
