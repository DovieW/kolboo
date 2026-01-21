//! Recording pipeline module that orchestrates audio capture → STT → LLM formatting → typing.
//!
//! This module provides the core pipeline for voice dictation, managing the
//! flow from audio recording through transcription to text output.
//!
//! ## Pipeline Hardening (Phase 5)
//! - Cancellation tokens for aborting in-flight tasks
//! - Timeouts on STT requests
//! - Bounded buffer sizes
//! - Proper error recovery (failures don't wedge the pipeline)
//! - Explicit state machine with guards
//!
//! ## LLM Formatting (Phase 6)
//! - Optional LLM-based text formatting after STT
//! - Multiple provider support (OpenAI, Anthropic, Ollama)
//! - Configurable prompts for dictation cleanup

use crate::audio_capture::{
    AudioCapture, AudioCaptureBackend, AudioCaptureDiagnostics, AudioCaptureEvent,
    AudioLevelSnapshot,
};
use crate::llm::LlmProvider;
use crate::stt::{SttProvider, SttRegistry};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::AppHandle;
use tokio_util::sync::CancellationToken;

mod config;
mod llm_provider;
mod program_profiles;
mod routing;
mod state_machine;
mod stt_flow;
mod stt_provider;
#[cfg(test)]
mod tests;
mod transcription_flow;
mod types;
mod utils;

use config::canonicalize_stt_provider_id;
pub use config::PipelineConfig;

pub use state_machine::PipelineState;
pub use types::{LlmNotAttemptedReason, LlmOutcome, PipelineError, TranscriptionResult};

pub(crate) use program_profiles::select_profile_for_foreground_app;
use program_profiles::{select_default_profile, select_effective_preset};

use llm_provider::{create_llm_provider, LlmProviderParams};
use stt_flow::run_stt_transcription;
use utils::{amp_to_dbfs, is_effectively_quiet, seconds_to_duration_or};

// Intent routing helpers live in `pipeline/routing.rs`.

/// Internal state for the recording pipeline
struct PipelineInner {
    audio_capture: Box<dyn AudioCaptureBackend>,
    stt_registry: SttRegistry,
    stt_provider_cache: HashMap<String, Arc<dyn SttProvider>>,
    llm_provider_cache: HashMap<String, Arc<dyn LlmProvider>>,
    /// Injected embeddings provider for testing (bypasses real API calls).
    injected_embeddings_provider: Option<Arc<dyn crate::embeddings::EmbeddingsProvider>>,
    state: PipelineState,
    config: PipelineConfig,
    /// Cancellation token for the current operation
    cancel_token: Option<CancellationToken>,

    /// Last captured audio (WAV bytes). Used for debugging/testing.
    last_wav_bytes: Option<Vec<u8>>,

    /// Last recording diagnostics (raw stats + optional speech detection).
    last_recording_diagnostics: Option<AudioCaptureDiagnostics>,
}

impl PipelineInner {
    fn transition_to(&mut self, next: PipelineState, context: &str) {
        if self.state.can_transition_to(next) {
            self.state = next;
            return;
        }

        self.set_error(&format!(
            "Invalid pipeline state transition {:?} -> {:?} ({})",
            self.state, next, context
        ));
    }
    fn local_whisper_model_key_for_cache(&self) -> String {
        #[cfg(feature = "local-whisper")]
        {
            self.config
                .whisper_model_path
                .as_ref()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| "<missing-model-path>".to_string())
        }

        #[cfg(not(feature = "local-whisper"))]
        {
            "<local-whisper-disabled>".to_string()
        }
    }

    fn local_whisper_cache_key(&self) -> String {
        format!(
            "local-whisper::{}",
            self.local_whisper_model_key_for_cache()
        )
    }

    fn is_local_whisper_loaded(&self) -> bool {
        let key = self.local_whisper_cache_key();
        self.stt_provider_cache.contains_key(&key)
    }

    fn unload_local_whisper(&mut self) {
        self.stt_provider_cache
            .retain(|k, _| !k.starts_with("local-whisper::"));
    }

    fn force_load_local_whisper(&mut self) -> Result<(), PipelineError> {
        let cache_key = self.local_whisper_cache_key();

        if self.stt_provider_cache.contains_key(&cache_key) {
            return Ok(());
        }

        #[cfg(feature = "local-whisper")]
        {
            let Some(model_path) = &self.config.whisper_model_path else {
                return Err(PipelineError::Config(
                    "Local Whisper: no model path configured".to_string(),
                ));
            };

            let provider =
                crate::stt::LocalWhisperProvider::with_config(crate::stt::LocalWhisperConfig {
                    model_path: model_path.clone(),
                    transcription_prompt: self.config.stt_transcription_prompt.clone(),
                    ..Default::default()
                })
                .map_err(|e| PipelineError::Config(format!("Local Whisper init failed: {}", e)))?;
            let provider = Arc::new(provider);
            self.stt_provider_cache.insert(cache_key, provider);
            return Ok(());
        }

        #[cfg(not(feature = "local-whisper"))]
        {
            Err(PipelineError::Config(
                "Local Whisper feature is not enabled".to_string(),
            ))
        }
    }

    fn build_http_client_with_timeout(
        &self,
        timeout: Duration,
    ) -> Result<reqwest::Client, PipelineError> {
        crate::network::build_http_client_with_timeout(&self.config.proxy_settings, timeout)
            .map_err(|e| PipelineError::Config(format!("Failed to create HTTP client: {}", e)))
    }

    fn new_with_audio_capture(
        config: PipelineConfig,
        audio_capture: Box<dyn AudioCaptureBackend>,
    ) -> Self {
        let mut inner = Self {
            audio_capture,
            stt_registry: SttRegistry::new(),
            stt_provider_cache: HashMap::new(),
            llm_provider_cache: HashMap::new(),
            injected_embeddings_provider: None,
            state: PipelineState::Idle,
            config: config.clone(),
            cancel_token: None,
            last_wav_bytes: None,
            last_recording_diagnostics: None,
        };
        inner.initialize_providers(&config);
        inner
    }

    fn new(config: PipelineConfig) -> Self {
        Self::new_with_audio_capture(
            config.clone(),
            Box::new(AudioCapture::with_vad_config(config.vad_config.clone())),
        )
    }

    fn get_or_create_stt_provider(
        &mut self,
        provider_id: &str,
        model: Option<String>,
    ) -> Result<Arc<dyn SttProvider>, PipelineError> {
        let provider_id = canonicalize_stt_provider_id(provider_id);

        // NOTE: for Local Whisper, the "model" setting is not meaningful (Whisper model is
        // selected via `whisper_model_path`). Using the global `stt_model` here can cause
        // unnecessary cache misses and, worse, repeated expensive model loads.
        let model_key = if provider_id == "local-whisper" {
            self.local_whisper_model_key_for_cache()
        } else {
            model.clone().unwrap_or_else(|| "<default>".to_string())
        };

        let cache_key = format!("{}::{}", provider_id, model_key);

        if let Some(p) = self.stt_provider_cache.get(&cache_key) {
            return Ok(p.clone());
        }

        // Manual local-whisper mode: require explicit preload to avoid surprise UI stalls
        // during stop/transcribe.
        if provider_id == "local-whisper" && self.config.local_whisper_load_mode == "manual" {
            return Err(PipelineError::Config(
                "Local Whisper is set to Manual load. Click 'Load model' in Settings (or switch load mode to 'On transcribe').".to_string(),
            ));
        }

        #[cfg(feature = "local-whisper")]
        if provider_id == "local-whisper" {
            if let Some(model_path) = &self.config.whisper_model_path {
                let provider =
                    crate::stt::LocalWhisperProvider::with_config(crate::stt::LocalWhisperConfig {
                        model_path: model_path.clone(),
                        transcription_prompt: self.config.stt_transcription_prompt.clone(),
                        ..Default::default()
                    })
                    .map_err(|e| {
                        PipelineError::Config(format!("Local Whisper init failed: {}", e))
                    })?;
                let provider = Arc::new(provider);
                self.stt_provider_cache.insert(cache_key, provider.clone());
                return Ok(provider);
            }

            return Err(PipelineError::Config(
                "Local Whisper selected but no model path configured".to_string(),
            ));
        }

        if provider_id == "whisper-server" {
            let base_url = self
                .config
                .whisper_server_base_url
                .clone()
                .unwrap_or_default();

            let provider = crate::stt::WhisperServerSttProvider::with_client(
                self.build_http_client_with_timeout(self.config.transcription_timeout)?,
                base_url,
                model,
                self.config.stt_transcription_prompt.clone(),
            )
            .map_err(|e| PipelineError::Config(format!("Whisper server init failed: {}", e)))?
            .with_request_log_store(self.config.request_log_store.clone());

            let provider = Arc::new(provider);
            self.stt_provider_cache.insert(cache_key, provider.clone());
            return Ok(provider);
        }

        // Cloud providers use the common factory
        let api_key = self
            .config
            .stt_api_keys
            .get(&provider_id)
            .cloned()
            .unwrap_or_default();

        let timeout = stt_provider::default_timeout_for_provider(&provider_id);
        let client = stt_provider::build_stt_client(&self.config.proxy_settings, timeout)?;

        let provider = stt_provider::create_cloud_stt_provider(
            client,
            stt_provider::SttProviderParams {
                provider_id,
                model,
                api_key,
                transcription_prompt: self.config.stt_transcription_prompt.clone(),
                request_log_store: self.config.request_log_store.clone(),
            },
        )?;

        self.stt_provider_cache.insert(cache_key, provider.clone());
        Ok(provider)
    }

    fn get_or_create_llm_provider(
        &mut self,
        provider_id: &str,
        params: LlmProviderParams,
    ) -> Result<Arc<dyn LlmProvider>, PipelineError> {
        let model_key = params
            .model
            .clone()
            .unwrap_or_else(|| "<default>".to_string());
        let url_key = params
            .ollama_url
            .clone()
            .unwrap_or_else(|| "<default-url>".to_string());
        let openai_effort_key = params
            .openai_reasoning_effort
            .clone()
            .unwrap_or_else(|| "<default-effort>".to_string());
        let gemini_budget_key = params
            .gemini_thinking_budget
            .map(|b| b.to_string())
            .unwrap_or_else(|| "<default-budget>".to_string());
        let gemini_level_key = params
            .gemini_thinking_level
            .clone()
            .unwrap_or_else(|| "<default-level>".to_string());
        let anthropic_budget_key = params
            .anthropic_thinking_budget
            .map(|b| b.to_string())
            .unwrap_or_else(|| "<default-budget>".to_string());
        let cache_key = format!(
            "{}::{}::{}::{}::{}::{}::{}::{}",
            provider_id,
            model_key,
            params.timeout.as_secs_f64(),
            url_key,
            openai_effort_key,
            gemini_budget_key,
            gemini_level_key,
            anthropic_budget_key
        );

        if let Some(p) = self.llm_provider_cache.get(&cache_key) {
            return Ok(p.clone());
        }

        let api_key = if provider_id == "ollama" {
            String::new()
        } else {
            self.config
                .llm_api_keys
                .get(provider_id)
                .cloned()
                .unwrap_or_default()
        };

        if provider_id != "ollama" && api_key.is_empty() {
            return Err(PipelineError::Config(format!(
                "LLM provider '{}' requires an API key",
                provider_id
            )));
        }

        // Preserve global LLM config (including provider-specific knobs) but override the
        // effective provider/model/timeout for this transcription.
        let mut cfg = self.config.llm_config.clone();
        cfg.enabled = true;
        cfg.provider = provider_id.to_string();
        cfg.api_key = api_key;
        cfg.model = params.model;
        cfg.ollama_url = params.ollama_url;
        cfg.timeout = params.timeout;
        cfg.openai_reasoning_effort = params.openai_reasoning_effort;
        cfg.gemini_thinking_budget = params.gemini_thinking_budget;
        cfg.gemini_thinking_level = params.gemini_thinking_level;
        cfg.anthropic_thinking_budget = params.anthropic_thinking_budget;

        let provider = create_llm_provider(
            &cfg,
            self.config.request_log_store.clone(),
            &self.config.proxy_settings,
        )?;
        self.llm_provider_cache.insert(cache_key, provider.clone());
        Ok(provider)
    }

    fn initialize_providers(&mut self, config: &PipelineConfig) {
        // Clear caches on config updates.
        // IMPORTANT: keep any cached local-whisper models unless we explicitly evicted them
        // (e.g. model path / transcription prompt changed). This prevents expensive model
        // reloads during routine config sync and makes "on_launch" preload actually stick.
        self.stt_provider_cache
            .retain(|k, _| k.starts_with("local-whisper::"));
        self.llm_provider_cache.clear();

        // Initialize STT providers
        self.stt_registry = SttRegistry::new();
        let canonical = canonicalize_stt_provider_id(&config.stt_provider);

        // Avoid blocking the pipeline lock during config sync.
        // Local Whisper model initialization can take noticeable time and should be done
        // lazily (when we actually need to transcribe).
        #[cfg(feature = "local-whisper")]
        if canonical == "local-whisper" {
            self.stt_registry.set_current_name_for_ui(&canonical);
            return;
        }

        match self.get_or_create_stt_provider(&canonical, config.stt_model.clone()) {
            Ok(provider) => {
                self.stt_registry.register(&canonical, provider);
                let _ = self.stt_registry.set_current(&canonical);
            }
            Err(e) => {
                // Keep the name for UI/telemetry even if provider init fails.
                self.stt_registry.set_current_name_for_ui(&canonical);
                log::warn!(
                    "Pipeline: Default STT provider '{}' not initialized: {}",
                    canonical,
                    e
                );
            }
        }

        // Note: LLM providers are created on-demand per transcription based on the active profile.
    }

    /// Reset to idle state, clearing any error condition
    fn reset_to_idle(&mut self) {
        self.transition_to(PipelineState::Idle, "reset_to_idle");
        self.cancel_token = None;
    }

    /// Transition to error state
    fn set_error(&mut self, msg: &str) {
        log::error!("Pipeline error: {}", msg);
        self.state = PipelineState::Error;
        self.cancel_token = None;
    }
}

// Re-export types from transcription_flow module (shared definitions)
use transcription_flow::{complete_transcription_flow, SessionPresetLock, TranscriptionContext};

/// Callbacks adapter for integrating transcription_flow with SharedPipeline.
///
/// This implements `TranscriptionCallbacks` by acquiring locks on the pipeline's
/// inner state as needed for state transitions and provider creation.
struct PipelineCallbacks {
    inner: Arc<Mutex<PipelineInner>>,
}

impl transcription_flow::TranscriptionCallbacks for PipelineCallbacks {
    fn transition_to_routing(&self) {
        if let Ok(mut inner) = self.inner.lock() {
            if inner.state == PipelineState::Transcribing {
                inner.transition_to(PipelineState::Routing, "transcription_flow (routing)");
            }
        }
    }

    fn transition_from_routing(&self) {
        if let Ok(mut inner) = self.inner.lock() {
            if inner.state == PipelineState::Routing {
                inner.transition_to(
                    PipelineState::Transcribing,
                    "transcription_flow (routing done)",
                );
            }
        }
    }

    fn transition_to_rewriting(&self) {
        if let Ok(mut inner) = self.inner.lock() {
            if inner.state == PipelineState::Transcribing {
                inner.transition_to(PipelineState::Rewriting, "transcription_flow (rewrite)");
            }
        }
    }

    fn get_or_create_llm_provider(
        &self,
        provider_id: &str,
        params: LlmProviderParams,
    ) -> Result<Arc<dyn LlmProvider>, PipelineError> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|e| PipelineError::Lock(e.to_string()))?;
        inner.get_or_create_llm_provider(provider_id, params)
    }
}

/// Thread-safe wrapper for the recording pipeline
///
/// Uses standard Mutex to be Send + Sync for Tauri state management.
/// Provides robust error handling and cancellation support.

#[derive(Clone)]
pub struct SharedPipeline {
    inner: Arc<Mutex<PipelineInner>>,
    level_meter: crate::audio_capture::SharedAudioLevelMeter,
    waveform_meter: crate::audio_capture::SharedAudioWaveformMeter,
    embedding_cache: Arc<Mutex<HashMap<String, Vec<f32>>>>,
    app_handle: Arc<Mutex<Option<AppHandle>>>,
    session_preset_lock: Arc<Mutex<Option<SessionPresetLock>>>,
    session_profile_override: Arc<Mutex<Option<String>>>,
}

impl SharedPipeline {
    /// Create a new shared pipeline
    pub fn new(config: PipelineConfig) -> Self {
        let inner = PipelineInner::new(config);
        let level_meter = inner.audio_capture.shared_level_meter();
        let waveform_meter = inner.audio_capture.shared_waveform_meter();
        Self {
            inner: Arc::new(Mutex::new(inner)),
            level_meter,
            waveform_meter,
            embedding_cache: Arc::new(Mutex::new(HashMap::new())),
            app_handle: Arc::new(Mutex::new(None)),
            session_preset_lock: Arc::new(Mutex::new(None)),
            session_profile_override: Arc::new(Mutex::new(None)),
        }
    }

    #[cfg(test)]
    #[allow(dead_code)]
    fn new_for_tests(config: PipelineConfig, audio_capture: Box<dyn AudioCaptureBackend>) -> Self {
        let inner = PipelineInner::new_with_audio_capture(config, audio_capture);
        let level_meter = inner.audio_capture.shared_level_meter();
        let waveform_meter = inner.audio_capture.shared_waveform_meter();
        Self {
            inner: Arc::new(Mutex::new(inner)),
            level_meter,
            waveform_meter,
            embedding_cache: Arc::new(Mutex::new(HashMap::new())),
            app_handle: Arc::new(Mutex::new(None)),
            session_preset_lock: Arc::new(Mutex::new(None)),
            session_profile_override: Arc::new(Mutex::new(None)),
        }
    }

    /// Test-only seam: inject an STT provider into the pipeline cache so we can run
    /// end-to-end pipeline tests without real network calls.
    ///
    /// This intentionally bypasses API-key validation in `get_or_create_stt_provider` by
    /// pre-populating the cache key the pipeline will look up.
    #[cfg(test)]
    fn inject_stt_provider_for_tests(
        &self,
        provider_id: &str,
        model: Option<&str>,
        provider: Arc<dyn SttProvider>,
    ) {
        let mut inner = self.inner.lock().expect("pipeline lock");
        let provider_id = canonicalize_stt_provider_id(provider_id);

        // Keep cache-key construction aligned with `PipelineInner::get_or_create_stt_provider`.
        // For Local Whisper this is special-cased; for tests we keep it simple and only
        // support a normal provider id.
        let model_key = if provider_id == "local-whisper" {
            inner.local_whisper_model_key_for_cache()
        } else {
            model
                .map(|s| s.to_string())
                .unwrap_or_else(|| "<default>".to_string())
        };
        let cache_key = format!("{}::{}", provider_id, model_key);

        inner.stt_provider_cache.insert(cache_key, provider.clone());
        inner.stt_registry.register(&provider_id, provider);
        let _ = inner.stt_registry.set_current(&provider_id);
    }

    /// Test-only seam: inject an LLM provider into the pipeline cache so we can run
    /// end-to-end pipeline tests with rewrite enabled without real network calls.
    #[cfg(test)]
    fn inject_llm_provider_for_tests(
        &self,
        provider_id: &str,
        model: Option<&str>,
        provider: Arc<dyn LlmProvider>,
    ) {
        let mut inner = self.inner.lock().expect("pipeline lock");

        // Build a cache key that matches the normal lookup in `get_or_create_llm_provider`.
        // We use simplified defaults for test keys; real lookups include more fields.
        let model_key = model
            .map(|s| s.to_string())
            .unwrap_or_else(|| "<default>".to_string());
        let cache_key = format!(
            "{}::{}::30::<default-url>::<default-effort>::<default-budget>::<default-level>::<default-budget>",
            provider_id, model_key
        );

        inner.llm_provider_cache.insert(cache_key, provider);
    }

    /// Test-only seam: inject an embeddings provider into the pipeline so we can run
    /// end-to-end pipeline tests with routing enabled without real network calls.
    #[cfg(test)]
    fn inject_embeddings_provider_for_tests(
        &self,
        provider: Arc<dyn crate::embeddings::EmbeddingsProvider>,
    ) {
        let mut inner = self.inner.lock().expect("pipeline lock");
        inner.injected_embeddings_provider = Some(provider);
    }

    /// Provide an app handle for best-effort persistence of recreatable caches.
    pub fn set_app_handle(&self, app: AppHandle) {
        if let Ok(mut guard) = self.app_handle.lock() {
            *guard = Some(app);
        }
    }

    /// Merge precomputed embeddings into the in-memory cache.
    ///
    /// This cache is used by embeddings routing to avoid recomputing per-preset hint embeddings.
    pub fn preload_embedding_cache(&self, entries: HashMap<String, Vec<f32>>) {
        if entries.is_empty() {
            return;
        }

        if let Ok(mut cache) = self.embedding_cache.lock() {
            // Keep cache bounded. (Note: persisted cache may be larger than the runtime cache.)
            if cache.len() + entries.len() > 2048 {
                cache.clear();
            }
            cache.extend(entries);
        }
    }

    pub fn embedding_cache_contains_key(&self, key: &str) -> bool {
        self.embedding_cache
            .lock()
            .ok()
            .and_then(|cache| cache.get(key).map(|v| !v.is_empty()))
            .unwrap_or(false)
    }

    pub fn embedding_cache_get(&self, key: &str) -> Option<Vec<f32>> {
        self.embedding_cache
            .lock()
            .ok()
            .and_then(|cache| cache.get(key).cloned())
            .and_then(|v| if v.is_empty() { None } else { Some(v) })
    }

    /// Set (or clear) the in-memory session profile override.
    ///
    /// This does not persist to disk. When set, the next transcription will prefer
    /// this profile id over selecting based on the current foreground application.
    ///
    /// This helps avoid Windows focus edge cases where our always-on-top overlay
    /// briefly becomes the foreground window during stop/transcribe.
    pub fn set_session_profile_override(
        &self,
        profile_id: Option<String>,
    ) -> Result<(), PipelineError> {
        let mut guard = self
            .session_profile_override
            .lock()
            .map_err(|e| PipelineError::Lock(e.to_string()))?;

        let normalized = profile_id.and_then(|s| {
            let t = s.trim().to_string();
            if t.is_empty() {
                None
            } else {
                Some(t)
            }
        });

        *guard = normalized;
        Ok(())
    }

    fn take_session_profile_override(&self) -> Option<String> {
        self.session_profile_override
            .lock()
            .ok()
            .and_then(|mut g| g.take())
    }

    /// Set (or clear) the in-memory session preset lock.
    ///
    /// This does not persist to disk. When set, it takes precedence over the
    /// persisted profile `active_preset_id` and intent router.
    pub fn set_session_preset_lock(
        &self,
        profile_id: Option<String>,
        preset_id: Option<String>,
    ) -> Result<(), PipelineError> {
        let mut lock = self
            .session_preset_lock
            .lock()
            .map_err(|e| PipelineError::Lock(e.to_string()))?;

        if let Some(preset_id) = preset_id.and_then(|s| {
            let t = s.trim().to_string();
            if t.is_empty() {
                None
            } else {
                Some(t)
            }
        }) {
            let pid = profile_id.and_then(|s| {
                let t = s.trim().to_string();
                if t.is_empty() {
                    None
                } else {
                    Some(t)
                }
            });
            *lock = Some(SessionPresetLock {
                profile_id: pid,
                preset_id,
            });
        } else {
            *lock = None;
        }

        Ok(())
    }

    /// Take (read and clear) the current session preset lock.
    fn take_session_preset_lock(&self) -> Option<SessionPresetLock> {
        self.session_preset_lock
            .lock()
            .ok()
            .and_then(|mut g| g.take())
    }

    /// Read (without clearing) the current session preset lock.
    pub fn peek_session_preset_lock(&self) -> Option<(Option<String>, String)> {
        let guard = self.session_preset_lock.lock().ok()?;
        guard
            .as_ref()
            .map(|lock| (lock.profile_id.clone(), lock.preset_id.clone()))
    }

    /// Try to read the current state without blocking.
    ///
    /// This is useful for UI publishers that should not stall the runtime when
    /// the pipeline mutex is briefly held (e.g., during start-up).
    pub fn try_state(&self) -> Option<PipelineState> {
        self.inner.try_lock().ok().map(|inner| inner.state)
    }

    /// Get the most recent realtime audio input level snapshot without locking
    /// the pipeline mutex.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn audio_level_snapshot_fast(&self) -> AudioLevelSnapshot {
        self.level_meter.snapshot()
    }

    /// Get the most recent realtime waveform min/max buckets without locking the
    /// pipeline mutex.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn audio_waveform_snapshot_fast(&self) -> crate::audio_capture::AudioWaveformSnapshot {
        self.waveform_meter.snapshot()
    }

    /// Start recording
    ///
    /// Creates a new cancellation token for this recording session.
    pub fn start_recording(&self) -> Result<(), PipelineError> {
        // Defensive: clear any previous session preset lock so we don't accidentally
        // apply an override from a prior (cancelled) session.
        //
        // The overlay/hotkey path can still set the lock again while recording.
        let _ = self.set_session_preset_lock(None, None);

        let mut inner = self
            .inner
            .lock()
            .map_err(|e| PipelineError::Lock(e.to_string()))?;

        // State guard: only allow starting from Idle or Error states
        if !inner.state.can_start_recording() {
            return Err(PipelineError::AlreadyRecording);
        }

        // Create a new cancellation token for this session
        let cancel_token = CancellationToken::new();
        inner.cancel_token = Some(cancel_token);

        let max_duration = inner.config.max_duration_secs;
        // Clone out of the config to avoid borrowing `inner` immutably while calling into
        // `audio_capture` mutably.
        let input_device_name = inner.config.input_device_name.clone();
        match inner
            .audio_capture
            .start_recording_session(max_duration, input_device_name.as_deref())
        {
            Ok(()) => {
                inner.transition_to(PipelineState::Recording, "start_recording");
                log::info!("Pipeline: Recording started");
                Ok(())
            }
            Err(e) => {
                inner.set_error(&format!("Failed to start recording: {}", e));
                Err(PipelineError::AudioCapture(e))
            }
        }
    }

    /// Stop recording and return the raw WAV audio
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn stop_recording(&self) -> Result<Vec<u8>, PipelineError> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|e| PipelineError::Lock(e.to_string()))?;

        if !inner.state.can_stop_recording() {
            return Err(PipelineError::NotRecording);
        }

        let encode_cfg = inner.config.audio_encode_config();
        match inner
            .audio_capture
            .stop_and_get_wav_with_diagnostics(encode_cfg)
        {
            Ok((wav_bytes, diagnostics)) => {
                // Keep a copy for STT testing/debugging UI.
                inner.last_wav_bytes = Some(wav_bytes.clone());
                inner.last_recording_diagnostics = Some(diagnostics);

                // Check size limit
                let max_bytes = inner.config.max_recording_bytes;
                if max_bytes > 0 && wav_bytes.len() > max_bytes {
                    inner.set_error(&format!("Recording too large: {} bytes", wav_bytes.len()));
                    return Err(PipelineError::RecordingTooLarge(wav_bytes.len(), max_bytes));
                }

                inner.reset_to_idle();
                log::info!(
                    "Pipeline: Recording stopped, {} bytes captured",
                    wav_bytes.len()
                );
                Ok(wav_bytes)
            }
            Err(e) => {
                inner.set_error(&format!("Failed to stop recording: {}", e));
                Err(PipelineError::AudioCapture(e))
            }
        }
    }

    /// Stop recording and return a before/after pair of WAV bytes.
    ///
    /// - before: raw capture with no preprocessing/gates
    /// - after: capture encoded with the current audio settings
    ///
    /// Intended for settings UI A/B testing.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn stop_recording_before_after(&self) -> Result<(Vec<u8>, Vec<u8>), PipelineError> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|e| PipelineError::Lock(e.to_string()))?;

        if !inner.state.can_stop_recording() {
            return Err(PipelineError::NotRecording);
        }

        let encode_cfg = inner.config.audio_encode_config();
        match inner
            .audio_capture
            .stop_and_get_wav_before_after(encode_cfg)
        {
            Ok((before_wav, after_wav, diagnostics)) => {
                // Keep a copy of the processed output for STT test + debugging.
                inner.last_wav_bytes = Some(after_wav.clone());
                inner.last_recording_diagnostics = Some(diagnostics);

                // Check size limit (both, to avoid surprising huge payloads)
                let max_bytes = inner.config.max_recording_bytes;
                if max_bytes > 0 {
                    if before_wav.len() > max_bytes {
                        inner
                            .set_error(&format!("Recording too large: {} bytes", before_wav.len()));
                        return Err(PipelineError::RecordingTooLarge(
                            before_wav.len(),
                            max_bytes,
                        ));
                    }
                    if after_wav.len() > max_bytes {
                        inner.set_error(&format!("Recording too large: {} bytes", after_wav.len()));
                        return Err(PipelineError::RecordingTooLarge(after_wav.len(), max_bytes));
                    }
                }

                inner.reset_to_idle();
                Ok((before_wav, after_wav))
            }
            Err(e) => {
                inner.set_error(&format!("Failed to stop recording: {}", e));
                Err(PipelineError::AudioCapture(e))
            }
        }
    }

    /// Transcribe the last captured audio (WAV bytes) using the current effective STT settings.
    ///
    /// This is intended for settings UI testing and debugging.
    pub async fn transcribe_last_audio_for_profile(
        &self,
        profile_id: Option<&str>,
    ) -> Result<String, PipelineError> {
        let (wav_bytes, stt_provider, retry_config, cancel_token) = {
            let mut inner = self
                .inner
                .lock()
                .map_err(|e| PipelineError::Lock(e.to_string()))?;

            let wav_bytes = inner.last_wav_bytes.clone().ok_or_else(|| {
                PipelineError::Config(
                    "No audio captured yet. Record once to create test audio.".to_string(),
                )
            })?;

            let config = inner.config.clone();

            // Resolve per-profile overrides. Note: program prompt profiles live under llm_config.
            let profile = profile_id
                .and_then(|id| if id == "default" { None } else { Some(id) })
                .and_then(|id| {
                    config
                        .llm_config
                        .program_prompt_profiles
                        .iter()
                        .find(|p| p.id == id)
                        .cloned()
                });

            let desired_stt_provider = canonicalize_stt_provider_id(
                profile
                    .as_ref()
                    .and_then(|p| p.stt_provider.as_deref())
                    .unwrap_or(config.stt_provider.as_str()),
            );
            let desired_stt_model = profile
                .as_ref()
                .and_then(|p| p.stt_model.clone())
                .or_else(|| config.stt_model.clone());

            let mut stt_provider_id_used = desired_stt_provider.clone();
            let mut stt_model_used: Option<String> = desired_stt_model.clone();

            #[cfg(feature = "local-whisper")]
            if stt_provider_id_used == "local-whisper" {
                stt_model_used = inner
                    .config
                    .whisper_model_path
                    .as_ref()
                    .and_then(|p| p.file_name())
                    .map(|f| f.to_string_lossy().to_string());
            }

            if let Some(store) = inner.config.request_log_store.as_ref() {
                store.with_current(|log| {
                    log.stt_provider = stt_provider_id_used.clone();
                    log.stt_model = stt_model_used.clone();
                });
            }

            let stt_provider = match inner
                .get_or_create_stt_provider(&desired_stt_provider, desired_stt_model.clone())
            {
                Ok(p) => p,
                Err(e) => {
                    // If the profile specified an override provider, fall back to global provider.
                    let global_provider = canonicalize_stt_provider_id(&config.stt_provider);
                    if global_provider != desired_stt_provider {
                        log::warn!(
                            "Pipeline: Profile STT provider '{}' unavailable ({}), falling back to '{}'",
                            desired_stt_provider,
                            e,
                            global_provider
                        );

                        let global_model = config.stt_model.clone();
                        stt_provider_id_used = global_provider.clone();
                        stt_model_used = global_model.clone();

                        #[cfg(feature = "local-whisper")]
                        if stt_provider_id_used == "local-whisper" {
                            stt_model_used = inner
                                .config
                                .whisper_model_path
                                .as_ref()
                                .and_then(|p| p.file_name())
                                .map(|f| f.to_string_lossy().to_string());
                        }

                        if let Some(store) = inner.config.request_log_store.as_ref() {
                            store.with_current(|log| {
                                log.stt_provider = stt_provider_id_used.clone();
                                log.stt_model = stt_model_used.clone();
                            });
                        }

                        inner
                            .get_or_create_stt_provider(&global_provider, global_model)
                            .map_err(|err| {
                                inner.set_error(&format!("No STT provider configured: {}", err));
                                err
                            })?
                    } else {
                        inner.set_error(&format!("STT provider init failed: {}", e));
                        return Err(e);
                    }
                }
            };

            let cancel_token = inner
                .cancel_token
                .clone()
                .unwrap_or_else(CancellationToken::new);

            (
                wav_bytes,
                stt_provider,
                config.retry_config.clone(),
                cancel_token,
            )
        };

        // Test endpoint intentionally does NOT enforce timeout
        let result = run_stt_transcription(
            stt_provider,
            &wav_bytes,
            &retry_config,
            None, // no timeout for test endpoint
            &cancel_token,
            "Pipeline (test)",
        )
        .await?;

        Ok(result.text)
    }

    /// Stop recording and transcribe the audio, returning a detailed result.
    ///
    /// This is the main end-to-end function for voice dictation.
    /// Includes:
    /// - Automatic retry with exponential backoff on transient failures
    /// - Timeout protection
    /// - Cancellation support
    /// - Proper error recovery
    /// - Optional LLM formatting
    pub async fn stop_and_transcribe_detailed(&self) -> Result<TranscriptionResult, PipelineError> {
        // Profile override is per recording session; take + clear it now so it doesn't
        // leak into the next request.
        let session_profile_override = self.take_session_profile_override();

        // Phase 1: Stop recording and prepare for transcription (synchronous, holds lock briefly)
        let (
            wav_bytes,
            stt_provider,
            retry_config,
            timeout,
            cancel_token,
            active_profile,
            default_rewrite_include_clipboard_context,
        ) = {
            let mut inner = self
                .inner
                .lock()
                .map_err(|e| PipelineError::Lock(e.to_string()))?;

            if !inner.state.can_stop_recording() {
                return Err(PipelineError::NotRecording);
            }

            let encode_cfg = inner.config.audio_encode_config();
            let (wav_bytes, diagnostics) = match inner
                .audio_capture
                .stop_and_get_wav_with_diagnostics(encode_cfg)
            {
                Ok(out) => out,
                Err(e) => {
                    inner.set_error(&format!("Failed to stop recording: {}", e));
                    return Err(PipelineError::AudioCapture(e));
                }
            };

            let stats = diagnostics.stats;

            // Persist diagnostics for UI readout.
            inner.last_recording_diagnostics = Some(diagnostics);

            // Keep a copy for STT testing/debugging UI.
            inner.last_wav_bytes = Some(wav_bytes.clone());

            // Optional extra hallucination protection: if VAD says "no speech", skip STT.
            if inner.config.quiet_audio_gate_enabled
                && inner.config.quiet_audio_require_speech
                && inner
                    .last_recording_diagnostics
                    .and_then(|d| d.speech_detected)
                    == Some(false)
            {
                log::info!(
                    "Pipeline: Skipping STT because no speech was detected by offline VAD (duration {:.2}s, rms {:.1} dBFS, peak {:.1} dBFS)",
                    stats.duration_secs,
                    amp_to_dbfs(stats.rms),
                    amp_to_dbfs(stats.peak)
                );

                inner.reset_to_idle();
                return Ok(TranscriptionResult {
                    stt_text: String::new(),
                    final_text: String::new(),
                    stt_duration_ms: 0,
                    llm_duration_ms: None,
                    llm_provider_used: None,
                    llm_model_used: None,
                    llm_outcome: LlmOutcome::NotAttempted(
                        LlmNotAttemptedReason::NoSpeechDetectedByVad,
                    ),
                });
            }

            if inner.config.quiet_audio_gate_enabled
                && is_effectively_quiet(
                    stats,
                    inner.config.quiet_audio_min_duration_secs,
                    inner.config.quiet_audio_rms_dbfs_threshold,
                    inner.config.quiet_audio_peak_dbfs_threshold,
                )
            {
                log::info!(
                    "Pipeline: Skipping STT because recording is quiet (duration {:.2}s, rms {:.1} dBFS, peak {:.1} dBFS)",
                    stats.duration_secs,
                    amp_to_dbfs(stats.rms),
                    amp_to_dbfs(stats.peak)
                );

                inner.reset_to_idle();
                return Ok(TranscriptionResult {
                    stt_text: String::new(),
                    final_text: String::new(),
                    stt_duration_ms: 0,
                    llm_duration_ms: None,
                    llm_provider_used: None,
                    llm_model_used: None,
                    llm_outcome: LlmOutcome::NotAttempted(LlmNotAttemptedReason::QuietAudioGate),
                });
            }

            // Check size limit
            let max_bytes = inner.config.max_recording_bytes;
            if max_bytes > 0 && wav_bytes.len() > max_bytes {
                inner.set_error(&format!("Recording too large: {} bytes", wav_bytes.len()));
                return Err(PipelineError::RecordingTooLarge(wav_bytes.len(), max_bytes));
            }

            inner.transition_to(PipelineState::Transcribing, "stop_and_transcribe_detailed");

            let llm_config = inner.config.llm_config.clone();
            let active_profile = session_profile_override
                .as_deref()
                .and_then(|id| {
                    llm_config
                        .program_prompt_profiles
                        .iter()
                        .find(|p| p.id == id)
                        .cloned()
                })
                .or_else(|| select_profile_for_foreground_app(&llm_config))
                .or_else(|| select_default_profile(&llm_config));

            let default_rewrite_include_clipboard_context = select_default_profile(&llm_config)
                .and_then(|p| p.rewrite_include_clipboard_context)
                .unwrap_or(false);

            let active_preset = active_profile
                .as_ref()
                .and_then(|profile| select_effective_preset(profile));

            // Persist the *actual* profile used for this request into the request log.
            // Note: picking the profile at transcription time tends to be more accurate than
            // at recording start (e.g. overlay window can steal focus).
            if let Some(store) = inner.config.request_log_store.as_ref() {
                let (profile_id, profile_name) = if let Some(p) = active_profile.as_ref() {
                    (Some(p.id.clone()), Some(p.name.clone()))
                } else if session_profile_override.as_deref() == Some("default") {
                    (Some("default".to_string()), Some("Default".to_string()))
                } else if let Some(id) = session_profile_override.as_deref() {
                    (Some(id.to_string()), None)
                } else {
                    (Some("default".to_string()), Some("Default".to_string()))
                };

                store.with_current(|log| {
                    log.profile_id = profile_id;
                    log.profile_name = profile_name;
                });
            }
            // Resolve effective STT settings (profile overrides -> global defaults, with safe fallback)
            let desired_stt_provider = canonicalize_stt_provider_id(
                active_preset
                    .and_then(|p| p.stt_provider.as_deref())
                    .or_else(|| {
                        active_profile
                            .as_ref()
                            .and_then(|p| p.stt_provider.as_deref())
                    })
                    .unwrap_or(inner.config.stt_provider.as_str()),
            );
            let desired_stt_model = active_preset
                .and_then(|p| p.stt_model.clone())
                .or_else(|| active_profile.as_ref().and_then(|p| p.stt_model.clone()))
                .or_else(|| inner.config.stt_model.clone());
            let desired_timeout = active_preset
                .and_then(|p| p.stt_timeout_seconds)
                .or_else(|| active_profile.as_ref().and_then(|p| p.stt_timeout_seconds))
                .map(|s| seconds_to_duration_or(s, inner.config.transcription_timeout))
                .unwrap_or(inner.config.transcription_timeout);

            let mut stt_provider_id_used = desired_stt_provider.clone();
            let mut stt_model_used: Option<String> = desired_stt_model.clone();

            #[cfg(feature = "local-whisper")]
            if stt_provider_id_used == "local-whisper" {
                stt_model_used = inner
                    .config
                    .whisper_model_path
                    .as_ref()
                    .and_then(|p| p.file_name())
                    .map(|f| f.to_string_lossy().to_string());
            }

            // Persist the *intended/effective* provider/model into the request log before
            // provider initialization. This keeps logs accurate even when provider creation
            // fails (e.g., Local Whisper manual mode without preload).
            if let Some(store) = inner.config.request_log_store.as_ref() {
                store.with_current(|log| {
                    log.stt_provider = stt_provider_id_used.clone();
                    log.stt_model = stt_model_used.clone();
                });
            }

            let stt_provider = match inner
                .get_or_create_stt_provider(&desired_stt_provider, desired_stt_model.clone())
            {
                Ok(p) => p,
                Err(e) => {
                    // If the profile specified an override provider, fall back to global provider.
                    let global_provider = canonicalize_stt_provider_id(&inner.config.stt_provider);
                    if global_provider != desired_stt_provider {
                        log::warn!(
                            "Pipeline: Profile STT provider '{}' unavailable ({}), falling back to '{}'",
                            desired_stt_provider,
                            e,
                            global_provider
                        );
                        let global_model = inner.config.stt_model.clone();
                        stt_provider_id_used = global_provider.clone();
                        stt_model_used = global_model.clone();

                        #[cfg(feature = "local-whisper")]
                        if stt_provider_id_used == "local-whisper" {
                            stt_model_used = inner
                                .config
                                .whisper_model_path
                                .as_ref()
                                .and_then(|p| p.file_name())
                                .map(|f| f.to_string_lossy().to_string());
                        }

                        if let Some(store) = inner.config.request_log_store.as_ref() {
                            store.with_current(|log| {
                                log.stt_provider = stt_provider_id_used.clone();
                                log.stt_model = stt_model_used.clone();
                            });
                        }

                        inner
                            .get_or_create_stt_provider(&global_provider, global_model)
                            .map_err(|err| {
                                inner.set_error(&format!("No STT provider configured: {}", err));
                                err
                            })?
                    } else {
                        // Preserve the real failure reason (e.g. missing API key, manual local-whisper not loaded)
                        // instead of collapsing into the generic NoProvider.
                        inner.set_error(&format!("STT provider init failed: {}", e));
                        return Err(e);
                    }
                }
            };

            let retry_config = inner.config.retry_config.clone();
            let cancel_token = inner
                .cancel_token
                .clone()
                .unwrap_or_else(CancellationToken::new);

            (
                wav_bytes,
                stt_provider,
                retry_config,
                desired_timeout,
                cancel_token,
                active_profile,
                default_rewrite_include_clipboard_context,
            )
        };

        log::info!(
            "Pipeline: Starting transcription ({} bytes, timeout {:?})",
            wav_bytes.len(),
            timeout
        );

        // Phase 2: Transcribe with retry logic (async, outside the lock)
        let stt_result = run_stt_transcription(
            stt_provider,
            &wav_bytes,
            &retry_config,
            Some(timeout),
            &cancel_token,
            "Pipeline",
        )
        .await;

        let (stt_text, stt_duration_ms) = match stt_result {
            Ok(result) => (result.text, result.duration_ms),
            Err(e) => {
                let mut inner = self
                    .inner
                    .lock()
                    .map_err(|err| PipelineError::Lock(err.to_string()))?;
                if matches!(e, PipelineError::Cancelled) {
                    inner.reset_to_idle();
                } else {
                    inner.set_error(&e.to_string());
                }
                return Err(e);
            }
        };

        // Phase 3-4: Routing and LLM rewrite via transcription_flow module
        let (proxy_settings, llm_api_keys, request_log_store, llm_enabled_global, llm_config) = {
            let inner = self
                .inner
                .lock()
                .map_err(|e| PipelineError::Lock(e.to_string()))?;
            (
                inner.config.proxy_settings.clone(),
                inner.config.llm_api_keys.clone(),
                inner.config.request_log_store.clone(),
                inner.config.llm_config.enabled,
                inner.config.llm_config.clone(),
            )
        };

        // Session lock is a one-shot override: take + clear it now so it only
        // applies to this transcription attempt.
        let session_lock = self.take_session_preset_lock();
        let persist_app = self.app_handle.lock().ok().and_then(|g| g.clone());

        // Retrieve injected embeddings provider for testing (None in production)
        let injected_embeddings_provider = {
            let inner = self
                .inner
                .lock()
                .map_err(|e| PipelineError::Lock(e.to_string()))?;
            inner.injected_embeddings_provider.clone()
        };

        let ctx = TranscriptionContext {
            active_profile: active_profile.clone(),
            llm_enabled_global,
            default_rewrite_include_clipboard_context,
            session_lock: session_lock.map(|l| transcription_flow::SessionPresetLock {
                profile_id: l.profile_id,
                preset_id: l.preset_id,
            }),
            proxy_settings,
            llm_api_keys,
            request_log_store,
            embedding_cache: &self.embedding_cache,
            persist_app,
            cancel_token: cancel_token.clone(),
            injected_embeddings_provider,
        };

        let callbacks = PipelineCallbacks {
            inner: self.inner.clone(),
        };

        let result =
            complete_transcription_flow(&ctx, &callbacks, &stt_text, stt_duration_ms, &llm_config)
                .await;

        // Phase 5: Update state to idle
        {
            let mut inner = self
                .inner
                .lock()
                .map_err(|e| PipelineError::Lock(e.to_string()))?;
            inner.reset_to_idle();
            log::info!(
                "Pipeline: Complete, {} chars output",
                result.final_text.len()
            );
        }

        Ok(result)
    }

    /// Transcribe provided WAV bytes using the same STT + optional LLM logic as the main pipeline.
    ///
    /// This is used for retrying failed requests from persisted audio.
    #[allow(dead_code)]
    pub async fn transcribe_wav_bytes_detailed(
        &self,
        wav_bytes: Vec<u8>,
    ) -> Result<TranscriptionResult, PipelineError> {
        self.transcribe_wav_bytes_detailed_for_profile(wav_bytes, None)
            .await
    }

    /// Transcribe provided WAV bytes, optionally forcing a specific prompt profile.
    ///
    /// When `profile_id_override` is provided, we attempt to use that per-program profile
    /// (by id) instead of selecting based on the current foreground application.
    pub async fn transcribe_wav_bytes_detailed_for_profile(
        &self,
        wav_bytes: Vec<u8>,
        profile_id_override: Option<&str>,
    ) -> Result<TranscriptionResult, PipelineError> {
        // Phase 1: Resolve providers/config under lock.
        let (
            stt_provider,
            retry_config,
            timeout,
            cancel_token,
            active_profile,
            default_rewrite_include_clipboard_context,
        ) = {
            let mut inner = self
                .inner
                .lock()
                .map_err(|e| PipelineError::Lock(e.to_string()))?;

            // Guard: don't run a retry while actively recording.
            if inner.state == PipelineState::Recording {
                return Err(PipelineError::AlreadyRecording);
            }
            if matches!(
                inner.state,
                PipelineState::Transcribing | PipelineState::Rewriting
            ) {
                return Err(PipelineError::Lock(
                    "Pipeline already transcribing".to_string(),
                ));
            }

            // Keep a copy for STT testing/debugging UI.
            inner.last_wav_bytes = Some(wav_bytes.clone());

            // Check size limit
            let max_bytes = inner.config.max_recording_bytes;
            if max_bytes > 0 && wav_bytes.len() > max_bytes {
                inner.set_error(&format!("Recording too large: {} bytes", wav_bytes.len()));
                return Err(PipelineError::RecordingTooLarge(wav_bytes.len(), max_bytes));
            }

            inner.transition_to(
                PipelineState::Transcribing,
                "transcribe_wav_bytes_detailed_for_profile",
            );

            // Ensure we have a cancellation token for this attempt.
            let cancel_token = CancellationToken::new();
            inner.cancel_token = Some(cancel_token.clone());

            let llm_config = inner.config.llm_config.clone();
            let active_profile = profile_id_override
                .and_then(|id| {
                    llm_config
                        .program_prompt_profiles
                        .iter()
                        .find(|p| p.id == id)
                        .cloned()
                })
                .or_else(|| select_profile_for_foreground_app(&llm_config))
                .or_else(|| select_default_profile(&llm_config));

            let default_rewrite_include_clipboard_context = select_default_profile(&llm_config)
                .and_then(|p| p.rewrite_include_clipboard_context)
                .unwrap_or(false);

            let active_preset = active_profile
                .as_ref()
                .and_then(|profile| select_effective_preset(profile));

            // Persist the profile being used for this retry attempt into the request log, if available.
            if let Some(store) = inner.config.request_log_store.as_ref() {
                let (profile_id, profile_name) = if let Some(p) = active_profile.as_ref() {
                    (Some(p.id.clone()), Some(p.name.clone()))
                } else if profile_id_override == Some("default") {
                    (Some("default".to_string()), Some("Default".to_string()))
                } else if let Some(id) = profile_id_override {
                    (Some(id.to_string()), None)
                } else {
                    (Some("default".to_string()), Some("Default".to_string()))
                };

                store.with_current(|log| {
                    log.profile_id = profile_id;
                    log.profile_name = profile_name;
                });
            }
            // Resolve effective STT settings (profile overrides -> global defaults, with safe fallback)
            let desired_stt_provider = canonicalize_stt_provider_id(
                active_preset
                    .and_then(|p| p.stt_provider.as_deref())
                    .or_else(|| {
                        active_profile
                            .as_ref()
                            .and_then(|p| p.stt_provider.as_deref())
                    })
                    .unwrap_or(inner.config.stt_provider.as_str()),
            );
            let desired_stt_model = active_preset
                .and_then(|p| p.stt_model.clone())
                .or_else(|| active_profile.as_ref().and_then(|p| p.stt_model.clone()))
                .or_else(|| inner.config.stt_model.clone());
            let desired_timeout = active_preset
                .and_then(|p| p.stt_timeout_seconds)
                .or_else(|| active_profile.as_ref().and_then(|p| p.stt_timeout_seconds))
                .map(|s| seconds_to_duration_or(s, inner.config.transcription_timeout))
                .unwrap_or(inner.config.transcription_timeout);

            let mut stt_provider_id_used = desired_stt_provider.clone();
            let mut stt_model_used: Option<String> = desired_stt_model.clone();

            #[cfg(feature = "local-whisper")]
            if stt_provider_id_used == "local-whisper" {
                stt_model_used = inner
                    .config
                    .whisper_model_path
                    .as_ref()
                    .and_then(|p| p.file_name())
                    .map(|f| f.to_string_lossy().to_string());
            }

            if let Some(store) = inner.config.request_log_store.as_ref() {
                store.with_current(|log| {
                    log.stt_provider = stt_provider_id_used.clone();
                    log.stt_model = stt_model_used.clone();
                });
            }

            let stt_provider = match inner
                .get_or_create_stt_provider(&desired_stt_provider, desired_stt_model.clone())
            {
                Ok(p) => p,
                Err(e) => {
                    // If the profile specified an override provider, fall back to global provider.
                    let global_provider = canonicalize_stt_provider_id(&inner.config.stt_provider);
                    if global_provider != desired_stt_provider {
                        log::warn!(
                            "Pipeline: Profile STT provider '{}' unavailable ({}), falling back to '{}'",
                            desired_stt_provider,
                            e,
                            global_provider
                        );
                        let global_model = inner.config.stt_model.clone();
                        stt_provider_id_used = global_provider.clone();
                        stt_model_used = global_model.clone();

                        #[cfg(feature = "local-whisper")]
                        if stt_provider_id_used == "local-whisper" {
                            stt_model_used = inner
                                .config
                                .whisper_model_path
                                .as_ref()
                                .and_then(|p| p.file_name())
                                .map(|f| f.to_string_lossy().to_string());
                        }

                        if let Some(store) = inner.config.request_log_store.as_ref() {
                            store.with_current(|log| {
                                log.stt_provider = stt_provider_id_used.clone();
                                log.stt_model = stt_model_used.clone();
                            });
                        }

                        inner
                            .get_or_create_stt_provider(&global_provider, global_model)
                            .map_err(|err| {
                                inner.set_error(&format!("No STT provider configured: {}", err));
                                err
                            })?
                    } else {
                        inner.set_error(&format!("STT provider init failed: {}", e));
                        return Err(e);
                    }
                }
            };

            let retry_config = inner.config.retry_config.clone();

            (
                stt_provider,
                retry_config,
                desired_timeout,
                cancel_token,
                active_profile,
                default_rewrite_include_clipboard_context,
            )
        };

        log::info!(
            "Pipeline: Starting retry transcription ({} bytes, timeout {:?})",
            wav_bytes.len(),
            timeout
        );

        // Phase 2: STT transcription
        let stt_result = run_stt_transcription(
            stt_provider,
            &wav_bytes,
            &retry_config,
            Some(timeout),
            &cancel_token,
            "Pipeline (retry)",
        )
        .await;

        let (stt_text, stt_duration_ms) = match stt_result {
            Ok(result) => (result.text, result.duration_ms),
            Err(e) => {
                let mut inner = self
                    .inner
                    .lock()
                    .map_err(|err| PipelineError::Lock(err.to_string()))?;
                if matches!(e, PipelineError::Cancelled) {
                    inner.reset_to_idle();
                } else {
                    inner.set_error(&e.to_string());
                }
                return Err(e);
            }
        };

        // Phase 3-4: Routing and LLM rewrite via transcription_flow module
        let (proxy_settings, llm_api_keys, request_log_store, llm_enabled_global, llm_config) = {
            let inner = self
                .inner
                .lock()
                .map_err(|e| PipelineError::Lock(e.to_string()))?;
            (
                inner.config.proxy_settings.clone(),
                inner.config.llm_api_keys.clone(),
                inner.config.request_log_store.clone(),
                inner.config.llm_config.enabled,
                inner.config.llm_config.clone(),
            )
        };

        let session_lock = self.take_session_preset_lock();
        let persist_app = self.app_handle.lock().ok().and_then(|g| g.clone());

        // Retrieve injected embeddings provider for testing (None in production)
        let injected_embeddings_provider = {
            let inner = self
                .inner
                .lock()
                .map_err(|e| PipelineError::Lock(e.to_string()))?;
            inner.injected_embeddings_provider.clone()
        };

        let ctx = TranscriptionContext {
            active_profile: active_profile.clone(),
            llm_enabled_global,
            default_rewrite_include_clipboard_context,
            session_lock: session_lock.map(|l| transcription_flow::SessionPresetLock {
                profile_id: l.profile_id,
                preset_id: l.preset_id,
            }),
            proxy_settings,
            llm_api_keys,
            request_log_store,
            embedding_cache: &self.embedding_cache,
            persist_app,
            cancel_token: cancel_token.clone(),
            injected_embeddings_provider,
        };

        let callbacks = PipelineCallbacks {
            inner: self.inner.clone(),
        };

        let result =
            complete_transcription_flow(&ctx, &callbacks, &stt_text, stt_duration_ms, &llm_config)
                .await;

        // Phase 5: Update state to idle
        {
            let mut inner = self
                .inner
                .lock()
                .map_err(|e| PipelineError::Lock(e.to_string()))?;
            inner.reset_to_idle();
            log::info!(
                "Pipeline: Retry complete, {} chars output",
                result.final_text.len()
            );
        }

        Ok(result)
    }

    /// Stop recording and transcribe the audio.
    ///
    /// Kept for backwards compatibility. Prefer `stop_and_transcribe_detailed`.
    #[allow(dead_code)]
    pub async fn stop_and_transcribe(&self) -> Result<String, PipelineError> {
        self.stop_and_transcribe_detailed()
            .await
            .map(|r| r.final_text)
    }

    /// Update configuration
    ///
    /// Note: This will not affect an in-progress recording.
    pub fn update_config(&self, config: PipelineConfig) -> Result<(), PipelineError> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|e| PipelineError::Lock(e.to_string()))?;

        // If the local-whisper model path changed, evict cached models.
        // Otherwise switching models could keep multiple large GGML files resident.
        let old_local_whisper_key = inner.local_whisper_model_key_for_cache();
        let old_stt_prompt = inner.config.stt_transcription_prompt.clone();

        // Don't update config while recording - could cause issues
        if inner.state == PipelineState::Recording {
            log::warn!("Pipeline: Config update requested while recording, will take effect after current session");
        }

        inner.config = config.clone();

        let new_local_whisper_key = inner.local_whisper_model_key_for_cache();
        if old_local_whisper_key != new_local_whisper_key {
            inner.unload_local_whisper();
        }

        // If the transcription prompt changed, the model should be reloaded so the new
        // prompt is applied. We unload only (no auto-load) to respect the user's load mode.
        if old_stt_prompt != inner.config.stt_transcription_prompt {
            inner.unload_local_whisper();
        }

        inner.stt_registry = SttRegistry::new();
        inner.initialize_providers(&config);
        // Update VAD config on audio capture
        inner.audio_capture.set_vad_config(config.vad_config);

        // Apply capture behavior (Hot Mic + auto-recovery).
        // Safe to call while recording: it won't stop the stream mid-session.
        inner
            .audio_capture
            .set_capture_behavior(
                config.hot_mic_enabled,
                config.hot_mic_pre_roll_ms,
                config.mic_auto_recover_enabled,
                config.input_device_name.as_deref(),
            )
            .map_err(PipelineError::AudioCapture)?;
        log::info!("Pipeline configuration updated");
        Ok(())
    }

    /// Temporarily override the audio capture behavior without updating the full PipelineConfig.
    ///
    /// This is intended for short-lived UI utilities (e.g. Settings mic level test) that
    /// need a CPAL stream running to drive realtime meters.
    pub fn set_capture_behavior_override(
        &self,
        hot_mic_enabled: bool,
        hot_mic_pre_roll_ms: u32,
        mic_auto_recover_enabled: bool,
        input_device_name: Option<&str>,
    ) -> Result<(), PipelineError> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|e| PipelineError::Lock(e.to_string()))?;

        // Never stop/retarget the stream mid-recording.
        if inner.state == PipelineState::Recording {
            return Err(PipelineError::AlreadyRecording);
        }

        inner
            .audio_capture
            .set_capture_behavior(
                hot_mic_enabled,
                hot_mic_pre_roll_ms,
                mic_auto_recover_enabled,
                input_device_name,
            )
            .map_err(PipelineError::AudioCapture)?;

        Ok(())
    }

    /// Check if recording
    pub fn is_recording(&self) -> bool {
        self.inner
            .lock()
            .map(|inner| inner.state == PipelineState::Recording)
            .unwrap_or(false)
    }

    /// Get a clone of the last captured WAV bytes, if present.
    pub fn clone_last_wav_bytes(&self) -> Option<Vec<u8>> {
        self.inner
            .lock()
            .ok()
            .and_then(|inner| inner.last_wav_bytes.clone())
    }

    /// Get a copy of the last recording diagnostics (raw stats + optional speech detection).
    pub fn last_recording_diagnostics(&self) -> Option<AudioCaptureDiagnostics> {
        self.inner
            .lock()
            .ok()
            .and_then(|inner| inner.last_recording_diagnostics)
    }

    /// Poll for VAD events (non-blocking)
    ///
    /// Returns the next VAD event if one is available, or None if no events are pending.
    #[allow(dead_code)]
    pub fn poll_vad_event(&self) -> Option<AudioCaptureEvent> {
        self.inner
            .lock()
            .ok()
            .and_then(|inner| inner.audio_capture.poll_vad_event())
    }

    /// Check if VAD auto-stop is enabled
    #[allow(dead_code)]
    pub fn is_vad_auto_stop_enabled(&self) -> bool {
        self.inner
            .lock()
            .map(|inner| inner.audio_capture.is_vad_auto_stop_enabled())
            .unwrap_or(false)
    }

    /// Cancel current operation
    ///
    /// This will:
    /// - Stop any ongoing recording
    /// - Signal cancellation to any in-flight transcription
    /// - Reset the pipeline to Idle state
    pub fn cancel(&self) {
        if let Ok(mut inner) = self.inner.lock() {
            if !inner.state.can_cancel() {
                log::debug!(
                    "Pipeline: Cancel requested but nothing to cancel (state: {:?})",
                    inner.state
                );
                return;
            }

            // Signal cancellation to any async tasks
            if let Some(token) = inner.cancel_token.take() {
                token.cancel();
            }

            // Stop audio capture if recording
            if inner.state == PipelineState::Recording {
                inner.audio_capture.stop_recording();
            }

            inner.reset_to_idle();
            log::info!("Pipeline: Cancelled and reset to idle");
        }
    }

    /// Force reset the pipeline to idle state
    ///
    /// Use this to recover from stuck states. Cancels any in-progress operations.
    pub fn force_reset(&self) {
        if let Ok(mut inner) = self.inner.lock() {
            // Cancel any async tasks
            if let Some(token) = inner.cancel_token.take() {
                token.cancel();
            }

            // Force stop audio capture
            inner.audio_capture.stop();

            // Reset state
            inner.reset_to_idle();
            log::warn!("Pipeline: Force reset to idle");
        }
    }

    /// Get current state
    pub fn state(&self) -> PipelineState {
        self.inner
            .lock()
            .map(|inner| inner.state)
            .unwrap_or(PipelineState::Error)
    }

    /// Get the most recent realtime audio input level snapshot.
    ///
    /// This is cheap and intended for UI metering (e.g., overlay waveform). The snapshot is
    /// updated from the CPAL input callback while recording.
    #[allow(dead_code)]
    pub fn audio_level_snapshot(&self) -> AudioLevelSnapshot {
        self.inner
            .lock()
            .map(|inner| inner.audio_capture.level_snapshot())
            .unwrap_or(AudioLevelSnapshot {
                seq: 0,
                rms: 0.0,
                peak: 0.0,
            })
    }

    /// Get the name of the current STT provider
    #[allow(dead_code)]
    pub fn current_provider_name(&self) -> String {
        self.inner
            .lock()
            .map(|inner| inner.stt_registry.current_name().to_string())
            .unwrap_or_default()
    }

    /// Get a clone of the current pipeline configuration
    pub fn config(&self) -> PipelineConfig {
        self.inner
            .lock()
            .map(|inner| inner.config.clone())
            .unwrap_or_default()
    }

    pub fn is_local_whisper_loaded(&self) -> bool {
        self.inner
            .lock()
            .map(|inner| inner.is_local_whisper_loaded())
            .unwrap_or(false)
    }

    pub fn unload_local_whisper(&self) -> Result<(), PipelineError> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|e| PipelineError::Lock(e.to_string()))?;

        if inner.state == PipelineState::Recording {
            return Err(PipelineError::AlreadyRecording);
        }

        inner.unload_local_whisper();
        Ok(())
    }

    pub fn force_load_local_whisper(&self) -> Result<(), PipelineError> {
        #[cfg(feature = "local-whisper")]
        {
            // Phase 1: fast path + capture config while holding the lock briefly.
            let (cache_key, model_path, transcription_prompt) = {
                let inner = self
                    .inner
                    .lock()
                    .map_err(|e| PipelineError::Lock(e.to_string()))?;

                if inner.state == PipelineState::Recording {
                    return Err(PipelineError::AlreadyRecording);
                }

                let cache_key = inner.local_whisper_cache_key();
                if inner.stt_provider_cache.contains_key(&cache_key) {
                    return Ok(());
                }

                let Some(model_path) = inner.config.whisper_model_path.clone() else {
                    return Err(PipelineError::Config(
                        "Local Whisper: no model path configured".to_string(),
                    ));
                };

                (
                    cache_key,
                    model_path,
                    inner.config.stt_transcription_prompt.clone(),
                )
            };

            // Phase 2: load the model outside the lock (this can take seconds).
            let provider =
                crate::stt::LocalWhisperProvider::with_config(crate::stt::LocalWhisperConfig {
                    model_path,
                    transcription_prompt,
                    ..Default::default()
                })
                .map_err(|e| PipelineError::Config(format!("Local Whisper init failed: {}", e)))?;
            let provider = Arc::new(provider);

            // Phase 3: insert into cache under lock.
            let mut inner = self
                .inner
                .lock()
                .map_err(|e| PipelineError::Lock(e.to_string()))?;

            // If recording started while we were loading, don't mutate pipeline state/caches.
            if inner.state == PipelineState::Recording {
                return Err(PipelineError::AlreadyRecording);
            }

            inner
                .stt_provider_cache
                .entry(cache_key)
                .or_insert(provider);

            Ok(())
        }

        #[cfg(not(feature = "local-whisper"))]
        {
            let mut inner = self
                .inner
                .lock()
                .map_err(|e| PipelineError::Lock(e.to_string()))?;

            if inner.state == PipelineState::Recording {
                return Err(PipelineError::AlreadyRecording);
            }

            inner.force_load_local_whisper()
        }
    }

    /// Check if the pipeline is in an error state
    pub fn is_error(&self) -> bool {
        self.inner
            .lock()
            .map(|inner| inner.state == PipelineState::Error)
            .unwrap_or(true)
    }

    /// Whether there is a previously captured audio buffer available for testing.
    pub fn has_last_audio(&self) -> bool {
        self.inner
            .lock()
            .ok()
            .and_then(|inner| inner.last_wav_bytes.as_ref().map(|b| !b.is_empty()))
            .unwrap_or(false)
    }

    /// Get the cancellation token for external use (e.g., for coordinating with other async tasks)
    #[allow(dead_code)]
    pub fn get_cancel_token(&self) -> Option<CancellationToken> {
        self.inner
            .lock()
            .ok()
            .and_then(|inner| inner.cancel_token.clone())
    }
}

impl Default for SharedPipeline {
    fn default() -> Self {
        Self::new(PipelineConfig::default())
    }
}

// Ensure SharedPipeline is Send + Sync for Tauri state
unsafe impl Send for SharedPipeline {}
unsafe impl Sync for SharedPipeline {}

// Tests have been moved to `pipeline/tests.rs` for better organization.
