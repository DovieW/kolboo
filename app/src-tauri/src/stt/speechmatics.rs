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
//! - `AddTranscript` messages are committed immediately (live output).
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

use super::streaming::{
    chunk_size_bytes_for_pcm_s16le, connect_ws_split_with_timeout, f32_to_pcm_s16le,
    is_ws_closed_error, ws_next_with_timeout, PartialTranscript, StreamingSttSession,
};
use super::{language, AudioEncoding, AudioFormat, SttError, SttProvider};
use crate::request_log::RequestLogStore;
use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value as JsonValue};
use std::io::Cursor;
use tokio::sync::mpsc;
use tokio::time::Duration;
use tokio_tungstenite::tungstenite::{client::IntoClientRequest, Message};

/// Speechmatics STT provider for speech-to-text (Realtime WebSocket API).
pub struct SpeechmaticsSttProvider {
    api_key: String,
    operating_point: String,
    language: String,
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
    pub fn new(api_key: String, model: Option<String>, language: Option<String>) -> Self {
        Self {
            api_key,
            operating_point: model.unwrap_or_else(|| "enhanced".to_string()),
            language: Self::normalize_language(language),
            request_log_store: None,
        }
    }

    pub fn with_request_log_store(mut self, store: Option<RequestLogStore>) -> Self {
        self.request_log_store = store;
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

    /// Extract text from a Speechmatics `results` array and return
    /// `(text, has_eos)`.  Unlike `append_transcript_from_results` (which
    /// inserts `\n` on `is_eos` for batch mode), the streaming variant never
    /// inserts line breaks — the caller decides what to do with `has_eos`.
    fn extract_streaming_text(results: &[JsonValue]) -> (String, bool) {
        let mut out = String::new();
        let mut has_eos = false;

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
                has_eos = true;
            }
        }

        (out, has_eos)
    }

    fn chunk_size_bytes(sample_rate: u32, channels: u8) -> usize {
        // Aim for ~100ms chunks, clamp to a reasonable range.
        chunk_size_bytes_for_pcm_s16le(sample_rate, channels, 100, 2_048, 32_768)
    }

    // ── Streaming helpers ──────────────────────────────────────────────

    /// Streaming WS endpoint. Speechmatics uses `eu2.rt` for the realtime
    /// streaming endpoint (distinct from the batch `eu.rt` endpoint).
    const STREAMING_WS_URL: &'static str = "wss://eu2.rt.speechmatics.com/v2/";

    /// Connection/message timeout for streaming sessions.
    const DEFAULT_WS_TIMEOUT: Duration = Duration::from_secs(30);

    /// Timeout when waiting for `EndOfTranscript` after sending `EndOfStream`.
    const POST_EOS_TIMEOUT: Duration = Duration::from_secs(15);

    /// Start a real-time WebSocket streaming session.
    async fn start_streaming_session(
        &self,
        sample_rate: u32,
    ) -> Result<StreamingSttSession, SttError> {
        let mut request = Self::STREAMING_WS_URL
            .into_client_request()
            .map_err(|e| SttError::NetworkMessage(format!("WS request build failed: {}", e)))?;

        let bearer = format!("Bearer {}", self.api_key.trim());
        request.headers_mut().insert(
            "Authorization",
            bearer.parse().map_err(|e| {
                SttError::Config(format!("Invalid Speechmatics auth header: {}", e))
            })?,
        );

        let (ws_write, mut ws_read) =
            connect_ws_split_with_timeout(request, Self::DEFAULT_WS_TIMEOUT).await?;

        // Send StartRecognition.
        let start_msg = json!({
            "message": "StartRecognition",
            "audio_format": {
                "type": "raw",
                "encoding": "pcm_s16le",
                "sample_rate": sample_rate,
            },
            "transcription_config": {
                "language": self.language.clone(),
                "operating_point": self.operating_point_for_api(),
                "max_delay": 1.0,
                "enable_partials": true,
            },
        });

        let mut ws_write = ws_write;
        ws_write
            .send(Message::Text(start_msg.to_string().into()))
            .await
            .map_err(|e| SttError::NetworkMessage(format!("WS send StartRecognition: {}", e)))?;

        // Wait for RecognitionStarted.
        loop {
            let msg = ws_next_with_timeout(&mut ws_read, Self::DEFAULT_WS_TIMEOUT).await?;
            let Some(Message::Text(text)) = msg else {
                continue;
            };
            let Ok(v) = serde_json::from_str::<JsonValue>(&text) else {
                continue;
            };
            match v.get("message").and_then(|m| m.as_str()).unwrap_or("") {
                "RecognitionStarted" => {
                    log::info!("Speechmatics streaming: RecognitionStarted");
                    break;
                }
                "Error" => {
                    return Err(SttError::Api(format!(
                        "Speechmatics streaming error during startup: {}",
                        text
                    )));
                }
                _ => {}
            }
        }

        let (audio_tx, audio_rx) = mpsc::channel::<Vec<f32>>(1024);
        let (partial_tx, partial_rx) = mpsc::channel::<PartialTranscript>(256);

        let request_log_store = self.request_log_store.clone();

        if let Some(store) = &request_log_store {
            let request_json = json!({
                "provider": "speechmatics",
                "endpoint": Self::STREAMING_WS_URL,
                "auth": "Authorization: Bearer <redacted>",
                "mode": "concurrent",
                "start_recognition": {
                    "transcription_config": {
                        "language": self.language.clone(),
                        "operating_point": self.operating_point_for_api(),
                        "max_delay": 1.0,
                        "enable_partials": true,
                    },
                    "audio_format": {
                        "type": "raw",
                        "encoding": "pcm_s16le",
                        "sample_rate": sample_rate,
                    },
                },
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

    /// Background task: receives f32 audio from `audio_rx`, converts to PCM
    /// s16le, sends binary frames over the WebSocket, collects transcripts,
    /// and returns the final concatenated transcript.
    ///
    /// ## Live output commit strategy
    ///
    /// Speechmatics sends two kinds of transcript messages:
    /// - `AddPartialTranscript` → interim text for the current utterance.
    ///   Maps to overlay text updates (not committed for live paste).
    /// - `AddTranscript` → finalized text with punctuation/casing.
    ///   Committed immediately for live output.
    async fn run_streaming_task(
        mut ws_write: super::streaming::WsWrite,
        mut ws_read: super::streaming::WsRead,
        mut audio_rx: mpsc::Receiver<Vec<f32>>,
        partial_tx: mpsc::Sender<PartialTranscript>,
        sample_rate: u32,
        request_log_store: Option<RequestLogStore>,
    ) -> Result<String, SttError> {
        let target_chunk_bytes = chunk_size_bytes_for_pcm_s16le(sample_rate, 1, 100, 1_600, 32_768);

        let session_start = std::time::Instant::now();
        let mut pcm_buffer: Vec<u8> = Vec::new();
        let mut num_chunks_sent: usize = 0;
        let mut last_seq_no: u64 = 0;

        // Committed (finalized) transcript segments + accumulating segment +
        // current interim partial.
        //
        // Speechmatics sends many small `AddTranscript` messages (often just
        // one or two words).  Instead of committing each individually (which
        // would cause word-by-word live output), we accumulate them into
        // `current_segment` and only commit when `is_eos` is true (end of
        // sentence) — giving us sentence-level live output commits.
        let mut committed_segments: Vec<String> = Vec::new();
        let mut current_segment = String::new();
        let mut current_partial = String::new();
        let mut logged_partials: Vec<JsonValue> = Vec::new();

        let mut audio_done = false;
        let mut ws_done = false;

        loop {
            if ws_done {
                break;
            }

            let ws_timeout = if audio_done {
                Self::POST_EOS_TIMEOUT
            } else {
                Self::DEFAULT_WS_TIMEOUT
            };

            tokio::select! {
                audio_chunk = audio_rx.recv(), if !audio_done => {
                    match audio_chunk {
                        Some(f32_samples) => {
                            let pcm = f32_to_pcm_s16le(&f32_samples);
                            pcm_buffer.extend_from_slice(&pcm);

                            while pcm_buffer.len() >= target_chunk_bytes {
                                let chunk: Vec<u8> = pcm_buffer.drain(..target_chunk_bytes).collect();
                                match ws_write.send(Message::Binary(chunk.into())).await {
                                    Ok(()) => { num_chunks_sent += 1; }
                                    Err(e) if is_ws_closed_error(&e) => {
                                        log::warn!("Speechmatics streaming: WS closed while sending audio");
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
                            // Send remaining buffered PCM.
                            if !pcm_buffer.is_empty() {
                                match ws_write.send(Message::Binary(pcm_buffer.clone().into())).await {
                                    Ok(()) => { num_chunks_sent += 1; }
                                    Err(e) if is_ws_closed_error(&e) => {
                                        log::warn!("Speechmatics streaming: WS closed while sending final audio");
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

                            // Signal end-of-stream with last_seq_no so the server
                            // knows all audio has been received.
                            let eos = json!({
                                "message": "EndOfStream",
                                "last_seq_no": last_seq_no,
                            });
                            match ws_write.send(Message::Text(eos.to_string().into())).await {
                                Ok(()) => {}
                                Err(e) if is_ws_closed_error(&e) => {
                                    log::warn!("Speechmatics streaming: WS closed while sending EndOfStream");
                                    ws_done = true;
                                }
                                Err(e) => {
                                    return Err(SttError::NetworkMessage(format!("WS send EndOfStream: {}", e)));
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
                                    "Speechmatics streaming: failed to parse JSON: {} (raw={})",
                                    e, text
                                ))
                            })?;

                            let msg_type = v.get("message").and_then(|m| m.as_str()).unwrap_or("");

                            match msg_type {
                                "AudioAdded" => {
                                    if let Some(seq) = v.get("seq_no").and_then(|s| s.as_u64()) {
                                        last_seq_no = seq;
                                    }
                                }
                                "AddTranscript" => {
                                    // Finalized text — accumulate into current_segment.
                                    // Only commit (for live output) when is_eos signals
                                    // a sentence boundary, so we get sentence-level
                                    // commits instead of word-by-word.
                                    let (text, has_eos) = if let Some(results) = v.get("results").and_then(|r| r.as_array()) {
                                        Self::extract_streaming_text(results)
                                    } else {
                                        (String::new(), false)
                                    };

                                    if !text.is_empty() {
                                        if !current_segment.is_empty() {
                                            current_segment.push(' ');
                                        }
                                        current_segment.push_str(&text);
                                    }
                                    current_partial.clear();

                                    let (full, committed) = if has_eos && !current_segment.is_empty() {
                                        // Sentence boundary — commit the accumulated segment.
                                        let seg = std::mem::take(&mut current_segment);
                                        committed_segments.push(seg.clone());
                                        let full = Self::join_segments(&committed_segments, "");
                                        (full, Some(seg))
                                    } else {
                                        // Mid-sentence — update full text but don't commit.
                                        let full = Self::join_segments(&committed_segments, &current_segment);
                                        (full, None)
                                    };

                                    let elapsed = session_start.elapsed().as_millis() as u64;
                                    logged_partials.push(json!({
                                        "type": "final",
                                        "text": &full,
                                        "is_eos": has_eos,
                                        "elapsed_ms": elapsed,
                                        "committed_segments": committed_segments.len(),
                                    }));

                                    if let Some(store) = &request_log_store {
                                        store.with_current(|log| {
                                            log.raw_transcript = Some(full.clone());
                                        });
                                    }

                                    let _ = partial_tx.try_send(PartialTranscript {
                                        text: full,
                                        committed_text: committed,
                                    });
                                }
                                "AddPartialTranscript" => {
                                    // Interim partial — update overlay only.
                                    let (partial, _) = if let Some(results) = v.get("results").and_then(|r| r.as_array()) {
                                        Self::extract_streaming_text(results)
                                    } else {
                                        (String::new(), false)
                                    };

                                    current_partial = partial;

                                    // Show committed + accumulating + partial.
                                    let full = Self::join_segments_with_accumulating(
                                        &committed_segments, &current_segment, &current_partial,
                                    );

                                    let elapsed = session_start.elapsed().as_millis() as u64;
                                    logged_partials.push(json!({
                                        "type": "partial",
                                        "text": &full,
                                        "elapsed_ms": elapsed,
                                    }));

                                    let _ = partial_tx.try_send(PartialTranscript {
                                        text: full,
                                        committed_text: None,
                                    });
                                }
                                "EndOfTranscript" => {
                                    log::info!("Speechmatics streaming: EndOfTranscript received");
                                    ws_done = true;
                                }
                                "Error" => {
                                    return Err(SttError::Api(format!(
                                        "Speechmatics streaming error: {}",
                                        text
                                    )));
                                }
                                _ => {
                                    // Info, Warning, etc. — ignore.
                                    log::debug!("Speechmatics streaming: {}", msg_type);
                                }
                            }
                        }
                        Ok(Some(Message::Close(frame))) => {
                            log::warn!("Speechmatics streaming: server sent Close frame {:?}", frame);
                            ws_done = true;
                        }
                        Ok(None) => {
                            log::warn!("Speechmatics streaming: WS stream ended (None)");
                            ws_done = true;
                        }
                        Ok(_) => {
                            // Ignore binary/ping/pong.
                        }
                        Err(SttError::Timeout) if audio_done => {
                            log::warn!("Speechmatics streaming: timed out waiting for EndOfTranscript, using accumulated segments");
                            ws_done = true;
                        }
                        Err(SttError::Timeout) => {
                            log::warn!(
                                "Speechmatics streaming: WS read timed out while audio still flowing ({}s)",
                                Self::DEFAULT_WS_TIMEOUT.as_secs()
                            );
                            ws_done = true;
                        }
                        Err(e) => {
                            log::error!("Speechmatics streaming: WS read error: {}", e);
                            return Err(e);
                        }
                    }
                }
            }
        }

        // Best-effort WS close with a short timeout to avoid hangs.
        match tokio::time::timeout(Duration::from_secs(3), ws_write.send(Message::Close(None)))
            .await
        {
            Ok(Ok(())) => {}
            Ok(Err(e)) if is_ws_closed_error(&e) => {}
            Ok(Err(e)) => log::debug!("Speechmatics streaming: WS close send error: {}", e),
            Err(_) => log::debug!("Speechmatics streaming: WS close timed out"),
        }

        // Drop write half explicitly so TCP connection tears down cleanly.
        drop(ws_write);

        // Commit any remaining accumulated segment + partial as one final chunk.
        let mut remaining = current_segment;
        if !current_partial.is_empty() {
            if !remaining.is_empty() {
                remaining.push(' ');
            }
            remaining.push_str(&current_partial);
        }

        if !remaining.is_empty() {
            committed_segments.push(remaining.clone());
            let final_text_with_remaining = Self::join_segments(&committed_segments, "");
            let _ = partial_tx.try_send(PartialTranscript {
                text: final_text_with_remaining,
                committed_text: Some(remaining),
            });
        }

        let final_text = Self::join_segments(&committed_segments, "");

        if let Some(store) = &request_log_store {
            let total_duration_ms = session_start.elapsed().as_millis() as u64;
            let response_json = json!({
                "provider": "speechmatics",
                "mode": "concurrent",
                "committed_segments": committed_segments,
                "chunks_sent": num_chunks_sent,
                "session_duration_ms": total_duration_ms,
                "partial_transcripts": logged_partials,
            });
            store.with_current(|log| {
                log.stt_response_json = Some(response_json);
            });
        }

        log::info!(
            "Speechmatics streaming: finalized, {} chars, {} segments, {} chunks sent",
            final_text.len(),
            committed_segments.len(),
            num_chunks_sent
        );
        Ok(final_text)
    }

    /// Join committed segments with an optional current partial, separated by
    /// spaces.
    fn join_segments(committed: &[String], current_partial: &str) -> String {
        let mut parts: Vec<&str> = committed
            .iter()
            .map(|s| s.as_str())
            .filter(|s| !s.is_empty())
            .collect();

        let trimmed = current_partial.trim();
        if !trimmed.is_empty() {
            parts.push(trimmed);
        }

        parts.join(" ")
    }

    /// Join committed segments + the accumulating (not-yet-committed) segment +
    /// the interim partial, all separated by spaces.
    fn join_segments_with_accumulating(
        committed: &[String],
        accumulating: &str,
        current_partial: &str,
    ) -> String {
        let mut parts: Vec<&str> = committed
            .iter()
            .map(|s| s.as_str())
            .filter(|s| !s.is_empty())
            .collect();

        let acc = accumulating.trim();
        if !acc.is_empty() {
            parts.push(acc);
        }

        let trimmed = current_partial.trim();
        if !trimmed.is_empty() {
            parts.push(trimmed);
        }

        parts.join(" ")
    }

    async fn transcribe_ws(
        &self,
        pcm: &[u8],
        sample_rate: u32,
        channels: u8,
    ) -> Result<(String, JsonValue), SttError> {
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
                "language": self.language.clone(),
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

        // Stream audio concurrently with receiving server messages.
        //
        // The old approach sent one chunk then waited for AudioAdded before
        // sending the next — causing ~210 sequential round-trips for a 21s
        // file, easily exceeding the 15s pipeline timeout.
        //
        // Now we fire-and-forget all audio as fast as the WS can accept it
        // while concurrently draining server messages.  After all audio is
        // sent we send EndOfStream and drain until EndOfTranscript.
        let mut last_seq_no: u64 = 0;
        let chunk_size = Self::chunk_size_bytes(sample_rate, channels);
        let mut transcript = String::new();
        let mut received_for_log: Vec<JsonValue> = Vec::new();

        let mut chunks = pcm.chunks(chunk_size);
        let mut audio_done = false;

        // Phase 1: send audio + receive concurrently via tokio::select!
        loop {
            tokio::select! {
                // Send the next chunk if audio is not yet fully sent.
                send_result = async {
                    if let Some(chunk) = chunks.next() {
                        ws_tx
                            .send(Message::Binary(chunk.to_vec().into()))
                            .await
                            .map(|()| true)
                    } else {
                        // No more chunks — send EndOfStream.
                        let eos_msg = json!({
                            "message": "EndOfStream",
                            "last_seq_no": last_seq_no,
                        });
                        ws_tx
                            .send(Message::Text(eos_msg.to_string().into()))
                            .await
                            .map(|()| false)
                    }
                }, if !audio_done => {
                    match send_result {
                        Ok(true) => {
                            // Chunk sent, continue.
                        }
                        Ok(false) => {
                            // EndOfStream sent.
                            audio_done = true;
                        }
                        Err(e) => {
                            return Err(SttError::NetworkMessage(format!(
                                "Speechmatics WS send failed: {}", e
                            )));
                        }
                    }
                }

                // Always drain server messages.
                msg = ws_rx.next() => {
                    let Some(msg) = msg else {
                        if audio_done {
                            break;
                        }
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
                                    }
                                    "AddTranscript" => {
                                        if let Some(results) =
                                            v.get("results").and_then(|r| r.as_array())
                                        {
                                            Self::append_transcript_from_results(
                                                &mut transcript,
                                                results,
                                            );
                                        }
                                        received_for_log.push(v);
                                    }
                                    "EndOfTranscript" => {
                                        received_for_log.push(v);
                                        // Done — all audio processed.
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
                        }
                        Message::Close(frame) => {
                            if audio_done {
                                break;
                            }
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
        }

        // Phase 2: if we broke out after sending EndOfStream but before
        // EndOfTranscript, drain remaining messages.
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
            return Err(SttError::Config(
                "Speechmatics API key is missing".to_string(),
            ));
        }

        let (pcm, sample_rate, channels) = Self::decode_to_pcm_s16le(audio, format)?;

        if let Some(store) = &self.request_log_store {
            let request_json = json!({
                "provider": "speechmatics",
                "endpoint": Self::DEFAULT_WS_URL,
                "auth": "Authorization: Bearer <redacted>",
                "start_recognition": {
                    "transcription_config": {
                        "language": self.language.clone(),
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

    #[test]
    fn test_join_segments() {
        assert_eq!(
            SpeechmaticsSttProvider::join_segments(&["hello".into()], "world"),
            "hello world"
        );
        assert_eq!(
            SpeechmaticsSttProvider::join_segments(&["hello".into()], ""),
            "hello"
        );
        assert_eq!(
            SpeechmaticsSttProvider::join_segments(&[], "partial"),
            "partial"
        );
        assert_eq!(SpeechmaticsSttProvider::join_segments(&[], ""), "");
    }

    #[test]
    fn test_join_segments_with_accumulating() {
        // committed + accumulating + partial
        assert_eq!(
            SpeechmaticsSttProvider::join_segments_with_accumulating(
                &["Hello.".into()],
                "The quick",
                "brown fox"
            ),
            "Hello. The quick brown fox"
        );
        // No accumulating text
        assert_eq!(
            SpeechmaticsSttProvider::join_segments_with_accumulating(
                &["Hello.".into()],
                "",
                "world"
            ),
            "Hello. world"
        );
        // No partial
        assert_eq!(
            SpeechmaticsSttProvider::join_segments_with_accumulating(
                &["Hello.".into()],
                "world",
                ""
            ),
            "Hello. world"
        );
    }

    #[test]
    fn test_extract_streaming_text() {
        let results = serde_json::json!([
            {
                "type": "word",
                "alternatives": [{"content": "Hello"}],
                "is_eos": false,
            },
            {
                "type": "punctuation",
                "alternatives": [{"content": "."}],
                "is_eos": true,
            }
        ]);
        let (text, has_eos) =
            SpeechmaticsSttProvider::extract_streaming_text(results.as_array().unwrap());
        assert_eq!(text, "Hello.");
        assert!(has_eos);
        // No newlines — unlike batch mode.
        assert!(!text.contains('\n'));
    }

    #[test]
    fn test_extract_streaming_text_no_eos() {
        let results = serde_json::json!([
            {
                "type": "word",
                "alternatives": [{"content": "the"}],
                "is_eos": false,
            },
            {
                "type": "word",
                "alternatives": [{"content": "quick"}],
                "is_eos": false,
            }
        ]);
        let (text, has_eos) =
            SpeechmaticsSttProvider::extract_streaming_text(results.as_array().unwrap());
        assert_eq!(text, "the quick");
        assert!(!has_eos);
    }
}
