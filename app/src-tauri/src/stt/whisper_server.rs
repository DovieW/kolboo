//! Whisper server STT provider implementation.
//!
//! This provider targets any OpenAI Whisper-compatible transcription API.
//!
//! Expected endpoint:
//! - POST {base_url}/audio/transcriptions
//!
//! Where `base_url` is typically something like:
//! - http://localhost:8000/v1
//! - https://example.com/v1
//!
//! Auth:
//! - None (keyless). If your server requires authentication, use a provider
//!   that supports API keys, or extend this provider accordingly.

use super::{AudioFormat, SttError, SttProvider};
use crate::request_log::RequestLogStore;
use async_trait::async_trait;
use reqwest::multipart;
use serde_json::json;

/// Whisper Server STT provider for OpenAI-compatible transcription servers.
pub struct WhisperServerSttProvider {
    client: reqwest::Client,
    base_url: String,
    model: String,
    default_prompt: Option<String>,
    request_log_store: Option<RequestLogStore>,
}

impl WhisperServerSttProvider {
    const DEFAULT_MODEL: &'static str = "whisper-1";
    const PROMPT_MAX_CHARS: usize = 224;

    fn normalize_base_url(base_url: &str) -> Result<String, SttError> {
        let trimmed = base_url.trim();
        if trimmed.is_empty() {
            return Err(SttError::Config(
                "Whisper server base URL is empty".to_string(),
            ));
        }

        // Validate that this looks like a URL early so we can show a clear error.
        // reqwest re-exports Url.
        reqwest::Url::parse(trimmed).map_err(|e| {
            SttError::Config(format!("Invalid Whisper server URL '{}': {}", trimmed, e))
        })?;

        Ok(trimmed.trim_end_matches('/').to_string())
    }

    fn normalize_model(model: Option<String>) -> String {
        model
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .unwrap_or_else(|| Self::DEFAULT_MODEL.to_string())
    }

    fn clamp_prompt(prompt: &str) -> Option<String> {
        let trimmed = prompt.trim();
        if trimmed.is_empty() {
            return None;
        }

        let clamped: String = trimmed.chars().take(Self::PROMPT_MAX_CHARS).collect();
        Some(clamped)
    }

    /// Create a new provider.
    #[allow(dead_code)]
    pub fn new(
        base_url: String,
        model: Option<String>,
        default_prompt: Option<String>,
    ) -> Result<Self, SttError> {
        let client = reqwest::Client::builder()
            .build()
            .map_err(|e| SttError::Config(format!("Failed to create HTTP client: {}", e)))?;

        Self::with_client(client, base_url, model, default_prompt)
    }

    /// Create a new provider with a custom HTTP client.
    pub fn with_client(
        client: reqwest::Client,
        base_url: String,
        model: Option<String>,
        default_prompt: Option<String>,
    ) -> Result<Self, SttError> {
        Ok(Self {
            client,
            base_url: Self::normalize_base_url(&base_url)?,
            model: Self::normalize_model(model),
            default_prompt,
            request_log_store: None,
        })
    }

    pub fn with_request_log_store(mut self, store: Option<RequestLogStore>) -> Self {
        self.request_log_store = store;
        self
    }

    fn endpoint(&self) -> String {
        format!("{}/audio/transcriptions", self.base_url)
    }
}

#[async_trait]
impl SttProvider for WhisperServerSttProvider {
    async fn transcribe(&self, audio: &[u8], _format: &AudioFormat) -> Result<String, SttError> {
        let endpoint = self.endpoint();

        if let Some(store) = &self.request_log_store {
            let prompt = self.default_prompt.as_deref().and_then(Self::clamp_prompt);

            let request_json = json!({
                "provider": "whisper-server",
                "endpoint": endpoint,
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

        let part = multipart::Part::bytes(audio.to_vec())
            .file_name("audio.wav")
            .mime_str("audio/wav")
            .map_err(|e| SttError::Audio(format!("Failed to create multipart: {}", e)))?;

        let mut form = multipart::Form::new()
            .part("file", part)
            .text("model", self.model.clone());

        if let Some(prompt) = self.default_prompt.as_deref().and_then(Self::clamp_prompt) {
            form = form.text("prompt", prompt);
        }

        let response = match self
            .client
            .post(endpoint.clone())
            .multipart(form)
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                if e.is_timeout() {
                    return Err(SttError::Timeout);
                }
                return Err(SttError::Network(e));
            }
        };

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(SttError::Api(format!(
                "Whisper server API error ({}): {}",
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

        let text = result.get("text").and_then(|v| v.as_str()).unwrap_or("");
        Ok(text.to_string())
    }

    fn name(&self) -> &'static str {
        "whisper-server"
    }
}
