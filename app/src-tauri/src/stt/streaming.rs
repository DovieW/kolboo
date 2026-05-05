//! Shared helpers for STT providers that stream audio (typically over WebSockets).
//!
//! Provider protocols differ, but there are a few cross-cutting concerns we want
//! to keep DRY as we move more providers to a streaming-input architecture:
//! - WebSocket connect + consistent timeout/error mapping
//! - Receiving the next message with a timeout
//! - Chunk sizing for PCM audio based on a target chunk duration
//! - `StreamingSttSession` abstraction for concurrent record-and-transcribe

use futures_util::stream::{SplitSink, SplitStream};
use futures_util::StreamExt;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::time::{timeout, Duration};
use tokio_tungstenite::tungstenite::http::Request;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};

use crate::stt::SttError;

pub(crate) type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;
pub(crate) type WsWrite = SplitSink<WsStream, Message>;
pub(crate) type WsRead = SplitStream<WsStream>;

/// Check if a tungstenite send error indicates the WebSocket has transitioned
/// to a closing/closed state (e.g. the server sent a `Close` frame).
///
/// When this returns `true`, callers should stop sending and treat the session
/// as finished rather than propagating a hard error — we can still return
/// whatever transcript has been accumulated so far.
pub(crate) fn is_ws_closed_error(e: &tokio_tungstenite::tungstenite::Error) -> bool {
    use tokio_tungstenite::tungstenite::Error;
    match e {
        Error::Protocol(ref reason) => {
            let msg = reason.to_string();
            msg.contains("closing") || msg.contains("closed") || msg.contains("after closing")
        }
        Error::ConnectionClosed | Error::AlreadyClosed => true,
        _ => false,
    }
}

/// Connect to a WebSocket endpoint and split into read/write halves.
///
/// This is intentionally STT-focused: it maps errors into `SttError`.
pub(crate) async fn connect_ws_split_with_timeout(
    req: Request<()>,
    connect_timeout: Duration,
) -> Result<(WsWrite, WsRead), SttError> {
    let connect_fut = connect_async(req);
    let (ws_stream, _) = timeout(connect_timeout, connect_fut)
        .await
        .map_err(|_| SttError::Timeout)?
        .map_err(|e| SttError::NetworkMessage(format!("WS connect failed: {}", e)))?;

    Ok(ws_stream.split())
}

/// Read the next WebSocket message with a timeout.
pub(crate) async fn ws_next_with_timeout(
    ws_read: &mut WsRead,
    read_timeout: Duration,
) -> Result<Option<Message>, SttError> {
    let next = timeout(read_timeout, ws_read.next())
        .await
        .map_err(|_| SttError::Timeout)?;

    let Some(msg) = next else {
        return Ok(None);
    };

    let msg = msg.map_err(|e| SttError::NetworkMessage(format!("WS recv failed: {}", e)))?;
    Ok(Some(msg))
}

/// Compute a chunk size (in bytes) for PCM s16le audio.
///
/// The returned value is aligned to a whole sample frame boundary.
pub(crate) fn chunk_size_bytes_for_pcm_s16le(
    sample_rate: u32,
    channels: u8,
    target_chunk_ms: u32,
    min_bytes: usize,
    max_bytes: usize,
) -> usize {
    let channels = channels.max(1) as usize;
    let bytes_per_frame = channels.saturating_mul(2); // i16 per channel
    let bytes_per_second = (sample_rate as usize)
        .saturating_mul(channels)
        .saturating_mul(2);

    let mut chunk = bytes_per_second.saturating_mul(target_chunk_ms as usize) / 1000;

    chunk = chunk.clamp(min_bytes, max_bytes);

    // Align up to a full sample frame.
    if bytes_per_frame > 0 {
        let rem = chunk % bytes_per_frame;
        if rem != 0 {
            chunk = chunk.saturating_add(bytes_per_frame - rem);
        }
    }

    chunk
}

/// Convert mono f32 samples into little-endian PCM s16le bytes.
///
/// Streaming providers all receive the same capture-thread representation
/// (`[-1.0, 1.0]` mono f32). Keeping this conversion here avoids tiny provider
/// copies drifting apart while still leaving each provider responsible for its
/// protocol-specific framing.
pub(crate) fn f32_to_pcm_s16le(samples: &[f32]) -> Vec<u8> {
    let mut pcm = Vec::with_capacity(samples.len() * 2);
    for &s in samples {
        let clamped = s.clamp(-1.0, 1.0);
        let val = (clamped * i16::MAX as f32).round() as i16;
        pcm.extend_from_slice(&val.to_le_bytes());
    }
    pcm
}

/// Resample mono f32 audio with simple linear interpolation.
///
/// This helper is intentionally small and dependency-free: STT streaming only
/// needs predictable sample-rate conversion before providers encode PCM. More
/// advanced resampling would belong behind a separately proven adapter seam.
pub(crate) fn resample_linear(input: &[f32], input_rate: u32, output_rate: u32) -> Vec<f32> {
    if input_rate == output_rate || input.is_empty() {
        return input.to_vec();
    }

    let ratio = input_rate as f64 / output_rate as f64;
    let output_len = (input.len() as f64 / ratio).ceil() as usize;
    let mut output = Vec::with_capacity(output_len);

    for i in 0..output_len {
        let src_idx = i as f64 * ratio;
        let idx0 = src_idx.floor() as usize;
        let idx1 = (idx0 + 1).min(input.len() - 1);
        let frac = (src_idx - idx0 as f64) as f32;
        output.push(input[idx0] * (1.0 - frac) + input[idx1] * frac);
    }

    output
}

// ─────────────────────────────────────────────────────────────────────────────
// Concurrent streaming session
// ─────────────────────────────────────────────────────────────────────────────

/// A partial transcript update emitted while the user is still recording.
#[derive(Debug, Clone)]
pub struct PartialTranscript {
    /// The current best-effort transcript text (full accumulated text).
    pub text: String,
    /// When `Some`, this partial represents a newly committed chunk that
    /// the live-output feature should paste immediately.
    /// The value is the *new* text that was just committed (not the full
    /// accumulated text).
    pub committed_text: Option<String>,
}

/// A handle to an active concurrent streaming STT session.
///
/// Audio chunks (mono f32 samples at the capture sample rate) are sent via
/// `audio_tx`. Partial transcripts can be read from `partial_rx`. When
/// recording finishes, call `finalize()` to close the audio channel and
/// retrieve the final transcript.
///
/// The background task converts f32 → provider-specific encoding and handles
/// the WebSocket protocol. Dropping `audio_tx` signals end-of-input.
pub struct StreamingSttSession {
    /// Send mono f32 audio samples (at capture sample rate).
    /// Dropping this signals end-of-audio.
    pub audio_tx: mpsc::Sender<Vec<f32>>,

    /// Receive partial transcript updates while recording.
    /// This is `Option` so it can be taken out for a consumer task while
    /// the session itself is stored elsewhere.
    pub partial_rx: Option<mpsc::Receiver<PartialTranscript>>,

    /// Background task that runs the provider protocol.
    /// Resolves to the final committed transcript.
    task: tokio::task::JoinHandle<Result<String, SttError>>,
}

impl StreamingSttSession {
    /// Create a new streaming session from its components.
    pub fn new(
        audio_tx: mpsc::Sender<Vec<f32>>,
        partial_rx: mpsc::Receiver<PartialTranscript>,
        task: tokio::task::JoinHandle<Result<String, SttError>>,
    ) -> Self {
        Self {
            audio_tx,
            partial_rx: Some(partial_rx),
            task,
        }
    }

    /// Take the partial transcript receiver out of the session.
    ///
    /// This is useful when you want to spawn a consumer task that forwards
    /// partials (e.g. as Tauri events) while the session itself is stored
    /// elsewhere for later finalization.
    pub fn take_partial_rx(&mut self) -> Option<mpsc::Receiver<PartialTranscript>> {
        self.partial_rx.take()
    }

    /// Finalize the session: drop the audio sender, then await the final transcript.
    ///
    /// This will cause the background task to see the channel close, send a commit
    /// to the server, and wait for the final committed transcript.
    pub async fn finalize(self) -> Result<String, SttError> {
        // Drop audio_tx to signal end-of-audio.
        drop(self.audio_tx);

        // Await the background task.
        self.task
            .await
            .map_err(|e| SttError::NetworkMessage(format!("Streaming task panicked: {}", e)))?
    }
}

#[cfg(test)]
mod tests {
    use super::{chunk_size_bytes_for_pcm_s16le, f32_to_pcm_s16le, resample_linear};

    #[test]
    fn chunk_size_respects_target_and_alignment_mono() {
        // 16kHz mono s16le = 32,000 bytes/sec.
        // 100ms target -> ~3,200 bytes.
        let chunk = chunk_size_bytes_for_pcm_s16le(16_000, 1, 100, 2_048, 32_768);
        assert_eq!(chunk, 3_200);
        assert_eq!(chunk % 2, 0);
    }

    #[test]
    fn chunk_size_aligns_to_frame_stereo() {
        // Stereo frames are 4 bytes; ensure alignment.
        let chunk = chunk_size_bytes_for_pcm_s16le(16_000, 2, 100, 2_048, 32_768);
        assert_eq!(chunk % 4, 0);
        assert!(chunk >= 2_048);
    }

    #[test]
    fn chunk_size_clamps() {
        // Very small target ms gets clamped up.
        let small = chunk_size_bytes_for_pcm_s16le(16_000, 1, 1, 2_048, 32_768);
        assert_eq!(small, 2_048);

        // Very large target ms gets clamped down.
        let large = chunk_size_bytes_for_pcm_s16le(48_000, 1, 10_000, 2_048, 32_768);
        assert_eq!(large, 32_768);
    }

    #[test]
    fn f32_to_pcm_s16le_converts_and_clamps_samples() {
        let samples = vec![0.0_f32, 1.0, -1.0, 0.5, 2.0, -2.0];
        let pcm = f32_to_pcm_s16le(&samples);

        assert_eq!(pcm.len(), samples.len() * 2);
        assert_eq!(i16::from_le_bytes([pcm[0], pcm[1]]), 0);
        assert_eq!(i16::from_le_bytes([pcm[2], pcm[3]]), i16::MAX);
        assert_eq!(i16::from_le_bytes([pcm[4], pcm[5]]), -i16::MAX);
        assert_eq!(i16::from_le_bytes([pcm[6], pcm[7]]), 16_384);
        assert_eq!(i16::from_le_bytes([pcm[8], pcm[9]]), i16::MAX);
        assert_eq!(i16::from_le_bytes([pcm[10], pcm[11]]), -i16::MAX);
    }

    #[test]
    fn f32_to_pcm_s16le_handles_empty_input() {
        assert!(f32_to_pcm_s16le(&[]).is_empty());
    }

    #[test]
    fn resample_linear_passthrough_for_same_rate() {
        let input = vec![1.0_f32, 2.0, 3.0];
        assert_eq!(resample_linear(&input, 16_000, 16_000), input);
    }

    #[test]
    fn resample_linear_downsamples_with_existing_provider_math() {
        // 48kHz → 16kHz = 3:1 ratio.
        let input: Vec<f32> = (0..300).map(|i| i as f32).collect();
        let output = resample_linear(&input, 48_000, 16_000);

        assert_eq!(output.len(), 100);
        assert_eq!(output[0], 0.0);
        assert_eq!(output[1], 3.0);
        assert_eq!(output[99], 297.0);
    }

    #[test]
    fn resample_linear_upsamples_with_interpolation() {
        let input = vec![0.0_f32, 10.0];
        let output = resample_linear(&input, 1, 4);

        assert_eq!(output, vec![0.0, 2.5, 5.0, 7.5, 10.0, 10.0, 10.0, 10.0]);
    }
}
