//! Anthropic (Claude) LLM provider for text formatting.

use super::{LlmError, LlmProvider, DEFAULT_LLM_TIMEOUT};
use async_trait::async_trait;
use crate::request_log::RequestLogStore;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::Duration;

const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";
const DEFAULT_MODEL: &str = "claude-3-haiku-20240307";
const API_VERSION: &str = "2023-06-01";

/// Anthropic (Claude) LLM provider using the Messages API
pub struct AnthropicLlmProvider {
    client: Client,
    api_key: String,
    model: String,
    timeout: Option<Duration>,
    thinking_budget_tokens: Option<i64>,
    request_log_store: Option<RequestLogStore>,
}

impl AnthropicLlmProvider {
    /// Create a new Anthropic provider with the given API key
    pub fn new(api_key: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
            model: DEFAULT_MODEL.to_string(),
            timeout: Some(DEFAULT_LLM_TIMEOUT),
            thinking_budget_tokens: None,
            request_log_store: None,
        }
    }

    /// Create with a specific model
    pub fn with_model(api_key: String, model: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
            model,
            timeout: Some(DEFAULT_LLM_TIMEOUT),
            thinking_budget_tokens: None,
            request_log_store: None,
        }
    }

    /// Create with custom client and settings
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn with_client(client: Client, api_key: String, model: Option<String>) -> Self {
        Self {
            client,
            api_key,
            model: model.unwrap_or_else(|| DEFAULT_MODEL.to_string()),
            timeout: Some(DEFAULT_LLM_TIMEOUT),
            thinking_budget_tokens: None,
            request_log_store: None,
        }
    }

    pub fn with_request_log_store(mut self, store: Option<RequestLogStore>) -> Self {
        self.request_log_store = store;
        self
    }

    /// Set the request timeout
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Disable request timeouts entirely.
    ///
    /// This is primarily intended for the Settings UI "Test" actions.
    pub fn without_timeout(mut self) -> Self {
        self.timeout = None;
        self
    }

    pub fn with_thinking_budget(mut self, budget_tokens: Option<i64>) -> Self {
        // Allow callers (UI/profile overrides) to explicitly disable thinking
        // by passing 0 (or negative) without producing noisy warnings later.
        self.thinking_budget_tokens = match budget_tokens {
            Some(n) if n <= 0 => None,
            other => other,
        };
        self
    }

    fn supports_extended_thinking(model: &str) -> bool {
        let m = model.to_ascii_lowercase();

        // Extended thinking is supported by newer Claude families. Be conservative:
        // - Claude 3.7 (explicit)
        // - Claude 4.x / 4.5 families
        //
        // (We avoid enabling for older 3.0/3.5 models unless explicitly named.)
        m.contains("claude-3-7")
            || m.contains("claude-4")
            || m.contains("-4-")
            || m.contains("-4.0")
            || m.contains("-4-5")
    }

    fn effective_thinking(&self) -> Option<ThinkingParam> {
        let budget = self.thinking_budget_tokens?;

        // Defensive: `with_thinking_budget` should already normalize this, but
        // keep the behavior stable if the struct is constructed differently.
        if budget <= 0 {
            return None;
        }

        if !Self::supports_extended_thinking(&self.model) {
            log::warn!(
                "Unsupported Anthropic extended thinking for model '{}' (budget_tokens={}); ignoring",
                self.model,
                budget
            );
            return None;
        }

        // Anthropic cookbook (extended thinking) notes a minimum budget of 1024.
        if budget < 1024 {
            log::warn!(
                "Anthropic thinking budget too small ({}); minimum is 1024. Ignoring.",
                budget
            );
            return None;
        }

        // Defensive cap to avoid obviously unreasonable values. Actual max may vary by model.
        if budget > 32768 {
            log::warn!(
                "Anthropic thinking budget too large ({}); capping at 32768.",
                budget
            );
        }

        let capped: i64 = budget.min(32768);

        Some(ThinkingParam {
            thinking_type: "enabled".to_string(),
            budget_tokens: capped as u32,
        })
    }
}

#[derive(Debug, Serialize)]
struct MessageContent {
    #[serde(rename = "type")]
    content_type: String,
    text: String,
}

#[derive(Debug, Serialize)]
struct Message {
    role: String,
    content: Vec<MessageContent>,
}

#[derive(Debug, Serialize)]
struct MessagesRequest {
    model: String,
    max_tokens: u32,
    system: String,
    messages: Vec<Message>,

    #[serde(skip_serializing_if = "Option::is_none")]
    thinking: Option<ThinkingParam>,
}

#[derive(Debug, Serialize)]
struct ThinkingParam {
    #[serde(rename = "type")]
    thinking_type: String,
    #[serde(rename = "budget_tokens")]
    budget_tokens: u32,
}

#[derive(Debug, Deserialize)]
struct ContentBlock {
    #[serde(rename = "type")]
    content_type: String,
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MessagesResponse {
    content: Vec<ContentBlock>,
}

#[derive(Debug, Deserialize)]
struct ErrorResponse {
    error: ErrorDetail,
}

#[derive(Debug, Deserialize)]
struct ErrorDetail {
    message: String,
}

#[async_trait]
impl LlmProvider for AnthropicLlmProvider {
    async fn complete(&self, system_prompt: &str, user_message: &str) -> Result<String, LlmError> {
        if self.api_key.is_empty() {
            return Err(LlmError::NoApiKey("anthropic".to_string()));
        }

        let request = MessagesRequest {
            model: self.model.clone(),
            max_tokens: 4096,
            system: system_prompt.to_string(),
            messages: vec![Message {
                role: "user".to_string(),
                content: vec![MessageContent {
                    content_type: "text".to_string(),
                    text: user_message.to_string(),
                }],
            }],
            thinking: self.effective_thinking(),
        };

        if let Some(store) = &self.request_log_store {
            let request_json = serde_json::to_value(&request).unwrap_or_else(|_| {
                json!({
                    "provider": "anthropic",
                    "error": "failed to serialize request",
                })
            });
            store.with_current(|log| {
                log.llm_request_json = Some(request_json);
            });
        }

        let mut req = self
            .client
            .post(ANTHROPIC_API_URL)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", API_VERSION)
            .header("content-type", "application/json")
            .json(&request);
        if let Some(timeout) = self.timeout {
            req = req.timeout(timeout);
        }

        let response = req.send().await.map_err(|e| {
            if e.is_timeout() {
                if let Some(timeout) = self.timeout {
                    LlmError::Timeout(timeout)
                } else {
                    // If we didn't configure a timeout, treat this as a generic network error.
                    LlmError::Network(e)
                }
            } else {
                LlmError::Network(e)
            }
        })?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            // Try to parse as error response
            if let Ok(error_response) = serde_json::from_str::<ErrorResponse>(&error_text) {
                return Err(LlmError::Api(format!(
                    "Anthropic API error ({}): {}",
                    status, error_response.error.message
                )));
            }
            return Err(LlmError::Api(format!(
                "Anthropic API error ({}): {}",
                status, error_text
            )));
        }

        let response_json: serde_json::Value = response.json().await.map_err(|e| {
            LlmError::InvalidResponse(format!("Failed to parse response: {}", e))
        })?;

        if let Some(store) = &self.request_log_store {
            let response_for_log = response_json.clone();
            store.with_current(|log| {
                log.llm_response_json = Some(response_for_log);
            });
        }

        // Extract the first text content block.
        response_json
            .get("content")
            .and_then(|v| v.as_array())
            .and_then(|arr| {
                arr.iter().find_map(|block| {
                    let ty = block.get("type").and_then(|t| t.as_str());
                    if ty != Some("text") {
                        return None;
                    }
                    block.get("text").and_then(|t| t.as_str()).map(|s| s.to_string())
                })
            })
            .ok_or_else(|| LlmError::InvalidResponse("No text content in response".to_string()))
    }

    fn name(&self) -> &'static str {
        "anthropic"
    }

    fn model(&self) -> &str {
        &self.model
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_name() {
        let provider = AnthropicLlmProvider::new("test-key".to_string());
        assert_eq!(provider.name(), "anthropic");
    }

    #[test]
    fn test_default_model() {
        let provider = AnthropicLlmProvider::new("test-key".to_string());
        assert_eq!(provider.model(), DEFAULT_MODEL);
    }

    #[test]
    fn test_custom_model() {
        let provider = AnthropicLlmProvider::with_model(
            "test-key".to_string(),
            "claude-3-opus-20240229".to_string(),
        );
        assert_eq!(provider.model(), "claude-3-opus-20240229");
    }

    #[test]
    fn test_without_timeout_disables_timeout() {
        let provider = AnthropicLlmProvider::new("test-key".to_string()).without_timeout();
        assert!(provider.timeout.is_none());
    }
}
