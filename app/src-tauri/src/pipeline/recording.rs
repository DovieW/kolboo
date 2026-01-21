//! Recording session helpers.
//!
//! This module extracts the shared logic for starting and stopping recording sessions,
//! including diagnostics capture and size limit validation.
//!
//! The `SharedPipeline` methods still orchestrate the high-level flow, but delegate to
//! these helpers for the core recording operations.

use crate::audio_capture::{AudioCaptureBackend, AudioCaptureDiagnostics, AudioEncodeConfig};

use super::state_machine::PipelineState;
use super::types::PipelineError;
use super::utils::{amp_to_dbfs, is_effectively_quiet};

/// Outcome of stopping a recording session.
pub(super) struct StopRecordingOutcome {
    /// The encoded WAV bytes.
    pub wav_bytes: Vec<u8>,
    /// Raw stats + optional speech detection.
    pub diagnostics: AudioCaptureDiagnostics,
}

/// Outcome of stopping with before/after comparison.
pub(super) struct StopBeforeAfterOutcome {
    /// Raw capture with no preprocessing.
    pub before_wav: Vec<u8>,
    /// Capture encoded with current audio settings.
    pub after_wav: Vec<u8>,
    /// Diagnostics for the processed output.
    pub diagnostics: AudioCaptureDiagnostics,
}

/// Start a recording session on the given audio capture backend.
///
/// Returns `Ok(())` if the session was started successfully.
pub(super) fn start_recording_session(
    audio_capture: &mut dyn AudioCaptureBackend,
    max_duration_secs: f32,
    input_device_name: Option<&str>,
) -> Result<(), PipelineError> {
    audio_capture
        .start_recording_session(max_duration_secs, input_device_name)
        .map_err(PipelineError::AudioCapture)
}

/// Stop a recording session and retrieve the WAV bytes with diagnostics.
///
/// Returns the encoded WAV bytes and diagnostics.
pub(super) fn stop_recording_session(
    audio_capture: &mut dyn AudioCaptureBackend,
    encode_cfg: AudioEncodeConfig,
) -> Result<StopRecordingOutcome, PipelineError> {
    let (wav_bytes, diagnostics) = audio_capture
        .stop_and_get_wav_with_diagnostics(encode_cfg)
        .map_err(PipelineError::AudioCapture)?;

    Ok(StopRecordingOutcome {
        wav_bytes,
        diagnostics,
    })
}

/// Stop a recording session and retrieve before/after WAV bytes for A/B comparison.
///
/// - before: raw capture with no preprocessing/gates
/// - after: capture encoded with the current audio settings
pub(super) fn stop_recording_before_after(
    audio_capture: &mut dyn AudioCaptureBackend,
    encode_cfg: AudioEncodeConfig,
) -> Result<StopBeforeAfterOutcome, PipelineError> {
    let (before_wav, after_wav, diagnostics) = audio_capture
        .stop_and_get_wav_before_after(encode_cfg)
        .map_err(PipelineError::AudioCapture)?;

    Ok(StopBeforeAfterOutcome {
        before_wav,
        after_wav,
        diagnostics,
    })
}

/// Validate that the recording size is within limits.
///
/// Returns `Ok(())` if valid, or an error if too large.
pub(super) fn validate_recording_size(
    wav_bytes_len: usize,
    max_bytes: usize,
) -> Result<(), PipelineError> {
    if max_bytes > 0 && wav_bytes_len > max_bytes {
        Err(PipelineError::RecordingTooLarge(wav_bytes_len, max_bytes))
    } else {
        Ok(())
    }
}

/// Configuration for the quiet audio gate check.
#[derive(Debug, Clone, Copy)]
pub(super) struct QuietAudioGateConfig {
    pub enabled: bool,
    pub require_speech: bool,
    pub min_duration_secs: f32,
    pub rms_dbfs_threshold: f32,
    pub peak_dbfs_threshold: f32,
}

/// Result of quiet audio gate evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum QuietAudioGateResult {
    /// Audio is not quiet, proceed with transcription.
    NotQuiet,
    /// Audio is quiet, skip STT.
    Quiet,
    /// No speech was detected by VAD, skip STT.
    NoSpeechDetected,
}

/// Evaluate whether the recording should skip STT due to quiet audio or no speech.
pub(super) fn evaluate_quiet_audio_gate(
    diagnostics: &AudioCaptureDiagnostics,
    config: QuietAudioGateConfig,
) -> QuietAudioGateResult {
    if !config.enabled {
        return QuietAudioGateResult::NotQuiet;
    }

    let stats = diagnostics.stats;

    // Optional: skip if VAD says "no speech".
    if config.require_speech && diagnostics.speech_detected == Some(false) {
        log::info!(
            "Recording: Skipping STT because no speech was detected by offline VAD \
             (duration {:.2}s, rms {:.1} dBFS, peak {:.1} dBFS)",
            stats.duration_secs,
            amp_to_dbfs(stats.rms),
            amp_to_dbfs(stats.peak)
        );
        return QuietAudioGateResult::NoSpeechDetected;
    }

    // Check amplitude thresholds.
    if is_effectively_quiet(
        stats,
        config.min_duration_secs,
        config.rms_dbfs_threshold,
        config.peak_dbfs_threshold,
    ) {
        log::info!(
            "Recording: Skipping STT because recording is quiet \
             (duration {:.2}s, rms {:.1} dBFS, peak {:.1} dBFS)",
            stats.duration_secs,
            amp_to_dbfs(stats.rms),
            amp_to_dbfs(stats.peak)
        );
        return QuietAudioGateResult::Quiet;
    }

    QuietAudioGateResult::NotQuiet
}

/// Check if the pipeline state allows starting a recording.
pub(super) fn can_start_recording(state: PipelineState) -> bool {
    state.can_start_recording()
}

/// Check if the pipeline state allows stopping a recording.
pub(super) fn can_stop_recording(state: PipelineState) -> bool {
    state.can_stop_recording()
}
