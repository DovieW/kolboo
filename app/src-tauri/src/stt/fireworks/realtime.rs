use super::FireworksSttProvider;
use crate::audio_normalization::{
    chunk_size_bytes_for_pcm_s16le, f32_to_pcm_s16le, resample_linear,
};
use crate::request_log::RequestLogStore;
use crate::stt::streaming::{
    connect_ws_split_with_timeout, ws_close_best_effort, ws_next_with_timeout,
    ws_send_binary_with_closed_handling, ws_send_json_text_with_closed_handling, PartialTranscript,
    StreamingSttSession, WsRead, WsSendOutcome, WsWrite,
};
use crate::stt::SttError;
use serde_json::{json, Value as JsonValue};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;

/// Required sample rate for the Fireworks realtime endpoint.
const STREAMING_SAMPLE_RATE: u32 = 16_000;
/// Timeout for connection setup and active-stream reads.
const DEFAULT_WS_TIMEOUT: Duration = Duration::from_secs(30);
/// Timeout while waiting for the final checkpoint after recording stops.
const POST_CHECKPOINT_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Clone, PartialEq, Eq)]
enum FireworksServerEvent {
    Segments(Vec<(usize, String)>),
    FinalCheckpoint,
    Error(String),
    Unknown,
}

#[derive(Debug, Default)]
struct FireworksTranscriptAccumulator {
    segments: Vec<String>,
    segment_stable_count: Vec<usize>,
    segment_first_seen: Vec<Instant>,
    committed_up_to: usize,
    logged_partials: Vec<JsonValue>,
}

impl FireworksTranscriptAccumulator {
    /// Number of consecutive unchanged updates before a segment is considered
    /// stable enough to commit immediately.
    const SEGMENT_STABILITY_THRESHOLD: usize = 3;
    /// Fallback age after which a segment can commit with lighter evidence.
    const SEGMENT_AGE_COMMIT_SECS: f64 = 1.5;

    fn apply_segments(
        &mut self,
        updated_segments: Vec<(usize, String)>,
        elapsed_ms: u64,
    ) -> PartialTranscript {
        let mut updated_ids = std::collections::HashSet::new();

        for (id, text) in updated_segments {
            updated_ids.insert(id);

            if id >= self.segments.len() {
                let new_len = id + 1;
                self.segments.resize(new_len, String::new());
                self.segment_stable_count.resize(new_len, 0);
                self.segment_first_seen.resize(new_len, Instant::now());
            }

            if self.segments[id] != text {
                self.segments[id] = text;
                self.segment_stable_count[id] = 0;
            } else {
                self.segment_stable_count[id] += 1;
            }
        }

        for index in self.committed_up_to..self.segments.len() {
            if !updated_ids.contains(&index) && !self.segments[index].is_empty() {
                self.segment_stable_count[index] += 1;
            }
        }

        let full_text = join_segments(&self.segments);
        self.logged_partials.push(json!({
            "text": &full_text,
            "elapsed_ms": elapsed_ms,
            "num_segments": self.segments.len(),
            "committed_up_to": self.committed_up_to,
        }));

        let previous_committed = self.committed_up_to;
        while self.committed_up_to < self.segments.len() {
            if self.segments[self.committed_up_to].is_empty() {
                break;
            }

            let stability = self.segment_stable_count[self.committed_up_to];
            let age_secs = self.segment_first_seen[self.committed_up_to]
                .elapsed()
                .as_secs_f64();
            let fully_stable = stability >= Self::SEGMENT_STABILITY_THRESHOLD;
            let age_stable = age_secs >= Self::SEGMENT_AGE_COMMIT_SECS && stability >= 1;

            if fully_stable || age_stable {
                self.committed_up_to += 1;
            } else {
                break;
            }
        }

        let committed_text = if self.committed_up_to > previous_committed {
            let text = join_segments(&self.segments[previous_committed..self.committed_up_to]);
            if text.is_empty() {
                None
            } else {
                Some(text)
            }
        } else {
            None
        };

        PartialTranscript {
            text: full_text,
            committed_text,
        }
    }

    fn finalize_remaining_commit(&self) -> Option<String> {
        if self.committed_up_to >= self.segments.len() {
            return None;
        }

        let remaining = join_segments(&self.segments[self.committed_up_to..]);
        if remaining.is_empty() {
            None
        } else {
            Some(remaining)
        }
    }

    fn final_text(&self) -> String {
        join_segments(&self.segments)
    }

    fn into_response_json(self, num_chunks_sent: usize, elapsed: Duration) -> JsonValue {
        json!({
            "segments": self.segments,
            "chunks_sent": num_chunks_sent,
            "mode": "concurrent",
            "session_duration_ms": elapsed.as_millis() as u64,
            "partial_transcripts": self.logged_partials,
        })
    }
}

pub(super) fn streaming_ws_url(provider: &FireworksSttProvider) -> Result<String, SttError> {
    let base = if let Some(base_url) = &provider.api_base_url {
        let ws_base = if base_url.starts_with("https") {
            base_url.replacen("https", "wss", 1)
        } else if base_url.starts_with("http") {
            base_url.replacen("http", "ws", 1)
        } else {
            base_url.clone()
        };
        format!(
            "{}/v1/audio/transcriptions/streaming",
            ws_base.trim_end_matches('/'),
        )
    } else {
        "wss://audio-streaming.api.fireworks.ai/v1/audio/transcriptions/streaming".to_string()
    };

    if let Some(language) = &provider.default_language {
        Ok(format!("{}?language={}", base, language))
    } else {
        Ok(base)
    }
}

pub(super) async fn start_streaming_session(
    provider: &FireworksSttProvider,
    sample_rate: u32,
) -> Result<StreamingSttSession, SttError> {
    let ws_url = streaming_ws_url(provider)?;

    let mut request = ws_url
        .clone()
        .into_client_request()
        .map_err(|e| SttError::NetworkMessage(format!("WS request build failed: {}", e)))?;

    request.headers_mut().insert(
        "Authorization",
        provider
            .api_key
            .parse()
            .map_err(|e| SttError::Config(format!("Invalid Fireworks API key header: {}", e)))?,
    );

    let (ws_write, ws_read) =
        connect_ws_split_with_timeout(request, DEFAULT_WS_TIMEOUT, &provider.proxy_settings)
            .await?;

    let (audio_tx, audio_rx) = mpsc::channel::<Vec<f32>>(1024);
    let (partial_tx, partial_rx) = mpsc::channel::<PartialTranscript>(256);

    let request_log_store = provider.request_log_store.clone();
    let model = provider.model.clone();
    let language = provider.default_language.clone();

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
                "sample_rate": STREAMING_SAMPLE_RATE,
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
    capture_sample_rate: u32,
    request_log_store: Option<RequestLogStore>,
) -> Result<String, SttError> {
    let target_chunk_bytes =
        chunk_size_bytes_for_pcm_s16le(STREAMING_SAMPLE_RATE, 1, 100, 1_600, 32_768);

    let session_start = std::time::Instant::now();
    let mut pcm_buffer: Vec<u8> = Vec::new();
    let mut num_chunks_sent: usize = 0;
    let mut transcript_state = FireworksTranscriptAccumulator::default();

    let mut audio_done = false;
    let mut ws_done = false;

    loop {
        if ws_done {
            break;
        }

        let ws_timeout = if audio_done {
            POST_CHECKPOINT_TIMEOUT
        } else {
            DEFAULT_WS_TIMEOUT
        };

        tokio::select! {
            audio_chunk = audio_rx.recv(), if !audio_done => {
                match audio_chunk {
                    Some(f32_samples) => {
                        let resampled = resample_linear(
                            &f32_samples,
                            capture_sample_rate,
                            STREAMING_SAMPLE_RATE,
                        );
                        let pcm = f32_to_pcm_s16le(&resampled);
                        pcm_buffer.extend_from_slice(&pcm);

                        while pcm_buffer.len() >= target_chunk_bytes {
                            let chunk: Vec<u8> = pcm_buffer.drain(..target_chunk_bytes).collect();
                            match ws_send_binary_with_closed_handling(
                                &mut ws_write,
                                chunk,
                                "Fireworks streaming: send audio chunk",
                            ).await? {
                                WsSendOutcome::Sent => {
                                    num_chunks_sent += 1;
                                }
                                WsSendOutcome::Closed => {
                                    log::warn!(
                                        "Fireworks streaming: websocket closed while sending audio, finishing early"
                                    );
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
                                "Fireworks streaming: send final audio chunk",
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
                            &json!({"checkpoint_id": "final"}),
                            "Fireworks streaming: send final checkpoint",
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
                                "Fireworks streaming: failed to parse JSON: {} (raw={})",
                                e, text,
                            ))
                        })?;

                        match parse_server_event(&value) {
                            FireworksServerEvent::Segments(updated_segments) => {
                                let partial = transcript_state.apply_segments(
                                    updated_segments,
                                    session_start.elapsed().as_millis() as u64,
                                );

                                if let Some(store) = &request_log_store {
                                    store.with_current(|log| {
                                        log.raw_transcript = Some(partial.text.clone());
                                    });
                                }

                                let _ = partial_tx.try_send(partial);
                            }
                            FireworksServerEvent::FinalCheckpoint => {
                                ws_done = true;
                            }
                            FireworksServerEvent::Error(error) => {
                                return Err(SttError::Api(format!(
                                    "Fireworks streaming error: {}",
                                    error,
                                )));
                            }
                            FireworksServerEvent::Unknown => {
                                // Ignore future provider-local events until we have a reason
                                // to project them into shared session state.
                            }
                        }
                    }
                    Ok(Some(Message::Close(_))) | Ok(None) => {
                        ws_done = true;
                    }
                    Ok(_) => {}
                    Err(SttError::Timeout) if audio_done => {
                        log::warn!(
                            "Fireworks streaming: timed out waiting for final checkpoint, using accumulated segments"
                        );
                        ws_done = true;
                    }
                    Err(error) => {
                        return Err(error);
                    }
                }
            }
        }
    }

    ws_close_best_effort(&mut ws_write, "Fireworks streaming", Duration::from_secs(3)).await;

    let final_text = transcript_state.final_text();
    if let Some(remaining) = transcript_state.finalize_remaining_commit() {
        let _ = partial_tx.try_send(PartialTranscript {
            text: final_text.clone(),
            committed_text: Some(remaining),
        });
    }

    let elapsed = session_start.elapsed();
    let response_json = transcript_state.into_response_json(num_chunks_sent, elapsed);

    if let Some(store) = &request_log_store {
        store.with_current(|log| {
            log.raw_transcript = Some(final_text.clone());
            log.stt_response_json = Some(response_json);
        });
    }

    log::info!(
        "Fireworks streaming: finalized, {} chars, {} chunks sent",
        final_text.len(),
        num_chunks_sent,
    );
    Ok(final_text)
}

fn parse_server_event(value: &JsonValue) -> FireworksServerEvent {
    if value.get("checkpoint_id").and_then(|id| id.as_str()) == Some("final") {
        return FireworksServerEvent::FinalCheckpoint;
    }

    if let Some(error) = value.get("error").and_then(|error| error.as_str()) {
        return FireworksServerEvent::Error(error.to_string());
    }

    if let Some(segments) = value
        .get("segments")
        .and_then(|segments| segments.as_array())
    {
        let parsed = segments
            .iter()
            .filter_map(|segment| {
                Some((
                    parse_segment_id(segment)?,
                    segment
                        .get("text")
                        .and_then(|text| text.as_str())
                        .unwrap_or("")
                        .to_string(),
                ))
            })
            .collect();
        return FireworksServerEvent::Segments(parsed);
    }

    FireworksServerEvent::Unknown
}

fn parse_segment_id(segment: &JsonValue) -> Option<usize> {
    segment.get("id").and_then(|id| {
        if let Some(number) = id.as_u64() {
            Some(number as usize)
        } else if let Some(text) = id.as_str() {
            text.strip_prefix("seg_")
                .or(Some(text))
                .and_then(|value| value.parse::<usize>().ok())
        } else {
            None
        }
    })
}

fn join_segments(segments: &[String]) -> String {
    segments
        .iter()
        .filter(|segment| !segment.is_empty())
        .cloned()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stt::SttProvider;

    #[test]
    fn streaming_ws_url_defaults_to_fireworks_endpoint() {
        let provider = FireworksSttProvider::new(
            "test".to_string(),
            Some("fireworks-asr-large".to_string()),
            None,
            None,
        );
        let url = streaming_ws_url(&provider).expect("ws url");
        assert_eq!(
            url,
            "wss://audio-streaming.api.fireworks.ai/v1/audio/transcriptions/streaming"
        );
        assert!(provider.supports_streaming());
    }

    #[test]
    fn streaming_ws_url_includes_language_and_override_scheme() {
        let provider = FireworksSttProvider::new(
            "test".to_string(),
            Some("fireworks-asr-v2".to_string()),
            Some("en".to_string()),
            None,
        )
        .with_api_base_url("http://localhost:8080".to_string());
        let url = streaming_ws_url(&provider).expect("ws url");
        assert_eq!(
            url,
            "ws://localhost:8080/v1/audio/transcriptions/streaming?language=en"
        );
    }

    #[test]
    fn parse_server_event_classifies_segments_checkpoint_and_errors() {
        assert_eq!(
            parse_server_event(&json!({
                "segments": [
                    { "id": "seg_0", "text": "hello" },
                    { "id": 1, "text": "world" }
                ]
            })),
            FireworksServerEvent::Segments(vec![
                (0, "hello".to_string()),
                (1, "world".to_string()),
            ]),
        );

        assert_eq!(
            parse_server_event(&json!({ "checkpoint_id": "final" })),
            FireworksServerEvent::FinalCheckpoint,
        );

        assert_eq!(
            parse_server_event(&json!({ "error": "bad auth" })),
            FireworksServerEvent::Error("bad auth".to_string()),
        );
    }

    #[test]
    fn parse_segment_id_handles_numeric_and_string_forms() {
        assert_eq!(parse_segment_id(&json!({ "id": 3 })), Some(3));
        assert_eq!(parse_segment_id(&json!({ "id": "seg_2" })), Some(2));
        assert_eq!(parse_segment_id(&json!({ "id": "5" })), Some(5));
        assert_eq!(parse_segment_id(&json!({ "text": "hello" })), None);
    }

    #[test]
    fn transcript_accumulator_commits_leading_stable_segments() {
        let mut accumulator = FireworksTranscriptAccumulator::default();

        let first = accumulator.apply_segments(vec![(0, "hello".to_string())], 10);
        assert_eq!(first.text, "hello");
        assert_eq!(first.committed_text, None);

        let _ = accumulator.apply_segments(vec![(0, "hello".to_string())], 20);
        let not_yet_committed = accumulator.apply_segments(vec![(0, "hello".to_string())], 30);
        assert_eq!(not_yet_committed.committed_text, None);

        // The stability threshold counts unchanged follow-up updates after the
        // segment is first observed, so the fourth identical frame is the one
        // that becomes commit-worthy.
        let committed = accumulator.apply_segments(vec![(0, "hello".to_string())], 40);
        assert_eq!(committed.text, "hello");
        assert_eq!(committed.committed_text.as_deref(), Some("hello"));
    }

    #[test]
    fn join_segments_filters_empty_strings() {
        let segments = vec!["Hello".to_string(), "".to_string(), "world".to_string()];
        assert_eq!(join_segments(&segments), "Hello world");
        assert_eq!(join_segments(&[]), "");
    }
}
