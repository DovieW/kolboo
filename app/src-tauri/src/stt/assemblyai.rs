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

use super::http;
use super::language;
use super::streaming::{
    connect_ws_split_with_timeout, is_ws_closed_error, ws_next_with_timeout, PartialTranscript,
    StreamingSttSession,
};
use super::{AudioFormat, SttError, SttProvider};
use crate::audio_normalization::{chunk_size_bytes_for_pcm_s16le, f32_to_pcm_s16le};
use crate::request_log::RequestLogStore;
use async_trait::async_trait;
use futures_util::SinkExt;
use serde::{Deserialize, Serialize};
use serde_json::json;
use serde_json::Value as JsonValue;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;

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
}

/// Models that use the real-time WebSocket streaming API.
const STREAMING_MODELS: &[&str] = &[
    "universal-streaming-english",
    "universal-streaming-multilingual",
];

impl AssemblyAiSttProvider {
    const DEFAULT_API_BASE_URL: &'static str = "https://api.assemblyai.com";
    /// Default streaming WebSocket endpoint.
    const DEFAULT_STREAMING_WS_URL: &'static str = "wss://streaming.assemblyai.com/v3/ws";
    /// Default connect / read timeout for WebSocket operations.
    const DEFAULT_WS_TIMEOUT: Duration = Duration::from_secs(30);
    /// Timeout for waiting for Termination message after sending Terminate.
    const POST_TERMINATE_TIMEOUT: Duration = Duration::from_secs(15);

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

    // ─────────────────────────────────────────────────────────────────────────
    // Streaming helpers
    // ─────────────────────────────────────────────────────────────────────────

    /// Whether this model uses the real-time WebSocket streaming API.
    fn is_streaming_model(&self) -> bool {
        STREAMING_MODELS.iter().any(|m| *m == self.model)
    }

    /// Build the WebSocket URL for real-time streaming transcription.
    ///
    /// `sample_rate` is the capture sample rate (e.g. 48 000 Hz). The v3 API
    /// accepts arbitrary integer sample rates so we send audio at the native
    /// capture rate and let the server resample internally (better quality
    /// than client-side linear interpolation).
    fn streaming_ws_url(&self, sample_rate: u32) -> Result<String, SttError> {
        let base = if self.api_base_url != Self::DEFAULT_API_BASE_URL {
            // Test override: convert http(s) to ws(s).
            let trimmed = self.api_base_url_trimmed();
            let ws_base = if trimmed.starts_with("https") {
                trimmed.replacen("https", "wss", 1)
            } else if trimmed.starts_with("http") {
                trimmed.replacen("http", "ws", 1)
            } else {
                trimmed.to_string()
            };
            format!("{}/v3/ws", ws_base)
        } else {
            Self::DEFAULT_STREAMING_WS_URL.to_string()
        };

        let mut params = vec![
            format!("sample_rate={}", sample_rate),
            "encoding=pcm_s16le".to_string(),
            "format_turns=true".to_string(),
        ];

        // Auth via query param (required by AssemblyAI v3 WebSocket API;
        // the Authorization header alone is not sufficient for the WS
        // handshake on some server configurations).
        params.push(format!("token={}", self.api_key));

        // The v3 streaming API only accepts `language=en` or `language=multi`;
        // the v2 batch API accepts locale codes like `en_us`, `fr`, `de`, etc.
        // Map the stored language code to the streaming-compatible value.
        if let Some(lang) = &self.language_code {
            let streaming_lang = match lang.as_str() {
                // Any English locale → "en"
                l if l.starts_with("en") => "en",
                // Non-English explicit language → force "multi" for multilingual model
                _ if self.model == "universal-streaming-multilingual" => "multi",
                // For the English model with a non-English language (unusual),
                // skip the language param and let the server default.
                _ => "",
            };
            if !streaming_lang.is_empty() {
                params.push(format!("language={}", streaming_lang));
            }
        } else if self.model == "universal-streaming-english" {
            // English model with no explicit language → default to "en".
            params.push("language=en".to_string());
        }

        if self.language_detection {
            params.push("language_detection=true".to_string());
        }

        Ok(format!("{}?{}", base, params.join("&")))
    }

    /// Start a real-time WebSocket streaming session.
    async fn start_streaming_session(
        &self,
        sample_rate: u32,
    ) -> Result<StreamingSttSession, SttError> {
        let ws_url = self.streaming_ws_url(sample_rate)?;

        let mut request = ws_url
            .clone()
            .into_client_request()
            .map_err(|e| SttError::NetworkMessage(format!("WS request build failed: {}", e)))?;

        request.headers_mut().insert(
            "Authorization",
            self.api_key.parse().map_err(|e| {
                SttError::Config(format!("Invalid AssemblyAI API key header: {}", e))
            })?,
        );

        let (ws_write, ws_read) =
            connect_ws_split_with_timeout(request, Self::DEFAULT_WS_TIMEOUT).await?;

        let (audio_tx, audio_rx) = mpsc::channel::<Vec<f32>>(1024);
        let (partial_tx, partial_rx) = mpsc::channel::<PartialTranscript>(256);

        let request_log_store = self.request_log_store.clone();
        let model = self.model.clone();
        let language = self.language_code.clone();

        if let Some(store) = &request_log_store {
            // Redact the API key from the logged URL.
            let safe_url = if let Some(idx) = ws_url.find("token=") {
                let end = ws_url[idx..].find('&').map_or(ws_url.len(), |i| idx + i);
                format!("{}token=REDACTED{}", &ws_url[..idx], &ws_url[end..])
            } else {
                ws_url.clone()
            };
            let request_json = json!({
                "provider": "assemblyai",
                "endpoint": safe_url,
                "content_type": "websocket-binary-streaming",
                "mode": "concurrent",
                "fields": {
                    "model": model,
                    "language": language,
                    "language_detection": self.language_detection,
                },
                "audio": {
                    "encoding": "pcm_s16le",
                    "sample_rate": sample_rate,
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

    /// Background task: receives f32 audio from `audio_rx`, converts to PCM s16le,
    /// sends binary frames over the WebSocket, collects Turn-based transcripts,
    /// and returns the final concatenated transcript.
    ///
    /// ## Live output commit strategy
    ///
    /// AssemblyAI streaming uses a turn-based protocol. Each Turn message has a
    /// `turn_is_formatted` flag:
    /// - `false` → interim partial for the current utterance (overlay text update)
    /// - `true`  → finalized turn with punctuation/casing (commit for live paste)
    ///
    /// This is much cleaner than stability-based approaches because the server
    /// explicitly tells us when a turn is done.
    async fn run_streaming_task(
        mut ws_write: super::streaming::WsWrite,
        mut ws_read: super::streaming::WsRead,
        mut audio_rx: mpsc::Receiver<Vec<f32>>,
        partial_tx: mpsc::Sender<PartialTranscript>,
        sample_rate: u32,
        request_log_store: Option<RequestLogStore>,
    ) -> Result<String, SttError> {
        // 100ms target chunks for responsive partials.
        let target_chunk_bytes = chunk_size_bytes_for_pcm_s16le(sample_rate, 1, 100, 1_600, 32_768);

        let session_start = std::time::Instant::now();
        let mut pcm_buffer: Vec<u8> = Vec::new();
        let mut num_chunks_sent: usize = 0;
        // Turn accumulation: completed (formatted) turns + current partial.
        let mut committed_turns: Vec<String> = Vec::new();
        let mut current_partial = String::new();
        let mut logged_partials: Vec<JsonValue> = Vec::new();

        let mut audio_done = false;
        let mut ws_done = false;

        loop {
            // Break when WS is done — either normal (Termination received after
            // audio finished) or early (server closed while audio still streaming).
            if ws_done {
                break;
            }
            if audio_done {
                // Only the WS branch is active from here.
            }

            let ws_timeout = if audio_done {
                Self::POST_TERMINATE_TIMEOUT
            } else {
                Self::DEFAULT_WS_TIMEOUT
            };

            tokio::select! {
                audio_chunk = audio_rx.recv(), if !audio_done => {
                    match audio_chunk {
                        Some(f32_samples) => {
                            // Convert f32 mono samples to PCM s16le at the native
                            // capture sample rate.  The server knows the rate from
                            // the `sample_rate` query param and resamples internally.
                            let pcm = f32_to_pcm_s16le(&f32_samples);
                            pcm_buffer.extend_from_slice(&pcm);

                            // Send binary chunks when we've accumulated enough.
                            while pcm_buffer.len() >= target_chunk_bytes {
                                let chunk: Vec<u8> = pcm_buffer.drain(..target_chunk_bytes).collect();
                                match ws_write.send(Message::Binary(chunk.into())).await {
                                    Ok(()) => { num_chunks_sent += 1; }
                                    Err(e) if is_ws_closed_error(&e) => {
                                        log::warn!("AssemblyAI streaming: WS closed while sending audio, finishing early");
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
                                        log::warn!("AssemblyAI streaming: WS closed while sending final audio");
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

                            // Signal end-of-session.
                            let terminate = json!({"type": "Terminate"});
                            match ws_write.send(Message::Text(terminate.to_string().into())).await {
                                Ok(()) => {}
                                Err(e) if is_ws_closed_error(&e) => {
                                    log::warn!("AssemblyAI streaming: WS closed while sending Terminate");
                                    ws_done = true;
                                }
                                Err(e) => {
                                    return Err(SttError::NetworkMessage(format!("WS send terminate failed: {}", e)));
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
                                    "AssemblyAI streaming: failed to parse JSON: {} (raw={})",
                                    e, text
                                ))
                            })?;

                            let msg_type = v.get("type").and_then(|t| t.as_str()).unwrap_or("");

                            match msg_type {
                                "Begin" => {
                                    log::info!(
                                        "AssemblyAI streaming session started (id={})",
                                        v.get("id").and_then(|i| i.as_str()).unwrap_or("unknown")
                                    );
                                }
                                "Turn" => {
                                    let transcript = v.get("transcript")
                                        .and_then(|t| t.as_str())
                                        .unwrap_or("")
                                        .to_string();
                                    let is_formatted = v.get("turn_is_formatted")
                                        .and_then(|f| f.as_bool())
                                        .unwrap_or(false);

                                    let (full_text, committed_text) = if is_formatted {
                                        // Finalized turn — commit for live output.
                                        if !transcript.is_empty() {
                                            committed_turns.push(transcript.clone());
                                        }
                                        current_partial.clear();

                                        let full = Self::join_turn_texts(&committed_turns, "");
                                        let committed = if transcript.is_empty() { None } else { Some(transcript) };
                                        (full, committed)
                                    } else {
                                        // Interim partial — update overlay text only.
                                        current_partial = transcript;

                                        let full = Self::join_turn_texts(&committed_turns, &current_partial);
                                        (full, None)
                                    };

                                    let elapsed = session_start.elapsed().as_millis() as u64;
                                    logged_partials.push(json!({
                                        "text": &full_text,
                                        "is_formatted": is_formatted,
                                        "elapsed_ms": elapsed,
                                        "committed_turns": committed_turns.len(),
                                    }));

                                    if let Some(store) = &request_log_store {
                                        store.with_current(|log| {
                                            log.raw_transcript = Some(full_text.clone());
                                        });
                                    }

                                    let _ = partial_tx.try_send(PartialTranscript {
                                        text: full_text,
                                        committed_text,
                                    });
                                }
                                "Termination" => {
                                    log::info!(
                                        "AssemblyAI streaming session terminated (audio={:.1}s, session={:.1}s)",
                                        v.get("audio_duration_seconds").and_then(|d| d.as_f64()).unwrap_or(0.0),
                                        v.get("session_duration_seconds").and_then(|d| d.as_f64()).unwrap_or(0.0),
                                    );
                                    ws_done = true;
                                }
                                other => {
                                    log::debug!("AssemblyAI streaming: unknown message type: {}", other);
                                }
                            }
                        }
                        Ok(Some(Message::Close(frame))) => {
                            log::warn!(
                                "AssemblyAI streaming: server sent Close frame {:?}",
                                frame
                            );
                            ws_done = true;
                        }
                        Ok(None) => {
                            log::warn!("AssemblyAI streaming: WS stream ended (None)");
                            ws_done = true;
                        }
                        Ok(_) => {
                            // Ignore binary/ping/pong.
                        }
                        Err(SttError::Timeout) if audio_done => {
                            log::warn!("AssemblyAI streaming: timed out waiting for Termination, using accumulated turns");
                            ws_done = true;
                        }
                        Err(SttError::Timeout) => {
                            log::warn!(
                                "AssemblyAI streaming: WS read timed out while audio still flowing ({}s)",
                                Self::DEFAULT_WS_TIMEOUT.as_secs()
                            );
                            ws_done = true;
                        }
                        Err(e) => {
                            log::error!("AssemblyAI streaming: WS read error: {}", e);
                            return Err(e);
                        }
                    }
                }
            }
        }

        // Best-effort close with a short timeout to avoid hanging on WS teardown.
        match tokio::time::timeout(Duration::from_secs(3), ws_write.send(Message::Close(None)))
            .await
        {
            Ok(Ok(())) => {}
            Ok(Err(e)) if is_ws_closed_error(&e) => {}
            Ok(Err(e)) => log::debug!("AssemblyAI streaming: WS close send error: {}", e),
            Err(_) => log::debug!("AssemblyAI streaming: WS close timed out"),
        }

        // Drop the write half explicitly so the TCP connection tears down
        // before we build the final response.
        drop(ws_write);

        let final_text = Self::join_turn_texts(&committed_turns, &current_partial);

        // If there's any uncommitted partial text, commit it as a final chunk.
        if !current_partial.is_empty() {
            let _ = partial_tx.try_send(PartialTranscript {
                text: final_text.clone(),
                committed_text: Some(current_partial),
            });
        }

        if let Some(store) = &request_log_store {
            let total_duration_ms = session_start.elapsed().as_millis() as u64;
            let response_json = json!({
                "committed_turns": committed_turns,
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
            "AssemblyAI streaming: finalized, {} chars, {} turns, {} chunks sent",
            final_text.len(),
            committed_turns.len(),
            num_chunks_sent
        );
        Ok(final_text)
    }

    /// Join committed turn texts with an optional current partial, separated by spaces.
    fn join_turn_texts(committed: &[String], current_partial: &str) -> String {
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

        let eng = AssemblyAiSttProvider::new(
            "key".to_string(),
            Some("universal-streaming-english".to_string()),
            None,
        );
        assert!(eng.is_streaming_model());
        assert!(eng.supports_streaming());
        assert!(eng.requires_streaming());

        let multi = AssemblyAiSttProvider::new(
            "key".to_string(),
            Some("universal-streaming-multilingual".to_string()),
            None,
        );
        assert!(multi.is_streaming_model());
        assert!(multi.supports_streaming());
        assert!(multi.requires_streaming());
    }

    #[test]
    fn test_streaming_ws_url_english_no_language() {
        let provider = AssemblyAiSttProvider::new(
            "my-api-key".to_string(),
            Some("universal-streaming-english".to_string()),
            None,
        );
        let url = provider.streaming_ws_url(48000).unwrap();
        assert!(url.starts_with("wss://streaming.assemblyai.com/v3/ws?"));
        assert!(url.contains("sample_rate=48000"));
        assert!(url.contains("encoding=pcm_s16le"));
        assert!(url.contains("format_turns=true"));
        assert!(url.contains("token=my-api-key"));
        // English model defaults to language=en (not en_us — streaming only accepts en/multi).
        assert!(url.contains("language=en"));
        assert!(!url.contains("language=en_us"));
        // No language → language_detection=true via normalize_language.
        assert!(url.contains("language_detection=true"));
    }

    #[test]
    fn test_streaming_ws_url_multilingual_no_language() {
        let provider = AssemblyAiSttProvider::new(
            "key".to_string(),
            Some("universal-streaming-multilingual".to_string()),
            None,
        );
        let url = provider.streaming_ws_url(48000).unwrap();
        assert!(url.starts_with("wss://streaming.assemblyai.com/v3/ws?"));
        // Multilingual model without explicit language → no language param but
        // language_detection=true.
        assert!(!url.contains("language=en_us"));
        assert!(url.contains("language_detection=true"));
    }

    #[test]
    fn test_streaming_ws_url_with_explicit_language() {
        let provider = AssemblyAiSttProvider::new(
            "key".to_string(),
            Some("universal-streaming-multilingual".to_string()),
            Some("fr".to_string()),
        );
        let url = provider.streaming_ws_url(48000).unwrap();
        // Non-English language on multilingual model → mapped to "multi" for streaming.
        assert!(url.contains("language=multi"));
        assert!(!url.contains("language=fr"));
        // Explicit language → no language detection.
        assert!(!url.contains("language_detection=true"));
    }

    #[test]
    fn test_streaming_ws_url_base_override() {
        let provider = AssemblyAiSttProvider::new(
            "key".to_string(),
            Some("universal-streaming-english".to_string()),
            None,
        )
        .with_api_base_url("http://localhost:9090".to_string());
        let url = provider.streaming_ws_url(16000).unwrap();
        assert!(url.starts_with("ws://localhost:9090/v3/ws?"));
    }

    #[test]
    fn test_join_turn_texts_committed_only() {
        let turns = vec!["Hello.".to_string(), "How are you?".to_string()];
        assert_eq!(
            AssemblyAiSttProvider::join_turn_texts(&turns, ""),
            "Hello. How are you?"
        );
    }

    #[test]
    fn test_join_turn_texts_with_partial() {
        let turns = vec!["Hello.".to_string()];
        assert_eq!(
            AssemblyAiSttProvider::join_turn_texts(&turns, "How are"),
            "Hello. How are"
        );
    }

    #[test]
    fn test_join_turn_texts_empty() {
        let empty: Vec<String> = vec![];
        assert_eq!(AssemblyAiSttProvider::join_turn_texts(&empty, ""), "");
    }

    #[test]
    fn test_join_turn_texts_partial_only() {
        let empty: Vec<String> = vec![];
        assert_eq!(
            AssemblyAiSttProvider::join_turn_texts(&empty, "  Hello  "),
            "Hello"
        );
    }

    /// Integration test: connects to real AssemblyAI WS, sends audio, verifies message flow.
    ///
    /// Run with: `cargo test test_assemblyai_streaming_integration -- --ignored --nocapture`
    #[tokio::test]
    #[ignore]
    async fn test_assemblyai_streaming_integration() {
        // Read API key from OS keychain.
        let entry = keyring::Entry::new("kolboo", "assemblyai_api_key")
            .expect("Failed to create keyring entry");
        let api_key = entry
            .get_password()
            .expect("No AssemblyAI API key in keychain");

        let provider = AssemblyAiSttProvider::new(
            api_key,
            Some("universal-streaming-english".to_string()),
            None,
        );

        // Start streaming session at 48kHz (like real capture).
        let capture_sample_rate = 48000u32;
        let mut session = provider
            .start_streaming(capture_sample_rate)
            .await
            .expect("Failed to start streaming session");

        let audio_tx = session.audio_tx.clone();

        // Take partial_rx to monitor what the task sends.
        let mut partial_rx = session.take_partial_rx().unwrap();

        // Spawn a reader for partials.
        let partial_reader = tokio::spawn(async move {
            let mut count = 0u32;
            while let Some(partial) = partial_rx.recv().await {
                count += 1;
                eprintln!(
                    "  partial #{}: text='{}' committed={:?}",
                    count,
                    &partial.text[..partial.text.len().min(80)],
                    partial
                        .committed_text
                        .as_deref()
                        .map(|s| &s[..s.len().min(40)]),
                );
            }
            eprintln!("  partial_rx closed after {} partials", count);
            count
        });

        // Simulate sending 3 seconds of audio at 48kHz in ~100ms chunks.
        let chunk_samples = capture_sample_rate as usize / 10; // 4800 samples per 100ms
        let total_chunks = 30; // 3 seconds
        let mut chunks_sent = 0u32;

        for i in 0..total_chunks {
            let mut chunk = Vec::with_capacity(chunk_samples);
            for s in 0..chunk_samples {
                let t = (i * chunk_samples + s) as f32 / capture_sample_rate as f32;
                let val = 0.3 * (2.0 * std::f32::consts::PI * 200.0 * t).sin()
                    + 0.2 * (2.0 * std::f32::consts::PI * 440.0 * t).sin();
                chunk.push(val);
            }
            match audio_tx.send(chunk).await {
                Ok(()) => {
                    chunks_sent += 1;
                    if chunks_sent.is_multiple_of(10) {
                        eprintln!("  audio chunk {}/{}", chunks_sent, total_chunks);
                    }
                }
                Err(_) => {
                    eprintln!("  audio_tx.send failed at chunk {} (task exited early!)", i);
                    break;
                }
            }
            // Use realistic timing (100ms per chunk) to match actual audio pacing.
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        eprintln!("Sent {chunks_sent}/{total_chunks} chunks, finalizing...");

        // Drop the cloned audio_tx to let the channel close when finalize() drops the original.
        // In production, this is done by `set_live_audio_tx(None)` before finalize().
        drop(audio_tx);

        // Finalize (drops original audio_tx, waits for the task result).
        let result = session.finalize().await;
        match &result {
            Ok(text) => eprintln!("Finalized OK: {} chars, text='{}'", text.len(), text),
            Err(e) => eprintln!("Finalized with error: {}", e),
        }

        let partial_count = partial_reader.await.unwrap();
        eprintln!("Total partials received: {}", partial_count);

        assert!(
            result.is_ok(),
            "Streaming session failed: {:?}",
            result.err()
        );
    }
}
