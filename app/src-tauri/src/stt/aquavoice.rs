//! Aquavoice (Avalon) STT provider implementation.
//!
//! Avalon is an OpenAI Whisper-compatible transcription API.
//! Docs: https://aquavoice.com/avalon-api
//!
//! Implementation notes:
//! - Uses the OpenAI-compatible `POST /v1/audio/transcriptions` endpoint.
//! - Auth is via `Authorization: Bearer <api_key>`.
//! - Model defaults to `avalon-1`.

use super::{AudioFormat, SttError, SttProvider};
use async_trait::async_trait;
use crate::request_log::RequestLogStore;
use reqwest::multipart;
use serde_json::json;
use std::time::Duration;

/// Aquavoice Avalon STT provider for speech-to-text
pub struct AquavoiceSttProvider {
    client: reqwest::Client,
    api_key: String,
    model: String,
    default_prompt: Option<String>,
    request_log_store: Option<RequestLogStore>,
}

impl AquavoiceSttProvider {
    const DEFAULT_BASE_URL: &'static str = "https://api.aqua.sh/v1";
    // Alternate base URL observed in the wild; used as a best-effort fallback when
    // the default host presents a TLS certificate for the wrong domain.
    const FALLBACK_BASE_URL: &'static str = "https://api.aquavoice.com/v1";
    const DEFAULT_MODEL: &'static str = "avalon-1";
    const PROMPT_MAX_CHARS: usize = 224;

    /// Create a new Aquavoice STT provider.
    ///
    /// # Arguments
    /// * `api_key` - Aquavoice API key
    /// * `model` - Model to use (default: `avalon-1`)
    /// * `default_prompt` - Optional transcription prompt (OpenAI-compatible `prompt` field)
    pub fn new(
        api_key: String,
        model: Option<String>,
        default_prompt: Option<String>,
    ) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            api_key,
            model: model.unwrap_or_else(|| Self::DEFAULT_MODEL.to_string()),
            default_prompt,
            request_log_store: None,
        }
    }

    /// Create a new provider with a custom HTTP client (mainly for tests).
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
            model: model.unwrap_or_else(|| Self::DEFAULT_MODEL.to_string()),
            default_prompt,
            request_log_store: None,
        }
    }

    pub fn with_request_log_store(mut self, store: Option<RequestLogStore>) -> Self {
        self.request_log_store = store;
        self
    }

    fn endpoint_for_base_url(base_url: &str) -> String {
        format!("{}/audio/transcriptions", base_url.trim_end_matches('/'))
    }

    fn endpoint(&self) -> String {
        Self::endpoint_for_base_url(Self::DEFAULT_BASE_URL)
    }

    fn looks_like_tls_hostname_mismatch(err: &reqwest::Error) -> bool {
        if !err.is_connect() {
            return false;
        }
        let msg = err.to_string().to_lowercase();
        // Windows Schannel: SEC_E_WRONG_PRINCIPAL / "target principal name is incorrect".
        // Rustls/nativetls: "certificate verify failed" / "invalid peer certificate".
        msg.contains("wrong_principal")
            || msg.contains("target principal")
            || msg.contains("certificate")
            || msg.contains("invalid peer")
            || msg.contains("tls")
    }

    fn network_error_message(endpoint: &str, err: &reqwest::Error) -> String {
        let mut msg = format!("error sending request for url ({}): {}", endpoint, err);
        if Self::looks_like_tls_hostname_mismatch(err) {
            msg.push_str("\n\nIt looks like a TLS certificate / hostname mismatch. ");
            msg.push_str(
                "This usually indicates the server certificate being presented is not valid for the documented host on this machine. ",
            );
            msg.push_str(
                "Tambourine Voice will automatically retry via an alternate Aquovoice host. If the problem persists, please check Aquovoice status or contact Aquovoice support.",
            );
        }
        msg
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
impl SttProvider for AquavoiceSttProvider {
    async fn transcribe(&self, audio: &[u8], _format: &AudioFormat) -> Result<String, SttError> {
        let endpoint = self.endpoint();

        if let Some(store) = &self.request_log_store {
            let prompt = self
                .default_prompt
                .as_deref()
                .and_then(Self::clamp_prompt);

            let request_json = json!({
                "provider": "aquavoice",
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

        let make_form = || -> Result<multipart::Form, SttError> {
            let part = multipart::Part::bytes(audio.to_vec())
                .file_name("audio.wav")
                .mime_str("audio/wav")
                .map_err(|e| SttError::Audio(format!("Failed to create multipart: {}", e)))?;

            let mut form = multipart::Form::new()
                .part("file", part)
                .text("model", self.model.clone());

            if let Some(prompt) = self
                .default_prompt
                .as_deref()
                .and_then(Self::clamp_prompt)
            {
                form = form.text("prompt", prompt);
            }

            Ok(form)
        };

        let response = match self
            .client
            .post(endpoint.clone())
            .bearer_auth(&self.api_key)
            .multipart(make_form()?)
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                if e.is_timeout() {
                    return Err(SttError::Timeout);
                }

                // If the default host is presenting a TLS certificate for the wrong
                // domain (observed on Windows), try the alternate Aquovoice API host.
                if Self::looks_like_tls_hostname_mismatch(&e) {
                    let fallback_endpoint = Self::endpoint_for_base_url(Self::FALLBACK_BASE_URL);
                    log::warn!(
                        "Aquovoice STT: TLS/hostname mismatch at {}, retrying via {}",
                        endpoint,
                        fallback_endpoint
                    );

                    if let Some(store) = &self.request_log_store {
                        store.with_current(|log| {
                            log.warn(
                                "Aquovoice STT: TLS/hostname mismatch at api.aqua.sh; retrying via api.aquavoice.com",
                            );
                        });
                    }

                    match self
                        .client
                        .post(fallback_endpoint.clone())
                        .bearer_auth(&self.api_key)
                        .multipart(make_form()?)
                        .send()
                        .await
                    {
                        Ok(r) => r,
                        Err(e2) => {
                            if e2.is_timeout() {
                                return Err(SttError::Timeout);
                            }
                            return Err(SttError::NetworkMessage(Self::network_error_message(
                                &fallback_endpoint,
                                &e2,
                            )));
                        }
                    }
                } else {
                    return Err(SttError::NetworkMessage(Self::network_error_message(
                        &endpoint,
                        &e,
                    )));
                }
            }
        };

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(SttError::Api(format!(
                "Aquavoice API error ({}): {}",
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
        "aquavoice"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_creation() {
        let provider = AquavoiceSttProvider::new("test-key".to_string(), None, None, None);
        assert_eq!(provider.name(), "aquavoice");
        assert_eq!(provider.model, AquavoiceSttProvider::DEFAULT_MODEL);
    }

    #[test]
    fn test_provider_with_custom_model() {
        let provider = AquavoiceSttProvider::new(
            "test-key".to_string(),
            Some("avalon-1".to_string()),
            None,
            None,
        );
        assert_eq!(provider.model, "avalon-1");
    }

    #[test]
    fn test_prompt_clamping() {
        let long = "x".repeat(AquavoiceSttProvider::PROMPT_MAX_CHARS + 10);
        let clamped = AquavoiceSttProvider::clamp_prompt(&long).unwrap();
        assert_eq!(clamped.len(), AquavoiceSttProvider::PROMPT_MAX_CHARS);
    }
}
