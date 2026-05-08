use super::DeepgramSttProvider;
use crate::audio_normalization::{chunk_size_bytes_for_pcm_s16le, f32_to_pcm_s16le};
use crate::request_log::RequestLogStore;
use crate::stt::streaming::{
    connect_ws_split_with_timeout, ws_close_best_effort, ws_next_with_timeout,
    ws_send_binary_with_closed_handling, ws_send_json_text_with_closed_handling, PartialTranscript,
    StreamingSttSession, WsRead, WsSendOutcome, WsWrite,
};
use crate::stt::SttError;
use reqwest::Url;
use serde_json::{json, Value as JsonValue};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::{http::HeaderValue, Message};

/// Default WebSocket streaming endpoint for Deepgram realtime transcription.
const DEFAULT_WS_URL: &str = "wss://api.deepgram.com/v1/listen";
/// Timeout for the WebSocket handshake and for active-stream reads.
const DEFAULT_WS_TIMEOUT: Duration = Duration::from_secs(30);
/// Timeout waiting for final results after sending `Finalize`.
const POST_FINALIZE_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Clone, PartialEq, Eq)]
enum DeepgramServerEvent {
    Results {
        transcript: String,
        is_final: bool,
    },
    Metadata {
        request_id: String,
        model_name: String,
    },
    UtteranceEnd,
    SpeechStarted,
    Error(String),
    Unknown(String),
}

#[derive(Debug, Default)]
struct DeepgramTranscriptAccumulator {
    committed_segments: Vec<String>,
    current_partial: String,
    logged_partials: Vec<JsonValue>,
}

impl DeepgramTranscriptAccumulator {
    fn apply_result(
        &mut self,
        transcript: String,
        is_final: bool,
        elapsed_ms: u64,
    ) -> PartialTranscript {
        let (full_text, committed_text) = if is_final {
            // Deepgram marks fully punctuated segments with `is_final=true`.
            // We commit those immediately so live output keeps sentence-sized chunks.
            if !transcript.is_empty() {
                self.committed_segments.push(transcript.clone());
            }
            self.current_partial.clear();

            let full = join_segments(&self.committed_segments, "");
            let committed = if transcript.is_empty() {
                None
            } else {
                Some(transcript)
            };
            (full, committed)
        } else {
            // Interims stay as the overlay-only tail until Deepgram finalizes them.
            self.current_partial = transcript;
            let full = join_segments(&self.committed_segments, &self.current_partial);
            (full, None)
        };

        self.logged_partials.push(json!({
            "text": &full_text,
            "is_final": is_final,
            "elapsed_ms": elapsed_ms,
            "committed_segments": self.committed_segments.len(),
        }));

        PartialTranscript {
            text: full_text,
            committed_text,
        }
    }

    fn finalize_pending_partial(&mut self) -> Option<PartialTranscript> {
        if self.current_partial.is_empty() {
            return None;
        }

        let trailing_partial = std::mem::take(&mut self.current_partial);
        self.committed_segments.push(trailing_partial.clone());

        Some(PartialTranscript {
            text: join_segments(&self.committed_segments, ""),
            committed_text: Some(trailing_partial),
        })
    }

    fn final_text(&self) -> String {
        join_segments(&self.committed_segments, &self.current_partial)
    }

    fn into_response_json(self, num_chunks_sent: usize, elapsed: Duration) -> JsonValue {
        json!({
            "streaming_session": true,
            "committed_segments": self.committed_segments.len(),
            "total_partials": self.logged_partials.len(),
            "session_ms": elapsed.as_millis() as u64,
            "chunks_sent": num_chunks_sent,
            "partials": self.logged_partials,
        })
    }
}

pub(super) fn streaming_ws_url(
    provider: &DeepgramSttProvider,
    sample_rate: u32,
) -> Result<String, SttError> {
    let base = if provider.api_base_url == DeepgramSttProvider::DEFAULT_DEEPGRAM_API_BASE_URL {
        DEFAULT_WS_URL.to_string()
    } else {
        // For custom base URLs (tests), swap http(s) to ws(s) while preserving the host.
        let trimmed = provider.api_base_url_trimmed();
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
        .append_pair("model", &provider.model)
        .append_pair("encoding", "linear16")
        .append_pair("sample_rate", &sample_rate.to_string())
        .append_pair("channels", "1")
        .append_pair("interim_results", "true")
        .append_pair("punctuate", "true")
        .append_pair("smart_format", "true")
        .append_pair("endpointing", "300")
        .append_pair("utterance_end_ms", "1500");

    if provider.detect_language {
        url.query_pairs_mut().append_pair("detect_language", "true");
    } else if let Some(language) = provider.language.as_deref() {
        url.query_pairs_mut().append_pair("language", language);
    }

    Ok(url.to_string())
}

pub(super) async fn start_streaming_session(
    provider: &DeepgramSttProvider,
    sample_rate: u32,
) -> Result<StreamingSttSession, SttError> {
    let ws_url = streaming_ws_url(provider, sample_rate)?;

    let mut request = ws_url
        .clone()
        .into_client_request()
        .map_err(|e| SttError::NetworkMessage(format!("WS request build failed: {}", e)))?;

    request.headers_mut().insert(
        "Authorization",
        HeaderValue::from_str(&format!("Token {}", provider.api_key))
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
        connect_ws_split_with_timeout(request, DEFAULT_WS_TIMEOUT, &provider.proxy_settings)
            .await?;

    let (audio_tx, audio_rx) = mpsc::channel::<Vec<f32>>(1024);
    let (partial_tx, partial_rx) = mpsc::channel::<PartialTranscript>(256);

    let request_log_store = provider.request_log_store.clone();
    let model = provider.model.clone();
    let language = provider.language.clone();

    if let Some(store) = &request_log_store {
        let request_json = json!({
            "provider": "deepgram",
            "endpoint": ws_url,
            "content_type": "websocket-binary-streaming",
            "mode": "concurrent",
            "fields": {
                "model": model,
                "language": language,
                "language_detection": provider.detect_language,
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

    let task = tokio::spawn(run_streaming_task(
        ws_write,
        ws_read,
        audio_rx,
        partial_tx,
        sample_rate,
        request_log_store,
    ));

    Ok(StreamingSttSession::new(audio_tx, partial_rx, task))
}

async fn run_streaming_task(
    mut ws_write: WsWrite,
    mut ws_read: WsRead,
    mut audio_rx: mpsc::Receiver<Vec<f32>>,
    partial_tx: mpsc::Sender<PartialTranscript>,
    sample_rate: u32,
    request_log_store: Option<RequestLogStore>,
) -> Result<String, SttError> {
    // 100ms chunks keep Deepgram responsive without exploding WS overhead.
    let target_chunk_bytes = chunk_size_bytes_for_pcm_s16le(sample_rate, 1, 100, 1_600, 32_768);

    let session_start = std::time::Instant::now();
    let mut pcm_buffer: Vec<u8> = Vec::new();
    let mut num_chunks_sent: usize = 0;
    let mut transcript_state = DeepgramTranscriptAccumulator::default();

    let mut audio_done = false;
    let mut ws_done = false;

    loop {
        if ws_done {
            break;
        }

        let ws_timeout = if audio_done {
            POST_FINALIZE_TIMEOUT
        } else {
            DEFAULT_WS_TIMEOUT
        };

        tokio::select! {
            audio_chunk = audio_rx.recv(), if !audio_done => {
                match audio_chunk {
                    Some(f32_samples) => {
                        let pcm = f32_to_pcm_s16le(&f32_samples);
                        pcm_buffer.extend_from_slice(&pcm);

                        while pcm_buffer.len() >= target_chunk_bytes {
                            let chunk: Vec<u8> = pcm_buffer.drain(..target_chunk_bytes).collect();
                            match ws_send_binary_with_closed_handling(
                                &mut ws_write,
                                chunk,
                                "Deepgram streaming: send audio chunk",
                            ).await? {
                                WsSendOutcome::Sent => {
                                    num_chunks_sent += 1;
                                }
                                WsSendOutcome::Closed => {
                                    ws_done = true;
                                    break;
                                }
                            }
                        }
                    }
                    None => {
                        if !pcm_buffer.is_empty() {
                            match ws_send_binary_with_closed_handling(
                                &mut ws_write,
                                pcm_buffer.clone(),
                                "Deepgram streaming: send final audio chunk",
                            ).await? {
                                WsSendOutcome::Sent => {
                                    num_chunks_sent += 1;
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

            ws_msg = ws_next_with_timeout(&mut ws_read, ws_timeout), if !ws_done => {
                match ws_msg {
                    Ok(Some(Message::Text(text))) => {
                        let value: JsonValue = match serde_json::from_str(&text) {
                            Ok(value) => value,
                            Err(error) => {
                                log::debug!(
                                    "Deepgram streaming: JSON parse error: {} (raw={})",
                                    error,
                                    text
                                );
                                continue;
                            }
                        };

                        match parse_server_event(&value) {
                            DeepgramServerEvent::Results { transcript, is_final } => {
                                let elapsed = session_start.elapsed().as_millis() as u64;
                                let partial = transcript_state.apply_result(
                                    transcript,
                                    is_final,
                                    elapsed,
                                );

                                if let Some(store) = &request_log_store {
                                    store.with_current(|log| {
                                        log.raw_transcript = Some(partial.text.clone());
                                    });
                                }

                                let _ = partial_tx.try_send(partial);
                            }
                            DeepgramServerEvent::Metadata {
                                request_id,
                                model_name,
                            } => {
                                log::info!(
                                    "Deepgram streaming session started (request_id={}, model={})",
                                    request_id,
                                    model_name,
                                );
                            }
                            DeepgramServerEvent::UtteranceEnd | DeepgramServerEvent::SpeechStarted => {
                                // Deepgram already encodes the live-output signal inside `Results`.
                                // These events are useful for logs, but they do not change transcript state.
                            }
                            DeepgramServerEvent::Error(message) => {
                                return Err(SttError::Api(format!("Deepgram error: {}", message)));
                            }
                            DeepgramServerEvent::Unknown(message_type) => {
                                log::debug!(
                                    "Deepgram streaming: unknown message type: {}",
                                    message_type,
                                );
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
                        log::warn!(
                            "Deepgram streaming: timed out waiting for final results, using accumulated segments"
                        );
                        ws_done = true;
                    }
                    Err(SttError::Timeout) => {
                        log::warn!(
                            "Deepgram streaming: WS read timed out while audio still flowing ({}s)",
                            DEFAULT_WS_TIMEOUT.as_secs(),
                        );
                        ws_done = true;
                    }
                    Err(error) => {
                        log::error!("Deepgram streaming: WS read error: {}", error);
                        return Err(error);
                    }
                }
            }
        }
    }

    ws_close_best_effort(&mut ws_write, "Deepgram streaming", Duration::from_secs(3)).await;
    drop(ws_write);

    if let Some(partial) = transcript_state.finalize_pending_partial() {
        let _ = partial_tx.try_send(partial);
    }

    let elapsed = session_start.elapsed();
    let final_text = transcript_state.final_text();
    let response_json = transcript_state.into_response_json(num_chunks_sent, elapsed);

    log::info!(
        "Deepgram streaming session complete: {} chunks sent, {:.1}s",
        num_chunks_sent,
        elapsed.as_secs_f64(),
    );

    if let Some(store) = &request_log_store {
        store.with_current(|log| {
            log.raw_transcript = Some(final_text.clone());
            log.stt_response_json = Some(response_json);
        });
    }

    Ok(final_text)
}

fn parse_server_event(value: &JsonValue) -> DeepgramServerEvent {
    let message_type = value
        .get("type")
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .to_string();

    match message_type.as_str() {
        "Results" => DeepgramServerEvent::Results {
            transcript: extract_transcript(value).to_string(),
            is_final: value
                .get("is_final")
                .and_then(|f| f.as_bool())
                .unwrap_or(false),
        },
        "Metadata" => DeepgramServerEvent::Metadata {
            request_id: value
                .get("request_id")
                .and_then(|id| id.as_str())
                .unwrap_or("unknown")
                .to_string(),
            model_name: value
                .get("model_info")
                .and_then(|model| model.get("name"))
                .and_then(|name| name.as_str())
                .unwrap_or("unknown")
                .to_string(),
        },
        "UtteranceEnd" => DeepgramServerEvent::UtteranceEnd,
        "SpeechStarted" => DeepgramServerEvent::SpeechStarted,
        "Error" => DeepgramServerEvent::Error(
            value
                .get("message")
                .or_else(|| value.get("description"))
                .and_then(|message| message.as_str())
                .unwrap_or("Unknown Deepgram error")
                .to_string(),
        ),
        _ => DeepgramServerEvent::Unknown(message_type),
    }
}

fn extract_transcript(value: &JsonValue) -> &str {
    value
        .get("channel")
        .and_then(|channel| channel.get("alternatives"))
        .and_then(|alternatives| alternatives.get(0))
        .and_then(|alternative| alternative.get("transcript"))
        .and_then(|transcript| transcript.as_str())
        .unwrap_or("")
}

fn join_segments(committed: &[String], current_partial: &str) -> String {
    let mut parts: Vec<&str> = committed
        .iter()
        .map(|segment| segment.as_str())
        .filter(|segment| !segment.is_empty())
        .collect();

    let trimmed_partial = current_partial.trim();
    if !trimmed_partial.is_empty() {
        parts.push(trimmed_partial);
    }

    parts.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn streaming_ws_url_default() {
        let provider = DeepgramSttProvider::new("k".to_string(), Some("nova-3".to_string()), None);
        let url = streaming_ws_url(&provider, 16000).expect("ws url");
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
    fn streaming_ws_url_with_language() {
        let provider = DeepgramSttProvider::new(
            "k".to_string(),
            Some("nova-3".to_string()),
            Some("en".to_string()),
        );
        let url = streaming_ws_url(&provider, 44100).expect("ws url");
        assert!(url.contains("language=en"));
        assert!(url.contains("sample_rate=44100"));
        assert!(!url.contains("detect_language"));
    }

    #[test]
    fn streaming_ws_url_with_detect_language() {
        let provider = DeepgramSttProvider::new("k".to_string(), None, Some("auto".to_string()));
        let url = streaming_ws_url(&provider, 16000).expect("ws url");
        assert!(url.contains("detect_language=true"));
        assert!(!url.contains("language=auto"));
    }

    #[test]
    fn parse_server_event_classifies_results_metadata_errors_and_unknowns() {
        assert_eq!(
            parse_server_event(&json!({
                "type": "Results",
                "is_final": true,
                "channel": {
                    "alternatives": [{ "transcript": "hello world" }]
                }
            })),
            DeepgramServerEvent::Results {
                transcript: "hello world".to_string(),
                is_final: true,
            },
        );

        assert_eq!(
            parse_server_event(&json!({
                "type": "Metadata",
                "request_id": "req-1",
                "model_info": { "name": "nova-3" }
            })),
            DeepgramServerEvent::Metadata {
                request_id: "req-1".to_string(),
                model_name: "nova-3".to_string(),
            },
        );

        assert_eq!(
            parse_server_event(&json!({
                "type": "Error",
                "description": "bad auth"
            })),
            DeepgramServerEvent::Error("bad auth".to_string()),
        );

        assert_eq!(
            parse_server_event(&json!({ "type": "SomethingNew" })),
            DeepgramServerEvent::Unknown("SomethingNew".to_string()),
        );
    }

    #[test]
    fn transcript_accumulator_tracks_partial_and_final_results() {
        let mut accumulator = DeepgramTranscriptAccumulator::default();

        let partial = accumulator.apply_result("hello".to_string(), false, 12);
        assert_eq!(partial.text, "hello");
        assert_eq!(partial.committed_text, None);

        let committed = accumulator.apply_result("hello world".to_string(), true, 24);
        assert_eq!(committed.text, "hello world");
        assert_eq!(committed.committed_text.as_deref(), Some("hello world"));
        assert_eq!(accumulator.final_text(), "hello world");
    }

    #[test]
    fn transcript_accumulator_finalizes_trailing_partial() {
        let mut accumulator = DeepgramTranscriptAccumulator::default();
        let _ = accumulator.apply_result("hello".to_string(), false, 10);
        let finalized = accumulator
            .finalize_pending_partial()
            .expect("trailing partial to finalize");

        assert_eq!(finalized.text, "hello");
        assert_eq!(finalized.committed_text.as_deref(), Some("hello"));
        assert_eq!(accumulator.final_text(), "hello");
    }

    #[test]
    fn join_segments_filters_empty_and_whitespace_only_parts() {
        assert_eq!(
            join_segments(&["Hello.".to_string(), "World.".to_string()], ""),
            "Hello. World.",
        );
        assert_eq!(
            join_segments(&["Hello.".to_string()], "world"),
            "Hello. world",
        );
        assert_eq!(join_segments(&[], "partial"), "partial");
        assert_eq!(join_segments(&[], ""), "");
        assert_eq!(
            join_segments(
                &["Hello.".to_string(), "".to_string(), "World.".to_string()],
                "",
            ),
            "Hello. World.",
        );
        assert_eq!(join_segments(&["Hello.".to_string()], "  "), "Hello.");
    }
}
