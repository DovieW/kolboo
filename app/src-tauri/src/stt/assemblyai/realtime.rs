use super::AssemblyAiSttProvider;
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
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;

#[derive(Debug, Clone, PartialEq)]
enum AssemblyAiServerEvent {
    Begin {
        id: String,
    },
    Turn {
        transcript: String,
        is_formatted: bool,
    },
    Termination {
        audio_duration_seconds: Option<f64>,
        session_duration_seconds: Option<f64>,
    },
    Unknown(String),
}

#[derive(Debug, Default)]
struct AssemblyAiTranscriptAccumulator {
    committed_turns: Vec<String>,
    current_partial: String,
    logged_partials: Vec<JsonValue>,
}

impl AssemblyAiTranscriptAccumulator {
    fn apply_turn(
        &mut self,
        transcript: String,
        is_formatted: bool,
        elapsed_ms: u64,
    ) -> PartialTranscript {
        let (full_text, committed_text) = if is_formatted {
            // AssemblyAI explicitly marks turn-final messages, so we can commit them
            // immediately instead of inventing provider-agnostic heuristics here.
            if !transcript.is_empty() {
                self.committed_turns.push(transcript.clone());
            }
            self.current_partial.clear();

            let full = join_turn_texts(&self.committed_turns, "");
            let committed = if transcript.is_empty() {
                None
            } else {
                Some(transcript)
            };
            (full, committed)
        } else {
            // Interim turns are overlay-only until AssemblyAI finalizes them.
            self.current_partial = transcript;
            let full = join_turn_texts(&self.committed_turns, &self.current_partial);
            (full, None)
        };

        self.logged_partials.push(json!({
            "text": &full_text,
            "is_formatted": is_formatted,
            "elapsed_ms": elapsed_ms,
            "committed_turns": self.committed_turns.len(),
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
        self.committed_turns.push(trailing_partial.clone());

        Some(PartialTranscript {
            text: join_turn_texts(&self.committed_turns, ""),
            committed_text: Some(trailing_partial),
        })
    }

    fn final_text(&self) -> String {
        join_turn_texts(&self.committed_turns, &self.current_partial)
    }

    fn into_response_json(self, num_chunks_sent: usize, elapsed: Duration) -> JsonValue {
        json!({
            "committed_turns": self.committed_turns,
            "chunks_sent": num_chunks_sent,
            "mode": "concurrent",
            "session_duration_ms": elapsed.as_millis() as u64,
            "partial_transcripts": self.logged_partials,
        })
    }
}

pub(super) fn streaming_ws_url(
    provider: &AssemblyAiSttProvider,
    sample_rate: u32,
) -> Result<String, SttError> {
    let base = if provider.api_base_url != AssemblyAiSttProvider::DEFAULT_API_BASE_URL {
        // Test override: convert http(s) to ws(s) without teaching the rest of the
        // adapter about alternate transport schemes.
        let trimmed = provider.api_base_url_trimmed();
        let ws_base = if trimmed.starts_with("https") {
            trimmed.replacen("https", "wss", 1)
        } else if trimmed.starts_with("http") {
            trimmed.replacen("http", "ws", 1)
        } else {
            trimmed.to_string()
        };
        format!("{}/v3/ws", ws_base)
    } else {
        AssemblyAiSttProvider::DEFAULT_STREAMING_WS_URL.to_string()
    };

    let mut params = vec![
        format!("sample_rate={}", sample_rate),
        "encoding=pcm_s16le".to_string(),
        "format_turns=true".to_string(),
        // AssemblyAI requires the WS token in the query string. We still send the
        // header too because some environments accept both and the old adapter did.
        format!("token={}", provider.api_key),
    ];

    if let Some(lang) = &provider.language_code {
        let streaming_lang = match lang.as_str() {
            // Any English locale maps to `en` for the streaming API.
            value if value.starts_with("en") => "en",
            // Non-English explicit language on the multilingual model maps to `multi`.
            _ if provider.model == "universal-streaming-multilingual" => "multi",
            // The english-only model should simply rely on the server default for
            // surprising non-English inputs instead of inventing a lossy mapping.
            _ => "",
        };
        if !streaming_lang.is_empty() {
            params.push(format!("language={}", streaming_lang));
        }
    } else if provider.model == "universal-streaming-english" {
        params.push("language=en".to_string());
    }

    if provider.language_detection {
        params.push("language_detection=true".to_string());
    }

    Ok(format!("{}?{}", base, params.join("&")))
}

pub(super) async fn start_streaming_session(
    provider: &AssemblyAiSttProvider,
    sample_rate: u32,
) -> Result<StreamingSttSession, SttError> {
    let ws_url = streaming_ws_url(provider, sample_rate)?;

    let mut request = ws_url
        .clone()
        .into_client_request()
        .map_err(|error| SttError::NetworkMessage(format!("WS request build failed: {}", error)))?;

    request.headers_mut().insert(
        "Authorization",
        provider.api_key.parse().map_err(|error| {
            SttError::Config(format!("Invalid AssemblyAI API key header: {}", error))
        })?,
    );

    let (ws_write, ws_read) = connect_ws_split_with_timeout(
        request,
        AssemblyAiSttProvider::DEFAULT_WS_TIMEOUT,
        &provider.proxy_settings,
    )
    .await?;

    let (audio_tx, audio_rx) = mpsc::channel::<Vec<f32>>(1024);
    let (partial_tx, partial_rx) = mpsc::channel::<PartialTranscript>(256);

    let request_log_store = provider.request_log_store.clone();
    let model = provider.model.clone();
    let language = provider.language_code.clone();

    if let Some(store) = &request_log_store {
        let request_json = json!({
            "provider": "assemblyai",
            "endpoint": redact_token_in_url(&ws_url),
            "content_type": "websocket-binary-streaming",
            "mode": "concurrent",
            "fields": {
                "model": model,
                "language": language,
                "language_detection": provider.language_detection,
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
    // 100 ms chunks keep partials responsive without spraying tiny WS frames.
    let target_chunk_bytes = chunk_size_bytes_for_pcm_s16le(sample_rate, 1, 100, 1_600, 32_768);

    let session_start = std::time::Instant::now();
    let mut pcm_buffer: Vec<u8> = Vec::new();
    let mut num_chunks_sent: usize = 0;
    let mut transcript_state = AssemblyAiTranscriptAccumulator::default();

    let mut audio_done = false;
    let mut ws_done = false;

    loop {
        if ws_done {
            break;
        }

        let ws_timeout = if audio_done {
            AssemblyAiSttProvider::POST_TERMINATE_TIMEOUT
        } else {
            AssemblyAiSttProvider::DEFAULT_WS_TIMEOUT
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
                                "AssemblyAI streaming: send audio chunk",
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
                                "AssemblyAI streaming: send final audio chunk",
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

                        let terminate = json!({"type": "Terminate"});
                        match ws_send_json_text_with_closed_handling(
                            &mut ws_write,
                            &terminate,
                            "AssemblyAI streaming: send Terminate",
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
                        let value: JsonValue = serde_json::from_str(&text).map_err(|error| {
                            SttError::Api(format!(
                                "AssemblyAI streaming: failed to parse JSON: {} (raw={})",
                                error,
                                text,
                            ))
                        })?;

                        match parse_server_event(&value) {
                            AssemblyAiServerEvent::Begin { id } => {
                                log::info!("AssemblyAI streaming session started (id={})", id);
                            }
                            AssemblyAiServerEvent::Turn {
                                transcript,
                                is_formatted,
                            } => {
                                let elapsed = session_start.elapsed().as_millis() as u64;
                                let partial = transcript_state.apply_turn(
                                    transcript,
                                    is_formatted,
                                    elapsed,
                                );

                                if let Some(store) = &request_log_store {
                                    store.with_current(|log| {
                                        log.raw_transcript = Some(partial.text.clone());
                                    });
                                }

                                let _ = partial_tx.try_send(partial);
                            }
                            AssemblyAiServerEvent::Termination {
                                audio_duration_seconds,
                                session_duration_seconds,
                            } => {
                                log::info!(
                                    "AssemblyAI streaming session terminated (audio={:.1}s, session={:.1}s)",
                                    audio_duration_seconds.unwrap_or(0.0),
                                    session_duration_seconds.unwrap_or(0.0),
                                );
                                ws_done = true;
                            }
                            AssemblyAiServerEvent::Unknown(message_type) => {
                                log::debug!(
                                    "AssemblyAI streaming: unknown message type: {}",
                                    message_type,
                                );
                            }
                        }
                    }
                    Ok(Some(Message::Close(frame))) => {
                        log::warn!("AssemblyAI streaming: server sent Close frame {:?}", frame);
                        ws_done = true;
                    }
                    Ok(None) => {
                        log::warn!("AssemblyAI streaming: WS stream ended (None)");
                        ws_done = true;
                    }
                    Ok(_) => {
                        // Ignore binary/ping/pong messages; AssemblyAI transcript frames are JSON.
                    }
                    Err(SttError::Timeout) if audio_done => {
                        log::warn!(
                            "AssemblyAI streaming: timed out waiting for Termination, using accumulated turns"
                        );
                        ws_done = true;
                    }
                    Err(SttError::Timeout) => {
                        log::warn!(
                            "AssemblyAI streaming: WS read timed out while audio still flowing ({}s)",
                            AssemblyAiSttProvider::DEFAULT_WS_TIMEOUT.as_secs(),
                        );
                        ws_done = true;
                    }
                    Err(error) => {
                        log::error!("AssemblyAI streaming: WS read error: {}", error);
                        return Err(error);
                    }
                }
            }
        }
    }

    ws_close_best_effort(
        &mut ws_write,
        "AssemblyAI streaming",
        Duration::from_secs(3),
    )
    .await;
    drop(ws_write);

    if let Some(partial) = transcript_state.finalize_pending_partial() {
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
        "AssemblyAI streaming: finalized, {} chars, {} turns, {} chunks sent",
        final_text.len(),
        final_text.split_whitespace().count(),
        num_chunks_sent,
    );

    Ok(final_text)
}

fn redact_token_in_url(url: &str) -> String {
    let Some(index) = url.find("token=") else {
        return url.to_string();
    };

    let token_end = url[index..]
        .find('&')
        .map_or(url.len(), |offset| index + offset);
    format!("{}token=REDACTED{}", &url[..index], &url[token_end..])
}

fn parse_server_event(value: &JsonValue) -> AssemblyAiServerEvent {
    let message_type = value
        .get("type")
        .and_then(|kind| kind.as_str())
        .unwrap_or("")
        .to_string();

    match message_type.as_str() {
        "Begin" => AssemblyAiServerEvent::Begin {
            id: value
                .get("id")
                .and_then(|id| id.as_str())
                .unwrap_or("unknown")
                .to_string(),
        },
        "Turn" => AssemblyAiServerEvent::Turn {
            transcript: value
                .get("transcript")
                .and_then(|transcript| transcript.as_str())
                .unwrap_or("")
                .to_string(),
            is_formatted: value
                .get("turn_is_formatted")
                .and_then(|is_formatted| is_formatted.as_bool())
                .unwrap_or(false),
        },
        "Termination" => AssemblyAiServerEvent::Termination {
            audio_duration_seconds: value
                .get("audio_duration_seconds")
                .and_then(|duration| duration.as_f64()),
            session_duration_seconds: value
                .get("session_duration_seconds")
                .and_then(|duration| duration.as_f64()),
        },
        _ => AssemblyAiServerEvent::Unknown(message_type),
    }
}

fn join_turn_texts(committed: &[String], current_partial: &str) -> String {
    let mut parts: Vec<&str> = committed
        .iter()
        .map(|turn| turn.as_str())
        .filter(|turn| !turn.is_empty())
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
    fn streaming_ws_url_english_no_language() {
        let provider = AssemblyAiSttProvider::new(
            "my-api-key".to_string(),
            Some("universal-streaming-english".to_string()),
            None,
        );
        let url = streaming_ws_url(&provider, 48000).expect("ws url");
        assert!(url.starts_with("wss://streaming.assemblyai.com/v3/ws?"));
        assert!(url.contains("sample_rate=48000"));
        assert!(url.contains("encoding=pcm_s16le"));
        assert!(url.contains("format_turns=true"));
        assert!(url.contains("token=my-api-key"));
        assert!(url.contains("language=en"));
        assert!(!url.contains("language=en_us"));
        assert!(url.contains("language_detection=true"));
    }

    #[test]
    fn streaming_ws_url_multilingual_no_language() {
        let provider = AssemblyAiSttProvider::new(
            "key".to_string(),
            Some("universal-streaming-multilingual".to_string()),
            None,
        );
        let url = streaming_ws_url(&provider, 48000).expect("ws url");
        assert!(url.starts_with("wss://streaming.assemblyai.com/v3/ws?"));
        assert!(!url.contains("language=en_us"));
        assert!(url.contains("language_detection=true"));
    }

    #[test]
    fn streaming_ws_url_with_explicit_language() {
        let provider = AssemblyAiSttProvider::new(
            "key".to_string(),
            Some("universal-streaming-multilingual".to_string()),
            Some("fr".to_string()),
        );
        let url = streaming_ws_url(&provider, 48000).expect("ws url");
        assert!(url.contains("language=multi"));
        assert!(!url.contains("language=fr"));
        assert!(!url.contains("language_detection=true"));
    }

    #[test]
    fn streaming_ws_url_base_override() {
        let provider = AssemblyAiSttProvider::new(
            "key".to_string(),
            Some("universal-streaming-english".to_string()),
            None,
        )
        .with_api_base_url("http://localhost:9090".to_string());
        let url = streaming_ws_url(&provider, 16000).expect("ws url");
        assert!(url.starts_with("ws://localhost:9090/v3/ws?"));
    }

    #[test]
    fn parse_server_event_classifies_begin_turn_and_termination_messages() {
        assert_eq!(
            parse_server_event(&json!({
                "type": "Begin",
                "id": "session-123"
            })),
            AssemblyAiServerEvent::Begin {
                id: "session-123".to_string(),
            },
        );

        assert_eq!(
            parse_server_event(&json!({
                "type": "Turn",
                "transcript": "hello world",
                "turn_is_formatted": true
            })),
            AssemblyAiServerEvent::Turn {
                transcript: "hello world".to_string(),
                is_formatted: true,
            },
        );

        assert_eq!(
            parse_server_event(&json!({
                "type": "Termination",
                "audio_duration_seconds": 1.5,
                "session_duration_seconds": 2.0,
            })),
            AssemblyAiServerEvent::Termination {
                audio_duration_seconds: Some(1.5),
                session_duration_seconds: Some(2.0),
            },
        );

        assert_eq!(
            parse_server_event(&json!({ "type": "SomethingNew" })),
            AssemblyAiServerEvent::Unknown("SomethingNew".to_string()),
        );
    }

    #[test]
    fn transcript_accumulator_tracks_partial_and_formatted_turns() {
        let mut accumulator = AssemblyAiTranscriptAccumulator::default();

        let partial = accumulator.apply_turn("hello".to_string(), false, 12);
        assert_eq!(partial.text, "hello");
        assert_eq!(partial.committed_text, None);

        let committed = accumulator.apply_turn("hello world".to_string(), true, 24);
        assert_eq!(committed.text, "hello world");
        assert_eq!(committed.committed_text.as_deref(), Some("hello world"));
        assert_eq!(accumulator.final_text(), "hello world");
    }

    #[test]
    fn transcript_accumulator_finalizes_trailing_partial() {
        let mut accumulator = AssemblyAiTranscriptAccumulator::default();
        let _ = accumulator.apply_turn("hello".to_string(), false, 10);
        let finalized = accumulator
            .finalize_pending_partial()
            .expect("trailing partial to finalize");

        assert_eq!(finalized.text, "hello");
        assert_eq!(finalized.committed_text.as_deref(), Some("hello"));
        assert_eq!(accumulator.final_text(), "hello");
    }

    #[test]
    fn join_turn_texts_filters_empty_and_whitespace_only_parts() {
        assert_eq!(
            join_turn_texts(&["Hello.".to_string(), "World.".to_string()], ""),
            "Hello. World.",
        );
        assert_eq!(
            join_turn_texts(&["Hello.".to_string()], "world"),
            "Hello. world"
        );
        assert_eq!(join_turn_texts(&[], "partial"), "partial");
        assert_eq!(join_turn_texts(&[], ""), "");
        assert_eq!(
            join_turn_texts(
                &["Hello.".to_string(), "".to_string(), "World.".to_string()],
                "",
            ),
            "Hello. World.",
        );
        assert_eq!(join_turn_texts(&["Hello.".to_string()], "  "), "Hello.");
    }

    /// Integration test: connects to real AssemblyAI WS, sends audio, verifies message flow.
    ///
    /// Run with: `cargo test test_assemblyai_streaming_integration -- --ignored --nocapture`
    #[tokio::test]
    #[ignore]
    async fn test_assemblyai_streaming_integration() {
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

        let capture_sample_rate = 48000u32;
        let mut session = start_streaming_session(&provider, capture_sample_rate)
            .await
            .expect("Failed to start streaming session");

        let audio_tx = session.audio_tx.clone();
        let mut partial_rx = session.take_partial_rx().expect("partial receiver");

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
                        .map(|text| &text[..text.len().min(40)]),
                );
            }
            eprintln!("  partial_rx closed after {} partials", count);
            count
        });

        let chunk_samples = capture_sample_rate as usize / 10;
        let total_chunks = 30;
        let mut chunks_sent = 0u32;

        for index in 0..total_chunks {
            let mut chunk = Vec::with_capacity(chunk_samples);
            for sample_index in 0..chunk_samples {
                let t = (index * chunk_samples + sample_index) as f32 / capture_sample_rate as f32;
                let value = 0.3 * (2.0 * std::f32::consts::PI * 200.0 * t).sin()
                    + 0.2 * (2.0 * std::f32::consts::PI * 440.0 * t).sin();
                chunk.push(value);
            }
            match audio_tx.send(chunk).await {
                Ok(()) => {
                    chunks_sent += 1;
                    if chunks_sent.is_multiple_of(10) {
                        eprintln!("  audio chunk {}/{}", chunks_sent, total_chunks);
                    }
                }
                Err(_) => {
                    eprintln!(
                        "  audio_tx.send failed at chunk {} (task exited early!)",
                        index
                    );
                    break;
                }
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        eprintln!("Sent {chunks_sent}/{total_chunks} chunks, finalizing...");
        drop(audio_tx);

        let result = session.finalize().await;
        match &result {
            Ok(text) => eprintln!("Finalized OK: {} chars, text='{}'", text.len(), text),
            Err(error) => eprintln!("Finalized with error: {}", error),
        }

        let partial_count = partial_reader.await.expect("partial reader task");
        eprintln!("Total partials received: {}", partial_count);

        assert!(
            result.is_ok(),
            "Streaming session failed: {:?}",
            result.err()
        );
    }
}
