//! Audio capture module using cpal for cross-platform audio input.
//!
//! This module provides functionality to capture audio from the system's
//! default input device and encode it to WAV format for STT processing.
//!
//! Supports optional Voice Activity Detection (VAD) for auto-stop functionality.

use crate::vad::{VadConfig, VadEvent, VadFrameProcessor};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::SampleFormat;
use hound::{WavSpec, WavWriter};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::io::Cursor;
use std::sync::mpsc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::thread::{self, JoinHandle};

const MIC_DEVICE_ID_PREFIX: &str = "mic:v1:";

/// Public device descriptor for the frontend.
///
/// NOTE: `id` is a stable-ish *selection token* for this session, not a true OS device ID.
/// It is guaranteed unique within the returned list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioInputDeviceInfo {
    pub id: String,
    pub name: String,
}

fn encode_mic_device_id(name: &str, ordinal_for_name: usize) -> String {
    // Base64url without padding so it is easy to embed in a string.
    let name_b64 = URL_SAFE_NO_PAD.encode(name.as_bytes());
    format!("{MIC_DEVICE_ID_PREFIX}{name_b64}:{ordinal_for_name}")
}

fn decode_mic_device_id(id: &str) -> Option<(String, usize)> {
    // Format: mic:v1:<base64url(name)>:<ordinal>
    let rest = id.strip_prefix(MIC_DEVICE_ID_PREFIX)?;
    let mut parts = rest.rsplitn(2, ':');
    let ordinal_str = parts.next()?;
    let name_b64 = parts.next()?;
    let ordinal = ordinal_str.parse::<usize>().ok()?;
    let name_bytes = URL_SAFE_NO_PAD.decode(name_b64).ok()?;
    let name = String::from_utf8(name_bytes).ok()?;
    Some((name, ordinal))
}

fn normalize_input_device_selection(input_device: Option<&str>) -> Option<(String, usize, bool)> {
    // Returns (desired_name, desired_ordinal, is_encoded_id)
    let raw = input_device
        .map(str::trim)
        .filter(|s| !s.is_empty() && *s != "default")?;

    if let Some((name, ordinal)) = decode_mic_device_id(raw) {
        return Some((name, ordinal, true));
    }

    Some((raw.to_string(), 0, false))
}

fn select_input_device_from_host(
    host: &cpal::Host,
    selection: Option<&str>,
) -> Option<cpal::Device> {
    let (desired_name, desired_ordinal, is_encoded) =
        normalize_input_device_selection(selection)?;

    // Prefer exact-name matching with ordinal disambiguation.
    let mut ordinal_for_name: usize = 0;
    if let Ok(devices) = host.input_devices() {
        for d in devices {
            let Ok(desc) = d.description() else { continue };
            let name = desc.to_string();
            if name == desired_name {
                if ordinal_for_name == desired_ordinal {
                    return Some(d);
                }
                ordinal_for_name = ordinal_for_name.saturating_add(1);
            }
        }
    }

    // Legacy fallback: some older stored values used partial matches.
    // For encoded IDs, do NOT do a contains() fallback (could pick the wrong device).
    if is_encoded {
        return None;
    }

    if let Ok(devices) = host.input_devices() {
        for d in devices {
            let Ok(desc) = d.description() else { continue };
            let name = desc.to_string();
            if name.contains(&desired_name) {
                return Some(d);
            }
        }
    }

    None
}

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
    #[allow(dead_code)]
    NotActive,

    #[error("Capture thread error: {0}")]
    #[allow(dead_code)]
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

    /// Update the buffer's format (sample rate + channels) without clearing samples.
    ///
    /// This is primarily used by long-running streams (Hot Mic) when the underlying
    /// device is restarted.
    pub fn set_format(&mut self, sample_rate: u32, channels: u16) {
        self.sample_rate = sample_rate;
        self.channels = channels.max(1);
    }

    /// Reset the buffer for a new recording session.
    ///
    /// Clears samples and sets the max duration (used for trimming during capture).
    pub fn reset_for_recording(&mut self, max_duration_secs: f32) {
        self.samples.clear();
        self.max_duration_secs = max_duration_secs.max(0.0);
        let capacity =
            (self.sample_rate as f32 * self.max_duration_secs * self.channels as f32) as usize;
        self.samples.reserve(capacity.saturating_sub(self.samples.capacity()));
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

/// A fixed-capacity rolling buffer for pre-roll audio.
///
/// Stores the last N *samples* (interleaved f32 PCM). This is designed to be
/// cheap to push to from the CPAL input callback.
#[derive(Debug, Clone)]
struct RollingBuffer {
    data: Vec<f32>,
    write_pos: usize,
    filled: usize,
}

impl RollingBuffer {
    fn new(capacity_samples: usize) -> Self {
        let cap = capacity_samples;
        Self {
            data: vec![0.0; cap],
            write_pos: 0,
            filled: 0,
        }
    }

    fn capacity(&self) -> usize {
        self.data.len()
    }

    fn clear(&mut self) {
        self.write_pos = 0;
        self.filled = 0;
    }

    fn set_capacity(&mut self, capacity_samples: usize) {
        if capacity_samples == self.capacity() {
            return;
        }

        if capacity_samples == 0 {
            self.data.clear();
            self.write_pos = 0;
            self.filled = 0;
            return;
        }

        let snapshot = self.snapshot();
        let keep = if snapshot.len() > capacity_samples {
            snapshot[snapshot.len() - capacity_samples..].to_vec()
        } else {
            snapshot
        };

        let mut data = vec![0.0; capacity_samples];
        let n = keep.len().min(capacity_samples);
        data[..n].copy_from_slice(&keep[..n]);

        self.data = data;
        self.filled = n;
        self.write_pos = if self.filled < self.capacity() { self.filled } else { 0 };
    }

    fn push(&mut self, samples: &[f32]) {
        let cap = self.capacity();
        if cap == 0 || samples.is_empty() {
            return;
        }

        // Fast path: if input is larger than the whole buffer, keep only the tail.
        if samples.len() >= cap {
            let tail = &samples[samples.len() - cap..];
            self.data.copy_from_slice(tail);
            self.write_pos = 0;
            self.filled = cap;
            return;
        }

        for &s in samples {
            self.data[self.write_pos] = s;
            self.write_pos += 1;
            if self.write_pos >= cap {
                self.write_pos = 0;
            }
            if self.filled < cap {
                self.filled += 1;
            }
        }
    }

    fn snapshot(&self) -> Vec<f32> {
        let cap = self.capacity();
        if self.filled == 0 || cap == 0 {
            return Vec::new();
        }

        if self.filled < cap {
            // Not wrapped yet; oldest is at 0.
            return self.data[..self.filled].to_vec();
        }

        // Wrapped / full: oldest is at write_pos.
        let mut out = Vec::with_capacity(cap);
        out.extend_from_slice(&self.data[self.write_pos..]);
        if self.write_pos > 0 {
            out.extend_from_slice(&self.data[..self.write_pos]);
        }
        out
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
    pre_roll: Arc<StdMutex<RollingBuffer>>,
    capture_handle: Option<CaptureHandle>,
    sample_rate: u32,
    channels: u16,
    vad_config: VadAutoStopConfig,

    // Capture behavior settings (synced from PipelineConfig)
    hot_mic_enabled: bool,
    pre_roll_ms: Arc<AtomicU32>,
    mic_auto_recover_enabled: Arc<AtomicBool>,
    desired_device_name: Arc<StdMutex<Option<String>>>,

    // Recording session state shared with the capture thread.
    recording_active: Arc<AtomicBool>,

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
            pre_roll: Arc::new(StdMutex::new(RollingBuffer::new(0))),
            capture_handle: None,
            sample_rate: 44100,
            channels: 1,
            vad_config: VadAutoStopConfig::default(),

            hot_mic_enabled: false,
            pre_roll_ms: Arc::new(AtomicU32::new(1500)),
            mic_auto_recover_enabled: Arc::new(AtomicBool::new(false)),
            desired_device_name: Arc::new(StdMutex::new(None)),
            recording_active: Arc::new(AtomicBool::new(false)),
            level_meter: Arc::new(AudioLevelMeter::default()),
            waveform_meter: Arc::new(AudioWaveformMeter::default()),
        }
    }

    /// Create a new audio capture instance with VAD configuration
    pub fn with_vad_config(vad_config: VadAutoStopConfig) -> Self {
        Self {
            buffer: Arc::new(StdMutex::new(AudioBuffer::new(44100, 1, 300.0))),
            pre_roll: Arc::new(StdMutex::new(RollingBuffer::new(0))),
            capture_handle: None,
            sample_rate: 44100,
            channels: 1,
            vad_config,

            hot_mic_enabled: false,
            pre_roll_ms: Arc::new(AtomicU32::new(1500)),
            mic_auto_recover_enabled: Arc::new(AtomicBool::new(false)),
            desired_device_name: Arc::new(StdMutex::new(None)),
            recording_active: Arc::new(AtomicBool::new(false)),
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

    /// Update capture behavior settings (Hot Mic + auto-recovery) and apply them.
    ///
    /// - When `hot_mic_enabled` is true, the input stream is kept open while idle.
    /// - When false, the stream is only opened on an explicit recording start.
    pub fn set_capture_behavior(
        &mut self,
        hot_mic_enabled: bool,
        hot_mic_pre_roll_ms: u32,
        mic_auto_recover_enabled: bool,
        input_device_name: Option<&str>,
    ) -> Result<(), AudioCaptureError> {
        self.hot_mic_enabled = hot_mic_enabled;
        self.pre_roll_ms
            .store(hot_mic_pre_roll_ms.min(5000), Ordering::Relaxed);
        self.mic_auto_recover_enabled
            .store(mic_auto_recover_enabled, Ordering::Relaxed);

        // Update desired device name for potential restarts.
        let desired_name = input_device_name
            .map(str::trim)
            .filter(|s| !s.is_empty() && *s != "default")
            .map(|s| s.to_string());
        if let Ok(mut lock) = self.desired_device_name.lock() {
            *lock = desired_name;
        }

        if hot_mic_enabled {
            // Keep the stream open while idle.
            self.ensure_stream_running(input_device_name)?;
        } else {
            // If we are not recording, close the stream to match push-to-talk behavior.
            if !self.recording_active.load(Ordering::Relaxed) {
                self.stop();
            }
        }

        Ok(())
    }

    /// Start a recording session.
    ///
    /// - In Hot Mic mode, this reuses the always-on stream and prepends the rolling pre-roll.
    /// - In push-to-talk mode, this starts a new stream and records normally.
    pub fn start_recording_session(
        &mut self,
        max_duration_secs: f32,
        input_device_name: Option<&str>,
    ) -> Result<(), AudioCaptureError> {
        if self.hot_mic_enabled {
            self.ensure_stream_running(input_device_name)?;
            self.begin_recording_with_pre_roll(max_duration_secs);
            return Ok(());
        }

        // Push-to-talk: start a new capture stream and record.
        self.start_with_device_name(max_duration_secs, input_device_name)
    }

    fn begin_recording_with_pre_roll(&mut self, max_duration_secs: f32) {
        // Snapshot pre-roll first (avoid holding lock while mutating recording buffer).
        let pre_roll = self
            .pre_roll
            .lock()
            .map(|b| b.snapshot())
            .unwrap_or_default();

        if let Ok(mut buf) = self.buffer.lock() {
            buf.reset_for_recording(max_duration_secs);
            if !pre_roll.is_empty() {
                buf.append(&pre_roll);
            }
        }

        self.recording_active.store(true, Ordering::Relaxed);
    }

    /// Stop recording. In Hot Mic mode, keeps the stream open.
    pub fn stop_recording(&mut self) {
        self.recording_active.store(false, Ordering::Relaxed);
        if !self.hot_mic_enabled {
            self.stop();
        }
    }

    fn ensure_stream_running(
        &mut self,
        input_device_name: Option<&str>,
    ) -> Result<(), AudioCaptureError> {
        if self.capture_handle.is_some() {
            // Stream already running; ensure pre-roll buffer capacity matches current config.
            let pre_ms = self.pre_roll_ms.load(Ordering::Relaxed) as f32;
            let (sr, ch) = self
                .buffer
                .lock()
                .map(|b| (b.sample_rate(), b.channels()))
                .unwrap_or((self.sample_rate, self.channels));
            let cap_samples = ((sr as f32 * (pre_ms / 1000.0) * ch as f32) as usize).min(10_000_000);
            if let Ok(mut pr) = self.pre_roll.lock() {
                pr.set_capacity(cap_samples);
            }
            return Ok(());
        }

        // Start a stream in "armed" mode (recording_active=false).
        // Reuse the existing device selection logic in start_with_device_name but
        // do not mark recording as active.
        self.start_stream_only(input_device_name)
    }

    fn start_stream_only(&mut self, input_device_name: Option<&str>) -> Result<(), AudioCaptureError> {
        // Stop any existing stream (defensive)
        self.stop();

        let host = cpal::default_host();

        let device = select_input_device_from_host(&host, input_device_name)
            .or_else(|| host.default_input_device())
            .ok_or(AudioCaptureError::NoInputDevice)?;

        let config = device
            .default_input_config()
            .map_err(|e| AudioCaptureError::DeviceConfig(e.to_string()))?;

        self.sample_rate = config.sample_rate();
        self.channels = config.channels().max(1);

        // Update pre-roll buffer capacity based on current stream format.
        let pre_ms = self.pre_roll_ms.load(Ordering::Relaxed) as f32;
        let cap_samples =
            ((self.sample_rate as f32 * (pre_ms / 1000.0) * self.channels as f32) as usize)
                .min(10_000_000);
        if let Ok(mut pr) = self.pre_roll.lock() {
            pr.set_capacity(cap_samples);
            pr.clear();
        }

        // Ensure the recording buffer format matches. Keep max_duration as-is for now.
        if let Ok(mut buf) = self.buffer.lock() {
            buf.set_format(self.sample_rate, self.channels);
        }

        // Prepare shared state for the capture thread.
        self.recording_active.store(false, Ordering::Relaxed);

        let buffer_clone = self.buffer.clone();
        let pre_roll_clone = self.pre_roll.clone();
        let recording_active = self.recording_active.clone();
        let meter = self.level_meter.clone();
        let waveform_meter = self.waveform_meter.clone();

        let (command_tx, command_rx) = mpsc::channel();
        let (event_tx, event_rx) = mpsc::channel();
        let sample_format = config.sample_format();
        let stream_config: cpal::StreamConfig = config.into();
        let vad_config = self.vad_config.clone();
        let pre_roll_ms = self.pre_roll_ms.clone();
        let auto_recover = self.mic_auto_recover_enabled.clone();
        let desired_device_name = self.desired_device_name.clone();

        // Spawn capture thread
        let thread_handle = thread::spawn(move || {
            run_capture_thread(
                device,
                stream_config,
                sample_format,
                buffer_clone,
                pre_roll_clone,
                recording_active,
                pre_roll_ms,
                meter,
                waveform_meter,
                command_rx,
                event_tx,
                vad_config,
                auto_recover,
                desired_device_name,
            )
        });

        self.capture_handle = Some(CaptureHandle {
            command_tx,
            event_rx,
            thread_handle,
        });

        Ok(())
    }

    /// Get the current VAD configuration
    #[allow(dead_code)]
    pub fn vad_config(&self) -> &VadAutoStopConfig {
        &self.vad_config
    }

    /// Start recording audio from the default input device.
    ///
    /// Prefer `start_with_device_name` when you need to honor a user-selected mic.
    ///
    /// # Arguments
    /// * `max_duration_secs` - Maximum recording duration in seconds (for buffer sizing)
    #[allow(dead_code)]
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
        // Reuse the stream-only path and then enable recording.
        self.start_stream_only(input_device_name)?;

        if let Ok(mut buf) = self.buffer.lock() {
            buf.reset_for_recording(max_duration_secs);
        }

        self.recording_active.store(true, Ordering::Relaxed);
        log::info!("Audio capture started");
        Ok(())
    }

    /// Stop recording and return the captured audio as WAV bytes
    #[allow(dead_code)]
    pub fn stop_and_get_wav(&mut self) -> Result<Vec<u8>, AudioCaptureError> {
        self.stop_and_get_wav_with_noise_gate(0)
    }

    /// Stop recording and return the captured audio as WAV bytes, applying an optional noise gate.
    ///
    /// `noise_gate_strength` is 0..=100 where 0 disables the noise gate.
    #[allow(dead_code)]
    pub fn stop_and_get_wav_with_noise_gate(
        &mut self,
        noise_gate_strength: u8,
    ) -> Result<Vec<u8>, AudioCaptureError> {
        let (wav_bytes, _diag) = self.stop_and_get_wav_with_stats_with_noise_gate(noise_gate_strength)?;
        Ok(wav_bytes)
    }

    /// Stop recording and return the captured audio as WAV bytes along with level stats.
    #[allow(dead_code)]
    pub fn stop_and_get_wav_with_stats(
        &mut self,
    ) -> Result<(Vec<u8>, AudioLevelStats), AudioCaptureError> {
        self.stop_and_get_wav_with_stats_with_noise_gate(0)
    }

    /// Stop recording and return WAV bytes + level stats, optionally applying an experimental noise gate.
    ///
    /// Note: stats are computed on the *raw* (pre-gate) samples.
    #[allow(dead_code)]
    pub fn stop_and_get_wav_with_stats_with_noise_gate(
        &mut self,
        noise_gate_strength: u8,
    ) -> Result<(Vec<u8>, AudioLevelStats), AudioCaptureError> {
        self.stop_recording();

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
        self.stop_recording();

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
        self.stop_recording();

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
        self.recording_active.store(false, Ordering::Relaxed);
        if let Some(handle) = self.capture_handle.take() {
            log::info!("Stopping audio capture");

            // Send stop command (ignore error if thread already stopped).
            let _ = handle.command_tx.send(CaptureCommand::Stop);

            // Join can block forever if the capture thread is stuck (e.g. callback stalls).
            // We use a helper joiner thread + recv_timeout so stop() itself is bounded.
            let (join_tx, join_rx) = mpsc::channel();
            thread::spawn(move || {
                let res = handle.thread_handle.join();
                let _ = join_tx.send(res);
            });

            const STOP_JOIN_TIMEOUT_MS: u64 = 2500;
            match join_rx.recv_timeout(std::time::Duration::from_millis(STOP_JOIN_TIMEOUT_MS)) {
                Ok(Ok(Ok(()))) => {}
                Ok(Ok(Err(e))) => {
                    log::warn!("Audio capture thread stopped with error: {}", e);
                }
                Ok(Err(_panic)) => {
                    log::warn!("Audio capture thread panicked while stopping");
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    log::warn!(
                        "AudioCapture::stop() timed out after {}ms; continuing without blocking",
                        STOP_JOIN_TIMEOUT_MS
                    );
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    log::warn!("AudioCapture::stop(): join waiter disconnected unexpectedly");
                }
            }
        }
    }

    /// Check if currently recording
    #[allow(dead_code)]
    pub fn is_recording(&self) -> bool {
        self.recording_active.load(Ordering::Relaxed)
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
    #[allow(dead_code)]
    pub fn duration_secs(&self) -> f32 {
        self.buffer
            .lock()
            .map(|b| b.duration_secs())
            .unwrap_or(0.0)
    }

    /// Get the sample rate
    #[allow(dead_code)]
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Get the number of channels
    #[allow(dead_code)]
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
    pre_roll: Arc<StdMutex<RollingBuffer>>,
    recording_active: Arc<AtomicBool>,
    pre_roll_ms: Arc<AtomicU32>,
    meter: Arc<AudioLevelMeter>,
    waveform_meter: Arc<AudioWaveformMeter>,
    command_rx: mpsc::Receiver<CaptureCommand>,
    event_tx: mpsc::Sender<AudioCaptureEvent>,
    vad_config: VadAutoStopConfig,
    auto_recover_enabled: Arc<AtomicBool>,
    desired_device_name: Arc<StdMutex<Option<String>>>,
) -> Result<(), AudioCaptureError> {
    use cpal::Sample;

    use std::time::{Duration, Instant};

    let mut device = device;
    let config = config;
    let sample_rate = config.sample_rate;

    // Used for watchdog timing (monotonic).
    let start = Instant::now();
    let last_callback_ms = Arc::new(AtomicU64::new(0));

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

    let build_stream = |device: &cpal::Device| -> Result<cpal::Stream, AudioCaptureError> {
        let buffer = buffer.clone();
        let pre_roll = pre_roll.clone();
        let recording_active = recording_active.clone();
        let pre_roll_ms = pre_roll_ms.clone();
        let meter = meter.clone();
        let waveform_meter = waveform_meter.clone();
        let last_callback_ms = last_callback_ms.clone();
        let vad_tx = if vad_config.enabled {
            Some(vad_samples_tx.clone())
        } else {
            None
        };
        let channels = config.channels as usize;
        let start_cb = start;

        match sample_format {
            SampleFormat::F32 => device
                .build_input_stream(
                    &config,
                    move |data: &[f32], _: &cpal::InputCallbackInfo| {
                        last_callback_ms.store(start_cb.elapsed().as_millis() as u64, Ordering::Relaxed);

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
                        waveform_meter.update_from_f32_interleaved(data, channels);

                        // Always maintain the rolling pre-roll buffer.
                        if let Ok(mut pr) = pre_roll.lock() {
                            pr.push(data);
                        }

                        // Record only when active.
                        if recording_active.load(Ordering::Relaxed) {
                            if let Ok(mut buf) = buffer.lock() {
                                buf.append(data);
                            }

                            if let Some(ref tx) = vad_tx {
                                let mono = if channels > 1 {
                                    downmix_interleaved_chunk_to_mono(data, channels)
                                } else {
                                    data.to_vec()
                                };
                                let _ = tx.send(mono);
                            }
                        }

                        // Keep capacity in sync if pre-roll ms changes drastically.
                        // (Avoids waiting for a config sync while stream is running.)
                        let _ = pre_roll_ms.load(Ordering::Relaxed);
                    },
                    err_fn,
                    None,
                ),
            SampleFormat::I16 => device
                .build_input_stream(
                    &config,
                    move |data: &[i16], _: &cpal::InputCallbackInfo| {
                        last_callback_ms.store(start_cb.elapsed().as_millis() as u64, Ordering::Relaxed);

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
                        waveform_meter.update_from_f32_interleaved(&samples, channels);

                        if let Ok(mut pr) = pre_roll.lock() {
                            pr.push(&samples);
                        }

                        if recording_active.load(Ordering::Relaxed) {
                            if let Ok(mut buf) = buffer.lock() {
                                buf.append(&samples);
                            }

                            if let Some(ref tx) = vad_tx {
                                let mono = if channels > 1 {
                                    downmix_interleaved_chunk_to_mono(&samples, channels)
                                } else {
                                    samples
                                };
                                let _ = tx.send(mono);
                            }
                        }

                        let _ = pre_roll_ms.load(Ordering::Relaxed);
                    },
                    err_fn,
                    None,
                ),
            SampleFormat::U16 => device
                .build_input_stream(
                    &config,
                    move |data: &[u16], _: &cpal::InputCallbackInfo| {
                        last_callback_ms.store(start_cb.elapsed().as_millis() as u64, Ordering::Relaxed);

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
                        waveform_meter.update_from_f32_interleaved(&samples, channels);

                        if let Ok(mut pr) = pre_roll.lock() {
                            pr.push(&samples);
                        }

                        if recording_active.load(Ordering::Relaxed) {
                            if let Ok(mut buf) = buffer.lock() {
                                buf.append(&samples);
                            }

                            if let Some(ref tx) = vad_tx {
                                let mono = if channels > 1 {
                                    downmix_interleaved_chunk_to_mono(&samples, channels)
                                } else {
                                    samples
                                };
                                let _ = tx.send(mono);
                            }
                        }

                        let _ = pre_roll_ms.load(Ordering::Relaxed);
                    },
                    err_fn,
                    None,
                ),
            _ => {
                return Err(AudioCaptureError::DeviceConfig(format!(
                    "Unsupported sample format: {:?}",
                    sample_format
                )));
            }
        }
        .map_err(|e| AudioCaptureError::StreamBuild(e.to_string()))
    };

    let mut stream = build_stream(&device)?;

    stream
        .play()
        .map_err(|e| AudioCaptureError::StreamStart(e.to_string()))?;

    // Wait for stop command, with optional watchdog restart.
    const WATCHDOG_CHECK_EVERY_MS: u64 = 100;
    const WATCHDOG_STALL_MS: u64 = 2000;
    let mut consecutive_restart_failures: u32 = 0;

    loop {
        match command_rx.recv_timeout(Duration::from_millis(WATCHDOG_CHECK_EVERY_MS)) {
            Ok(CaptureCommand::Stop) => break,
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // Watchdog: if we're idle and callbacks have stalled, attempt to restart.
                if !auto_recover_enabled.load(Ordering::Relaxed) {
                    continue;
                }
                if recording_active.load(Ordering::Relaxed) {
                    continue;
                }

                let now_ms = start.elapsed().as_millis() as u64;
                let last_ms = last_callback_ms.load(Ordering::Relaxed);
                if last_ms == 0 {
                    // Stream may not have delivered a callback yet.
                    continue;
                }

                if now_ms.saturating_sub(last_ms) < WATCHDOG_STALL_MS {
                    continue;
                }

                log::warn!(
                    "Audio capture watchdog: no callback for {}ms; attempting stream restart",
                    now_ms.saturating_sub(last_ms)
                );

                // Clear pre-roll to avoid carrying stale audio across a restart.
                if let Ok(mut pr) = pre_roll.lock() {
                    pr.clear();
                }

                // Try rebuilding the stream on the current device.
                match build_stream(&device).and_then(|s| {
                    s.play()
                        .map_err(|e| AudioCaptureError::StreamStart(e.to_string()))
                        .map(|_| s)
                }) {
                    Ok(new_stream) => {
                        let _previous_stream = std::mem::replace(&mut stream, new_stream);
                        consecutive_restart_failures = 0;
                        last_callback_ms.store(now_ms, Ordering::Relaxed);
                        continue;
                    }
                    Err(e) => {
                        consecutive_restart_failures = consecutive_restart_failures.saturating_add(1);
                        log::warn!("Audio capture watchdog: restart failed: {}", e);
                    }
                }

                // Optional: try rebinding to the desired/default device using the same config.
                let maybe_name = desired_device_name.lock().ok().and_then(|n| n.clone());
                let host = cpal::default_host();
                let rebound = select_input_device_from_host(&host, maybe_name.as_deref())
                    .or_else(|| host.default_input_device());
                if let Some(new_device) = rebound {
                    device = new_device;
                    if let Ok(mut buf) = buffer.lock() {
                        buf.set_format(config.sample_rate, config.channels);
                    }
                    // Resize pre-roll based on current config and configured ms.
                    let pre_ms = pre_roll_ms.load(Ordering::Relaxed).min(5000) as f32;
                    let cap_samples =
                        ((config.sample_rate as f32 * (pre_ms / 1000.0) * config.channels as f32) as usize)
                            .min(10_000_000);
                    if let Ok(mut pr) = pre_roll.lock() {
                        pr.set_capacity(cap_samples);
                    }

                    match build_stream(&device).and_then(|s| {
                        s.play()
                            .map_err(|e| AudioCaptureError::StreamStart(e.to_string()))
                            .map(|_| s)
                    }) {
                        Ok(new_stream) => {
                            let _previous_stream = std::mem::replace(&mut stream, new_stream);
                            consecutive_restart_failures = 0;
                            last_callback_ms.store(now_ms, Ordering::Relaxed);
                            continue;
                        }
                        Err(e) => {
                            consecutive_restart_failures = consecutive_restart_failures.saturating_add(1);
                            log::warn!("Audio capture watchdog: rebind restart failed: {}", e);
                        }
                    }
                }

                // Backoff to avoid tight restart loops.
                let backoff_ms = match consecutive_restart_failures {
                    0 | 1 => 200,
                    2 => 500,
                    3 => 1000,
                    _ => 2000,
                };
                std::thread::sleep(Duration::from_millis(backoff_ms));
            }
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
            // Defensive: CPAL device descriptions are not guaranteed unique on Windows.
            // The legacy API returns names only; dedupe to avoid downstream UI crashes
            // in case a caller uses names as unique keys.
            let mut out: Vec<String> = Vec::new();
            let mut seen: HashSet<String> = HashSet::new();

            for name in devices.filter_map(|d| d.description().ok().map(|desc| desc.to_string())) {
                if seen.insert(name.clone()) {
                    out.push(name);
                }
            }

            out
        })
        .unwrap_or_default()
}

/// Get the list of available input devices, with unique IDs suitable for UI option values.
///
/// The ID format is `mic:v1:<base64url(name)>:<ordinal>` where ordinal is the 0-based
/// occurrence index for that exact name in the CPAL enumeration order.
pub fn list_input_devices_v2() -> Vec<AudioInputDeviceInfo> {
    let host = cpal::default_host();

    let Ok(devices) = host.input_devices() else {
        return Vec::new();
    };

    let mut name_ordinals: HashMap<String, usize> = HashMap::new();
    let mut out: Vec<AudioInputDeviceInfo> = Vec::new();

    for d in devices {
        let Ok(desc) = d.description() else { continue };
        let name = desc.to_string();
        let ordinal = name_ordinals.get(&name).copied().unwrap_or(0);
        name_ordinals.insert(name.clone(), ordinal.saturating_add(1));

        out.push(AudioInputDeviceInfo {
            id: encode_mic_device_id(&name, ordinal),
            name,
        });
    }

    // Extra defensive: ensure uniqueness even if encoding logic changes.
    // (Should never trigger, but guarantees the UI can't crash.)
    let mut seen_ids: HashMap<String, usize> = HashMap::new();
    for device in &mut out {
        let n = seen_ids.get(&device.id).copied().unwrap_or(0);
        if n > 0 {
            device.id = format!("{}:dup{}", device.id, n);
        }
        seen_ids.insert(device.id.clone(), n.saturating_add(1));
    }

    out
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
