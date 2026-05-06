//! Fireworks STT provider implementation.
//!
//! Fireworks exposes Whisper transcription endpoints on separate hosts:
//! - whisper-v3:       https://audio-prod.api.fireworks.ai/v1/audio/transcriptions
//! - whisper-v3-turbo: https://audio-turbo.api.fireworks.ai/v1/audio/transcriptions
//!
//! Fireworks also supports real-time streaming transcription via WebSocket:
//! - fireworks-asr-large: wss://audio-streaming.api.fireworks.ai/v1/audio/transcriptions/streaming
//! - fireworks-asr-v2:    wss://audio-streaming.api.fireworks.ai/v1/audio/transcriptions/streaming
//!
//! Streaming protocol:
//! - Send binary frames: PCM 16-bit LE, 16kHz, mono, 50–400ms chunks
//! - Receive JSON with `segments` array (`id`, `text`)
//! - End-of-stream: send JSON `{"checkpoint_id": "final"}`
//! - Server replies with `{"checkpoint_id": "final"}` when done

use super::streaming::{
    connect_ws_split_with_timeout, is_ws_closed_error, ws_next_with_timeout, PartialTranscript,
    StreamingSttSession,
};
use super::{http, language, openai_compat};
use super::{AudioFormat, SttError, SttProvider};
use crate::audio_normalization::{
    chunk_size_bytes_for_pcm_s16le, f32_to_pcm_s16le, resample_linear,
};
use crate::request_log::RequestLogStore;
use async_trait::async_trait;
use futures_util::SinkExt;
use serde_json::json;
use serde_json::Value as JsonValue;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;

/// Fireworks STT provider for speech-to-text (Whisper v3 / v3-turbo / ASR streaming).
pub struct FireworksSttProvider {
    client: reqwest::Client,
    api_key: String,
    model: String,
    default_prompt: Option<String>,
    default_language: Option<String>,
    api_base_url: Option<String>,
    request_log_store: Option<RequestLogStore>,
}

/// Models that use the real-time WebSocket streaming API.
const STREAMING_MODELS: &[&str] = &["fireworks-asr-large", "fireworks-asr-v2"];

impl FireworksSttProvider {
    /// Required sample rate for the Fireworks streaming endpoint (16 kHz).
    const STREAMING_SAMPLE_RATE: u32 = 16_000;
    /// Default connect / read timeout for WebSocket operations.
    const DEFAULT_WS_TIMEOUT: Duration = Duration::from_secs(30);
    /// Timeout for waiting for final transcript after sending checkpoint.
    const POST_CHECKPOINT_TIMEOUT: Duration = Duration::from_secs(15);
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

    /// Whether this model uses the real-time WebSocket streaming API.
    fn is_streaming_model(&self) -> bool {
        STREAMING_MODELS.iter().any(|m| *m == self.model)
    }

    /// Build the WebSocket URL for real-time streaming transcription.
    fn streaming_ws_url(&self) -> Result<String, SttError> {
        let base = if let Some(base_url) = &self.api_base_url {
            // Test override: convert http(s) to ws(s)
            let ws_base = if base_url.starts_with("https") {
                base_url.replace("https", "wss")
            } else if base_url.starts_with("http") {
                base_url.replace("http", "ws")
            } else {
                base_url.clone()
            };
            format!(
                "{}/v1/audio/transcriptions/streaming",
                ws_base.trim_end_matches('/')
            )
        } else {
            "wss://audio-streaming.api.fireworks.ai/v1/audio/transcriptions/streaming".to_string()
        };

        // Append language as query param if set.
        if let Some(lang) = &self.default_language {
            Ok(format!("{}?language={}", base, lang))
        } else {
            Ok(base)
        }
    }

    /// Start a real-time WebSocket streaming session.
    async fn start_streaming_session(
        &self,
        sample_rate: u32,
    ) -> Result<StreamingSttSession, SttError> {
        let ws_url = self.streaming_ws_url()?;

        let mut request = ws_url
            .clone()
            .into_client_request()
            .map_err(|e| SttError::NetworkMessage(format!("WS request build failed: {}", e)))?;

        request.headers_mut().insert(
            "Authorization",
            self.api_key.parse().map_err(|e| {
                SttError::Config(format!("Invalid Fireworks API key header: {}", e))
            })?,
        );

        let (ws_write, ws_read) =
            connect_ws_split_with_timeout(request, Self::DEFAULT_WS_TIMEOUT).await?;

        let (audio_tx, audio_rx) = mpsc::channel::<Vec<f32>>(1024);
        let (partial_tx, partial_rx) = mpsc::channel::<PartialTranscript>(256);

        let request_log_store = self.request_log_store.clone();
        let model = self.model.clone();
        let language = self.default_language.clone();

        if let Some(store) = &request_log_store {
            let request_json = json!({
                "provider": "fireworks",
                "endpoint": ws_url,
                "content_type": "websocket-binary-streaming",
                "mode": "concurrent",
                "fields": {
                    "model": model,
                    "language": language,
                },
                "audio": {
                    "encoding": "pcm_s16le_mono",
                    "sample_rate": Self::STREAMING_SAMPLE_RATE,
                    "capture_sample_rate": sample_rate,
                }
            });
            store.with_current(|log| {
                log.stt_request_json = Some(request_json);
            });
        }

        let task = tokio::spawn(Self::run_streaming_task(
            ws_write,
            ws_read,
            audio_rx,
            partial_tx,
            sample_rate,
            request_log_store,
        ));

        Ok(StreamingSttSession::new(audio_tx, partial_rx, task))
    }

    /// Background task: receives f32 audio from `audio_rx`, resamples to 16 kHz,
    /// sends PCM binary frames over the WebSocket, collects segment-based
    /// transcripts, and returns the final concatenated transcript.
    ///
    /// ## Live output commit strategy
    ///
    /// Fireworks streaming returns segments that can be overwritten. The server
    /// may include ALL segments (starting from ID 0) in every update rather than
    /// omitting finalised ones. To support live output (pasting text while still
    /// recording), we track per-segment **text stability**:
    ///
    /// - For each segment, count how many consecutive updates its text has been
    ///   unchanged.
    /// - When a leading (lowest-ID uncommitted) segment has been stable for
    ///   `SEGMENT_STABILITY_THRESHOLD` consecutive updates, commit it for live
    ///   paste.
    /// - **Time-based fallback**: if a segment has existed for longer than
    ///   `SEGMENT_AGE_COMMIT_SECS` and has at least 1 consecutive unchanged
    ///   update, treat it as stable enough to commit. This ensures live output
    ///   works for short recordings where continuous speech keeps resetting
    ///   stability counters.
    /// - When the session ends, all remaining uncommitted segments are committed
    ///   as one final chunk.
    async fn run_streaming_task(
        mut ws_write: super::streaming::WsWrite,
        mut ws_read: super::streaming::WsRead,
        mut audio_rx: mpsc::Receiver<Vec<f32>>,
        partial_tx: mpsc::Sender<PartialTranscript>,
        capture_sample_rate: u32,
        request_log_store: Option<RequestLogStore>,
    ) -> Result<String, SttError> {
        /// Number of consecutive unchanged updates before a segment is
        /// considered stable enough to commit for live output.
        const SEGMENT_STABILITY_THRESHOLD: usize = 3;

        /// Age (in seconds) after which a segment can be committed with
        /// relaxed stability (≥ 1 consecutive unchanged update instead of
        /// the full `SEGMENT_STABILITY_THRESHOLD`). This ensures live output
        /// works during short recordings where continuous speech prevents
        /// segments from ever reaching the full stability count.
        const SEGMENT_AGE_COMMIT_SECS: f64 = 1.5;

        // 100ms target chunks for responsive partials.
        let target_chunk_bytes =
            chunk_size_bytes_for_pcm_s16le(Self::STREAMING_SAMPLE_RATE, 1, 100, 1_600, 32_768);

        let session_start = std::time::Instant::now();
        let mut pcm_buffer: Vec<u8> = Vec::new();
        let mut num_chunks_sent: usize = 0;
        // Segment state: segments can be updated/overwritten by the server.
        let mut segments: Vec<String> = Vec::new();
        let mut logged_partials: Vec<JsonValue> = Vec::new();
        // Per-segment stability: how many consecutive updates each segment's
        // text has been unchanged.
        let mut segment_stable_count: Vec<usize> = Vec::new();
        // Per-segment first-seen timestamp for time-based commit fallback.
        let mut segment_first_seen: Vec<std::time::Instant> = Vec::new();
        // Live-output commit tracking: segments [0, committed_up_to) have been
        // pasted already.
        let mut committed_up_to: usize = 0;

        let mut audio_done = false;
        let mut ws_done = false;

        loop {
            // Break when WS is done — either normal (final checkpoint received
            // after audio finished) or early (server closed while audio still
            // streaming).
            if ws_done {
                break;
            }
            if audio_done {
                // Only the WS branch is active from here.
            }

            let ws_timeout = if audio_done {
                Self::POST_CHECKPOINT_TIMEOUT
            } else {
                Self::DEFAULT_WS_TIMEOUT
            };

            tokio::select! {
                audio_chunk = audio_rx.recv(), if !audio_done => {
                    match audio_chunk {
                        Some(f32_samples) => {
                            // Resample to 16 kHz if needed.
                            let resampled = resample_linear(
                                &f32_samples,
                                capture_sample_rate,
                                Self::STREAMING_SAMPLE_RATE,
                            );
                            let pcm = f32_to_pcm_s16le(&resampled);
                            pcm_buffer.extend_from_slice(&pcm);

                            // Send binary chunks when we've accumulated enough.
                            while pcm_buffer.len() >= target_chunk_bytes {
                                let chunk: Vec<u8> = pcm_buffer.drain(..target_chunk_bytes).collect();
                                match ws_write.send(Message::Binary(chunk.into())).await {
                                    Ok(()) => { num_chunks_sent += 1; }
                                    Err(e) if is_ws_closed_error(&e) => {
                                        log::warn!("Fireworks streaming: WS closed while sending audio, finishing early");
                                        ws_done = true;
                                        break;
                                    }
                                    Err(e) => {
                                        return Err(SttError::NetworkMessage(format!("WS send failed: {}", e)));
                                    }
                                }
                            }
                        }
                        None => {
                            // Audio channel closed (recording stopped).
                            // Send any remaining buffered PCM.
                            if !pcm_buffer.is_empty() {
                                match ws_write.send(Message::Binary(pcm_buffer.clone().into())).await {
                                    Ok(()) => { num_chunks_sent += 1; }
                                    Err(e) if is_ws_closed_error(&e) => {
                                        log::warn!("Fireworks streaming: WS closed while sending final audio");
                                        audio_done = true;
                                        ws_done = true;
                                        pcm_buffer.clear();
                                        continue;
                                    }
                                    Err(e) => {
                                        return Err(SttError::NetworkMessage(format!("WS send failed: {}", e)));
                                    }
                                }
                                pcm_buffer.clear();
                            }

                            // Signal end-of-stream with a checkpoint message.
                            let checkpoint = json!({"checkpoint_id": "final"});
                            match ws_write.send(Message::Text(checkpoint.to_string().into())).await {
                                Ok(()) => {}
                                Err(e) if is_ws_closed_error(&e) => {
                                    log::warn!("Fireworks streaming: WS closed while sending checkpoint");
                                    ws_done = true;
                                }
                                Err(e) => {
                                    return Err(SttError::NetworkMessage(format!("WS send checkpoint failed: {}", e)));
                                }
                            }

                            audio_done = true;
                        }
                    }
                }

                ws_msg = ws_next_with_timeout(&mut ws_read, ws_timeout), if !ws_done => {
                    match ws_msg {
                        Ok(Some(Message::Text(text))) => {
                            let v: JsonValue = serde_json::from_str(&text).map_err(|e| {
                                SttError::Api(format!(
                                    "Fireworks streaming: failed to parse JSON: {} (raw={})",
                                    e, text
                                ))
                            })?;

                            // Check for final checkpoint.
                            if v.get("checkpoint_id").and_then(|c| c.as_str()) == Some("final") {
                                ws_done = true;
                                continue;
                            }

                            // Check for error.
                            if let Some(err) = v.get("error").and_then(|e| e.as_str()) {
                                return Err(SttError::Api(format!(
                                    "Fireworks streaming error: {}", err
                                )));
                            }

                            // Process segments and update stability tracking.
                            if let Some(segs) = v.get("segments").and_then(|s| s.as_array()) {
                                // Track which segment IDs appear in this update.
                                let mut updated_ids = std::collections::HashSet::new();

                                for seg in segs {
                                    let id = Self::parse_segment_id(seg);
                                    let text = seg.get("text")
                                        .and_then(|t| t.as_str())
                                        .unwrap_or("")
                                        .to_string();

                                    if let Some(id) = id {
                                        updated_ids.insert(id);

                                        // Extend vectors if needed.
                                        if id >= segments.len() {
                                            let new_len = id + 1;
                                            segments.resize(new_len, String::new());
                                            segment_stable_count.resize(new_len, 0);
                                            segment_first_seen.resize(new_len, std::time::Instant::now());
                                        }

                                        // Check if text actually changed.
                                        if segments[id] != text {
                                            segments[id] = text;
                                            segment_stable_count[id] = 0;
                                        } else {
                                            segment_stable_count[id] += 1;
                                        }
                                    }
                                }

                                // Segments NOT mentioned in this update are
                                // implicitly unchanged → increment their stability.
                                for i in committed_up_to..segments.len() {
                                    if !updated_ids.contains(&i) && !segments[i].is_empty() {
                                        segment_stable_count[i] += 1;
                                    }
                                }

                                // Build full text from all segments.
                                let full_text = Self::join_segments(&segments);

                                let elapsed = session_start.elapsed().as_millis() as u64;
                                logged_partials.push(json!({
                                    "text": &full_text,
                                    "elapsed_ms": elapsed,
                                    "num_segments": segments.len(),
                                }));

                                if let Some(store) = &request_log_store {
                                    store.with_current(|log| {
                                        log.raw_transcript = Some(full_text.clone());
                                    });
                                }

                                // Commit leading segments that have stabilised
                                // (either full stability or time-based fallback).
                                let prev_committed = committed_up_to;
                                while committed_up_to < segments.len() {
                                    if segments[committed_up_to].is_empty() {
                                        break;
                                    }
                                    let stability = segment_stable_count[committed_up_to];
                                    let age_secs = segment_first_seen[committed_up_to]
                                        .elapsed()
                                        .as_secs_f64();
                                    let fully_stable = stability >= SEGMENT_STABILITY_THRESHOLD;
                                    let age_stable = age_secs >= SEGMENT_AGE_COMMIT_SECS
                                        && stability >= 1;
                                    if fully_stable || age_stable {
                                        log::debug!(
                                            "Fireworks streaming: committing segment {} \
                                             (stability={}, age={:.1}s, reason={})",
                                            committed_up_to,
                                            stability,
                                            age_secs,
                                            if fully_stable { "stable" } else { "age" },
                                        );
                                        committed_up_to += 1;
                                    } else {
                                        break;
                                    }
                                }

                                let committed_text = if committed_up_to > prev_committed {
                                    let new_text = Self::join_segments(
                                        &segments[prev_committed..committed_up_to],
                                    );
                                    if new_text.is_empty() { None } else { Some(new_text) }
                                } else {
                                    None
                                };

                                let _ = partial_tx.try_send(PartialTranscript {
                                    text: full_text,
                                    committed_text,
                                });
                            }
                        }
                        Ok(Some(Message::Close(_))) | Ok(None) => {
                            ws_done = true;
                        }
                        Ok(_) => {
                            // Ignore binary/ping/pong.
                        }
                        Err(SttError::Timeout) if audio_done => {
                            // Timed out waiting for final checkpoint — use what we have.
                            log::warn!("Fireworks streaming: timed out waiting for final checkpoint, using accumulated segments");
                            ws_done = true;
                        }
                        Err(e) => {
                            return Err(e);
                        }
                    }
                }
            }
        }

        // Best-effort close.
        let _ = ws_write.send(Message::Close(None)).await;

        let final_text = Self::join_segments(&segments);

        // Commit any remaining uncommitted segments for live output before
        // the partial channel closes.
        if committed_up_to < segments.len() {
            let remaining = Self::join_segments(&segments[committed_up_to..]);
            if !remaining.is_empty() {
                let _ = partial_tx.try_send(PartialTranscript {
                    text: final_text.clone(),
                    committed_text: Some(remaining),
                });
            }
        }

        if let Some(store) = &request_log_store {
            let total_duration_ms = session_start.elapsed().as_millis() as u64;
            let response_json = json!({
                "segments": segments,
                "chunks_sent": num_chunks_sent,
                "mode": "concurrent",
                "session_duration_ms": total_duration_ms,
                "partial_transcripts": logged_partials,
            });
            store.with_current(|log| {
                log.stt_response_json = Some(response_json);
            });
        }

        log::info!(
            "Fireworks streaming: finalized, {} chars, {} segments, {} chunks sent",
            final_text.len(),
            segments.len(),
            num_chunks_sent
        );
        Ok(final_text)
    }

    /// Parse a segment ID from a JSON segment object.
    ///
    /// Handles both numeric IDs (`0`, `1`) and string IDs (`"seg_0"`, `"0"`).
    fn parse_segment_id(seg: &JsonValue) -> Option<usize> {
        seg.get("id").and_then(|i| {
            if let Some(n) = i.as_u64() {
                Some(n as usize)
            } else if let Some(s) = i.as_str() {
                s.strip_prefix("seg_")
                    .or(Some(s))
                    .and_then(|n| n.parse::<usize>().ok())
            } else {
                None
            }
        })
    }

    /// Join non-empty segment texts with spaces.
    fn join_segments(segments: &[String]) -> String {
        segments
            .iter()
            .filter(|s| !s.is_empty())
            .cloned()
            .collect::<Vec<_>>()
            .join(" ")
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

        if self.is_streaming_model() {
            return Err(SttError::Config(format!(
                "Model '{}' only supports real-time streaming, not batch transcription",
                self.model
            )));
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

    #[test]
    fn test_streaming_model_detection() {
        let batch = FireworksSttProvider::new(
            "test".to_string(),
            Some("whisper-v3".to_string()),
            None,
            None,
        );
        assert!(!batch.is_streaming_model());
        assert!(!batch.supports_streaming());

        let asr_large = FireworksSttProvider::new(
            "test".to_string(),
            Some("fireworks-asr-large".to_string()),
            None,
            None,
        );
        assert!(asr_large.is_streaming_model());
        assert!(asr_large.supports_streaming());
        assert!(asr_large.requires_streaming());

        let asr_v2 = FireworksSttProvider::new(
            "test".to_string(),
            Some("fireworks-asr-v2".to_string()),
            None,
            None,
        );
        assert!(asr_v2.is_streaming_model());
        assert!(asr_v2.supports_streaming());
        assert!(asr_v2.requires_streaming());
    }

    #[test]
    fn test_streaming_ws_url_no_language() {
        let provider = FireworksSttProvider::new(
            "test".to_string(),
            Some("fireworks-asr-large".to_string()),
            None,
            None,
        );
        let url = provider.streaming_ws_url().unwrap();
        assert_eq!(
            url,
            "wss://audio-streaming.api.fireworks.ai/v1/audio/transcriptions/streaming"
        );
    }

    #[test]
    fn test_streaming_ws_url_with_language() {
        let provider = FireworksSttProvider::new(
            "test".to_string(),
            Some("fireworks-asr-v2".to_string()),
            Some("en".to_string()),
            None,
        );
        let url = provider.streaming_ws_url().unwrap();
        assert_eq!(
            url,
            "wss://audio-streaming.api.fireworks.ai/v1/audio/transcriptions/streaming?language=en"
        );
    }

    #[test]
    fn test_streaming_ws_url_with_base_override() {
        let provider = FireworksSttProvider::new(
            "test".to_string(),
            Some("fireworks-asr-large".to_string()),
            None,
            None,
        )
        .with_api_base_url("http://localhost:8080".to_string());
        let url = provider.streaming_ws_url().unwrap();
        assert_eq!(url, "ws://localhost:8080/v1/audio/transcriptions/streaming");
    }

    #[test]
    fn test_parse_segment_id_numeric() {
        let seg = json!({"id": 3, "text": "hello"});
        assert_eq!(FireworksSttProvider::parse_segment_id(&seg), Some(3));
    }

    #[test]
    fn test_parse_segment_id_string() {
        let seg = json!({"id": "seg_2", "text": "hello"});
        assert_eq!(FireworksSttProvider::parse_segment_id(&seg), Some(2));

        let seg2 = json!({"id": "5", "text": "world"});
        assert_eq!(FireworksSttProvider::parse_segment_id(&seg2), Some(5));
    }

    #[test]
    fn test_parse_segment_id_missing() {
        let seg = json!({"text": "hello"});
        assert_eq!(FireworksSttProvider::parse_segment_id(&seg), None);
    }

    #[test]
    fn test_join_segments() {
        let segs = vec!["Hello".to_string(), "".to_string(), "world".to_string()];
        assert_eq!(FireworksSttProvider::join_segments(&segs), "Hello world");
    }

    #[test]
    fn test_join_segments_empty() {
        let segs: Vec<String> = vec![];
        assert_eq!(FireworksSttProvider::join_segments(&segs), "");
    }
}
