//! Simulated streaming: converts any batch `SttProvider` into a
//! `StreamingSttSession` by periodically transcribing the *entire* accumulated
//! audio buffer during recording.
//!
//! This is a provider-agnostic wrapper: it works with any `SttProvider` that
//! supports batch transcription, giving models that lack native WebSocket
//! streaming the ability to produce progressive transcripts during recording.
//!
//! ## Design
//!
//! Each intermediate call sends all audio captured so far (cumulative), not just
//! the new segment.  This eliminates boundary artefacts (words cut in half
//! between chunks) and gives the model full context for accurate transcription.
//!
//! For live-output paste, we compute a word-level diff between the previous full
//! transcript and the new one, extracting only the newly committed suffix.
//!
//! When recording stops, a final cumulative call is made with the complete audio,
//! producing the same quality as a normal batch call – no tiny-chunk
//! hallucination risk.

use std::io::Cursor;
use std::sync::Arc;

use hound::{WavSpec, WavWriter};
use tokio::sync::mpsc;

use super::streaming::{PartialTranscript, StreamingSttSession};
use super::{AudioEncoding, AudioFormat, SttError, SttProvider};

/// Default chunk interval — how much *new* audio (in seconds) must accumulate
/// before we fire the next cumulative batch transcription request.
const DEFAULT_CHUNK_INTERVAL_SECS: f32 = 3.0;

/// Minimum total audio length (in seconds) worth sending.  Anything shorter is
/// likely just breathing/noise and would produce empty or hallucinated text.
const MIN_AUDIO_SECS: f32 = 0.5;

/// Minimum *new* audio (seconds) since the last successful transcription before
/// we bother re-transcribing at finalization.  Below this threshold the model is
/// unlikely to produce new meaningful text and may hallucinate; we just keep the
/// last known-good result.
const MIN_FINAL_NEW_AUDIO_SECS: f32 = 1.5;

/// Start a simulated streaming session that wraps a batch `SttProvider`.
///
/// Audio flows in through the returned `StreamingSttSession::audio_tx` just like
/// a native streaming session. Under the hood, the full accumulated buffer is
/// periodically sent to the batch API. Partial transcripts are emitted via
/// `partial_rx` for overlay display and optional live output.
pub fn start_simulated_streaming(
    provider: Arc<dyn SttProvider>,
    sample_rate: u32,
) -> StreamingSttSession {
    start_simulated_streaming_with_interval(provider, sample_rate, DEFAULT_CHUNK_INTERVAL_SECS)
}

/// Same as [`start_simulated_streaming`] but with a configurable chunk interval.
pub fn start_simulated_streaming_with_interval(
    provider: Arc<dyn SttProvider>,
    sample_rate: u32,
    chunk_interval_secs: f32,
) -> StreamingSttSession {
    let (audio_tx, audio_rx) = mpsc::channel::<Vec<f32>>(64);
    let (partial_tx, partial_rx) = mpsc::channel::<PartialTranscript>(32);

    let task = tokio::spawn(run_simulated_streaming_task(
        provider,
        audio_rx,
        partial_tx,
        sample_rate,
        chunk_interval_secs,
    ));

    StreamingSttSession::new(audio_tx, partial_rx, task)
}

/// Background task that accumulates audio, sends periodic cumulative batch
/// transcriptions, and emits partial transcripts.
async fn run_simulated_streaming_task(
    provider: Arc<dyn SttProvider>,
    mut audio_rx: mpsc::Receiver<Vec<f32>>,
    partial_tx: mpsc::Sender<PartialTranscript>,
    sample_rate: u32,
    chunk_interval_secs: f32,
) -> Result<String, SttError> {
    let interval_samples = (sample_rate as f32 * chunk_interval_secs.max(MIN_AUDIO_SECS)) as usize;
    let min_samples = (sample_rate as f32 * MIN_AUDIO_SECS) as usize;
    let min_final_new_samples = (sample_rate as f32 * MIN_FINAL_NEW_AUDIO_SECS) as usize;

    let mut buffer: Vec<f32> = Vec::new();
    let mut samples_at_last_send: usize = 0;
    let mut last_full_transcript = String::new();

    // ── live recording loop ──
    while let Some(samples) = audio_rx.recv().await {
        buffer.extend_from_slice(&samples);

        let new_since_last = buffer.len().saturating_sub(samples_at_last_send);
        if new_since_last >= interval_samples {
            if let Some(text) =
                transcribe_buffer(&provider, &buffer, sample_rate, min_samples).await
            {
                let committed = extract_committed_text(&last_full_transcript, &text);

                let _ = partial_tx
                    .send(PartialTranscript {
                        text: text.clone(),
                        committed_text: if committed.is_empty() {
                            None
                        } else {
                            Some(committed)
                        },
                    })
                    .await;

                log::debug!(
                    "Simulated streaming: cumulative transcription ({:.1}s audio) → {} chars",
                    buffer.len() as f32 / sample_rate as f32,
                    text.len(),
                );

                last_full_transcript = text;
                samples_at_last_send = buffer.len();
            }
        }
    }

    // ── finalization: channel closed (recording stopped) ──
    // Re-transcribe the full buffer if there's meaningful new audio since the
    // last call; otherwise keep the last known-good result to avoid hallucinating
    // on a tiny delta.
    let new_since_last = buffer.len().saturating_sub(samples_at_last_send);
    let should_retranscribe =
        new_since_last >= min_final_new_samples || last_full_transcript.is_empty();

    if should_retranscribe && buffer.len() >= min_samples {
        if let Some(text) = transcribe_buffer(&provider, &buffer, sample_rate, min_samples).await {
            let committed = extract_committed_text(&last_full_transcript, &text);
            if !committed.is_empty() {
                let _ = partial_tx
                    .send(PartialTranscript {
                        text: text.clone(),
                        committed_text: Some(committed),
                    })
                    .await;
            }
            log::info!(
                "Simulated streaming: final transcription ({:.1}s audio) → {} chars",
                buffer.len() as f32 / sample_rate as f32,
                text.len(),
            );
            return Ok(text);
        }
    }

    if last_full_transcript.is_empty() {
        Err(SttError::Audio(
            "Simulated streaming produced no text from any call".into(),
        ))
    } else {
        log::info!(
            "Simulated streaming: using last known-good transcript ({} chars)",
            last_full_transcript.len(),
        );
        Ok(last_full_transcript)
    }
}

/// Extract the newly committed text by comparing the previous and new full
/// transcripts at word level.
///
/// Punctuation is normalized during comparison so that minor reformatting
/// (e.g. "Hello" → "Hello,") doesn't cause duplicate output.
fn extract_committed_text(previous: &str, new_full: &str) -> String {
    if previous.is_empty() {
        return new_full.to_string();
    }

    let prev_words: Vec<&str> = previous.split_whitespace().collect();
    let new_words: Vec<&str> = new_full.split_whitespace().collect();

    // Find how many leading words match (ignoring trailing punctuation).
    let common = prev_words
        .iter()
        .zip(new_words.iter())
        .take_while(|(a, b)| normalize_word(a) == normalize_word(b))
        .count();

    if common == 0 {
        // No common prefix at all — the model rephrased entirely.  Return the
        // full new text (caller should still paste something).
        return new_full.to_string();
    }

    if common >= new_words.len() {
        // The new transcript is a subset of (or identical to) the old one.
        // Nothing new was committed.
        return String::new();
    }

    new_words[common..].join(" ")
}

/// Strip trailing ASCII punctuation for comparison purposes.
fn normalize_word(word: &str) -> String {
    word.trim_end_matches(|c: char| c.is_ascii_punctuation())
        .to_lowercase()
}

/// Encode the full buffer as WAV and send it to the batch API.
async fn transcribe_buffer(
    provider: &Arc<dyn SttProvider>,
    buffer: &[f32],
    sample_rate: u32,
    min_samples: usize,
) -> Option<String> {
    if buffer.len() < min_samples {
        return None;
    }

    let wav_bytes = match encode_f32_mono_to_wav(buffer, sample_rate) {
        Ok(bytes) => bytes,
        Err(e) => {
            log::warn!("Simulated streaming: WAV encoding failed: {}", e);
            return None;
        }
    };

    let format = AudioFormat {
        sample_rate,
        channels: 1,
        encoding: AudioEncoding::Wav,
    };

    match provider.transcribe(&wav_bytes, &format).await {
        Ok(text) => {
            let text = text.trim().to_string();
            if text.is_empty() {
                log::debug!(
                    "Simulated streaming: empty result for {:.1}s audio",
                    buffer.len() as f32 / sample_rate as f32,
                );
                None
            } else {
                Some(text)
            }
        }
        Err(e) => {
            log::warn!("Simulated streaming: batch transcription failed: {}", e);
            None
        }
    }
}

/// Encode mono f32 samples as a WAV byte buffer (16-bit PCM).
fn encode_f32_mono_to_wav(samples: &[f32], sample_rate: u32) -> Result<Vec<u8>, SttError> {
    let spec = WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut cursor = Cursor::new(Vec::new());
    {
        let mut writer = WavWriter::new(&mut cursor, spec)
            .map_err(|e| SttError::Audio(format!("WAV writer init failed: {}", e)))?;

        for &sample in samples {
            let clamped = sample.clamp(-1.0, 1.0);
            let sample_i16 = (clamped * i16::MAX as f32) as i16;
            writer
                .write_sample(sample_i16)
                .map_err(|e| SttError::Audio(format!("WAV write failed: {}", e)))?;
        }

        writer
            .finalize()
            .map_err(|e| SttError::Audio(format!("WAV finalize failed: {}", e)))?;
    }

    Ok(cursor.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Mutex;

    // ── extract_committed_text unit tests ──

    #[test]
    fn committed_text_first_transcript() {
        assert_eq!(extract_committed_text("", "Hello world"), "Hello world");
    }

    #[test]
    fn committed_text_simple_append() {
        assert_eq!(
            extract_committed_text("Hello world", "Hello world how are you"),
            "how are you"
        );
    }

    #[test]
    fn committed_text_punctuation_change() {
        // Model adds a comma — should still count as a match and only return new words.
        assert_eq!(
            extract_committed_text("Hello world", "Hello, world, how are you"),
            "how are you"
        );
    }

    #[test]
    fn committed_text_identical() {
        assert_eq!(extract_committed_text("Hello world", "Hello world"), "");
    }

    #[test]
    fn committed_text_total_rephrase() {
        // If the model completely rephrases, return the full new text.
        assert_eq!(
            extract_committed_text("I like cats", "Dogs are great pets"),
            "Dogs are great pets"
        );
    }

    #[test]
    fn committed_text_subset_shrink() {
        // New transcript is shorter — nothing new to commit.
        assert_eq!(
            extract_committed_text("Hello world how are you", "Hello world"),
            ""
        );
    }

    #[test]
    fn normalize_word_strips_punctuation() {
        assert_eq!(normalize_word("Hello,"), "hello");
        assert_eq!(normalize_word("world."), "world");
        assert_eq!(normalize_word("end!?"), "end");
        assert_eq!(normalize_word("plain"), "plain");
    }

    // ── integration tests ──

    /// Fake provider that returns progressively longer transcripts when called
    /// with more audio (simulating cumulative transcription).
    struct CumulativeFakeProvider {
        call_count: AtomicU32,
        responses: Mutex<Vec<String>>,
    }

    impl CumulativeFakeProvider {
        fn new(responses: Vec<&str>) -> Self {
            Self {
                call_count: AtomicU32::new(0),
                responses: Mutex::new(responses.into_iter().map(String::from).collect()),
            }
        }
    }

    #[async_trait]
    impl SttProvider for CumulativeFakeProvider {
        async fn transcribe(
            &self,
            _audio: &[u8],
            _format: &AudioFormat,
        ) -> Result<String, SttError> {
            let n = self.call_count.fetch_add(1, Ordering::SeqCst) as usize;
            let responses = self.responses.lock().unwrap();
            // Cycle the last response if we get more calls than expected.
            let text = responses.get(n).unwrap_or(responses.last().unwrap());
            Ok(text.clone())
        }

        fn name(&self) -> &'static str {
            "cumulative-fake"
        }
    }

    #[tokio::test]
    async fn simulated_session_produces_incremental_commits() {
        let provider = Arc::new(CumulativeFakeProvider::new(vec![
            "Hello how are you",             // 1st cumulative call (after ~1s)
            "Hello how are you doing",       // 2nd call (after ~2s)
            "Hello how are you doing today", // final
        ]));
        let sample_rate = 16000;
        let mut session =
            start_simulated_streaming_with_interval(provider.clone(), sample_rate, 1.0);

        let mut partial_rx = session.take_partial_rx().unwrap();

        // Send 2.5 seconds of audio (enough for 2 intermediate calls).
        let one_second = vec![0.1_f32; 16000];
        for _ in 0..2 {
            // Send in smaller pieces to be realistic.
            for chunk in one_second.chunks(4000) {
                session.audio_tx.send(chunk.to_vec()).await.unwrap();
            }
        }

        // Receive first partial: full transcript "Hello how are you".
        let p1 = tokio::time::timeout(std::time::Duration::from_secs(5), partial_rx.recv())
            .await
            .expect("timeout")
            .expect("channel closed");
        assert_eq!(p1.text, "Hello how are you");
        assert_eq!(
            p1.committed_text.as_deref(),
            Some("Hello how are you"),
            "first commit should be the full text (no previous)"
        );

        // Send another second to trigger second call.
        for chunk in one_second.chunks(4000) {
            session.audio_tx.send(chunk.to_vec()).await.unwrap();
        }

        let p2 = tokio::time::timeout(std::time::Duration::from_secs(5), partial_rx.recv())
            .await
            .expect("timeout")
            .expect("channel closed");
        assert_eq!(p2.text, "Hello how are you doing");
        assert_eq!(
            p2.committed_text.as_deref(),
            Some("doing"),
            "second commit should be only the new word"
        );

        // Finalize (send enough remaining audio to trigger final call).
        for chunk in one_second.chunks(4000) {
            session.audio_tx.send(chunk.to_vec()).await.unwrap();
        }
        let final_text = session.finalize().await.unwrap();
        assert_eq!(final_text, "Hello how are you doing today");
    }

    #[tokio::test]
    async fn simulated_session_empty_audio_returns_error() {
        let provider = Arc::new(CumulativeFakeProvider::new(vec![""]));
        let session = start_simulated_streaming_with_interval(provider, 16000, 5.0);

        // Send very little audio (below minimum).
        session.audio_tx.send(vec![0.0_f32; 100]).await.unwrap();

        // Finalize should fail because no text was produced.
        let result = session.finalize().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn simulated_session_skips_tiny_final_chunk() {
        // Provider returns a good transcript on first call but would hallucinate
        // if called again with barely any new audio.
        let provider = Arc::new(CumulativeFakeProvider::new(vec![
            "The quick brown fox",
            "The quick brown fox HALLUCINATION HALLUCINATION",
        ]));
        let sample_rate = 16000;
        let mut session =
            start_simulated_streaming_with_interval(provider.clone(), sample_rate, 1.0);
        let _partial_rx = session.take_partial_rx().unwrap();

        // Send 1.5s of audio (triggers one intermediate call).
        session.audio_tx.send(vec![0.1_f32; 24000]).await.unwrap();
        // Let the intermediate call complete.
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // Send only 0.5s more (below MIN_FINAL_NEW_AUDIO_SECS = 1.5s).
        session.audio_tx.send(vec![0.1_f32; 8000]).await.unwrap();

        let final_text = session.finalize().await.unwrap();
        // Should use the last known-good result, not re-transcribe.
        assert_eq!(final_text, "The quick brown fox");
    }

    #[test]
    fn wav_encoding_roundtrip() {
        let samples: Vec<f32> = (0..1600).map(|i| (i as f32 / 1600.0).sin()).collect();
        let wav = encode_f32_mono_to_wav(&samples, 16000).unwrap();
        assert_eq!(&wav[..4], b"RIFF");
        assert!(wav.len() > 44);
    }
}
