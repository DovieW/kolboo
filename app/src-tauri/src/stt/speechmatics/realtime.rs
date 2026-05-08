use super::SpeechmaticsSttProvider;
use crate::audio_normalization::{chunk_size_bytes_for_pcm_s16le, f32_to_pcm_s16le};
use crate::request_log::RequestLogStore;
use crate::stt::streaming::{
    connect_ws_split_with_timeout, ws_close_best_effort, ws_next_with_timeout,
    ws_send_binary_with_closed_handling, ws_send_json_text_with_closed_handling, PartialTranscript,
    StreamingSttSession, WsRead, WsSendOutcome, WsWrite,
};
use crate::stt::SttError;
use serde_json::{json, Value as JsonValue};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::{client::IntoClientRequest, Message};

/// Speechmatics batch (one-shot) WebSocket endpoint.
const DEFAULT_WS_URL: &str = "wss://eu.rt.speechmatics.com/v2/";
/// Speechmatics concurrent-streaming WebSocket endpoint.
const STREAMING_WS_URL: &str = "wss://eu2.rt.speechmatics.com/v2/";
/// Timeout for handshakes and active-stream reads.
const DEFAULT_WS_TIMEOUT: Duration = Duration::from_secs(30);
/// Timeout waiting for `EndOfTranscript` after `EndOfStream`.
const POST_EOS_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Clone, PartialEq, Eq)]
enum SpeechmaticsServerEvent {
    RecognitionStarted,
    AudioAdded { seq_no: u64 },
    AddTranscript { text: String, has_eos: bool },
    AddPartialTranscript { text: String },
    EndOfTranscript,
    Error(String),
    Unknown(String),
}

#[derive(Debug, Default)]
struct SpeechmaticsTranscriptAccumulator {
    committed_segments: Vec<String>,
    current_segment: String,
    current_partial: String,
    logged_partials: Vec<JsonValue>,
}

impl SpeechmaticsTranscriptAccumulator {
    fn apply_final_text(
        &mut self,
        text: String,
        has_eos: bool,
        elapsed_ms: u64,
    ) -> PartialTranscript {
        if !text.is_empty() {
            if !self.current_segment.is_empty() {
                self.current_segment.push(' ');
            }
            self.current_segment.push_str(&text);
        }
        self.current_partial.clear();

        let (full_text, committed_text) = if has_eos && !self.current_segment.is_empty() {
            // Speechmatics finalizes sentence boundaries via `is_eos`, so keep the
            // provider-specific sentence commit rule here instead of inventing a
            // fake shared parser policy.
            let committed = std::mem::take(&mut self.current_segment);
            self.committed_segments.push(committed.clone());
            (join_segments(&self.committed_segments, ""), Some(committed))
        } else {
            (
                join_segments_with_accumulating(
                    &self.committed_segments,
                    &self.current_segment,
                    "",
                ),
                None,
            )
        };

        self.logged_partials.push(json!({
            "type": "final",
            "text": &full_text,
            "is_eos": has_eos,
            "elapsed_ms": elapsed_ms,
            "committed_segments": self.committed_segments.len(),
        }));

        PartialTranscript {
            text: full_text,
            committed_text,
        }
    }

    fn apply_partial_text(&mut self, partial_text: String, elapsed_ms: u64) -> PartialTranscript {
        self.current_partial = partial_text;

        let full_text = join_segments_with_accumulating(
            &self.committed_segments,
            &self.current_segment,
            &self.current_partial,
        );

        self.logged_partials.push(json!({
            "type": "partial",
            "text": &full_text,
            "elapsed_ms": elapsed_ms,
        }));

        PartialTranscript {
            text: full_text,
            committed_text: None,
        }
    }

    fn finalize_pending_text(&mut self) -> Option<PartialTranscript> {
        let mut trailing = std::mem::take(&mut self.current_segment);
        let trailing_partial = std::mem::take(&mut self.current_partial);

        let trailing_partial = trailing_partial.trim();
        if !trailing_partial.is_empty() {
            if !trailing.is_empty() {
                trailing.push(' ');
            }
            trailing.push_str(trailing_partial);
        }

        if trailing.is_empty() {
            return None;
        }

        self.committed_segments.push(trailing.clone());
        Some(PartialTranscript {
            text: join_segments(&self.committed_segments, ""),
            committed_text: Some(trailing),
        })
    }

    fn final_text(&self) -> String {
        join_segments_with_accumulating(
            &self.committed_segments,
            &self.current_segment,
            &self.current_partial,
        )
    }

    fn into_response_json(self, num_chunks_sent: usize, elapsed: Duration) -> JsonValue {
        json!({
            "provider": "speechmatics",
            "mode": "concurrent",
            "committed_segments": self.committed_segments,
            "chunks_sent": num_chunks_sent,
            "session_duration_ms": elapsed.as_millis() as u64,
            "partial_transcripts": self.logged_partials,
        })
    }
}

pub(super) async fn transcribe_ws(
    provider: &SpeechmaticsSttProvider,
    pcm: &[u8],
    sample_rate: u32,
    channels: u8,
) -> Result<(String, JsonValue), SttError> {
    let mut request = DEFAULT_WS_URL
        .into_client_request()
        .map_err(|e| SttError::NetworkMessage(format!("Invalid websocket URL: {}", e)))?;

    let bearer = format!("Bearer {}", provider.api_key.trim());
    request.headers_mut().insert(
        "Authorization",
        bearer
            .parse()
            .map_err(|e| SttError::Config(format!("Invalid Authorization header: {}", e)))?,
    );

    let (mut ws_write, mut ws_read) =
        connect_ws_split_with_timeout(request, DEFAULT_WS_TIMEOUT, &provider.proxy_settings)
            .await?;

    let request_json = json!({
        "provider": "speechmatics",
        "endpoint": DEFAULT_WS_URL,
        "auth": "Authorization: Bearer <redacted>",
        "start_recognition": start_recognition_message(provider, sample_rate, false),
        "audio": {
            "bytes": pcm.len(),
            "sample_rate": sample_rate,
            "channels": channels,
            "encoding": "pcm_s16le",
            "chunk_bytes": batch_chunk_size_bytes(sample_rate, channels),
        }
    });

    if let Some(store) = &provider.request_log_store {
        store.with_current(|log| {
            log.stt_request_json = Some(request_json);
        });
    }

    send_start_recognition(
        &mut ws_write,
        &start_recognition_message(provider, sample_rate, false),
        "Speechmatics batch transcription",
    )
    .await?;
    wait_for_recognition_started(&mut ws_read, "batch").await?;

    // Stream audio and drain protocol messages concurrently so we do not turn a
    // long recording into hundreds of serialized client/server round-trips.
    let mut last_seq_no: u64 = 0;
    let mut transcript = String::new();
    let mut received_for_log: Vec<JsonValue> = Vec::new();
    let mut chunks = pcm.chunks(batch_chunk_size_bytes(sample_rate, channels));
    let mut audio_done = false;

    loop {
        tokio::select! {
            send_result = async {
                if let Some(chunk) = chunks.next() {
                    match ws_send_binary_with_closed_handling(
                        &mut ws_write,
                        chunk.to_vec(),
                        "Speechmatics batch: send audio chunk",
                    ).await? {
                        WsSendOutcome::Sent => Ok(true),
                        WsSendOutcome::Closed => Err(SttError::NetworkMessage(
                            "Speechmatics websocket closed while sending audio".to_string(),
                        )),
                    }
                } else {
                    match ws_send_json_text_with_closed_handling(
                        &mut ws_write,
                        &json!({
                            "message": "EndOfStream",
                            "last_seq_no": last_seq_no,
                        }),
                        "Speechmatics batch: send EndOfStream",
                    ).await? {
                        WsSendOutcome::Sent => Ok(false),
                        WsSendOutcome::Closed => Err(SttError::NetworkMessage(
                            "Speechmatics websocket closed while sending EndOfStream".to_string(),
                        )),
                    }
                }
            }, if !audio_done => {
                match send_result {
                    Ok(true) => {}
                    Ok(false) => {
                        audio_done = true;
                    }
                    Err(error) => return Err(error),
                }
            }

            msg = ws_next_with_timeout(&mut ws_read, DEFAULT_WS_TIMEOUT) => {
                let Some(msg) = msg? else {
                    if audio_done {
                        break;
                    }
                    return Err(SttError::NetworkMessage(
                        "Speechmatics websocket closed while sending audio".to_string(),
                    ));
                };

                match msg {
                    Message::Text(text) => {
                        let Ok(value) = serde_json::from_str::<JsonValue>(&text) else {
                            continue;
                        };

                        match value.get("message").and_then(|m| m.as_str()).unwrap_or("") {
                            "AudioAdded" => {
                                if let Some(seq) = value.get("seq_no").and_then(|seq| seq.as_u64()) {
                                    last_seq_no = seq;
                                }
                            }
                            "AddTranscript" => {
                                if let Some(results) = value.get("results").and_then(|results| results.as_array()) {
                                    SpeechmaticsSttProvider::append_transcript_from_results(
                                        &mut transcript,
                                        results,
                                    );
                                }
                                received_for_log.push(value);
                            }
                            "EndOfTranscript" => {
                                received_for_log.push(value);
                                break;
                            }
                            "Error" => {
                                return Err(SttError::Api(text.to_string()));
                            }
                            _ => {
                                if received_for_log.len() < 100 {
                                    received_for_log.push(value);
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
                            frame,
                        )));
                    }
                    _ => {
                        // Ignore binary/ping/pong frames; Speechmatics transcript events are JSON.
                    }
                }
            }
        }
    }

    // Drain the post-EOS tail if the server has more finalized transcript frames
    // to deliver before `EndOfTranscript`.
    loop {
        let Some(msg) = ws_next_with_timeout(&mut ws_read, POST_EOS_TIMEOUT).await? else {
            break;
        };

        match msg {
            Message::Text(text) => {
                let Ok(value) = serde_json::from_str::<JsonValue>(&text) else {
                    continue;
                };

                match value.get("message").and_then(|m| m.as_str()).unwrap_or("") {
                    "AddTranscript" => {
                        if let Some(results) =
                            value.get("results").and_then(|results| results.as_array())
                        {
                            SpeechmaticsSttProvider::append_transcript_from_results(
                                &mut transcript,
                                results,
                            );
                        }
                        received_for_log.push(value);
                    }
                    "EndOfTranscript" => {
                        received_for_log.push(value);
                        break;
                    }
                    "Error" => {
                        return Err(SttError::Api(text.to_string()));
                    }
                    _ => {
                        if received_for_log.len() < 100 {
                            received_for_log.push(value);
                        }
                    }
                }
            }
            Message::Close(_) => break,
            _ => {}
        }
    }

    ws_close_best_effort(
        &mut ws_write,
        "Speechmatics batch transcription",
        Duration::from_secs(3),
    )
    .await;

    Ok((
        transcript.clone(),
        json!({
            "provider": "speechmatics",
            "operating_point": provider.operating_point_for_api(),
            "audio_format": {
                "sample_rate": sample_rate,
                "channels": channels,
                "encoding": "pcm_s16le",
                "bytes": pcm.len(),
            },
            "transcript": transcript,
            "received_messages": received_for_log,
        }),
    ))
}

pub(super) async fn start_streaming_session(
    provider: &SpeechmaticsSttProvider,
    sample_rate: u32,
) -> Result<StreamingSttSession, SttError> {
    let mut request = STREAMING_WS_URL
        .into_client_request()
        .map_err(|e| SttError::NetworkMessage(format!("WS request build failed: {}", e)))?;

    let bearer = format!("Bearer {}", provider.api_key.trim());
    request.headers_mut().insert(
        "Authorization",
        bearer
            .parse()
            .map_err(|e| SttError::Config(format!("Invalid Speechmatics auth header: {}", e)))?,
    );

    let (mut ws_write, mut ws_read) =
        connect_ws_split_with_timeout(request, DEFAULT_WS_TIMEOUT, &provider.proxy_settings)
            .await?;

    send_start_recognition(
        &mut ws_write,
        &start_recognition_message(provider, sample_rate, true),
        "Speechmatics streaming",
    )
    .await?;
    wait_for_recognition_started(&mut ws_read, "streaming").await?;

    let (audio_tx, audio_rx) = mpsc::channel::<Vec<f32>>(1024);
    let (partial_tx, partial_rx) = mpsc::channel::<PartialTranscript>(256);

    let request_log_store = provider.request_log_store.clone();
    if let Some(store) = &request_log_store {
        let request_json = json!({
            "provider": "speechmatics",
            "endpoint": STREAMING_WS_URL,
            "auth": "Authorization: Bearer <redacted>",
            "mode": "concurrent",
            "start_recognition": start_recognition_message(provider, sample_rate, true),
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
    let target_chunk_bytes = chunk_size_bytes_for_pcm_s16le(sample_rate, 1, 100, 1_600, 32_768);

    let session_start = std::time::Instant::now();
    let mut pcm_buffer: Vec<u8> = Vec::new();
    let mut num_chunks_sent: usize = 0;
    let mut last_seq_no: u64 = 0;
    let mut transcript_state = SpeechmaticsTranscriptAccumulator::default();

    let mut audio_done = false;
    let mut ws_done = false;

    loop {
        if ws_done {
            break;
        }

        let ws_timeout = if audio_done {
            POST_EOS_TIMEOUT
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
                                "Speechmatics streaming: send audio chunk",
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
                                "Speechmatics streaming: send final audio chunk",
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

                        match ws_send_json_text_with_closed_handling(
                            &mut ws_write,
                            &json!({
                                "message": "EndOfStream",
                                "last_seq_no": last_seq_no,
                            }),
                            "Speechmatics streaming: send EndOfStream",
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
                        let value: JsonValue = serde_json::from_str(&text).map_err(|e| {
                            SttError::Api(format!(
                                "Speechmatics streaming: failed to parse JSON: {} (raw={})",
                                e, text
                            ))
                        })?;

                        match parse_server_event(&value) {
                            SpeechmaticsServerEvent::AudioAdded { seq_no } => {
                                last_seq_no = seq_no;
                            }
                            SpeechmaticsServerEvent::AddTranscript { text, has_eos } => {
                                let partial = transcript_state.apply_final_text(
                                    text,
                                    has_eos,
                                    session_start.elapsed().as_millis() as u64,
                                );

                                if let Some(store) = &request_log_store {
                                    store.with_current(|log| {
                                        log.raw_transcript = Some(partial.text.clone());
                                    });
                                }

                                let _ = partial_tx.try_send(partial);
                            }
                            SpeechmaticsServerEvent::AddPartialTranscript { text } => {
                                let partial = transcript_state.apply_partial_text(
                                    text,
                                    session_start.elapsed().as_millis() as u64,
                                );
                                let _ = partial_tx.try_send(partial);
                            }
                            SpeechmaticsServerEvent::EndOfTranscript => {
                                log::info!("Speechmatics streaming: EndOfTranscript received");
                                ws_done = true;
                            }
                            SpeechmaticsServerEvent::Error(message) => {
                                return Err(SttError::Api(format!(
                                    "Speechmatics streaming error: {}",
                                    message,
                                )));
                            }
                            SpeechmaticsServerEvent::RecognitionStarted => {
                                // Startup already waited for this. Ignore late echoes.
                            }
                            SpeechmaticsServerEvent::Unknown(message_type) => {
                                log::debug!("Speechmatics streaming: {}", message_type);
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
                    Ok(_) => {}
                    Err(SttError::Timeout) if audio_done => {
                        log::warn!(
                            "Speechmatics streaming: timed out waiting for EndOfTranscript, using accumulated segments"
                        );
                        ws_done = true;
                    }
                    Err(SttError::Timeout) => {
                        log::warn!(
                            "Speechmatics streaming: WS read timed out while audio still flowing ({}s)",
                            DEFAULT_WS_TIMEOUT.as_secs(),
                        );
                        ws_done = true;
                    }
                    Err(error) => {
                        log::error!("Speechmatics streaming: WS read error: {}", error);
                        return Err(error);
                    }
                }
            }
        }
    }

    ws_close_best_effort(
        &mut ws_write,
        "Speechmatics streaming",
        Duration::from_secs(3),
    )
    .await;
    drop(ws_write);

    if let Some(partial) = transcript_state.finalize_pending_text() {
        let _ = partial_tx.try_send(partial);
    }

    let elapsed = session_start.elapsed();
    let final_text = transcript_state.final_text();
    let response_json = transcript_state.into_response_json(num_chunks_sent, elapsed);

    if let Some(store) = &request_log_store {
        store.with_current(|log| {
            log.raw_transcript = Some(final_text.clone());
            log.stt_response_json = Some(response_json);
        });
    }

    log::info!(
        "Speechmatics streaming: finalized, {} chars, {} chunks sent",
        final_text.len(),
        num_chunks_sent,
    );
    Ok(final_text)
}

fn start_recognition_message(
    provider: &SpeechmaticsSttProvider,
    sample_rate: u32,
    enable_partials: bool,
) -> JsonValue {
    json!({
        "message": "StartRecognition",
        "audio_format": {
            "type": "raw",
            "encoding": "pcm_s16le",
            "sample_rate": sample_rate,
        },
        "transcription_config": {
            "language": provider.language.clone(),
            "operating_point": provider.operating_point_for_api(),
            "max_delay": 1.0,
            "enable_partials": enable_partials,
        },
    })
}

async fn send_start_recognition(
    ws_write: &mut WsWrite,
    message: &JsonValue,
    context: &str,
) -> Result<(), SttError> {
    match ws_send_json_text_with_closed_handling(ws_write, message, context).await? {
        WsSendOutcome::Sent => Ok(()),
        WsSendOutcome::Closed => Err(SttError::NetworkMessage(format!(
            "{} websocket closed before StartRecognition completed",
            context,
        ))),
    }
}

async fn wait_for_recognition_started(ws_read: &mut WsRead, mode: &str) -> Result<(), SttError> {
    loop {
        let msg = ws_next_with_timeout(ws_read, DEFAULT_WS_TIMEOUT).await?;
        match msg {
            Some(Message::Text(text)) => {
                let Ok(value) = serde_json::from_str::<JsonValue>(&text) else {
                    continue;
                };

                match value
                    .get("message")
                    .and_then(|message| message.as_str())
                    .unwrap_or("")
                {
                    "RecognitionStarted" => return Ok(()),
                    "Error" => {
                        return Err(SttError::Api(format!(
                            "Speechmatics {} error during startup: {}",
                            mode, text,
                        )));
                    }
                    _ => {}
                }
            }
            Some(Message::Close(frame)) => {
                return Err(SttError::NetworkMessage(format!(
                    "Speechmatics {} websocket closed during startup: {:?}",
                    mode, frame,
                )));
            }
            None => {
                return Err(SttError::NetworkMessage(format!(
                    "Speechmatics {} websocket closed during startup",
                    mode,
                )));
            }
            _ => {}
        }
    }
}

fn batch_chunk_size_bytes(sample_rate: u32, channels: u8) -> usize {
    chunk_size_bytes_for_pcm_s16le(sample_rate, channels, 100, 2_048, 32_768)
}

fn parse_server_event(value: &JsonValue) -> SpeechmaticsServerEvent {
    let message_type = value
        .get("message")
        .and_then(|message| message.as_str())
        .unwrap_or("")
        .to_string();

    match message_type.as_str() {
        "RecognitionStarted" => SpeechmaticsServerEvent::RecognitionStarted,
        "AudioAdded" => SpeechmaticsServerEvent::AudioAdded {
            seq_no: value
                .get("seq_no")
                .and_then(|seq| seq.as_u64())
                .unwrap_or_default(),
        },
        "AddTranscript" => {
            let (text, has_eos) = value
                .get("results")
                .and_then(|results| results.as_array())
                .map_or((String::new(), false), |results| {
                    extract_streaming_text(results)
                });
            SpeechmaticsServerEvent::AddTranscript { text, has_eos }
        }
        "AddPartialTranscript" => {
            let (text, _) = value
                .get("results")
                .and_then(|results| results.as_array())
                .map_or((String::new(), false), |results| {
                    extract_streaming_text(results)
                });
            SpeechmaticsServerEvent::AddPartialTranscript { text }
        }
        "EndOfTranscript" => SpeechmaticsServerEvent::EndOfTranscript,
        "Error" => SpeechmaticsServerEvent::Error(
            value
                .get("detail")
                .or_else(|| value.get("error"))
                .and_then(|detail| detail.as_str())
                .unwrap_or("Unknown Speechmatics error")
                .to_string(),
        ),
        _ => SpeechmaticsServerEvent::Unknown(message_type),
    }
}

fn extract_streaming_text(results: &[JsonValue]) -> (String, bool) {
    let mut out = String::new();
    let mut has_eos = false;

    for result in results {
        let result_type = result
            .get("type")
            .and_then(|value| value.as_str())
            .unwrap_or("");
        let content = result
            .get("alternatives")
            .and_then(|alternatives| alternatives.as_array())
            .and_then(|alternatives| alternatives.first())
            .and_then(|alternative| alternative.get("content"))
            .and_then(|content| content.as_str());

        let Some(content) = content else {
            continue;
        };

        if result_type == "word" {
            if !out.is_empty() {
                out.push(' ');
            }
            out.push_str(content);
        } else {
            out.push_str(content);
        }

        if result
            .get("is_eos")
            .and_then(|value| value.as_bool())
            .unwrap_or(false)
        {
            has_eos = true;
        }
    }

    (out, has_eos)
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

fn join_segments_with_accumulating(
    committed: &[String],
    accumulating: &str,
    current_partial: &str,
) -> String {
    let mut parts: Vec<&str> = committed
        .iter()
        .map(|segment| segment.as_str())
        .filter(|segment| !segment.is_empty())
        .collect();

    let trimmed_accumulating = accumulating.trim();
    if !trimmed_accumulating.is_empty() {
        parts.push(trimmed_accumulating);
    }

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
    fn parse_server_event_classifies_core_messages() {
        assert_eq!(
            parse_server_event(&json!({ "message": "RecognitionStarted" })),
            SpeechmaticsServerEvent::RecognitionStarted,
        );

        assert_eq!(
            parse_server_event(&json!({
                "message": "AudioAdded",
                "seq_no": 7,
            })),
            SpeechmaticsServerEvent::AudioAdded { seq_no: 7 },
        );

        assert_eq!(
            parse_server_event(&json!({
                "message": "AddTranscript",
                "results": [
                    {
                        "type": "word",
                        "alternatives": [{ "content": "Hello" }],
                        "is_eos": false,
                    },
                    {
                        "type": "punctuation",
                        "alternatives": [{ "content": "." }],
                        "is_eos": true,
                    }
                ]
            })),
            SpeechmaticsServerEvent::AddTranscript {
                text: "Hello.".to_string(),
                has_eos: true,
            },
        );

        assert_eq!(
            parse_server_event(&json!({
                "message": "Error",
                "detail": "bad auth"
            })),
            SpeechmaticsServerEvent::Error("bad auth".to_string()),
        );
    }

    #[test]
    fn transcript_accumulator_tracks_sentence_commits_and_partials() {
        let mut accumulator = SpeechmaticsTranscriptAccumulator::default();

        let first_partial = accumulator.apply_partial_text("hello".to_string(), 10);
        assert_eq!(first_partial.text, "hello");
        assert_eq!(first_partial.committed_text, None);

        let not_yet_committed = accumulator.apply_final_text("hello".to_string(), false, 20);
        assert_eq!(not_yet_committed.text, "hello");
        assert_eq!(not_yet_committed.committed_text, None);

        let committed = accumulator.apply_final_text("world.".to_string(), true, 30);
        assert_eq!(committed.text, "hello world.");
        assert_eq!(committed.committed_text.as_deref(), Some("hello world."));
    }

    #[test]
    fn transcript_accumulator_finalizes_trailing_text() {
        let mut accumulator = SpeechmaticsTranscriptAccumulator::default();
        let _ = accumulator.apply_final_text("Hello".to_string(), false, 10);
        let _ = accumulator.apply_partial_text("world".to_string(), 20);

        let finalized = accumulator
            .finalize_pending_text()
            .expect("trailing text to finalize");
        assert_eq!(finalized.text, "Hello world");
        assert_eq!(finalized.committed_text.as_deref(), Some("Hello world"));
        assert_eq!(accumulator.final_text(), "Hello world");
    }

    #[test]
    fn join_helpers_filter_empty_parts() {
        assert_eq!(join_segments(&["hello".into()], "world"), "hello world");
        assert_eq!(join_segments(&[], "partial"), "partial");
        assert_eq!(join_segments(&[], "   "), "");

        assert_eq!(
            join_segments_with_accumulating(&["Hello.".into()], "The quick", "brown fox"),
            "Hello. The quick brown fox"
        );
        assert_eq!(
            join_segments_with_accumulating(&["Hello.".into()], "", "world"),
            "Hello. world"
        );
    }

    #[test]
    fn extract_streaming_text_preserves_sentence_boundary_flag() {
        let results = json!([
            {
                "type": "word",
                "alternatives": [{"content": "the"}],
                "is_eos": false,
            },
            {
                "type": "word",
                "alternatives": [{"content": "quick"}],
                "is_eos": false,
            },
            {
                "type": "punctuation",
                "alternatives": [{"content": "."}],
                "is_eos": true,
            }
        ]);

        let (text, has_eos) = extract_streaming_text(results.as_array().unwrap());
        assert_eq!(text, "the quick.");
        assert!(has_eos);
    }
}
