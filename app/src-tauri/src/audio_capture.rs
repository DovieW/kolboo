//! Audio capture module using cpal for cross-platform audio input.
//!
//! This module provides functionality to capture audio from the system's
//! default input device and encode it to WAV format for STT processing.
//!
//! Supports optional Voice Activity Detection (VAD) for auto-stop functionality.

use crate::vad::{VadConfig, VadEvent, VadFrameProcessor};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::SampleFormat;
use hound::{WavSpec, WavWriter};
use serde::{Deserialize, Serialize};
use std::io::Cursor;
use std::sync::mpsc;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::thread::{self, JoinHandle};

fn clamp_u8_0_100(v: u8) -> u8 {
    v.min(100)
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn db_to_amp(db: f32) -> f32 {
    // db is dBFS; 0 dBFS == full-scale amplitude (1.0).
    10.0_f32.powf(db / 20.0)
}

fn downmix_interleaved_to_mono(samples: &[f32], channels: usize) -> Vec<f32> {
    let channels = channels.max(1);
    if channels == 1 {
        return samples.to_vec();
    }

    let frames = samples.len() / channels;
    let mut mono = Vec::with_capacity(frames);
    for frame_idx in 0..frames {
        let base = frame_idx * channels;
        let mut sum = 0.0_f32;
        for c in 0..channels {
            sum += samples[base + c];
        }
        mono.push(sum / channels as f32);
    }
    mono
}

fn downmix_interleaved_chunk_to_mono(samples: &[f32], channels: usize) -> Vec<f32> {
    // Same as full downmix, but kept separate for clarity.
    downmix_interleaved_to_mono(samples, channels)
}

fn apply_highpass_dc_block(samples: &mut [f32], sample_rate: u32) {
    // Simple DC-blocking high-pass filter.
    // Good enough to reduce rumble / DC offset without heavy DSP.
    let sr = sample_rate.max(1) as f32;
    // Choose r based on a rough cutoff. Keep stable across SR.
    // r close to 1.0 => lower cutoff.
    let cutoff_hz = 80.0_f32;
    let r = (-2.0 * std::f32::consts::PI * cutoff_hz / sr).exp();
    let mut y_prev = 0.0_f32;
    let mut x_prev = 0.0_f32;
    for x in samples.iter_mut() {
        let y = *x - x_prev + r * y_prev;
        x_prev = *x;
        y_prev = y;
        *x = y;
    }
}

fn apply_agc(samples: &mut [f32]) {
    // Lightweight gain normalization.
    // Target a strong peak while capping max gain to avoid crazy amplification.
    let mut peak = 0.0_f32;
    let mut sum_sq = 0.0_f64;
    for &s in samples.iter() {
        peak = peak.max(s.abs());
        sum_sq += (s as f64) * (s as f64);
    }
    if samples.is_empty() {
        return;
    }
    let rms = (sum_sq / samples.len() as f64).sqrt() as f32;

    // Avoid amplifying true silence.
    if peak < 1e-6 && rms < 1e-6 {
        return;
    }

    let target_peak = 0.90_f32;
    let target_rms = 0.10_f32; // ~ -20 dBFS
    let max_gain = 8.0_f32;

    let gain_peak = if peak > 0.0 { target_peak / peak } else { 1.0 };
    let gain_rms = if rms > 0.0 { target_rms / rms } else { 1.0 };
    let gain = gain_peak.min(gain_rms).clamp(0.1, max_gain);

    for s in samples.iter_mut() {
        *s = (*s * gain).clamp(-1.0, 1.0);
    }
}

fn apply_light_noise_suppression(samples: &mut [f32], sample_rate: u32) {
    // Extremely lightweight noise suppression:
    // estimate a noise floor from the first ~200ms and apply soft subtraction.
    if samples.is_empty() {
        return;
    }

    let sr = sample_rate.max(1) as usize;
    let window = (sr as f32 * 0.20) as usize; // ~200ms
    let n = window.clamp(1, samples.len());

    let mut sum_sq = 0.0_f64;
    for &s in samples.iter().take(n) {
        sum_sq += (s as f64) * (s as f64);
    }
    let floor_rms = (sum_sq / n as f64).sqrt() as f32;
    if !floor_rms.is_finite() || floor_rms <= 0.0 {
        return;
    }

    // Subtract most of the estimated floor; keep some to avoid pumping.
    let subtract = floor_rms * 0.8;
    for s in samples.iter_mut() {
        let a = s.abs();
        let sign = if *s >= 0.0 { 1.0 } else { -1.0 };
        let out = (a - subtract).max(0.0);
        *s = (sign * out).clamp(-1.0, 1.0);
    }
}

/// Apply a simple noise gate to interleaved samples.
///
/// - `samples` are interleaved f32 in [-1, 1]
/// - `channels` is the interleaving width
/// - `threshold_dbfs` is a negative dBFS value (e.g. -60). `None` => bypass.
///
/// This is intentionally lightweight and runs at stop-time (offline), not in the
/// real-time capture callback.
fn apply_noise_gate_interleaved(
    samples: &[f32],
    sample_rate: u32,
    channels: u16,
    threshold_dbfs: Option<f32>,
) -> Vec<f32> {
    let Some(threshold_dbfs) = threshold_dbfs else {
        return samples.to_vec();
    };
    if samples.is_empty() {
        return samples.to_vec();
    }

    // UI-range clamp. Keep conservative.
    let threshold_dbfs = if threshold_dbfs.is_finite() {
        threshold_dbfs.clamp(-75.0, -30.0)
    } else {
        -60.0
    };

    let channels_usize = channels.max(1) as usize;
    let frames = samples.len() / channels_usize;

    let threshold_amp = db_to_amp(threshold_dbfs);

    // Hysteresis reduces "chattering" around the threshold.
    let close_threshold_amp = threshold_amp * 0.85;

    // Attack/release smoothing on the *gain* to avoid clicks.
    let fs = sample_rate.max(1) as f32;
    let attack_s = 0.005_f32;
    let release_s = 0.120_f32;

    let attack_alpha = (-1.0 / (attack_s * fs)).exp();
    let release_alpha = (-1.0 / (release_s * fs)).exp();

    let mut out = Vec::with_capacity(samples.len());
    let mut gate_open = false;
    let mut gain: f32 = 0.0;

    for frame_idx in 0..frames {
        let base = frame_idx * channels_usize;

        // Envelope: max abs across channels for this frame.
        let mut env = 0.0_f32;
        for c in 0..channels_usize {
            env = env.max(samples[base + c].abs());
        }

        if gate_open {
            if env < close_threshold_amp {
                gate_open = false;
            }
        } else if env > threshold_amp {
            gate_open = true;
        }

        let target = if gate_open { 1.0_f32 } else { 0.0_f32 };
        let alpha = if target > gain { attack_alpha } else { release_alpha };
        gain = target + alpha * (gain - target);

        for c in 0..channels_usize {
            out.push(samples[base + c] * gain);
        }
    }

    // If there were trailing samples not forming a whole frame, just copy them.
    // (Shouldn't happen in normal capture, but keep it safe.)
    let consumed = frames * channels_usize;
    if consumed < samples.len() {
        out.extend_from_slice(&samples[consumed..]);
    }

    out
}

#[derive(Debug, Clone, Copy)]
pub struct AudioEncodeConfig {
    /// If set, apply a noise gate with the given threshold.
    pub noise_gate_threshold_dbfs: Option<f32>,
    /// Convert the captured audio to mono before WAV encoding.
    pub downmix_to_mono: bool,
    /// Resample to 16kHz before WAV encoding.
    pub resample_to_16khz: bool,
    /// Apply a lightweight high-pass (DC/rumble) filter.
    pub highpass_enabled: bool,
    /// Apply a lightweight gain normalization.
    pub agc_enabled: bool,
    /// Apply a lightweight noise suppression.
    pub noise_suppression_enabled: bool,
    /// If enabled, compute a best-effort speech presence boolean using WebRTC VAD.
    pub detect_speech_presence: bool,
}

impl Default for AudioEncodeConfig {
    fn default() -> Self {
        Self {
            noise_gate_threshold_dbfs: None,
            downmix_to_mono: true,
            resample_to_16khz: false,
            highpass_enabled: true,
            agc_enabled: false,
            noise_suppression_enabled: false,
            detect_speech_presence: false,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct AudioCaptureDiagnostics {
    pub stats: AudioLevelStats,
    pub speech_detected: Option<bool>,
}

/// Errors that can occur during audio capture
#[derive(Debug, thiserror::Error)]
pub enum AudioCaptureError {
    #[error("No input device available")]
    NoInputDevice,

    #[error("Failed to get device config: {0}")]
    DeviceConfig(String),

    #[error("Failed to build audio stream: {0}")]
    StreamBuild(String),

    #[error("Failed to start audio stream: {0}")]
    StreamStart(String),

    #[error("Failed to encode audio: {0}")]
    Encoding(String),

    #[error("Audio capture not active")]
    #[cfg_attr(not(test), allow(dead_code))]
    NotActive,

    #[error("Capture thread error: {0}")]
    #[cfg_attr(not(test), allow(dead_code))]
    ThreadError(String),
}

/// Audio buffer that accumulates samples during recording
#[derive(Debug, Clone)]
pub struct AudioBuffer {
    samples: Vec<f32>,
    sample_rate: u32,
    channels: u16,
    max_duration_secs: f32,
}

impl AudioBuffer {
    /// Create a new audio buffer with the specified parameters
    pub fn new(sample_rate: u32, channels: u16, max_duration_secs: f32) -> Self {
        let capacity = (sample_rate as f32 * max_duration_secs * channels as f32) as usize;
        Self {
            samples: Vec::with_capacity(capacity),
            sample_rate,
            channels,
            max_duration_secs,
        }
    }

    /// Append samples to the buffer
    pub fn append(&mut self, new_samples: &[f32]) {
        self.samples.extend_from_slice(new_samples);

        // Trim if exceeds max duration
        let max_samples =
            (self.sample_rate as f32 * self.max_duration_secs * self.channels as f32) as usize;
        if self.samples.len() > max_samples {
            let drain_count = self.samples.len() - max_samples;
            self.samples.drain(0..drain_count);
        }
    }

    /// Clear all samples from the buffer
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn clear(&mut self) {
        self.samples.clear();
    }

    /// Get the number of samples in the buffer
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn len(&self) -> usize {
        self.samples.len()
    }

    /// Check if the buffer is empty
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    /// Get the duration of audio in the buffer in seconds
    pub fn duration_secs(&self) -> f32 {
        self.samples.len() as f32 / (self.sample_rate as f32 * self.channels as f32)
    }

    /// Compute simple signal level statistics over the captured samples.
    ///
    /// Samples are expected to be normalized floats in [-1.0, 1.0].
    pub fn level_stats(&self) -> AudioLevelStats {
        let mut peak: f32 = 0.0;
        let mut sum_sq: f64 = 0.0;
        let mut n: u64 = 0;

        for &s in &self.samples {
            let a = s.abs();
            if a > peak {
                peak = a;
            }

            // Promote to f64 for numerical stability on long recordings.
            sum_sq += (s as f64) * (s as f64);
            n += 1;
        }

        let rms = if n == 0 {
            0.0
        } else {
            (sum_sq / n as f64).sqrt() as f32
        };

        AudioLevelStats {
            duration_secs: self.duration_secs(),
            rms,
            peak,
        }
    }

    /// Convert the buffer contents to WAV bytes
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn to_wav_bytes(&self) -> Result<Vec<u8>, AudioCaptureError> {
        self.to_wav_bytes_with_noise_gate(0)
    }

    /// Convert the buffer contents to WAV bytes, optionally applying an experimental noise gate.
    ///
    /// `noise_gate_strength` is 0..=100 where 0 disables the noise gate.
    pub fn to_wav_bytes_with_noise_gate(
        &self,
        noise_gate_strength: u8,
    ) -> Result<Vec<u8>, AudioCaptureError> {
        let strength = clamp_u8_0_100(noise_gate_strength);
        let threshold_dbfs = if strength == 0 {
            None
        } else {
            let t = strength as f32 / 100.0;
            Some(lerp(-75.0, -30.0, t))
        };

        let (wav_bytes, _diagnostics) = self.to_wav_bytes_with_config(AudioEncodeConfig {
            noise_gate_threshold_dbfs: threshold_dbfs,
            ..Default::default()
        })?;
        Ok(wav_bytes)
    }

    pub fn to_wav_bytes_with_config(
        &self,
        cfg: AudioEncodeConfig,
    ) -> Result<(Vec<u8>, AudioCaptureDiagnostics), AudioCaptureError> {
        let diagnostics = if cfg.detect_speech_presence {
            Some(detect_speech_presence(
                &self.samples,
                self.sample_rate,
                self.channels,
            ))
        } else {
            None
        };

        let mut processed_samples = if cfg.downmix_to_mono {
            downmix_interleaved_to_mono(&self.samples, self.channels as usize)
        } else {
            self.samples.to_vec()
        };

        let mut out_sample_rate = self.sample_rate;
        let out_channels: u16 = if cfg.downmix_to_mono { 1 } else { self.channels.max(1) };

        // If we didn't downmix, most processing is skipped (keeps code simple and predictable).
        if cfg.downmix_to_mono {
            if cfg.noise_suppression_enabled {
                apply_light_noise_suppression(&mut processed_samples, out_sample_rate);
            }
            if cfg.highpass_enabled {
                apply_highpass_dc_block(&mut processed_samples, out_sample_rate);
            }
            if cfg.agc_enabled {
                apply_agc(&mut processed_samples);
            }

            // Optional resample after filtering/gain.
            if cfg.resample_to_16khz && out_sample_rate != 16000 {
                processed_samples = crate::vad::resample_to_16khz(&processed_samples, out_sample_rate);
                out_sample_rate = 16000;
            }

            // Noise gate (mono)
            processed_samples = apply_noise_gate_interleaved(
                &processed_samples,
                out_sample_rate,
                1,
                cfg.noise_gate_threshold_dbfs,
            );
        } else {
            // Noise gate (interleaved)
            processed_samples = apply_noise_gate_interleaved(
                &processed_samples,
                out_sample_rate,
                out_channels,
                cfg.noise_gate_threshold_dbfs,
            );
        }

        let spec = WavSpec {
            channels: out_channels,
            sample_rate: out_sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };

        let mut cursor = Cursor::new(Vec::new());
        {
            let mut writer = WavWriter::new(&mut cursor, spec)
                .map_err(|e| AudioCaptureError::Encoding(e.to_string()))?;

            for &sample in &processed_samples {
                let sample_i16 = (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
                writer
                    .write_sample(sample_i16)
                    .map_err(|e| AudioCaptureError::Encoding(e.to_string()))?;
            }

            writer
                .finalize()
                .map_err(|e| AudioCaptureError::Encoding(e.to_string()))?;
        }

        Ok((
            cursor.into_inner(),
            AudioCaptureDiagnostics {
                stats: self.level_stats(),
                speech_detected: diagnostics,
            },
        ))
    }

    /// Get the sample rate
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Get the number of channels
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn channels(&self) -> u16 {
        self.channels
    }
}

/// Basic audio level metrics for gating/diagnostics.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct AudioLevelStats {
    pub duration_secs: f32,
    /// Root-mean-square amplitude in [0, 1].
    pub rms: f32,
    /// Peak (max absolute) amplitude in [0, 1].
    pub peak: f32,
}

fn detect_speech_presence(samples: &[f32], sample_rate: u32, channels: u16) -> bool {
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
    pub fn snapshot(&self) -> AudioWaveformSnapshot {
        self.inner.snapshot()
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
    pub fn snapshot(&self) -> AudioLevelSnapshot {
        self.inner.snapshot()
    }
}

#[derive(Debug)]
struct AudioWaveformMeter {
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
    fn snapshot(&self) -> AudioWaveformSnapshot {
        let seq = self.seq.load(Ordering::Relaxed);
        let mut mins = Vec::with_capacity(WAVEFORM_BINS);
        let mut maxes = Vec::with_capacity(WAVEFORM_BINS);
        for i in 0..WAVEFORM_BINS {
            mins.push(f32::from_bits(self.min_bits[i].load(Ordering::Relaxed)));
            maxes.push(f32::from_bits(self.max_bits[i].load(Ordering::Relaxed)));
        }
        AudioWaveformSnapshot { seq, mins, maxes }
    }

    fn update_from_f32_interleaved(&self, data: &[f32], channels: usize) {
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
struct AudioLevelMeter {
    seq: AtomicU64,
    rms_bits: AtomicU32,
    peak_bits: AtomicU32,
}

impl AudioLevelMeter {
    fn snapshot(&self) -> AudioLevelSnapshot {
        let seq = self.seq.load(Ordering::Relaxed);
        let rms = f32::from_bits(self.rms_bits.load(Ordering::Relaxed));
        let peak = f32::from_bits(self.peak_bits.load(Ordering::Relaxed));
        AudioLevelSnapshot { seq, rms, peak }
    }

    fn update(&self, rms: f32, peak: f32) {
        // Clamp to sane range and avoid NaNs propagating into the UI.
        let rms = if rms.is_finite() { rms.clamp(0.0, 1.0) } else { 0.0 };
        let peak = if peak.is_finite() { peak.clamp(0.0, 1.0) } else { 0.0 };

        self.rms_bits.store(rms.to_bits(), Ordering::Relaxed);
        self.peak_bits.store(peak.to_bits(), Ordering::Relaxed);
        self.seq.fetch_add(1, Ordering::Relaxed);
    }
}

/// Commands sent to the audio capture thread
enum CaptureCommand {
    Stop,
}

/// VAD events sent from the capture thread
#[derive(Debug, Clone)]
pub enum AudioCaptureEvent {
    /// Speech detected (with pre-roll audio)
    SpeechStart,
    /// Speech ended after hangover period
    SpeechEnd,
}

/// Configuration for VAD-based auto-stop
#[derive(Debug, Clone)]
pub struct VadAutoStopConfig {
    /// Enable VAD processing
    pub enabled: bool,
    /// Automatically stop recording when speech ends
    #[cfg_attr(not(test), allow(dead_code))]
    pub auto_stop: bool,
    /// VAD configuration
    pub vad_config: VadConfig,
}

impl Default for VadAutoStopConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            auto_stop: false,
            vad_config: VadConfig::default(),
        }
    }
}

/// Handle to a running audio capture session
struct CaptureHandle {
    command_tx: mpsc::Sender<CaptureCommand>,
    #[cfg_attr(not(test), allow(dead_code))]
    event_rx: mpsc::Receiver<AudioCaptureEvent>,
    thread_handle: JoinHandle<Result<(), AudioCaptureError>>,
}

/// Thread-safe audio capture manager
///
/// This runs audio capture in a separate thread to avoid Send/Sync issues
/// with cpal::Stream. The captured audio is stored in a shared buffer.
pub struct AudioCapture {
    buffer: Arc<StdMutex<AudioBuffer>>,
    capture_handle: Option<CaptureHandle>,
    sample_rate: u32,
    channels: u16,
    vad_config: VadAutoStopConfig,

    // Most recent realtime level stats (for UI metering / overlay waveform).
    level_meter: Arc<AudioLevelMeter>,

    // Most recent realtime waveform buckets (for true waveform rendering).
    waveform_meter: Arc<AudioWaveformMeter>,
}

impl AudioCapture {
    /// Create a new audio capture instance
    pub fn new() -> Self {
        Self {
            buffer: Arc::new(StdMutex::new(AudioBuffer::new(44100, 1, 300.0))),
            capture_handle: None,
            sample_rate: 44100,
            channels: 1,
            vad_config: VadAutoStopConfig::default(),
            level_meter: Arc::new(AudioLevelMeter::default()),
            waveform_meter: Arc::new(AudioWaveformMeter::default()),
        }
    }

    /// Create a new audio capture instance with VAD configuration
    pub fn with_vad_config(vad_config: VadAutoStopConfig) -> Self {
        Self {
            buffer: Arc::new(StdMutex::new(AudioBuffer::new(44100, 1, 300.0))),
            capture_handle: None,
            sample_rate: 44100,
            channels: 1,
            vad_config,
            level_meter: Arc::new(AudioLevelMeter::default()),
            waveform_meter: Arc::new(AudioWaveformMeter::default()),
        }
    }

    /// Get the most recent realtime input level snapshot.
    ///
    /// This is updated continuously while recording (per CPAL callback). When not recording,
    /// it returns the last observed values.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn level_snapshot(&self) -> AudioLevelSnapshot {
        self.level_meter.snapshot()
    }

    pub fn shared_level_meter(&self) -> SharedAudioLevelMeter {
        SharedAudioLevelMeter {
            inner: self.level_meter.clone(),
        }
    }

    pub fn shared_waveform_meter(&self) -> SharedAudioWaveformMeter {
        SharedAudioWaveformMeter {
            inner: self.waveform_meter.clone(),
        }
    }

    /// Update VAD configuration
    pub fn set_vad_config(&mut self, config: VadAutoStopConfig) {
        self.vad_config = config;
    }

    /// Get the current VAD configuration
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn vad_config(&self) -> &VadAutoStopConfig {
        &self.vad_config
    }

    /// Start recording audio from the default input device.
    ///
    /// Prefer `start_with_device_name` when you need to honor a user-selected mic.
    ///
    /// # Arguments
    /// * `max_duration_secs` - Maximum recording duration in seconds (for buffer sizing)
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn start(&mut self, max_duration_secs: f32) -> Result<(), AudioCaptureError> {
        self.start_with_device_name(max_duration_secs, None)
    }

    /// Start recording audio from a specific input device (by CPAL device name),
    /// falling back to the system default if not found.
    pub fn start_with_device_name(
        &mut self,
        max_duration_secs: f32,
        input_device_name: Option<&str>,
    ) -> Result<(), AudioCaptureError> {
        // Stop any existing recording
        self.stop();

        // Get device info first (on main thread)
        let host = cpal::default_host();

        let desired_name = input_device_name
            .map(str::trim)
            .filter(|s| !s.is_empty() && *s != "default");

        let mut selected: Option<cpal::Device> = None;
        if let Some(name) = desired_name {
            if let Ok(devices) = host.input_devices() {
                for d in devices {
                    let Ok(n) = d.description() else { continue };
                    let n = n.to_string();
                    // Backwards-compatible matching: older settings may have stored just the
                    // plain device name (cpal `name()`), while newer CPAL versions recommend
                    // using `description()`.
                    if n == name || n.contains(name) {
                        selected = Some(d);
                        break;
                    }
                }
            }
        }

        let device = match selected {
            Some(d) => {
                log::info!("Using selected input device: {}", desired_name.unwrap_or("<unknown>"));
                d
            }
            None => {
                if let Some(name) = desired_name {
                    log::warn!(
                        "Selected input device '{}' not found; falling back to default input device",
                        name
                    );
                }
                host.default_input_device()
                    .ok_or(AudioCaptureError::NoInputDevice)?
            }
        };

        let config = device
            .default_input_config()
            .map_err(|e| AudioCaptureError::DeviceConfig(e.to_string()))?;

        self.sample_rate = config.sample_rate();
        self.channels = config.channels();

        log::info!(
            "Audio config: {} Hz, {} channels, {:?}",
            self.sample_rate,
            self.channels,
            config.sample_format()
        );

        // Create new buffer with correct params
        self.buffer = Arc::new(StdMutex::new(AudioBuffer::new(
            self.sample_rate,
            self.channels,
            max_duration_secs,
        )));

        let buffer_clone = self.buffer.clone();
        let meter = self.level_meter.clone();
        let waveform_meter = self.waveform_meter.clone();
        let (command_tx, command_rx) = mpsc::channel();
        let (event_tx, event_rx) = mpsc::channel();
        let sample_format = config.sample_format();
        let stream_config: cpal::StreamConfig = config.into();
        let vad_config = self.vad_config.clone();
        let sample_rate = self.sample_rate;

        // Spawn capture thread
        let thread_handle = thread::spawn(move || {
            run_capture_thread(
                device,
                stream_config,
                sample_format,
                buffer_clone,
                meter,
                waveform_meter,
                command_rx,
                event_tx,
                vad_config,
                sample_rate,
            )
        });

        self.capture_handle = Some(CaptureHandle {
            command_tx,
            event_rx,
            thread_handle,
        });

        log::info!("Audio capture started");
        Ok(())
    }

    /// Stop recording and return the captured audio as WAV bytes
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn stop_and_get_wav(&mut self) -> Result<Vec<u8>, AudioCaptureError> {
        self.stop_and_get_wav_with_noise_gate(0)
    }

    /// Stop recording and return the captured audio as WAV bytes, applying an optional noise gate.
    ///
    /// `noise_gate_strength` is 0..=100 where 0 disables the noise gate.
    pub fn stop_and_get_wav_with_noise_gate(
        &mut self,
        noise_gate_strength: u8,
    ) -> Result<Vec<u8>, AudioCaptureError> {
        let (wav_bytes, _diag) = self.stop_and_get_wav_with_stats_with_noise_gate(noise_gate_strength)?;
        Ok(wav_bytes)
    }

    /// Stop recording and return the captured audio as WAV bytes along with level stats.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn stop_and_get_wav_with_stats(
        &mut self,
    ) -> Result<(Vec<u8>, AudioLevelStats), AudioCaptureError> {
        self.stop_and_get_wav_with_stats_with_noise_gate(0)
    }

    /// Stop recording and return WAV bytes + level stats, optionally applying an experimental noise gate.
    ///
    /// Note: stats are computed on the *raw* (pre-gate) samples.
    pub fn stop_and_get_wav_with_stats_with_noise_gate(
        &mut self,
        noise_gate_strength: u8,
    ) -> Result<(Vec<u8>, AudioLevelStats), AudioCaptureError> {
        self.stop();

        let buffer = self
            .buffer
            .lock()
            .map_err(|_| AudioCaptureError::Encoding("Failed to lock buffer".to_string()))?;

        let stats = buffer.level_stats();
        let (wav_bytes, _diag) = buffer.to_wav_bytes_with_config(AudioEncodeConfig {
            noise_gate_threshold_dbfs: {
                let strength = clamp_u8_0_100(noise_gate_strength);
                if strength == 0 {
                    None
                } else {
                    let t = strength as f32 / 100.0;
                    Some(lerp(-75.0, -30.0, t))
                }
            },
            ..Default::default()
        })?;

        log::info!(
            "Audio capture stopped, {} bytes captured (duration {:.2}s, rms {:.6}, peak {:.6})",
            wav_bytes.len(),
            stats.duration_secs,
            stats.rms,
            stats.peak
        );

        Ok((wav_bytes, stats))
    }

    /// Stop recording and return WAV bytes + diagnostics, applying preprocessing.
    pub fn stop_and_get_wav_with_diagnostics(
        &mut self,
        cfg: AudioEncodeConfig,
    ) -> Result<(Vec<u8>, AudioCaptureDiagnostics), AudioCaptureError> {
        self.stop();

        let buffer = self
            .buffer
            .lock()
            .map_err(|_| AudioCaptureError::Encoding("Failed to lock buffer".to_string()))?;

        buffer.to_wav_bytes_with_config(cfg)
    }

    /// Stop recording and return two WAV encodes of the same captured audio:
    /// - "before": raw, with no preprocessing/gates
    /// - "after": encoded with the provided config
    ///
    /// This is intended for UI A/B testing of audio settings.
    pub fn stop_and_get_wav_before_after(
        &mut self,
        after_cfg: AudioEncodeConfig,
    ) -> Result<(Vec<u8>, Vec<u8>, AudioCaptureDiagnostics), AudioCaptureError> {
        self.stop();

        let buffer = self
            .buffer
            .lock()
            .map_err(|_| AudioCaptureError::Encoding("Failed to lock buffer".to_string()))?;

        // "Before": as-captured (no downmix/resample/filters/gates).
        let (before_wav, _before_diag) = buffer.to_wav_bytes_with_config(AudioEncodeConfig {
            noise_gate_threshold_dbfs: None,
            downmix_to_mono: false,
            resample_to_16khz: false,
            highpass_enabled: false,
            agc_enabled: false,
            noise_suppression_enabled: false,
            detect_speech_presence: false,
        })?;

        // "After": apply current user settings.
        let (after_wav, after_diag) = buffer.to_wav_bytes_with_config(after_cfg)?;

        Ok((before_wav, after_wav, after_diag))
    }

    /// Stop recording without returning audio data
    pub fn stop(&mut self) {
        if let Some(handle) = self.capture_handle.take() {
            log::info!("Stopping audio capture");
            // Send stop command (ignore error if thread already stopped)
            let _ = handle.command_tx.send(CaptureCommand::Stop);
            // Wait for thread to finish (with timeout in case of issues)
            let _ = handle.thread_handle.join();
        }
    }

    /// Check if currently recording
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn is_recording(&self) -> bool {
        self.capture_handle.is_some()
    }

    /// Poll for VAD events (non-blocking)
    ///
    /// Returns the next VAD event if one is available, or None if no events are pending.
    /// This should be called periodically to check for speech start/end events.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn poll_vad_event(&self) -> Option<AudioCaptureEvent> {
        if let Some(ref handle) = self.capture_handle {
            handle.event_rx.try_recv().ok()
        } else {
            None
        }
    }

    /// Check if VAD auto-stop is enabled
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn is_vad_auto_stop_enabled(&self) -> bool {
        self.vad_config.enabled && self.vad_config.auto_stop
    }

    /// Get the duration of recorded audio in seconds
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn duration_secs(&self) -> f32 {
        self.buffer
            .lock()
            .map(|b| b.duration_secs())
            .unwrap_or(0.0)
    }

    /// Get the sample rate
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Get the number of channels
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn channels(&self) -> u16 {
        self.channels
    }
}

impl Default for AudioCapture {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for AudioCapture {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Run the audio capture in a dedicated thread
fn run_capture_thread(
    device: cpal::Device,
    config: cpal::StreamConfig,
    sample_format: SampleFormat,
    buffer: Arc<StdMutex<AudioBuffer>>,
    meter: Arc<AudioLevelMeter>,
    waveform_meter: Arc<AudioWaveformMeter>,
    command_rx: mpsc::Receiver<CaptureCommand>,
    event_tx: mpsc::Sender<AudioCaptureEvent>,
    vad_config: VadAutoStopConfig,
    sample_rate: u32,
) -> Result<(), AudioCaptureError> {
    use cpal::Sample;

    let err_fn = |err| {
        log::error!("Audio stream error: {}", err);
    };

    // Create a channel for passing samples to the VAD processing thread
    let (vad_samples_tx, vad_samples_rx): (mpsc::Sender<Vec<f32>>, mpsc::Receiver<Vec<f32>>) =
        mpsc::channel();

    // Spawn a separate thread for VAD processing (since webrtc-vad is not Send)
    let vad_handle = if vad_config.enabled {
        let event_tx_clone = event_tx.clone();
        let vad_cfg = vad_config.vad_config.clone();
        Some(thread::spawn(move || {
            let mut processor = VadFrameProcessor::new(vad_cfg, sample_rate);
            log::info!("VAD processor initialized for {} Hz audio in dedicated thread", sample_rate);

            loop {
                match vad_samples_rx.recv_timeout(std::time::Duration::from_millis(100)) {
                    Ok(samples) => {
                        for event in processor.process(&samples) {
                            let capture_event = match event {
                                VadEvent::SpeechStart { .. } => AudioCaptureEvent::SpeechStart,
                                VadEvent::SpeechEnd => AudioCaptureEvent::SpeechEnd,
                                VadEvent::None => continue,
                            };
                            let _ = event_tx_clone.send(capture_event);
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => continue,
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }
        }))
    } else {
        None
    };

    let stream = match sample_format {
        SampleFormat::F32 => {
            let buffer = buffer.clone();
            let meter = meter.clone();
            let waveform_meter = waveform_meter.clone();
            let vad_tx = if vad_config.enabled { Some(vad_samples_tx.clone()) } else { None };
            let channels = config.channels as usize;
            device.build_input_stream(
                &config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    // Realtime meter (cheap math, no allocations).
                    let mut peak: f32 = 0.0;
                    let mut sum_sq: f64 = 0.0;
                    let mut n: u64 = 0;
                    for &s in data {
                        let a = s.abs();
                        if a > peak {
                            peak = a;
                        }
                        sum_sq += (s as f64) * (s as f64);
                        n += 1;
                    }
                    let rms = if n == 0 { 0.0 } else { (sum_sq / n as f64).sqrt() as f32 };
                    meter.update(rms, peak);

                    // True waveform buckets for UI.
                    waveform_meter.update_from_f32_interleaved(data, channels);

                    // Store audio in buffer
                    if let Ok(mut buf) = buffer.lock() {
                        buf.append(data);
                    }

                    // Send samples to VAD thread if enabled
                    if let Some(ref tx) = vad_tx {
                        let mono = if channels > 1 {
                            downmix_interleaved_chunk_to_mono(data, channels)
                        } else {
                            data.to_vec()
                        };
                        let _ = tx.send(mono);
                    }
                },
                err_fn,
                None,
            )
        }
        SampleFormat::I16 => {
            let buffer = buffer.clone();
            let meter = meter.clone();
            let waveform_meter = waveform_meter.clone();
            let vad_tx = if vad_config.enabled { Some(vad_samples_tx.clone()) } else { None };
            let channels = config.channels as usize;
            device.build_input_stream(
                &config,
                move |data: &[i16], _: &cpal::InputCallbackInfo| {
                    let mut peak: f32 = 0.0;
                    let mut sum_sq: f64 = 0.0;
                    let samples: Vec<f32> = data
                        .iter()
                        .map(|&s| {
                            let f = s.to_float_sample();
                            let a = f.abs();
                            if a > peak {
                                peak = a;
                            }
                            sum_sq += (f as f64) * (f as f64);
                            f
                        })
                        .collect();
                    let n = samples.len() as u64;
                    let rms = if n == 0 { 0.0 } else { (sum_sq / n as f64).sqrt() as f32 };
                    meter.update(rms, peak);

                    // True waveform buckets for UI.
                    waveform_meter.update_from_f32_interleaved(&samples, channels);

                    // Store audio in buffer
                    if let Ok(mut buf) = buffer.lock() {
                        buf.append(&samples);
                    }

                    // Send samples to VAD thread if enabled
                    if let Some(ref tx) = vad_tx {
                        let mono = if channels > 1 {
                            downmix_interleaved_chunk_to_mono(&samples, channels)
                        } else {
                            samples
                        };
                        let _ = tx.send(mono);
                    }
                },
                err_fn,
                None,
            )
        }
        SampleFormat::U16 => {
            let buffer = buffer.clone();
            let meter = meter.clone();
            let waveform_meter = waveform_meter.clone();
            let vad_tx = if vad_config.enabled { Some(vad_samples_tx.clone()) } else { None };
            let channels = config.channels as usize;
            device.build_input_stream(
                &config,
                move |data: &[u16], _: &cpal::InputCallbackInfo| {
                    let mut peak: f32 = 0.0;
                    let mut sum_sq: f64 = 0.0;
                    let samples: Vec<f32> = data
                        .iter()
                        .map(|&s| {
                            let f = s.to_float_sample();
                            let a = f.abs();
                            if a > peak {
                                peak = a;
                            }
                            sum_sq += (f as f64) * (f as f64);
                            f
                        })
                        .collect();
                    let n = samples.len() as u64;
                    let rms = if n == 0 { 0.0 } else { (sum_sq / n as f64).sqrt() as f32 };
                    meter.update(rms, peak);

                    // True waveform buckets for UI.
                    waveform_meter.update_from_f32_interleaved(&samples, channels);

                    // Store audio in buffer
                    if let Ok(mut buf) = buffer.lock() {
                        buf.append(&samples);
                    }

                    // Send samples to VAD thread if enabled
                    if let Some(ref tx) = vad_tx {
                        let mono = if channels > 1 {
                            downmix_interleaved_chunk_to_mono(&samples, channels)
                        } else {
                            samples
                        };
                        let _ = tx.send(mono);
                    }
                },
                err_fn,
                None,
            )
        }
        _ => {
            return Err(AudioCaptureError::DeviceConfig(format!(
                "Unsupported sample format: {:?}",
                sample_format
            )));
        }
    }
    .map_err(|e| AudioCaptureError::StreamBuild(e.to_string()))?;

    stream
        .play()
        .map_err(|e| AudioCaptureError::StreamStart(e.to_string()))?;

    // Wait for stop command
    loop {
        match command_rx.recv_timeout(std::time::Duration::from_millis(100)) {
            Ok(CaptureCommand::Stop) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    // Drop the VAD sender to signal the VAD thread to stop
    drop(vad_samples_tx);

    // Wait for VAD thread to finish
    if let Some(handle) = vad_handle {
        let _ = handle.join();
    }

    // Stream is dropped here, stopping capture
    Ok(())
}

/// Get the list of available input devices
#[cfg_attr(not(test), allow(dead_code))]
pub fn list_input_devices() -> Vec<String> {
    let host = cpal::default_host();
    host.input_devices()
        .map(|devices| {
            devices
                .filter_map(|d| d.description().ok().map(|desc| desc.to_string()))
                .collect()
        })
        .unwrap_or_default()
}

/// Get information about the default input device
#[cfg_attr(not(test), allow(dead_code))]
pub fn get_default_input_device_info() -> Option<(String, u32, u16)> {
    let host = cpal::default_host();
    let device = host.default_input_device()?;
    let name = device.description().ok()?.to_string();
    let config = device.default_input_config().ok()?;
    Some((name, config.sample_rate(), config.channels()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_buffer_creation() {
        let buffer = AudioBuffer::new(16000, 1, 60.0);
        assert!(buffer.is_empty());
        assert_eq!(buffer.sample_rate(), 16000);
        assert_eq!(buffer.channels(), 1);
    }

    #[test]
    fn test_audio_buffer_append() {
        let mut buffer = AudioBuffer::new(16000, 1, 60.0);
        buffer.append(&[0.5, -0.5, 0.0]);
        assert_eq!(buffer.len(), 3);
    }

    #[test]
    fn test_audio_buffer_clear() {
        let mut buffer = AudioBuffer::new(16000, 1, 60.0);
        buffer.append(&[0.5, -0.5, 0.0]);
        buffer.clear();
        assert!(buffer.is_empty());
    }

    #[test]
    fn test_audio_buffer_to_wav() {
        let mut buffer = AudioBuffer::new(16000, 1, 60.0);
        // Add some test samples
        buffer.append(&[0.0; 1600]); // 0.1 seconds of silence
        let wav_bytes = buffer.to_wav_bytes().expect("Failed to encode WAV");

        // WAV header is 44 bytes, plus samples
        assert!(wav_bytes.len() > 44);
        // Check WAV magic bytes "RIFF"
        assert_eq!(&wav_bytes[0..4], b"RIFF");
    }

    #[test]
    fn test_audio_buffer_max_duration() {
        let mut buffer = AudioBuffer::new(1000, 1, 1.0); // 1 second max
        // Add 2 seconds worth of samples
        buffer.append(&[0.0; 2000]);
        // Should be trimmed to 1 second
        assert_eq!(buffer.len(), 1000);
    }
}
