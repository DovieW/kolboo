use super::OpenAiSttProvider;
use crate::audio_normalization::{
    chunk_size_bytes_for_pcm_s16le, f32_to_pcm_s16le, resample_linear,
};
use crate::request_log::RequestLogStore;
use crate::stt::http;
use crate::stt::streaming::{
    connect_ws_split_with_timeout, ws_close_best_effort, ws_next_with_timeout,
    ws_send_json_text_with_closed_handling, PartialTranscript, StreamingSttSession, WsSendOutcome,
    WsWrite,
};
use crate::stt::SttError;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde_json::{json, Value as JsonValue};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::{client::IntoClientRequest, Message};

/// OpenAI Realtime transcription task configuration.
///
/// Keeping this bundle local to the provider-local module avoids another long
/// argument list on the background-task entry point.
struct StreamingTaskConfig {
    capture_sample_rate: u32,
    model: String,
    language: Option<String>,
    prompt: Option<String>,
    request_log_store: Option<RequestLogStore>,
}

/// WebSocket timeout for realtime transcription operations.
const DEFAULT_WS_TRANSCRIPTION_TIMEOUT: Duration = Duration::from_secs(120);
/// Shorter timeout after the final commit so VAD-complete sessions don't wait
/// for the full streaming timeout.
const POST_COMMIT_TIMEOUT: Duration = Duration::from_secs(5);
/// OpenAI Realtime API requires audio at exactly 24 kHz.
const REALTIME_SAMPLE_RATE: u32 = 24_000;

#[derive(Debug, Clone, PartialEq, Eq)]
enum RealtimeServerEvent {
    SessionCreated,
    SessionUpdated,
    TranscriptionSessionCreated,
    TranscriptionSessionUpdated,
    Delta(String),
    Completed(String),
    BufferCommitted,
    SpeechStarted,
    SpeechStopped,
    Error { code: String, message: String },
    Unknown(String),
}

#[derive(Debug, Default)]
struct RealtimeTranscriptAccumulator {
    committed: Vec<String>,
    current_partial_text: String,
    logged_partials: Vec<JsonValue>,
}

impl RealtimeTranscriptAccumulator {
    fn apply_delta(&mut self, delta: &str, elapsed_ms: u64) -> Option<PartialTranscript> {
        if delta.is_empty() {
            return None;
        }

        self.current_partial_text.push_str(delta);
        let full_text = self.full_text();
        self.logged_partials.push(json!({
            "text": self.current_partial_text,
            "delta": delta,
            "elapsed_ms": elapsed_ms,
        }));

        Some(PartialTranscript {
            text: full_text,
            committed_text: None,
        })
    }

    fn apply_completed(&mut self, transcript_text: String) -> Option<PartialTranscript> {
        let is_nonempty = !transcript_text.trim().is_empty();
        if is_nonempty {
            self.committed.push(transcript_text.clone());
        }
        self.current_partial_text.clear();

        if !is_nonempty {
            return None;
        }

        Some(PartialTranscript {
            text: self.full_text(),
            committed_text: Some(transcript_text),
        })
    }

    fn finalize_pending_partial(&mut self) -> Option<PartialTranscript> {
        let current_partial = self.current_partial_text.trim().to_string();
        self.current_partial_text.clear();
        if current_partial.is_empty() {
            return None;
        }

        self.committed.push(current_partial.clone());
        Some(PartialTranscript {
            text: self.full_text(),
            committed_text: Some(current_partial),
        })
    }

    fn full_text(&self) -> String {
        build_full_text(&self.committed, &self.current_partial_text)
    }

    fn into_response_parts(self) -> (String, Vec<String>, Vec<JsonValue>) {
        (
            build_full_text(&self.committed, &self.current_partial_text),
            self.committed,
            self.logged_partials,
        )
    }
}

fn build_full_text(committed: &[String], current_partial_text: &str) -> String {
    if committed.is_empty() {
        return current_partial_text.to_string();
    }

    let mut full_text = committed.join(" ");
    if !current_partial_text.is_empty() {
        full_text.push(' ');
        full_text.push_str(current_partial_text);
    }
    full_text
}

fn parse_server_event(value: &JsonValue) -> RealtimeServerEvent {
    let event_type = value
        .get("type")
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .to_string();

    match event_type.as_str() {
        "session.created" => RealtimeServerEvent::SessionCreated,
        "session.updated" => RealtimeServerEvent::SessionUpdated,
        "transcription_session.created" => RealtimeServerEvent::TranscriptionSessionCreated,
        "transcription_session.updated" => RealtimeServerEvent::TranscriptionSessionUpdated,
        "conversation.item.input_audio_transcription.delta" => RealtimeServerEvent::Delta(
            value
                .get("delta")
                .and_then(|delta| delta.as_str())
                .unwrap_or("")
                .to_string(),
        ),
        "conversation.item.input_audio_transcription.completed" => RealtimeServerEvent::Completed(
            value
                .get("transcript")
                .and_then(|transcript| transcript.as_str())
                .unwrap_or("")
                .to_string(),
        ),
        "input_audio_buffer.committed" => RealtimeServerEvent::BufferCommitted,
        "input_audio_buffer.speech_started" => RealtimeServerEvent::SpeechStarted,
        "input_audio_buffer.speech_stopped" => RealtimeServerEvent::SpeechStopped,
        "error" => RealtimeServerEvent::Error {
            code: value
                .get("error")
                .and_then(|error| error.get("code"))
                .and_then(|code| code.as_str())
                .unwrap_or("unknown")
                .to_string(),
            message: value
                .get("error")
                .and_then(|error| error.get("message"))
                .and_then(|message| message.as_str())
                .unwrap_or("Unknown error")
                .to_string(),
        },
        _ => RealtimeServerEvent::Unknown(event_type),
    }
}

pub(super) fn supports_realtime_streaming(provider: &OpenAiSttProvider) -> bool {
    provider.model.contains("realtime")
}

pub(super) fn realtime_transcription_model(provider: &OpenAiSttProvider) -> String {
    provider.model.replace("-realtime", "")
}

pub(super) fn realtime_ws_url(provider: &OpenAiSttProvider) -> Result<String, SttError> {
    let base = http::trim_base_url(&provider.api_base_url);
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

pub(super) async fn start_streaming_session(
    provider: &OpenAiSttProvider,
    sample_rate: u32,
) -> Result<StreamingSttSession, SttError> {
    let ws_url = realtime_ws_url(provider)?;

    let mut request = ws_url
        .clone()
        .into_client_request()
        .map_err(|e| SttError::NetworkMessage(format!("WS request build failed: {}", e)))?;

    let auth_value = format!("Bearer {}", provider.api_key);
    request.headers_mut().insert(
        "Authorization",
        auth_value
            .parse()
            .map_err(|e| SttError::Config(format!("Invalid OpenAI API key header: {}", e)))?,
    );
    request.headers_mut().insert(
        "OpenAI-Beta",
        "realtime=v1"
            .parse()
            .map_err(|e| SttError::Config(format!("Invalid header: {}", e)))?,
    );

    let (ws_write, ws_read) = connect_ws_split_with_timeout(
        request,
        DEFAULT_WS_TRANSCRIPTION_TIMEOUT,
        &provider.proxy_settings,
    )
    .await?;

    let (audio_tx, audio_rx) = mpsc::channel::<Vec<f32>>(1024);
    let (partial_tx, partial_rx) = mpsc::channel::<PartialTranscript>(256);

    let request_log_store = provider.request_log_store.clone();
    let model = provider.model.clone();
    let transcription_model = realtime_transcription_model(provider);
    let language = provider.default_language.clone();
    let prompt = provider.default_prompt.clone();
    let ws_url_for_log = ws_url.to_string();

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
                "sample_rate": REALTIME_SAMPLE_RATE,
                "capture_sample_rate": sample_rate,
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

async fn run_streaming_task(
    mut ws_write: WsWrite,
    mut ws_read: crate::stt::streaming::WsRead,
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

    let target_chunk_bytes =
        chunk_size_bytes_for_pcm_s16le(REALTIME_SAMPLE_RATE, 1, 50, 1_600, 32_768);

    let session_start = std::time::Instant::now();
    let mut pcm_buffer: Vec<u8> = Vec::new();
    let mut num_chunks_sent: usize = 0;
    let mut transcript_state = RealtimeTranscriptAccumulator::default();

    let mut audio_done = false;
    let mut ws_done = false;
    let mut has_audio_since_last_commit = false;
    let mut awaiting_final_commit = false;

    loop {
        if ws_done {
            break;
        }

        let ws_timeout = if audio_done {
            POST_COMMIT_TIMEOUT
        } else {
            DEFAULT_WS_TRANSCRIPTION_TIMEOUT
        };

        tokio::select! {
            audio_chunk = audio_rx.recv(), if !audio_done => {
                match audio_chunk {
                    Some(f32_samples) => {
                        let resampled = resample_linear(
                            &f32_samples,
                            capture_sample_rate,
                            REALTIME_SAMPLE_RATE,
                        );
                        let pcm = f32_to_pcm_s16le(&resampled);
                        pcm_buffer.extend_from_slice(&pcm);

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

            ws_msg = ws_next_with_timeout(&mut ws_read, ws_timeout), if !ws_done => {
                match ws_msg {
                    Err(SttError::Timeout) if audio_done => {
                        log::debug!(
                            "OpenAI realtime: post-commit timeout, finalizing with {} transcripts",
                            transcript_state.committed.len()
                        );
                        ws_done = true;
                    }
                    Err(e) => return Err(e),
                    Ok(msg) => {
                        match msg {
                            Some(Message::Text(text)) => {
                                let value: JsonValue = serde_json::from_str(&text).map_err(|e| {
                                    SttError::Api(format!(
                                        "OpenAI realtime: failed to parse JSON: {} (raw={})",
                                        e, text
                                    ))
                                })?;

                                match parse_server_event(&value) {
                                    RealtimeServerEvent::SessionCreated => {
                                        log::debug!("OpenAI realtime: session.created");
                                        if let Some(store) = &request_log_store {
                                            store.with_current(|log| {
                                                log.info("Realtime transcription session connected");
                                            });
                                        }
                                    }
                                    RealtimeServerEvent::SessionUpdated => {
                                        log::debug!("OpenAI realtime: session.updated");
                                    }
                                    RealtimeServerEvent::TranscriptionSessionCreated => {
                                        log::debug!("OpenAI realtime: transcription_session.created");
                                        if let Some(store) = &request_log_store {
                                            store.with_current(|log| {
                                                log.info("Realtime transcription session connected");
                                            });
                                        }
                                    }
                                    RealtimeServerEvent::TranscriptionSessionUpdated => {
                                        log::debug!("OpenAI realtime: transcription_session.updated");
                                    }
                                    RealtimeServerEvent::Delta(delta) => {
                                        let elapsed = session_start.elapsed().as_millis() as u64;
                                        if let Some(partial) = transcript_state.apply_delta(&delta, elapsed) {
                                            if let Some(store) = &request_log_store {
                                                store.with_current(|log| {
                                                    log.raw_transcript = Some(partial.text.clone());
                                                });
                                            }
                                            let _ = partial_tx.try_send(partial);
                                        }
                                    }
                                    RealtimeServerEvent::Completed(transcript_text) => {
                                        if let Some(partial) = transcript_state.apply_completed(transcript_text) {
                                            if let Some(store) = &request_log_store {
                                                store.with_current(|log| {
                                                    log.raw_transcript = Some(partial.text.clone());
                                                });
                                            }
                                            let _ = partial_tx.try_send(partial);
                                        }

                                        if audio_done {
                                            ws_done = true;
                                        }
                                    }
                                    RealtimeServerEvent::BufferCommitted => {
                                        log::debug!("OpenAI realtime: audio buffer committed");
                                    }
                                    RealtimeServerEvent::SpeechStarted => {
                                        log::debug!("OpenAI realtime: speech started");
                                        has_audio_since_last_commit = true;
                                    }
                                    RealtimeServerEvent::SpeechStopped => {
                                        log::debug!("OpenAI realtime: speech stopped (VAD)");
                                        has_audio_since_last_commit = false;
                                    }
                                    RealtimeServerEvent::Error { code, message } => {
                                        if audio_done {
                                            log::warn!(
                                                "OpenAI realtime: non-fatal error after audio done ({}): {}",
                                                code,
                                                message
                                            );
                                            if awaiting_final_commit {
                                                ws_done = true;
                                            }
                                        } else {
                                            return Err(SttError::Api(format!(
                                                "OpenAI realtime error ({}): {}",
                                                code, message
                                            )));
                                        }
                                    }
                                    RealtimeServerEvent::Unknown(event_type) => {
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
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    ws_close_best_effort(&mut ws_write, "OpenAI realtime", Duration::from_secs(3)).await;
    drop(ws_write);

    if let Some(partial) = transcript_state.finalize_pending_partial() {
        let _ = partial_tx.try_send(partial);
    }

    let (final_text, committed, logged_partials) = transcript_state.into_response_parts();

    if let Some(store) = &request_log_store {
        let total_duration_ms = session_start.elapsed().as_millis() as u64;
        let response_json = json!({
            "committed_transcripts": committed,
            "chunks_sent": num_chunks_sent,
            "mode": "concurrent",
            "session_duration_ms": total_duration_ms,
            "partial_transcripts": logged_partials,
            "capture_sample_rate": capture_sample_rate,
            "target_sample_rate": REALTIME_SAMPLE_RATE,
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

#[cfg(test)]
mod tests {
    use super::super::OpenAiSttProvider;
    use super::*;

    #[test]
    fn parse_server_event_classifies_known_openai_events() {
        assert_eq!(
            parse_server_event(&json!({
                "type": "conversation.item.input_audio_transcription.delta",
                "delta": "hello"
            })),
            RealtimeServerEvent::Delta("hello".to_string())
        );
        assert_eq!(
            parse_server_event(&json!({
                "type": "conversation.item.input_audio_transcription.completed",
                "transcript": "hello world"
            })),
            RealtimeServerEvent::Completed("hello world".to_string())
        );
        assert_eq!(
            parse_server_event(&json!({ "type": "session.created" })),
            RealtimeServerEvent::SessionCreated
        );
        assert_eq!(
            parse_server_event(&json!({ "type": "input_audio_buffer.speech_stopped" })),
            RealtimeServerEvent::SpeechStopped
        );
    }

    #[test]
    fn parse_server_event_extracts_error_payloads_and_unknowns() {
        assert_eq!(
            parse_server_event(&json!({
                "type": "error",
                "error": { "code": "bad_request", "message": "buffer is empty" }
            })),
            RealtimeServerEvent::Error {
                code: "bad_request".to_string(),
                message: "buffer is empty".to_string(),
            }
        );
        assert_eq!(
            parse_server_event(&json!({ "type": "some.future.event" })),
            RealtimeServerEvent::Unknown("some.future.event".to_string())
        );
    }

    #[test]
    fn transcript_accumulator_builds_partial_and_committed_updates() {
        let mut accumulator = RealtimeTranscriptAccumulator::default();

        let partial = accumulator
            .apply_delta("hello", 12)
            .expect("partial update");
        assert_eq!(partial.text, "hello");
        assert_eq!(partial.committed_text, None);
        assert_eq!(accumulator.logged_partials.len(), 1);

        let committed = accumulator
            .apply_completed("hello world".to_string())
            .expect("committed update");
        assert_eq!(committed.text, "hello world");
        assert_eq!(committed.committed_text.as_deref(), Some("hello world"));
        assert_eq!(accumulator.full_text(), "hello world");
    }

    #[test]
    fn transcript_accumulator_finalizes_trailing_partial_as_committed_text() {
        let mut accumulator = RealtimeTranscriptAccumulator::default();
        accumulator.apply_delta("hello", 10);
        accumulator.apply_delta(" world", 20);

        let finalized = accumulator
            .finalize_pending_partial()
            .expect("finalized partial");
        assert_eq!(finalized.text, "hello world");
        assert_eq!(finalized.committed_text.as_deref(), Some("hello world"));
    }

    #[test]
    fn realtime_helpers_preserve_openai_model_and_url_conventions() {
        let provider = OpenAiSttProvider::new(
            "test-key".to_string(),
            Some("gpt-4o-mini-realtime-transcribe".to_string()),
            None,
            None,
        );
        assert!(supports_realtime_streaming(&provider));
        assert_eq!(
            realtime_transcription_model(&provider),
            "gpt-4o-mini-transcribe"
        );
        assert_eq!(
            realtime_ws_url(&provider).expect("ws url"),
            "wss://api.openai.com/v1/realtime?intent=transcription"
        );
    }
}
