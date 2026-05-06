//! Deepgram STT provider implementation.
//!
//! Supports both batch (pre-recorded) and real-time streaming transcription:
//!
//! **Batch** (HTTP POST):
//! - Endpoint: `POST /v1/listen`
//! - Audio: binary WAV body
//!
//! **Streaming** (WebSocket):
//! - Endpoint: `wss://api.deepgram.com/v1/listen`
//! - Audio: PCM 16-bit LE, mono at capture sample rate (`encoding=linear16`)
//! - Protocol: binary audio frames → JSON `Results` messages with `is_final`
//!   flag. Send `{"type": "Finalize"}` to flush, `{"type": "CloseStream"}`
//!   to close the session.
//!
//! Docs:
//! - <https://developers.deepgram.com/reference/speech-to-text/listen-streaming>
//! - <https://developers.deepgram.com/docs/finalize>
//! - <https://developers.deepgram.com/docs/close-stream>

use super::http;
use super::language;
use super::streaming::{
    connect_ws_split_with_timeout, ws_close_best_effort, ws_next_with_timeout,
    ws_send_binary_with_closed_handling, ws_send_json_text_with_closed_handling, PartialTranscript,
    StreamingSttSession, WsSendOutcome,
};
use super::{AudioFormat, SttError, SttProvider};
use crate::audio_normalization::{chunk_size_bytes_for_pcm_s16le, f32_to_pcm_s16le};
use crate::request_log::RequestLogStore;
use async_trait::async_trait;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use reqwest::Url;
use serde_json::json;
use serde_json::Value as JsonValue;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;

/// Deepgram API provider for speech-to-text
pub struct DeepgramSttProvider {
    client: reqwest::Client,
    api_key: String,
    model: String,
    language: Option<String>,
    detect_language: bool,
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

        if self.detect_language {
            url.query_pairs_mut().append_pair("detect_language", "true");
        } else if let Some(language) = self.language.as_deref() {
            url.query_pairs_mut().append_pair("language", language);
        }

        Ok(url)
    }

    /// Create a new Deepgram STT provider
    ///
    /// # Arguments
    /// * `api_key` - Deepgram API key
    /// * `model` - Model to use (e.g., "nova-2")
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn new(api_key: String, model: Option<String>, language: Option<String>) -> Self {
        let client = crate::network::build_plain_http_client_with_timeout(Duration::from_secs(60));
        let (language, detect_language) = Self::normalize_language(language);

        Self {
            client,
            api_key,
            model: model.unwrap_or_else(|| "nova-2".to_string()),
            language,
            detect_language,
            api_base_url: Self::DEFAULT_DEEPGRAM_API_BASE_URL.to_string(),
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
    ) -> Self {
        let (language, detect_language) = Self::normalize_language(language);
        Self {
            client,
            api_key,
            model: model.unwrap_or_else(|| "nova-2".to_string()),
            language,
            detect_language,
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

    fn normalize_language(language: Option<String>) -> (Option<String>, bool) {
        language::normalize_language_with_detection(language)
    }

    // ── Streaming helpers ────────────────────────────────────────────────

    /// Default WebSocket streaming endpoint.
    const DEFAULT_WS_URL: &'static str = "wss://api.deepgram.com/v1/listen";

    /// Timeout for the WebSocket handshake and for post-Finalize drain.
    const DEFAULT_WS_TIMEOUT: Duration = Duration::from_secs(30);

    /// Timeout waiting for final results after sending Finalize.
    const POST_FINALIZE_TIMEOUT: Duration = Duration::from_secs(15);

    /// Build the streaming WebSocket URL with query parameters.
    fn streaming_ws_url(&self, sample_rate: u32) -> Result<String, SttError> {
        let base = if self.api_base_url == Self::DEFAULT_DEEPGRAM_API_BASE_URL {
            Self::DEFAULT_WS_URL.to_string()
        } else {
            // For custom base URLs (tests), swap https:// → wss://
            let trimmed = http::trim_base_url(&self.api_base_url);
            format!(
                "{}/v1/listen",
                trimmed
                    .replace("https://", "wss://")
                    .replace("http://", "ws://")
            )
        };

        let mut url = Url::parse(&base)
            .map_err(|e| SttError::Config(format!("Invalid Deepgram WS URL: {}", e)))?;

        url.query_pairs_mut()
            .append_pair("model", &self.model)
            .append_pair("encoding", "linear16")
            .append_pair("sample_rate", &sample_rate.to_string())
            .append_pair("channels", "1")
            .append_pair("interim_results", "true")
            .append_pair("punctuate", "true")
            .append_pair("smart_format", "true")
            .append_pair("endpointing", "300")
            .append_pair("utterance_end_ms", "1500");

        if self.detect_language {
            url.query_pairs_mut().append_pair("detect_language", "true");
        } else if let Some(language) = self.language.as_deref() {
            url.query_pairs_mut().append_pair("language", language);
        }

        Ok(url.to_string())
    }

    /// Extract the transcript text from a Deepgram streaming Results message.
    ///
    /// Streaming response shape:
    /// ```json
    /// { "type": "Results", "is_final": bool, "speech_final": bool,
    ///   "channel": { "alternatives": [{ "transcript": "..." }] } }
    /// ```
    fn extract_transcript(v: &JsonValue) -> &str {
        v.get("channel")
            .and_then(|ch| ch.get("alternatives"))
            .and_then(|alts| alts.get(0))
            .and_then(|alt| alt.get("transcript"))
            .and_then(|t| t.as_str())
            .unwrap_or("")
    }

    /// Start a concurrent streaming STT session.
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
            HeaderValue::from_str(&format!("Token {}", self.api_key))
                .map_err(|e| SttError::Config(format!("Invalid Deepgram API key header: {}", e)))?,
        );
        if let Some((client_id, client_secret)) =
            crate::http::cloudflare_access_headers_for_url(&ws_url)
        {
            request.headers_mut().insert(
                "CF-Access-Client-Id",
                HeaderValue::from_str(&client_id).map_err(|e| {
                    SttError::Config(format!("Invalid Cloudflare Access client id header: {}", e))
                })?,
            );
            request.headers_mut().insert(
                "CF-Access-Client-Secret",
                HeaderValue::from_str(&client_secret).map_err(|e| {
                    SttError::Config(format!(
                        "Invalid Cloudflare Access client secret header: {}",
                        e
                    ))
                })?,
            );
        }

        let (ws_write, ws_read) =
            connect_ws_split_with_timeout(request, Self::DEFAULT_WS_TIMEOUT).await?;

        let (audio_tx, audio_rx) = mpsc::channel::<Vec<f32>>(1024);
        let (partial_tx, partial_rx) = mpsc::channel::<PartialTranscript>(256);

        let request_log_store = self.request_log_store.clone();
        let model = self.model.clone();
        let language = self.language.clone();

        if let Some(store) = &request_log_store {
            let request_json = json!({
                "provider": "deepgram",
                "endpoint": ws_url,
                "content_type": "websocket-binary-streaming",
                "mode": "concurrent",
                "fields": {
                    "model": model,
                    "language": language,
                    "language_detection": self.detect_language,
                },
                "audio": {
                    "encoding": "pcm_s16le_mono",
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

    /// Background task: reads f32 chunks from `audio_rx`, sends PCM over the WS,
    /// collects partial and final transcripts, and returns the committed text.
    ///
    /// ## Live output commit strategy
    ///
    /// Deepgram returns `Results` messages with two key booleans:
    /// - `is_final: false` → interim partial for the current utterance (overlay update)
    /// - `is_final: true`  → finalized segment with punctuation (commit for live paste)
    ///
    /// We accumulate finalized segments and track the latest interim partial
    /// to build the full running transcript.
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
        let mut committed_segments: Vec<String> = Vec::new();
        let mut current_partial = String::new();
        let mut logged_partials: Vec<JsonValue> = Vec::new();

        let mut audio_done = false;
        let mut ws_done = false;

        loop {
            if ws_done {
                break;
            }

            let ws_timeout = if audio_done {
                Self::POST_FINALIZE_TIMEOUT
            } else {
                Self::DEFAULT_WS_TIMEOUT
            };

            tokio::select! {
                // Branch 1: read audio chunks from capture thread.
                audio_chunk = audio_rx.recv(), if !audio_done => {
                    match audio_chunk {
                        Some(f32_samples) => {
                            let pcm = f32_to_pcm_s16le(&f32_samples);
                            pcm_buffer.extend_from_slice(&pcm);

                            // Send binary chunks when we've accumulated enough.
                            while pcm_buffer.len() >= target_chunk_bytes {
                                let chunk: Vec<u8> = pcm_buffer.drain(..target_chunk_bytes).collect();
                                match ws_send_binary_with_closed_handling(
                                    &mut ws_write,
                                    chunk,
                                    "Deepgram streaming: send audio chunk",
                                ).await? {
                                    WsSendOutcome::Sent => { num_chunks_sent += 1; }
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
                                match ws_send_binary_with_closed_handling(
                                    &mut ws_write,
                                    pcm_buffer.clone(),
                                    "Deepgram streaming: send final audio chunk",
                                ).await? {
                                    WsSendOutcome::Sent => { num_chunks_sent += 1; }
                                    WsSendOutcome::Closed => {
                                        audio_done = true;
                                        ws_done = true;
                                        pcm_buffer.clear();
                                        continue;
                                    }
                                }
                                pcm_buffer.clear();
                            }

                            // Flush any pending transcription.
                            let finalize = json!({"type": "Finalize"});
                            match ws_send_json_text_with_closed_handling(
                                &mut ws_write,
                                &finalize,
                                "Deepgram streaming: send Finalize",
                            ).await? {
                                WsSendOutcome::Sent => {}
                                WsSendOutcome::Closed => {
                                    ws_done = true;
                                }
                            }

                            // Signal end-of-session.
                            let close_stream = json!({"type": "CloseStream"});
                            match ws_send_json_text_with_closed_handling(
                                &mut ws_write,
                                &close_stream,
                                "Deepgram streaming: send CloseStream",
                            ).await? {
                                WsSendOutcome::Sent => {}
                                WsSendOutcome::Closed => {
                                    ws_done = true;
                                }
                            }

                            audio_done = true;
                        }
                    }
                }

                // Branch 2: receive server messages.
                ws_msg = ws_next_with_timeout(&mut ws_read, ws_timeout), if !ws_done => {
                    match ws_msg {
                        Ok(Some(Message::Text(text))) => {
                            let v: JsonValue = match serde_json::from_str(&text) {
                                Ok(v) => v,
                                Err(e) => {
                                    log::debug!("Deepgram streaming: JSON parse error: {} (raw={})", e, text);
                                    continue;
                                }
                            };

                            let msg_type = v.get("type").and_then(|t| t.as_str()).unwrap_or("");

                            match msg_type {
                                "Results" => {
                                    let transcript = Self::extract_transcript(&v).to_string();
                                    let is_final = v.get("is_final")
                                        .and_then(|f| f.as_bool())
                                        .unwrap_or(false);

                                    let (full_text, committed_text) = if is_final {
                                        // Finalized segment — commit for live output.
                                        if !transcript.is_empty() {
                                            committed_segments.push(transcript.clone());
                                        }
                                        current_partial.clear();

                                        let full = Self::join_segments(&committed_segments, "");
                                        let committed = if transcript.is_empty() { None } else { Some(transcript) };
                                        (full, committed)
                                    } else {
                                        // Interim partial — update overlay text only.
                                        current_partial = transcript;

                                        let full = Self::join_segments(&committed_segments, &current_partial);
                                        (full, None)
                                    };

                                    let elapsed = session_start.elapsed().as_millis() as u64;
                                    logged_partials.push(json!({
                                        "text": &full_text,
                                        "is_final": is_final,
                                        "elapsed_ms": elapsed,
                                        "committed_segments": committed_segments.len(),
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
                                "Metadata" => {
                                    log::info!(
                                        "Deepgram streaming session started (request_id={}, model={})",
                                        v.get("request_id").and_then(|i| i.as_str()).unwrap_or("unknown"),
                                        v.get("model_info")
                                            .and_then(|m| m.get("name"))
                                            .and_then(|n| n.as_str())
                                            .unwrap_or("unknown"),
                                    );
                                }
                                "UtteranceEnd" | "SpeechStarted" => {
                                    // Informational; no action needed.
                                }
                                "Error" => {
                                    let err_msg = v.get("message")
                                        .or_else(|| v.get("description"))
                                        .and_then(|m| m.as_str())
                                        .unwrap_or(&text);
                                    return Err(SttError::Api(format!("Deepgram error: {}", err_msg)));
                                }
                                _ => {
                                    log::debug!("Deepgram streaming: unknown message type: {}", msg_type);
                                }
                            }
                        }
                        Ok(Some(Message::Close(frame))) => {
                            log::info!("Deepgram streaming: server sent Close frame {:?}", frame);
                            ws_done = true;
                        }
                        Ok(None) => {
                            log::info!("Deepgram streaming: WS stream ended");
                            ws_done = true;
                        }
                        Ok(_) => {
                            // Ignore binary/ping/pong.
                        }
                        Err(SttError::Timeout) if audio_done => {
                            log::warn!("Deepgram streaming: timed out waiting for final results, using accumulated segments");
                            ws_done = true;
                        }
                        Err(SttError::Timeout) => {
                            log::warn!(
                                "Deepgram streaming: WS read timed out while audio still flowing ({}s)",
                                Self::DEFAULT_WS_TIMEOUT.as_secs()
                            );
                            ws_done = true;
                        }
                        Err(e) => {
                            log::error!("Deepgram streaming: WS read error: {}", e);
                            return Err(e);
                        }
                    }
                }
            }
        }

        ws_close_best_effort(&mut ws_write, "Deepgram streaming", Duration::from_secs(3)).await;

        // Drop the write half explicitly so the TCP connection tears down cleanly.
        drop(ws_write);

        // Commit any remaining partial text that wasn't finalized by the server.
        if !current_partial.is_empty() {
            committed_segments.push(current_partial.clone());
            let final_with_partial = Self::join_segments(&committed_segments, "");
            let _ = partial_tx.try_send(PartialTranscript {
                text: final_with_partial,
                committed_text: Some(current_partial),
            });
        }

        let elapsed = session_start.elapsed();
        log::info!(
            "Deepgram streaming session complete: {} chunks sent, {} committed segments, {:.1}s",
            num_chunks_sent,
            committed_segments.len(),
            elapsed.as_secs_f64(),
        );

        // Final transcript: all committed segments joined.
        let final_text = Self::join_segments(&committed_segments, "");

        if let Some(store) = &request_log_store {
            store.with_current(|log| {
                log.raw_transcript = Some(final_text.clone());
                log.stt_response_json = Some(json!({
                    "streaming_session": true,
                    "committed_segments": committed_segments.len(),
                    "total_partials": logged_partials.len(),
                    "session_ms": elapsed.as_millis() as u64,
                    "chunks_sent": num_chunks_sent,
                    "partials": logged_partials,
                }));
            });
        }

        Ok(final_text)
    }

    /// Join committed segments and optionally an active partial into a single string.
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

        let response = crate::http::with_cloudflare_access_headers_if_target(
            self.client.post(url.clone()).headers(headers),
            url.as_str(),
        )
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
    fn test_provider_creation() {
        let provider = DeepgramSttProvider::new("test-key".to_string(), None, None);
        assert_eq!(provider.name(), "deepgram");
        assert_eq!(provider.model, "nova-2");
    }

    #[test]
    fn test_provider_with_custom_model() {
        let provider = DeepgramSttProvider::new(
            "test-key".to_string(),
            Some("nova-2-general".to_string()),
            None,
        );
        assert_eq!(provider.model, "nova-2-general");
    }

    #[test]
    fn test_supports_streaming() {
        let provider = DeepgramSttProvider::new("test-key".to_string(), None, None);
        assert!(provider.supports_streaming());
    }

    #[test]
    fn test_streaming_ws_url_default() {
        let provider = DeepgramSttProvider::new("k".to_string(), Some("nova-3".to_string()), None);
        let url = provider.streaming_ws_url(16000).unwrap();
        assert!(url.starts_with("wss://api.deepgram.com/v1/listen?"));
        assert!(url.contains("model=nova-3"));
        assert!(url.contains("encoding=linear16"));
        assert!(url.contains("sample_rate=16000"));
        assert!(url.contains("channels=1"));
        assert!(url.contains("interim_results=true"));
        assert!(url.contains("punctuate=true"));
        assert!(url.contains("smart_format=true"));
    }

    #[test]
    fn test_streaming_ws_url_with_language() {
        let provider = DeepgramSttProvider::new(
            "k".to_string(),
            Some("nova-3".to_string()),
            Some("en".to_string()),
        );
        let url = provider.streaming_ws_url(44100).unwrap();
        assert!(url.contains("language=en"));
        assert!(url.contains("sample_rate=44100"));
        assert!(!url.contains("detect_language"));
    }

    #[test]
    fn test_streaming_ws_url_with_detect_language() {
        let provider = DeepgramSttProvider::new("k".to_string(), None, Some("auto".to_string()));
        let url = provider.streaming_ws_url(16000).unwrap();
        assert!(url.contains("detect_language=true"));
        assert!(!url.contains("language=auto"));
    }

    #[test]
    fn test_extract_transcript() {
        let v: JsonValue = serde_json::from_str(
            r#"{
            "type": "Results",
            "is_final": true,
            "channel": {
                "alternatives": [{"transcript": "hello world", "confidence": 0.98}]
            }
        }"#,
        )
        .unwrap();
        assert_eq!(DeepgramSttProvider::extract_transcript(&v), "hello world");
    }

    #[test]
    fn test_extract_transcript_empty() {
        let v: JsonValue = serde_json::from_str(
            r#"{
            "type": "Results",
            "is_final": false,
            "channel": {
                "alternatives": [{"transcript": "", "confidence": 0.0}]
            }
        }"#,
        )
        .unwrap();
        assert_eq!(DeepgramSttProvider::extract_transcript(&v), "");
    }

    #[test]
    fn test_join_segments() {
        assert_eq!(
            DeepgramSttProvider::join_segments(&["Hello.".to_string(), "World.".to_string()], ""),
            "Hello. World."
        );
        assert_eq!(
            DeepgramSttProvider::join_segments(&["Hello.".to_string()], "world"),
            "Hello. world"
        );
        assert_eq!(
            DeepgramSttProvider::join_segments(&[], "partial"),
            "partial"
        );
        assert_eq!(DeepgramSttProvider::join_segments(&[], ""), "");
        // Empty segments are filtered out.
        assert_eq!(
            DeepgramSttProvider::join_segments(
                &["Hello.".to_string(), "".to_string(), "World.".to_string()],
                ""
            ),
            "Hello. World."
        );
        // Whitespace-only partials are trimmed/filtered.
        assert_eq!(
            DeepgramSttProvider::join_segments(&["Hello.".to_string()], "  "),
            "Hello."
        );
    }
}
