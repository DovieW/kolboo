//! Google Gemini (AI Studio / Gemini Developer API) LLM provider for text formatting.

use super::{LlmError, LlmProvider, DEFAULT_LLM_TIMEOUT};
use async_trait::async_trait;
use crate::request_log::RequestLogStore;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::Duration;

const GEMINI_API_ROOT: &str = "https://generativelanguage.googleapis.com/v1beta";
const DEFAULT_MODEL: &str = "gemini-2.5-flash";

/// Gemini LLM provider using the `models.generateContent` REST API.
pub struct GeminiLlmProvider {
    client: Client,
    api_key: String,
    model: String,
    timeout: Option<Duration>,
    thinking_budget: Option<i64>,
    thinking_level: Option<String>,
    structured_outputs: bool,
    request_log_store: Option<RequestLogStore>,
}

impl GeminiLlmProvider {
    pub fn new(api_key: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
            model: DEFAULT_MODEL.to_string(),
            timeout: Some(DEFAULT_LLM_TIMEOUT),
            thinking_budget: None,
            thinking_level: None,
            structured_outputs: true,
            request_log_store: None,
        }
    }

    pub fn with_model(api_key: String, model: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
            model,
            timeout: Some(DEFAULT_LLM_TIMEOUT),
            thinking_budget: None,
            thinking_level: None,
            structured_outputs: true,
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
            thinking_budget: None,
            thinking_level: None,
            structured_outputs: true,
            request_log_store: None,
        }
    }

    /// Enable/disable Structured Outputs (JSON schema mode).
    ///
    /// This provider defaults to **enabled** because it improves determinism for transcript
    /// rewrite formatting. For ad-hoc completions (like transcript analysis), callers should
    /// disable it so the model can return free-form text.
    pub fn with_structured_outputs(mut self, enabled: bool) -> Self {
        self.structured_outputs = enabled;
        self
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

    pub fn with_thinking_budget(mut self, budget: Option<i64>) -> Self {
        self.thinking_budget = budget;
        self
    }

    pub fn with_thinking_level(mut self, level: Option<String>) -> Self {
        self.thinking_level = level;
        self
    }

    fn normalize_model_name(model: &str) -> String {
        let trimmed = model.trim();
        if trimmed.starts_with("models/") {
            trimmed.to_string()
        } else {
            format!("models/{}", trimmed)
        }
    }

    fn rewrite_response_schema() -> serde_json::Value {
        // Keep the schema intentionally tiny: the app only needs the final rewritten text.
        json!({
            "type": "object",
            "properties": {
                "rewritten_text": {
                    "type": "string",
                    "description": "The final rewritten transcript text. Use this string directly as output. Preserve meaning and formatting. Do not wrap in markdown or add commentary."
                }
            },
            "required": ["rewritten_text"],
            "additionalProperties": false
        })
    }

    fn effective_thinking_config(&self) -> Option<ThinkingConfig> {
        // We only send thinkingConfig when the user chose a value.
        // If the model does not support thinking, the API will return an error.

        // Gemini 3+ models use `thinkingLevel` per API reference.
        if self.model.contains("gemini-3") {
            if let Some(level_raw) = self.thinking_level.as_deref() {
                let level = level_raw.trim().to_ascii_lowercase();
                if level.is_empty() {
                    return None;
                }

                // Docs (2025-12):
                // - Gemini 3 Pro supports: low, high
                // - Gemini 3 Flash supports: minimal, low, medium, high
                let is_pro = self.model.contains("gemini-3-pro");
                let is_flash = self.model.contains("gemini-3-flash");

                let allowed = if is_pro {
                    matches!(level.as_str(), "low" | "high")
                } else if is_flash {
                    matches!(level.as_str(), "minimal" | "low" | "medium" | "high")
                } else {
                    // Unknown Gemini 3 variant: be conservative.
                    matches!(level.as_str(), "low" | "high")
                };

                if !allowed {
                    log::warn!(
                        "Unsupported gemini-3 thinking level '{}' for model '{}'; ignoring",
                        level,
                        self.model
                    );
                    return None;
                }

                return Some(ThinkingConfig {
                    include_thoughts: None,
                    thinking_budget: None,
                    thinking_level: Some(level),
                });
            }

            return None;
        }

        // Gemini 2.5 and earlier models use integer budget.
        if self.model.contains("gemini-2.5") {
            if let Some(budget) = self.thinking_budget {
                // Docs: flash-lite does not think; don't send a budget knob.
                if self.model.contains("gemini-2.5-flash-lite") {
                    log::warn!(
                        "gemini-2.5-flash-lite does not support thinkingBudget; ignoring"
                    );
                    return None;
                }

                // Docs (2025-12): thinkingBudget valid values:
                // -1 = dynamic thinking
                //  0 = disable thinking (not supported by gemini-2.5-pro)
                //  N = specific token budget (range depends on model)
                let is_pro = self.model.contains("gemini-2.5-pro");
                let is_flash = self.model.contains("gemini-2.5-flash");

                let ok = if budget == -1 {
                    true
                } else if budget < -1 {
                    false
                } else if is_pro {
                    // Pro: cannot disable (0). Range: 128..=32768
                    if budget == 0 {
                        false
                    } else {
                        (128..=32768).contains(&budget)
                    }
                } else if is_flash {
                    // Flash: can disable (0). Range: 0..=24576
                    (0..=24576).contains(&budget)
                } else {
                    // Unknown 2.5 variant: be conservative.
                    (0..=32768).contains(&budget)
                };

                if !ok {
                    log::warn!(
                        "Unsupported gemini-2.5 thinkingBudget={} for model '{}'; ignoring",
                        budget,
                        self.model
                    );
                    return None;
                }

                return Some(ThinkingConfig {
                    include_thoughts: None,
                    thinking_budget: Some(budget),
                    thinking_level: None,
                });
            }
        }

        None
    }

    fn extract_text(response: &GenerateContentResponse) -> Result<String, LlmError> {
        let candidate = response
            .candidates
            .as_ref()
            .and_then(|c| c.first())
            .ok_or_else(|| {
                LlmError::InvalidResponse("Gemini API returned no candidates".to_string())
            })?;

        let parts = candidate
            .content
            .as_ref()
            .map(|c| c.parts.as_slice())
            .ok_or_else(|| {
                LlmError::InvalidResponse(
                    "Gemini API returned a candidate without content.parts".to_string(),
                )
            })?;

        let mut combined = String::new();
        for p in parts {
            if let Some(text) = p.text.as_ref() {
                combined.push_str(text);
            }
        }

        if combined.trim().is_empty() {
            return Err(LlmError::InvalidResponse(
                "Gemini API returned empty candidate text".to_string(),
            ));
        }

        Ok(combined)
    }
}

#[derive(Debug, Serialize)]
struct GenerateContentRequest {
    #[serde(skip_serializing_if = "Option::is_none", rename = "systemInstruction")]
    system_instruction: Option<Content>,
    contents: Vec<Content>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "generationConfig")]
    generation_config: Option<GenerationConfig>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Content {
    #[serde(skip_serializing_if = "Option::is_none")]
    role: Option<String>,
    parts: Vec<Part>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Part {
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,
}

#[derive(Debug, Serialize)]
struct GenerationConfig {
    #[serde(rename = "maxOutputTokens")]
    max_output_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,

    // Structured outputs
    #[serde(rename = "responseMimeType")]
    response_mime_type: String,
    #[serde(skip_serializing_if = "Option::is_none", rename = "responseJsonSchema")]
    response_json_schema: Option<serde_json::Value>,

    #[serde(skip_serializing_if = "Option::is_none", rename = "thinkingConfig")]
    thinking_config: Option<ThinkingConfig>,
}

#[derive(Debug, Serialize)]
struct ThinkingConfig {
    #[serde(skip_serializing_if = "Option::is_none", rename = "includeThoughts")]
    include_thoughts: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "thinkingBudget")]
    thinking_budget: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "thinkingLevel")]
    thinking_level: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GenerateContentResponse {
    #[serde(default)]
    candidates: Option<Vec<Candidate>>,
}

#[derive(Debug, Deserialize)]
struct Candidate {
    #[serde(default)]
    content: Option<Content>,
}

#[derive(Debug, Deserialize)]
struct GeminiErrorResponse {
    error: GeminiErrorDetail,
}

#[derive(Debug, Deserialize)]
struct GeminiErrorDetail {
    message: String,
}

#[async_trait]
impl LlmProvider for GeminiLlmProvider {
    async fn complete(&self, system_prompt: &str, user_message: &str) -> Result<String, LlmError> {
        if self.api_key.trim().is_empty() {
            return Err(LlmError::NoApiKey("gemini".to_string()));
        }

        let model = Self::normalize_model_name(&self.model);
        let url = format!("{}/{model}:generateContent", GEMINI_API_ROOT);

        // For deterministic formatting/rewrite.
        // Gemini docs note that for Gemini 3 models it's recommended to keep temperature at the
        // default value to avoid unexpected behavior. For older models we keep temperature low.
        let temperature = if self.model.contains("gemini-3") {
            None
        } else {
            Some(0.0)
        };

        // Preserve historical behavior: `complete()` returns rewritten text for rewrite.
        if self.structured_outputs {
            let v = self
                .complete_json_schema(
                    system_prompt,
                    user_message,
                    "rewrite_response",
                    "Structured output for a dictation transcript rewrite. The model must emit valid JSON matching the schema.",
                    Self::rewrite_response_schema(),
                )
                .await?;

            let rewritten = v
                .get("rewritten_text")
                .and_then(|t| t.as_str())
                .ok_or_else(|| {
                    LlmError::InvalidResponse(format!(
                        "Gemini structured output missing required field 'rewritten_text' (content: {})",
                        v
                    ))
                })?;

            return Ok(rewritten.to_string());
        }

        let generation_config = GenerationConfig {
            max_output_tokens: 4096,
            temperature,
            response_mime_type: "text/plain".to_string(),
            response_json_schema: None,
            thinking_config: self.effective_thinking_config(),
        };

        let request = GenerateContentRequest {
            system_instruction: Some(Content {
                role: None,
                parts: vec![Part {
                    text: Some(if self.structured_outputs {
                        format!(
                            "{}\n\nReturn ONLY valid JSON that matches the provided JSON Schema (no markdown, no extra keys).",
                            system_prompt
                        )
                    } else {
                        system_prompt.to_string()
                    }),
                }],
            }),
            contents: vec![Content {
                role: Some("user".to_string()),
                parts: vec![Part {
                    text: Some(user_message.to_string()),
                }],
            }],
            generation_config: Some(generation_config),
        };

        if let Some(store) = &self.request_log_store {
            let request_json = serde_json::to_value(&request).unwrap_or_else(|_| {
                json!({
                    "provider": "gemini",
                    "error": "failed to serialize request",
                })
            });
            store.with_current(|log| {
                log.llm_request_json = Some(request_json);
            });
        }

        let mut req = self
            .client
            .post(url)
            .header("x-goog-api-key", self.api_key.trim())
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
            if let Ok(error_response) = serde_json::from_str::<GeminiErrorResponse>(&error_text) {
                return Err(LlmError::Api(format!(
                    "Gemini API error ({}): {}",
                    status, error_response.error.message
                )));
            }
            return Err(LlmError::Api(format!(
                "Gemini API error ({}): {}",
                status, error_text
            )));
        }

        let response_value: serde_json::Value = response.json().await.map_err(|e| {
            LlmError::InvalidResponse(format!("Failed to parse Gemini response: {}", e))
        })?;

        if let Some(store) = &self.request_log_store {
            let response_for_log = response_value.clone();
            store.with_current(|log| {
                log.llm_response_json = Some(response_for_log);
            });
        }

        let response_json: GenerateContentResponse = serde_json::from_value(response_value)
            .map_err(|e| LlmError::InvalidResponse(format!("Failed to parse Gemini response: {}", e)))?;

        let output_text = Self::extract_text(&response_json)?;

        Ok(output_text)
    }

    async fn complete_json_schema(
        &self,
        system_prompt: &str,
        user_message: &str,
        _schema_name: &str,
        _schema_description: &str,
        schema: serde_json::Value,
    ) -> Result<serde_json::Value, LlmError> {
        if self.api_key.trim().is_empty() {
            return Err(LlmError::NoApiKey("gemini".to_string()));
        }

        let model = Self::normalize_model_name(&self.model);
        let url = format!("{}/{model}:generateContent", GEMINI_API_ROOT);

        let temperature = if self.model.contains("gemini-3") {
            None
        } else {
            Some(0.0)
        };

        let generation_config = GenerationConfig {
            max_output_tokens: 1024,
            temperature,
            response_mime_type: "application/json".to_string(),
            response_json_schema: Some(schema),
            thinking_config: self.effective_thinking_config(),
        };

        let request = GenerateContentRequest {
            system_instruction: Some(Content {
                role: None,
                parts: vec![Part {
                    text: Some(format!(
                        "{}\n\nReturn ONLY valid JSON that matches the provided JSON Schema (no markdown, no extra keys).",
                        system_prompt
                    )),
                }],
            }),
            contents: vec![Content {
                role: Some("user".to_string()),
                parts: vec![Part {
                    text: Some(user_message.to_string()),
                }],
            }],
            generation_config: Some(generation_config),
        };

        if let Some(store) = &self.request_log_store {
            let request_json = serde_json::to_value(&request).unwrap_or_else(|_| {
                json!({
                    "provider": "gemini",
                    "error": "failed to serialize request",
                })
            });
            store.with_current(|log| {
                log.llm_request_json = Some(request_json);
            });
        }

        let mut req = self
            .client
            .post(url)
            .header("x-goog-api-key", self.api_key.trim())
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
            if let Ok(error_response) = serde_json::from_str::<GeminiErrorResponse>(&error_text) {
                return Err(LlmError::Api(format!(
                    "Gemini API error ({}): {}",
                    status, error_response.error.message
                )));
            }
            return Err(LlmError::Api(format!(
                "Gemini API error ({}): {}",
                status, error_text
            )));
        }

        let response_value: serde_json::Value = response.json().await.map_err(|e| {
            LlmError::InvalidResponse(format!("Failed to parse Gemini response: {}", e))
        })?;

        if let Some(store) = &self.request_log_store {
            let response_for_log = response_value.clone();
            store.with_current(|log| {
                log.llm_response_json = Some(response_for_log);
            });
        }

        let response_json: GenerateContentResponse = serde_json::from_value(response_value)
            .map_err(|e| LlmError::InvalidResponse(format!("Failed to parse Gemini response: {}", e)))?;

        let output_text = Self::extract_text(&response_json)?;
        serde_json::from_str::<serde_json::Value>(output_text.trim()).map_err(|e| {
            LlmError::InvalidResponse(format!(
                "Gemini structured output was not valid JSON: {} (content: {})",
                e, output_text
            ))
        })
    }

    fn name(&self) -> &'static str {
        "gemini"
    }

    fn model(&self) -> &str {
        &self.model
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_model_name() {
        assert_eq!(
            GeminiLlmProvider::normalize_model_name("gemini-2.5-flash"),
            "models/gemini-2.5-flash"
        );
        assert_eq!(
            GeminiLlmProvider::normalize_model_name("models/gemini-2.5-flash"),
            "models/gemini-2.5-flash"
        );
    }
}
