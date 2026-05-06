//! ElevenLabs Speech-to-Text (STT) provider implementation.
//!
//! Primary path: ElevenLabs Realtime Speech-to-Text (WebSocket)
//! - WSS wss://api.elevenlabs.io/v1/speech-to-text/realtime
//! - Audio chunks are sent as JSON `input_audio_chunk` messages
//! - Results are returned as `partial_transcript` and `committed_transcript` events
//!
//! Fallback path (legacy models): ElevenLabs "Create transcript" endpoint (HTTP multipart)
//! - POST https://api.elevenlabs.io/v1/speech-to-text
//!
//! Docs:
//! - Realtime: https://elevenlabs.io/docs/api-reference/speech-to-text/v-1-speech-to-text-realtime
//! - Create transcript: https://elevenlabs.io/docs/api-reference/speech-to-text/convert

use super::http;
use super::language;
use super::streaming::{
    connect_ws_split_with_timeout, is_ws_closed_error, ws_next_with_timeout, PartialTranscript,
    StreamingSttSession,
};
use super::{AudioEncoding, AudioFormat, SttError, SttProvider};
use crate::audio_normalization::{chunk_size_bytes_for_pcm_s16le, f32_to_pcm_s16le};
use crate::request_log::RequestLogStore;
use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use futures_util::SinkExt;
use reqwest::multipart;
use serde_json::json;
use serde_json::Value as JsonValue;
use std::io::Cursor;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::{client::IntoClientRequest, http::HeaderValue, Message};

/// ElevenLabs STT provider for speech-to-text.
///
/// Model ids currently supported by the endpoint include:
/// - `scribe_v2` (supports realtime streaming)
/// - `scribe_v1`
pub struct ElevenLabsSttProvider {
    client: reqwest::Client,
    api_key: String,
    model: String,
    language_code: Option<String>,
    api_base_url: String,
    request_log_store: Option<RequestLogStore>,
    /// When true, use `commit_strategy=vad` so the server auto-commits speech
    /// segments during recording (enables live output for ElevenLabs).
    use_vad_commit: bool,
}

impl ElevenLabsSttProvider {
    const DEFAULT_ELEVENLABS_API_BASE_URL: &'static str = "https://api.elevenlabs.io";
    const DEFAULT_WS_TRANSCRIPTION_TIMEOUT: Duration = Duration::from_secs(120);

    /// Create a new ElevenLabs STT provider.
    ///
    /// # Arguments
    /// * `api_key` - ElevenLabs API key
    /// * `model` - Model to use (e.g., "scribe_v1")
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn new(api_key: String, model: Option<String>, language: Option<String>) -> Self {
        let client = crate::network::build_plain_http_client_with_timeout(Duration::from_secs(60));
        let language_code = Self::normalize_language(language);

        Self {
            client,
            api_key,
            // Default to Scribe v2 (and use the realtime API under the hood).
            model: model.unwrap_or_else(|| "scribe_v2".to_string()),
            language_code,
            api_base_url: Self::DEFAULT_ELEVENLABS_API_BASE_URL.to_string(),
            request_log_store: None,
            use_vad_commit: false,
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
        let language_code = Self::normalize_language(language);
        Self {
            client,
            api_key,
            model: model.unwrap_or_else(|| "scribe_v2".to_string()),
            language_code,
            api_base_url: Self::DEFAULT_ELEVENLABS_API_BASE_URL.to_string(),
            request_log_store: None,
            use_vad_commit: false,
        }
    }

    /// Enable VAD-based commit strategy for live output.
    pub fn with_vad_commit(mut self, enabled: bool) -> Self {
        self.use_vad_commit = enabled;
        self
    }

    /// Override the API base URL (defaults to https://api.elevenlabs.io).
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

    fn ws_base_url_trimmed(&self) -> Result<String, SttError> {
        let trimmed = http::trim_base_url(&self.api_base_url).to_string();

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

    fn speech_to_text_url(&self) -> String {
        http::join_base_url(self.api_base_url_trimmed(), "/v1/speech-to-text")
    }

    fn speech_to_text_realtime_ws_url(
        &self,
        model_id: &str,
        audio_format: &str,
    ) -> Result<String, SttError> {
        let ws_base = self.ws_base_url_trimmed()?;
        let mut url = http::join_base_url(&ws_base, "/v1/speech-to-text/realtime");

        let commit_strategy = if self.use_vad_commit { "vad" } else { "manual" };

        // We keep this string-based to match the rest of the repo (see `crate::http`).
        // `model_id` and `audio_format` are controlled values in our UI/settings.
        let mut qs = vec![
            format!("model_id={}", model_id.trim()),
            format!("audio_format={}", audio_format.trim()),
            format!("commit_strategy={}", commit_strategy),
            // We currently only need text; timestamps can be added later when we add UX.
            "include_timestamps=false".to_string(),
        ];

        // For VAD mode, use tight silence thresholds for low-latency live output.
        if self.use_vad_commit {
            qs.push("vad_silence_threshold_secs=0.5".to_string());
            qs.push("min_silence_duration_ms=300".to_string());
        }

        if let Some(language_code) = self.language_code.as_deref() {
            let lc = language_code.trim();
            if !lc.is_empty() {
                qs.push(format!("language_code={}", lc));
            }
        }

        url.push('?');
        url.push_str(&qs.join("&"));
        Ok(url)
    }

    fn normalize_language(language: Option<String>) -> Option<String> {
        language::normalize_language_code(language)
    }

    fn should_use_realtime_api(&self) -> bool {
        // ElevenLabs realtime STT is documented around the Scribe v2 realtime model.
        // We map the user-facing "scribe_v2" choice to "scribe_v2_realtime".
        let m = self.model.trim().to_lowercase();
        m == "scribe_v2" || m == "scribe_v2_realtime"
    }

    fn realtime_model_id(&self) -> &'static str {
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

                // Decode to i16 samples.
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

                // Downmix to mono if needed.
                let mut mono: Vec<i16> = Vec::new();
                if channels == 1 {
                    mono = samples_i16;
                } else {
                    let mut i = 0;
                    while i + channels <= samples_i16.len() {
                        let frame = &samples_i16[i..i + channels];
                        let sum: i32 = frame.iter().map(|&v| v as i32).sum();
                        let avg = (sum / channels as i32) as i16;
                        mono.push(avg);
                        i += channels;
                    }
                }

                let mut pcm = Vec::with_capacity(mono.len() * 2);
                for s in mono {
                    pcm.extend_from_slice(&s.to_le_bytes());
                }

                Ok((pcm, sample_rate))
            }
        }
    }

    async fn transcribe_realtime_ws(
        &self,
        audio: &[u8],
        format: &AudioFormat,
    ) -> Result<String, SttError> {
        let (pcm, sample_rate) = Self::decode_to_pcm_s16le_mono(audio, format)?;
        let audio_format = Self::realtime_audio_format_for_sample_rate(sample_rate)?;
        let ws_url = self.speech_to_text_realtime_ws_url(self.realtime_model_id(), audio_format)?;

        if let Some(store) = &self.request_log_store {
            let request_json = json!({
                "provider": "elevenlabs",
                "endpoint": ws_url,
                "content_type": "websocket-json",
                "fields": {
                    "model_id": self.realtime_model_id(),
                    "audio_format": audio_format,
                    "language_code": self.language_code.clone(),
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
            .into_client_request()
            .map_err(|e| SttError::NetworkMessage(format!("WS request build failed: {}", e)))?;

        request.headers_mut().insert(
            "xi-api-key",
            HeaderValue::from_str(&self.api_key).map_err(|e| {
                SttError::Config(format!("Invalid ElevenLabs API key header: {}", e))
            })?,
        );

        let (mut ws_write, mut ws_read) =
            connect_ws_split_with_timeout(request, Self::DEFAULT_WS_TRANSCRIPTION_TIMEOUT).await?;

        // Chunk sizing: 0.5s is a good compromise between overhead and latency.
        // (Docs recommend 0.1s - 1s.)
        let chunk_size = chunk_size_bytes_for_pcm_s16le(sample_rate, 1, 500, 3_200, 262_144);

        let mut num_chunks = 0usize;
        for (idx, chunk) in pcm.chunks(chunk_size).enumerate() {
            let is_last = idx + 1 == pcm.len().div_ceil(chunk_size);
            let msg = json!({
                "message_type": "input_audio_chunk",
                "audio_base_64": STANDARD.encode(chunk),
                "sample_rate": sample_rate,
                "commit": is_last,
            });
            ws_write
                .send(Message::Text(msg.to_string().into()))
                .await
                .map_err(|e| SttError::NetworkMessage(format!("WS send failed: {}", e)))?;
            num_chunks += 1;
        }

        // Collect committed transcript(s).
        let mut committed: Vec<String> = Vec::new();
        loop {
            let Some(msg) =
                ws_next_with_timeout(&mut ws_read, Self::DEFAULT_WS_TRANSCRIPTION_TIMEOUT).await?
            else {
                break;
            };
            match msg {
                Message::Text(text) => {
                    let v: JsonValue = serde_json::from_str(&text).map_err(|e| {
                        SttError::Api(format!(
                            "ElevenLabs realtime: failed to parse JSON message: {} (raw={})",
                            e, text
                        ))
                    })?;
                    let msg_type = v.get("message_type").and_then(|t| t.as_str()).unwrap_or("");

                    match msg_type {
                        "session_started" => {
                            // ignore
                        }
                        "partial_transcript" => {
                            // We don't surface partials yet (Live mode comes later).
                        }
                        "committed_transcript" => {
                            if let Some(t) = v.get("text").and_then(|t| t.as_str()) {
                                if !t.trim().is_empty() {
                                    committed.push(t.to_string());
                                }
                            }
                            // For our current usage (buffered audio, commit on last chunk),
                            // a single committed transcript is expected.
                            break;
                        }
                        "committed_transcript_with_timestamps" => {
                            if let Some(t) = v.get("text").and_then(|t| t.as_str()) {
                                if !t.trim().is_empty() {
                                    committed.push(t.to_string());
                                }
                            }
                            break;
                        }
                        other if other.ends_with("_error") || other == "error" => {
                            let msg = v
                                .get("message")
                                .and_then(|m| m.as_str())
                                .unwrap_or("Unknown error");
                            return Err(SttError::Api(format!(
                                "ElevenLabs realtime error ({}): {}",
                                other, msg
                            )));
                        }
                        _ => {
                            // Ignore unknown events.
                        }
                    }
                }
                Message::Close(_) => break,
                _ => {
                    // Ignore binary/ping/pong.
                }
            }
        }

        // Best-effort close.
        let _ = ws_write.send(Message::Close(None)).await;

        if let Some(store) = &self.request_log_store {
            let response_json = json!({
                "committed_transcripts": committed,
                "chunks_sent": num_chunks,
            });
            store.with_current(|log| {
                log.stt_response_json = Some(response_json);
            });
        }

        Ok(committed.join(" "))
    }

    async fn transcribe_batch_http(&self, audio: &[u8]) -> Result<String, SttError> {
        if let Some(store) = &self.request_log_store {
            let request_json = json!({
                "provider": "elevenlabs",
                "endpoint": self.speech_to_text_url(),
                "content_type": "multipart/form-data",
                "fields": {
                    "model_id": self.model,
                    "language_code": self.language_code.clone(),
                    // We intentionally omit optional advanced fields (diarization, timestamps, etc.)
                    // until the app has UX for them.
                },
                "file": {
                    "name": "audio.wav",
                    "mime": "audio/wav",
                    "bytes": audio.len(),
                    "data": "<binary audio omitted>",
                }
            });

            store.with_current(|log| {
                log.stt_request_json = Some(request_json);
            });
        }

        let part = multipart::Part::bytes(audio.to_vec())
            .file_name("audio.wav")
            .mime_str("audio/wav")
            .map_err(|e| SttError::Audio(format!("Failed to create multipart: {}", e)))?;

        let mut form = multipart::Form::new()
            .part("file", part)
            .text("model_id", self.model.clone());

        if let Some(language_code) = self.language_code.as_deref() {
            form = form.text("language_code", language_code.to_string());
        }

        let response = self
            .client
            .post(self.speech_to_text_url())
            .header("xi-api-key", &self.api_key)
            .multipart(form)
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
                "ElevenLabs STT API error ({}): {}",
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

        let text = result["text"].as_str().unwrap_or("").to_string();
        Ok(text)
    }

    /// Start a concurrent streaming STT session.
    ///
    /// The returned `StreamingSttSession` accepts mono f32 audio chunks via its
    /// `audio_tx` channel. When the channel is closed (recording stops), the
    /// background task commits the audio and waits for the final transcript.
    ///
    /// Partial transcripts are emitted through `partial_rx` as the server
    /// returns them during recording.
    async fn start_streaming_session(
        &self,
        sample_rate: u32,
    ) -> Result<StreamingSttSession, SttError> {
        let audio_format = Self::realtime_audio_format_for_sample_rate(sample_rate)?;
        let ws_url = self.speech_to_text_realtime_ws_url(self.realtime_model_id(), audio_format)?;

        let mut request = ws_url
            .clone()
            .into_client_request()
            .map_err(|e| SttError::NetworkMessage(format!("WS request build failed: {}", e)))?;

        request.headers_mut().insert(
            "xi-api-key",
            HeaderValue::from_str(&self.api_key).map_err(|e| {
                SttError::Config(format!("Invalid ElevenLabs API key header: {}", e))
            })?,
        );

        let (ws_write, ws_read) =
            connect_ws_split_with_timeout(request, Self::DEFAULT_WS_TRANSCRIPTION_TIMEOUT).await?;

        // Generous buffer: ~60s of audio at 10 chunks/sec should never fill up.
        let (audio_tx, audio_rx) = mpsc::channel::<Vec<f32>>(1024);
        let (partial_tx, partial_rx) = mpsc::channel::<PartialTranscript>(256);

        let request_log_store = self.request_log_store.clone();
        let ws_url_for_log = ws_url.clone();
        let realtime_model_id = self.realtime_model_id().to_string();
        let language_code = self.language_code.clone();

        // Log the streaming session start.
        if let Some(store) = &request_log_store {
            let request_json = json!({
                "provider": "elevenlabs",
                "endpoint": ws_url_for_log,
                "content_type": "websocket-json-streaming",
                "mode": "concurrent",
                "fields": {
                    "model_id": realtime_model_id,
                    "audio_format": audio_format,
                    "language_code": language_code,
                    "commit_strategy": if self.use_vad_commit { "vad" } else { "manual" },
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

        let use_vad = self.use_vad_commit;

        let task = tokio::spawn(Self::run_streaming_task(
            ws_write,
            ws_read,
            audio_rx,
            partial_tx,
            sample_rate,
            request_log_store,
            use_vad,
        ));

        Ok(StreamingSttSession::new(audio_tx, partial_rx, task))
    }

    /// Background task: reads f32 chunks from `audio_rx`, sends PCM over the WS,
    /// collects partials, and returns the final committed transcript.
    async fn run_streaming_task(
        mut ws_write: super::streaming::WsWrite,
        mut ws_read: super::streaming::WsRead,
        mut audio_rx: mpsc::Receiver<Vec<f32>>,
        partial_tx: mpsc::Sender<PartialTranscript>,
        sample_rate: u32,
        request_log_store: Option<RequestLogStore>,
        use_vad: bool,
    ) -> Result<String, SttError> {
        // Use a smaller chunk duration for streaming (100ms) for lower latency.
        let target_chunk_bytes = chunk_size_bytes_for_pcm_s16le(sample_rate, 1, 100, 1_600, 32_768);

        let session_start = std::time::Instant::now();
        let mut pcm_buffer: Vec<u8> = Vec::new();
        let mut num_chunks_sent: usize = 0;
        let mut committed: Vec<String> = Vec::new();
        let mut logged_partials: Vec<JsonValue> = Vec::new();

        // We run two concurrent loops:
        // 1) Reading f32 audio from the capture thread, converting to PCM, and sending over WS
        // 2) Reading WS messages for partials / session events
        //
        // We use a select! loop that terminates when the audio channel closes.
        let mut audio_done = false;
        let mut ws_done = false;

        loop {
            if ws_done {
                break;
            }
            if audio_done {
                // Only the WS branch is active from here.
            }

            tokio::select! {
                // Read audio chunks from the capture thread.
                audio_chunk = audio_rx.recv(), if !audio_done => {
                    match audio_chunk {
                        Some(f32_samples) => {
                            let pcm = f32_to_pcm_s16le(&f32_samples);
                            pcm_buffer.extend_from_slice(&pcm);

                            // Send chunks when we've accumulated enough.
                            while pcm_buffer.len() >= target_chunk_bytes {
                                let chunk: Vec<u8> = pcm_buffer.drain(..target_chunk_bytes).collect();
                                let msg = json!({
                                    "message_type": "input_audio_chunk",
                                    "audio_base_64": STANDARD.encode(&chunk),
                                    "sample_rate": sample_rate,
                                    "commit": false,
                                });
                                match ws_write.send(Message::Text(msg.to_string().into())).await {
                                    Ok(()) => { num_chunks_sent += 1; }
                                    Err(e) if is_ws_closed_error(&e) => {
                                        log::warn!("ElevenLabs streaming: WS closed while sending audio, finishing early");
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
                            // Send any remaining buffered PCM, then commit.
                            if !pcm_buffer.is_empty() {
                                let msg = json!({
                                    "message_type": "input_audio_chunk",
                                    "audio_base_64": STANDARD.encode(&pcm_buffer),
                                    "sample_rate": sample_rate,
                                    "commit": true,
                                });
                                match ws_write.send(Message::Text(msg.to_string().into())).await {
                                    Ok(()) => { num_chunks_sent += 1; }
                                    Err(e) if is_ws_closed_error(&e) => {
                                        log::warn!("ElevenLabs streaming: WS closed while sending final audio");
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
                            } else {
                                // Send explicit commit with no audio.
                                let msg = json!({
                                    "message_type": "commit_audio",
                                });
                                match ws_write.send(Message::Text(msg.to_string().into())).await {
                                    Ok(()) => {}
                                    Err(e) if is_ws_closed_error(&e) => {
                                        log::warn!("ElevenLabs streaming: WS closed while sending commit");
                                        audio_done = true;
                                        ws_done = true;
                                        continue;
                                    }
                                    Err(e) => {
                                        return Err(SttError::NetworkMessage(format!("WS send failed: {}", e)));
                                    }
                                }
                            }
                            audio_done = true;
                        }
                    }
                }

                // Read WS messages (partials, committed, errors).
                ws_msg = ws_next_with_timeout(&mut ws_read, Self::DEFAULT_WS_TRANSCRIPTION_TIMEOUT), if !ws_done => {
                    match ws_msg? {
                        Some(Message::Text(text)) => {
                            let v: JsonValue = serde_json::from_str(&text).map_err(|e| {
                                SttError::Api(format!(
                                    "ElevenLabs streaming: failed to parse JSON: {} (raw={})",
                                    e, text
                                ))
                            })?;
                            let msg_type = v.get("message_type").and_then(|t| t.as_str()).unwrap_or("");

                            match msg_type {
                                "session_started" => {
                                    log::debug!("ElevenLabs streaming: session started");
                                    if let Some(store) = &request_log_store {
                                        store.with_current(|log| {
                                            log.info("Streaming STT session connected");
                                        });
                                    }
                                }
                                "partial_transcript" => {
                                    if let Some(t) = v.get("text").and_then(|t| t.as_str()) {
                                        if !t.trim().is_empty() {
                                            let elapsed = session_start.elapsed().as_millis() as u64;
                                            logged_partials.push(json!({
                                                "text": t,
                                                "elapsed_ms": elapsed,
                                            }));
                                            // Build the full accumulated text for the overlay.
                                            let full_text = if committed.is_empty() {
                                                t.to_string()
                                            } else {
                                                format!("{} {}", committed.join(" "), t)
                                            };
                                            // Update raw_transcript live so the UI streams it.
                                            if let Some(store) = &request_log_store {
                                                store.with_current(|log| {
                                                    log.raw_transcript = Some(full_text.clone());
                                                });
                                            }
                                            let _ = partial_tx.try_send(PartialTranscript {
                                                text: full_text,
                                                committed_text: None,
                                            });
                                        }
                                    }
                                }
                                "committed_transcript" | "committed_transcript_with_timestamps" => {
                                    let transcript_text = v.get("text")
                                        .and_then(|t| t.as_str())
                                        .unwrap_or("")
                                        .to_string();
                                    let is_nonempty = !transcript_text.trim().is_empty();
                                    if is_nonempty {
                                        committed.push(transcript_text.clone());
                                    }

                                    if use_vad && !audio_done {
                                        // VAD mode: this is a mid-recording commit.
                                        // Send committed text for live output.
                                        if is_nonempty {
                                            let full_text = committed.join(" ");
                                            if let Some(store) = &request_log_store {
                                                store.with_current(|log| {
                                                    log.raw_transcript = Some(full_text.clone());
                                                });
                                            }
                                            let _ = partial_tx.try_send(PartialTranscript {
                                                text: full_text,
                                                committed_text: Some(transcript_text),
                                            });
                                        }
                                    } else {
                                        // Manual mode, or VAD after audio_done (final commit).
                                        ws_done = true;
                                    }
                                }
                                other if other.ends_with("_error") || other == "error" => {
                                    let msg = v.get("message").and_then(|m| m.as_str()).unwrap_or("Unknown error");
                                    return Err(SttError::Api(format!(
                                        "ElevenLabs streaming error ({}): {}", other, msg
                                    )));
                                }
                                _ => {
                                    // Ignore unknown events.
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

        // Best-effort close.
        let _ = ws_write.send(Message::Close(None)).await;

        if let Some(store) = &request_log_store {
            let total_duration_ms = session_start.elapsed().as_millis() as u64;
            let response_json = json!({
                "committed_transcripts": committed,
                "chunks_sent": num_chunks_sent,
                "mode": "concurrent",
                "session_duration_ms": total_duration_ms,
                "partial_transcripts": logged_partials,
            });
            store.with_current(|log| {
                log.stt_response_json = Some(response_json);
            });
        }

        let final_text = committed.join(" ");
        log::info!(
            "ElevenLabs streaming: finalized, {} chars, {} chunks sent",
            final_text.len(),
            num_chunks_sent
        );
        Ok(final_text)
    }

    pub fn with_request_log_store(mut self, store: Option<RequestLogStore>) -> Self {
        self.request_log_store = store;
        self
    }
}

#[async_trait]
impl SttProvider for ElevenLabsSttProvider {
    async fn transcribe(&self, audio: &[u8], format: &AudioFormat) -> Result<String, SttError> {
        if self.should_use_realtime_api() {
            return self.transcribe_realtime_ws(audio, format).await;
        }

        // Legacy models currently use the batch HTTP endpoint.
        self.transcribe_batch_http(audio).await
    }

    fn name(&self) -> &'static str {
        "elevenlabs"
    }

    fn supports_streaming(&self) -> bool {
        self.should_use_realtime_api()
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
        let provider = ElevenLabsSttProvider::new("test-key".to_string(), None, None);
        assert_eq!(provider.name(), "elevenlabs");
        assert_eq!(provider.model, "scribe_v2");
    }

    #[test]
    fn test_provider_with_custom_model() {
        let provider =
            ElevenLabsSttProvider::new("test-key".to_string(), Some("scribe_v1".to_string()), None);
        assert_eq!(provider.model, "scribe_v1");
    }
}
