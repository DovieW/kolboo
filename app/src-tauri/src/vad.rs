//! Voice Activity Detection (VAD) module.
//!
//! This module provides voice activity detection to automatically detect
//! when speech starts and stops. It uses the webrtc-vad crate and includes
//! proper handling of pre-roll buffering and hangover periods.

use std::collections::VecDeque;
use webrtc_vad::{Vad, VadMode};

/// VAD aggressiveness level (maps to webrtc-vad modes)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VadAggressiveness {
    /// Quality mode - less aggressive, fewer false negatives
    Quality,
    /// Low bitrate mode
    LowBitrate,
    /// Aggressive mode
    #[default]
    Aggressive,
    /// Very aggressive mode - more aggressive, more false negatives
    VeryAggressive,
}

impl VadAggressiveness {
    fn to_vad_mode(self) -> VadMode {
        match self {
            VadAggressiveness::Quality => VadMode::Quality,
            VadAggressiveness::LowBitrate => VadMode::LowBitrate,
            VadAggressiveness::Aggressive => VadMode::Aggressive,
            VadAggressiveness::VeryAggressive => VadMode::VeryAggressive,
        }
    }
}

/// Configuration for the VAD
#[derive(Debug, Clone)]
pub struct VadConfig {
    /// VAD aggressiveness mode (higher = more aggressive filtering)
    pub aggressiveness: VadAggressiveness,
    /// Number of consecutive speech frames required to trigger speech start
    pub speech_frames_threshold: u32,
    /// Number of consecutive silence frames required to trigger speech end (hangover)
    pub hangover_frames: u32,
    /// Pre-roll duration in milliseconds (audio to keep before speech start)
    pub pre_roll_ms: u32,
    /// Frame duration in milliseconds (10, 20, or 30ms supported by webrtc-vad)
    pub frame_duration_ms: u32,
    /// Sample rate to use for VAD (must be 8000, 16000, 32000, or 48000)
    #[cfg_attr(not(test), allow(dead_code))]
    pub sample_rate: u32,
}

impl VadConfig {
    /// Normalize settings to match what the current implementation actually supports.
    ///
    /// Today we always run webrtc-vad at 16kHz, and we only support 10/20/30ms frames.
    pub fn normalized(mut self) -> Self {
        self.frame_duration_ms = match self.frame_duration_ms {
            10 | 20 | 30 => self.frame_duration_ms,
            // Defensive default: 10ms provides lower-latency speech boundaries.
            other => {
                log::warn!(
                    "VadConfig.frame_duration_ms={} is unsupported; falling back to 10ms",
                    other
                );
                10
            }
        };

        // The current pipeline always resamples to 16kHz before calling webrtc-vad.
        if self.sample_rate != 16000 {
            log::warn!(
                "VadConfig.sample_rate={} is not supported by current implementation; forcing 16000",
                self.sample_rate
            );
            self.sample_rate = 16000;
        }

        self
    }
}

impl Default for VadConfig {
    fn default() -> Self {
        Self {
            aggressiveness: VadAggressiveness::Aggressive,
            speech_frames_threshold: 3,
            hangover_frames: 30, // ~300ms at 10ms frames
            pre_roll_ms: 300,
            frame_duration_ms: 10,
            sample_rate: 16000,
        }
    }
}

/// Events emitted by the VAD
#[derive(Debug, Clone)]
pub enum VadEvent {
    /// No significant event
    None,
    /// Speech has started, includes pre-roll audio
    #[allow(dead_code)]
    SpeechStart {
        /// Pre-roll audio samples (before speech was detected)
        pre_roll: Vec<i16>,
    },
    /// Speech has ended
    SpeechEnd,
}

/// Voice Activity Detector with pre-roll buffering and hangover
pub struct VoiceActivityDetector {
    vad: Vad,
    config: VadConfig,
    /// Whether we're currently in a speech segment
    is_speaking: bool,
    /// Count of consecutive silence frames
    silence_frames: u32,
    /// Count of consecutive speech frames
    speech_frames: u32,
    /// Pre-roll ring buffer storing recent audio frames
    pre_roll_buffer: VecDeque<Vec<i16>>,
    /// Maximum number of frames to keep in pre-roll buffer
    pre_roll_max_frames: usize,
}

impl VoiceActivityDetector {
    /// Create a new VAD with the given configuration
    pub fn new(config: VadConfig) -> Self {
        let config = config.normalized();

        let mut vad = Vad::new();
        vad.set_mode(config.aggressiveness.to_vad_mode());
        vad.set_sample_rate(webrtc_vad::SampleRate::Rate16kHz);

        // Calculate pre-roll buffer size in frames
        let pre_roll_max_frames = (config.pre_roll_ms / config.frame_duration_ms) as usize;

        Self {
            vad,
            config,
            is_speaking: false,
            silence_frames: 0,
            speech_frames: 0,
            pre_roll_buffer: VecDeque::with_capacity(pre_roll_max_frames + 1),
            pre_roll_max_frames,
        }
    }

    /// Process a frame of audio samples and return any VAD events
    ///
    /// # Arguments
    /// * `samples` - PCM16 audio samples at 16kHz. Frame must be exactly
    ///   the size expected for the configured frame duration:
    ///   - 10ms: 160 samples
    ///   - 20ms: 320 samples
    ///   - 30ms: 480 samples
    ///
    /// # Returns
    /// A VAD event indicating speech start, speech end, or no event
    pub fn process_frame(&mut self, samples: &[i16]) -> VadEvent {
        // Always maintain pre-roll buffer (even during speech for potential restart)
        self.pre_roll_buffer.push_back(samples.to_vec());
        if self.pre_roll_buffer.len() > self.pre_roll_max_frames {
            self.pre_roll_buffer.pop_front();
        }

        // Run VAD on the frame
        let is_speech = self.vad.is_voice_segment(samples).unwrap_or(false);

        if is_speech {
            self.speech_frames += 1;
            self.silence_frames = 0;

            // Detect speech start after threshold frames of consecutive speech
            if !self.is_speaking && self.speech_frames >= self.config.speech_frames_threshold {
                self.is_speaking = true;

                // Collect pre-roll audio
                let pre_roll: Vec<i16> = self.pre_roll_buffer.iter().flatten().cloned().collect();

                log::debug!(
                    "VAD: Speech started (pre-roll: {} samples, {} frames)",
                    pre_roll.len(),
                    self.pre_roll_buffer.len()
                );

                return VadEvent::SpeechStart { pre_roll };
            }
        } else {
            self.silence_frames += 1;
            self.speech_frames = 0;

            // Detect speech end after hangover period
            if self.is_speaking && self.silence_frames >= self.config.hangover_frames {
                self.is_speaking = false;

                log::debug!(
                    "VAD: Speech ended (after {} silence frames)",
                    self.silence_frames
                );

                return VadEvent::SpeechEnd;
            }
        }

        VadEvent::None
    }

    /// Reset the VAD state (call when starting a new recording session)
    #[allow(dead_code)]
    pub fn reset(&mut self) {
        self.is_speaking = false;
        self.silence_frames = 0;
        self.speech_frames = 0;
        self.pre_roll_buffer.clear();
    }

    /// Check if currently detecting speech
    #[allow(dead_code)]
    pub fn is_speaking(&self) -> bool {
        self.is_speaking
    }

    /// Get the expected frame size in samples for the configured duration
    pub fn frame_size(&self) -> usize {
        // At 16kHz: 10ms = 160, 20ms = 320, 30ms = 480
        (16000 * self.config.frame_duration_ms / 1000) as usize
    }

    /// Get the VAD configuration
    #[allow(dead_code)]
    pub fn config(&self) -> &VadConfig {
        &self.config
    }
}

impl Default for VoiceActivityDetector {
    fn default() -> Self {
        Self::new(VadConfig::default())
    }
}
/// Resample audio from source sample rate to 16kHz for VAD processing
///
/// Uses the rubato library for high-quality resampling.
pub fn resample_to_16khz(samples: &[f32], source_sample_rate: u32) -> Vec<f32> {
    use audioadapter_buffers::direct::InterleavedSlice;
    use rubato::{
        Async, FixedAsync, Resampler, SincInterpolationParameters, SincInterpolationType,
        WindowFunction,
    };

    if source_sample_rate == 16000 {
        return samples.to_vec();
    }

    if samples.is_empty() {
        return Vec::new();
    }

    let params = SincInterpolationParameters {
        sinc_len: 256,
        f_cutoff: 0.95,
        interpolation: SincInterpolationType::Linear,
        oversampling_factor: 256,
        window: WindowFunction::BlackmanHarris2,
    };

    if source_sample_rate == 0 {
        log::warn!("resample_to_16khz: source_sample_rate=0; returning input unchanged");
        return samples.to_vec();
    }

    let resample_ratio = 16000.0 / source_sample_rate as f64;
    let input_len_frames = samples.len();
    let chunk_size_frames = input_len_frames.clamp(1, 1024);

    // rubato v1 uses AudioAdapter-based input/output buffers.
    // We only use mono data here.
    let mut resampler = match Async::<f32>::new_sinc(
        resample_ratio,
        2.0, // max relative ratio (allows some drift if reused for streams)
        &params,
        chunk_size_frames,
        1, // mono
        FixedAsync::Input,
    ) {
        Ok(r) => r,
        Err(e) => {
            log::error!("Failed to create resampler: {}", e);
            return samples.to_vec();
        }
    };

    let input_adapter = match InterleavedSlice::new(samples, 1, input_len_frames) {
        Ok(a) => a,
        Err(e) => {
            log::error!("Failed to create input adapter: {}", e);
            return samples.to_vec();
        }
    };

    let output_len_frames = resampler.process_all_needed_output_len(input_len_frames);
    let mut out = vec![0.0_f32; output_len_frames];
    let out_capacity_frames = out.len();
    let mut output_adapter = match InterleavedSlice::new_mut(&mut out, 1, out_capacity_frames) {
        Ok(a) => a,
        Err(e) => {
            log::error!("Failed to create output adapter: {}", e);
            return samples.to_vec();
        }
    };

    match resampler.process_all_into_buffer(
        &input_adapter,
        &mut output_adapter,
        input_len_frames,
        None,
    ) {
        Ok((_frames_read, frames_written)) => {
            out.truncate(frames_written);
            out
        }
        Err(e) => {
            log::error!("Resampling failed: {}", e);
            samples.to_vec()
        }
    }
}

/// Convert f32 samples to i16 for webrtc-vad
pub fn f32_to_i16(samples: &[f32]) -> Vec<i16> {
    samples
        .iter()
        .map(|&s| (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16)
        .collect()
}

/// Convert i16 samples back to f32
#[allow(dead_code)]
pub fn i16_to_f32(samples: &[i16]) -> Vec<f32> {
    samples
        .iter()
        .map(|&s| s as f32 / i16::MAX as f32)
        .collect()
}

/// Audio frame processor that handles resampling and frame splitting for VAD
pub struct VadFrameProcessor {
    vad: VoiceActivityDetector,
    /// Source sample rate
    source_sample_rate: u32,
    /// Buffer for accumulating samples until we have a full frame
    frame_buffer: Vec<f32>,
    /// Resampled buffer
    resampled_buffer: Vec<f32>,
}

#[allow(dead_code)]
impl VadFrameProcessor {
    /// Create a new frame processor
    pub fn new(config: VadConfig, source_sample_rate: u32) -> Self {
        Self {
            vad: VoiceActivityDetector::new(config),
            source_sample_rate,
            frame_buffer: Vec::new(),
            resampled_buffer: Vec::new(),
        }
    }

    /// Process incoming audio samples and emit VAD events
    ///
    /// This handles:
    /// - Accumulating samples into frames
    /// - Resampling to 16kHz if needed
    /// - Splitting into the correct frame size for webrtc-vad
    ///
    /// # Returns
    /// A vector of VAD events (may be empty, one, or multiple)
    pub fn process(&mut self, samples: &[f32]) -> Vec<VadEvent> {
        let mut events = Vec::new();

        // Accumulate samples
        self.frame_buffer.extend_from_slice(samples);

        // Calculate how many source samples we need for one VAD frame
        // VAD frame at 16kHz = frame_size samples
        // At source rate, we need: frame_size * (source_rate / 16000) samples
        let frame_size = self.vad.frame_size();
        let source_frame_size =
            (frame_size as f64 * self.source_sample_rate as f64 / 16000.0).ceil() as usize;

        // Process complete frames
        while self.frame_buffer.len() >= source_frame_size {
            // Take one frame worth of samples
            let frame: Vec<f32> = self.frame_buffer.drain(..source_frame_size).collect();

            // Resample to 16kHz
            let resampled = resample_to_16khz(&frame, self.source_sample_rate);

            // Accumulate resampled samples
            self.resampled_buffer.extend(resampled);

            // Process complete VAD frames
            while self.resampled_buffer.len() >= frame_size {
                let vad_frame: Vec<f32> = self.resampled_buffer.drain(..frame_size).collect();
                let vad_frame_i16 = f32_to_i16(&vad_frame);

                let event = self.vad.process_frame(&vad_frame_i16);
                if !matches!(event, VadEvent::None) {
                    events.push(event);
                }
            }
        }

        events
    }

    /// Reset the processor state
    #[allow(dead_code)]
    pub fn reset(&mut self) {
        self.vad.reset();
        self.frame_buffer.clear();
        self.resampled_buffer.clear();
    }

    /// Check if currently detecting speech
    #[allow(dead_code)]
    pub fn is_speaking(&self) -> bool {
        self.vad.is_speaking()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vad_config_default() {
        let config = VadConfig::default();
        assert_eq!(config.frame_duration_ms, 10);
        assert_eq!(config.hangover_frames, 30);
        assert_eq!(config.pre_roll_ms, 300);
    }

    #[test]
    fn test_vad_creation() {
        let vad = VoiceActivityDetector::new(VadConfig::default());
        assert!(!vad.is_speaking());
        assert_eq!(vad.frame_size(), 160); // 10ms at 16kHz
    }

    #[test]
    fn test_vad_frame_size() {
        let config = VadConfig {
            frame_duration_ms: 20,
            ..Default::default()
        };
        let vad = VoiceActivityDetector::new(config);
        assert_eq!(vad.frame_size(), 320); // 20ms at 16kHz
    }

    #[test]
    fn test_vad_reset() {
        let mut vad = VoiceActivityDetector::new(VadConfig::default());
        // Process some frames to change state
        let silence = vec![0i16; 160];
        for _ in 0..10 {
            vad.process_frame(&silence);
        }
        vad.reset();
        assert!(!vad.is_speaking());
    }

    #[test]
    fn test_f32_to_i16_conversion() {
        let samples = vec![0.0, 0.5, -0.5, 1.0, -1.0];
        let converted = f32_to_i16(&samples);
        assert_eq!(converted[0], 0);
        assert!(converted[1] > 0);
        assert!(converted[2] < 0);
        assert_eq!(converted[3], i16::MAX);
        assert_eq!(converted[4], -i16::MAX);
    }

    #[test]
    fn test_i16_to_f32_conversion() {
        let samples = vec![0, i16::MAX / 2, -i16::MAX / 2];
        let converted = i16_to_f32(&samples);
        assert!((converted[0] - 0.0).abs() < 0.001);
        assert!((converted[1] - 0.5).abs() < 0.01);
        assert!((converted[2] - (-0.5)).abs() < 0.01);
    }

    #[test]
    fn test_frame_processor_creation() {
        let processor = VadFrameProcessor::new(VadConfig::default(), 44100);
        assert!(!processor.is_speaking());
    }
}
