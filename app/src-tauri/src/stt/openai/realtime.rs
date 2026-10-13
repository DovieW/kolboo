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
    Delta {
        item: String,
        event: Option<String>,
        text: String,
    },
    Completed {
        item: String,
        text: String,
    },
    BufferCommitted {
        item: String,
        previous: Option<String>,
    },
    SpeechStarted,
    SpeechStopped {
        item: String,
        audio_end_ms: u64,
    },
    Error {
        code: String,
        message: String,
    },
    Unknown(String),
    InvalidTranscriptEvent,
}

#[derive(Debug, Default)]
struct TranscriptItem {
    previous: Option<Option<String>>,
    finalized: Option<String>,
    partial: String,
    published: bool,
}

#[derive(Debug, Default)]
struct RealtimeTranscriptAccumulator {
    committed: Vec<String>,
    items: std::collections::BTreeMap<String, TranscriptItem>,
    last_published: Option<String>,
    preview_item: Option<String>,
    delta_events: std::collections::HashSet<String>,
    logged_partials: Vec<JsonValue>,
}

impl RealtimeTranscriptAccumulator {
    fn apply_delta(
        &mut self,
        item: String,
        event: Option<String>,
        delta: &str,
        elapsed_ms: u64,
    ) -> Option<PartialTranscript> {
        if delta.is_empty() || event.is_some_and(|event| !self.delta_events.insert(event)) {
            return None;
        }

        let entry = self.items.entry(item.clone()).or_default();
        if entry.finalized.is_some() {
            return None;
        }
        entry.partial.push_str(delta);
        self.preview_item = Some(item.clone());
        let full_text = self.full_text();
        self.logged_partials.push(json!({
            "text": self.items[&item].partial,
            "delta": delta,
            "elapsed_ms": elapsed_ms,
        }));

        Some(PartialTranscript {
            text: full_text,
            committed_text: None,
        })
    }

    fn apply_completed(
        &mut self,
        item: String,
        text: String,
    ) -> Result<Option<PartialTranscript>, SttError> {
        let entry = self.items.entry(item).or_default();
        if entry.finalized.is_some() {
            return Ok(None);
        }
        entry.finalized = Some(text);
        entry.partial.clear();
        self.publish_ordered()
    }

    fn register_item(
        &mut self,
        item: String,
        previous: Option<String>,
    ) -> Result<Option<PartialTranscript>, SttError> {
        let entry = self.items.entry(item).or_default();
        if entry
            .previous
            .as_ref()
            .is_some_and(|known| *known != previous)
        {
            return Err(SttError::Api(
                "OpenAI returned conflicting transcript order metadata".into(),
            ));
        }
        entry.previous = Some(previous);
        self.publish_ordered()
    }

    fn publish_ordered(&mut self) -> Result<Option<PartialTranscript>, SttError> {
        let mut added = Vec::new();
        loop {
            let mut successors = self.items.iter().filter(|(_, entry)| {
                !entry.published && entry.previous.as_ref() == Some(&self.last_published)
            });
            let next = successors
                .next()
                .map(|(id, entry)| (id.clone(), entry.finalized.clone()));
            if successors.next().is_some() {
                return Err(SttError::Api(
                    "OpenAI returned ambiguous transcript order metadata".into(),
                ));
            }
            let Some((id, Some(text))) = next else { break };
            self.items
                .get_mut(&id)
                .expect("selected existing item")
                .published = true;
            self.last_published = Some(id);
            if !text.trim().is_empty() {
                self.committed.push(text.clone());
                added.push(text);
            }
        }
        if added.is_empty() {
            return Ok(None);
        }
        Ok(Some(PartialTranscript {
            text: self.full_text(),
            committed_text: Some(added.join(" ")),
        }))
    }

    fn complete(&self) -> bool {
        !self.items.is_empty() && self.items.values().all(|item| item.published)
    }

    fn full_text(&self) -> String {
        let partial = self
            .preview_item
            .as_ref()
            .and_then(|id| self.items.get(id))
            .map(|item| item.partial.as_str())
            .unwrap_or("");
        build_full_text(&self.committed, partial)
    }

    fn into_response_parts(self) -> Result<(String, Vec<String>, Vec<JsonValue>), SttError> {
        if !self.complete() {
            return Err(SttError::Api(
                "OpenAI did not finalize every audio item; the recording is retained".into(),
            ));
        }
        Ok((self.full_text(), self.committed, self.logged_partials))
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
        "conversation.item.input_audio_transcription.delta"
        | "conversation.item.input_audio_transcription.completed"
        | "input_audio_buffer.committed" => {
            let Some(item) = value
                .get("item_id")
                .and_then(JsonValue::as_str)
                .filter(|id| !id.is_empty())
            else {
                return RealtimeServerEvent::InvalidTranscriptEvent;
            };
            match event_type.as_str() {
                "conversation.item.input_audio_transcription.delta" => RealtimeServerEvent::Delta {
                    item: item.into(),
                    event: value
                        .get("event_id")
                        .and_then(JsonValue::as_str)
                        .map(str::to_owned),
                    text: value
                        .get("delta")
                        .and_then(JsonValue::as_str)
                        .unwrap_or("")
                        .into(),
                },
                "conversation.item.input_audio_transcription.completed" => {
                    let Some(text) = value.get("transcript").and_then(JsonValue::as_str) else {
                        return RealtimeServerEvent::InvalidTranscriptEvent;
                    };
                    RealtimeServerEvent::Completed {
                        item: item.into(),
                        text: text.into(),
                    }
                }
                _ => RealtimeServerEvent::BufferCommitted {
                    item: item.into(),
                    previous: value
                        .get("previous_item_id")
                        .and_then(JsonValue::as_str)
                        .map(str::to_owned),
                },
            }
        }
        "input_audio_buffer.speech_started" => RealtimeServerEvent::SpeechStarted,
        "input_audio_buffer.speech_stopped" => match (
            value
                .get("item_id")
                .and_then(JsonValue::as_str)
                .filter(|id| !id.is_empty()),
            value.get("audio_end_ms").and_then(JsonValue::as_u64),
        ) {
            (Some(item), Some(audio_end_ms)) => RealtimeServerEvent::SpeechStopped {
                item: item.into(),
                audio_end_ms,
            },
            _ => RealtimeServerEvent::InvalidTranscriptEvent,
        },
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
    provider.model == "gpt-live-transcribe" || provider.model.contains("realtime")
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
    if provider.model != "gpt-live-transcribe" {
        request
            .headers_mut()
            .insert("OpenAI-Beta", "realtime=v1".parse().expect("static header"));
    }

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

fn transcription_session_update(
    model: &str,
    language: Option<&str>,
    prompt: Option<&str>,
) -> JsonValue {
    let mut transcription = json!({ "model": model });
    if let Some(prompt) = prompt {
        transcription["prompt"] = json!(prompt);
    }
    if model == "gpt-live-transcribe" {
        if let Some(language) = language {
            transcription["languages"] = json!([language]);
        }
        // The live model does not support server VAD. Our recording lifecycle
        // supplies one final commit when the user stops, with deltas during capture.
        json!({
            "type": "session.update",
            "session": {
                "type": "transcription",
                "audio": { "input": {
                    "format": { "type": "audio/pcm", "rate": REALTIME_SAMPLE_RATE },
                    "transcription": transcription,
                    "turn_detection": null,
                }}
            }
        })
    } else {
        if let Some(language) = language {
            transcription["language"] = json!(language);
        }
        json!({
            "type": "transcription_session.update",
            "session": {
                "input_audio_format": "pcm16",
                "input_audio_transcription": transcription,
                "turn_detection": {
                    "type": "server_vad", "threshold": 0.5,
                    "prefix_padding_ms": 300, "silence_duration_ms": 300
                },
                "input_audio_noise_reduction": { "type": "near_field" }
            }
        })
    }
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

    let session_update =
        transcription_session_update(&model, language.as_deref(), prompt.as_deref());

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
    let mut sent_audio_bytes = 0_u64;
    let mut pending_vad_items = std::collections::HashSet::new();

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
                            sent_audio_bytes += chunk.len() as u64;
                            let msg = json!({
                                "type": "input_audio_buffer.append",
                                "audio": STANDARD.encode(&chunk),
                            });
                            ws_send_json_text_with_closed_handling(
                                &mut ws_write,
                                &msg,
                                "OpenAI realtime: send audio append",
                            ).await?.require_sent()?;
                            num_chunks_sent += 1;
                            has_audio_since_last_commit = true;
                        }
                    }
                    None => {
                        if !pcm_buffer.is_empty() {
                            sent_audio_bytes += pcm_buffer.len() as u64;
                            let msg = json!({
                                "type": "input_audio_buffer.append",
                                "audio": STANDARD.encode(&pcm_buffer),
                            });
                            ws_send_json_text_with_closed_handling(
                                &mut ws_write,
                                &msg,
                                "OpenAI realtime: send final audio append",
                            ).await?.require_sent()?;
                            num_chunks_sent += 1;
                            has_audio_since_last_commit = true;
                            pcm_buffer.clear();
                        }

                        if has_audio_since_last_commit {
                            let commit = json!({ "type": "input_audio_buffer.commit" });
                            ws_send_json_text_with_closed_handling(
                                &mut ws_write,
                                &commit,
                                "OpenAI realtime: send final commit",
                            ).await?.require_sent()?;
                            awaiting_final_commit = true;
                            log::debug!("OpenAI realtime: sent final commit (audio done)");
                        } else {
                            log::debug!("OpenAI realtime: audio done, no pending audio to commit");
                            ws_done = transcript_state.complete();
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
                                let value: JsonValue = serde_json::from_str(&text)
                                    .map_err(|_| SttError::Api("OpenAI realtime returned invalid JSON".into()))?;

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
                                    RealtimeServerEvent::Delta { item, event, text: delta } => {
                                        let elapsed = session_start.elapsed().as_millis() as u64;
                                        if let Some(partial) = transcript_state.apply_delta(item, event, &delta, elapsed) {
                                            if let Some(store) = &request_log_store {
                                                store.with_current(|log| {
                                                    log.raw_transcript = Some(partial.text.clone());
                                                });
                                            }
                                            crate::stt::streaming::publish_partial(&partial_tx, partial)?;
                                        }
                                    }
                                    RealtimeServerEvent::Completed { item, text } => {
                                        if let Some(partial) = transcript_state.apply_completed(item, text)? {
                                            if let Some(store) = &request_log_store {
                                                store.with_current(|log| {
                                                    log.raw_transcript = Some(partial.text.clone());
                                                });
                                            }
                                            crate::stt::streaming::publish_partial(&partial_tx, partial)?;
                                        }

                                        if audio_done && !awaiting_final_commit && transcript_state.complete() {
                                            ws_done = true;
                                        }
                                    }
                                    RealtimeServerEvent::BufferCommitted { item, previous } => {
                                        log::debug!("OpenAI realtime: audio buffer committed");
                                        let automatic_commit = pending_vad_items.remove(&item);
                                        let duplicate_commit = transcript_state.items.get(&item).is_some_and(|entry| entry.previous.is_some());
                                        if let Some(partial) = transcript_state.register_item(item, previous)? {
                                            crate::stt::streaming::publish_partial(&partial_tx, partial)?;
                                        }
                                        if audio_done {
                                            if !automatic_commit && !duplicate_commit {
                                                awaiting_final_commit = false;
                                            }
                                            ws_done = !awaiting_final_commit && transcript_state.complete();
                                        }
                                    }
                                    RealtimeServerEvent::SpeechStarted => {
                                        log::debug!("OpenAI realtime: speech started");
                                        has_audio_since_last_commit = true;
                                    }
                                    RealtimeServerEvent::SpeechStopped { item, audio_end_ms } => {
                                        log::debug!("OpenAI realtime: speech stopped (VAD)");
                                        // A VAD event covers only its audio timestamp, not
                                        // chunks the client delivered after that turn.
                                        has_audio_since_last_commit = sent_audio_bytes > audio_end_ms.saturating_mul(u64::from(REALTIME_SAMPLE_RATE)).saturating_mul(2) / 1000;
                                        transcript_state.items.entry(item.clone()).or_default();
                                        pending_vad_items.insert(item);
                                    }
                                    RealtimeServerEvent::Error { .. } => return Err(SttError::Api("OpenAI realtime rejected transcription; the recording is retained".into())),
                                    RealtimeServerEvent::InvalidTranscriptEvent => return Err(SttError::Api("OpenAI returned invalid transcript identity metadata".into())),
                                    RealtimeServerEvent::Unknown(_) => {
                                        log::trace!("OpenAI realtime: ignoring an unknown event");
                                    }
                                }
                            }
                            Some(Message::Close(_)) | None => return Err(SttError::NetworkMessage("OpenAI closed without confirming finalization".into())),
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    ws_close_best_effort(&mut ws_write, "OpenAI realtime", Duration::from_secs(3)).await;
    drop(ws_write);

    if awaiting_final_commit {
        return Err(SttError::Timeout);
    }
    let (final_text, committed, logged_partials) = transcript_state.into_response_parts()?;

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
#[path = "tests/realtime_contract.rs"]
mod realtime_contract;

#[cfg(test)]
mod tests {
    use super::super::OpenAiSttProvider;
    use super::*;

    #[tokio::test]
    async fn websocket_finalization_reconciles_out_of_order_items_and_rejects_unsafe_results() {
        use futures_util::{SinkExt, StreamExt};
        for scenario in ["ordered", "malformed", "missing_order", "provider_error"] {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let server = tokio::spawn(async move {
                let (socket, _) = listener.accept().await.unwrap();
                let mut ws = tokio_tungstenite::accept_async(socket).await.unwrap();
                let configuration = ws.next().await.unwrap().unwrap().into_text().unwrap();
                assert_eq!(
                    serde_json::from_str::<JsonValue>(&configuration).unwrap()["type"],
                    "session.update"
                );
                ws.send(Message::Text(
                    json!({"type":"session.updated"}).to_string().into(),
                ))
                .await
                .unwrap();
                loop {
                    let frame = ws.next().await.unwrap().unwrap().into_text().unwrap();
                    if serde_json::from_str::<JsonValue>(&frame).unwrap()["type"]
                        == "input_audio_buffer.commit"
                    {
                        break;
                    }
                }
                let events = match scenario {
                    "malformed" => vec!["synthetic private content, invalid JSON".to_string()],
                    "provider_error" => vec![json!({"type":"error","error":{"code":"synthetic","message":"synthetic private content"}}).to_string()],
                    "missing_order" => vec![json!({"type":"input_audio_buffer.committed","item_id":"b","previous_item_id":"missing"}).to_string(), json!({"type":"conversation.item.input_audio_transcription.completed","item_id":"b","transcript":"second"}).to_string()],
                    _ => vec![
                        json!({"type":"input_audio_buffer.committed","item_id":"b","previous_item_id":"a"}).to_string(),
                        json!({"type":"conversation.item.input_audio_transcription.completed","item_id":"b","transcript":"second"}).to_string(),
                        json!({"type":"conversation.item.input_audio_transcription.completed","item_id":"b","transcript":"duplicate"}).to_string(),
                        json!({"type":"input_audio_buffer.committed","item_id":"a","previous_item_id":null}).to_string(),
                        json!({"type":"conversation.item.input_audio_transcription.delta","item_id":"a","event_id":"e","delta":"first"}).to_string(),
                        json!({"type":"conversation.item.input_audio_transcription.delta","item_id":"a","event_id":"e","delta":"first"}).to_string(),
                        json!({"type":"conversation.item.input_audio_transcription.completed","item_id":"a","transcript":"first"}).to_string(),
                    ],
                };
                for event in events {
                    ws.send(Message::Text(event.into())).await.unwrap();
                }
                if scenario == "missing_order" {
                    ws.close(None).await.unwrap();
                } else {
                    let _ = ws.next().await;
                }
            });
            let provider = OpenAiSttProvider::new(
                "synthetic-key".into(),
                Some("gpt-live-transcribe".into()),
                None,
                None,
            )
            .with_api_base_url(format!("http://{address}"));
            let mut session = start_streaming_session(&provider, 24000).await.unwrap();
            let mut previews = session.take_partial_rx().unwrap();
            session.audio_tx.send(vec![0.1; 4800]).await.unwrap();
            let result = session.finalize().await;
            if scenario == "ordered" {
                assert_eq!(result.unwrap(), "first second");
                let mut commits = Vec::new();
                while let Some(partial) = previews.recv().await {
                    if let Some(text) = partial.committed_text {
                        commits.push(text);
                    }
                }
                assert_eq!(commits, ["first second"]);
            } else {
                let error = result.unwrap_err().to_string();
                assert!(!error.contains("synthetic private content"));
            }
            server.await.unwrap();
        }
    }

    #[test]
    fn session_configuration_preserves_legacy_and_ga_optional_fields() {
        for model in ["gpt-live-transcribe", "gpt-4o-transcribe"] {
            for (language, prompt) in [(None, None), (Some("he"), Some("terms"))] {
                let value = transcription_session_update(model, language, prompt);
                let transcription = if model == "gpt-live-transcribe" {
                    assert_eq!(value["type"], "session.update");
                    &value["session"]["audio"]["input"]["transcription"]
                } else {
                    assert_eq!(value["type"], "transcription_session.update");
                    assert_eq!(value["session"]["turn_detection"]["type"], "server_vad");
                    &value["session"]["input_audio_transcription"]
                };
                assert_eq!(transcription["model"], model);
                assert_eq!(
                    transcription.get("prompt"),
                    prompt.map(|v| json!(v)).as_ref()
                );
                if model == "gpt-live-transcribe" {
                    assert_eq!(
                        transcription.get("languages"),
                        language.map(|v| json!([v])).as_ref()
                    );
                    assert!(transcription.get("language").is_none());
                } else {
                    assert_eq!(
                        transcription.get("language"),
                        language.map(|v| json!(v)).as_ref()
                    );
                    assert!(transcription.get("languages").is_none());
                }
            }
        }
    }

    #[test]
    fn parse_server_event_classifies_known_openai_events() {
        assert_eq!(
            parse_server_event(&json!({
                "type": "conversation.item.input_audio_transcription.delta",
                "item_id": "item",
                "delta": "hello"
            })),
            RealtimeServerEvent::Delta {
                item: "item".into(),
                event: None,
                text: "hello".into()
            }
        );
        assert_eq!(
            parse_server_event(&json!({
                "type": "conversation.item.input_audio_transcription.completed",
                "item_id": "item",
                "transcript": "hello world"
            })),
            RealtimeServerEvent::Completed {
                item: "item".into(),
                text: "hello world".into()
            }
        );
        assert_eq!(
            parse_server_event(&json!({ "type": "session.created" })),
            RealtimeServerEvent::SessionCreated
        );
        assert_eq!(
            parse_server_event(
                &json!({ "type": "input_audio_buffer.speech_stopped", "item_id":"item", "audio_end_ms":100 })
            ),
            RealtimeServerEvent::SpeechStopped {
                item: "item".into(),
                audio_end_ms: 100
            }
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
        assert!(accumulator
            .register_item("item".into(), None)
            .unwrap()
            .is_none());

        let partial = accumulator
            .apply_delta("item".into(), None, "hello", 12)
            .expect("partial update");
        assert_eq!(partial.text, "hello");
        assert_eq!(partial.committed_text, None);
        assert_eq!(accumulator.logged_partials.len(), 1);

        let committed = accumulator
            .apply_completed("item".into(), "hello world".to_string())
            .unwrap()
            .expect("committed update");
        assert_eq!(committed.text, "hello world");
        assert_eq!(committed.committed_text.as_deref(), Some("hello world"));
        assert_eq!(accumulator.full_text(), "hello world");
    }

    #[test]
    fn transcript_accumulator_never_promotes_unfinalized_partials() {
        let mut accumulator = RealtimeTranscriptAccumulator::default();
        accumulator.apply_delta("item".into(), None, "hello", 10);
        accumulator.apply_delta("item".into(), None, " world", 20);
        assert!(accumulator.into_response_parts().is_err());
    }

    #[test]
    fn items_and_deltas_are_reconciled_once_in_server_declared_order() {
        let mut a = RealtimeTranscriptAccumulator::default();
        assert!(a
            .register_item("b".into(), Some("a".into()))
            .unwrap()
            .is_none());
        assert!(a
            .apply_delta("b".into(), Some("event".into()), "preview", 0)
            .is_some());
        assert!(a
            .apply_delta("b".into(), Some("event".into()), "preview", 0)
            .is_none());
        assert!(a.apply_delta("b".into(), None, "", 0).is_none());
        assert!(a
            .apply_completed("b".into(), "second".into())
            .unwrap()
            .is_none());
        assert!(!a.complete());
        assert!(a
            .apply_completed("b".into(), "duplicate".into())
            .unwrap()
            .is_none());
        assert!(a.apply_delta("b".into(), None, "stale", 0).is_none());
        assert!(a.register_item("a".into(), None).unwrap().is_none());
        let update = a
            .apply_completed("a".into(), "first".into())
            .unwrap()
            .unwrap();
        assert_eq!(update.committed_text.as_deref(), Some("first second"));
        assert_eq!(a.into_response_parts().unwrap().0, "first second");
        let mut a = RealtimeTranscriptAccumulator::default();
        assert!(a.apply_completed("a".into(), "".into()).unwrap().is_none());
        assert!(a.register_item("a".into(), None).unwrap().is_none());
        assert_eq!(a.full_text(), "");
        assert!(a.complete());
        assert!(a.register_item("a".into(), Some("other".into())).is_err());
        let mut a = RealtimeTranscriptAccumulator::default();
        a.register_item("a".into(), None).unwrap();
        assert!(a.register_item("b".into(), None).is_err());
        assert_eq!(
            parse_server_event(&json!({"type":"input_audio_buffer.committed"})),
            RealtimeServerEvent::InvalidTranscriptEvent
        );
        assert_eq!(
            parse_server_event(
                &json!({"type":"conversation.item.input_audio_transcription.completed","item_id":"a"})
            ),
            RealtimeServerEvent::InvalidTranscriptEvent
        );
        assert_eq!(
            parse_server_event(
                &json!({"type":"input_audio_buffer.committed","item_id":"a","previous_item_id":"b"})
            ),
            RealtimeServerEvent::BufferCommitted {
                item: "a".into(),
                previous: Some("b".into())
            }
        );
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
