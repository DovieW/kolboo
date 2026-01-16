//! AssemblyAI STT provider implementation.
//!
//! AssemblyAI's pre-recorded transcription API is asynchronous:
//! 1) Upload the audio bytes to `/v2/upload` to get an `upload_url`.
//! 2) Submit a transcript job to `/v2/transcript`.
//! 3) Poll `/v2/transcript/{id}` until it completes.
//!
//! Docs (fetched 2025-12-28):
//! - https://www.assemblyai.com/docs/api-reference/files/upload
//! - https://www.assemblyai.com/docs/api-reference/transcripts/submit
//! - https://www.assemblyai.com/docs/api-reference/transcripts/get

use super::{AudioFormat, SttError, SttProvider};
use crate::request_log::RequestLogStore;
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
    api_base_url: String,
    request_log_store: Option<RequestLogStore>,
}

impl AssemblyAiSttProvider {
    const DEFAULT_API_BASE_URL: &'static str = "https://api.assemblyai.com";

    /// Create a new AssemblyAI provider.
    ///
    /// Supported models (per API docs):
    /// - "universal" (default)
    /// - "slam-1"
    /// - "best" (legacy)
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn new(api_key: String, model: Option<String>) -> Self {
        let client = reqwest::Client::builder()
            // AssemblyAI transcription is async; allow a longer HTTP timeout.
            // The pipeline still applies its own overall timeout.
            .timeout(Duration::from_secs(120))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            api_key,
            model: model.unwrap_or_else(|| "universal".to_string()),
            api_base_url: Self::DEFAULT_API_BASE_URL.to_string(),
            request_log_store: None,
        }
    }

    /// Create a new provider with a custom HTTP client.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn with_client(client: reqwest::Client, api_key: String, model: Option<String>) -> Self {
        Self {
            client,
            api_key,
            model: model.unwrap_or_else(|| "universal".to_string()),
            api_base_url: Self::DEFAULT_API_BASE_URL.to_string(),
            request_log_store: None,
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

    fn api_base_url_trimmed(&self) -> &str {
        self.api_base_url.trim_end_matches('/')
    }

    fn upload_url(&self) -> String {
        format!("{}/v2/upload", self.api_base_url_trimmed())
    }

    fn transcript_url(&self) -> String {
        format!("{}/v2/transcript", self.api_base_url_trimmed())
    }

    fn transcript_get_url(&self, transcript_id: &str) -> String {
        format!(
            "{}/v2/transcript/{}",
            self.api_base_url_trimmed(),
            transcript_id
        )
    }

    async fn upload_audio(&self, audio: &[u8]) -> Result<String, SttError> {
        let resp = self
            .client
            .post(self.upload_url())
            .header("Authorization", &self.api_key)
            .header("Content-Type", "application/octet-stream")
            .body(audio.to_vec())
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    SttError::Timeout
                } else {
                    SttError::Network(e)
                }
            })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let error_text = resp
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(SttError::Api(format!(
                "AssemblyAI upload error ({}): {}",
                status, error_text
            )));
        }

        let parsed: UploadResponse = resp.json().await.map_err(SttError::Network)?;
        Ok(parsed.upload_url)
    }

    async fn submit_transcript(&self, upload_url: &str) -> Result<String, SttError> {
        // `speech_model` is deprecated; `speech_models` is the preferred param.
        // Supplying a single model is a direct selection.
        let body = json!({
            "audio_url": upload_url,
            "speech_models": [self.model.clone()],
            // Keep output consistent with other providers.
            "punctuate": true,
            "format_text": true,
        });

        let resp = self
            .client
            .post(self.transcript_url())
            .header("Authorization", &self.api_key)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    SttError::Timeout
                } else {
                    SttError::Network(e)
                }
            })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let error_text = resp
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(SttError::Api(format!(
                "AssemblyAI submit error ({}): {}",
                status, error_text
            )));
        }

        let parsed: TranscriptSubmitResponse = resp.json().await.map_err(SttError::Network)?;
        Ok(parsed.id)
    }

    async fn get_transcript(&self, transcript_id: &str) -> Result<TranscriptGetResponse, SttError> {
        let resp = self
            .client
            .get(self.transcript_get_url(transcript_id))
            .header("Authorization", &self.api_key)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    SttError::Timeout
                } else {
                    SttError::Network(e)
                }
            })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let error_text = resp
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(SttError::Api(format!(
                "AssemblyAI get error ({}): {}",
                status, error_text
            )));
        }

        resp.json().await.map_err(SttError::Network)
    }

    async fn poll_until_done(&self, transcript_id: &str) -> Result<String, SttError> {
        // Poll with a small backoff. The outer pipeline has its own overall timeout.
        let mut delay = Duration::from_millis(250);
        let max_delay = Duration::from_secs(2);

        loop {
            let res = self.get_transcript(transcript_id).await?;

            match res.status {
                TranscriptStatus::Completed => {
                    return Ok(res.text.unwrap_or_default());
                }
                TranscriptStatus::Error => {
                    return Err(SttError::Api(format!(
                        "AssemblyAI transcription failed: {}",
                        res.error.unwrap_or_else(|| "Unknown error".to_string())
                    )));
                }
                TranscriptStatus::Queued | TranscriptStatus::Processing => {
                    tokio::time::sleep(delay).await;
                    delay = std::cmp::min(max_delay, delay.saturating_mul(2));
                }
            }
        }
    }
}

#[async_trait]
impl SttProvider for AssemblyAiSttProvider {
    async fn transcribe(&self, audio: &[u8], _format: &AudioFormat) -> Result<String, SttError> {
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
                            "audio_url": "<upload_url from previous step>",
                        }
                    },
                    {
                        "endpoint": format!("{}/v2/transcript/{{id}}", self.api_base_url_trimmed()),
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
            // Best effort: fetch the final transcript JSON for logging.
            if let Ok(final_resp) = self.get_transcript(&transcript_id).await {
                let response_json = serde_json::to_value(final_resp).unwrap_or_else(|_| json!({}));
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_creation_defaults() {
        let provider = AssemblyAiSttProvider::new("test-key".to_string(), None);
        assert_eq!(provider.name(), "assemblyai");
        assert_eq!(provider.model, "universal");
    }

    #[test]
    fn test_provider_with_custom_model() {
        let provider =
            AssemblyAiSttProvider::new("test-key".to_string(), Some("slam-1".to_string()));
        assert_eq!(provider.model, "slam-1");
    }
}
