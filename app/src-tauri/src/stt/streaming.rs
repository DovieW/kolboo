//! Shared helpers for STT providers that stream audio (typically over WebSockets).
//!
//! Provider protocols differ, but there are a few cross-cutting concerns we want
//! to keep DRY as we move more providers to a streaming-input architecture:
//! - WebSocket connect + consistent timeout/error mapping
//! - Receiving the next message with a timeout
//! - Chunk sizing for PCM audio based on a target chunk duration
//! - `StreamingSttSession` abstraction for concurrent record-and-transcribe

use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::time::{timeout, Duration};
use tokio_tungstenite::tungstenite::http::Request;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

use crate::settings::ProxySettings;
use crate::stt::SttError;

pub(crate) type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;
pub(crate) type WsWrite = SplitSink<WsStream, Message>;
pub(crate) type WsRead = SplitStream<WsStream>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WsSendOutcome {
    Sent,
    Closed,
}

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

/// Describe any remaining websocket transport-policy gaps for realtime STT.
///
/// The underlying connection policy now lives in `stt/websocket_transport.rs`; keep this wrapper
/// here so callers do not need to know where the transport seam lives.
pub(crate) fn describe_websocket_transport_policy_gap(
    proxy_settings: &ProxySettings,
) -> Option<String> {
    crate::stt::websocket_transport::describe_websocket_transport_policy_gap(proxy_settings)
}

/// Connect to a WebSocket endpoint and split into read/write halves.
///
/// This is intentionally STT-focused: it maps errors into `SttError`.
pub(crate) async fn connect_ws_split_with_timeout(
    req: Request<()>,
    connect_timeout: Duration,
    proxy_settings: &ProxySettings,
) -> Result<(WsWrite, WsRead), SttError> {
    let ws_stream = crate::stt::websocket_transport::connect_ws_with_transport_policy(
        req,
        connect_timeout,
        proxy_settings,
    )
    .await?;

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

/// Send a JSON WebSocket text frame and normalize the provider-independent closed-socket path.
pub(crate) async fn ws_send_json_text_with_closed_handling(
    ws_write: &mut WsWrite,
    value: &serde_json::Value,
    context: &str,
) -> Result<WsSendOutcome, SttError> {
    ws_send_text_with_closed_handling(ws_write, value.to_string(), context).await
}

/// Send a WebSocket text frame and return `Closed` instead of forcing every Adapter to duplicate
/// tungstenite's closing/closed error matching.
pub(crate) async fn ws_send_text_with_closed_handling(
    ws_write: &mut WsWrite,
    text: String,
    context: &str,
) -> Result<WsSendOutcome, SttError> {
    match ws_write.send(Message::Text(text.into())).await {
        Ok(()) => Ok(WsSendOutcome::Sent),
        Err(e) if is_ws_closed_error(&e) => {
            log::warn!("{}: WS closed while sending", context);
            Ok(WsSendOutcome::Closed)
        }
        Err(e) => Err(SttError::NetworkMessage(format!(
            "{} failed: {}",
            context, e
        ))),
    }
}

/// Send a WebSocket binary frame and normalize the provider-independent closed-socket path.
pub(crate) async fn ws_send_binary_with_closed_handling(
    ws_write: &mut WsWrite,
    bytes: Vec<u8>,
    context: &str,
) -> Result<WsSendOutcome, SttError> {
    match ws_write.send(Message::Binary(bytes.into())).await {
        Ok(()) => Ok(WsSendOutcome::Sent),
        Err(e) if is_ws_closed_error(&e) => {
            log::warn!("{}: WS closed while sending", context);
            Ok(WsSendOutcome::Closed)
        }
        Err(e) => Err(SttError::NetworkMessage(format!(
            "{} failed: {}",
            context, e
        ))),
    }
}

/// Best-effort close for provider WebSocket sessions.
///
/// Closing noise is not a provider protocol failure after we've already collected a transcript, so
/// the transport Module owns the timeout and closed-socket handling in one place.
pub(crate) async fn ws_close_best_effort(
    ws_write: &mut WsWrite,
    context: &str,
    close_timeout: Duration,
) {
    match timeout(close_timeout, ws_write.send(Message::Close(None))).await {
        Ok(Ok(())) => {}
        Ok(Err(e)) if is_ws_closed_error(&e) => {}
        Ok(Err(e)) => log::debug!("{}: WS close send error: {}", context, e),
        Err(_) => log::debug!("{}: WS close send timed out", context),
    }
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
    use super::*;
    use crate::settings::ProxyMode;

    #[test]
    fn websocket_transport_gap_is_none_for_default_system_settings() {
        assert_eq!(
            describe_websocket_transport_policy_gap(&ProxySettings::default()),
            None
        );
    }

    #[test]
    fn websocket_transport_gap_is_none_for_supported_manual_http_proxy() {
        let mut proxy_settings = ProxySettings {
            mode: ProxyMode::Manual,
            ..ProxySettings::default()
        };
        proxy_settings.manual.proxy_url = "http://127.0.0.1:8080".to_string();

        assert_eq!(
            describe_websocket_transport_policy_gap(&proxy_settings),
            None
        );
    }

    #[test]
    fn websocket_transport_gap_mentions_https_proxy_urls() {
        let message = describe_websocket_transport_policy_gap(&ProxySettings {
            mode: ProxyMode::Manual,
            manual: ProxySettings::default().manual,
            ..ProxySettings::default()
        })
        .unwrap_or_default();

        assert!(!message.contains("no-proxy mode"));

        let mut proxy_settings = ProxySettings {
            mode: ProxyMode::Manual,
            ..ProxySettings::default()
        };
        proxy_settings.manual.proxy_url = "https://proxy.example.test:8443".to_string();

        let message = describe_websocket_transport_policy_gap(&proxy_settings)
            .expect("expected https-proxy warning");

        assert!(message.contains("HTTPS proxy URLs"));
    }
}
