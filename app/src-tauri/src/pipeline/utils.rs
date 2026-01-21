use crate::audio_capture::AudioLevelStats;
use std::time::Duration;

/// Normalize STT output text.
///
/// Some providers (notably Whisper-based APIs) may include a leading space as a
/// tokenization artifact (many vocabularies encode " space+word" as a single token).
/// We trim only *leading* whitespace to avoid changing internal formatting.
pub(super) fn normalize_stt_text(text: String) -> String {
    match text.chars().next() {
        Some(c) if c.is_whitespace() => text.trim_start().to_string(),
        _ => text,
    }
}

pub(super) fn seconds_to_duration_or(seconds: f64, fallback: Duration) -> Duration {
    // Guard against invalid values.
    if !seconds.is_finite() || seconds <= 0.0 {
        return fallback;
    }
    Duration::from_secs_f64(seconds)
}

pub(super) fn amp_to_dbfs(amp: f32) -> f32 {
    if !amp.is_finite() || amp <= 0.0 {
        f32::NEG_INFINITY
    } else {
        20.0 * amp.log10()
    }
}

pub(super) fn is_effectively_quiet(
    stats: AudioLevelStats,
    min_duration_secs: f32,
    rms_dbfs_threshold: f32,
    peak_dbfs_threshold: f32,
) -> bool {
    // Very short recordings are usually accidental taps; treat as quiet.
    if stats.duration_secs < min_duration_secs {
        return true;
    }

    let rms_dbfs = amp_to_dbfs(stats.rms);
    let peak_dbfs = amp_to_dbfs(stats.peak);

    rms_dbfs < rms_dbfs_threshold && peak_dbfs < peak_dbfs_threshold
}
