//! OpenAI LLM provider for text formatting.

use super::{LlmError, LlmProvider, DEFAULT_LLM_TIMEOUT};
use crate::request_log::RequestLogStore;
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::Duration;

const DEFAULT_OPENAI_API_BASE_URL: &str = "https://api.openai.com";
const DEFAULT_MODEL: &str = "gpt-4o-mini";

/// OpenAI LLM provider using the Chat Completions API
pub struct OpenAiLlmProvider {
    client: Client,
    api_key: String,
    model: String,
    api_base_url: String,
    timeout: Option<Duration>,
    reasoning_effort: Option<String>,
    structured_outputs: bool,
    request_log_store: Option<RequestLogStore>,
}

impl OpenAiLlmProvider {
    /// Create a new OpenAI provider with the given API key
    pub fn new(api_key: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
            model: DEFAULT_MODEL.to_string(),
            api_base_url: DEFAULT_OPENAI_API_BASE_URL.to_string(),
            timeout: Some(DEFAULT_LLM_TIMEOUT),
            reasoning_effort: None,
            structured_outputs: true,
            request_log_store: None,
        }
    }

    /// Create with a specific model
    pub fn with_model(api_key: String, model: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
            model,
            api_base_url: DEFAULT_OPENAI_API_BASE_URL.to_string(),
            timeout: Some(DEFAULT_LLM_TIMEOUT),
            reasoning_effort: None,
            structured_outputs: true,
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
            api_base_url: DEFAULT_OPENAI_API_BASE_URL.to_string(),
            timeout: Some(DEFAULT_LLM_TIMEOUT),
            reasoning_effort: None,
            structured_outputs: true,
            request_log_store: None,
        }
    }

    /// Override the API base URL (defaults to https://api.openai.com).
    ///
    /// This is primarily intended for deterministic contract tests (e.g., Wiremock).
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn with_api_base_url(mut self, base_url: String) -> Self {
        self.api_base_url = base_url;
        self
    }

    fn responses_url(&self) -> String {
        crate::http::join_base_url(&self.api_base_url, "/v1/responses")
    }

    /// Enable/disable Structured Outputs (JSON schema mode).
    ///
    /// This provider defaults to **enabled** because it dramatically improves determinism
    /// for rewrite formatting. For ad-hoc completions (like transcript analysis), callers
    /// should disable it so the model can return free-form text.
    pub fn with_structured_outputs(mut self, enabled: bool) -> Self {
        self.structured_outputs = enabled;
        self
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

    /// Configure OpenAI reasoning effort (reasoning models only).
    ///
    /// Docs (2025-12): supported values vary by model family and include
    /// "none", "minimal", "low", "medium", "high", "xhigh".
    /// We validate per-model and ignore unsupported values to avoid 400s.
    pub fn with_reasoning_effort(mut self, effort: Option<String>) -> Self {
        self.reasoning_effort = effort;
        self
    }

    fn supports_structured_outputs(model: &str) -> bool {
        // Structured Outputs (schema adherence) is available in newer models.
        // We keep a conservative allowlist to avoid 400s on unsupported models.
        //
        // Docs: https://platform.openai.com/docs/guides/structured-outputs
        model.starts_with("gpt-4.1") || model.starts_with("gpt-4o") || model.starts_with("gpt-5")
    }

    fn supports_reasoning_effort(model: &str) -> bool {
        // OpenAI docs: "gpt-5 and o-series models only".
        // We treat any model starting with "o" as o-series.
        model.starts_with("gpt-5") || model.starts_with('o')
    }

    fn allowed_reasoning_efforts(model: &str) -> &'static [&'static str] {
        // From OpenAI API docs:
        // - gpt-5.1 supports: none, low, medium, high
        // - models before gpt-5.1 do not support `none`
        // We keep this conservative so users don't hit 400s.
        if model.starts_with("gpt-5.1") || model.starts_with("gpt-5.2") {
            &["none", "low", "medium", "high"]
        } else if model.starts_with("gpt-5") {
            &["low", "medium", "high"]
        } else if model.starts_with('o') {
            // o-series details vary; keep conservative.
            &["low", "medium", "high"]
        } else {
            &[]
        }
    }

    fn validated_reasoning_effort(&self) -> Option<String> {
        let raw = self.reasoning_effort.as_deref()?.trim();
        if raw.is_empty() {
            return None;
        }

        let lower = raw.to_ascii_lowercase();
        if !Self::supports_reasoning_effort(&self.model) {
            return None;
        }

        let allowed = Self::allowed_reasoning_efforts(&self.model);
        if allowed.contains(&lower.as_str()) {
            return Some(lower);
        }

        log::warn!(
            "OpenAI reasoning effort '{}' not supported for model '{}'; ignoring",
            lower,
            self.model
        );
        None
    }

    fn supports_temperature_param(model: &str, reasoning_effort: Option<&str>) -> bool {
        // Docs (GPT-5.2 parameter compatibility):
        // - temperature/top_p/logprobs only supported when reasoning effort is `none`
        //   for gpt-5.2 and gpt-5.1
        // - older GPT-5 models error if these fields are included at all
        if model.starts_with("gpt-5.1") || model.starts_with("gpt-5.2") {
            let effective = reasoning_effort.unwrap_or("none");
            return effective == "none";
        }

        if model.starts_with("gpt-5") {
            return false;
        }

        if model.starts_with('o') {
            // Conservative default for o-series.
            return false;
        }

        true
    }

    fn rewrite_response_format() -> TextFormat {
        // Keep the schema intentionally tiny: the app only needs the final rewritten text.
        // Rich field descriptions make the model's job (and prompt-writing) easier.
        // NOTE: For the Responses API, structured output is configured via `text.format`,
        // and `name/strict/schema` are top-level fields under `format`.
        // Docs: https://platform.openai.com/docs/guides/structured-outputs_api-mode=responses
        TextFormat {
            format_type: "json_schema".to_string(),
            name: Some("rewrite_response".to_string()),
            strict: Some(true),
            description: Some(
                "Structured output for a dictation transcript rewrite. The model must emit valid JSON matching the schema."
                    .to_string(),
            ),
            schema: Some(json!({
                "type": "object",
                "properties": {
                    "rewritten_text": {
                        "type": "string",
                        "description": "The final rewritten transcript text. This string will be used directly as the output. Preserve meaning, intent, and any required formatting. Do not wrap in markdown or add extra commentary. Return an empty string only if the input transcript is empty."
                    }
                },
                "required": ["rewritten_text"],
                "additionalProperties": false
            })),
        }
    }

    fn json_schema_response_format(
        name: &str,
        description: &str,
        schema: serde_json::Value,
    ) -> TextFormat {
        TextFormat {
            format_type: "json_schema".to_string(),
            name: Some(name.to_string()),
            strict: Some(true),
            description: Some(description.to_string()),
            schema: Some(schema),
        }
    }

    fn extract_responses_output_text(value: &serde_json::Value) -> Result<String, LlmError> {
        // Prefer the SDK-style convenience field when present.
        if let Some(s) = value.get("output_text").and_then(|v| v.as_str()) {
            return Ok(s.to_string());
        }

        let output = value
            .get("output")
            .and_then(|v| v.as_array())
            .ok_or_else(|| {
                LlmError::InvalidResponse("Responses API returned no 'output' array".to_string())
            })?;

        for item in output {
            if item.get("type").and_then(|t| t.as_str()) != Some("message") {
                continue;
            }

            let content = match item.get("content").and_then(|c| c.as_array()) {
                Some(c) => c,
                None => continue,
            };

            for part in content {
                match part.get("type").and_then(|t| t.as_str()) {
                    Some("refusal") => {
                        let refusal = part.get("refusal").and_then(|r| r.as_str()).unwrap_or("");
                        return Err(LlmError::Api(format!("OpenAI refusal: {}", refusal)));
                    }
                    Some("output_text") => {
                        if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                            return Ok(text.to_string());
                        }
                    }
                    _ => {}
                }
            }
        }

        Err(LlmError::InvalidResponse(
            "Responses API returned no output_text content".to_string(),
        ))
    }
}

#[derive(Debug, Serialize)]
struct ResponsesRequest {
    model: String,
    input: Vec<ResponseInputMessage>,
    max_output_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning: Option<ReasoningConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<TextConfig>,
}

#[derive(Debug, Serialize)]
struct ReasoningConfig {
    effort: String,
}

#[derive(Debug, Serialize)]
struct ResponseInputMessage {
    role: String,
    content: String,
}

#[derive(Debug, Serialize)]
struct TextConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    format: Option<TextFormat>,
}

#[derive(Debug, Serialize)]
struct TextFormat {
    #[serde(rename = "type")]
    format_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    strict: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    schema: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
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
impl LlmProvider for OpenAiLlmProvider {
    async fn complete(&self, system_prompt: &str, user_message: &str) -> Result<String, LlmError> {
        if self.api_key.is_empty() {
            return Err(LlmError::NoApiKey("openai".to_string()));
        }

        let use_structured_outputs =
            self.structured_outputs && Self::supports_structured_outputs(&self.model);

        if use_structured_outputs {
            // Preserve historical behavior: `complete()` returns rewritten text for rewrite.
            let v = self
                .complete_json_schema(
                    system_prompt,
                    user_message,
                    "rewrite_response",
                    "Structured output for a dictation transcript rewrite. The model must emit valid JSON matching the schema.",
                    Self::rewrite_response_format()
                        .schema
                        .clone()
                        .unwrap_or_else(|| json!({})),
                )
                .await?;

            let rewritten = v
                .get("rewritten_text")
                .and_then(|t| t.as_str())
                .ok_or_else(|| {
                    LlmError::InvalidResponse(format!(
                        "Structured output missing required field 'rewritten_text' (content: {})",
                        v
                    ))
                })?;

            return Ok(rewritten.to_string());
        }

        let reasoning_effort = if Self::supports_reasoning_effort(&self.model) {
            self.validated_reasoning_effort()
        } else {
            None
        };

        let request = ResponsesRequest {
            model: self.model.clone(),
            input: vec![
                ResponseInputMessage {
                    role: "system".to_string(),
                    content: system_prompt.to_string(),
                },
                ResponseInputMessage {
                    role: "user".to_string(),
                    content: user_message.to_string(),
                },
            ],
            max_output_tokens: 4096,
            reasoning: reasoning_effort
                .clone()
                .map(|effort| ReasoningConfig { effort }),
            temperature: Self::supports_temperature_param(&self.model, reasoning_effort.as_deref())
                .then_some(0.0),
            text: None,
        };

        if let Some(store) = &self.request_log_store {
            let request_json = serde_json::to_value(&request).unwrap_or_else(|_| {
                json!({
                    "provider": "openai",
                    "error": "failed to serialize request",
                })
            });
            store.with_current(|log| {
                log.llm_request_json = Some(request_json);
            });
        }

        let mut req = self
            .client
            .post(self.responses_url())
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
            // Try to parse as error response
            if let Ok(error_response) = serde_json::from_str::<ErrorResponse>(&error_text) {
                return Err(LlmError::Api(format!(
                    "OpenAI API error ({}): {}",
                    status, error_response.error.message
                )));
            }
            return Err(LlmError::Api(format!(
                "OpenAI API error ({}): {}",
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

        let output_text = Self::extract_responses_output_text(&response_json)?;
        Ok(output_text)
    }

    async fn complete_json_schema(
        &self,
        system_prompt: &str,
        user_message: &str,
        schema_name: &str,
        schema_description: &str,
        schema: serde_json::Value,
    ) -> Result<serde_json::Value, LlmError> {
        if self.api_key.is_empty() {
            return Err(LlmError::NoApiKey("openai".to_string()));
        }

        // Use server-enforced Structured Outputs when supported; otherwise fall back to
        // instruction-based JSON parsing.
        let supports = Self::supports_structured_outputs(&self.model);
        if !supports {
            let out = self.complete(system_prompt, user_message).await?;
            return serde_json::from_str::<serde_json::Value>(out.trim()).map_err(|e| {
                LlmError::InvalidResponse(format!(
                    "Structured output was not valid JSON: {} (content: {})",
                    e, out
                ))
            });
        }

        let system_prompt = format!(
            "{}\n\nReturn ONLY valid JSON that matches the provided JSON Schema (no markdown, no extra keys).",
            system_prompt
        );

        let reasoning_effort = if Self::supports_reasoning_effort(&self.model) {
            self.validated_reasoning_effort()
        } else {
            None
        };

        let request = ResponsesRequest {
            model: self.model.clone(),
            input: vec![
                ResponseInputMessage {
                    role: "system".to_string(),
                    content: system_prompt,
                },
                ResponseInputMessage {
                    role: "user".to_string(),
                    content: user_message.to_string(),
                },
            ],
            max_output_tokens: 1024,
            reasoning: reasoning_effort
                .clone()
                .map(|effort| ReasoningConfig { effort }),
            temperature: Self::supports_temperature_param(&self.model, reasoning_effort.as_deref())
                .then_some(0.0),
            text: Some(TextConfig {
                format: Some(Self::json_schema_response_format(
                    schema_name,
                    schema_description,
                    schema,
                )),
            }),
        };

        if let Some(store) = &self.request_log_store {
            let request_json = serde_json::to_value(&request).unwrap_or_else(|_| {
                json!({
                    "provider": "openai",
                    "error": "failed to serialize request",
                })
            });
            store.with_current(|log| {
                log.llm_request_json = Some(request_json);
            });
        }

        let mut req = self
            .client
            .post(self.responses_url())
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
                    "OpenAI API error ({}): {}",
                    status, error_response.error.message
                )));
            }
            return Err(LlmError::Api(format!(
                "OpenAI API error ({}): {}",
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

        let output_text = Self::extract_responses_output_text(&response_json)?;
        serde_json::from_str::<serde_json::Value>(output_text.trim()).map_err(|e| {
            LlmError::InvalidResponse(format!(
                "Structured output was not valid JSON: {} (content: {})",
                e, output_text
            ))
        })
    }

    fn name(&self) -> &'static str {
        "openai"
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
        let provider = OpenAiLlmProvider::new("test-key".to_string());
        assert_eq!(provider.name(), "openai");
    }

    #[test]
    fn test_default_model() {
        let provider = OpenAiLlmProvider::new("test-key".to_string());
        assert_eq!(provider.model(), DEFAULT_MODEL);
    }

    #[test]
    fn test_custom_model() {
        let provider = OpenAiLlmProvider::with_model("test-key".to_string(), "gpt-4".to_string());
        assert_eq!(provider.model(), "gpt-4");
    }

    #[test]
    fn test_without_timeout_disables_timeout() {
        let provider = OpenAiLlmProvider::new("test-key".to_string()).without_timeout();
        assert!(provider.timeout.is_none());
    }
}
