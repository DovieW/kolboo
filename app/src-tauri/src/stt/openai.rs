//! OpenAI STT provider implementation.
//!
//! Supports three modes:
//! - Legacy Whisper API (whisper-1) - uses /v1/audio/transcriptions
//! - Audio chat models (e.g., gpt-4o-audio-preview) - uses /v1/responses with audio input
//! - Realtime transcription (gpt-4o-realtime-transcribe, gpt-4o-mini-realtime-transcribe) -
//!   uses WebSocket wss://api.openai.com/v1/realtime for concurrent streaming.
//!   These are separate model entries that map to OpenAI's transcription models
//!   (gpt-4o-transcribe, gpt-4o-mini-transcribe) within the realtime session.
//!
//! Realtime transcription docs:
//! - Guide: https://platform.openai.com/docs/guides/realtime-transcription
//! - WebSocket: https://platform.openai.com/docs/guides/realtime-websocket
//! - Client events: https://platform.openai.com/docs/api-reference/realtime-client-events

use super::http;
use super::language;
use super::openai_compat;
use super::streaming::{
    connect_ws_split_with_timeout, ws_close_best_effort, ws_next_with_timeout,
    ws_send_json_text_with_closed_handling, PartialTranscript, StreamingSttSession, WsSendOutcome,
};
use super::{AudioFormat, SttError, SttProvider};
use crate::audio_normalization::{
    chunk_size_bytes_for_pcm_s16le, f32_to_pcm_s16le, resample_linear,
};
use crate::request_log::RequestLogStore;
use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde_json::json;
use serde_json::Value as JsonValue;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::{client::IntoClientRequest, Message};

/// Config bundle for the OpenAI realtime streaming background task.
/// This exists to keep the argument count under clippy's threshold.
struct StreamingTaskConfig {
    capture_sample_rate: u32,
    model: String,
    language: Option<String>,
    prompt: Option<String>,
    request_log_store: Option<RequestLogStore>,
}

/// OpenAI STT provider for speech-to-text
pub struct OpenAiSttProvider {
    client: reqwest::Client,
    api_key: String,
    model: String,
    default_prompt: Option<String>,
    default_language: Option<String>,
    api_base_url: String,
    request_log_store: Option<RequestLogStore>,
}

impl OpenAiSttProvider {
    const WHISPER_PROMPT_MAX_CHARS: usize = 224;
    const DEFAULT_OPENAI_API_BASE_URL: &'static str = "https://api.openai.com";
    /// WebSocket timeout for realtime transcription operations.
    const DEFAULT_WS_TRANSCRIPTION_TIMEOUT: Duration = Duration::from_secs(120);
    /// Shorter timeout for waiting after the final commit (audio done).
    /// If VAD already committed everything, the server may not send another
    /// completed event, so we don't want to wait 120s.
    const POST_COMMIT_TIMEOUT: Duration = Duration::from_secs(5);
    /// OpenAI Realtime API requires audio at exactly 24 kHz.
    const REALTIME_SAMPLE_RATE: u32 = 24_000;

    /// Create a new OpenAI STT provider
    ///
    /// # Arguments
    /// * `api_key` - OpenAI API key
    /// * `model` - Model to use:
    ///   - "gpt-4o-audio-preview" (default) - GPT-4o with audio input
    ///   - "gpt-4o-mini-audio-preview" - Smaller/faster GPT-4o audio
    ///   - "whisper-1" - Legacy Whisper API
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn new(
        api_key: String,
        model: Option<String>,
        language: Option<String>,
        default_prompt: Option<String>,
    ) -> Self {
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
            default_language: Self::normalize_language(language),
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
        language: Option<String>,
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
            default_language: Self::normalize_language(language),
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

    fn normalize_language(language: Option<String>) -> Option<String> {
        language::normalize_language_code(language)
    }

    pub fn with_request_log_store(mut self, store: Option<RequestLogStore>) -> Self {
        self.request_log_store = store;
        self
    }

    /// Whether this model uses the OpenAI Realtime Transcription API.
    ///
    /// Only our dedicated realtime model entries (`gpt-4o-realtime-transcribe`,
    /// `gpt-4o-mini-realtime-transcribe`) use the realtime WebSocket path.
    /// The underlying OpenAI transcription model is specified via
    /// [`realtime_transcription_model`](Self::realtime_transcription_model).
    fn supports_realtime_streaming(&self) -> bool {
        self.model.contains("realtime")
    }

    /// Map our internal realtime model name to the OpenAI transcription model
    /// used inside the `session.update` payload.
    ///
    /// - `gpt-4o-realtime-transcribe` → `gpt-4o-transcribe`
    /// - `gpt-4o-mini-realtime-transcribe` → `gpt-4o-mini-transcribe`
    fn realtime_transcription_model(&self) -> String {
        self.model.replace("-realtime", "")
    }

    /// Build the WebSocket URL for the Realtime Transcription API.
    ///
    /// Format: `wss://api.openai.com/v1/realtime?intent=transcription`
    ///
    /// The transcription model (e.g. `gpt-4o-transcribe`) is sent in the
    /// `transcription_session.update` payload, not in the URL.
    fn realtime_ws_url(&self) -> Result<String, SttError> {
        let base = http::trim_base_url(&self.api_base_url);
        let ws_scheme = if base.starts_with("https") || base.starts_with("wss") {
            "wss"
        } else {
            "ws"
        };
        let host = base
            .trim_start_matches("https://")
            .trim_start_matches("http://")
            .trim_start_matches("wss://")
            .trim_start_matches("ws://");
        Ok(format!(
            "{}://{}/v1/realtime?intent=transcription",
            ws_scheme, host,
        ))
    }

    /// Start a concurrent streaming STT session via the OpenAI Realtime API.
    ///
    /// Audio is resampled to 24 kHz (the only rate supported by the Realtime API),
    /// encoded as PCM s16le, and sent as `input_audio_buffer.append` events.
    /// Server-side VAD detects speech turns and emits per-turn transcripts.
    async fn start_streaming_session(
        &self,
        sample_rate: u32,
    ) -> Result<StreamingSttSession, SttError> {
        let ws_url = self.realtime_ws_url()?;

        let mut request = ws_url
            .clone()
            .into_client_request()
            .map_err(|e| SttError::NetworkMessage(format!("WS request build failed: {}", e)))?;

        // OpenAI authenticates via Authorization header.
        let auth_value = format!("Bearer {}", self.api_key);
        request.headers_mut().insert(
            "Authorization",
            auth_value
                .parse()
                .map_err(|e| SttError::Config(format!("Invalid OpenAI API key header: {}", e)))?,
        );
        // Required by the Realtime API.
        request.headers_mut().insert(
            "OpenAI-Beta",
            "realtime=v1"
                .parse()
                .map_err(|e| SttError::Config(format!("Invalid header: {}", e)))?,
        );

        let (ws_write, ws_read) =
            connect_ws_split_with_timeout(request, Self::DEFAULT_WS_TRANSCRIPTION_TIMEOUT).await?;

        let (audio_tx, audio_rx) = mpsc::channel::<Vec<f32>>(1024);
        let (partial_tx, partial_rx) = mpsc::channel::<PartialTranscript>(256);

        let request_log_store = self.request_log_store.clone();
        let model = self.model.clone();
        // The model sent to OpenAI in the session.update payload.
        // For realtime entries this maps e.g. "gpt-4o-realtime-transcribe" → "gpt-4o-transcribe".
        let transcription_model = self.realtime_transcription_model();
        let language = self.default_language.clone();
        let prompt = self.default_prompt.clone();
        let ws_url_for_log = ws_url.to_string();

        // Log the streaming session start.
        if let Some(store) = &request_log_store {
            let request_json = json!({
                "provider": "openai",
                "endpoint": ws_url_for_log,
                "content_type": "websocket-json-streaming",
                "mode": "concurrent",
                "fields": {
                    "model": model,
                    "transcription_model": transcription_model,
                    "language": language,
                    "prompt": prompt,
                },
                "audio": {
                    "encoding": "pcm_s16le_mono",
                    "sample_rate": Self::REALTIME_SAMPLE_RATE,
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
            StreamingTaskConfig {
                capture_sample_rate: sample_rate,
                model: transcription_model,
                language,
                prompt,
                request_log_store,
            },
        ));

        Ok(StreamingSttSession::new(audio_tx, partial_rx, task))
    }

    /// Background task: receives f32 audio from `audio_rx`, resamples to 24 kHz,
    /// sends PCM chunks over the WebSocket, collects per-turn transcripts, and
    /// returns the final concatenated transcript.
    async fn run_streaming_task(
        mut ws_write: super::streaming::WsWrite,
        mut ws_read: super::streaming::WsRead,
        mut audio_rx: mpsc::Receiver<Vec<f32>>,
        partial_tx: mpsc::Sender<PartialTranscript>,
        config: StreamingTaskConfig,
    ) -> Result<String, SttError> {
        let StreamingTaskConfig {
            capture_sample_rate,
            model,
            language,
            prompt,
            request_log_store,
        } = config;
        // Configure the transcription session.
        // The Realtime Transcription API uses a flat structure with
        // `transcription_session.update` (not `session.update`).
        let mut transcription_config = json!({
            "model": model
        });
        if let Some(lang) = &language {
            transcription_config["language"] = serde_json::Value::String(lang.clone());
        }
        if let Some(p) = &prompt {
            transcription_config["prompt"] = serde_json::Value::String(p.clone());
        }

        let session_update = json!({
            "type": "transcription_session.update",
            "session": {
                "input_audio_format": "pcm16",
                "input_audio_transcription": transcription_config,
                "turn_detection": {
                    "type": "server_vad",
                    "threshold": 0.5,
                    "prefix_padding_ms": 300,
                    "silence_duration_ms": 300
                },
                "input_audio_noise_reduction": {
                    "type": "near_field"
                }
            }
        });

        if ws_send_json_text_with_closed_handling(
            &mut ws_write,
            &session_update,
            "OpenAI realtime: send transcription_session.update",
        )
        .await?
            == WsSendOutcome::Closed
        {
            return Err(SttError::NetworkMessage(
                "OpenAI realtime: WebSocket closed before session update completed".to_string(),
            ));
        }

        // Chunk size in 24 kHz PCM s16le bytes (50ms target for snappy partials).
        let target_chunk_bytes =
            chunk_size_bytes_for_pcm_s16le(Self::REALTIME_SAMPLE_RATE, 1, 50, 1_600, 32_768);

        let session_start = std::time::Instant::now();
        let mut pcm_buffer: Vec<u8> = Vec::new();
        let mut num_chunks_sent: usize = 0;
        let mut committed: Vec<String> = Vec::new();
        let mut logged_partials: Vec<JsonValue> = Vec::new();
        let mut current_partial_text = String::new();

        let mut audio_done = false;
        let mut ws_done = false;
        // Track whether we've sent audio since the last VAD-triggered commit.
        // If false when audio closes, there's no point sending a manual commit
        // (the buffer is empty and the server will error).
        let mut has_audio_since_last_commit = false;
        // Whether we sent a manual commit on audio close and are waiting for its
        // completed event.
        let mut awaiting_final_commit = false;

        loop {
            if ws_done {
                break;
            }
            if audio_done {
                // Only the WS branch is active from here.
            }

            // After audio is done, use a shorter timeout so we don't block for
            // 120s if the server has nothing left to send.
            let ws_timeout = if audio_done {
                Self::POST_COMMIT_TIMEOUT
            } else {
                Self::DEFAULT_WS_TRANSCRIPTION_TIMEOUT
            };

            tokio::select! {
                // Read audio chunks from the capture thread.
                audio_chunk = audio_rx.recv(), if !audio_done => {
                    match audio_chunk {
                        Some(f32_samples) => {
                            // Resample to 24 kHz if needed.
                            let resampled = resample_linear(
                                &f32_samples,
                                capture_sample_rate,
                                Self::REALTIME_SAMPLE_RATE,
                            );
                            let pcm = f32_to_pcm_s16le(&resampled);
                            pcm_buffer.extend_from_slice(&pcm);

                            // Send chunks when we've accumulated enough.
                            while pcm_buffer.len() >= target_chunk_bytes {
                                let chunk: Vec<u8> = pcm_buffer.drain(..target_chunk_bytes).collect();
                                let msg = json!({
                                    "type": "input_audio_buffer.append",
                                    "audio": STANDARD.encode(&chunk),
                                });
                                match ws_send_json_text_with_closed_handling(
                                    &mut ws_write,
                                    &msg,
                                    "OpenAI realtime: send audio append",
                                ).await? {
                                    WsSendOutcome::Sent => {
                                        num_chunks_sent += 1;
                                        has_audio_since_last_commit = true;
                                    }
                                    WsSendOutcome::Closed => {
                                        ws_done = true;
                                        break;
                                    }
                                }
                            }
                        }
                        None => {
                            // Audio channel closed (recording stopped).
                            // Send any remaining buffered PCM.
                            if !pcm_buffer.is_empty() {
                                let msg = json!({
                                    "type": "input_audio_buffer.append",
                                    "audio": STANDARD.encode(&pcm_buffer),
                                });
                                match ws_send_json_text_with_closed_handling(
                                    &mut ws_write,
                                    &msg,
                                    "OpenAI realtime: send final audio append",
                                ).await? {
                                    WsSendOutcome::Sent => {
                                        num_chunks_sent += 1;
                                        has_audio_since_last_commit = true;
                                    }
                                    WsSendOutcome::Closed => {
                                        audio_done = true;
                                        ws_done = true;
                                        pcm_buffer.clear();
                                        continue;
                                    }
                                }
                                pcm_buffer.clear();
                            }

                            // Only commit if we've sent audio since the last
                            // server-side VAD commit. Committing an empty buffer
                            // causes the server to return an error.
                            if has_audio_since_last_commit {
                                let commit = json!({ "type": "input_audio_buffer.commit" });
                                match ws_send_json_text_with_closed_handling(
                                    &mut ws_write,
                                    &commit,
                                    "OpenAI realtime: send final commit",
                                ).await? {
                                    WsSendOutcome::Sent => {
                                        awaiting_final_commit = true;
                                        log::debug!("OpenAI realtime: sent final commit (audio done)");
                                    }
                                    WsSendOutcome::Closed => {
                                        ws_done = true;
                                    }
                                }
                            } else {
                                log::debug!("OpenAI realtime: audio done, no pending audio to commit");
                                ws_done = true;
                            }

                            audio_done = true;
                        }
                    }
                }

                // Read WS messages (session events, deltas, completions).
                ws_msg = ws_next_with_timeout(&mut ws_read, ws_timeout), if !ws_done => {
                    match ws_msg {
                        Err(SttError::Timeout) if audio_done => {
                            // Post-commit timeout expired. VAD likely already
                            // committed everything — use what we have.
                            log::debug!(
                                "OpenAI realtime: post-commit timeout, finalizing with {} transcripts",
                                committed.len()
                            );
                            ws_done = true;
                        }
                        Err(e) => return Err(e),
                        Ok(msg) => {
                    match msg {
                        Some(Message::Text(text)) => {
                            let v: JsonValue = serde_json::from_str(&text).map_err(|e| {
                                SttError::Api(format!(
                                    "OpenAI realtime: failed to parse JSON: {} (raw={})",
                                    e, text
                                ))
                            })?;
                            let event_type = v.get("type").and_then(|t| t.as_str()).unwrap_or("");

                            match event_type {
                                "session.created" | "session.updated"
                                | "transcription_session.created" | "transcription_session.updated" => {
                                    log::debug!("OpenAI realtime: {}", event_type);
                                    if event_type == "session.created" || event_type == "transcription_session.created" {
                                        if let Some(store) = &request_log_store {
                                            store.with_current(|log| {
                                                log.info("Realtime transcription session connected");
                                            });
                                        }
                                    }
                                }

                                "conversation.item.input_audio_transcription.delta" => {
                                    if let Some(delta) = v.get("delta").and_then(|d| d.as_str()) {
                                        if !delta.is_empty() {
                                            current_partial_text.push_str(delta);
                                            let elapsed = session_start.elapsed().as_millis() as u64;
                                            logged_partials.push(json!({
                                                "text": current_partial_text,
                                                "delta": delta,
                                                "elapsed_ms": elapsed,
                                            }));
                                            // Build the full text so far: all committed turns
                                            // plus the current in-progress partial.
                                            let full_text = if committed.is_empty() {
                                                current_partial_text.clone()
                                            } else {
                                                let mut s = committed.join(" ");
                                                s.push(' ');
                                                s.push_str(&current_partial_text);
                                                s
                                            };
                                            // Update raw_transcript live so the UI streams it.
                                            if let Some(store) = &request_log_store {
                                                store.with_current(|log| {
                                                    log.raw_transcript = Some(full_text.clone());
                                                });
                                            }
                                            let _ = partial_tx.try_send(PartialTranscript {
                                                text: full_text,
                                                committed_text: None,
                                            });
                                        }
                                    }
                                }

                                "conversation.item.input_audio_transcription.completed" => {
                                    let transcript_text = v.get("transcript")
                                        .and_then(|t| t.as_str())
                                        .unwrap_or("")
                                        .to_string();
                                    let is_nonempty = !transcript_text.trim().is_empty();
                                    if is_nonempty {
                                        committed.push(transcript_text.clone());
                                    }
                                    // Reset partial accumulator for the next turn.
                                    current_partial_text.clear();

                                    // Send a partial with the committed flag so the
                                    // live-output feature can paste this chunk.
                                    if is_nonempty {
                                        let full_text = committed.join(" ");
                                        if let Some(store) = &request_log_store {
                                            store.with_current(|log| {
                                                log.raw_transcript = Some(full_text.clone());
                                            });
                                        }
                                        let _ = partial_tx.try_send(PartialTranscript {
                                            text: full_text,
                                            committed_text: Some(transcript_text),
                                        });
                                    }

                                    // If this was our final commit's completion, we're done.
                                    if audio_done {
                                        ws_done = true;
                                    }
                                }

                                "input_audio_buffer.committed" => {
                                    log::debug!("OpenAI realtime: audio buffer committed");
                                }

                                "input_audio_buffer.speech_started" => {
                                    log::debug!("OpenAI realtime: speech started");
                                    has_audio_since_last_commit = true;
                                }

                                "input_audio_buffer.speech_stopped" => {
                                    log::debug!("OpenAI realtime: speech stopped (VAD)");
                                    // VAD will auto-commit this turn. Reset the flag
                                    // so we know whether new audio arrives after this.
                                    has_audio_since_last_commit = false;
                                }

                                "error" => {
                                    let error_obj = v.get("error");
                                    let msg = error_obj
                                        .and_then(|e| e.get("message"))
                                        .and_then(|m| m.as_str())
                                        .unwrap_or("Unknown error");
                                    let code = error_obj
                                        .and_then(|e| e.get("code"))
                                        .and_then(|c| c.as_str())
                                        .unwrap_or("unknown");

                                    if audio_done {
                                        // After audio is done, errors are typically
                                        // benign (e.g. "buffer is empty" from a
                                        // redundant commit). Log and finish gracefully.
                                        log::warn!(
                                            "OpenAI realtime: non-fatal error after audio done ({}): {}",
                                            code, msg
                                        );
                                        if awaiting_final_commit {
                                            // Our commit was empty — nothing more to wait for.
                                            ws_done = true;
                                        }
                                    } else {
                                        return Err(SttError::Api(format!(
                                            "OpenAI realtime error ({}): {}", code, msg
                                        )));
                                    }
                                }

                                _ => {
                                    // Ignore unknown events.
                                    log::trace!("OpenAI realtime: ignoring event '{}'", event_type);
                                }
                            }
                        }
                        Some(Message::Close(_)) => {
                            ws_done = true;
                        }
                        None => {
                            ws_done = true;
                        }
                        _ => {
                            // Ignore binary/ping/pong.
                        }
                    }
                        }
                    }
                }
            }
        }

        ws_close_best_effort(&mut ws_write, "OpenAI realtime", Duration::from_secs(3)).await;

        // Drop the write half explicitly so the TCP connection tears down cleanly.
        drop(ws_write);

        // Commit any remaining partial text that wasn't finalized by the server.
        let current_partial = current_partial_text.trim().to_string();
        if !current_partial.is_empty() {
            committed.push(current_partial.clone());
            let full_text = committed.join(" ");
            let _ = partial_tx.try_send(PartialTranscript {
                text: full_text,
                committed_text: Some(current_partial),
            });
        }

        let final_text = committed.join(" ");

        if let Some(store) = &request_log_store {
            let total_duration_ms = session_start.elapsed().as_millis() as u64;
            let response_json = json!({
                "committed_transcripts": committed,
                "chunks_sent": num_chunks_sent,
                "mode": "concurrent",
                "session_duration_ms": total_duration_ms,
                "partial_transcripts": logged_partials,
                "capture_sample_rate": capture_sample_rate,
                "target_sample_rate": Self::REALTIME_SAMPLE_RATE,
            });
            store.with_current(|log| {
                log.raw_transcript = Some(final_text.clone());
                log.stt_response_json = Some(response_json);
            });
        }

        log::info!(
            "OpenAI realtime: finalized, {} chars, {} chunks sent",
            final_text.len(),
            num_chunks_sent
        );
        Ok(final_text)
    }

    /// Check if this model should use /v1/audio/transcriptions.
    ///
    /// Per OpenAI docs, `whisper-1` and the `*-transcribe` models are used via the
    /// dedicated transcription endpoint.  The realtime model entries also map to
    /// transcription models but go through the WS path instead.
    fn uses_transcriptions_endpoint(&self) -> bool {
        // Realtime-only models never go through the batch HTTP path.
        if self.supports_realtime_streaming() {
            return false;
        }
        self.model == "whisper-1"
            || self.model.contains("transcribe")
            || self.model.contains("whisper")
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
        let endpoint = self.transcriptions_url();
        let clamped_prompt = self.clamp_prompt_for_model(prompt);
        let language = self.default_language.as_deref();
        openai_compat::transcribe_wav_multipart_openai_compat(
            &self.client,
            "openai",
            "OpenAI Whisper API error",
            &endpoint,
            audio,
            &self.model,
            clamped_prompt.as_deref(),
            language,
            self.request_log_store.as_ref(),
            |rb| rb.bearer_auth(&self.api_key),
            SttError::Network,
        )
        .await
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
        if let Some(language) = self.default_language.as_deref() {
            instruction.push_str("\n\nLanguage: ");
            instruction.push_str(language);
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

        let responses_url = self.responses_url();
        let response = crate::http::with_cloudflare_access_headers_if_target(
            self.client
                .post(&responses_url)
                .bearer_auth(&self.api_key)
                .json(&request_body),
            &responses_url,
        )
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
                "OpenAI-compatible Responses API error (model={}, status={}): {}",
                self.model, status, error_text
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
        if self.supports_realtime_streaming() {
            return Err(SttError::Config(format!(
                "Model '{}' is realtime-only and cannot be used for batch transcription",
                self.model
            )));
        }
        self.transcribe_with_prompt(audio, _format, self.default_prompt.as_deref())
            .await
    }

    fn name(&self) -> &'static str {
        "openai"
    }

    fn supports_streaming(&self) -> bool {
        self.supports_realtime_streaming()
    }

    fn requires_streaming(&self) -> bool {
        self.supports_realtime_streaming()
    }

    async fn start_streaming(&self, sample_rate: u32) -> Result<StreamingSttSession, SttError> {
        self.start_streaming_session(sample_rate).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_creation() {
        let provider = OpenAiSttProvider::new("test-key".to_string(), None, None, None);
        assert_eq!(provider.name(), "openai");
        assert_eq!(provider.model, "gpt-4o-audio-preview");
    }

    #[test]
    fn test_provider_with_custom_model() {
        let provider = OpenAiSttProvider::new(
            "test-key".to_string(),
            Some("whisper-1".to_string()),
            None,
            None,
        );
        assert_eq!(provider.model, "whisper-1");
    }

    #[test]
    fn test_is_chat_audio_model() {
        let provider = OpenAiSttProvider::new("test-key".to_string(), None, None, None);
        assert!(!provider.uses_transcriptions_endpoint());

        let provider = OpenAiSttProvider::new(
            "test-key".to_string(),
            Some("gpt-4o-mini-audio-preview".to_string()),
            None,
            None,
        );
        assert!(!provider.uses_transcriptions_endpoint());

        let provider = OpenAiSttProvider::new(
            "test-key".to_string(),
            Some("gpt-audio".to_string()),
            None,
            None,
        );
        assert!(!provider.uses_transcriptions_endpoint());

        let provider = OpenAiSttProvider::new(
            "test-key".to_string(),
            Some("gpt-audio-mini".to_string()),
            None,
            None,
        );
        assert!(!provider.uses_transcriptions_endpoint());

        // Batch transcription models use the transcriptions endpoint.
        let provider = OpenAiSttProvider::new(
            "test-key".to_string(),
            Some("whisper-1".to_string()),
            None,
            None,
        );
        assert!(provider.uses_transcriptions_endpoint());

        // Whisper-family model ids (used by some OpenAI-compatible gateways/providers)
        // should also use the transcriptions endpoint.
        let provider = OpenAiSttProvider::new(
            "test-key".to_string(),
            Some("whisper-large-v3-turbo".to_string()),
            None,
            None,
        );
        assert!(provider.uses_transcriptions_endpoint());

        let provider = OpenAiSttProvider::new(
            "test-key".to_string(),
            Some("gpt-4o-transcribe".to_string()),
            None,
            None,
        );
        assert!(provider.uses_transcriptions_endpoint());

        // Realtime models do NOT use the transcriptions endpoint
        // (they go through the WebSocket path instead).
        let provider = OpenAiSttProvider::new(
            "test-key".to_string(),
            Some("gpt-4o-realtime-transcribe".to_string()),
            None,
            None,
        );
        assert!(!provider.uses_transcriptions_endpoint());

        let provider = OpenAiSttProvider::new(
            "test-key".to_string(),
            Some("gpt-4o-mini-realtime-transcribe".to_string()),
            None,
            None,
        );
        assert!(!provider.uses_transcriptions_endpoint());
    }

    #[test]
    fn test_supports_realtime_streaming() {
        // Models that support realtime streaming (dedicated realtime entries).
        for model in &[
            "gpt-4o-realtime-transcribe",
            "gpt-4o-mini-realtime-transcribe",
        ] {
            let provider =
                OpenAiSttProvider::new("test-key".to_string(), Some(model.to_string()), None, None);
            assert!(
                provider.supports_realtime_streaming(),
                "Expected {} to support realtime streaming",
                model
            );
            assert!(provider.supports_streaming());
            assert!(provider.requires_streaming());
        }

        // Models that do NOT support realtime streaming (batch-only or non-transcription).
        for model in &[
            "gpt-4o-transcribe",
            "gpt-4o-mini-transcribe",
            "whisper-1",
            "gpt-4o-audio-preview",
            "gpt-4o-mini-audio-preview",
            "gpt-audio",
        ] {
            let provider =
                OpenAiSttProvider::new("test-key".to_string(), Some(model.to_string()), None, None);
            assert!(
                !provider.supports_realtime_streaming(),
                "Expected {} to NOT support realtime streaming",
                model
            );
            assert!(!provider.supports_streaming());
            assert!(!provider.requires_streaming());
        }
    }

    #[test]
    fn test_realtime_transcription_model() {
        let provider = OpenAiSttProvider::new(
            "test-key".to_string(),
            Some("gpt-4o-realtime-transcribe".to_string()),
            None,
            None,
        );
        assert_eq!(provider.realtime_transcription_model(), "gpt-4o-transcribe");

        let provider = OpenAiSttProvider::new(
            "test-key".to_string(),
            Some("gpt-4o-mini-realtime-transcribe".to_string()),
            None,
            None,
        );
        assert_eq!(
            provider.realtime_transcription_model(),
            "gpt-4o-mini-transcribe"
        );
    }

    #[test]
    fn test_realtime_ws_url() {
        // The URL uses intent=transcription, regardless of which
        // transcription model the provider was configured with.
        let provider = OpenAiSttProvider::new(
            "test-key".to_string(),
            Some("gpt-4o-realtime-transcribe".to_string()),
            None,
            None,
        );
        let url = provider.realtime_ws_url().unwrap();
        assert_eq!(url, "wss://api.openai.com/v1/realtime?intent=transcription");

        // Custom base URL
        let provider = provider.with_api_base_url("http://localhost:8080".to_string());
        let url = provider.realtime_ws_url().unwrap();
        assert_eq!(url, "ws://localhost:8080/v1/realtime?intent=transcription");
    }
}
