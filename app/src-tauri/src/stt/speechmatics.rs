//! Speechmatics Realtime STT provider implementation.
//!
//! Speechmatics Realtime transcription is WebSocket-based.
//! API reference:
//! - https://docs.speechmatics.com/api-ref/realtime-transcription-websocket
//!
//! Implementation notes:
//! - Uses the server-to-server auth scheme: `Authorization: Bearer <api_key>` in the WebSocket
//!   handshake request.
//! - Uses `StartRecognition` with `audio_format.type = raw` and `encoding = pcm_s16le`.
//! - Streams audio as binary frames (`AddAudio`).
//! - Collects `AddTranscript` final messages until `EndOfTranscript`.

use super::{AudioEncoding, AudioFormat, SttError, SttProvider};
use async_trait::async_trait;
use crate::request_log::RequestLogStore;
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value as JsonValue};
use std::io::Cursor;
use tokio_tungstenite::tungstenite::{client::IntoClientRequest, Message};

/// Speechmatics STT provider for speech-to-text (Realtime WebSocket API).
pub struct SpeechmaticsSttProvider {
    api_key: String,
    operating_point: String,
    request_log_store: Option<RequestLogStore>,
}

impl SpeechmaticsSttProvider {
    // Speechmatics Realtime WebSocket endpoint (EU region).
    const DEFAULT_WS_URL: &'static str = "wss://eu.rt.speechmatics.com/v2/";

    /// Create a new Speechmatics provider.
    ///
    /// `model` maps to Speechmatics `transcription_config.operating_point`.
    /// Supported values:
    /// - "enhanced" (default)
    /// - "standard"
    pub fn new(api_key: String, model: Option<String>) -> Self {
        Self {
            api_key,
            operating_point: model.unwrap_or_else(|| "enhanced".to_string()),
            request_log_store: None,
        }
    }

    pub fn with_request_log_store(mut self, store: Option<RequestLogStore>) -> Self {
        self.request_log_store = store;
        self
    }

    fn operating_point_for_api(&self) -> &str {
        let m = self.operating_point.trim().to_lowercase();
        if m == "standard" { "standard" } else { "enhanced" }
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
                            let s = s.map_err(|e| SttError::Audio(format!("WAV sample read failed: {}", e)))?;
                            pcm.extend_from_slice(&s.to_le_bytes());
                        }
                    }
                    (hound::SampleFormat::Int, 32) => {
                        // Scale i32 samples down to i16.
                        for s in reader.samples::<i32>() {
                            let s = s.map_err(|e| SttError::Audio(format!("WAV sample read failed: {}", e)))?;
                            let s16 = (s >> 16) as i16;
                            pcm.extend_from_slice(&s16.to_le_bytes());
                        }
                    }
                    (hound::SampleFormat::Float, 32) => {
                        for s in reader.samples::<f32>() {
                            let s = s.map_err(|e| SttError::Audio(format!("WAV sample read failed: {}", e)))?;
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

    fn chunk_size_bytes(sample_rate: u32, channels: u8) -> usize {
        // Aim for ~100ms chunks, clamp to a reasonable range.
        let bytes_per_100ms = (sample_rate as usize)
            .saturating_mul(channels.max(1) as usize)
            .saturating_mul(2)
            / 10;

        bytes_per_100ms.clamp(2_048, 32_768)
    }

    async fn transcribe_ws(&self, pcm: &[u8], sample_rate: u32, channels: u8) -> Result<(String, JsonValue), SttError> {
        let mut req = Self::DEFAULT_WS_URL
            .into_client_request()
            .map_err(|e| SttError::NetworkMessage(format!("Invalid websocket URL: {}", e)))?;

        let bearer = format!("Bearer {}", self.api_key.trim());
        req.headers_mut().insert(
            "Authorization",
            bearer
                .parse()
                .map_err(|e| SttError::Config(format!("Invalid Authorization header: {}", e)))?,
        );

        let (ws_stream, _resp) = tokio_tungstenite::connect_async(req)
            .await
            .map_err(|e| SttError::NetworkMessage(e.to_string()))?;

        let (mut ws_tx, mut ws_rx) = ws_stream.split();

        let start_msg = json!({
            "message": "StartRecognition",
            "audio_format": {
                "type": "raw",
                "encoding": "pcm_s16le",
                "sample_rate": sample_rate,
            },
            "transcription_config": {
                // TODO: consider making language configurable.
                "language": "en",
                "operating_point": self.operating_point_for_api(),
                // Default: balance latency/accuracy.
                "max_delay": 1.0,
                // Partials aren't needed for the app's one-shot STT interface.
                "enable_partials": false,
            },
            // Multi-channel support is not used by the current app pipeline.
        });

        ws_tx
            .send(Message::Text(start_msg.to_string().into()))
            .await
            .map_err(|e| SttError::NetworkMessage(e.to_string()))?;

        // Wait for RecognitionStarted before sending audio.
        loop {
            let Some(msg) = ws_rx.next().await else {
                return Err(SttError::NetworkMessage(
                    "Speechmatics websocket closed during startup".to_string(),
                ));
            };
            let msg = msg.map_err(|e| SttError::NetworkMessage(e.to_string()))?;

            let Message::Text(text) = msg else {
                continue;
            };

            let Ok(v) = serde_json::from_str::<JsonValue>(&text) else {
                continue;
            };

            let m = v.get("message").and_then(|x| x.as_str()).unwrap_or("");
            match m {
                "RecognitionStarted" => break,
                "Error" => {
                    return Err(SttError::Api(text.to_string()));
                }
                _ => {
                    // Info/Warning/etc.
                }
            }
        }

        // Stream audio.
        let mut last_seq_no: u64 = 0;
        let chunk_size = Self::chunk_size_bytes(sample_rate, channels);
        let mut transcript = String::new();
        let mut received_for_log: Vec<JsonValue> = Vec::new();

        for chunk in pcm.chunks(chunk_size) {
            ws_tx
                .send(Message::Binary(chunk.to_vec().into()))
                .await
                .map_err(|e| SttError::NetworkMessage(e.to_string()))?;

            // Consume server messages until we get an AudioAdded ack for this chunk.
            loop {
                let Some(msg) = ws_rx.next().await else {
                    return Err(SttError::NetworkMessage(
                        "Speechmatics websocket closed while sending audio".to_string(),
                    ));
                };
                let msg = msg.map_err(|e| SttError::NetworkMessage(e.to_string()))?;

                match msg {
                    Message::Text(text) => {
                        if let Ok(v) = serde_json::from_str::<JsonValue>(&text) {
                            let m = v.get("message").and_then(|x| x.as_str()).unwrap_or("");

                            match m {
                                "AudioAdded" => {
                                    if let Some(seq) = v.get("seq_no").and_then(|x| x.as_u64()) {
                                        last_seq_no = seq;
                                    }
                                    break;
                                }
                                "AddTranscript" => {
                                    if let Some(results) = v.get("results").and_then(|r| r.as_array()) {
                                        Self::append_transcript_from_results(&mut transcript, results);
                                    }
                                    received_for_log.push(v);
                                }
                                "Error" => {
                                    return Err(SttError::Api(text.to_string()));
                                }
                                _ => {
                                    // Ignore (Info/Warning/partials/etc.)
                                    // But keep a small sample for debugging.
                                    if received_for_log.len() < 20 {
                                        received_for_log.push(v);
                                    }
                                }
                            }
                        }
                    }
                    Message::Close(frame) => {
                        return Err(SttError::NetworkMessage(format!(
                            "Speechmatics websocket closed: {:?}",
                            frame
                        )));
                    }
                    _ => {
                        // Ignore binary/ping/pong
                    }
                }
            }
        }

        // Signal end-of-stream.
        let eos_msg = json!({
            "message": "EndOfStream",
            "last_seq_no": last_seq_no,
        });
        ws_tx
            .send(Message::Text(eos_msg.to_string().into()))
            .await
            .map_err(|e| SttError::NetworkMessage(e.to_string()))?;

        // Read until EndOfTranscript.
        loop {
            let Some(msg) = ws_rx.next().await else {
                break;
            };
            let msg = msg.map_err(|e| SttError::NetworkMessage(e.to_string()))?;

            match msg {
                Message::Text(text) => {
                    let Ok(v) = serde_json::from_str::<JsonValue>(&text) else {
                        continue;
                    };
                    let m = v.get("message").and_then(|x| x.as_str()).unwrap_or("");
                    match m {
                        "AddTranscript" => {
                            if let Some(results) = v.get("results").and_then(|r| r.as_array()) {
                                Self::append_transcript_from_results(&mut transcript, results);
                            }
                            received_for_log.push(v);
                        }
                        "EndOfTranscript" => {
                            received_for_log.push(v);
                            break;
                        }
                        "Error" => {
                            return Err(SttError::Api(text.to_string()));
                        }
                        _ => {
                            if received_for_log.len() < 100 {
                                received_for_log.push(v);
                            }
                        }
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }

        let response_json = json!({
            "provider": "speechmatics",
            "operating_point": self.operating_point_for_api(),
            "audio_format": {
                "sample_rate": sample_rate,
                "channels": channels,
                "encoding": "pcm_s16le",
                "bytes": pcm.len(),
            },
            "transcript": transcript,
            "received_messages": received_for_log,
        });

        Ok((transcript, response_json))
    }
}

#[async_trait]
impl SttProvider for SpeechmaticsSttProvider {
    async fn transcribe(&self, audio: &[u8], format: &AudioFormat) -> Result<String, SttError> {
        if self.api_key.trim().is_empty() {
            return Err(SttError::Config("Speechmatics API key is missing".to_string()));
        }

        let (pcm, sample_rate, channels) = Self::decode_to_pcm_s16le(audio, format)?;

        if let Some(store) = &self.request_log_store {
            let request_json = json!({
                "provider": "speechmatics",
                "endpoint": Self::DEFAULT_WS_URL,
                "auth": "Authorization: Bearer <redacted>",
                "start_recognition": {
                    "transcription_config": {
                        "language": "en",
                        "operating_point": self.operating_point_for_api(),
                        "max_delay": 1.0,
                        "enable_partials": false,
                    },
                    "audio_format": {
                        "type": "raw",
                        "encoding": "pcm_s16le",
                        "sample_rate": sample_rate,
                        "channels": channels,
                    },
                },
                "audio": {
                    "input_encoding": format!("{:?}", format.encoding),
                    "bytes": audio.len(),
                    "decoded_pcm_bytes": pcm.len(),
                    "chunk_bytes": Self::chunk_size_bytes(sample_rate, channels),
                }
            });

            store.with_current(|log| {
                log.stt_request_json = Some(request_json);
            });
        }

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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_creation_defaults() {
        let provider = SpeechmaticsSttProvider::new("test-key".to_string(), None);
        assert_eq!(provider.name(), "speechmatics");
        assert_eq!(provider.operating_point, "enhanced");
    }

    #[test]
    fn test_provider_with_custom_model() {
        let provider = SpeechmaticsSttProvider::new("test-key".to_string(), Some("standard".to_string()));
        assert_eq!(provider.operating_point, "standard");
    }
}
