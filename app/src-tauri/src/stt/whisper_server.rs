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

use super::{http, openai_compat, AudioFormat, SttError, SttProvider};
use crate::network::build_plain_http_client_with_timeout;
use crate::request_log::RequestLogStore;
use async_trait::async_trait;
use std::time::Duration;

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
        let raw = base_url.trim();
        if raw.is_empty() {
            return Err(SttError::Config(
                "Whisper server base URL is empty".to_string(),
            ));
        }

        // Validate that this looks like a URL early so we can show a clear error.
        // reqwest re-exports Url.
        reqwest::Url::parse(raw).map_err(|e| {
            SttError::Config(format!("Invalid Whisper server URL '{}': {}", raw, e))
        })?;

        Ok(String::from(http::trim_base_url(raw)))
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
        // Whisper transcription can take a while on slower machines/servers.
        let client = build_plain_http_client_with_timeout(Duration::from_secs(120));

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
        http::join_base_url(&self.base_url, "/audio/transcriptions")
    }
}

#[async_trait]
impl SttProvider for WhisperServerSttProvider {
    async fn transcribe(&self, audio: &[u8], _format: &AudioFormat) -> Result<String, SttError> {
        let endpoint = self.endpoint();

        let prompt = self.default_prompt.as_deref().and_then(Self::clamp_prompt);
        openai_compat::transcribe_wav_multipart_openai_compat(
            &self.client,
            "whisper-server",
            "Whisper server API error",
            &endpoint,
            audio,
            &self.model,
            prompt.as_deref(),
            self.request_log_store.as_ref(),
            |rb| rb,
            SttError::Network,
        )
        .await
    }

    fn name(&self) -> &'static str {
        "whisper-server"
    }
}
