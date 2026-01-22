//! Deepgram STT provider implementation.

use super::http;
use super::{AudioFormat, SttError, SttProvider};
use crate::request_log::RequestLogStore;
use async_trait::async_trait;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use reqwest::Url;
use serde_json::json;
use std::time::Duration;

/// Deepgram API provider for speech-to-text
pub struct DeepgramSttProvider {
    client: reqwest::Client,
    api_key: String,
    model: String,
    api_base_url: String,
    request_log_store: Option<RequestLogStore>,
}

impl DeepgramSttProvider {
    const DEFAULT_DEEPGRAM_API_BASE_URL: &'static str = "https://api.deepgram.com";

    /// Build the Deepgram /v1/listen URL with required query parameters.
    ///
    /// We always enable `smart_format=true` for all Deepgram calls to improve
    /// readability (e.g., numerals/date formatting), and we keep `punctuate=true`
    /// enabled for clean transcripts.
    fn listen_url(&self) -> Result<Url, SttError> {
        let url = http::join_base_url(self.api_base_url_trimmed(), "/v1/listen");
        let mut url = Url::parse(&url)
            .map_err(|e| SttError::Config(format!("Invalid Deepgram base URL: {}", e)))?;

        url.query_pairs_mut()
            .append_pair("model", &self.model)
            .append_pair("smart_format", "true")
            .append_pair("punctuate", "true");

        Ok(url)
    }

    /// Create a new Deepgram STT provider
    ///
    /// # Arguments
    /// * `api_key` - Deepgram API key
    /// * `model` - Model to use (e.g., "nova-2")
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn new(api_key: String, model: Option<String>) -> Self {
        let client = crate::network::build_plain_http_client_with_timeout(Duration::from_secs(60));

        Self {
            client,
            api_key,
            model: model.unwrap_or_else(|| "nova-2".to_string()),
            api_base_url: Self::DEFAULT_DEEPGRAM_API_BASE_URL.to_string(),
            request_log_store: None,
        }
    }

    /// Create a new provider with a custom HTTP client
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn with_client(client: reqwest::Client, api_key: String, model: Option<String>) -> Self {
        Self {
            client,
            api_key,
            model: model.unwrap_or_else(|| "nova-2".to_string()),
            api_base_url: Self::DEFAULT_DEEPGRAM_API_BASE_URL.to_string(),
            request_log_store: None,
        }
    }

    /// Override the API base URL (defaults to https://api.deepgram.com).
    ///
    /// This is primarily intended for deterministic contract tests (e.g., Wiremock).
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn with_api_base_url(mut self, base_url: String) -> Self {
        self.api_base_url = base_url;
        self
    }

    fn api_base_url_trimmed(&self) -> &str {
        http::trim_base_url(&self.api_base_url)
    }

    pub fn with_request_log_store(mut self, store: Option<RequestLogStore>) -> Self {
        self.request_log_store = store;
        self
    }
}

#[async_trait]
impl SttProvider for DeepgramSttProvider {
    async fn transcribe(&self, audio: &[u8], _format: &AudioFormat) -> Result<String, SttError> {
        if let Some(store) = &self.request_log_store {
            let url = self.listen_url()?;
            let request_json = json!({
                "provider": "deepgram",
                "endpoint": url.as_str(),
                "headers": {
                    "content-type": "audio/wav",
                    // Authorization intentionally omitted.
                },
                "body": {
                    "bytes": audio.len(),
                    "data": "<binary audio omitted>",
                }
            });

            store.with_current(|log| {
                log.stt_request_json = Some(request_json);
            });
        }

        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Token {}", self.api_key))
                .map_err(|e| SttError::Config(format!("Invalid API key format: {}", e)))?,
        );
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("audio/wav"));

        let url = self.listen_url()?;

        let response = self
            .client
            .post(url)
            .headers(headers)
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

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(SttError::Api(format!(
                "Deepgram API error ({}): {}",
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

        // Deepgram response structure:
        // { "results": { "channels": [{ "alternatives": [{ "transcript": "..." }] }] } }
        let text = result["results"]["channels"]
            .get(0)
            .and_then(|ch| ch["alternatives"].get(0))
            .and_then(|alt| alt["transcript"].as_str())
            .unwrap_or("")
            .to_string();

        Ok(text)
    }

    fn name(&self) -> &'static str {
        "deepgram"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_creation() {
        let provider = DeepgramSttProvider::new("test-key".to_string(), None);
        assert_eq!(provider.name(), "deepgram");
        assert_eq!(provider.model, "nova-2");
    }

    #[test]
    fn test_provider_with_custom_model() {
        let provider =
            DeepgramSttProvider::new("test-key".to_string(), Some("nova-2-general".to_string()));
        assert_eq!(provider.model, "nova-2-general");
    }
}
