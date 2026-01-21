use crate::audio_capture::{AudioEncodeConfig, VadAutoStopConfig};
use crate::llm::LlmConfig;
use crate::request_log::RequestLogStore;
use crate::settings::ProxySettings;
use crate::stt::RetryConfig;
use std::collections::HashMap;
use std::time::Duration;

/// Default timeout for STT transcription requests
const DEFAULT_TRANSCRIPTION_TIMEOUT: Duration = Duration::from_secs(10);

/// Maximum WAV file size in bytes (50MB) to prevent memory issues
const MAX_WAV_SIZE_BYTES: usize = 50 * 1024 * 1024;

/// Default values for the quiet-audio gate.
///
/// Thresholds are in dBFS (decibels relative to full scale, where 0 dBFS is max amplitude).
const DEFAULT_QUIET_AUDIO_MIN_DURATION_SECS: f32 = 0.15;
const DEFAULT_QUIET_AUDIO_RMS_DBFS_THRESHOLD: f32 = -60.0;
const DEFAULT_QUIET_AUDIO_PEAK_DBFS_THRESHOLD: f32 = -50.0;

/// Configuration for the recording pipeline
#[derive(Debug, Clone)]
pub struct PipelineConfig {
    /// Optional backend input device name (CPAL device name).
    ///
    /// When set, recording will attempt to use the first input device whose name
    /// matches exactly, falling back to the system default if not found.
    pub input_device_name: Option<String>,
    /// Maximum recording duration in seconds
    pub max_duration_secs: f32,
    /// STT provider to use
    pub stt_provider: String,
    /// API key for the STT provider
    #[allow(dead_code)]
    pub stt_api_key: String,
    /// API keys for all configured STT providers (provider id -> key)
    pub stt_api_keys: HashMap<String, String>,
    /// Optional model override for STT
    pub stt_model: Option<String>,

    /// Optional global transcription prompt.
    ///
    /// Applied by STT providers that support prompting (currently OpenAI transcription endpoint models).
    pub stt_transcription_prompt: Option<String>,

    /// Base URL for an OpenAI-compatible Whisper transcription server.
    ///
    /// Example: http://localhost:8000/v1
    pub whisper_server_base_url: Option<String>,
    /// Retry configuration for STT requests
    pub retry_config: RetryConfig,
    /// VAD auto-stop configuration
    pub vad_config: VadAutoStopConfig,
    /// Timeout for transcription requests
    pub transcription_timeout: Duration,
    /// Maximum recording size in bytes (0 = no limit beyond default)
    pub max_recording_bytes: usize,

    /// Outgoing HTTP proxy settings.
    pub proxy_settings: ProxySettings,

    /// Enable a quiet-audio gate to avoid silent-audio hallucinations.
    pub quiet_audio_gate_enabled: bool,
    /// Treat recordings shorter than this as effectively quiet.
    pub quiet_audio_min_duration_secs: f32,
    /// RMS threshold (in dBFS) below which the audio is considered quiet.
    pub quiet_audio_rms_dbfs_threshold: f32,
    /// Peak threshold (in dBFS) below which the audio is considered quiet.
    pub quiet_audio_peak_dbfs_threshold: f32,

    /// Optional noise gate threshold (dBFS), applied at stop-time before WAV encoding.
    ///
    /// Recommended range: -75..-30. `None` disables the noise gate.
    pub noise_gate_threshold_dbfs: Option<f32>,

    // ------------------------------------------------------------------------
    // Voice pickup (preprocessing) options
    // ------------------------------------------------------------------------
    /// Convert captured audio to mono before WAV encoding.
    pub audio_downmix_to_mono: bool,
    /// Resample to 16kHz before WAV encoding.
    pub audio_resample_to_16khz: bool,
    /// Apply a lightweight high-pass (DC/rumble) filter.
    pub audio_highpass_enabled: bool,
    /// Apply a lightweight auto-gain/normalization.
    pub audio_agc_enabled: bool,
    /// Apply a lightweight noise suppression.
    pub audio_noise_suppression_enabled: bool,

    // ------------------------------------------------------------------------
    // Extra hallucination protection
    // ------------------------------------------------------------------------
    /// If enabled, run an offline VAD scan at stop-time and skip STT when no speech is detected.
    pub quiet_audio_require_speech: bool,

    // ------------------------------------------------------------------------
    // Capture behavior (Hot Mic + recovery)
    // ------------------------------------------------------------------------
    /// When enabled, keep the input stream open while idle and maintain a rolling pre-roll
    /// buffer to prepend when recording starts.
    pub hot_mic_enabled: bool,
    /// How much audio to keep before record start (milliseconds).
    pub hot_mic_pre_roll_ms: u32,
    /// When enabled, watchdog the mic stream and attempt auto-recovery (restart/rebind)
    /// on hangs/disconnects.
    pub mic_auto_recover_enabled: bool,
    /// LLM formatting configuration
    pub llm_config: LlmConfig,
    /// API keys for all configured LLM providers (provider id -> key)
    pub llm_api_keys: HashMap<String, String>,

    /// Optional request log store for capturing provider request/response payloads.
    pub request_log_store: Option<RequestLogStore>,
    /// Path to local Whisper model (for local-whisper feature)
    #[cfg(feature = "local-whisper")]
    pub whisper_model_path: Option<std::path::PathBuf>,

    /// When to load the local whisper.cpp model file.
    ///
    /// Values:
    /// - "manual": require explicit UI load
    /// - "on_transcribe": load when first needed
    /// - "on_launch": best-effort preload at startup
    pub local_whisper_load_mode: String,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            input_device_name: None,
            max_duration_secs: 300.0, // 5 minutes max
            stt_provider: "groq".to_string(),
            stt_api_key: String::new(),
            stt_api_keys: HashMap::new(),
            stt_model: None,
            stt_transcription_prompt: None,
            whisper_server_base_url: None,
            retry_config: RetryConfig::default(),
            vad_config: VadAutoStopConfig::default(),
            transcription_timeout: DEFAULT_TRANSCRIPTION_TIMEOUT,
            max_recording_bytes: MAX_WAV_SIZE_BYTES,

            proxy_settings: ProxySettings::default(),

            quiet_audio_gate_enabled: true,
            quiet_audio_min_duration_secs: DEFAULT_QUIET_AUDIO_MIN_DURATION_SECS,
            quiet_audio_rms_dbfs_threshold: DEFAULT_QUIET_AUDIO_RMS_DBFS_THRESHOLD,
            quiet_audio_peak_dbfs_threshold: DEFAULT_QUIET_AUDIO_PEAK_DBFS_THRESHOLD,

            noise_gate_threshold_dbfs: None,

            audio_downmix_to_mono: true,
            audio_resample_to_16khz: false,
            audio_highpass_enabled: true,
            audio_agc_enabled: false,
            audio_noise_suppression_enabled: false,

            quiet_audio_require_speech: false,

            hot_mic_enabled: false,
            hot_mic_pre_roll_ms: 1500,
            mic_auto_recover_enabled: false,

            llm_config: LlmConfig::default(),
            llm_api_keys: HashMap::new(),
            request_log_store: None,
            #[cfg(feature = "local-whisper")]
            whisper_model_path: None,

            local_whisper_load_mode: "manual".to_string(),
        }
    }
}

impl PipelineConfig {
    /// Build the audio encoding config from the current pipeline settings.
    ///
    /// This is used when stopping recording to apply noise gate, resampling,
    /// and other preprocessing options.
    pub fn audio_encode_config(&self) -> AudioEncodeConfig {
        AudioEncodeConfig {
            noise_gate_threshold_dbfs: self.noise_gate_threshold_dbfs,
            downmix_to_mono: self.audio_downmix_to_mono,
            resample_to_16khz: self.audio_resample_to_16khz,
            highpass_enabled: self.audio_highpass_enabled,
            agc_enabled: self.audio_agc_enabled,
            noise_suppression_enabled: self.audio_noise_suppression_enabled,
            detect_speech_presence: self.quiet_audio_require_speech,
        }
    }
}

pub(crate) fn canonicalize_stt_provider_id(id: &str) -> String {
    match id {
        // Historical UI value.
        // If the current build does not include local-whisper support, fall back
        // to a default cloud provider to avoid a confusing "requires an API key"
        // error for an unavailable feature.
        "whisper" | "local-whisper" if !cfg!(feature = "local-whisper") => "groq".to_string(),
        "whisper" => "local-whisper".to_string(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pipeline_config_default() {
        let config = PipelineConfig::default();
        assert_eq!(config.max_duration_secs, 300.0);
        assert_eq!(config.stt_provider, "groq");
        assert_eq!(config.transcription_timeout, DEFAULT_TRANSCRIPTION_TIMEOUT);
        assert_eq!(config.max_recording_bytes, MAX_WAV_SIZE_BYTES);
    }

    #[test]
    fn test_canonicalize_stt_provider_id_whisper_alias() {
        let result = canonicalize_stt_provider_id("whisper");
        if cfg!(feature = "local-whisper") {
            assert_eq!(result, "local-whisper");
        } else {
            assert_eq!(result, "groq");
        }
    }
}
