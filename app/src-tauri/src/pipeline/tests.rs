//! Pipeline integration tests.
//!
//! These tests exercise the recording pipeline without requiring real hardware (CPAL audio
//! devices) or network (STT/LLM API calls) by injecting fake/mock implementations.

use super::*;
use crate::audio_capture::{
    AudioCaptureBackend, AudioCaptureDiagnostics, AudioCaptureError, AudioCaptureEvent,
    AudioEncodeConfig, AudioLevelSnapshot, AudioLevelStats, SharedAudioLevelMeter,
    SharedAudioWaveformMeter,
};
use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

/// A fake audio capture backend that returns canned WAV data without using CPAL.
pub(super) struct FakeAudioCapture {
    level_meter: SharedAudioLevelMeter,
    waveform_meter: SharedAudioWaveformMeter,
    vad_enabled: bool,
    vad_auto_stop: bool,
    wav: Vec<u8>,
    before_wav: Vec<u8>,
    after_wav: Vec<u8>,
    _queued_events: std::collections::VecDeque<AudioCaptureEvent>,
}

impl FakeAudioCapture {
    pub fn new() -> Self {
        Self {
            level_meter: SharedAudioLevelMeter::new_for_tests(),
            waveform_meter: SharedAudioWaveformMeter::new_for_tests(),
            vad_enabled: false,
            vad_auto_stop: false,
            wav: vec![1, 2, 3],
            before_wav: vec![9],
            after_wav: vec![8],
            _queued_events: std::collections::VecDeque::new(),
        }
    }

    fn diagnostics() -> AudioCaptureDiagnostics {
        AudioCaptureDiagnostics {
            stats: AudioLevelStats {
                duration_secs: 0.5,
                rms: 0.1,
                peak: 0.2,
            },
            speech_detected: None,
        }
    }
}

impl AudioCaptureBackend for FakeAudioCapture {
    fn shared_level_meter(&self) -> SharedAudioLevelMeter {
        self.level_meter.clone()
    }

    fn shared_waveform_meter(&self) -> SharedAudioWaveformMeter {
        self.waveform_meter.clone()
    }

    fn level_snapshot(&self) -> AudioLevelSnapshot {
        self.level_meter.snapshot()
    }

    fn set_vad_config(&mut self, config: crate::audio_capture::VadAutoStopConfig) {
        self.vad_enabled = config.enabled;
        self.vad_auto_stop = config.auto_stop;
    }

    fn set_capture_behavior(
        &mut self,
        _hot_mic_enabled: bool,
        _hot_mic_pre_roll_ms: u32,
        _mic_auto_recover_enabled: bool,
        _input_device_name: Option<&str>,
    ) -> Result<(), AudioCaptureError> {
        Ok(())
    }

    fn start_recording_session(
        &mut self,
        _max_duration_secs: f32,
        _input_device_name: Option<&str>,
    ) -> Result<(), AudioCaptureError> {
        Ok(())
    }

    fn stop_and_get_wav_with_diagnostics(
        &mut self,
        _cfg: AudioEncodeConfig,
    ) -> Result<(Vec<u8>, AudioCaptureDiagnostics), AudioCaptureError> {
        Ok((self.wav.clone(), Self::diagnostics()))
    }

    fn stop_and_get_wav_before_after(
        &mut self,
        _after_cfg: AudioEncodeConfig,
    ) -> Result<(Vec<u8>, Vec<u8>, AudioCaptureDiagnostics), AudioCaptureError> {
        Ok((
            self.before_wav.clone(),
            self.after_wav.clone(),
            Self::diagnostics(),
        ))
    }

    fn stop_recording(&mut self) {}
    fn stop(&mut self) {}

    fn poll_vad_event(&self) -> Option<AudioCaptureEvent> {
        None
    }

    fn is_vad_auto_stop_enabled(&self) -> bool {
        self.vad_enabled && self.vad_auto_stop
    }
}

/// Configurable behavior for mock providers.
#[derive(Clone, Default)]
pub(super) struct MockBehavior {
    /// Artificial delay before returning a response.
    pub delay: Option<std::time::Duration>,
    /// If set, the mock will return this error instead of success.
    pub error: Option<String>,
}

/// A mock STT provider that returns a canned transcript without making network calls.
/// Supports configurable latency and error simulation for testing edge cases.
struct MockSttProvider {
    text: String,
    behavior: MockBehavior,
}

impl MockSttProvider {
    fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            behavior: MockBehavior::default(),
        }
    }

    #[allow(dead_code)]
    fn with_behavior(mut self, behavior: MockBehavior) -> Self {
        self.behavior = behavior;
        self
    }
}

#[async_trait]
impl SttProvider for MockSttProvider {
    async fn transcribe(
        &self,
        _audio: &[u8],
        _format: &AudioFormat,
    ) -> Result<String, crate::stt::SttError> {
        if let Some(delay) = self.behavior.delay {
            tokio::time::sleep(delay).await;
        }
        if let Some(ref err) = self.behavior.error {
            return Err(crate::stt::SttError::Api(err.clone()));
        }
        Ok(self.text.clone())
    }

    fn name(&self) -> &'static str {
        "mock"
    }
}

/// A mock LLM provider that returns a canned completion without making network calls.
/// Supports configurable latency and error simulation for testing edge cases.
struct MockLlmProvider {
    response: String,
    behavior: MockBehavior,
}

impl MockLlmProvider {
    fn new(response: impl Into<String>) -> Self {
        Self {
            response: response.into(),
            behavior: MockBehavior::default(),
        }
    }

    #[allow(dead_code)]
    fn with_behavior(mut self, behavior: MockBehavior) -> Self {
        self.behavior = behavior;
        self
    }
}

#[async_trait]
impl crate::llm::LlmProvider for MockLlmProvider {
    async fn complete(
        &self,
        _system_prompt: &str,
        _user_message: &str,
    ) -> Result<String, crate::llm::LlmError> {
        if let Some(delay) = self.behavior.delay {
            tokio::time::sleep(delay).await;
        }
        if let Some(ref err) = self.behavior.error {
            return Err(crate::llm::LlmError::Api(err.clone()));
        }
        Ok(self.response.clone())
    }

    fn name(&self) -> &'static str {
        "mock"
    }

    fn model(&self) -> &str {
        "mock-model"
    }
}

fn set_state_for_test(
    pipeline: &SharedPipeline,
    state: PipelineState,
    token: Option<CancellationToken>,
) {
    let mut inner = pipeline.inner.lock().expect("pipeline lock");
    inner.state = state;
    inner.cancel_token = token;
}

#[test]
fn test_shared_pipeline_creation() {
    let config = PipelineConfig {
        stt_api_key: "test-key".to_string(),
        ..Default::default()
    };
    let pipeline = SharedPipeline::new(config);
    assert_eq!(pipeline.state(), PipelineState::Idle);
    assert!(!pipeline.is_error());
}

#[test]
fn test_force_reset() {
    let config = PipelineConfig {
        stt_api_key: "test-key".to_string(),
        ..Default::default()
    };
    let pipeline = SharedPipeline::new(config);

    // Force reset should always work
    pipeline.force_reset();
    assert_eq!(pipeline.state(), PipelineState::Idle);
}

#[test]
fn test_cancel_from_recording_transitions_to_idle() {
    let pipeline = SharedPipeline::new(PipelineConfig::default());
    let token = CancellationToken::new();

    // Given a pipeline in Recording with an active cancel token
    set_state_for_test(&pipeline, PipelineState::Recording, Some(token.clone()));

    // When cancellation is requested
    pipeline.cancel();

    // Then the pipeline resets to Idle and the token is cancelled
    assert_eq!(pipeline.state(), PipelineState::Idle);
    assert!(token.is_cancelled());
    assert!(pipeline.get_cancel_token().is_none());
}

#[test]
fn test_cancel_from_transcribing_transitions_to_idle() {
    let pipeline = SharedPipeline::new(PipelineConfig::default());
    let token = CancellationToken::new();

    // Given a pipeline in Transcribing with an active cancel token
    set_state_for_test(&pipeline, PipelineState::Transcribing, Some(token.clone()));

    // When cancellation is requested
    pipeline.cancel();

    // Then the pipeline resets to Idle and the token is cancelled
    assert_eq!(pipeline.state(), PipelineState::Idle);
    assert!(token.is_cancelled());
}

#[test]
fn test_stop_recording_transitions_to_idle() {
    let pipeline = SharedPipeline::new(PipelineConfig::default());
    let token = CancellationToken::new();

    // Given a pipeline marked as Recording
    set_state_for_test(&pipeline, PipelineState::Recording, Some(token));

    // When stopping the recording
    let result = pipeline.stop_recording();

    // Then it resets to Idle and captures a WAV buffer
    assert!(result.is_ok());
    assert_eq!(pipeline.state(), PipelineState::Idle);
    assert!(pipeline.clone_last_wav_bytes().is_some());
    assert!(pipeline.get_cancel_token().is_none());
}

#[test]
fn pipeline_can_start_and_stop_without_cpal() {
    let mut config = PipelineConfig::default();
    config.max_recording_bytes = 1024;

    let fake = FakeAudioCapture::new();
    let meter_handle = fake.shared_level_meter();
    let waveform_handle = fake.shared_waveform_meter();

    let p = SharedPipeline::new_for_tests(config, Box::new(fake));

    assert_eq!(p.try_state(), Some(PipelineState::Idle));
    p.start_recording().expect("start recording should succeed");
    assert_eq!(p.try_state(), Some(PipelineState::Recording));

    // Simulate realtime meter updates without needing an actual CPAL callback.
    let s0 = p.audio_level_snapshot_fast();
    meter_handle.set_for_tests(0.25, 0.5);
    let s1 = p.audio_level_snapshot_fast();
    assert!(s1.seq > s0.seq);
    assert!((s1.rms - 0.25).abs() < 1e-6);
    assert!((s1.peak - 0.5).abs() < 1e-6);

    // Same idea for the waveform meter: simulate samples without needing CPAL.
    let w0 = p.audio_waveform_snapshot_fast();
    waveform_handle.set_from_samples_for_tests(&[0.0, -0.25, 0.5, 1.0], 1);
    let w1 = p.audio_waveform_snapshot_fast();
    assert!(w1.seq > w0.seq);

    let wav = p.stop_recording().expect("stop recording should succeed");
    assert_eq!(wav, vec![1, 2, 3]);
    assert_eq!(p.try_state(), Some(PipelineState::Idle));
}

#[tokio::test]
async fn pipeline_can_transcribe_without_network_or_hardware() {
    // Given: a pipeline with fake audio capture (no CPAL device) and a fake STT provider
    // injected into the provider cache (no network).
    let mut config = PipelineConfig::default();
    config.stt_provider = "mock".to_string();
    config.max_recording_bytes = 1024;
    config.quiet_audio_gate_enabled = false;

    let p = SharedPipeline::new_for_tests(config, Box::new(FakeAudioCapture::new()));
    p.inject_stt_provider_for_tests(
        "mock",
        None,
        Arc::new(MockSttProvider::new("hello from tests")),
    );

    p.start_recording().expect("start recording should succeed");

    // When: we stop and transcribe
    let result = p
        .stop_and_transcribe_detailed()
        .await
        .expect("stop/transcribe should succeed");

    // Then: we get a transcript without hitting any real IO.
    assert_eq!(result.stt_text, "hello from tests");
    assert_eq!(result.final_text, "hello from tests");
    assert_eq!(p.state(), PipelineState::Idle);
}

#[tokio::test]
async fn pipeline_can_transcribe_and_rewrite_without_network_or_hardware() {
    // Given: a pipeline with fake audio capture, fake STT, AND fake LLM providers.
    use crate::llm::{LlmConfig, PromptSections};

    let llm_config = LlmConfig {
        enabled: true,
        provider: "mock".to_string(),
        api_key: String::new(),
        model: None,
        ollama_url: None,
        openai_reasoning_effort: None,
        gemini_thinking_budget: None,
        gemini_thinking_level: None,
        anthropic_thinking_budget: None,
        prompts: PromptSections::default(),
        program_prompt_profiles: Vec::new(),
        timeout: std::time::Duration::from_secs(30),
    };

    let mut config = PipelineConfig::default();
    config.stt_provider = "mock".to_string();
    config.max_recording_bytes = 1024;
    config.quiet_audio_gate_enabled = false;
    config.llm_config = llm_config;
    // Insert fake API key so the pipeline doesn't bail early.
    config
        .llm_api_keys
        .insert("mock".to_string(), "test-key".to_string());

    let p = SharedPipeline::new_for_tests(config, Box::new(FakeAudioCapture::new()));
    p.inject_stt_provider_for_tests(
        "mock",
        None,
        Arc::new(MockSttProvider::new("hello from stt")),
    );
    p.inject_llm_provider_for_tests(
        "mock",
        None,
        Arc::new(MockLlmProvider::new("Hello from LLM")),
    );

    p.start_recording().expect("start recording should succeed");

    // When: we stop and transcribe (with rewrite enabled)
    let result = p
        .stop_and_transcribe_detailed()
        .await
        .expect("stop/transcribe should succeed");

    // Then: the final output should be the LLM-rewritten text, not the raw STT.
    assert_eq!(result.stt_text, "hello from stt");
    assert_eq!(result.final_text, "Hello from LLM");
    assert!(result.llm_attempted());
    assert_eq!(result.llm_outcome, crate::pipeline::LlmOutcome::Succeeded);
    assert_eq!(p.state(), PipelineState::Idle);
}

#[tokio::test]
async fn rewrite_disabled_falls_back_to_stt_text() {
    // Given: a pipeline with LLM rewrite explicitly disabled.
    use crate::llm::{LlmConfig, PromptSections};

    let llm_config = LlmConfig {
        enabled: false, // <-- rewrite disabled
        provider: "mock".to_string(),
        api_key: String::new(),
        model: None,
        ollama_url: None,
        openai_reasoning_effort: None,
        gemini_thinking_budget: None,
        gemini_thinking_level: None,
        anthropic_thinking_budget: None,
        prompts: PromptSections::default(),
        program_prompt_profiles: Vec::new(),
        timeout: std::time::Duration::from_secs(30),
    };

    let mut config = PipelineConfig::default();
    config.stt_provider = "mock".to_string();
    config.max_recording_bytes = 1024;
    config.quiet_audio_gate_enabled = false;
    config.llm_config = llm_config;

    let p = SharedPipeline::new_for_tests(config, Box::new(FakeAudioCapture::new()));
    p.inject_stt_provider_for_tests(
        "mock",
        None,
        Arc::new(MockSttProvider::new("raw stt transcript")),
    );
    // NOTE: we do NOT inject an LLM provider — rewrite should not be attempted.

    p.start_recording().expect("start recording should succeed");

    // When: we stop and transcribe
    let result = p
        .stop_and_transcribe_detailed()
        .await
        .expect("stop/transcribe should succeed");

    // Then: final_text should match stt_text (no rewrite).
    assert_eq!(result.stt_text, "raw stt transcript");
    assert_eq!(result.final_text, "raw stt transcript");
    assert!(!result.llm_attempted());
    assert!(matches!(
        result.llm_outcome,
        crate::pipeline::LlmOutcome::NotAttempted(_)
    ));
    assert_eq!(p.state(), PipelineState::Idle);
}

#[tokio::test]
async fn stt_error_propagates_correctly() {
    // Given: a pipeline with an STT provider that will fail.
    let mut config = PipelineConfig::default();
    config.stt_provider = "mock".to_string();
    config.max_recording_bytes = 1024;
    config.quiet_audio_gate_enabled = false;

    let p = SharedPipeline::new_for_tests(config, Box::new(FakeAudioCapture::new()));
    p.inject_stt_provider_for_tests(
        "mock",
        None,
        Arc::new(MockSttProvider::new("unused").with_behavior(MockBehavior {
            error: Some("Simulated API failure".to_string()),
            ..Default::default()
        })),
    );

    p.start_recording().expect("start recording should succeed");

    // When: we stop and transcribe
    let result = p.stop_and_transcribe_detailed().await;

    // Then: we get an error, not a success.
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        matches!(err, crate::pipeline::PipelineError::Stt(_)),
        "Expected STT error, got: {:?}",
        err
    );
    // Pipeline transitions to Error state after a failed transcription.
    assert_eq!(p.state(), PipelineState::Error);
}
