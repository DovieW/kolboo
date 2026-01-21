//! STT transcription flow helpers.
//!
//! This module extracts the shared logic for running STT transcription with
//! retry, optional timeout, and cancellation.

use std::sync::Arc;
use std::time::Duration;

use tokio_util::sync::CancellationToken;

use crate::stt::{with_retry, AudioFormat, RetryConfig, SttProvider};

use super::types::PipelineError;
use super::utils::normalize_stt_text;

/// Result of running STT transcription.
pub(super) struct SttResult {
    /// The transcribed (and normalized) text.
    pub text: String,
    /// Duration of the STT request in milliseconds.
    pub duration_ms: u64,
}

/// Run STT transcription with retry, optional timeout, and cancellation.
///
/// Returns the transcribed text and duration, or an error.
/// If `timeout` is None, no timeout is applied (useful for test endpoints).
pub(super) async fn run_stt_transcription(
    stt_provider: Arc<dyn SttProvider>,
    wav_bytes: &[u8],
    retry_config: &RetryConfig,
    timeout: Option<Duration>,
    cancel_token: &CancellationToken,
    log_prefix: &str,
) -> Result<SttResult, PipelineError> {
    let format = AudioFormat::default();
    let wav = Arc::new(wav_bytes.to_vec());

    let transcription_future = async {
        with_retry(retry_config, || {
            let provider = stt_provider.clone();
            let wav = wav.clone();
            let format = format.clone();
            async move { provider.transcribe(wav.as_slice(), &format).await }
        })
        .await
    };

    let stt_start = std::time::Instant::now();

    let stt_result = if let Some(timeout_duration) = timeout {
        // With timeout
        tokio::select! {
            biased;

            _ = cancel_token.cancelled() => {
                log::info!("{}: Transcription cancelled", log_prefix);
                Err(PipelineError::Cancelled)
            }

            _ = tokio::time::sleep(timeout_duration) => {
                log::warn!("{}: Transcription timed out after {:?}", log_prefix, timeout_duration);
                Err(PipelineError::Timeout(timeout_duration))
            }

            result = transcription_future => {
                result.map_err(PipelineError::from)
            }
        }
    } else {
        // No timeout, just cancellation
        tokio::select! {
            biased;

            _ = cancel_token.cancelled() => {
                log::info!("{}: Transcription cancelled", log_prefix);
                Err(PipelineError::Cancelled)
            }

            result = transcription_future => {
                result.map_err(PipelineError::from)
            }
        }
    };

    let duration_ms = stt_start.elapsed().as_millis() as u64;

    match stt_result {
        Ok(text) => {
            let normalized = normalize_stt_text(text);
            log::info!("{}: STT complete, {} chars", log_prefix, normalized.len());
            Ok(SttResult {
                text: normalized,
                duration_ms,
            })
        }
        Err(e) => Err(e),
    }
}
