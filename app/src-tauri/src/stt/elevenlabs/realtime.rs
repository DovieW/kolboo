use super::ElevenLabsSttProvider;
use crate::audio_normalization::{chunk_size_bytes_for_pcm_s16le, f32_to_pcm_s16le};
use crate::request_log::RequestLogStore;
use crate::stt::streaming::{
    connect_ws_split_with_timeout, ws_close_best_effort, ws_next_with_timeout,
    ws_send_json_text_with_closed_handling, PartialTranscript, StreamingSttSession, WsRead,
    WsSendOutcome, WsWrite,
};
use crate::stt::{AudioEncoding, AudioFormat, SttError};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use futures_util::SinkExt;
use serde_json::{json, Value as JsonValue};
use std::io::Cursor;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::{client::IntoClientRequest, http::HeaderValue, Message};

const DEFAULT_WS_TRANSCRIPTION_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Debug, Clone, PartialEq, Eq)]
enum ElevenLabsServerEvent {
    SessionStarted,
    PartialTranscript(String),
    CommittedTranscript(String),
    Error { kind: String, message: String },
    Unknown(String),
}

#[derive(Debug, Default)]
struct ElevenLabsTranscriptAccumulator {
    committed_segments: Vec<String>,
    logged_partials: Vec<JsonValue>,
}

impl ElevenLabsTranscriptAccumulator {
    fn apply_partial(&mut self, transcript: &str, elapsed_ms: u64) -> Option<PartialTranscript> {
        let trimmed = transcript.trim();
        if trimmed.is_empty() {
            return None;
        }

        let full_text = join_segments(&self.committed_segments, trimmed);
        self.logged_partials.push(json!({
            "text": trimmed,
            "full_text": &full_text,
            "committed": false,
            "elapsed_ms": elapsed_ms,
            "committed_segments": self.committed_segments.len(),
        }));

        Some(PartialTranscript {
            text: full_text,
            committed_text: None,
        })
    }

    fn apply_committed(&mut self, transcript: &str, elapsed_ms: u64) -> Option<PartialTranscript> {
        let trimmed = transcript.trim();
        if trimmed.is_empty() {
            return None;
        }

        self.committed_segments.push(trimmed.to_string());
        let full_text = join_segments(&self.committed_segments, "");
        self.logged_partials.push(json!({
            "text": trimmed,
            "full_text": &full_text,
            "committed": true,
            "elapsed_ms": elapsed_ms,
            "committed_segments": self.committed_segments.len(),
        }));

        Some(PartialTranscript {
            text: full_text,
            committed_text: Some(trimmed.to_string()),
        })
    }

    fn final_text(&self) -> String {
        join_segments(&self.committed_segments, "")
    }

    fn into_response_json(self, num_chunks_sent: usize, elapsed: Duration) -> JsonValue {
        json!({
            "committed_transcripts": self.committed_segments,
            "chunks_sent": num_chunks_sent,
            "mode": "concurrent",
            "session_duration_ms": elapsed.as_millis() as u64,
            "partial_transcripts": self.logged_partials,
        })
    }
}

pub(super) fn speech_to_text_realtime_ws_url(
    provider: &ElevenLabsSttProvider,
    model_id: &str,
    audio_format: &str,
) -> Result<String, SttError> {
    let ws_base = ws_base_url_trimmed(provider)?;
    let mut url = super::http::join_base_url(&ws_base, "/v1/speech-to-text/realtime");

    let commit_strategy = if provider.use_vad_commit {
        "vad"
    } else {
        "manual"
    };

    // Keep query construction string-based to match the rest of the repo's URL helpers.
    let mut qs = vec![
        format!("model_id={}", model_id.trim()),
        format!("audio_format={}", audio_format.trim()),
        format!("commit_strategy={}", commit_strategy),
        "include_timestamps=false".to_string(),
    ];

    // Tight silence thresholds keep VAD live-output responsive without creating a separate
    // provider-family policy seam.
    if provider.use_vad_commit {
        qs.push("vad_silence_threshold_secs=0.5".to_string());
        qs.push("min_silence_duration_ms=300".to_string());
    }

    if let Some(language_code) = provider.language_code.as_deref() {
        let lc = language_code.trim();
        if !lc.is_empty() {
            qs.push(format!("language_code={}", lc));
        }
    }

    url.push('?');
    url.push_str(&qs.join("&"));
    Ok(url)
}

pub(super) async fn transcribe_realtime_ws(
    provider: &ElevenLabsSttProvider,
    audio: &[u8],
    format: &AudioFormat,
) -> Result<String, SttError> {
    let (pcm, sample_rate) = decode_to_pcm_s16le_mono(audio, format)?;
    let audio_format = realtime_audio_format_for_sample_rate(sample_rate)?;
    let ws_url = speech_to_text_realtime_ws_url(provider, realtime_model_id(), audio_format)?;

    if let Some(store) = &provider.request_log_store {
        let request_json = json!({
            "provider": "elevenlabs",
            "endpoint": ws_url,
            "content_type": "websocket-json",
            "fields": {
                "model_id": realtime_model_id(),
                "audio_format": audio_format,
                "language_code": provider.language_code.clone(),
                "commit_strategy": "manual",
            },
            "audio": {
                "encoding": "pcm_s16le_mono",
                "sample_rate": sample_rate,
                "bytes": pcm.len(),
            }
        });

        store.with_current(|log| {
            log.stt_request_json = Some(request_json);
        });
    }

    let mut request = ws_url
        .clone()
        .into_client_request()
        .map_err(|e| SttError::NetworkMessage(format!("WS request build failed: {}", e)))?;

    request.headers_mut().insert(
        "xi-api-key",
        HeaderValue::from_str(&provider.api_key)
            .map_err(|e| SttError::Config(format!("Invalid ElevenLabs API key header: {}", e)))?,
    );

    let (mut ws_write, mut ws_read) = connect_ws_split_with_timeout(
        request,
        DEFAULT_WS_TRANSCRIPTION_TIMEOUT,
        &provider.proxy_settings,
    )
    .await?;

    // 0.5s chunks balance WS overhead with latency for buffered uploads.
    let chunk_size = chunk_size_bytes_for_pcm_s16le(sample_rate, 1, 500, 3_200, 262_144);
    let mut num_chunks = 0usize;

    for (idx, chunk) in pcm.chunks(chunk_size).enumerate() {
        let is_last = idx + 1 == pcm.len().div_ceil(chunk_size);
        let msg = input_audio_chunk_message(STANDARD.encode(chunk), sample_rate, is_last);

        ws_write
            .send(Message::Text(msg.to_string().into()))
            .await
            .map_err(|e| SttError::NetworkMessage(format!("WS send failed: {}", e)))?;
        num_chunks += 1;
    }

    let mut committed_segments: Vec<String> = Vec::new();
    loop {
        let Some(msg) =
            ws_next_with_timeout(&mut ws_read, DEFAULT_WS_TRANSCRIPTION_TIMEOUT).await?
        else {
            break;
        };

        match msg {
            Message::Text(text) => {
                let value: JsonValue = serde_json::from_str(&text).map_err(|e| {
                    SttError::Api(format!(
                        "ElevenLabs realtime: failed to parse JSON message: {} (raw={})",
                        e, text
                    ))
                })?;

                match parse_server_event(&value) {
                    ElevenLabsServerEvent::SessionStarted => {}
                    ElevenLabsServerEvent::PartialTranscript(_) => {
                        // Buffered websocket transcription only cares about the final committed result.
                    }
                    ElevenLabsServerEvent::CommittedTranscript(text) => {
                        let trimmed = text.trim();
                        if !trimmed.is_empty() {
                            committed_segments.push(trimmed.to_string());
                        }
                        break;
                    }
                    ElevenLabsServerEvent::Error { kind, message } => {
                        return Err(SttError::Api(format!(
                            "ElevenLabs realtime error ({}): {}",
                            kind, message
                        )));
                    }
                    ElevenLabsServerEvent::Unknown(_) => {
                        // Ignore future event types until the product has a reason to care.
                    }
                }
            }
            Message::Close(_) => break,
            _ => {
                // Ignore binary/ping/pong.
            }
        }
    }

    ws_close_best_effort(
        &mut ws_write,
        "ElevenLabs realtime buffered transcription",
        Duration::from_secs(3),
    )
    .await;

    if let Some(store) = &provider.request_log_store {
        let response_json = json!({
            "committed_transcripts": committed_segments,
            "chunks_sent": num_chunks,
        });
        store.with_current(|log| {
            log.stt_response_json = Some(response_json);
        });
    }

    Ok(join_segments(&committed_segments, ""))
}

pub(super) async fn start_streaming_session(
    provider: &ElevenLabsSttProvider,
    sample_rate: u32,
) -> Result<StreamingSttSession, SttError> {
    let audio_format = realtime_audio_format_for_sample_rate(sample_rate)?;
    let ws_url = speech_to_text_realtime_ws_url(provider, realtime_model_id(), audio_format)?;

    let mut request = ws_url
        .clone()
        .into_client_request()
        .map_err(|e| SttError::NetworkMessage(format!("WS request build failed: {}", e)))?;

    request.headers_mut().insert(
        "xi-api-key",
        HeaderValue::from_str(&provider.api_key)
            .map_err(|e| SttError::Config(format!("Invalid ElevenLabs API key header: {}", e)))?,
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
    let language_code = provider.language_code.clone();

    if let Some(store) = &request_log_store {
        let request_json = json!({
            "provider": "elevenlabs",
            "endpoint": ws_url,
            "content_type": "websocket-json-streaming",
            "mode": "concurrent",
            "fields": {
                "model_id": realtime_model_id(),
                "audio_format": audio_format,
                "language_code": language_code,
                "commit_strategy": if provider.use_vad_commit { "vad" } else { "manual" },
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
        provider.use_vad_commit,
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
    use_vad: bool,
) -> Result<String, SttError> {
    // 100ms chunks keep live output responsive without overwhelming the socket.
    let target_chunk_bytes = chunk_size_bytes_for_pcm_s16le(sample_rate, 1, 100, 1_600, 32_768);

    let session_start = std::time::Instant::now();
    let mut pcm_buffer: Vec<u8> = Vec::new();
    let mut num_chunks_sent: usize = 0;
    let mut transcript_state = ElevenLabsTranscriptAccumulator::default();
    let mut audio_done = false;
    let mut ws_done = false;

    loop {
        if ws_done {
            break;
        }

        tokio::select! {
            audio_chunk = audio_rx.recv(), if !audio_done => {
                match audio_chunk {
                    Some(f32_samples) => {
                        let pcm = f32_to_pcm_s16le(&f32_samples);
                        pcm_buffer.extend_from_slice(&pcm);

                        while pcm_buffer.len() >= target_chunk_bytes {
                            let chunk: Vec<u8> = pcm_buffer.drain(..target_chunk_bytes).collect();
                            let msg = input_audio_chunk_message(STANDARD.encode(&chunk), sample_rate, false);
                            match ws_send_json_text_with_closed_handling(
                                &mut ws_write,
                                &msg,
                                "ElevenLabs streaming: send audio chunk",
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
                            let msg = input_audio_chunk_message(STANDARD.encode(&pcm_buffer), sample_rate, true);
                            match ws_send_json_text_with_closed_handling(
                                &mut ws_write,
                                &msg,
                                "ElevenLabs streaming: send final audio chunk",
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
                        } else {
                            let commit = json!({"message_type": "commit_audio"});
                            match ws_send_json_text_with_closed_handling(
                                &mut ws_write,
                                &commit,
                                "ElevenLabs streaming: send commit",
                            ).await? {
                                WsSendOutcome::Sent => {}
                                WsSendOutcome::Closed => {
                                    audio_done = true;
                                    ws_done = true;
                                    continue;
                                }
                            }
                        }

                        audio_done = true;
                    }
                }
            }

            ws_msg = ws_next_with_timeout(&mut ws_read, DEFAULT_WS_TRANSCRIPTION_TIMEOUT), if !ws_done => {
                match ws_msg? {
                    Some(Message::Text(text)) => {
                        let value: JsonValue = serde_json::from_str(&text).map_err(|e| {
                            SttError::Api(format!(
                                "ElevenLabs streaming: failed to parse JSON: {} (raw={})",
                                e, text
                            ))
                        })?;

                        match parse_server_event(&value) {
                            ElevenLabsServerEvent::SessionStarted => {
                                log::debug!("ElevenLabs streaming: session started");
                                if let Some(store) = &request_log_store {
                                    store.with_current(|log| {
                                        log.info("Streaming STT session connected");
                                    });
                                }
                            }
                            ElevenLabsServerEvent::PartialTranscript(text) => {
                                if let Some(partial) = transcript_state.apply_partial(
                                    &text,
                                    session_start.elapsed().as_millis() as u64,
                                ) {
                                    if let Some(store) = &request_log_store {
                                        store.with_current(|log| {
                                            log.raw_transcript = Some(partial.text.clone());
                                        });
                                    }
                                    let _ = partial_tx.try_send(partial);
                                }
                            }
                            ElevenLabsServerEvent::CommittedTranscript(text) => {
                                let committed_update = transcript_state.apply_committed(
                                    &text,
                                    session_start.elapsed().as_millis() as u64,
                                );

                                if use_vad && !audio_done {
                                    if let Some(partial) = committed_update {
                                        if let Some(store) = &request_log_store {
                                            store.with_current(|log| {
                                                log.raw_transcript = Some(partial.text.clone());
                                            });
                                        }
                                        let _ = partial_tx.try_send(partial);
                                    }
                                } else {
                                    // In manual mode, or once recording has stopped in VAD mode, the next
                                    // committed chunk is the final transcript for this session.
                                    ws_done = true;
                                }
                            }
                            ElevenLabsServerEvent::Error { kind, message } => {
                                return Err(SttError::Api(format!(
                                    "ElevenLabs streaming error ({}): {}",
                                    kind, message
                                )));
                            }
                            ElevenLabsServerEvent::Unknown(message_type) => {
                                log::debug!(
                                    "ElevenLabs streaming: unknown message type: {}",
                                    message_type,
                                );
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

    ws_close_best_effort(
        &mut ws_write,
        "ElevenLabs streaming",
        Duration::from_secs(3),
    )
    .await;

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
        "ElevenLabs streaming: finalized, {} chars, {} chunks sent",
        final_text.len(),
        num_chunks_sent
    );
    Ok(final_text)
}

fn ws_base_url_trimmed(provider: &ElevenLabsSttProvider) -> Result<String, SttError> {
    let trimmed = super::http::trim_base_url(&provider.api_base_url).to_string();

    if trimmed.starts_with("wss://") || trimmed.starts_with("ws://") {
        return Ok(trimmed);
    }

    if let Some(rest) = trimmed.strip_prefix("https://") {
        return Ok(format!("wss://{}", rest));
    }

    if let Some(rest) = trimmed.strip_prefix("http://") {
        return Ok(format!("ws://{}", rest));
    }

    Err(SttError::Config(format!(
        "Unsupported ElevenLabs base URL scheme: {}",
        trimmed
    )))
}

fn realtime_model_id() -> &'static str {
    "scribe_v2_realtime"
}

fn realtime_audio_format_for_sample_rate(sample_rate: u32) -> Result<&'static str, SttError> {
    match sample_rate {
		8000 => Ok("pcm_8000"),
		16000 => Ok("pcm_16000"),
		22050 => Ok("pcm_22050"),
		24000 => Ok("pcm_24000"),
		44100 => Ok("pcm_44100"),
		48000 => Ok("pcm_48000"),
		other => Err(SttError::Audio(format!(
			"Unsupported sample rate for ElevenLabs realtime STT: {} (expected one of 8000, 16000, 22050, 24000, 44100, 48000)",
			other
		))),
	}
}

fn decode_to_pcm_s16le_mono(
    audio: &[u8],
    format: &AudioFormat,
) -> Result<(Vec<u8>, u32), SttError> {
    match format.encoding {
        AudioEncoding::Pcm16 => {
            if format.channels != 1 {
                return Err(SttError::Audio(format!(
                    "ElevenLabs realtime expects mono PCM; got channels={}",
                    format.channels
                )));
            }
            Ok((audio.to_vec(), format.sample_rate))
        }
        AudioEncoding::Wav => {
            let cursor = Cursor::new(audio);
            let mut reader =
                hound::WavReader::new(cursor).map_err(|e| SttError::Audio(e.to_string()))?;

            let spec = reader.spec();
            let sample_rate = spec.sample_rate;
            let channels = spec.channels as usize;

            if channels == 0 {
                return Err(SttError::Audio("WAV has 0 channels".to_string()));
            }

            let mut samples_i16: Vec<i16> = Vec::new();
            match (spec.sample_format, spec.bits_per_sample) {
                (hound::SampleFormat::Int, 16) => {
                    for s in reader.samples::<i16>() {
                        samples_i16.push(
                            s.map_err(|e| SttError::Audio(format!("WAV read failed: {}", e)))?,
                        );
                    }
                }
                (hound::SampleFormat::Int, 32) => {
                    for s in reader.samples::<i32>() {
                        let s =
                            s.map_err(|e| SttError::Audio(format!("WAV read failed: {}", e)))?;
                        samples_i16.push((s >> 16) as i16);
                    }
                }
                (hound::SampleFormat::Float, 32) => {
                    for s in reader.samples::<f32>() {
                        let s =
                            s.map_err(|e| SttError::Audio(format!("WAV read failed: {}", e)))?;
                        let clipped = s.clamp(-1.0, 1.0);
                        samples_i16.push((clipped * i16::MAX as f32).round() as i16);
                    }
                }
                other => {
                    return Err(SttError::Audio(format!(
						"Unsupported WAV format for ElevenLabs realtime: {:?} bits_per_sample={} (expected 16-bit PCM)",
						other.0, other.1
					)));
                }
            }

            let mono: Vec<i16> = if channels == 1 {
                samples_i16
            } else {
                let mut mono = Vec::new();
                let mut i = 0;
                while i + channels <= samples_i16.len() {
                    let frame = &samples_i16[i..i + channels];
                    let sum: i32 = frame.iter().map(|&v| v as i32).sum();
                    let avg = (sum / channels as i32) as i16;
                    mono.push(avg);
                    i += channels;
                }
                mono
            };

            let mut pcm = Vec::with_capacity(mono.len() * 2);
            for sample in mono {
                pcm.extend_from_slice(&sample.to_le_bytes());
            }

            Ok((pcm, sample_rate))
        }
    }
}

fn parse_server_event(value: &JsonValue) -> ElevenLabsServerEvent {
    let message_type = value
        .get("message_type")
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .to_string();

    match message_type.as_str() {
        "session_started" => ElevenLabsServerEvent::SessionStarted,
        "partial_transcript" => ElevenLabsServerEvent::PartialTranscript(
            value
                .get("text")
                .and_then(|text| text.as_str())
                .unwrap_or("")
                .to_string(),
        ),
        "committed_transcript" | "committed_transcript_with_timestamps" => {
            ElevenLabsServerEvent::CommittedTranscript(
                value
                    .get("text")
                    .and_then(|text| text.as_str())
                    .unwrap_or("")
                    .to_string(),
            )
        }
        other if other.ends_with("_error") || other == "error" => ElevenLabsServerEvent::Error {
            kind: other.to_string(),
            message: value
                .get("message")
                .and_then(|message| message.as_str())
                .unwrap_or("Unknown error")
                .to_string(),
        },
        _ => ElevenLabsServerEvent::Unknown(message_type),
    }
}

fn input_audio_chunk_message(audio_base_64: String, sample_rate: u32, commit: bool) -> JsonValue {
    json!({
        "message_type": "input_audio_chunk",
        "audio_base_64": audio_base_64,
        "sample_rate": sample_rate,
        "commit": commit,
    })
}

fn join_segments(committed: &[String], trailing_partial: &str) -> String {
    let mut parts: Vec<&str> = committed
        .iter()
        .map(|segment| segment.as_str())
        .filter(|segment| !segment.is_empty())
        .collect();

    let trimmed_partial = trailing_partial.trim();
    if !trimmed_partial.is_empty() {
        parts.push(trimmed_partial);
    }

    parts.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speech_to_text_realtime_ws_url_defaults_to_manual_commit() {
        let provider = ElevenLabsSttProvider::new("k".to_string(), None, None);
        let url = speech_to_text_realtime_ws_url(&provider, realtime_model_id(), "pcm_16000")
            .expect("ws url");

        assert!(url.starts_with("wss://api.elevenlabs.io/v1/speech-to-text/realtime?"));
        assert!(url.contains("model_id=scribe_v2_realtime"));
        assert!(url.contains("audio_format=pcm_16000"));
        assert!(url.contains("commit_strategy=manual"));
        assert!(url.contains("include_timestamps=false"));
        assert!(!url.contains("vad_silence_threshold_secs"));
    }

    #[test]
    fn speech_to_text_realtime_ws_url_includes_vad_and_language() {
        let provider = ElevenLabsSttProvider::new("k".to_string(), None, Some("en".to_string()))
            .with_vad_commit(true);
        let url = speech_to_text_realtime_ws_url(&provider, realtime_model_id(), "pcm_44100")
            .expect("ws url");

        assert!(url.contains("commit_strategy=vad"));
        assert!(url.contains("vad_silence_threshold_secs=0.5"));
        assert!(url.contains("min_silence_duration_ms=300"));
        assert!(url.contains("language_code=en"));
    }

    #[test]
    fn parse_server_event_classifies_core_events() {
        assert_eq!(
            parse_server_event(&json!({ "message_type": "session_started" })),
            ElevenLabsServerEvent::SessionStarted,
        );
        assert_eq!(
            parse_server_event(&json!({
                "message_type": "partial_transcript",
                "text": "hello"
            })),
            ElevenLabsServerEvent::PartialTranscript("hello".to_string()),
        );
        assert_eq!(
            parse_server_event(&json!({
                "message_type": "committed_transcript_with_timestamps",
                "text": "hello world"
            })),
            ElevenLabsServerEvent::CommittedTranscript("hello world".to_string()),
        );
        assert_eq!(
            parse_server_event(&json!({
                "message_type": "auth_error",
                "message": "bad key"
            })),
            ElevenLabsServerEvent::Error {
                kind: "auth_error".to_string(),
                message: "bad key".to_string(),
            },
        );
    }

    #[test]
    fn transcript_accumulator_builds_partial_and_committed_text() {
        let mut accumulator = ElevenLabsTranscriptAccumulator::default();

        let partial = accumulator
            .apply_partial("hello", 12)
            .expect("partial transcript");
        assert_eq!(partial.text, "hello");
        assert_eq!(partial.committed_text, None);

        let committed = accumulator
            .apply_committed("hello world", 24)
            .expect("committed transcript");
        assert_eq!(committed.text, "hello world");
        assert_eq!(committed.committed_text.as_deref(), Some("hello world"));
        assert_eq!(accumulator.final_text(), "hello world");
    }

    #[test]
    fn join_segments_filters_empty_and_whitespace_only_parts() {
        assert_eq!(
            join_segments(&["Hello.".to_string(), "World.".to_string()], ""),
            "Hello. World.",
        );
        assert_eq!(
            join_segments(&["Hello.".to_string()], " world "),
            "Hello. world"
        );
        assert_eq!(join_segments(&[], "partial"), "partial");
        assert_eq!(join_segments(&[], "   "), "");
    }
}
