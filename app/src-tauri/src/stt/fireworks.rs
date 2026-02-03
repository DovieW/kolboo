//! Fireworks STT provider implementation.
//!
//! Fireworks exposes Whisper transcription endpoints on separate hosts:
//! - whisper-v3:       https://audio-prod.api.fireworks.ai/v1/audio/transcriptions
//! - whisper-v3-turbo: https://audio-turbo.api.fireworks.ai/v1/audio/transcriptions

use super::{http, language, openai_compat};
use super::{AudioFormat, SttError, SttProvider};
use crate::request_log::RequestLogStore;
use async_trait::async_trait;

/// Fireworks STT provider for speech-to-text (Whisper v3 / v3-turbo).
pub struct FireworksSttProvider {
    client: reqwest::Client,
    api_key: String,
    model: String,
    default_prompt: Option<String>,
    default_language: Option<String>,
    api_base_url: Option<String>,
    request_log_store: Option<RequestLogStore>,
}

impl FireworksSttProvider {
    /// Create a new Fireworks STT provider.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn new(
        api_key: String,
        model: Option<String>,
        language: Option<String>,
        default_prompt: Option<String>,
    ) -> Self {
        Self::with_client(
            reqwest::Client::new(),
            api_key,
            model,
            language,
            default_prompt,
        )
    }

    /// Create a new provider with a custom HTTP client.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn with_client(
        client: reqwest::Client,
        api_key: String,
        model: Option<String>,
        language: Option<String>,
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
            default_language: Self::normalize_language(language),
            api_base_url: None,
            request_log_store: None,
        }
    }

    /// Override the API base URL (defaults to Fireworks audio hosts).
    ///
    /// This is primarily intended for deterministic contract tests (e.g., Wiremock).
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn with_api_base_url(mut self, base_url: String) -> Self {
        self.api_base_url = Some(base_url);
        self
    }

    pub fn with_request_log_store(mut self, store: Option<RequestLogStore>) -> Self {
        self.request_log_store = store;
        self
    }

    fn transcriptions_url(&self) -> String {
        if let Some(base_url) = &self.api_base_url {
            http::join_base_url(base_url, "/v1/audio/transcriptions")
        } else if self.model.contains("turbo") {
            "https://audio-turbo.api.fireworks.ai/v1/audio/transcriptions".to_string()
        } else {
            "https://audio-prod.api.fireworks.ai/v1/audio/transcriptions".to_string()
        }
    }

    fn prompt(&self) -> Option<String> {
        self.default_prompt
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
    }

    fn normalize_language(language: Option<String>) -> Option<String> {
        language::normalize_language_code(language)
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

        let prompt = self.prompt();
        let language = self.default_language.as_deref();
        // Fireworks docs show `Authorization: <API_KEY>` for audio endpoints.
        // We pass the stored value through as-is to avoid double-prefixing.
        openai_compat::transcribe_wav_multipart_openai_compat(
            &self.client,
            "fireworks",
            "Fireworks transcription API error",
            &url,
            audio,
            &self.model,
            prompt.as_deref(),
            language,
            self.request_log_store.as_ref(),
            |rb| rb.header("Authorization", &self.api_key),
            SttError::Network,
        )
        .await
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
        let provider = FireworksSttProvider::new("test".to_string(), None, None, None);
        assert_eq!(provider.name(), "fireworks");
    }

    #[test]
    fn test_default_model() {
        let provider = FireworksSttProvider::new("test".to_string(), None, None, None);
        assert_eq!(provider.model, "whisper-v3-turbo");
    }

    #[test]
    fn test_transcriptions_url_switches_on_turbo() {
        let p1 = FireworksSttProvider::new(
            "test".to_string(),
            Some("whisper-v3".to_string()),
            None,
            None,
        );
        assert!(p1.transcriptions_url().contains("audio-prod"));

        let p2 = FireworksSttProvider::new(
            "test".to_string(),
            Some("whisper-v3-turbo".to_string()),
            None,
            None,
        );
        assert!(p2.transcriptions_url().contains("audio-turbo"));
    }
}
