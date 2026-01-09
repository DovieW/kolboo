//! Ollama LLM provider for local text formatting.

use super::{LlmError, LlmProvider};
use async_trait::async_trait;
use crate::request_log::RequestLogStore;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::Duration;

const DEFAULT_OLLAMA_URL: &str = "http://localhost:11434";
const DEFAULT_MODEL: &str = "llama3.2";
/// Longer timeout for local models which may be slower
const DEFAULT_OLLAMA_TIMEOUT: Duration = Duration::from_secs(120);

/// Ollama LLM provider for local inference
pub struct OllamaLlmProvider {
    client: Client,
    base_url: String,
    model: String,
    timeout: Option<Duration>,
    request_log_store: Option<RequestLogStore>,
}

impl OllamaLlmProvider {
    /// Create a new Ollama provider with default settings
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            base_url: DEFAULT_OLLAMA_URL.to_string(),
            model: DEFAULT_MODEL.to_string(),
            timeout: Some(DEFAULT_OLLAMA_TIMEOUT),
            request_log_store: None,
        }
    }

    /// Create with a specific model
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn with_model(model: String) -> Self {
        Self {
            client: Client::new(),
            base_url: DEFAULT_OLLAMA_URL.to_string(),
            model,
            timeout: Some(DEFAULT_OLLAMA_TIMEOUT),
            request_log_store: None,
        }
    }

    /// Create with custom URL and model
    pub fn with_url(base_url: String, model: Option<String>) -> Self {
        Self {
            client: Client::new(),
            base_url,
            model: model.unwrap_or_else(|| DEFAULT_MODEL.to_string()),
            timeout: Some(DEFAULT_OLLAMA_TIMEOUT),
            request_log_store: None,
        }
    }

    /// Create with custom client and settings
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn with_client(client: Client, base_url: Option<String>, model: Option<String>) -> Self {
        Self {
            client,
            base_url: base_url.unwrap_or_else(|| DEFAULT_OLLAMA_URL.to_string()),
            model: model.unwrap_or_else(|| DEFAULT_MODEL.to_string()),
            timeout: Some(DEFAULT_OLLAMA_TIMEOUT),
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

    /// Check if Ollama is available at the configured URL
    #[allow(dead_code)]
    pub async fn is_available(&self) -> bool {
        let url = format!("{}/api/tags", self.base_url);
        self.client
            .get(&url)
            .timeout(Duration::from_secs(5))
            .send()
            .await
            .is_ok()
    }

    /// List available models
    #[allow(dead_code)]
    pub async fn list_models(&self) -> Result<Vec<String>, LlmError> {
        let url = format!("{}/api/tags", self.base_url);
        let response = self
            .client
            .get(&url)
            .timeout(Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| LlmError::ProviderNotAvailable(format!("Ollama not reachable: {}", e)))?;

        if !response.status().is_success() {
            return Err(LlmError::ProviderNotAvailable(
                "Failed to list Ollama models".to_string(),
            ));
        }

        let tags_response: TagsResponse = response.json().await.map_err(|e| {
            LlmError::InvalidResponse(format!("Failed to parse tags response: {}", e))
        })?;

        Ok(tags_response.models.into_iter().map(|m| m.name).collect())
    }
}

impl Default for OllamaLlmProvider {
    fn default() -> Self {
        Self::new()
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
    stream: bool,
    options: Option<ChatOptions>,
}

#[derive(Debug, Serialize)]
struct ChatOptions {
    temperature: f32,
    num_predict: i32,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct TagsResponse {
    models: Vec<ModelInfo>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct ModelInfo {
    name: String,
}

#[derive(Debug, Deserialize)]
struct ErrorResponse {
    error: String,
}

#[async_trait]
impl LlmProvider for OllamaLlmProvider {
    async fn complete(&self, system_prompt: &str, user_message: &str) -> Result<String, LlmError> {
        let url = format!("{}/api/chat", self.base_url);

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
            stream: false,
            options: Some(ChatOptions {
                temperature: 0.3,
                num_predict: 4096,
            }),
        };

        if let Some(store) = &self.request_log_store {
            let request_json = serde_json::to_value(&request).unwrap_or_else(|_| {
                json!({
                    "provider": "ollama",
                    "error": "failed to serialize request",
                })
            });
            store.with_current(|log| {
                log.llm_request_json = Some(request_json);
            });
        }

        let mut req = self.client.post(&url).json(&request);
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
            } else if e.is_connect() {
                LlmError::ProviderNotAvailable(format!(
                    "Ollama not reachable at {}: {}",
                    self.base_url, e
                ))
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
                    "Ollama error ({}): {}",
                    status, error_response.error
                )));
            }
            return Err(LlmError::Api(format!(
                "Ollama error ({}): {}",
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

        response_json
            .get("message")
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| LlmError::InvalidResponse("No message content in response".to_string()))
    }

    fn name(&self) -> &'static str {
        "ollama"
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
        let provider = OllamaLlmProvider::new();
        assert_eq!(provider.name(), "ollama");
    }

    #[test]
    fn test_default_model() {
        let provider = OllamaLlmProvider::new();
        assert_eq!(provider.model(), DEFAULT_MODEL);
    }

    #[test]
    fn test_custom_model() {
        let provider = OllamaLlmProvider::with_model("mistral".to_string());
        assert_eq!(provider.model(), "mistral");
    }

    #[test]
    fn test_without_timeout_disables_timeout() {
        let provider = OllamaLlmProvider::new().without_timeout();
        assert!(provider.timeout.is_none());
    }

    #[test]
    fn test_custom_url() {
        let provider = OllamaLlmProvider::with_url(
            "http://192.168.1.100:11434".to_string(),
            Some("codellama".to_string()),
        );
        assert_eq!(provider.base_url, "http://192.168.1.100:11434");
        assert_eq!(provider.model(), "codellama");
    }
}
