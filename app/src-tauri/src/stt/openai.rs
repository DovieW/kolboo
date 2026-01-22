//! OpenAI STT provider implementation.
//!
//! Supports two modes:
//! - Legacy Whisper API (whisper-1) - uses /v1/audio/transcriptions
//! - Audio chat models (e.g., gpt-4o-audio-preview) - uses /v1/responses with audio input

use super::http;
use super::openai_compat;
use super::{AudioFormat, SttError, SttProvider};
use crate::request_log::RequestLogStore;
use async_trait::async_trait;
use serde_json::json;
use std::time::Duration;

/// OpenAI STT provider for speech-to-text
pub struct OpenAiSttProvider {
    client: reqwest::Client,
    api_key: String,
    model: String,
    default_prompt: Option<String>,
    api_base_url: String,
    request_log_store: Option<RequestLogStore>,
}

impl OpenAiSttProvider {
    const WHISPER_PROMPT_MAX_CHARS: usize = 224;
    const DEFAULT_OPENAI_API_BASE_URL: &'static str = "https://api.openai.com";

    /// Create a new OpenAI STT provider
    ///
    /// # Arguments
    /// * `api_key` - OpenAI API key
    /// * `model` - Model to use:
    ///   - "gpt-4o-audio-preview" (default) - GPT-4o with audio input
    ///   - "gpt-4o-mini-audio-preview" - Smaller/faster GPT-4o audio
    ///   - "whisper-1" - Legacy Whisper API
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn new(api_key: String, model: Option<String>, default_prompt: Option<String>) -> Self {
        let client = crate::network::build_plain_http_client_with_timeout(Duration::from_secs(120));

        Self {
            client,
            api_key,
            model: model.unwrap_or_else(|| "gpt-4o-audio-preview".to_string()),
            default_prompt: default_prompt
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string()),
            api_base_url: Self::DEFAULT_OPENAI_API_BASE_URL.to_string(),
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
            model: model.unwrap_or_else(|| "gpt-4o-audio-preview".to_string()),
            default_prompt: default_prompt
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string()),
            api_base_url: Self::DEFAULT_OPENAI_API_BASE_URL.to_string(),
            request_log_store: None,
        }
    }

    /// Override the API base URL (defaults to https://api.openai.com).
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

    fn transcriptions_url(&self) -> String {
        http::join_base_url(self.api_base_url_trimmed(), "/v1/audio/transcriptions")
    }

    fn responses_url(&self) -> String {
        http::join_base_url(self.api_base_url_trimmed(), "/v1/responses")
    }

    pub fn with_request_log_store(mut self, store: Option<RequestLogStore>) -> Self {
        self.request_log_store = store;
        self
    }

    /// Check if this model should use /v1/audio/transcriptions.
    ///
    /// Per OpenAI docs, `whisper-1` and the `*-transcribe` models are used via the
    /// dedicated transcription endpoint.
    fn uses_transcriptions_endpoint(&self) -> bool {
        self.model == "whisper-1" || self.model.contains("transcribe")
    }

    fn clamp_prompt_for_model(&self, prompt: Option<&str>) -> Option<String> {
        let prompt = prompt.map(str::trim).filter(|s| !s.is_empty())?;

        // Prompt support is only enabled for the dedicated transcription endpoint models.
        // If the user selected an OpenAI audio-chat model (Responses API path), ignore the prompt.
        if !self.uses_transcriptions_endpoint() {
            return None;
        }

        // Diarize models do not support the `prompt` parameter.
        if self.model.contains("diarize") {
            return None;
        }

        // OpenAI docs say Whisper only considers 224 tokens. Tokenization differs by language.
        // For a simple, predictable UX (and to match our UI), we clamp to 224 characters.
        if self.model == "whisper-1" && prompt.len() > Self::WHISPER_PROMPT_MAX_CHARS {
            return Some(
                prompt
                    .chars()
                    .take(Self::WHISPER_PROMPT_MAX_CHARS)
                    .collect(),
            );
        }

        Some(prompt.to_string())
    }

    /// Transcribe using the dedicated OpenAI transcription endpoint.
    async fn transcribe_audio_transcriptions(
        &self,
        audio: &[u8],
        prompt: Option<&str>,
    ) -> Result<String, SttError> {
        if let Some(store) = &self.request_log_store {
            let prompt = self.clamp_prompt_for_model(prompt);
            let request_json = json!({
                "provider": "openai",
                "endpoint": self.transcriptions_url(),
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

        let clamped_prompt = self.clamp_prompt_for_model(prompt);
        let form =
            openai_compat::wav_transcription_form(audio, &self.model, clamped_prompt.as_deref())?;

        let response = self
            .client
            .post(self.transcriptions_url())
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
                "OpenAI Whisper API error ({}): {}",
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

    fn extract_responses_output_text(value: &serde_json::Value) -> Result<String, SttError> {
        if let Some(s) = value.get("output_text").and_then(|v| v.as_str()) {
            return Ok(s.to_string());
        }

        let output = value
            .get("output")
            .and_then(|v| v.as_array())
            .ok_or_else(|| SttError::Api("Responses API returned no 'output' array".to_string()))?;

        for item in output {
            if item.get("type").and_then(|t| t.as_str()) != Some("message") {
                continue;
            }

            let content = match item.get("content").and_then(|c| c.as_array()) {
                Some(c) => c,
                None => continue,
            };

            for part in content {
                match part.get("type").and_then(|t| t.as_str()) {
                    Some("refusal") => {
                        let refusal = part.get("refusal").and_then(|r| r.as_str()).unwrap_or("");
                        return Err(SttError::Api(format!("OpenAI refusal: {}", refusal)));
                    }
                    Some("output_text") => {
                        if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                            return Ok(text.to_string());
                        }
                    }
                    _ => {}
                }
            }
        }

        Err(SttError::Api(
            "Responses API returned no output_text content".to_string(),
        ))
    }

    /// Transcribe using the Responses API with audio input.
    async fn transcribe_responses_audio(
        &self,
        audio: &[u8],
        prompt: Option<&str>,
    ) -> Result<String, SttError> {
        use base64::{engine::general_purpose::STANDARD, Engine};

        // Encode audio as base64
        let audio_base64 = STANDARD.encode(audio);

        let mut instruction =
            "Transcribe this audio. Output only the transcribed text, nothing else.".to_string();
        if let Some(prompt) = self.clamp_prompt_for_model(prompt) {
            instruction.push_str("\n\nContext/prompt: ");
            instruction.push_str(&prompt);
        }

        let request_body = json!({
            "model": self.model,
            "input": [
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "input_audio",
                            "input_audio": {
                                "data": audio_base64,
                                "format": "wav"
                            }
                        },
                        {
                            "type": "text",
                            "text": instruction
                        }
                    ]
                }
            ],
            "text": {
                "format": {"type": "text"}
            }
        });

        if let Some(store) = &self.request_log_store {
            let request_json = json!({
                "provider": "openai",
                "endpoint": self.responses_url(),
                "body": {
                    "model": self.model,
                    "input": [
                        {
                            "role": "user",
                            "content": [
                                {
                                    "type": "input_audio",
                                    "input_audio": {
                                        "data": "<base64 audio omitted>",
                                        "format": "wav",
                                        "bytes": audio.len(),
                                        "base64_len": audio_base64.len(),
                                    }
                                },
                                {
                                    "type": "text",
                                    "text": instruction
                                }
                            ]
                        }
                    ],
                    "text": {
                        "format": {"type": "text"}
                    }
                }
            });

            store.with_current(|log| {
                log.stt_request_json = Some(request_json);
            });
        }

        let response = self
            .client
            .post(self.responses_url())
            .bearer_auth(&self.api_key)
            .json(&request_body)
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
                "OpenAI GPT-4o API error ({}): {}",
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

        Self::extract_responses_output_text(&result)
    }

    /// Transcribe with an optional prompt.
    ///
    /// This is primarily used by the Settings "Test transcription" UI.
    pub async fn transcribe_with_prompt(
        &self,
        audio: &[u8],
        _format: &AudioFormat,
        prompt: Option<&str>,
    ) -> Result<String, SttError> {
        if self.uses_transcriptions_endpoint() {
            self.transcribe_audio_transcriptions(audio, prompt).await
        } else {
            self.transcribe_responses_audio(audio, prompt).await
        }
    }
}

#[async_trait]
impl SttProvider for OpenAiSttProvider {
    async fn transcribe(&self, audio: &[u8], _format: &AudioFormat) -> Result<String, SttError> {
        self.transcribe_with_prompt(audio, _format, self.default_prompt.as_deref())
            .await
    }

    fn name(&self) -> &'static str {
        "openai"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_creation() {
        let provider = OpenAiSttProvider::new("test-key".to_string(), None, None);
        assert_eq!(provider.name(), "openai");
        assert_eq!(provider.model, "gpt-4o-audio-preview");
    }

    #[test]
    fn test_provider_with_custom_model() {
        let provider =
            OpenAiSttProvider::new("test-key".to_string(), Some("whisper-1".to_string()), None);
        assert_eq!(provider.model, "whisper-1");
    }

    #[test]
    fn test_is_chat_audio_model() {
        let provider = OpenAiSttProvider::new("test-key".to_string(), None, None);
        assert!(!provider.uses_transcriptions_endpoint());

        let provider = OpenAiSttProvider::new(
            "test-key".to_string(),
            Some("gpt-4o-mini-audio-preview".to_string()),
            None,
        );
        assert!(!provider.uses_transcriptions_endpoint());

        let provider =
            OpenAiSttProvider::new("test-key".to_string(), Some("gpt-audio".to_string()), None);
        assert!(!provider.uses_transcriptions_endpoint());

        let provider = OpenAiSttProvider::new(
            "test-key".to_string(),
            Some("gpt-audio-mini".to_string()),
            None,
        );
        assert!(!provider.uses_transcriptions_endpoint());

        let provider =
            OpenAiSttProvider::new("test-key".to_string(), Some("whisper-1".to_string()), None);
        assert!(provider.uses_transcriptions_endpoint());

        let provider = OpenAiSttProvider::new(
            "test-key".to_string(),
            Some("gpt-4o-transcribe".to_string()),
            None,
        );
        assert!(provider.uses_transcriptions_endpoint());
    }
}
