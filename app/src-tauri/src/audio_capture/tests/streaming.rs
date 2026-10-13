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
        // The real buffer owns both persistence and nonblocking delivery.
        capture.buffer.lock().unwrap().append(&[sample]);
    }
    raw_sender.closed().await;
    AudioCaptureBackend::set_live_audio_tx(&mut capture, None);
    assert!(capture.buffer.lock().unwrap().live_audio_tx.is_none());
    assert!(matches!(session.finalize().await, Err(SttError::Audio(_))));
    assert_eq!(capture.buffer.lock().unwrap().snapshot(), [0.1, 0.2]);
    let (wav, _) = capture
        .stop_and_get_wav_with_diagnostics(AudioEncodeConfig::default())
        .unwrap();
    let saved = hound::WavReader::new(std::io::Cursor::new(wav)).unwrap();
    assert_eq!(saved.duration(), 2);
}

fn collecting_session() -> (
    StreamingSttSession,
    tokio::sync::oneshot::Receiver<Vec<Vec<f32>>>,
) {
    let (tx, mut rx) = tokio::sync::mpsc::channel(32);
    let (_partial_tx, partial_rx) = tokio::sync::mpsc::channel(1);
    let (done, collected) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(async move {
        let mut chunks = Vec::new();
        while let Some(chunk) = rx.recv().await {
            chunks.push(chunk);
        }
        done.send(chunks).unwrap();
        Ok("complete".into())
    });
    (StreamingSttSession::new(tx, partial_rx, task), collected)
}

#[tokio::test]
async fn late_attachment_replays_the_prefix_once_before_new_chunks() {
    for channels in [1, 2] {
        let mut capture = AudioCapture::new();
        capture.sample_rate = 16000;
        capture.channels = channels;
        capture.buffer.lock().unwrap().set_format(16000, channels);
        let interleaved = |mono: &[f32]| {
            mono.iter()
                .flat_map(|sample| std::iter::repeat_n(*sample, channels as usize))
                .collect::<Vec<_>>()
        };
        // Includes pre-roll and any audio recorded while connecting.
        capture
            .buffer
            .lock()
            .unwrap()
            .append(&interleaved(&[0.1, 0.2]));
        let (session, collected) = collecting_session();
        AudioCaptureBackend::set_live_audio_tx(&mut capture, Some(session.audio_tx.clone()));
        capture.buffer.lock().unwrap().append(&interleaved(&[0.3]));
        capture.buffer.lock().unwrap().append(&[]);
        capture
            .buffer
            .lock()
            .unwrap()
            .append(&interleaved(&[0.4, 0.5]));
        AudioCaptureBackend::set_live_audio_tx(&mut capture, None);
        assert_eq!(session.finalize().await.unwrap(), "complete");
        assert_eq!(
            collected.await.unwrap(),
            [vec![0.1, 0.2], vec![0.3], vec![0.4, 0.5]]
        );
        assert_eq!(
            capture.buffer.lock().unwrap().snapshot(),
            interleaved(&[0.1, 0.2, 0.3, 0.4, 0.5])
        );
    }
}

#[tokio::test]
async fn appending_and_attaching_share_one_atomic_audio_boundary() {
    let mut capture = AudioCapture::new();
    capture.buffer.lock().unwrap().append(&[0.1]);
    let buffer = capture.buffer.clone();
    let (appended, resume) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        // The next worker chunk cannot interleave inside prefix replay.
        resume.recv().unwrap();
        buffer.lock().unwrap().append(&[0.2]);
    });
    let (session, collected) = collecting_session();
    capture.set_live_audio_tx(Some(session.audio_tx.clone()));
    appended.send(()).unwrap();
    worker.join().unwrap();
    capture.set_live_audio_tx(None);
    assert_eq!(session.finalize().await.unwrap(), "complete");
    assert_eq!(collected.await.unwrap(), [vec![0.1], vec![0.2]]);
}

#[tokio::test]
async fn attaching_after_wrapping_or_failed_storage_rejects_incomplete_input() {
    for failed_storage in [false, true] {
        let mut capture = AudioCapture::new();
        *capture.buffer.lock().unwrap() = AudioBuffer::new(1, 1, 1.0);
        let (session, collected) = collecting_session();
        {
            let mut buffer = capture.buffer.lock().unwrap();
            if failed_storage {
                buffer.journal_failed = true;
            } else {
                buffer.append(&[0.1, 0.2]);
                assert_eq!(buffer.snapshot(), [0.2]);
            }
        }
        capture.set_live_audio_tx(Some(session.audio_tx.clone()));
        capture.set_live_audio_tx(None);
        assert!(matches!(session.finalize().await, Err(SttError::Audio(_))));
        assert!(collected.await.is_err());
    }
}

#[tokio::test]
async fn poison_or_reset_cannot_leave_an_old_input_running() {
    for action in ["poison", "reset", "replace", "format", "storage"] {
        let mut capture = AudioCapture::new();
        let (session, collected) = collecting_session();
        if action == "poison" {
            let buffer = capture.buffer.clone();
            let _ = std::panic::catch_unwind(|| {
                let _guard = buffer.lock().unwrap();
                panic!("synthetic lock failure");
            });
            capture.set_live_audio_tx(Some(session.audio_tx.clone()));
            // Clearing an already poisoned capture must remain harmless.
            capture.set_live_audio_tx(None);
        } else {
            capture.set_live_audio_tx(Some(session.audio_tx.clone()));
            {
                let mut buffer = capture.buffer.lock().unwrap();
                match action {
                    "reset" => buffer.reset_for_recording(30.0),
                    "replace" => {
                        let (replacement, _) = collecting_session();
                        buffer.set_live_audio_tx(Some(replacement.audio_tx.clone()));
                        drop(replacement);
                    }
                    "format" => buffer.set_format(16000, 2),
                    "storage" => {
                        buffer.journal_failed = true;
                        buffer.append(&[0.1]);
                    }
                    _ => unreachable!(),
                }
            }
            capture.set_live_audio_tx(None);
        }
        assert!(matches!(session.finalize().await, Err(SttError::Audio(_))));
        assert!(collected.await.is_err());
    }
}

#[tokio::test]
async fn storage_failures_during_streaming_abort_the_provider() {
    for phase in ["create", "append", "lock"] {
        let dir = tempfile::tempdir().unwrap();
        let mut capture = AudioCapture::new();
        let (session, collected) = collecting_session();
        capture.set_live_audio_tx(Some(session.audio_tx.clone()));
        {
            let mut buffer = capture.buffer.lock().unwrap();
            let path = dir.path().join("capture.pcm");
            if phase == "create" {
                // create_new must reject an existing file without altering it.
                std::fs::write(&path, b"retained").unwrap();
                buffer.recovery_path = Some(path);
            } else {
                let journal = Arc::new(StdMutex::new(
                    journal::Journal::create(&path, 16000, 1).unwrap(),
                ));
                if phase == "lock" {
                    let _ = std::panic::catch_unwind(|| {
                        let _guard = journal.lock().unwrap();
                        panic!("synthetic journal lock failure");
                    });
                }
                // Append uses the capture's 44100-Hz format, so an unpoisoned
                // 16000-Hz journal also fails closed instead of mixing formats.
                buffer.journal = Some(journal);
            }
            buffer.append(&[0.1]);
            assert!(buffer.journal_failed);
        }
        capture.set_live_audio_tx(None);
        assert!(matches!(session.finalize().await, Err(SttError::Audio(_))));
        assert!(collected.await.is_err());
    }
}
