use super::DeepgramSttProvider;
use crate::audio_normalization::{chunk_size_bytes_for_pcm_s16le, f32_to_pcm_s16le};
use crate::request_log::RequestLogStore;
use crate::stt::streaming::{
    connect_ws_split_with_timeout, ws_close_best_effort, ws_next_with_timeout,
    ws_send_binary_with_closed_handling, ws_send_json_text_with_closed_handling, PartialTranscript,
    StreamingSttSession, WsRead, WsWrite,
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

#[derive(Debug, Clone, PartialEq)]
enum DeepgramServerEvent {
    Results {
        start: f64,
        duration: f64,
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
    Unknown,
    InvalidResults,
}

#[derive(Debug, Default)]
struct DeepgramTranscriptAccumulator {
    finalized_ranges: std::collections::HashSet<(u64, u64)>,
    final_end: f64,
    committed_segments: Vec<String>,
    current_partial: String,
    logged_partials: Vec<JsonValue>,
}

impl DeepgramTranscriptAccumulator {
    fn apply_result(
        &mut self,
        start: f64,
        duration: f64,
        transcript: String,
        is_final: bool,
        elapsed_ms: u64,
    ) -> Result<PartialTranscript, SttError> {
        let range = (start.to_bits(), duration.to_bits());
        if self.finalized_ranges.contains(&range)
            || (!is_final && start + 0.000001 < self.final_end)
        {
            return Ok(PartialTranscript {
                text: join_segments(&self.committed_segments, &self.current_partial),
                committed_text: None,
            });
        }
        if is_final && start + 0.000001 < self.final_end {
            return Err(SttError::Api("Deepgram returned overlapping or out-of-order final audio ranges; the recording is retained".into()));
        }
        let (full_text, committed_text) = if is_final {
            self.finalized_ranges.insert(range);
            self.final_end = start + duration;
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

        Ok(PartialTranscript {
            text: full_text,
            committed_text,
        })
    }

    fn final_text(&self) -> Result<String, SttError> {
        if !self.current_partial.is_empty() {
            return Err(SttError::Api(
                "Deepgram did not finalize its last transcript; the recording is retained".into(),
            ));
        }
        Ok(join_segments(&self.committed_segments, ""))
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
        if !matches!(
            provider.model.as_str(),
            "nova-2" | "nova-3" | "nova-3-medical"
        ) {
            return Err(SttError::Config("This Deepgram model needs an explicit language for realtime input. Choose a language hint or After recording; the model will not be substituted".into()));
        }
        // Batch language detection is not a realtime API parameter. Nova's
        // documented multilingual mode preserves the selected model.
        url.query_pairs_mut().append_pair("language", "multi");
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
                            ws_send_binary_with_closed_handling(
                                &mut ws_write,
                                chunk,
                                "Deepgram streaming: send audio chunk",
                            ).await?.require_sent()?;
                                    num_chunks_sent += 1;

                        }
                    }
                    None => {
                        if !pcm_buffer.is_empty() {
                            ws_send_binary_with_closed_handling(
                                &mut ws_write,
                                pcm_buffer.clone(),
                                "Deepgram streaming: send final audio chunk",
                            ).await?.require_sent()?;
                                    num_chunks_sent += 1;

                            pcm_buffer.clear();
                        }

                        let finalize = json!({"type": "Finalize"});
                        ws_send_json_text_with_closed_handling(
                            &mut ws_write,
                            &finalize,
                            "Deepgram streaming: send Finalize",
                        ).await?.require_sent()?;

                        let close_stream = json!({"type": "CloseStream"});
                        ws_send_json_text_with_closed_handling(
                            &mut ws_write,
                            &close_stream,
                            "Deepgram streaming: send CloseStream",
                        ).await?.require_sent()?;

                        audio_done = true;
                    }
                }
            }

            ws_msg = ws_next_with_timeout(&mut ws_read, ws_timeout), if !ws_done => {
                match ws_msg {
                    Ok(Some(Message::Text(text))) => {
                        let value: JsonValue = match serde_json::from_str(&text) {
                            Ok(value) => value,
                                    Err(_) => return Err(SttError::Api("Deepgram returned invalid JSON".into())),
                        };

                        match parse_server_event(&value) {
                                    DeepgramServerEvent::Results { start, duration, transcript, is_final } => {
                                let elapsed = session_start.elapsed().as_millis() as u64;
                                let partial = transcript_state.apply_result(
                                            start, duration,
                                    transcript,
                                    is_final,
                                    elapsed,
                                        )?;

                                if let Some(store) = &request_log_store {
                                    store.with_current(|log| {
                                        log.raw_transcript = Some(partial.text.clone());
                                    });
                                }

                                        crate::stt::streaming::publish_partial(&partial_tx, partial)?;
                            }
                            DeepgramServerEvent::Metadata {
                                        ..
                            } => {
                                log::info!(
                                            "Deepgram streaming session started",
                                );
                            }
                            DeepgramServerEvent::UtteranceEnd | DeepgramServerEvent::SpeechStarted => {
                                // Deepgram already encodes the live-output signal inside `Results`.
                                // These events are useful for logs, but they do not change transcript state.
                            }
                            DeepgramServerEvent::Error(message) => {
                                        let _ = message;
                                        return Err(SttError::Api("Deepgram rejected the stream; the recording is retained".into()));
                            }
                            DeepgramServerEvent::Unknown => {
                                log::debug!("Deepgram streaming: ignored an unknown event");
                            }
                                    DeepgramServerEvent::InvalidResults => return Err(SttError::Api("Deepgram returned invalid audio range metadata".into())),
                        }
                    }
                            Ok(Some(Message::Close(_))) | Ok(None) => {
                                if !audio_done { return Err(SttError::NetworkMessage("Deepgram closed before audio delivery completed".into())); }
                                log::info!("Deepgram streaming: server closed its connection");
                        ws_done = true;
                    }
                    Ok(_) => {
                        // Ignore binary/ping/pong.
                    }
                    Err(SttError::Timeout) if audio_done => {
                                return Err(SttError::Timeout);
                    }
                    Err(SttError::Timeout) => {
                        log::warn!(
                            "Deepgram streaming: WS read timed out while audio still flowing ({}s)",
                            DEFAULT_WS_TIMEOUT.as_secs(),
                        );
                                return Err(SttError::Timeout);
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

    let elapsed = session_start.elapsed();
    let final_text = transcript_state.final_text()?;
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
        "Results" => {
            let Some(start) = value
                .get("start")
                .and_then(JsonValue::as_f64)
                .filter(|value| value.is_finite() && *value >= 0.0)
            else {
                return DeepgramServerEvent::InvalidResults;
            };
            let Some(duration) = value
                .get("duration")
                .and_then(JsonValue::as_f64)
                .filter(|value| value.is_finite() && *value >= 0.0 && (start + *value).is_finite())
            else {
                return DeepgramServerEvent::InvalidResults;
            };
            DeepgramServerEvent::Results {
                start,
                duration,
                transcript: extract_transcript(value).to_string(),
                is_final: value
                    .get("is_final")
                    .and_then(|f| f.as_bool())
                    .unwrap_or(false),
            }
        }
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
        _ => DeepgramServerEvent::Unknown,
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
#[path = "tests/realtime_contract.rs"]
mod contract;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stt::SttProvider;

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
        assert!(url.contains("language=multi"));
        assert!(!url.contains("detect_language"));
        assert!(!url.contains("language=auto"));
    }

    #[test]
    fn auto_streaming_never_substitutes_an_unsupported_model() {
        let provider = DeepgramSttProvider::new("k".into(), Some("whisper-large".into()), None);
        assert!(!provider.supports_streaming());
        assert!(matches!(
            streaming_ws_url(&provider, 16000),
            Err(SttError::Config(_))
        ));
        let explicit =
            DeepgramSttProvider::new("k".into(), Some("whisper-large".into()), Some("he".into()));
        assert!(explicit.supports_streaming());
        assert!(streaming_ws_url(&explicit, 16000)
            .unwrap()
            .contains("language=he"));
    }

    #[test]
    fn parse_server_event_classifies_results_metadata_errors_and_unknowns() {
        assert_eq!(
            parse_server_event(&json!({
                "type": "Results",
                "start": 0.0,
                "duration": 1.0,
                "is_final": true,
                "channel": {
                    "alternatives": [{ "transcript": "hello world" }]
                }
            })),
            DeepgramServerEvent::Results {
                start: 0.0,
                duration: 1.0,
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
            DeepgramServerEvent::Unknown,
        );
    }

    #[test]
    fn transcript_accumulator_tracks_partial_and_final_results() {
        let mut accumulator = DeepgramTranscriptAccumulator::default();

        let partial = accumulator
            .apply_result(0.0, 1.0, "hello".to_string(), false, 12)
            .unwrap();
        assert_eq!(partial.text, "hello");
        assert_eq!(partial.committed_text, None);

        let committed = accumulator
            .apply_result(0.0, 1.0, "hello world".to_string(), true, 24)
            .unwrap();
        assert_eq!(committed.text, "hello world");
        assert_eq!(committed.committed_text.as_deref(), Some("hello world"));
        assert_eq!(accumulator.final_text().unwrap(), "hello world");
    }

    #[test]
    fn transcript_accumulator_never_promotes_unfinalized_partials() {
        let mut accumulator = DeepgramTranscriptAccumulator::default();
        accumulator
            .apply_result(0.0, 1.0, "hello".to_string(), false, 10)
            .unwrap();
        assert!(accumulator.final_text().is_err());
    }

    #[test]
    fn finalized_audio_ranges_are_not_duplicated_or_reordered() {
        let mut a = DeepgramTranscriptAccumulator::default();
        assert_eq!(
            a.apply_result(0.0, 1.0, "first".into(), true, 0)
                .unwrap()
                .committed_text
                .as_deref(),
            Some("first")
        );
        assert_eq!(
            a.apply_result(0.0, 1.0, "duplicate".into(), true, 0)
                .unwrap()
                .committed_text,
            None
        );
        assert_eq!(
            a.apply_result(0.0, 0.5, "stale".into(), false, 0)
                .unwrap()
                .text,
            "first"
        );
        assert!(a.apply_result(0.5, 1.0, "overlap".into(), true, 0).is_err());
        assert_eq!(
            a.apply_result(1.0, 1.0, "first".into(), true, 0)
                .unwrap()
                .text,
            "first first"
        );
        assert_eq!(a.final_text().unwrap(), "first first");
        for event in [
            json!({"type":"Results"}),
            json!({"type":"Results","start":-1,"duration":1}),
            json!({"type":"Results","start":0,"duration":-1}),
            json!({"type":"Results","start":0}),
        ] {
            assert_eq!(
                parse_server_event(&event),
                DeepgramServerEvent::InvalidResults
            );
        }
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
