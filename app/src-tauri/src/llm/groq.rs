//! Groq LLM provider for text formatting.
//!
//! Groq exposes an OpenAI-compatible Chat Completions API.
//! This provider uses the same request/response shape as OpenAI, but targets
//! Groq's base URL.

use super::openai_compat;
use super::{LlmError, LlmProvider, DEFAULT_LLM_TIMEOUT};
use crate::request_log::RequestLogStore;
use async_trait::async_trait;
use reqwest::Client;
use serde_json::json;
use std::time::Duration;

const GROQ_API_URL: &str = "https://api.groq.com/openai/v1/chat/completions";
const DEFAULT_MODEL: &str = "llama-3.3-70b-versatile";

/// Groq LLM provider using the OpenAI-compatible Chat Completions API.
pub struct GroqLlmProvider {
    client: Client,
    api_key: String,
    model: String,
    api_base_url: String,
    timeout: Option<Duration>,
    request_log_store: Option<RequestLogStore>,
}

impl GroqLlmProvider {
    /// Create a new Groq provider with the given API key.
    pub fn new(api_key: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
            model: DEFAULT_MODEL.to_string(),
            api_base_url: GROQ_API_URL.to_string(),
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
            api_base_url: GROQ_API_URL.to_string(),
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
            api_base_url: GROQ_API_URL.to_string(),
            timeout: Some(DEFAULT_LLM_TIMEOUT),
            request_log_store: None,
        }
    }

    /// Override the API base URL (defaults to https://api.groq.com/openai/v1/chat/completions).
    ///
    /// This is primarily intended for deterministic contract tests (e.g., Wiremock).
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn with_api_base_url(mut self, base_url: String) -> Self {
        self.api_base_url = base_url;
        self
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

#[async_trait]
impl LlmProvider for GroqLlmProvider {
    async fn complete(&self, system_prompt: &str, user_message: &str) -> Result<String, LlmError> {
        if self.api_key.is_empty() {
            return Err(LlmError::NoApiKey("groq".to_string()));
        }

        let request = openai_compat::ChatRequest::new(
            self.model.clone(),
            system_prompt,
            user_message,
            4096,
            0.3,
        );

        if let Some(store) = &self.request_log_store {
            let request_json = serde_json::to_value(&request).unwrap_or_else(|_| {
                json!({
                    "provider": "groq",
                    "error": "failed to serialize request",
                })
            });
            store.with_current(|log| {
                log.llm_request_json = Some(request_json);
            });
        }

        let mut req = self
            .client
            .post(&self.api_base_url)
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
            if let Ok(error_response) =
                serde_json::from_str::<openai_compat::ErrorResponse>(&error_text)
            {
                return Err(LlmError::Api(format!(
                    "Groq API error ({}): {}",
                    status, error_response.error.message
                )));
            }
            return Err(LlmError::Api(format!(
                "Groq API error ({}): {}",
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

        openai_compat::extract_first_choice_text(&response_json)
            .ok_or_else(|| LlmError::InvalidResponse("No response choices returned".to_string()))
    }

    fn name(&self) -> &'static str {
        "groq"
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
        let provider = GroqLlmProvider::new("test-key".to_string());
        assert_eq!(provider.name(), "groq");
    }

    #[test]
    fn test_default_model() {
        let provider = GroqLlmProvider::new("test-key".to_string());
        assert_eq!(provider.model(), DEFAULT_MODEL);
    }

    #[test]
    fn test_custom_model() {
        let provider = GroqLlmProvider::with_model("test-key".to_string(), "llama".to_string());
        assert_eq!(provider.model(), "llama");
    }

    #[test]
    fn test_without_timeout_disables_timeout() {
        let provider = GroqLlmProvider::new("test-key".to_string()).without_timeout();
        assert!(provider.timeout.is_none());
    }
}
