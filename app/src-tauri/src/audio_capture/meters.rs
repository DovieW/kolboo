//! Audio metering and speech-presence helpers.
//!
//! These types are intentionally realtime-friendly: the capture callback writes
//! atomics through the worker thread, while UI/overlay readers take cheap
//! snapshots without borrowing the full `AudioCapture` state machine.

use crate::audio_normalization::downmix_interleaved_to_mono;
use crate::vad::{VadConfig, VadEvent, VadFrameProcessor};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;

/// Basic audio level metrics for gating/diagnostics.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
pub struct AudioLevelStats {
    pub duration_secs: f32,
    /// Root-mean-square amplitude in [0, 1].
    pub rms: f32,
    /// Peak (max absolute) amplitude in [0, 1].
    pub peak: f32,
}

pub(super) fn compute_rms_peak(data: &[f32]) -> (f32, f32) {
    let mut peak: f32 = 0.0;
    let mut sum_sq: f64 = 0.0;
    for &s in data {
        let a = s.abs();
        if a > peak {
            peak = a;
        }
        sum_sq += (s as f64) * (s as f64);
    }
    let n = data.len() as u64;
    let rms = if n == 0 {
        0.0
    } else {
        (sum_sq / n as f64).sqrt() as f32
    };
    (rms, peak)
}

pub(super) fn detect_speech_presence(samples: &[f32], sample_rate: u32, channels: u16) -> bool {
    if samples.is_empty() {
        return false;
    }

    let mono = downmix_interleaved_to_mono(samples, channels.max(1) as usize);
    let mut processor = VadFrameProcessor::new(VadConfig::default(), sample_rate.max(1));

    for event in processor.process(&mono) {
        if matches!(event, VadEvent::SpeechStart { .. }) {
            return true;
        }
    }
    false
}

/// Realtime-safe snapshot of the most recent input level.
///
/// Updated by the CPAL input callback using atomics (no allocations, no event emission).
#[derive(Debug, Clone, Copy)]
pub struct AudioLevelSnapshot {
    pub seq: u64,
    /// Root-mean-square amplitude in [0, 1] for the most recent callback chunk.
    pub rms: f32,
    /// Peak (max abs) amplitude in [0, 1] for the most recent callback chunk.
    pub peak: f32,
}

/// Number of min/max buckets sent to the overlay for waveform rendering.
///
/// Keep this modest: payload size is 2 * N floats per frame.
pub const WAVEFORM_BINS: usize = 64;

/// Realtime-safe snapshot of the most recent min/max waveform buckets.
///
/// `mins[i]` and `maxes[i]` are in [-1, 1] representing the min/max sample value
/// for that bucket.
#[derive(Debug, Clone)]
pub struct AudioWaveformSnapshot {
    pub seq: u64,
    pub mins: Vec<f32>,
    pub maxes: Vec<f32>,
}

/// A cheap-to-clone handle for reading realtime waveform buckets without needing
/// to borrow the full `AudioCapture`.
#[derive(Clone)]
pub struct SharedAudioWaveformMeter {
    inner: Arc<AudioWaveformMeter>,
}

impl SharedAudioWaveformMeter {
    pub(super) fn from_meter(inner: Arc<AudioWaveformMeter>) -> Self {
        Self { inner }
    }

    pub fn snapshot(&self) -> AudioWaveformSnapshot {
        self.inner.snapshot()
    }

    #[cfg(test)]
    pub fn new_for_tests() -> Self {
        Self {
            inner: Arc::new(AudioWaveformMeter::default()),
        }
    }

    #[cfg(test)]
    #[allow(dead_code)]
    pub fn set_from_samples_for_tests(&self, samples: &[f32], channels: usize) {
        self.inner.update_from_f32_interleaved(samples, channels);
    }
}

/// A cheap-to-clone handle for reading realtime audio levels without needing to
/// borrow the full `AudioCapture`.
///
/// This wrapper avoids exposing the internal `AudioLevelMeter` implementation.
#[derive(Clone)]
pub struct SharedAudioLevelMeter {
    inner: Arc<AudioLevelMeter>,
}

impl SharedAudioLevelMeter {
    pub(super) fn from_meter(inner: Arc<AudioLevelMeter>) -> Self {
        Self { inner }
    }

    pub fn snapshot(&self) -> AudioLevelSnapshot {
        self.inner.snapshot()
    }

    #[cfg(test)]
    pub fn new_for_tests() -> Self {
        Self {
            inner: Arc::new(AudioLevelMeter::default()),
        }
    }

    #[cfg(test)]
    pub fn set_for_tests(&self, rms: f32, peak: f32) {
        self.inner.update(rms, peak);
    }
}

#[derive(Debug)]
pub(super) struct AudioWaveformMeter {
    seq: AtomicU64,
    min_bits: [AtomicU32; WAVEFORM_BINS],
    max_bits: [AtomicU32; WAVEFORM_BINS],
}

impl Default for AudioWaveformMeter {
    fn default() -> Self {
        Self {
            seq: AtomicU64::new(0),
            min_bits: std::array::from_fn(|_| AtomicU32::new(0f32.to_bits())),
            max_bits: std::array::from_fn(|_| AtomicU32::new(0f32.to_bits())),
        }
    }
}

impl AudioWaveformMeter {
    pub(super) fn snapshot(&self) -> AudioWaveformSnapshot {
        let seq = self.seq.load(Ordering::Relaxed);
        let mut mins = Vec::with_capacity(WAVEFORM_BINS);
        let mut maxes = Vec::with_capacity(WAVEFORM_BINS);
        for i in 0..WAVEFORM_BINS {
            mins.push(f32::from_bits(self.min_bits[i].load(Ordering::Relaxed)));
            maxes.push(f32::from_bits(self.max_bits[i].load(Ordering::Relaxed)));
        }
        AudioWaveformSnapshot { seq, mins, maxes }
    }

    pub(super) fn update_from_f32_interleaved(&self, data: &[f32], channels: usize) {
        let channels = channels.max(1);
        let frames = data.len() / channels;
        if frames == 0 {
            return;
        }

        for bin in 0..WAVEFORM_BINS {
            let start = (bin * frames) / WAVEFORM_BINS;
            let end = ((bin + 1) * frames) / WAVEFORM_BINS;
            if start >= end {
                self.min_bits[bin].store(0f32.to_bits(), Ordering::Relaxed);
                self.max_bits[bin].store(0f32.to_bits(), Ordering::Relaxed);
                continue;
            }

            let mut min_v: f32 = 1.0;
            let mut max_v: f32 = -1.0;

            for frame in start..end {
                let base = frame * channels;
                let mut acc: f32 = 0.0;
                for c in 0..channels {
                    acc += data.get(base + c).copied().unwrap_or(0.0);
                }
                let s = (acc / channels as f32).clamp(-1.0, 1.0);
                if s < min_v {
                    min_v = s;
                }
                if s > max_v {
                    max_v = s;
                }
            }

            self.min_bits[bin].store(min_v.to_bits(), Ordering::Relaxed);
            self.max_bits[bin].store(max_v.to_bits(), Ordering::Relaxed);
        }

        self.seq.fetch_add(1, Ordering::Relaxed);
    }
}

#[derive(Debug, Default)]
pub(super) struct AudioLevelMeter {
    seq: AtomicU64,
    rms_bits: AtomicU32,
    peak_bits: AtomicU32,
}

impl AudioLevelMeter {
    pub(super) fn snapshot(&self) -> AudioLevelSnapshot {
        let seq = self.seq.load(Ordering::Relaxed);
        let rms = f32::from_bits(self.rms_bits.load(Ordering::Relaxed));
        let peak = f32::from_bits(self.peak_bits.load(Ordering::Relaxed));
        AudioLevelSnapshot { seq, rms, peak }
    }

    pub(super) fn update(&self, rms: f32, peak: f32) {
        // Clamp to sane range and avoid NaNs propagating into the UI.
        let rms = if rms.is_finite() {
            rms.clamp(0.0, 1.0)
        } else {
            0.0
        };
        let peak = if peak.is_finite() {
            peak.clamp(0.0, 1.0)
        } else {
            0.0
        };

        self.rms_bits.store(rms.to_bits(), Ordering::Relaxed);
        self.peak_bits.store(peak.to_bits(), Ordering::Relaxed);
        self.seq.fetch_add(1, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rms_peak_handles_empty_and_absolute_peak() {
        assert_eq!(compute_rms_peak(&[]), (0.0, 0.0));

        let (rms, peak) = compute_rms_peak(&[-0.5, 0.25, 0.75]);
        assert_eq!(peak, 0.75);
        assert!(rms > 0.0);
    }

    #[test]
    fn level_meter_clamps_non_finite_and_out_of_range_values() {
        let meter = SharedAudioLevelMeter::new_for_tests();
        meter.set_for_tests(f32::NAN, 2.5);
        let snapshot = meter.snapshot();
        assert_eq!(snapshot.seq, 1);
        assert_eq!(snapshot.rms, 0.0);
        assert_eq!(snapshot.peak, 1.0);
    }

    #[test]
    fn waveform_meter_returns_fixed_bucket_shape() {
        let meter = SharedAudioWaveformMeter::new_for_tests();
        let samples: Vec<f32> = (0..WAVEFORM_BINS)
            .map(|idx| if idx % 2 == 0 { -0.25 } else { 0.75 })
            .collect();
        meter.set_from_samples_for_tests(&samples, 1);

        let snapshot = meter.snapshot();
        assert_eq!(snapshot.seq, 1);
        assert_eq!(snapshot.mins.len(), WAVEFORM_BINS);
        assert_eq!(snapshot.maxes.len(), WAVEFORM_BINS);
        assert!(snapshot.mins.iter().any(|v| *v < 0.0));
        assert!(snapshot.maxes.iter().any(|v| *v > 0.0));
    }

    #[test]
    fn speech_presence_is_false_for_empty_audio() {
        assert!(!detect_speech_presence(&[], 16_000, 1));
    }
}
