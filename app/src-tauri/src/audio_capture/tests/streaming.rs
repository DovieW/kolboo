//! Capture storage/sender boundary, without CPAL devices or network access.
use super::*;
use crate::stt::{StreamingSttSession, SttError};

#[tokio::test]
async fn live_capture_queue_failure_preserves_audio_for_recovery() {
    let mut capture = AudioCapture::new();
    capture.sample_rate = 16000;
    capture.channels = 1;
    capture.buffer.lock().unwrap().set_format(16000, 1);
    let (audio_tx, audio_rx) = tokio::sync::mpsc::channel(1);
    let raw_sender = audio_tx.clone();
    let (_partial_tx, partial_rx) = tokio::sync::mpsc::channel(1);
    let (started, ready) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(async move {
        let _audio_rx = audio_rx;
        started.send(()).unwrap();
        std::future::pending::<Result<String, SttError>>().await
    });
    ready.await.unwrap();
    let session = StreamingSttSession::new(audio_tx, partial_rx, task);
    AudioCaptureBackend::set_live_audio_tx(&mut capture, Some(session.audio_tx.clone()));
    for sample in [0.1, 0.2] {
        // Mirror the worker's ordering: save first, then nonblocking delivery.
        capture.buffer.lock().unwrap().append(&[sample]);
        let live = capture.live_audio_tx.lock().unwrap();
        let result = live.as_ref().unwrap().try_send(vec![sample]);
        assert_eq!(result.is_ok(), sample == 0.1);
    }
    raw_sender.closed().await;
    AudioCaptureBackend::set_live_audio_tx(&mut capture, None);
    assert!(capture.live_audio_tx.lock().unwrap().is_none());
    assert!(matches!(session.finalize().await, Err(SttError::Audio(_))));
    assert_eq!(capture.buffer.lock().unwrap().snapshot(), [0.1, 0.2]);
    let (wav, _) = capture
        .stop_and_get_wav_with_diagnostics(AudioEncodeConfig::default())
        .unwrap();
    let saved = hound::WavReader::new(std::io::Cursor::new(wav)).unwrap();
    assert_eq!(saved.duration(), 2);
}
