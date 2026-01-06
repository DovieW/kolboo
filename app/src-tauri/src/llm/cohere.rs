//! Cohere LLM provider for text formatting.
//!
//! Uses Cohere Chat API (v2).
//!
//! - Free-form completions: POST https://api.cohere.com/v2/chat
//! - Structured outputs: uses `response_format: { type: "json_object", schema: ... }`

use super::{LlmError, LlmProvider, DEFAULT_LLM_TIMEOUT};
use async_trait::async_trait;
use crate::request_log::RequestLogStore;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};
use std::time::Duration;

const COHERE_CHAT_URL: &str = "https://api.cohere.com/v2/chat";
const DEFAULT_MODEL: &str = "command-r-08-2024";

pub struct CohereLlmProvider {
    client: Client,
    api_key: String,
    model: String,
    timeout: Option<Duration>,
    request_log_store: Option<RequestLogStore>,
}

impl CohereLlmProvider {
    pub fn new(api_key: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
            model: DEFAULT_MODEL.to_string(),
            timeout: Some(DEFAULT_LLM_TIMEOUT),
            request_log_store: None,
        }
    }

    pub fn with_model(api_key: String, model: String) -> Self {
        Self {
            model,
            ..Self::new(api_key)
        }
    }

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

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    pub fn without_timeout(mut self) -> Self {
        self.timeout = None;
        self
    }

    fn extract_output_text(response_json: &JsonValue) -> Result<String, LlmError> {
        // Cohere Chat v2 returns: { message: { content: [ { type: "text", text: "..." }, ... ] }, ... }
        let content = response_json
            .get("message")
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_array())
            .ok_or_else(|| {
                LlmError::InvalidResponse(
                    "Cohere response missing message.content array".to_string(),
                )
            })?;

        let mut out = String::new();
        for part in content {
            let part_type = part.get("type").and_then(|t| t.as_str()).unwrap_or("");
            if part_type == "text" {
                if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                    out.push_str(text);
                }
            }
        }

        let trimmed = out.trim().to_string();
        if trimmed.is_empty() {
            return Err(LlmError::InvalidResponse(
                "Cohere response contained no text content".to_string(),
            ));
        }

        Ok(trimmed)
    }
}

#[derive(Debug, Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Serialize)]
struct ResponseFormat {
    #[serde(rename = "type")]
    type_: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    schema: Option<JsonValue>,
}

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    stream: bool,
    messages: Vec<ChatMessage>,

    #[serde(skip_serializing_if = "Option::is_none")]
    response_format: Option<ResponseFormat>,

    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct CohereErrorResponse {
    message: Option<String>,
    error: Option<String>,
}

#[async_trait]
impl LlmProvider for CohereLlmProvider {
    async fn complete(&self, system_prompt: &str, user_message: &str) -> Result<String, LlmError> {
        if self.api_key.is_empty() {
            return Err(LlmError::NoApiKey("cohere".to_string()));
        }

        let request = ChatRequest {
            model: self.model.clone(),
            stream: false,
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
            response_format: None,
            // Keep rewrite deterministic.
            temperature: Some(0.0),
            max_tokens: Some(1024),
        };

        if let Some(store) = &self.request_log_store {
            let request_json = serde_json::to_value(&request).unwrap_or_else(|_| {
                json!({
                    "provider": "cohere",
                    "error": "failed to serialize request",
                })
            });
            store.with_current(|log| {
                log.llm_request_json = Some(request_json);
            });
        }

        let mut req = self
            .client
            .post(COHERE_CHAT_URL)
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
            let body = response.text().await.unwrap_or_default();
            if let Ok(parsed) = serde_json::from_str::<CohereErrorResponse>(&body) {
                let msg = parsed
                    .message
                    .or(parsed.error)
                    .unwrap_or_else(|| body.clone());
                return Err(LlmError::Api(format!(
                    "Cohere API error ({}): {}",
                    status, msg
                )));
            }
            return Err(LlmError::Api(format!(
                "Cohere API error ({}): {}",
                status, body
            )));
        }

        let response_json: JsonValue = response.json().await.map_err(|e| {
            LlmError::InvalidResponse(format!("Failed to parse response: {}", e))
        })?;

        if let Some(store) = &self.request_log_store {
            let response_for_log = response_json.clone();
            store.with_current(|log| {
                log.llm_response_json = Some(response_for_log);
            });
        }

        let output_text = Self::extract_output_text(&response_json)?;
        Ok(output_text)
    }

    async fn complete_json_schema(
        &self,
        system_prompt: &str,
        user_message: &str,
        _schema_name: &str,
        _schema_description: &str,
        schema: JsonValue,
    ) -> Result<JsonValue, LlmError> {
        if self.api_key.is_empty() {
            return Err(LlmError::NoApiKey("cohere".to_string()));
        }

        // Cohere requires the prompt to explicitly instruct JSON output when using json_object.
        let system_prompt = format!(
            "{}\n\nReturn ONLY valid JSON that matches the provided JSON Schema (no markdown, no extra keys).",
            system_prompt
        );

        let request = ChatRequest {
            model: self.model.clone(),
            stream: false,
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: system_prompt,
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: format!(
                        "Generate a JSON object matching the provided schema.\n\n{}",
                        user_message
                    ),
                },
            ],
            response_format: Some(ResponseFormat {
                type_: "json_object".to_string(),
                schema: Some(schema),
            }),
            temperature: Some(0.0),
            max_tokens: Some(1024),
        };

        if let Some(store) = &self.request_log_store {
            let request_json = serde_json::to_value(&request).unwrap_or_else(|_| {
                json!({
                    "provider": "cohere",
                    "error": "failed to serialize request",
                })
            });
            store.with_current(|log| {
                log.llm_request_json = Some(request_json);
            });
        }

        let mut req = self
            .client
            .post(COHERE_CHAT_URL)
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
            let body = response.text().await.unwrap_or_default();
            if let Ok(parsed) = serde_json::from_str::<CohereErrorResponse>(&body) {
                let msg = parsed
                    .message
                    .or(parsed.error)
                    .unwrap_or_else(|| body.clone());
                return Err(LlmError::Api(format!(
                    "Cohere API error ({}): {}",
                    status, msg
                )));
            }
            return Err(LlmError::Api(format!(
                "Cohere API error ({}): {}",
                status, body
            )));
        }

        let response_json: JsonValue = response.json().await.map_err(|e| {
            LlmError::InvalidResponse(format!("Failed to parse response: {}", e))
        })?;

        if let Some(store) = &self.request_log_store {
            let response_for_log = response_json.clone();
            store.with_current(|log| {
                log.llm_response_json = Some(response_for_log);
            });
        }

        let output_text = Self::extract_output_text(&response_json)?;
        serde_json::from_str::<JsonValue>(output_text.trim()).map_err(|e| {
            LlmError::InvalidResponse(format!(
                "Structured output was not valid JSON: {} (content: {})",
                e, output_text
            ))
        })
    }

    fn name(&self) -> &'static str {
        "cohere"
    }

    fn model(&self) -> &str {
        &self.model
    }
}
