//! Speechmatics STT provider implementation.
//!
//! Supports both batch (one-shot) and real-time streaming transcription, both
//! WebSocket-based:
//!
//! **Batch** (one-shot):
//! - Send full audio after recording → get transcript back.
//! - `supports_streaming() = true`, `requires_streaming() = false` so the
//!   pipeline can fall back to batch if the streaming session fails.
//!
//! **Streaming** (concurrent):
//! - Audio chunks are streamed during recording for near-instant results.
//! - `AddTranscript` messages are accumulated into sentence-sized live-output commits.
//! - `AddPartialTranscript` messages update the overlay in real-time.
//!
//! API reference:
//! - <https://docs.speechmatics.com/api-ref/realtime-transcription-websocket>
//!
//! Implementation notes:
//! - Auth: `Authorization: Bearer <api_key>` header on WS handshake.
//! - Audio: PCM s16le at the capture sample rate (server accepts arbitrary rates).
//! - Protocol: `StartRecognition` → binary audio → `EndOfStream` → `EndOfTranscript`
//! - Partials: `AddPartialTranscript` (interim), `AddTranscript` (final/committed).

mod realtime;

use super::{language, AudioEncoding, AudioFormat, SttError, SttProvider};
use crate::request_log::RequestLogStore;
use crate::settings::ProxySettings;
use crate::stt::StreamingSttSession;
use async_trait::async_trait;
use serde_json::Value as JsonValue;
use std::io::Cursor;

/// Speechmatics STT provider for speech-to-text (Realtime WebSocket API).
pub struct SpeechmaticsSttProvider {
    api_key: String,
    operating_point: String,
    language: String,
    request_log_store: Option<RequestLogStore>,
    proxy_settings: ProxySettings,
}

impl SpeechmaticsSttProvider {
    /// Create a new Speechmatics provider.
    ///
    /// `model` maps to Speechmatics `transcription_config.operating_point`.
    /// Supported values:
    /// - "enhanced" (default)
    /// - "standard"
    pub fn new(api_key: String, model: Option<String>, language: Option<String>) -> Self {
        Self {
            api_key,
            operating_point: model.unwrap_or_else(|| "enhanced".to_string()),
            language: Self::normalize_language(language),
            request_log_store: None,
            proxy_settings: ProxySettings::default(),
        }
    }

    pub fn with_request_log_store(mut self, store: Option<RequestLogStore>) -> Self {
        self.request_log_store = store;
        self
    }

    pub fn with_proxy_settings(mut self, proxy_settings: ProxySettings) -> Self {
        self.proxy_settings = proxy_settings;
        self
    }

    fn operating_point_for_api(&self) -> &str {
        let m = self.operating_point.trim().to_lowercase();
        if m == "standard" {
            "standard"
        } else {
            "enhanced"
        }
    }

    fn normalize_language(language: Option<String>) -> String {
        language::normalize_language_setting(language).unwrap_or_else(|| "en".to_string())
    }

    fn decode_to_pcm_s16le(
        audio: &[u8],
        format: &AudioFormat,
    ) -> Result<(Vec<u8>, u32, u8), SttError> {
        match format.encoding {
            AudioEncoding::Pcm16 => {
                // Assume the caller already provided raw PCM (little-endian i16).
                Ok((audio.to_vec(), format.sample_rate, format.channels))
            }
            AudioEncoding::Wav => {
                let cursor = Cursor::new(audio);
                let mut reader = hound::WavReader::new(cursor)
                    .map_err(|e| SttError::Audio(format!("Failed to read WAV: {}", e)))?;

                let spec = reader.spec();
                let sample_rate = spec.sample_rate;
                let channels = spec.channels as u8;

                let mut pcm = Vec::new();

                match (spec.sample_format, spec.bits_per_sample) {
                    (hound::SampleFormat::Int, 16) => {
                        for s in reader.samples::<i16>() {
                            let s = s.map_err(|e| {
                                SttError::Audio(format!("WAV sample read failed: {}", e))
                            })?;
                            pcm.extend_from_slice(&s.to_le_bytes());
                        }
                    }
                    (hound::SampleFormat::Int, 32) => {
                        // Scale i32 samples down to i16.
                        for s in reader.samples::<i32>() {
                            let s = s.map_err(|e| {
                                SttError::Audio(format!("WAV sample read failed: {}", e))
                            })?;
                            let s16 = (s >> 16) as i16;
                            pcm.extend_from_slice(&s16.to_le_bytes());
                        }
                    }
                    (hound::SampleFormat::Float, 32) => {
                        for s in reader.samples::<f32>() {
                            let s = s.map_err(|e| {
                                SttError::Audio(format!("WAV sample read failed: {}", e))
                            })?;
                            let clipped = s.clamp(-1.0, 1.0);
                            let s16 = (clipped * i16::MAX as f32).round() as i16;
                            pcm.extend_from_slice(&s16.to_le_bytes());
                        }
                    }
                    other => {
                        return Err(SttError::Audio(format!(
                            "Unsupported WAV format: {:?} bits_per_sample={} (expected 16-bit PCM)",
                            other.0, other.1
                        )));
                    }
                }

                Ok((pcm, sample_rate, channels))
            }
        }
    }

    fn append_transcript_from_results(out: &mut String, results: &[JsonValue]) {
        for r in results {
            let t = r.get("type").and_then(|v| v.as_str()).unwrap_or("");
            let content = r
                .get("alternatives")
                .and_then(|a| a.as_array())
                .and_then(|a| a.first())
                .and_then(|alt| alt.get("content"))
                .and_then(|c| c.as_str());

            let Some(content) = content else {
                continue;
            };

            if t == "word" {
                if !out.is_empty() {
                    out.push(' ');
                }
                out.push_str(content);
            } else {
                // punctuation/end-of-utterance etc.
                out.push_str(content);
            }

            if r.get("is_eos").and_then(|v| v.as_bool()).unwrap_or(false) {
                out.push('\n');
            }
        }
    }

    /// Start a real-time WebSocket streaming session.
    async fn start_streaming_session(
        &self,
        sample_rate: u32,
    ) -> Result<StreamingSttSession, SttError> {
        realtime::start_streaming_session(self, sample_rate).await
    }

    async fn transcribe_ws(
        &self,
        pcm: &[u8],
        sample_rate: u32,
        channels: u8,
    ) -> Result<(String, JsonValue), SttError> {
        realtime::transcribe_ws(self, pcm, sample_rate, channels).await
    }
}

#[async_trait]
impl SttProvider for SpeechmaticsSttProvider {
    async fn transcribe(&self, audio: &[u8], format: &AudioFormat) -> Result<String, SttError> {
        if self.api_key.trim().is_empty() {
            return Err(SttError::Config(
                "Speechmatics API key is missing".to_string(),
            ));
        }

        let (pcm, sample_rate, channels) = Self::decode_to_pcm_s16le(audio, format)?;

        let (text, response_json) = self.transcribe_ws(&pcm, sample_rate, channels).await?;

        if let Some(store) = &self.request_log_store {
            store.with_current(|log| {
                log.stt_response_json = Some(response_json);
            });
        }

        Ok(text)
    }

    fn name(&self) -> &'static str {
        "speechmatics"
    }

    fn supports_streaming(&self) -> bool {
        true
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
        let provider = SpeechmaticsSttProvider::new("test-key".to_string(), None, None);
        assert_eq!(provider.name(), "speechmatics");
        assert_eq!(provider.operating_point, "enhanced");
        assert!(provider.supports_streaming());
        assert!(!provider.requires_streaming());
    }

    #[test]
    fn test_provider_with_custom_model() {
        let provider = SpeechmaticsSttProvider::new(
            "test-key".to_string(),
            Some("standard".to_string()),
            None,
        );
        assert_eq!(provider.operating_point, "standard");
        assert!(provider.supports_streaming());
    }
}
