//! ElevenLabs Speech-to-Text (STT) provider implementation.
//!
//! Uses the ElevenLabs "Create transcript" endpoint:
//! POST https://api.elevenlabs.io/v1/speech-to-text
//!
//! Docs:
//! - https://elevenlabs.io/docs/api-reference/speech-to-text/convert

use super::{AudioFormat, SttError, SttProvider};
use crate::request_log::RequestLogStore;
use async_trait::async_trait;
use reqwest::multipart;
use serde_json::json;
use std::time::Duration;

/// ElevenLabs STT provider for speech-to-text.
///
/// Model ids currently supported by the endpoint include:
/// - `scribe_v1`
/// - `scribe_v1_experimental`
pub struct ElevenLabsSttProvider {
    client: reqwest::Client,
    api_key: String,
    model: String,
    api_base_url: String,
    request_log_store: Option<RequestLogStore>,
}

impl ElevenLabsSttProvider {
    const DEFAULT_ELEVENLABS_API_BASE_URL: &'static str = "https://api.elevenlabs.io";

    /// Create a new ElevenLabs STT provider.
    ///
    /// # Arguments
    /// * `api_key` - ElevenLabs API key
    /// * `model` - Model to use (e.g., "scribe_v1")
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn new(api_key: String, model: Option<String>) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            api_key,
            model: model.unwrap_or_else(|| "scribe_v1".to_string()),
            api_base_url: Self::DEFAULT_ELEVENLABS_API_BASE_URL.to_string(),
            request_log_store: None,
        }
    }

    /// Create a new provider with a custom HTTP client.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn with_client(client: reqwest::Client, api_key: String, model: Option<String>) -> Self {
        Self {
            client,
            api_key,
            model: model.unwrap_or_else(|| "scribe_v1".to_string()),
            api_base_url: Self::DEFAULT_ELEVENLABS_API_BASE_URL.to_string(),
            request_log_store: None,
        }
    }

    /// Override the API base URL (defaults to https://api.elevenlabs.io).
    ///
    /// This is primarily intended for deterministic contract tests (e.g., Wiremock).
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn with_api_base_url(mut self, base_url: String) -> Self {
        self.api_base_url = base_url;
        self
    }

    fn api_base_url_trimmed(&self) -> &str {
        self.api_base_url.trim_end_matches('/')
    }

    fn speech_to_text_url(&self) -> String {
        format!("{}/v1/speech-to-text", self.api_base_url_trimmed())
    }

    pub fn with_request_log_store(mut self, store: Option<RequestLogStore>) -> Self {
        self.request_log_store = store;
        self
    }
}

#[async_trait]
impl SttProvider for ElevenLabsSttProvider {
    async fn transcribe(&self, audio: &[u8], _format: &AudioFormat) -> Result<String, SttError> {
        if let Some(store) = &self.request_log_store {
            let request_json = json!({
                "provider": "elevenlabs",
                "endpoint": self.speech_to_text_url(),
                "content_type": "multipart/form-data",
                "fields": {
                    "model_id": self.model,
                    // We intentionally omit optional advanced fields (diarization, timestamps, etc.)
                    // until the app has UX for them.
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

        let form = multipart::Form::new()
            .part("file", part)
            .text("model_id", self.model.clone());

        let response = self
            .client
            .post(self.speech_to_text_url())
            .header("xi-api-key", &self.api_key)
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
                "ElevenLabs STT API error ({}): {}",
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
        "elevenlabs"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_creation() {
        let provider = ElevenLabsSttProvider::new("test-key".to_string(), None);
        assert_eq!(provider.name(), "elevenlabs");
        assert_eq!(provider.model, "scribe_v1");
    }

    #[test]
    fn test_provider_with_custom_model() {
        let provider = ElevenLabsSttProvider::new(
            "test-key".to_string(),
            Some("scribe_v1_experimental".to_string()),
        );
        assert_eq!(provider.model, "scribe_v1_experimental");
    }
}
