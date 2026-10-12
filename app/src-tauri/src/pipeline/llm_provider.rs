use crate::llm::{
    AnthropicLlmProvider, CerebrasLlmProvider, CohereLlmProvider, FireworksLlmProvider,
    GeminiLlmProvider, GroqLlmProvider, LlmConfig, LlmProvider, ManagedLlmProvider,
    OllamaLlmProvider, OpenAiLlmProvider, PromptSections,
};
use crate::request_log::RequestLogStore;
use crate::settings::ProxySettings;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use super::{
    managed_gateway_ready, resolve_llm_provider_for_runtime, PipelineConfig, PipelineError,
};

fn managed_llm_api_base_url(provider: &str, gateway_url: &str) -> Option<String> {
    let gateway = gateway_url.trim().trim_end_matches('/');
    if gateway.is_empty() {
        return None;
    }

    match provider {
        "managed" => Some(format!("{gateway}/v1/chat/completions")),
        // OpenAI provider appends /v1/responses internally.
        "openai" => Some(gateway.to_string()),
        // Provider adapters that expect a full endpoint URL.
        "groq" => Some(format!("{gateway}/groq/openai/v1/chat/completions")),
        "fireworks" => Some(format!("{gateway}/fireworks/inference/v1/chat/completions")),
        _ => None,
    }
}

#[derive(Debug, Clone)]
pub(crate) struct LlmProviderParams {
    pub selection: Option<crate::model_selection::ModelSelection>,
    pub model: Option<String>,
    pub timeout: Duration,
    pub ollama_url: Option<String>,
    pub openai_reasoning_effort: Option<String>,
    pub gemini_thinking_budget: Option<i64>,
    pub gemini_thinking_level: Option<String>,
    pub anthropic_thinking_budget: Option<i64>,
}

pub(super) struct ResolvedCachedLlmProviderConfig {
    #[cfg_attr(not(test), allow(dead_code))]
    pub provider_id: String,
    pub cache_key: String,
    pub managed_transport_active: bool,
    pub config: LlmConfig,
}

pub(super) fn llm_provider_cache_key(provider_id: &str, params: &LlmProviderParams) -> String {
    if let Some(selection) = &params.selection {
        return serde_json::json!([
            selection,
            params.timeout.as_secs_f64(),
            params.ollama_url,
            params.openai_reasoning_effort,
            params.gemini_thinking_budget,
            params.gemini_thinking_level,
            params.anthropic_thinking_budget
        ])
        .to_string();
    }
    let model_key = params
        .model
        .clone()
        .unwrap_or_else(|| "<default>".to_string());
    let url_key = params
        .ollama_url
        .clone()
        .unwrap_or_else(|| "<default-url>".to_string());
    let openai_effort_key = params
        .openai_reasoning_effort
        .clone()
        .unwrap_or_else(|| "<default-effort>".to_string());
    let gemini_budget_key = params
        .gemini_thinking_budget
        .map(|b| b.to_string())
        .unwrap_or_else(|| "<default-budget>".to_string());
    let gemini_level_key = params
        .gemini_thinking_level
        .clone()
        .unwrap_or_else(|| "<default-level>".to_string());
    let anthropic_budget_key = params
        .anthropic_thinking_budget
        .map(|b| b.to_string())
        .unwrap_or_else(|| "<default-budget>".to_string());

    format!(
        "{}::{}::{}::{}::{}::{}::{}::{}",
        provider_id,
        model_key,
        params.timeout.as_secs_f64(),
        url_key,
        openai_effort_key,
        gemini_budget_key,
        gemini_level_key,
        anthropic_budget_key
    )
}

pub(super) fn resolve_cached_llm_provider_config(
    pipeline_config: &PipelineConfig,
    provider_id: &str,
    params: LlmProviderParams,
) -> Result<ResolvedCachedLlmProviderConfig, PipelineError> {
    if let Some(error) = &pipeline_config.model_selection_error {
        return Err(PipelineError::Config(error.clone()));
    }
    let provider_id = match &params.selection {
        Some(selection) => selected_llm_provider(selection)?,
        None => resolve_llm_provider_for_runtime(pipeline_config, provider_id),
    };
    let managed_ready =
        pipeline_config.managed_inference_enabled && managed_gateway_ready(pipeline_config);
    let managed_transport_active = managed_ready && provider_id == "managed";

    if provider_id == "managed" && !managed_transport_active {
        return Err(PipelineError::Config(
            "Managed inference is not available for this session".to_string(),
        ));
    }

    let api_key = if provider_id == "ollama" {
        String::new()
    } else if managed_transport_active {
        pipeline_config
            .managed_inference_access_token
            .clone()
            .unwrap_or_default()
    } else {
        pipeline_config
            .llm_api_keys
            .get(&provider_id)
            .cloned()
            .unwrap_or_default()
    };

    if provider_id != "ollama" && api_key.is_empty() {
        return Err(PipelineError::Config(format!(
            "Could not load a usable API key for LLM provider '{}'. Check provider settings and your system wallet",
            provider_id
        )));
    }

    // Preserve global runtime config (including provider-specific knobs) but override the
    // effective provider/model/timeout for this transcription.
    let mut config = pipeline_config.llm_config.clone();
    config.enabled = true;
    config.provider = provider_id.clone();
    config.api_key = api_key;
    config.model = params
        .selection
        .as_ref()
        .map(|choice| choice.model_id.clone())
        .or_else(|| params.model.clone());
    config.selection = params.selection.clone();
    config.managed_access_token = None;
    config.ollama_url = params.ollama_url.clone();
    config.timeout = params.timeout;
    config.managed_gateway_url = if managed_transport_active {
        pipeline_config.managed_inference_gateway_url.clone()
    } else {
        None
    };
    config.openai_reasoning_effort = params.openai_reasoning_effort.clone();
    config.gemini_thinking_budget = params.gemini_thinking_budget;
    config.gemini_thinking_level = params.gemini_thinking_level.clone();
    config.anthropic_thinking_budget = params.anthropic_thinking_budget;

    validate_custom_llm(&config)?;
    Ok(ResolvedCachedLlmProviderConfig {
        cache_key: llm_provider_cache_key(&provider_id, &params),
        provider_id,
        managed_transport_active,
        config,
    })
}

pub(crate) fn resolve_one_off_llm_config(
    base_config: &LlmConfig,
    llm_api_keys: &HashMap<String, String>,
    provider_id: &str,
    params: LlmProviderParams,
) -> Result<LlmConfig, PipelineError> {
    if let Some(error) = &base_config.selection_error {
        return Err(PipelineError::Config(error.clone()));
    }
    let provider_id = match &params.selection {
        Some(selection) => selected_llm_provider(selection)?,
        None => provider_id.trim().to_string(),
    };
    let api_key = if provider_id == "ollama" {
        String::new()
    } else if provider_id == "managed" {
        let token = base_config
            .managed_access_token
            .as_ref()
            .or_else(|| (base_config.provider == "managed").then_some(&base_config.api_key));
        if base_config.managed_gateway_url.as_deref().is_none()
            || token.is_none_or(|token| token.trim().is_empty())
        {
            return Err(PipelineError::Config(
                "Managed inference is not available for this session".to_string(),
            ));
        }
        token.cloned().unwrap_or_default()
    } else {
        llm_api_keys.get(&provider_id).cloned().unwrap_or_default()
    };

    if provider_id != "ollama" && api_key.trim().is_empty() {
        return Err(PipelineError::Config(format!(
            "No API key configured for provider: {}",
            provider_id
        )));
    }

    let mut config = base_config.clone();
    config.enabled = true;
    config.managed_gateway_url = if provider_id == "managed" {
        base_config.managed_gateway_url.clone()
    } else {
        None
    };
    config.managed_access_token = None;
    config.provider = provider_id;
    config.api_key = api_key;
    config.model = params
        .selection
        .as_ref()
        .map(|choice| choice.model_id.clone())
        .or(params.model);
    config.selection = params.selection;
    config.ollama_url = params.ollama_url;
    config.timeout = params.timeout;
    config.openai_reasoning_effort = params.openai_reasoning_effort;
    config.gemini_thinking_budget = params.gemini_thinking_budget;
    config.gemini_thinking_level = params.gemini_thinking_level;
    config.anthropic_thinking_budget = params.anthropic_thinking_budget;
    // One-off callers provide prompts explicitly and should not drag along persisted profile
    // matching state into ad-hoc completions.
    config.prompts = PromptSections::default();
    config.program_prompt_profiles.clear();

    validate_custom_llm(&config)?;
    Ok(config)
}

fn selected_llm_provider(
    selection: &crate::model_selection::ModelSelection,
) -> Result<String, PipelineError> {
    selection.validate().map_err(PipelineError::Config)?;
    if selection.source == crate::model_selection::ModelSource::Managed {
        return Ok("managed".into());
    }
    let provider = selection.provider_id.as_str();
    if !provider.starts_with("custom_")
        && !matches!(
            provider,
            "openai"
                | "anthropic"
                | "groq"
                | "cerebras"
                | "gemini"
                | "cohere"
                | "fireworks"
                | "ollama"
        )
    {
        return Err(PipelineError::Config("The selected language-model provider is unavailable; your model choice has not changed".into()));
    }
    Ok(provider.into())
}

fn validate_custom_llm(config: &LlmConfig) -> Result<(), PipelineError> {
    if !config.provider.starts_with("custom_") {
        return Ok(());
    }
    let provider = config
        .custom_providers
        .iter()
        .find(|p| p.id == config.provider)
        .ok_or_else(|| PipelineError::Config("Custom provider is no longer configured".into()))?;
    if !config
        .model
        .as_ref()
        .is_some_and(|m| provider.llm_models.contains(m))
    {
        return Err(PipelineError::Config(
            "Select a configured custom language model".into(),
        ));
    }
    Ok(())
}

fn custom_llm(config: &LlmConfig, client: reqwest::Client) -> GroqLlmProvider {
    // All callers validate before construction. An absent endpoint is never
    // replaced with a vendor endpoint, even for malformed in-memory config.
    let endpoint = config
        .custom_providers
        .iter()
        .find(|p| p.id == config.provider)
        .map(|p| format!("{}/chat/completions", p.base_url))
        .unwrap_or_default();
    GroqLlmProvider::with_client(client, config.api_key.clone(), config.model.clone())
        .for_custom_endpoint(endpoint)
}

pub(crate) fn create_one_off_llm_provider_unstructured(
    base_config: &LlmConfig,
    llm_api_keys: &HashMap<String, String>,
    provider_id: &str,
    params: LlmProviderParams,
    proxy_settings: &ProxySettings,
) -> Result<Arc<dyn LlmProvider>, PipelineError> {
    let config = resolve_one_off_llm_config(base_config, llm_api_keys, provider_id, params)?;
    if config.provider.starts_with("custom_") {
        let client = crate::network::build_custom_provider_client(proxy_settings)
            .map_err(PipelineError::Config)?;
        return Ok(Arc::new(
            custom_llm(&config, client).with_timeout(config.timeout),
        ));
    }
    let client =
        crate::network::build_http_client(proxy_settings).map_err(PipelineError::Config)?;
    Ok(create_llm_provider_unstructured(&config, client))
}

pub(crate) fn create_one_off_llm_provider_without_timeout(
    base_config: &LlmConfig,
    llm_api_keys: &HashMap<String, String>,
    provider_id: &str,
    params: LlmProviderParams,
    proxy_settings: &ProxySettings,
    request_log_store: Option<RequestLogStore>,
) -> Result<Arc<dyn LlmProvider>, PipelineError> {
    let config = resolve_one_off_llm_config(base_config, llm_api_keys, provider_id, params)?;
    if config.provider.starts_with("custom_") {
        let client = crate::network::build_custom_provider_client(proxy_settings)
            .map_err(PipelineError::Config)?;
        return Ok(Arc::new(
            custom_llm(&config, client)
                .without_timeout()
                .with_request_log_store(request_log_store),
        ));
    }
    let client =
        crate::network::build_http_client(proxy_settings).map_err(PipelineError::Config)?;
    Ok(create_llm_provider_without_timeout(
        &config,
        client,
        request_log_store,
    ))
}

/// Create an LLM provider for one-off ad-hoc completions where callers expect
/// free-form text instead of rewrite-oriented structured outputs.
///
/// Keeping this constructor in the LLM Provider Resolution Module prevents
/// command handlers and Quick Actions from growing provider-specific match
/// statements whenever a provider knob is added.
fn create_llm_provider_unstructured(
    config: &LlmConfig,
    client: reqwest::Client,
) -> Arc<dyn LlmProvider> {
    match config.provider.as_str() {
        "managed" => {
            let api_url = config
                .managed_gateway_url
                .as_deref()
                .and_then(|gateway| managed_llm_api_base_url("managed", gateway))
                .unwrap_or_default();
            Arc::new(
                ManagedLlmProvider::with_client(
                    client,
                    config.api_key.clone(),
                    config.model.clone(),
                    api_url,
                )
                .with_selected_provider(
                    config
                        .selection
                        .as_ref()
                        .map(|choice| choice.provider_id.clone()),
                )
                .with_timeout(config.timeout),
            )
        }
        "cerebras" => {
            let provider = CerebrasLlmProvider::with_client(
                client,
                config.api_key.clone(),
                config.model.clone(),
            );
            Arc::new(
                provider
                    .with_timeout(config.timeout)
                    .with_reasoning_effort(config.openai_reasoning_effort.clone()),
            )
        }
        "anthropic" => {
            let provider = AnthropicLlmProvider::with_client(
                client,
                config.api_key.clone(),
                config.model.clone(),
            );
            Arc::new(
                provider
                    .with_timeout(config.timeout)
                    .with_thinking_budget(config.anthropic_thinking_budget),
            )
        }
        "groq" => {
            let provider =
                GroqLlmProvider::with_client(client, config.api_key.clone(), config.model.clone());
            Arc::new(provider.with_timeout(config.timeout))
        }
        "gemini" => {
            let provider = GeminiLlmProvider::with_client(
                client,
                config.api_key.clone(),
                config.model.clone(),
            );

            Arc::new(
                provider
                    .with_timeout(config.timeout)
                    .with_thinking_budget(config.gemini_thinking_budget)
                    .with_thinking_level(config.gemini_thinking_level.clone())
                    .with_structured_outputs(false),
            )
        }
        "cohere" => {
            let provider = CohereLlmProvider::with_client(
                client,
                config.api_key.clone(),
                config.model.clone(),
            );
            Arc::new(provider.with_timeout(config.timeout))
        }
        "fireworks" => {
            let provider = FireworksLlmProvider::with_client(
                client,
                config.api_key.clone(),
                config.model.clone(),
            );
            Arc::new(provider.with_timeout(config.timeout))
        }
        "ollama" => {
            let provider = OllamaLlmProvider::with_client(
                client,
                config.ollama_url.clone(),
                config.model.clone(),
            );
            Arc::new(provider.with_timeout(config.timeout))
        }
        _ => {
            // Default to OpenAI
            let provider = OpenAiLlmProvider::with_client(
                client,
                config.api_key.clone(),
                config.model.clone(),
            );
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
fn create_llm_provider_without_timeout(
    config: &LlmConfig,
    client: reqwest::Client,
    request_log_store: Option<RequestLogStore>,
) -> Arc<dyn LlmProvider> {
    match config.provider.as_str() {
        "managed" => {
            let api_url = config
                .managed_gateway_url
                .as_deref()
                .and_then(|gateway| managed_llm_api_base_url("managed", gateway))
                .unwrap_or_default();
            Arc::new(
                ManagedLlmProvider::with_client(
                    client,
                    config.api_key.clone(),
                    config.model.clone(),
                    api_url,
                )
                .with_selected_provider(
                    config
                        .selection
                        .as_ref()
                        .map(|choice| choice.provider_id.clone()),
                )
                .without_timeout()
                .with_request_log_store(request_log_store.clone()),
            )
        }
        "cerebras" => {
            let provider = CerebrasLlmProvider::with_client(
                client,
                config.api_key.clone(),
                config.model.clone(),
            );
            Arc::new(
                provider
                    .without_timeout()
                    .with_request_log_store(request_log_store.clone())
                    .with_reasoning_effort(config.openai_reasoning_effort.clone()),
            )
        }
        "anthropic" => {
            let provider = AnthropicLlmProvider::with_client(
                client,
                config.api_key.clone(),
                config.model.clone(),
            );
            Arc::new(
                provider
                    .without_timeout()
                    .with_request_log_store(request_log_store.clone())
                    .with_thinking_budget(config.anthropic_thinking_budget),
            )
        }
        "groq" => {
            let provider =
                GroqLlmProvider::with_client(client, config.api_key.clone(), config.model.clone());
            Arc::new(
                provider
                    .without_timeout()
                    .with_request_log_store(request_log_store.clone()),
            )
        }
        "gemini" => {
            let provider = GeminiLlmProvider::with_client(
                client,
                config.api_key.clone(),
                config.model.clone(),
            );

            Arc::new(
                provider
                    .without_timeout()
                    .with_request_log_store(request_log_store.clone())
                    .with_thinking_budget(config.gemini_thinking_budget)
                    .with_thinking_level(config.gemini_thinking_level.clone()),
            )
        }
        "cohere" => {
            let provider = CohereLlmProvider::with_client(
                client,
                config.api_key.clone(),
                config.model.clone(),
            );
            Arc::new(
                provider
                    .without_timeout()
                    .with_request_log_store(request_log_store.clone()),
            )
        }
        "fireworks" => {
            let provider = FireworksLlmProvider::with_client(
                client,
                config.api_key.clone(),
                config.model.clone(),
            );
            Arc::new(
                provider
                    .without_timeout()
                    .with_request_log_store(request_log_store.clone()),
            )
        }
        "ollama" => {
            let provider = OllamaLlmProvider::with_client(
                client,
                config.ollama_url.clone(),
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
            let provider = OpenAiLlmProvider::with_client(
                client,
                config.api_key.clone(),
                config.model.clone(),
            );
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

    if config.provider.starts_with("custom_") {
        validate_custom_llm(config)?;
        let client = crate::network::build_custom_provider_client(proxy_settings)
            .map_err(PipelineError::Config)?;
        return Ok(Arc::new(
            custom_llm(config, client)
                .with_timeout(config.timeout)
                .with_request_log_store(request_log_store),
        ));
    }

    let provider: Arc<dyn LlmProvider> = match config.provider.as_str() {
        "managed" => {
            let gateway = config.managed_gateway_url.as_deref().ok_or_else(|| {
                crate::pipeline::PipelineError::Config(
                    "Managed inference gateway is not configured".to_string(),
                )
            })?;
            let api_url = managed_llm_api_base_url("managed", gateway).ok_or_else(|| {
                crate::pipeline::PipelineError::Config(
                    "Managed inference gateway URL is invalid".to_string(),
                )
            })?;
            Arc::new(
                ManagedLlmProvider::with_client(
                    client.clone(),
                    config.api_key.clone(),
                    config.model.clone(),
                    api_url,
                )
                .with_selected_provider(
                    config
                        .selection
                        .as_ref()
                        .map(|choice| choice.provider_id.clone()),
                )
                .with_timeout(config.timeout)
                .with_request_log_store(request_log_store.clone()),
            )
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn managed_constructors_pin_atomic_provider_and_model_without_vendor_credentials() {
        use crate::model_selection::{ModelSelection, ModelSource};
        use wiremock::{
            matchers::{method, path},
            Mock, MockServer, ResponseTemplate,
        };
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({"choices":[{"message":{"content":"synthetic output"}}]}),
            ))
            .expect(6)
            .mount(&server)
            .await;
        for selection in [
            None,
            Some(ModelSelection {
                source: ModelSource::Managed,
                provider_id: "openai".into(),
                model_id: "raw-model".into(),
            }),
        ] {
            let config = LlmConfig {
                provider: "managed".into(),
                api_key: "synthetic-session".into(),
                model: Some("raw-model".into()),
                managed_gateway_url: Some(server.uri()),
                selection: selection.clone(),
                ..Default::default()
            };
            let providers = [
                create_llm_provider_unstructured(&config, reqwest::Client::new()),
                create_llm_provider_without_timeout(&config, reqwest::Client::new(), None),
                create_llm_provider(&config, None, &Default::default()).unwrap(),
            ];
            for provider in providers {
                assert_eq!(
                    provider.complete("sys", "user").await.unwrap(),
                    "synthetic output"
                );
                assert_eq!(provider.model(), "raw-model");
            }
        }
        for (index, request) in server.received_requests().await.unwrap().iter().enumerate() {
            assert_eq!(
                request.headers.get("authorization").unwrap(),
                "Bearer synthetic-session"
            );
            assert_eq!(
                request
                    .headers
                    .get("x-kolboo-model-provider")
                    .map(|value| value.to_str().unwrap()),
                (index >= 3).then_some("openai")
            );
            assert_eq!(
                serde_json::from_slice::<serde_json::Value>(&request.body).unwrap()["model"],
                "raw-model"
            );
        }
    }

    fn atomic_params(
        source: crate::model_selection::ModelSource,
        provider: &str,
        model: &str,
    ) -> LlmProviderParams {
        LlmProviderParams {
            selection: Some(crate::model_selection::ModelSelection {
                source,
                provider_id: provider.into(),
                model_id: model.into(),
            }),
            model: Some("stale-model".into()),
            timeout: Duration::from_secs(10),
            ollama_url: None,
            openai_reasoning_effort: None,
            gemini_thinking_budget: None,
            gemini_thinking_level: None,
            anthropic_thinking_budget: None,
        }
    }

    #[test]
    fn atomic_routes_do_not_change_model_source_or_reuse_other_credentials() {
        use crate::model_selection::ModelSource::{Byok, Managed};
        let mut config = PipelineConfig {
            managed_inference_enabled: true,
            managed_inference_gateway_url: Some("https://gateway.example.test".into()),
            managed_inference_access_token: Some("synthetic-session".into()),
            ..Default::default()
        };
        config.llm_config.managed_gateway_url = config.managed_inference_gateway_url.clone();
        config.llm_config.managed_access_token = config.managed_inference_access_token.clone();
        config.llm_config.api_key = "unrelated-key".into();
        for provider in [
            "openai",
            "anthropic",
            "groq",
            "cerebras",
            "gemini",
            "cohere",
            "fireworks",
            "ollama",
        ] {
            config
                .llm_api_keys
                .insert(provider.into(), format!("synthetic-{provider}"));
            let params = atomic_params(Byok, provider, " raw::א ");
            let resolved =
                resolve_cached_llm_provider_config(&config, "managed", params.clone()).unwrap();
            assert_eq!(resolved.provider_id, provider);
            assert!(!resolved.managed_transport_active);
            assert_eq!(resolved.config.model.as_deref(), Some(" raw::א "));
            assert!(
                resolved.config.managed_gateway_url.is_none()
                    && resolved.config.managed_access_token.is_none()
            );
            let oneoff = resolve_one_off_llm_config(
                &config.llm_config,
                &config.llm_api_keys,
                "managed",
                params,
            )
            .unwrap();
            assert_eq!(oneoff.provider, provider);
            assert!(oneoff.managed_gateway_url.is_none());
            assert_eq!(
                oneoff.api_key,
                if provider == "ollama" {
                    String::new()
                } else {
                    format!("synthetic-{provider}")
                }
            );
        }
        let managed = atomic_params(Managed, "openai", "catalog-model");
        let cached = resolve_cached_llm_provider_config(&config, "groq", managed.clone()).unwrap();
        assert_eq!(cached.provider_id, "managed");
        assert_eq!(cached.config.api_key, "synthetic-session");
        assert_eq!(cached.config.model.as_deref(), Some("catalog-model"));
        let oneoff = resolve_one_off_llm_config(
            &config.llm_config,
            &config.llm_api_keys,
            "groq",
            managed.clone(),
        )
        .unwrap();
        assert_eq!(oneoff.api_key, "synthetic-session");
        assert_eq!(oneoff.model.as_deref(), Some("catalog-model"));
        assert_ne!(
            llm_provider_cache_key("managed", &managed),
            llm_provider_cache_key("openai", &atomic_params(Byok, "openai", "catalog-model"))
        );
        for provider in ["missing", "custom_missing"] {
            assert!(resolve_cached_llm_provider_config(
                &config,
                "openai",
                atomic_params(Byok, provider, "raw")
            )
            .is_err());
            assert!(resolve_one_off_llm_config(
                &config.llm_config,
                &config.llm_api_keys,
                "openai",
                atomic_params(Byok, provider, "raw")
            )
            .is_err());
        }
        assert!(resolve_one_off_llm_config(
            &config.llm_config,
            &config.llm_api_keys,
            "openai",
            atomic_params(Byok, "", "raw")
        )
        .is_err());
        config.model_selection_error = Some("Invalid model settings".into());
        assert!(resolve_cached_llm_provider_config(&config, "openai", managed.clone()).is_err());
        config.llm_config.selection_error = config.model_selection_error.take();
        assert!(resolve_one_off_llm_config(
            &config.llm_config,
            &config.llm_api_keys,
            "openai",
            managed.clone()
        )
        .is_err());
        config.llm_config.selection_error = None;
        config.llm_config.managed_access_token = None;
        // A vendor key must not be interpreted as a Managed session token.
        assert!(resolve_one_off_llm_config(
            &config.llm_config,
            &config.llm_api_keys,
            "managed",
            managed.clone()
        )
        .is_err());
        config.llm_config.provider = "managed".into();
        assert_eq!(
            resolve_one_off_llm_config(
                &config.llm_config,
                &config.llm_api_keys,
                "managed",
                managed.clone()
            )
            .unwrap()
            .api_key,
            "unrelated-key"
        );
        config.llm_config.managed_gateway_url = None;
        assert!(resolve_one_off_llm_config(
            &config.llm_config,
            &config.llm_api_keys,
            "managed",
            managed.clone()
        )
        .is_err());
        config.llm_config.managed_gateway_url = Some("https://gateway.example.test".into());
        config.llm_config.api_key = " ".into();
        assert!(resolve_one_off_llm_config(
            &config.llm_config,
            &config.llm_api_keys,
            "managed",
            managed
        )
        .is_err());
    }
    use crate::llm::PromptSections;

    fn custom_config(base_url: String) -> LlmConfig {
        LlmConfig {
            provider: "custom_test".into(),
            api_key: "fake-key".into(),
            model: Some("my-model".into()),
            custom_providers: vec![crate::custom_providers::CustomProvider {
                id: "custom_test".into(),
                name: "Example".into(),
                base_url,
                stt_models: vec![],
                llm_models: vec!["my-model".into()],
                stt_model_names: Default::default(),
                llm_model_names: Default::default(),
            }],
            ..Default::default()
        }
    }

    #[test]
    fn one_off_clients_reject_invalid_proxy_for_every_provider() {
        use crate::pipeline::llm_provider_request::resolve_one_off_llm_request_for_profile;
        let proxy = ProxySettings {
            mode: crate::settings::ProxyMode::Manual,
            ..Default::default()
        };
        for id in [
            "custom_test",
            "managed",
            "cerebras",
            "anthropic",
            "groq",
            "gemini",
            "cohere",
            "fireworks",
            "ollama",
            "openai",
        ] {
            let mut config = custom_config("http://localhost:1234/v1".into());
            config.provider = id.into();
            config.managed_gateway_url = Some("https://unused.invalid".into());
            let keys = HashMap::from([(id.to_string(), "fixture-key".to_string())]);
            let request = resolve_one_off_llm_request_for_profile(&config, None, &[]);
            assert!(
                request.create_unstructured(&config, &keys, &proxy).is_err(),
                "{id}"
            );
            assert!(
                request
                    .create_without_timeout(&config, &keys, &proxy, None)
                    .is_err(),
                "{id}"
            );
        }
    }

    #[tokio::test]
    async fn one_off_custom_clients_honor_manual_proxy_and_bypass() {
        use crate::pipeline::llm_provider_request::resolve_one_off_llm_request_for_profile;
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};
        let destination = MockServer::start().await;
        let proxy_server = MockServer::start().await;
        for (server, text, expected) in [(&destination, "direct", 4), (&proxy_server, "proxied", 2)]
        {
            Mock::given(method("POST"))
                .and(path("/v1/chat/completions"))
                .respond_with(
                    ResponseTemplate::new(200).set_body_json(
                        serde_json::json!({"choices":[{"message":{"content":text}}]}),
                    ),
                )
                .expect(expected)
                .mount(server)
                .await;
        }
        let config = custom_config(format!("{}/v1", destination.uri()));
        let keys = HashMap::from([("custom_test".to_string(), "fixture-key".to_string())]);
        let request = resolve_one_off_llm_request_for_profile(&config, None, &[]);
        for mode in 0..3 {
            let mut proxy = ProxySettings {
                mode: crate::settings::ProxyMode::Manual,
                ..Default::default()
            };
            proxy.manual.proxy_url = proxy_server.uri();
            proxy.manual.no_proxy.clear();
            if mode == 1 {
                proxy.manual.no_proxy = "127.0.0.1".into();
            }
            if mode == 2 {
                proxy.mode = crate::settings::ProxyMode::NoProxy;
            }
            let expected = if mode == 0 { "proxied" } else { "direct" };
            let provider = request.create_unstructured(&config, &keys, &proxy).unwrap();
            assert_eq!(
                provider.complete("fixture", "fixture").await.unwrap(),
                expected
            );
            let provider = request
                .create_without_timeout(&config, &keys, &proxy, None)
                .unwrap();
            assert_eq!(
                provider.complete("fixture", "fixture").await.unwrap(),
                expected
            );
        }
    }

    #[test]
    fn custom_llm_rejects_removed_provider_and_model() {
        let mut config = custom_config("http://localhost:1234/v1".into());
        assert!(validate_custom_llm(&config).is_ok());
        config.model = Some("not-configured".into());
        assert!(validate_custom_llm(&config).is_err());
        config.custom_providers.clear();
        assert!(create_llm_provider(&config, None, &ProxySettings::default()).is_err());
    }

    #[tokio::test]
    async fn custom_llm_posts_own_key_to_configured_chat_endpoint() {
        use wiremock::matchers::{body_partial_json, header, method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .and(header("authorization", "Bearer fake-key"))
            .and(body_partial_json(serde_json::json!({"model": "my-model"})))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"choices":[{"message":{"content":"ok"}}]})),
            )
            .expect(1)
            .mount(&server)
            .await;
        let mut config = custom_config(format!("{}/v1", server.uri()));
        config.managed_gateway_url = Some("https://unused.invalid".into());
        let provider = create_llm_provider(&config, None, &ProxySettings::default()).unwrap();
        assert_eq!(provider.name(), "custom");
        assert_eq!(provider.complete("system", "hello").await.unwrap(), "ok");
    }

    #[tokio::test]
    async fn custom_llm_does_not_follow_redirects() {
        use wiremock::matchers::method;
        use wiremock::{Mock, MockServer, ResponseTemplate};
        let target = MockServer::start().await;
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(307).insert_header("Location", target.uri()))
            .expect(1)
            .mount(&server)
            .await;
        let config = custom_config(server.uri());
        let provider = create_llm_provider(&config, None, &ProxySettings::default()).unwrap();
        assert!(provider.complete("system", "hello").await.is_err());
        assert!(target.received_requests().await.unwrap().is_empty());
    }

    #[test]
    fn llm_provider_cache_key_tracks_provider_identity_knobs() {
        let params_a = LlmProviderParams {
            selection: None,
            model: Some("gpt-5".to_string()),
            timeout: Duration::from_secs(30),
            ollama_url: None,
            openai_reasoning_effort: Some("low".to_string()),
            gemini_thinking_budget: None,
            gemini_thinking_level: None,
            anthropic_thinking_budget: None,
        };
        let params_b = LlmProviderParams {
            selection: None,
            openai_reasoning_effort: Some("high".to_string()),
            ..params_a.clone()
        };

        assert_ne!(
            llm_provider_cache_key("openai", &params_a),
            llm_provider_cache_key("openai", &params_b)
        );
    }

    #[test]
    fn resolve_cached_managed_config_uses_session_token_and_gateway() {
        let config = PipelineConfig {
            managed_inference_enabled: true,
            managed_inference_gateway_url: Some("https://managed.example.test".to_string()),
            managed_inference_access_token: Some("managed-token".to_string()),
            ..PipelineConfig::default()
        };

        let resolved = resolve_cached_llm_provider_config(
            &config,
            "managed",
            LlmProviderParams {
                selection: None,
                model: Some("gpt-5".to_string()),
                timeout: Duration::from_secs(45),
                ollama_url: None,
                openai_reasoning_effort: Some("medium".to_string()),
                gemini_thinking_budget: None,
                gemini_thinking_level: None,
                anthropic_thinking_budget: None,
            },
        )
        .expect("managed runtime config should resolve");

        assert_eq!(resolved.provider_id, "managed");
        assert!(resolved.managed_transport_active);
        assert_eq!(resolved.config.api_key, "managed-token");
        assert_eq!(
            resolved.config.managed_gateway_url.as_deref(),
            Some("https://managed.example.test")
        );
    }

    #[test]
    fn resolve_cached_byok_config_uses_provider_key_for_entitled_user() {
        let mut config = PipelineConfig {
            managed_inference_enabled: true,
            managed_inference_gateway_url: Some("https://managed.example.test".to_string()),
            managed_inference_access_token: Some("managed-token".to_string()),
            ..PipelineConfig::default()
        };
        config
            .llm_api_keys
            .insert("openai".to_string(), "user-openai-key".to_string());

        let resolved = resolve_cached_llm_provider_config(
            &config,
            "openai",
            LlmProviderParams {
                selection: None,
                model: Some("gpt-5".to_string()),
                timeout: Duration::from_secs(45),
                ollama_url: None,
                openai_reasoning_effort: Some("medium".to_string()),
                gemini_thinking_budget: None,
                gemini_thinking_level: None,
                anthropic_thinking_budget: None,
            },
        )
        .expect("BYOK runtime config should resolve");

        assert_eq!(resolved.provider_id, "openai");
        assert!(!resolved.managed_transport_active);
        assert_eq!(resolved.config.api_key, "user-openai-key");
        assert!(resolved.config.managed_gateway_url.is_none());
    }

    #[test]
    fn resolve_cached_managed_config_fails_without_managed_session() {
        let error = match resolve_cached_llm_provider_config(
            &PipelineConfig::default(),
            "managed",
            LlmProviderParams {
                selection: None,
                model: Some("gpt-5".to_string()),
                timeout: Duration::from_secs(45),
                ollama_url: None,
                openai_reasoning_effort: None,
                gemini_thinking_budget: None,
                gemini_thinking_level: None,
                anthropic_thinking_budget: None,
            },
        ) {
            Ok(_) => panic!("managed runtime config must fail without a managed session"),
            Err(error) => error,
        };

        assert!(error
            .to_string()
            .contains("Managed inference is not available"));
    }

    #[test]
    fn resolve_one_off_config_uses_provider_key_and_resets_prompt_state() {
        let base = LlmConfig {
            managed_gateway_url: Some("https://gateway.example.test".to_string()),
            prompts: PromptSections {
                system_custom: Some("Persisted prompt".to_string()),
            },
            ..LlmConfig::default()
        };

        let mut llm_api_keys = HashMap::new();
        llm_api_keys.insert("groq".to_string(), "groq-key".to_string());

        let resolved = resolve_one_off_llm_config(
            &base,
            &llm_api_keys,
            "groq",
            LlmProviderParams {
                selection: None,
                model: Some("llama-3.3-70b-versatile".to_string()),
                timeout: Duration::from_secs(30),
                ollama_url: None,
                openai_reasoning_effort: None,
                gemini_thinking_budget: None,
                gemini_thinking_level: None,
                anthropic_thinking_budget: None,
            },
        )
        .expect("one-off config should resolve");

        assert_eq!(resolved.provider, "groq");
        assert_eq!(resolved.api_key, "groq-key");
        assert_eq!(resolved.model.as_deref(), Some("llama-3.3-70b-versatile"));
        assert!(resolved.managed_gateway_url.is_none());
        assert!(resolved.prompts.system_custom.is_none());
        assert!(resolved.program_prompt_profiles.is_empty());
    }

    #[test]
    fn resolve_one_off_managed_config_uses_session_token() {
        let base = LlmConfig {
            api_key: "unused-user-key".to_string(),
            managed_access_token: Some("managed-session-token".to_string()),
            managed_gateway_url: Some("https://gateway.example.test".to_string()),
            ..LlmConfig::default()
        };

        let resolved = resolve_one_off_llm_config(
            &base,
            &HashMap::new(),
            "managed",
            LlmProviderParams {
                selection: None,
                model: Some("gemini-3-flash".to_string()),
                timeout: Duration::from_secs(30),
                ollama_url: None,
                openai_reasoning_effort: None,
                gemini_thinking_budget: None,
                gemini_thinking_level: None,
                anthropic_thinking_budget: None,
            },
        )
        .expect("managed one-off config should resolve");

        assert_eq!(resolved.provider, "managed");
        assert_eq!(resolved.api_key, "managed-session-token");
        assert_eq!(resolved.model.as_deref(), Some("gemini-3-flash"));
    }

    #[test]
    fn resolve_one_off_managed_config_fails_without_session() {
        let error = resolve_one_off_llm_config(
            &LlmConfig {
                managed_gateway_url: Some("https://gateway.example.test".to_string()),
                ..LlmConfig::default()
            },
            &HashMap::new(),
            "managed",
            LlmProviderParams {
                selection: None,
                model: None,
                timeout: Duration::from_secs(30),
                ollama_url: None,
                openai_reasoning_effort: None,
                gemini_thinking_budget: None,
                gemini_thinking_level: None,
                anthropic_thinking_budget: None,
            },
        )
        .expect_err("managed one-off config must fail without a session token");

        assert!(error.to_string().contains("not available for this session"));
    }
}
