//! Fireworks LLM provider for text formatting.
//!
//! Fireworks exposes an OpenAI-compatible Chat Completions API at:
//! https://api.fireworks.ai/inference/v1/chat/completions

use super::{LlmError, LlmProvider, DEFAULT_LLM_TIMEOUT};
use async_trait::async_trait;
use crate::request_log::RequestLogStore;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::Duration;

const FIREWORKS_API_URL: &str = "https://api.fireworks.ai/inference/v1/chat/completions";

// Keep this in sync with `llm::defaults::default_llm_model_for_provider`.
const DEFAULT_MODEL: &str = "accounts/fireworks/models/llama-v3p1-8b-instruct";

/// Fireworks LLM provider using the OpenAI-compatible Chat Completions API.
pub struct FireworksLlmProvider {
    client: Client,
    api_key: String,
    model: String,
    timeout: Option<Duration>,
    request_log_store: Option<RequestLogStore>,
}

impl FireworksLlmProvider {
    /// Create a new Fireworks provider with the given API key.
    pub fn new(api_key: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
            model: DEFAULT_MODEL.to_string(),
            timeout: Some(DEFAULT_LLM_TIMEOUT),
            request_log_store: None,
        }
    }

    /// Create with a specific model.
    pub fn with_model(api_key: String, model: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
            model,
            timeout: Some(DEFAULT_LLM_TIMEOUT),
            request_log_store: None,
        }
    }

    /// Create with a custom HTTP client.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn with_client(client: Client, api_key: String, model: Option<String>) -> Self {
        Self {
            client,
            api_key,
            model: model.unwrap_or_else(|| DEFAULT_MODEL.to_string()),
            timeout: Some(DEFAULT_LLM_TIMEOUT),
            request_log_store: None,
        }
    }

    pub fn with_request_log_store(mut self, store: Option<RequestLogStore>) -> Self {
        self.request_log_store = store;
        self
    }

    /// Set the request timeout.
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
}

#[derive(Debug, Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    max_tokens: u32,
    temperature: f32,
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
impl LlmProvider for FireworksLlmProvider {
    async fn complete(&self, system_prompt: &str, user_message: &str) -> Result<String, LlmError> {
        if self.api_key.is_empty() {
            return Err(LlmError::NoApiKey("fireworks".to_string()));
        }

        let request = ChatRequest {
            model: self.model.clone(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: system_prompt.to_string(),
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: user_message.to_string(),
                },
            ],
            max_tokens: 4096,
            temperature: 0.3,
        };

        if let Some(store) = &self.request_log_store {
            let request_json = serde_json::to_value(&request).unwrap_or_else(|_| {
                json!({
                    "provider": "fireworks",
                    "error": "failed to serialize request",
                })
            });
            store.with_current(|log| {
                log.llm_request_json = Some(request_json);
            });
        }

        let mut req = self
            .client
            .post(FIREWORKS_API_URL)
            .bearer_auth(&self.api_key)
            .json(&request);
        if let Some(timeout) = self.timeout {
            req = req.timeout(timeout);
        }

        let response = req.send().await.map_err(|e| {
            if e.is_timeout() {
                if let Some(timeout) = self.timeout {
                    LlmError::Timeout(timeout)
                } else {
                    LlmError::Network(e)
                }
            } else {
                LlmError::Network(e)
            }
        })?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            if let Ok(error_response) = serde_json::from_str::<ErrorResponse>(&error_text) {
                return Err(LlmError::Api(format!(
                    "Fireworks API error ({}): {}",
                    status, error_response.error.message
                )));
            }
            return Err(LlmError::Api(format!(
                "Fireworks API error ({}): {}",
                status, error_text
            )));
        }

        let response_json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| LlmError::InvalidResponse(format!("Failed to parse response: {}", e)))?;

        if let Some(store) = &self.request_log_store {
            let response_for_log = response_json.clone();
            store.with_current(|log| {
                log.llm_response_json = Some(response_for_log);
            });
        }

        response_json
            .get("choices")
            .and_then(|v| v.as_array())
            .and_then(|choices| choices.first())
            .and_then(|choice| choice.get("message"))
            .and_then(|msg| msg.get("content"))
            .and_then(|c| c.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| LlmError::InvalidResponse("No response choices returned".to_string()))
    }

    fn name(&self) -> &'static str {
        "fireworks"
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
        let provider = FireworksLlmProvider::new("test-key".to_string());
        assert_eq!(provider.name(), "fireworks");
    }

    #[test]
    fn test_default_model() {
        let provider = FireworksLlmProvider::new("test-key".to_string());
        assert_eq!(provider.model(), DEFAULT_MODEL);
    }

    #[test]
    fn test_custom_model() {
        let provider = FireworksLlmProvider::with_model(
            "test-key".to_string(),
            "accounts/fireworks/models/llama-v3p1-70b-instruct".to_string(),
        );
        assert_eq!(provider.model(), "accounts/fireworks/models/llama-v3p1-70b-instruct");
    }

    #[test]
    fn test_without_timeout_disables_timeout() {
        let provider = FireworksLlmProvider::new("test-key".to_string()).without_timeout();
        assert!(provider.timeout.is_none());
    }
}
