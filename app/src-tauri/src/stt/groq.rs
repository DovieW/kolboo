//! Groq Whisper API STT provider implementation.

use super::openai_compat;
use super::{AudioFormat, SttError, SttProvider};
use crate::request_log::RequestLogStore;
use async_trait::async_trait;
use serde_json::json;
use std::time::Duration;

/// Groq Whisper API provider for speech-to-text
pub struct GroqSttProvider {
    client: reqwest::Client,
    api_key: String,
    model: String,
    default_prompt: Option<String>,
    api_base_url: String,
    request_log_store: Option<RequestLogStore>,
}

impl GroqSttProvider {
    const PROMPT_MAX_CHARS: usize = 224;
    const DEFAULT_API_BASE_URL: &'static str = "https://api.groq.com";

    /// Create a new Groq STT provider
    ///
    /// # Arguments
    /// * `api_key` - Groq API key
    /// * `model` - Model to use (e.g., "whisper-large-v3-turbo")
    /// * `default_prompt` - Optional transcription prompt (OpenAI-compatible `prompt` field)
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn new(api_key: String, model: Option<String>, default_prompt: Option<String>) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            api_key,
            model: model.unwrap_or_else(|| "whisper-large-v3-turbo".to_string()),
            default_prompt,
            api_base_url: Self::DEFAULT_API_BASE_URL.to_string(),
            request_log_store: None,
        }
    }

    /// Create a new provider with a custom HTTP client
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn with_client(
        client: reqwest::Client,
        api_key: String,
        model: Option<String>,
        default_prompt: Option<String>,
    ) -> Self {
        Self {
            client,
            api_key,
            model: model.unwrap_or_else(|| "whisper-large-v3-turbo".to_string()),
            default_prompt,
            api_base_url: Self::DEFAULT_API_BASE_URL.to_string(),
            request_log_store: None,
        }
    }

    /// Override the API base URL (defaults to https://api.groq.com).
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

    fn api_base_url_trimmed(&self) -> &str {
        self.api_base_url.trim_end_matches('/')
    }

    fn transcriptions_url(&self) -> String {
        format!(
            "{}/openai/v1/audio/transcriptions",
            self.api_base_url_trimmed()
        )
    }

    fn clamp_prompt(prompt: &str) -> Option<String> {
        let trimmed = prompt.trim();
        if trimmed.is_empty() {
            return None;
        }

        let clamped: String = trimmed.chars().take(Self::PROMPT_MAX_CHARS).collect();
        Some(clamped)
    }
}

#[async_trait]
impl SttProvider for GroqSttProvider {
    async fn transcribe(&self, audio: &[u8], _format: &AudioFormat) -> Result<String, SttError> {
        let endpoint = self.transcriptions_url();

        if let Some(store) = &self.request_log_store {
            let prompt = self.default_prompt.as_deref().and_then(Self::clamp_prompt);
            let request_json = json!({
                "provider": "groq",
                "endpoint": endpoint.clone(),
                "content_type": "multipart/form-data",
                "fields": {
                    "model": self.model,
                    "prompt": prompt,
                },
                "file": {
                    "name": "audio.wav",
                    "mime": "audio/wav",
                    "bytes": audio.len(),
                    "data": "<binary audio omitted>",
                }
            });

            store.with_current(|log| {
                log.stt_request_json = Some(request_json);
            });
        }

        let prompt = self.default_prompt.as_deref().and_then(Self::clamp_prompt);
        let form = openai_compat::wav_transcription_form(audio, &self.model, prompt.as_deref())?;

        let response = self
            .client
            .post(endpoint)
            .bearer_auth(&self.api_key)
            .multipart(form)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    SttError::Timeout
                } else {
                    SttError::Network(e)
                }
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(SttError::Api(format!(
                "Groq API error ({}): {}",
                status, error_text
            )));
        }

        let result: serde_json::Value = response.json().await?;

        if let Some(store) = &self.request_log_store {
            let result_for_log = result.clone();
            store.with_current(|log| {
                log.stt_response_json = Some(result_for_log);
            });
        }
        let text = result["text"].as_str().unwrap_or("").to_string();

        Ok(text)
    }

    fn name(&self) -> &'static str {
        "groq"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_creation() {
        let provider = GroqSttProvider::new("test-key".to_string(), None, None);
        assert_eq!(provider.name(), "groq");
        assert_eq!(provider.model, "whisper-large-v3-turbo");
    }

    #[test]
    fn test_provider_with_custom_model() {
        let provider = GroqSttProvider::new(
            "test-key".to_string(),
            Some("whisper-large-v3-turbo".to_string()),
            None,
        );
        assert_eq!(provider.model, "whisper-large-v3-turbo");
    }

    #[test]
    fn test_prompt_clamping() {
        let long = "x".repeat(GroqSttProvider::PROMPT_MAX_CHARS + 10);
        let clamped = GroqSttProvider::clamp_prompt(&long).unwrap();
        assert_eq!(clamped.len(), GroqSttProvider::PROMPT_MAX_CHARS);
    }
}
