use super::*;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

async fn transcribe_blocks<F, Fut>(
    wav: &[u8],
    path: Option<&Path>,
    settings_key: &str,
    cancel: &CancellationToken,
    block_frames: usize,
    upload: F,
) -> Result<SttResult, PipelineError>
where
    F: FnMut(Vec<u8>) -> Fut,
    Fut: std::future::Future<Output = Result<SttResult, PipelineError>>,
{
    transcribe_source(
        wav.to_vec().into(),
        path,
        settings_key,
        cancel,
        block_frames,
        upload,
    )
    .await
}

fn audio(frames: usize) -> Vec<u8> {
    let mut cursor = std::io::Cursor::new(Vec::new());
    let mut writer = hound::WavWriter::new(
        &mut cursor,
        hound::WavSpec {
            channels: 1,
            sample_rate: 16000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        },
    )
    .unwrap();
    for _ in 0..frames {
        writer.write_sample(0i16).unwrap();
    }
    writer.finalize().unwrap();
    cursor.into_inner()
}
fn result(text: &str) -> SttResult {
    SttResult {
        segments: Vec::new(),
        text: text.into(),
        duration_ms: 1,
        retry: Default::default(),
    }
}
fn path() -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "kolboo-meeting-{}.transcripts",
        uuid::Uuid::new_v4()
    ))
}

#[test]
fn cancellation_during_sample_read_stops_before_encoding() {
    struct CancellingReader {
        inner: std::io::Cursor<Vec<u8>>,
        armed: Arc<std::sync::atomic::AtomicBool>,
        token: CancellationToken,
    }
    impl Read for CancellingReader {
        fn read(&mut self, data: &mut [u8]) -> std::io::Result<usize> {
            let count = self.inner.read(data)?;
            if self.armed.load(Ordering::SeqCst) {
                self.token.cancel();
            }
            Ok(count)
        }
    }
    impl Seek for CancellingReader {
        fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
            self.inner.seek(pos)
        }
    }
    let token = CancellationToken::new();
    let armed = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let source: Box<dyn AudioReader> = Box::new(CancellingReader {
        inner: std::io::Cursor::new(audio(16)),
        armed: armed.clone(),
        token: token.clone(),
    });
    let reader = hound::WavReader::new(source).unwrap();
    armed.store(true, Ordering::SeqCst);
    assert!(matches!(
        read_block(reader, 0, 16, &token),
        Err(PipelineError::Cancelled)
    ));
}

#[test]
fn malformed_or_oversized_meeting_audio_is_rejected_before_uploading() {
    let token = CancellationToken::new();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("oversized.wav");
    std::fs::write(&path, audio(1)).unwrap();
    std::fs::OpenOptions::new()
        .write(true)
        .open(&path)
        .unwrap()
        .set_len(MAX_MEETING_WAV_BYTES as u64 + 1)
        .unwrap();
    let source = TranscriptionAudio::open(path.clone()).unwrap();
    assert!(matches!(
        prepare_audio(source, &token),
        Err(PipelineError::Config(message)) if message == "Unsupported meeting audio format"
    ));
    assert_eq!(
        std::fs::metadata(path).unwrap().len(),
        MAX_MEETING_WAV_BYTES as u64 + 1
    );

    let mut wrong_rate = audio(16);
    wrong_rate[24..28].copy_from_slice(&8000_u32.to_le_bytes());
    wrong_rate[28..32].copy_from_slice(&16000_u32.to_le_bytes());
    assert!(matches!(
        prepare_audio(wrong_rate.into(), &token),
        Err(PipelineError::Config(message)) if message == "Unsupported meeting audio format"
    ));
    assert!(matches!(
        prepare_audio(vec![0; 44].into(), &token),
        Err(PipelineError::Config(_))
    ));
    let (reader, _) = prepare_audio(audio(16).into(), &token).unwrap();
    assert!(matches!(
        read_block(reader, 16, 16, &token),
        Err(PipelineError::Config(message)) if message.contains("Your audio is retained")
    ));
}

#[tokio::test]
async fn cancelled_preparation_and_chunk_workers_never_upload() {
    let wav = audio(16000);
    let calls = AtomicUsize::new(0);
    let token = CancellationToken::new();
    token.cancel();
    let outcome = transcribe_blocks(&wav, None, "model", &token, 16000, |_| {
        calls.fetch_add(1, Ordering::SeqCst);
        async { Ok(result("unexpected upload")) }
    })
    .await;
    assert!(matches!(outcome, Err(PipelineError::Cancelled)));
    assert_eq!(calls.load(Ordering::SeqCst), 0);

    assert!(matches!(
        prepare_audio(wav.clone().into(), &token),
        Err(PipelineError::Cancelled)
    ));
    let (reader, _) = prepare_audio(wav.clone().into(), &CancellationToken::new()).unwrap();
    let spec = reader.spec();
    assert!(matches!(
        read_block(reader, 0, 16000, &token),
        Err(PipelineError::Cancelled)
    ));
    assert!(matches!(
        encode_block(vec![0; 16000], spec, &token),
        Err(PipelineError::Cancelled)
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn cancellation_interrupts_preparation_before_any_upload() {
    let wav = audio(WORK_BATCH_BYTES * 2);
    let token = CancellationToken::new();
    let cancel = token.clone();
    // A single-thread runtime proves CPU preparation yields, rather than
    // accidentally relying on another worker thread or a timing sleep.
    let cancellation = async move {
        tokio::task::yield_now().await;
        cancel.cancel();
    };
    let transcription = transcribe_blocks(&wav, None, "model", &token, 16000, |_| async {
        panic!("cancelled preparation must never upload")
    });
    let (result, ()) = tokio::join!(transcription, cancellation);
    assert!(matches!(result, Err(PipelineError::Cancelled)));
}

#[tokio::test]
async fn diarized_meeting_checkpoints_preserve_request_local_speakers() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("checkpoint");
    let wav = audio(32000);
    let token = CancellationToken::new();
    let original = transcribe_blocks(&wav, Some(&path), "diarize", &token, 16000, |_| async {
        let mut result = result("Hello");
        result.segments.push(crate::stt::SpeakerSegment {
            speaker: "A".into(),
            text: "Hello".into(),
            start_seconds: 0.0,
            end_seconds: 0.5,
            part: 1,
        });
        Ok(result)
    })
    .await
    .unwrap();
    let resumed = transcribe_blocks(&wav, Some(&path), "diarize", &token, 16000, |_| async {
        panic!("completed uploads must not repeat")
    })
    .await
    .unwrap();
    assert_eq!(resumed.segments.len(), 2);
    assert_eq!(resumed.segments[0].part, 1);
    assert_eq!(resumed.segments[1].part, 2);
    assert_eq!(resumed.segments[1].start_seconds, 1.0);
    assert_eq!(
        serde_json::to_value(&original.segments).unwrap(),
        serde_json::to_value(&resumed.segments).unwrap()
    );
    let document = crate::stt::speaker_document(&resumed.text, &resumed.segments);
    assert!(document.contains("Part 2"));
    assert_eq!(document.matches("Speaker A").count(), 2);
}

#[tokio::test]
async fn failed_upload_resumes_and_ignores_partial_crash_checkpoint() {
    let path = path();
    let wav = audio(48000);
    let cancel = CancellationToken::new();
    let calls = Arc::new(AtomicUsize::new(0));
    let count = calls.clone();
    let failed = transcribe_blocks(&wav, Some(&path), "model-a", &cancel, 16000, move |_| {
        let index = count.fetch_add(1, Ordering::SeqCst);
        async move {
            if index == 1 {
                Err(PipelineError::Cancelled)
            } else {
                Ok(result("first"))
            }
        }
    })
    .await;
    assert!(failed.is_err());
    OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap()
        .write_all(b"{partial")
        .unwrap();
    let count = calls.clone();
    let resumed = transcribe_blocks(&wav, Some(&path), "model-a", &cancel, 16000, move |_| {
        count.fetch_add(1, Ordering::SeqCst);
        async { Ok(result("next")) }
    })
    .await
    .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 4); // successful first upload not repeated
    assert_eq!(resumed.text, "first\nnext\nnext");
    let cached = transcribe_blocks(&wav, Some(&path), "model-a", &cancel, 16000, |_| async {
        panic!("completed uploads must not run again")
    })
    .await
    .unwrap();
    assert_eq!(cached.text, resumed.text);
    let count = calls.clone();
    transcribe_blocks(&wav, Some(&path), "model-b", &cancel, 16000, move |_| {
        count.fetch_add(1, Ordering::SeqCst);
        async { Ok(result("new model")) }
    })
    .await
    .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 7);
    let mut changed_audio = wav.clone();
    *changed_audio.last_mut().unwrap() = 1;
    let count = calls.clone();
    transcribe_blocks(
        &changed_audio,
        Some(&path),
        "model-b",
        &cancel,
        16000,
        move |_| {
            count.fetch_add(1, Ordering::SeqCst);
            async { Ok(result("changed audio")) }
        },
    )
    .await
    .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 10);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    std::fs::remove_file(path).unwrap();
}

#[tokio::test]
async fn cancellation_between_uploads_preserves_completed_work() {
    let path = path();
    let wav = audio(32000);
    let token = CancellationToken::new();
    let cancel = token.clone();
    assert!(
        transcribe_blocks(&wav, Some(&path), "model", &token, 16000, move |_| {
            cancel.cancel();
            async { Ok(result("saved")) }
        })
        .await
        .is_err()
    );
    let result = transcribe_blocks(
        &wav,
        Some(&path),
        "model",
        &CancellationToken::new(),
        16000,
        |_| async { Ok(result("remaining")) },
    )
    .await
    .unwrap();
    assert_eq!(result.text, "saved\nremaining");
    std::fs::remove_file(path).unwrap();
}

#[tokio::test]
async fn long_meeting_has_small_uploads_no_missing_audio_and_one_result() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("progress");
    let audio_path = directory.path().join("long.wav");
    let frames = 16000 * 35 * 60;
    // A sparse, silent 35-minute WAV exercises the real disk path without
    // materializing a whole-meeting fixture in memory.
    let mut header = audio(0);
    header[4..8].copy_from_slice(&((frames * 2 + 36) as u32).to_le_bytes());
    header[40..44].copy_from_slice(&((frames * 2) as u32).to_le_bytes());
    std::fs::write(&audio_path, header).unwrap();
    OpenOptions::new()
        .write(true)
        .open(&audio_path)
        .unwrap()
        .set_len((frames * 2 + 44) as u64)
        .unwrap();
    let wav = TranscriptionAudio::open(audio_path).unwrap();
    assert!(wav.len() > 50 * 1024 * 1024);
    assert!(wav.memory().is_none());
    let total = Arc::new(AtomicUsize::new(0));
    let calls = Arc::new(AtomicUsize::new(0));
    let uploaded = total.clone();
    let count = calls.clone();
    let output = transcribe(
        wav.clone(),
        Some(&path),
        "model",
        &CancellationToken::new(),
        move |chunk| {
            assert!(chunk.len() < 20 * 1024 * 1024);
            let reader = hound::WavReader::new(std::io::Cursor::new(chunk)).unwrap();
            uploaded.fetch_add(reader.duration() as usize, Ordering::SeqCst);
            count.fetch_add(1, Ordering::SeqCst);
            async { Ok(result("part")) }
        },
    )
    .await
    .unwrap();
    assert_eq!(total.load(Ordering::SeqCst), frames);
    assert_eq!(calls.load(Ordering::SeqCst), 4);
    assert_eq!(output.text, "part\npart\npart\npart");
    let mut unexpected_uploads = 0;
    let replay = transcribe(wav, Some(&path), "model", &CancellationToken::new(), |_| {
        unexpected_uploads += 1;
        async { Ok(result("unexpected")) }
    })
    .await
    .unwrap();
    assert_eq!(unexpected_uploads, 0);
    assert_eq!(replay.text, output.text);
}
