//! Fireworks STT provider implementation.
//!
//! Fireworks exposes Whisper transcription endpoints on separate hosts:
//! - whisper-v3:       https://audio-prod.api.fireworks.ai/v1/audio/transcriptions
//! - whisper-v3-turbo: https://audio-turbo.api.fireworks.ai/v1/audio/transcriptions

use super::{AudioFormat, SttError, SttProvider};
use async_trait::async_trait;
use crate::request_log::RequestLogStore;
use reqwest::multipart;
use serde_json::json;

/// Fireworks STT provider for speech-to-text (Whisper v3 / v3-turbo).
pub struct FireworksSttProvider {
    client: reqwest::Client,
    api_key: String,
    model: String,
    default_prompt: Option<String>,
    request_log_store: Option<RequestLogStore>,
}

impl FireworksSttProvider {
    /// Create a new Fireworks STT provider.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn new(api_key: String, model: Option<String>, default_prompt: Option<String>) -> Self {
        Self::with_client(reqwest::Client::new(), api_key, model, default_prompt)
    }

    /// Create a new provider with a custom HTTP client.
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
            model: model.unwrap_or_else(|| "whisper-v3-turbo".to_string()),
            default_prompt: default_prompt
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string()),
            request_log_store: None,
        }
    }

    pub fn with_request_log_store(mut self, store: Option<RequestLogStore>) -> Self {
        self.request_log_store = store;
        self
    }

    fn transcriptions_url(&self) -> &'static str {
        if self.model.contains("turbo") {
            "https://audio-turbo.api.fireworks.ai/v1/audio/transcriptions"
        } else {
            "https://audio-prod.api.fireworks.ai/v1/audio/transcriptions"
        }
    }

    fn prompt(&self) -> Option<String> {
        self.default_prompt
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
    }
}

#[async_trait]
impl SttProvider for FireworksSttProvider {
    async fn transcribe(&self, audio: &[u8], _format: &AudioFormat) -> Result<String, SttError> {
        if self.api_key.is_empty() {
            return Err(SttError::Config(
                "No API key configured for provider: fireworks".to_string(),
            ));
        }

        let url = self.transcriptions_url();

        if let Some(store) = &self.request_log_store {
            let request_json = json!({
                "provider": "fireworks",
                "endpoint": url,
                "content_type": "multipart/form-data",
                "fields": {
                    "model": self.model,
                    "prompt": self.prompt(),
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

        if let Some(prompt) = self.prompt() {
            form = form.text("prompt", prompt);
        }

        // Fireworks docs show `Authorization: <API_KEY>` for audio endpoints.
        // We pass the stored value through as-is to avoid double-prefixing.
        let response = self
            .client
            .post(url)
            .header("Authorization", &self.api_key)
            .multipart(form)
            .send()
            .await
            .map_err(|e| if e.is_timeout() { SttError::Timeout } else { SttError::Network(e) })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(SttError::Api(format!(
                "Fireworks transcription API error ({}): {}",
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
        "fireworks"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_name() {
        let provider = FireworksSttProvider::new("test".to_string(), None, None);
        assert_eq!(provider.name(), "fireworks");
    }

    #[test]
    fn test_default_model() {
        let provider = FireworksSttProvider::new("test".to_string(), None, None);
        assert_eq!(provider.model, "whisper-v3-turbo");
    }

    #[test]
    fn test_transcriptions_url_switches_on_turbo() {
        let p1 = FireworksSttProvider::new("test".to_string(), Some("whisper-v3".to_string()), None);
        assert!(p1.transcriptions_url().contains("audio-prod"));

        let p2 = FireworksSttProvider::new(
            "test".to_string(),
            Some("whisper-v3-turbo".to_string()),
            None,
        );
        assert!(p2.transcriptions_url().contains("audio-turbo"));
    }
}
