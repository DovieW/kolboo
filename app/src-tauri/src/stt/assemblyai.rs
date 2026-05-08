//! AssemblyAI STT provider implementation.
//!
//! Supports both batch (pre-recorded) and real-time streaming transcription:
//!
//! **Batch** (upload → submit → poll):
//! - Models: "universal" (default), "slam-1", "best" (legacy)
//! - Upload to `/v2/upload`, submit to `/v2/transcript`, poll until done.
//!
//! **Streaming** (WebSocket):
//! - Models: "universal-streaming-english", "universal-streaming-multilingual"
//! - Endpoint: `wss://streaming.assemblyai.com/v3/ws`
//! - Audio: PCM 16-bit LE, mono at capture sample rate (`encoding=pcm_s16le`)
//! - Protocol: binary audio frames → Turn messages with `turn_is_formatted`
//!   flag for live-output commits. Send `{"type": "Terminate"}` to end session.
//!
//! Docs:
//! - <https://www.assemblyai.com/docs/api-reference/files/upload>
//! - <https://www.assemblyai.com/docs/api-reference/transcripts/submit>
//! - <https://www.assemblyai.com/docs/api-reference/streaming-api/streaming-api>

mod realtime;

use super::http;
use super::language;
use super::streaming::StreamingSttSession;
use super::{AudioFormat, SttError, SttProvider};
use crate::request_log::RequestLogStore;
use crate::settings::ProxySettings;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::Duration;

#[derive(Debug, Deserialize, Serialize)]
struct UploadResponse {
    upload_url: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct TranscriptSubmitResponse {
    id: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum TranscriptStatus {
    Queued,
    Processing,
    Completed,
    Error,
}

#[derive(Debug, Deserialize, Serialize)]
struct TranscriptGetResponse {
    status: TranscriptStatus,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    error: Option<String>,
}

/// AssemblyAI STT provider for speech-to-text.
pub struct AssemblyAiSttProvider {
    client: reqwest::Client,
    api_key: String,
    model: String,
    language_code: Option<String>,
    language_detection: bool,
    api_base_url: String,
    request_log_store: Option<RequestLogStore>,
    proxy_settings: ProxySettings,
}

/// Models that use the real-time WebSocket streaming API.
const STREAMING_MODELS: &[&str] = &[
    "universal-streaming-english",
    "universal-streaming-multilingual",
];

impl AssemblyAiSttProvider {
    pub(super) const DEFAULT_API_BASE_URL: &'static str = "https://api.assemblyai.com";
    /// Default streaming WebSocket endpoint.
    pub(super) const DEFAULT_STREAMING_WS_URL: &'static str =
        "wss://streaming.assemblyai.com/v3/ws";
    /// Default connect / read timeout for WebSocket operations.
    pub(super) const DEFAULT_WS_TIMEOUT: Duration = Duration::from_secs(30);
    /// Timeout for waiting for the server `Termination` message after `Terminate`.
    pub(super) const POST_TERMINATE_TIMEOUT: Duration = Duration::from_secs(15);

    /// Create a new AssemblyAI provider.
    ///
    /// Supported models (per API docs):
    /// - "universal" (default)
    /// - "slam-1"
    /// - "best" (legacy)
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn new(api_key: String, model: Option<String>, language: Option<String>) -> Self {
        let client = crate::network::build_plain_http_client_with_timeout(Duration::from_secs(120));
        let (language_code, language_detection) = Self::normalize_language(language);

        Self {
            client,
            api_key,
            model: model.unwrap_or_else(|| "universal".to_string()),
            language_code,
            language_detection,
            api_base_url: Self::DEFAULT_API_BASE_URL.to_string(),
            request_log_store: None,
            proxy_settings: ProxySettings::default(),
        }
    }

    /// Create a new provider with a custom HTTP client.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn with_client(
        client: reqwest::Client,
        api_key: String,
        model: Option<String>,
        language: Option<String>,
    ) -> Self {
        let (language_code, language_detection) = Self::normalize_language(language);
        Self {
            client,
            api_key,
            model: model.unwrap_or_else(|| "universal".to_string()),
            language_code,
            language_detection,
            api_base_url: Self::DEFAULT_API_BASE_URL.to_string(),
            request_log_store: None,
            proxy_settings: ProxySettings::default(),
        }
    }

    /// Override the API base URL (defaults to https://api.assemblyai.com).
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

    pub fn with_proxy_settings(mut self, proxy_settings: ProxySettings) -> Self {
        self.proxy_settings = proxy_settings;
        self
    }

    fn api_base_url_trimmed(&self) -> &str {
        http::trim_base_url(&self.api_base_url)
    }

    fn upload_url(&self) -> String {
        http::join_base_url(self.api_base_url_trimmed(), "/v2/upload")
    }

    fn transcript_url(&self) -> String {
        http::join_base_url(self.api_base_url_trimmed(), "/v2/transcript")
    }

    fn transcript_get_url(&self, transcript_id: &str) -> String {
        http::join_base_url(
            self.api_base_url_trimmed(),
            &format!("/v2/transcript/{}", transcript_id),
        )
    }

    fn normalize_language(language: Option<String>) -> (Option<String>, bool) {
        let Some(raw) = language::normalize_language_setting(language) else {
            return (None, true);
        };

        let mapped = match raw.as_str() {
            "en" => "en_us",
            "es" => "es",
            "fr" => "fr",
            "de" => "de",
            "it" => "it",
            "pt" => "pt",
            "zh" => "zh",
            "ja" => "ja",
            "ko" => "ko",
            "hi" => "hi",
            "ar" => "ar",
            "ru" => "ru",
            other => other,
        };

        (Some(mapped.to_string()), false)
    }

    async fn upload_audio(&self, audio: &[u8]) -> Result<String, SttError> {
        let response = self
            .client
            .post(self.upload_url())
            .header("Authorization", &self.api_key)
            .header("Content-Type", "application/octet-stream")
            .body(audio.to_vec())
            .send()
            .await
            .map_err(|error| {
                if error.is_timeout() {
                    SttError::Timeout
                } else {
                    SttError::Network(error)
                }
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(SttError::Api(format!(
                "AssemblyAI upload error ({}): {}",
                status, error_text
            )));
        }

        let parsed: UploadResponse = response.json().await.map_err(SttError::Network)?;
        Ok(parsed.upload_url)
    }

    async fn submit_transcript(&self, upload_url: &str) -> Result<String, SttError> {
        // `speech_model` is deprecated; `speech_models` is the preferred param.
        // Supplying a single model is a direct selection.
        let mut body = json!({
            "audio_url": upload_url,
            "speech_models": [self.model.clone()],
            // Keep output consistent with other providers.
            "punctuate": true,
            "format_text": true,
        });

        if let Some(language_code) = self.language_code.as_deref() {
            body["language_code"] = json!(language_code);
        }
        if self.language_detection {
            body["language_detection"] = json!(true);
        }

        let response = self
            .client
            .post(self.transcript_url())
            .header("Authorization", &self.api_key)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|error| {
                if error.is_timeout() {
                    SttError::Timeout
                } else {
                    SttError::Network(error)
                }
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(SttError::Api(format!(
                "AssemblyAI submit error ({}): {}",
                status, error_text
            )));
        }

        let parsed: TranscriptSubmitResponse = response.json().await.map_err(SttError::Network)?;
        Ok(parsed.id)
    }

    async fn get_transcript(&self, transcript_id: &str) -> Result<TranscriptGetResponse, SttError> {
        let response = self
            .client
            .get(self.transcript_get_url(transcript_id))
            .header("Authorization", &self.api_key)
            .send()
            .await
            .map_err(|error| {
                if error.is_timeout() {
                    SttError::Timeout
                } else {
                    SttError::Network(error)
                }
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(SttError::Api(format!(
                "AssemblyAI get error ({}): {}",
                status, error_text
            )));
        }

        response.json().await.map_err(SttError::Network)
    }

    async fn poll_until_done(&self, transcript_id: &str) -> Result<String, SttError> {
        // Poll with a small backoff. The outer pipeline has its own overall timeout.
        let mut delay = Duration::from_millis(250);
        let max_delay = Duration::from_secs(2);

        loop {
            let response = self.get_transcript(transcript_id).await?;

            match response.status {
                TranscriptStatus::Completed => {
                    return Ok(response.text.unwrap_or_default());
                }
                TranscriptStatus::Error => {
                    return Err(SttError::Api(format!(
                        "AssemblyAI transcription failed: {}",
                        response
                            .error
                            .unwrap_or_else(|| "Unknown error".to_string())
                    )));
                }
                TranscriptStatus::Queued | TranscriptStatus::Processing => {
                    tokio::time::sleep(delay).await;
                    delay = std::cmp::min(max_delay, delay.saturating_mul(2));
                }
            }
        }
    }

    /// Whether this model uses the real-time WebSocket streaming API.
    fn is_streaming_model(&self) -> bool {
        STREAMING_MODELS.iter().any(|model| *model == self.model)
    }

    /// Start a real-time WebSocket streaming session.
    async fn start_streaming_session(
        &self,
        sample_rate: u32,
    ) -> Result<StreamingSttSession, SttError> {
        realtime::start_streaming_session(self, sample_rate).await
    }
}

#[async_trait]
impl SttProvider for AssemblyAiSttProvider {
    async fn transcribe(&self, audio: &[u8], _format: &AudioFormat) -> Result<String, SttError> {
        if self.is_streaming_model() {
            return Err(SttError::Config(format!(
                "Model '{}' only supports real-time streaming, not batch transcription",
                self.model
            )));
        }

        if let Some(store) = &self.request_log_store {
            let request_json = json!({
                "provider": "assemblyai",
                "steps": [
                    {
                        "endpoint": self.upload_url(),
                        "content_type": "application/octet-stream",
                        "body": {
                            "bytes": audio.len(),
                            "data": "<binary audio omitted>",
                        }
                    },
                    {
                        "endpoint": self.transcript_url(),
                        "content_type": "application/json",
                        "body": {
                            "speech_models": [self.model.clone()],
                            "punctuate": true,
                            "format_text": true,
                            "language_code": self.language_code.clone(),
                            "language_detection": self.language_detection,
                            "audio_url": "<upload_url from previous step>",
                        }
                    },
                    {
                        "endpoint": http::join_base_url(self.api_base_url_trimmed(), "/v2/transcript/{id}"),
                        "method": "GET",
                        "note": "Polled until status=completed",
                    }
                ]
            });

            store.with_current(|log| {
                log.stt_request_json = Some(request_json);
            });
        }

        let upload_url = self.upload_audio(audio).await?;
        let transcript_id = self.submit_transcript(&upload_url).await?;
        let text = self.poll_until_done(&transcript_id).await?;

        if let Some(store) = &self.request_log_store {
            // Best effort: fetch the final transcript JSON for logging so request logs
            // still capture AssemblyAI's terminal payload after the batch workflow.
            if let Ok(final_response) = self.get_transcript(&transcript_id).await {
                let response_json =
                    serde_json::to_value(final_response).unwrap_or_else(|_| json!({}));
                store.with_current(|log| {
                    log.stt_response_json = Some(response_json);
                });
            }
        }

        Ok(text)
    }

    fn name(&self) -> &'static str {
        "assemblyai"
    }

    fn supports_streaming(&self) -> bool {
        self.is_streaming_model()
    }

    fn requires_streaming(&self) -> bool {
        self.is_streaming_model()
    }

    async fn start_streaming(&self, sample_rate: u32) -> Result<StreamingSttSession, SttError> {
        self.start_streaming_session(sample_rate).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_creation_defaults() {
        let provider = AssemblyAiSttProvider::new("test-key".to_string(), None, None);
        assert_eq!(provider.name(), "assemblyai");
        assert_eq!(provider.model, "universal");
    }

    #[test]
    fn test_provider_with_custom_model() {
        let provider =
            AssemblyAiSttProvider::new("test-key".to_string(), Some("slam-1".to_string()), None);
        assert_eq!(provider.model, "slam-1");
    }

    #[test]
    fn test_streaming_model_detection() {
        let batch =
            AssemblyAiSttProvider::new("key".to_string(), Some("universal".to_string()), None);
        assert!(!batch.is_streaming_model());
        assert!(!batch.supports_streaming());

        let english = AssemblyAiSttProvider::new(
            "key".to_string(),
            Some("universal-streaming-english".to_string()),
            None,
        );
        assert!(english.is_streaming_model());
        assert!(english.supports_streaming());
        assert!(english.requires_streaming());

        let multilingual = AssemblyAiSttProvider::new(
            "key".to_string(),
            Some("universal-streaming-multilingual".to_string()),
            None,
        );
        assert!(multilingual.is_streaming_model());
        assert!(multilingual.supports_streaming());
        assert!(multilingual.requires_streaming());
    }
}
