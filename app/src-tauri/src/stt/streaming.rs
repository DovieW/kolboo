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
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
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

/// Capture never waits for a provider queue. If a chunk cannot be delivered,
/// mark this input incomplete and stop its provider rather than accepting a
/// transcript with missing audio. All sender clones share that failure state.
#[derive(Clone, Debug)]
pub struct StreamingAudioSender {
    sender: mpsc::Sender<Vec<f32>>,
    failed: Arc<AtomicBool>,
    provider: tokio::task::AbortHandle,
}

impl StreamingAudioSender {
    pub(crate) fn fail_input(&self) {
        self.failed.store(true, Ordering::SeqCst);
        self.provider.abort();
    }

    pub fn try_send(&self, audio: Vec<f32>) -> Result<(), mpsc::error::TrySendError<Vec<f32>>> {
        if self.failed.load(Ordering::SeqCst) {
            return Err(mpsc::error::TrySendError::Closed(audio));
        }
        self.sender
            .try_send(audio)
            .inspect_err(|_| self.fail_input())
    }

    /// Offline callers (such as CLI file playback) can await capacity. A
    /// closed provider still marks incomplete input, exactly like capture.
    pub async fn send(&self, audio: Vec<f32>) -> Result<(), mpsc::error::SendError<Vec<f32>>> {
        if self.failed.load(Ordering::SeqCst) {
            return Err(mpsc::error::SendError(audio));
        }
        self.sender
            .send(audio)
            .await
            .inspect_err(|_| self.fail_input())
    }

    #[cfg(test)]
    fn is_closed(&self) -> bool {
        self.sender.is_closed()
    }
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
    pub audio_tx: StreamingAudioSender,

    /// Receive partial transcript updates while recording.
    /// This is `Option` so it can be taken out for a consumer task while
    /// the session itself is stored elsewhere.
    pub partial_rx: Option<mpsc::Receiver<PartialTranscript>>,

    /// Background task that runs the provider protocol.
    /// Resolves to the final committed transcript.
    task: OwnedStreamingTask<Result<String, SttError>>,

    /// The preview/live-insertion consumer belongs to the same session. On
    /// success it must drain before final output; on failure/drop it is aborted.
    partial_consumer: Option<OwnedStreamingTask<()>>,
    partial_consumer_start: Option<tokio::sync::oneshot::Sender<()>>,
}

/// A JoinHandle detaches on drop by default. This guard travels with the
/// finalization future too, so cancelling that future cannot orphan a provider.
struct OwnedStreamingTask<T>(tokio::task::JoinHandle<T>);

impl<T> Drop for OwnedStreamingTask<T> {
    fn drop(&mut self) {
        self.0.abort();
    }
}

impl StreamingSttSession {
    /// Create a new streaming session from its components.
    pub fn new(
        audio_tx: mpsc::Sender<Vec<f32>>,
        partial_rx: mpsc::Receiver<PartialTranscript>,
        task: tokio::task::JoinHandle<Result<String, SttError>>,
    ) -> Self {
        Self {
            audio_tx: StreamingAudioSender {
                sender: audio_tx,
                failed: Arc::new(AtomicBool::new(false)),
                provider: task.abort_handle(),
            },
            partial_rx: Some(partial_rx),
            task: OwnedStreamingTask(task),
            partial_consumer: None,
            partial_consumer_start: None,
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

    /// Replacing a consumer aborts the previous task and retires its permit.
    fn set_partial_consumer(&mut self, task: tokio::task::JoinHandle<()>) {
        self.partial_consumer = Some(OwnedStreamingTask(task));
        self.partial_consumer_start.take();
    }

    /// Prepare user-visible work without running it. The pipeline releases
    /// this barrier under its state lock only after accepting this connection.
    pub(crate) fn defer_partial_consumer(
        &mut self,
        consumer: impl std::future::Future<Output = ()> + Send + 'static,
    ) {
        let (start, ready) = tokio::sync::oneshot::channel();
        self.set_partial_consumer(tokio::spawn(async move {
            if ready.await.is_ok() {
                consumer.await;
            }
        }));
        self.partial_consumer_start = Some(start);
    }

    pub(crate) fn activate_partial_consumer(&mut self) {
        if let Some(start) = self.partial_consumer_start.take() {
            let _ = start.send(());
        }
    }

    /// Finalize the session: drop the audio sender, then await the final transcript.
    ///
    /// This will cause the background task to see the channel close, send a commit
    /// to the server, and wait for the final committed transcript.
    pub async fn finalize(self) -> Result<String, SttError> {
        self.finalize_with_timeout(Duration::from_secs(60)).await
    }

    /// Bound the tail after recording stops, independently of the session's
    /// recording duration. Timeout/cancellation drops the owned task guard.
    pub async fn finalize_with_timeout(self, deadline: Duration) -> Result<String, SttError> {
        let input_failed = self.audio_tx.failed.clone();
        // Drop audio_tx to signal end-of-audio.
        drop(self.audio_tx);
        let mut task = self.task;
        let mut partial_consumer = self.partial_consumer;
        let result = timeout(deadline, async {
            // Join errors can contain a panic's payload. Do not expose it.
            let text = (&mut task.0)
                .await
                .map_err(|_| SttError::NetworkMessage("Streaming provider task failed".into()))??;
            if let Some(consumer) = partial_consumer.as_mut() {
                (&mut consumer.0).await.map_err(|_| {
                    SttError::NetworkMessage("Streaming partial consumer task failed".into())
                })?;
            }
            Ok(text)
        })
        .await
        .map_err(|_| SttError::Timeout)?;
        if input_failed.load(Ordering::SeqCst) {
            return Err(SttError::Audio(
                "Streaming audio could not be delivered completely. The recording is available for recovery.".into(),
            ));
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::ProxyMode;

    /// The returned sender closes only when the spawned provider future drops.
    /// A startup barrier removes scheduling/timing assumptions from abort tests.
    async fn pending_session() -> (StreamingSttSession, tokio::sync::oneshot::Sender<()>) {
        let (audio_tx, audio_rx) = mpsc::channel(1);
        let (_partial_tx, partial_rx) = mpsc::channel(1);
        let (started, ready) = tokio::sync::oneshot::channel();
        let (stop, wait) = tokio::sync::oneshot::channel::<()>();
        let task = tokio::spawn(async move {
            let _audio_rx = audio_rx;
            started.send(()).unwrap();
            let _ = wait.await;
            Ok("synthetic completed transcript".into())
        });
        ready.await.unwrap();
        (StreamingSttSession::new(audio_tx, partial_rx, task), stop)
    }

    #[tokio::test]
    async fn dropping_a_session_aborts_the_provider_even_with_a_sender_clone() {
        let (session, mut stop) = pending_session().await;
        let sender = session.audio_tx.clone();
        assert!(!sender.is_closed());
        drop(session);
        stop.closed().await;
        assert!(sender.is_closed());
    }

    async fn pending_consumer() -> (
        tokio::task::JoinHandle<()>,
        tokio::sync::oneshot::Sender<()>,
    ) {
        let (started, ready) = tokio::sync::oneshot::channel();
        let (stop, wait) = tokio::sync::oneshot::channel::<()>();
        let task = tokio::spawn(async move {
            started.send(()).unwrap();
            let _ = wait.await;
        });
        ready.await.unwrap();
        (task, stop)
    }

    #[tokio::test]
    async fn replacing_or_dropping_a_consumer_aborts_it_with_the_session() {
        let (mut session, mut provider_stop) = pending_session().await;
        let (task, mut first_stop) = pending_consumer().await;
        session.set_partial_consumer(task);
        let (task, mut second_stop) = pending_consumer().await;
        session.set_partial_consumer(task);
        first_stop.closed().await;
        assert!(!second_stop.is_closed());
        drop(session);
        second_stop.closed().await;
        provider_stop.closed().await;
        assert!(second_stop.is_closed());
    }

    #[tokio::test]
    async fn a_deferred_consumer_cannot_run_before_acceptance_or_after_rejection() {
        for accepted in [true, false] {
            let (mut session, provider_stop) = pending_session().await;
            let (started, mut notified) = tokio::sync::oneshot::channel();
            session.defer_partial_consumer(notify_consumer_started(started));
            assert!(matches!(
                notified.try_recv(),
                Err(tokio::sync::oneshot::error::TryRecvError::Empty)
            ));
            if accepted {
                session.activate_partial_consumer();
                notified.await.unwrap();
                // Releasing twice is harmless and cannot repeat insertion.
                session.activate_partial_consumer();
            } else {
                // A rejected permit cannot execute the supplied future.
                session.partial_consumer_start.take();
                assert!(notified.await.is_err());
            }
            provider_stop.send(()).unwrap();
            assert_eq!(
                session.finalize().await.unwrap(),
                "synthetic completed transcript"
            );
        }
    }

    fn notify_consumer_started(
        started: tokio::sync::oneshot::Sender<()>,
    ) -> impl std::future::Future<Output = ()> {
        async move {
            started.send(()).unwrap();
        }
    }

    #[tokio::test]
    async fn dropping_an_unaccepted_session_drops_user_visible_work_without_running_it() {
        let (mut session, mut provider_stop) = pending_session().await;
        let (started, notified) = tokio::sync::oneshot::channel();
        session.defer_partial_consumer(notify_consumer_started(started));
        drop(session);
        assert!(notified.await.is_err());
        provider_stop.closed().await;
    }

    #[tokio::test]
    async fn finalization_drains_the_partial_consumer_before_returning_text() {
        let (mut session, provider_stop) = pending_session().await;
        let mut partials = session.take_partial_rx().unwrap();
        // The synthetic provider drops its partial sender immediately. Model
        // slow insertion after receipt without relying on scheduler sleeps.
        let (draining, drain_started) = tokio::sync::oneshot::channel();
        let (insert, insert_ready) = tokio::sync::oneshot::channel::<()>();
        let inserted = Arc::new(AtomicBool::new(false));
        let flag = inserted.clone();
        session.set_partial_consumer(tokio::spawn(async move {
            assert!(partials.recv().await.is_none());
            draining.send(()).unwrap();
            insert_ready.await.unwrap();
            flag.store(true, Ordering::SeqCst);
        }));
        provider_stop.send(()).unwrap();
        let finalized = tokio::spawn(session.finalize());
        drain_started.await.unwrap();
        assert!(!finalized.is_finished());
        assert!(!inserted.load(Ordering::SeqCst));
        insert.send(()).unwrap();
        assert_eq!(
            finalized.await.unwrap().unwrap(),
            "synthetic completed transcript"
        );
        assert!(inserted.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn cancelling_finalization_after_provider_completion_aborts_the_consumer() {
        use std::{
            future::Future,
            task::{Context, Poll, Waker},
        };
        let (audio_tx, _audio_rx) = mpsc::channel(1);
        let (_partial_tx, partial_rx) = mpsc::channel(1);
        let (completed, completion) = tokio::sync::oneshot::channel();
        let provider = tokio::spawn(async move {
            completed.send(()).unwrap();
            Ok("completed provider".into())
        });
        completion.await.unwrap();
        let mut session = StreamingSttSession::new(audio_tx, partial_rx, provider);
        let (task, mut consumer_stop) = pending_consumer().await;
        session.set_partial_consumer(task);
        // Awaiting the provider's JoinHandle in the finalize future is now
        // ready; the owned consumer remains pending.
        let mut finalized = Box::pin(session.finalize());
        assert!(matches!(
            finalized
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop())),
            Poll::Pending
        ));
        drop(finalized);
        consumer_stop.closed().await;
        assert!(consumer_stop.is_closed());
    }

    #[tokio::test(start_paused = true)]
    async fn provider_and_consumer_share_one_finalization_deadline() {
        let (audio_tx, _audio_rx) = mpsc::channel(1);
        let (_partial_tx, partial_rx) = mpsc::channel(1);
        let provider = tokio::spawn(async {
            tokio::time::sleep(Duration::from_secs(2)).await;
            Ok("finished provider".into())
        });
        let mut session = StreamingSttSession::new(audio_tx, partial_rx, provider);
        let (task, mut stop) = pending_consumer().await;
        session.set_partial_consumer(task);
        let start = tokio::time::Instant::now();
        assert!(matches!(
            session.finalize_with_timeout(Duration::from_secs(3)).await,
            Err(SttError::Timeout)
        ));
        assert_eq!(start.elapsed(), Duration::from_secs(3));
        stop.closed().await;
    }

    #[tokio::test]
    async fn provider_failure_aborts_the_consumer_and_consumer_panics_are_private() {
        for fail_provider in [true, false] {
            let (audio_tx, _audio_rx) = mpsc::channel(1);
            let (_partial_tx, partial_rx) = mpsc::channel(1);
            let provider = tokio::spawn(async move {
                if fail_provider {
                    Err(SttError::Api("synthetic provider failure".into()))
                } else {
                    Ok("must not accept unfinished output".into())
                }
            });
            let mut session = StreamingSttSession::new(audio_tx, partial_rx, provider);
            if fail_provider {
                let (task, mut stop) = pending_consumer().await;
                session.set_partial_consumer(task);
                assert!(
                    matches!(session.finalize().await, Err(SttError::Api(message)) if message == "synthetic provider failure")
                );
                stop.closed().await;
            } else {
                session.set_partial_consumer(tokio::spawn(async {
                    panic!("synthetic private consumer payload")
                }));
                assert!(
                    matches!(session.finalize().await, Err(SttError::NetworkMessage(message)) if message == "Streaming partial consumer task failed")
                );
            }
        }
    }

    #[tokio::test]
    async fn a_provider_finishing_before_finalization_keeps_its_result() {
        let (mut session, stop) = pending_session().await;
        let (consumer, consumer_stop) = pending_consumer().await;
        session.set_partial_consumer(consumer);
        consumer_stop.send(()).unwrap();
        stop.send(()).unwrap();
        assert_eq!(
            session.finalize().await.unwrap(),
            "synthetic completed transcript"
        );
    }

    #[tokio::test]
    async fn queue_overflow_is_nonblocking_and_invalidates_the_whole_stream() {
        let (session, mut stop) = pending_session().await;
        let sender = session.audio_tx.clone();
        assert!(sender.try_send(vec![1.0]).is_ok());
        assert!(matches!(
            sender.try_send(vec![2.0]),
            Err(mpsc::error::TrySendError::Full(audio)) if audio == vec![2.0]
        ));
        assert!(matches!(
            sender.try_send(vec![3.0]),
            Err(mpsc::error::TrySendError::Closed(audio)) if audio == vec![3.0]
        ));
        assert!(matches!(
            sender.send(vec![4.0]).await,
            Err(mpsc::error::SendError(audio)) if audio == vec![4.0]
        ));
        stop.closed().await;
        assert!(sender.is_closed());
        assert!(matches!(session.finalize().await,
            Err(SttError::Audio(message)) if message.contains("recording is available for recovery")));
    }

    #[tokio::test]
    async fn closed_input_cannot_turn_a_premature_provider_result_into_success() {
        for nonblocking in [true, false] {
            let (audio_tx, audio_rx) = mpsc::channel(1);
            let (_partial_tx, partial_rx) = mpsc::channel(1);
            drop(audio_rx);
            let (done, ready) = tokio::sync::oneshot::channel();
            let task = tokio::spawn(async {
                done.send(()).unwrap();
                Ok("incomplete transcript".into())
            });
            ready.await.unwrap();
            let session = StreamingSttSession::new(audio_tx, partial_rx, task);
            if nonblocking {
                assert!(matches!(session.audio_tx.try_send(vec![1.0]),
                    Err(mpsc::error::TrySendError::Closed(audio)) if audio == vec![1.0]));
            } else {
                assert!(matches!(session.audio_tx.send(vec![1.0]).await,
                    Err(mpsc::error::SendError(audio)) if audio == vec![1.0]));
            }
            assert!(matches!(session.finalize().await, Err(SttError::Audio(_))));
        }
    }

    #[tokio::test]
    async fn cancelling_finalization_aborts_the_provider_before_or_after_polling() {
        use std::{
            future::Future,
            task::{Context, Poll, Waker},
        };
        for poll in [false, true] {
            let (session, mut stop) = pending_session().await;
            let mut finalize = Box::pin(session.finalize());
            if poll {
                assert!(matches!(
                    finalize
                        .as_mut()
                        .poll(&mut Context::from_waker(Waker::noop())),
                    Poll::Pending
                ));
            }
            drop(finalize);
            stop.closed().await;
            assert!(stop.is_closed());
        }
    }

    #[tokio::test(start_paused = true)]
    async fn a_finalization_deadline_aborts_a_provider_that_never_finishes() {
        for deadline in [Some(Duration::from_secs(3)), None] {
            let (session, mut stop) = pending_session().await;
            let begin = tokio::time::Instant::now();
            let result = match deadline {
                Some(deadline) => session.finalize_with_timeout(deadline).await,
                None => session.finalize().await,
            };
            assert!(matches!(result, Err(SttError::Timeout)));
            stop.closed().await;
            assert_eq!(begin.elapsed(), deadline.unwrap_or(Duration::from_secs(60)));
        }
    }

    #[tokio::test]
    async fn finalization_closes_input_and_preserves_order_and_provider_errors() {
        let (audio_tx, mut audio_rx) = mpsc::channel(4);
        let (partial_tx, partial_rx) = mpsc::channel(1);
        let task = tokio::spawn(async move {
            let mut samples = Vec::new();
            while let Some(chunk) = audio_rx.recv().await {
                samples.extend(chunk);
            }
            assert_eq!(samples, vec![1.0, 2.0, 3.0]);
            partial_tx
                .send(PartialTranscript {
                    text: "א גוטן טאָג".into(),
                    committed_text: None,
                })
                .await
                .unwrap();
            Ok("א גוטן טאָג".into())
        });
        let mut session = StreamingSttSession::new(audio_tx, partial_rx, task);
        let mut partials = session.take_partial_rx().unwrap();
        assert!(session.take_partial_rx().is_none());
        session.audio_tx.send(vec![1.0, 2.0]).await.unwrap();
        session.audio_tx.send(vec![3.0]).await.unwrap();
        assert_eq!(session.finalize().await.unwrap(), "א גוטן טאָג");
        assert_eq!(partials.recv().await.unwrap().text, "א גוטן טאָג");
        assert!(partials.recv().await.is_none());

        let (audio_tx, _audio_rx) = mpsc::channel(1);
        let (_partial_tx, partial_rx) = mpsc::channel(1);
        let session = StreamingSttSession::new(
            audio_tx,
            partial_rx,
            tokio::spawn(async { Err(SttError::Api("synthetic error".into())) }),
        );
        assert!(
            matches!(session.finalize().await, Err(SttError::Api(message)) if message == "synthetic error")
        );
    }

    #[tokio::test]
    async fn join_errors_do_not_expose_provider_panic_payloads() {
        let (audio_tx, _audio_rx) = mpsc::channel(1);
        let (_partial_tx, partial_rx) = mpsc::channel(1);
        let session = StreamingSttSession::new(
            audio_tx,
            partial_rx,
            tokio::spawn(async { panic!("synthetic private payload") }),
        );
        assert!(
            matches!(session.finalize().await, Err(SttError::NetworkMessage(message)) if message == "Streaming provider task failed")
        );
    }

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
