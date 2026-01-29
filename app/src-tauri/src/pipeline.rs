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
use crate::event_payloads::OverlayOcrContextUnavailablePayload;
use crate::events;
use crate::llm::LlmProvider;
use crate::request_log::RequestLogStore;
use crate::stt::{SttProvider, SttRegistry};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tokio_util::sync::CancellationToken;

mod config;
mod llm_provider;
mod program_profiles;
mod recording;
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
pub use config::{OcrConfig, PipelineConfig};

pub use state_machine::PipelineState;
pub use types::{LlmNotAttemptedReason, LlmOutcome, PipelineError, TranscriptionResult};

pub(crate) use program_profiles::{
    resolve_quick_ask_active_window_ocr_mode, resolve_quick_replace_active_window_ocr_mode,
    resolve_rewrite_active_window_ocr_mode, select_profile_for_foreground_app,
};
use program_profiles::{select_default_profile, select_effective_preset};

pub(crate) fn should_auto_start_active_window_ocr(
    is_quick_ask_session: bool,
    rewrite_ocr_mode: &str,
    quick_ask_ocr_mode: &str,
    quick_replace_ocr_mode: &str,
) -> bool {
    program_profiles::should_auto_start_active_window_ocr(
        is_quick_ask_session,
        rewrite_ocr_mode,
        quick_ask_ocr_mode,
        quick_replace_ocr_mode,
    )
}

use llm_provider::{create_llm_provider, LlmProviderParams};
use stt_flow::run_stt_transcription;
use utils::seconds_to_duration_or;

fn truncate_overlay_reason(reason: &str) -> String {
    // Keep overlay messages short; users can inspect request logs for full details.
    const CAP: usize = 220;
    let trimmed = reason.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if trimmed.chars().count() <= CAP {
        return trimmed.to_string();
    }
    let mut out = String::new();
    for (i, ch) in trimmed.chars().enumerate() {
        if i >= CAP.saturating_sub(1) {
            break;
        }
        out.push(ch);
    }
    out.push('…');
    out
}

fn emit_overlay_ocr_context_unavailable(app: &AppHandle, reason: Option<String>) {
    // Best-effort: correlate with current request id so the user can click into logs.
    let request_id = app
        .try_state::<RequestLogStore>()
        .and_then(|store| store.with_current(|log| log.id.clone()));

    let reason = reason.and_then(|r| {
        let t = truncate_overlay_reason(&r);
        if t.is_empty() {
            None
        } else {
            Some(t)
        }
    });

    let payload = OverlayOcrContextUnavailablePayload {
        message: "OCR context unavailable".to_string(),
        reason,
        request_id,
    };

    if let Some(overlay) = app.get_webview_window("overlay") {
        let _ = overlay.emit(events::EVENT_OVERLAY_OCR_CONTEXT_UNAVAILABLE, payload);
    } else {
        let _ = app.emit(events::EVENT_OVERLAY_OCR_CONTEXT_UNAVAILABLE, payload);
    }
}

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

    /// Identifier for the current user-visible request/session.
    ///
    /// This is intentionally decoupled from the pipeline's internal state machine so that
    /// best-effort OCR can remain consumable across internal transitions like `reset_to_idle()`.
    ///
    /// For now we reuse the Request Log id (UUID string) as the session id.
    ocr_session_id: Option<String>,

    /// In-flight OCR task for the current session (best-effort).
    ocr_task: Option<OcrTaskHandle>,
    /// Completed OCR result for reuse within the session.
    ocr_result: Option<crate::ocr::OcrResult>,
    /// Best-effort OCR failure reason (sanitized).
    ocr_failed_reason: Option<String>,
    /// Whether OCR was explicitly cancelled for this session.
    ocr_cancelled: bool,
    /// Whether the OCR task is currently being awaited by `get_ocr_result_with_timeout`.
    ///
    /// This flag is needed because `get_ocr_result_with_timeout` temporarily "takes" the
    /// task handle to await it, making `ocr_task` appear `None` even though OCR is still running.
    ocr_awaiting: bool,

    /// True after STT portion completes but before LLM / output.
    ///
    /// Used by the overlay to indicate "waiting for OCR" when the pipeline is
    /// still in Transcribing state but STT work has finished.
    stt_complete: bool,

    /// Last captured audio (WAV bytes). Used for debugging/testing.
    last_wav_bytes: Option<Vec<u8>>,

    /// Last recording diagnostics (raw stats + optional speech detection).
    last_recording_diagnostics: Option<AudioCaptureDiagnostics>,
}

struct OcrTaskHandle {
    handle: tokio::task::JoinHandle<Result<crate::ocr::OcrResult, String>>,
}

impl PipelineInner {
    fn cancel_ocr_task(&mut self, mark_cancelled: bool) {
        if let Some(task) = self.ocr_task.take() {
            log::debug!(
                "cancel_ocr_task called: mark_cancelled={}, aborting task",
                mark_cancelled
            );
            task.handle.abort();
        } else {
            log::debug!(
                "cancel_ocr_task called: mark_cancelled={}, no task to abort",
                mark_cancelled
            );
        }
        self.ocr_result = None;
        self.ocr_failed_reason = None;
        self.ocr_cancelled = mark_cancelled;
        self.ocr_awaiting = false;
    }

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

    #[allow(dead_code)]
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
            ocr_session_id: None,
            ocr_task: None,
            ocr_result: None,
            ocr_failed_reason: None,
            ocr_cancelled: false,
            ocr_awaiting: false,
            stt_complete: false,
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
                stt_provider::build_stt_client(&self.config.proxy_settings)?,
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

        let client = stt_provider::build_stt_client(&self.config.proxy_settings)?;

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
        // NOTE: Do NOT reset stt_complete here. OCR may still be running for Quick Ask / Quick Replace,
        // which need the UI to show "OCR..." while waiting. stt_complete is reset when starting a new recording.
        //
        // IMPORTANT (Option A): do not implicitly cancel/clear OCR on reset-to-idle.
        //
        // We may still need OCR for post-transcription flows like Quick Ask / Quick Replace,
        // which run after the pipeline has returned to Idle.
        //
        // OCR should be cancelled/cleared only on explicit session end/cancel (Escape,
        // superseded by new session, force reset).
    }

    /// Transition to error state
    fn set_error(&mut self, msg: &str) {
        log::error!("Pipeline error: {}", msg);
        self.state = PipelineState::Error;
        self.cancel_token = None;
        self.stt_complete = false;
        self.cancel_ocr_task(false);
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

    pub(crate) fn start_ocr_task_if_auto(&self, ocr_config: &OcrConfig, should_run: bool) {
        if !should_run {
            // Best-effort request-log breadcrumb. (Detailed mode reasoning is recorded at callsites.)
            if let Ok(app_guard) = self.app_handle.lock() {
                if let Some(app) = app_guard.as_ref() {
                    if let Some(store) = app.try_state::<RequestLogStore>() {
                        store.with_current(|log| {
                            if log.ocr_status.is_none() {
                                log.ocr_status = Some("not_started".to_string());
                            }
                            if log.ocr_not_attempted_reason.is_none() {
                                log.ocr_not_attempted_reason = Some("not_triggered".to_string());
                            }
                            log.debug("OCR: auto-start skipped".to_string());
                        });
                    }
                }
            }
            return;
        }

        self.start_ocr_task(ocr_config);
    }

    pub(crate) fn start_ocr_task(&self, ocr_config: &OcrConfig) {
        let app_handle = self.app_handle.lock().ok().and_then(|g| g.clone());
        let Some(app_handle) = app_handle else {
            return;
        };

        let log_store = app_handle
            .try_state::<RequestLogStore>()
            .map(|s| s.inner().clone());

        // Best-effort: bind OCR to the current request log id as a stable session id.
        // This makes OCR consumption resilient to internal pipeline transitions.
        let request_id_for_session: Option<String> = log_store
            .as_ref()
            .and_then(|s| s.with_current(|log| log.id.clone()));

        let base_url = ocr_config.base_url.clone();
        let model = ocr_config.model.clone();
        let auth_mode = ocr_config.auth_mode.clone();
        let prompt = ocr_config.prompt.clone();
        let max_tokens = ocr_config.max_tokens;
        let temperature = ocr_config.temperature;
        let top_p = ocr_config.top_p;
        let timeout_ms = ocr_config.request_timeout_ms;
        let context_max_chars = ocr_config.context_max_chars;
        let hallucination_protection = ocr_config.hallucination_protection;
        let hallucination_threshold = ocr_config.hallucination_threshold;
        let resize_max_dimension = ocr_config.resize_max_dimension;
        let resize_filter = ocr_config.resize_filter.clone();

        let base_url_trimmed = base_url.as_deref().unwrap_or("").trim().to_string();
        if base_url_trimmed.is_empty() {
            if let Some(store) = log_store {
                store.with_current(|log| {
                    log.ocr_status = Some("not_started".to_string());
                    log.ocr_not_attempted_reason = Some("provider_unavailable".to_string());
                    log.info("OCR: not started (OCR base URL not set)".to_string());
                });
            }

            emit_overlay_ocr_context_unavailable(
                &app_handle,
                Some("OCR base URL not set".to_string()),
            );
            return;
        }

        if reqwest::Url::parse(&base_url_trimmed).is_err() {
            if let Some(store) = log_store {
                store.with_current(|log| {
                    log.ocr_status = Some("not_started".to_string());
                    log.ocr_not_attempted_reason = Some("invalid_base_url".to_string());
                    log.info("OCR: not started (OCR base URL is invalid)".to_string());
                });
            }

            emit_overlay_ocr_context_unavailable(
                &app_handle,
                Some("OCR base URL is invalid".to_string()),
            );
            return;
        }

        // Validate auth before we capture a screenshot.
        // If we can't call the provider (missing key), we should *not* capture.
        let api_key = if auth_mode == "bearer_api_key" {
            crate::secrets::get_api_key(&app_handle, "ocr_api_key")
        } else {
            None
        };

        if auth_mode == "bearer_api_key"
            && api_key
                .as_deref()
                .map(|s| s.trim())
                .unwrap_or("")
                .is_empty()
        {
            if let Some(store) = log_store {
                store.with_current(|log| {
                    log.ocr_status = Some("not_started".to_string());
                    log.ocr_not_attempted_reason = Some("missing_api_key".to_string());
                    log.info("OCR: not started (OCR API key not set)".to_string());
                });
            }

            emit_overlay_ocr_context_unavailable(
                &app_handle,
                Some("OCR API key not set".to_string()),
            );
            return;
        }

        if let Ok(mut inner) = self.inner.lock() {
            if inner.ocr_session_id.is_none() {
                inner.ocr_session_id = request_id_for_session.clone();
            }

            if inner.ocr_task.is_some() || inner.ocr_result.is_some() {
                return;
            }

            inner.ocr_failed_reason = None;
            inner.ocr_cancelled = false;

            if let Some(store) = log_store.clone() {
                store.with_current(|log| {
                    log.ocr_status = Some("running".to_string());
                    log.ocr_started_at = Some(chrono::Utc::now());
                    log.info("OCR: started".to_string());
                });
            }

            let handle = tokio::spawn(async move {
                let ocr_started_at = chrono::Utc::now();

                #[cfg(target_os = "windows")]
                let capture_target = crate::windows_apps::get_foreground_window_capture_target();

                #[cfg(target_os = "windows")]
                let (capture_hwnd_raw, capture_process_path, capture_target_json) = {
                    let hwnd_raw = capture_target.as_ref().map(|t| t.hwnd_raw);
                    let process_path = capture_target.as_ref().map(|t| t.process_path.clone());
                    let json = capture_target.as_ref().map(|t| {
                        serde_json::json!({
                            "process": crate::app_shared::basename_for_log(&t.process_path),
                            "external_fallback": t.used_external_fallback,
                        })
                    });
                    (hwnd_raw, process_path, json)
                };

                #[cfg(not(target_os = "windows"))]
                let capture_target: Option<()> = None;

                #[cfg(not(target_os = "windows"))]
                let (capture_hwnd_raw, capture_process_path, capture_target_json): (
                    Option<usize>,
                    Option<String>,
                    Option<serde_json::Value>,
                ) = (None, None, None);

                #[cfg(target_os = "windows")]
                if let Some(target) = capture_target.as_ref() {
                    if let Some(store) = log_store.clone() {
                        store.with_current(|log| {
                            log.debug(format!(
                                "OCR: capture target selected (process={}, external_fallback={})",
                                crate::app_shared::basename_for_log(&target.process_path),
                                target.used_external_fallback
                            ));
                        });
                    }
                }

                let capture = match tokio::task::spawn_blocking(move || {
                    #[cfg(target_os = "windows")]
                    {
                        if let (Some(hwnd_raw), Some(process_path)) =
                            (capture_hwnd_raw, capture_process_path)
                        {
                            // Guardrail: if we somehow ended up with our own window, refuse to OCR.
                            // This prevents "OCR reading Kolboo" when focus flips during capture.
                            // (Profile matching already tries to avoid this; capture must too.)
                            // If Kolboo is still the chosen window (very rare), bail out.
                            // We can't perfectly detect this from HWND alone here, but we can cheaply
                            // sanity-check by process basename.
                            let base =
                                crate::app_shared::basename_for_log(&process_path).to_lowercase();
                            if base.contains("kolboo") {
                                return Err(
                                    "Refused OCR capture: target window appears to be Kolboo"
                                        .to_string(),
                                );
                            }

                            return crate::active_window_capture::capture_window_png(
                                windows::Win32::Foundation::HWND(
                                    hwnd_raw as *mut core::ffi::c_void,
                                ),
                                resize_max_dimension,
                                &resize_filter,
                            );
                        }

                        crate::active_window_capture::capture_active_window_png(
                            resize_max_dimension,
                            &resize_filter,
                        )
                    }

                    #[cfg(not(target_os = "windows"))]
                    {
                        crate::active_window_capture::capture_active_window_png(
                            resize_max_dimension,
                            &resize_filter,
                        )
                    }
                })
                .await
                {
                    Ok(Ok(capture)) => capture,
                    Ok(Err(err)) => {
                        if let Some(store) = log_store.clone() {
                            store.with_current(|log| {
                                log.ocr_status = Some("failed".to_string());
                                log.ocr_failed_reason = Some(err.clone());
                                let duration_ms = (chrono::Utc::now() - ocr_started_at)
                                    .num_milliseconds()
                                    .max(0)
                                    as u64;
                                log.ocr_duration_ms = Some(duration_ms);
                                log.warn("OCR: capture failed".to_string());
                            });
                        }

                        // If the capture failed because we refused to OCR Kolboo, emit a friendly hint.
                        if err.to_lowercase().contains("refused ocr capture") {
                            emit_overlay_ocr_context_unavailable(
                                &app_handle,
                                Some("OCR can’t run while Kolboo is focused. Switch back to the target app and try again.".to_string()),
                            );
                        }
                        return Err(err);
                    }
                    Err(join_err) => {
                        let err = format!("OCR capture failed: {}", join_err);
                        if let Some(store) = log_store.clone() {
                            store.with_current(|log| {
                                log.ocr_status = Some("failed".to_string());
                                log.ocr_failed_reason = Some(err.clone());
                                let duration_ms = (chrono::Utc::now() - ocr_started_at)
                                    .num_milliseconds()
                                    .max(0)
                                    as u64;
                                log.ocr_duration_ms = Some(duration_ms);
                                log.warn("OCR: capture join failed".to_string());
                            });
                        }
                        return Err(err);
                    }
                };

                if let Some(store) = log_store.clone() {
                    store.with_current(|log| {
                        log.debug(format!(
                            "OCR: captured active window ({}x{}, png_bytes={})",
                            capture.image_width_px,
                            capture.image_height_px,
                            capture.image_png_bytes.len()
                        ));
                    });
                }

                // OCR hallucination protection: validate image quality before sending to API.
                // This catches uniform-color images that would cause the model to hallucinate.
                if hallucination_protection {
                    // Decode the PNG to validate its contents.
                    let validation_result = match image::load_from_memory(&capture.image_png_bytes)
                    {
                        Ok(img) => {
                            let rgba = img.to_rgba8();
                            crate::active_window_capture::validate_image_for_ocr(
                                rgba.as_raw(),
                                hallucination_threshold,
                            )
                        }
                        Err(e) => {
                            log::warn!(
                                "OCR: Failed to decode captured image for validation: {}",
                                e
                            );
                            // If we can't decode it, skip validation and let the API handle it.
                            crate::active_window_capture::ImageValidationResult {
                                validation: crate::active_window_capture::ImageValidation::Valid,
                                variance: 0,
                                threshold: hallucination_threshold,
                                mean_rgb: (0, 0, 0),
                            }
                        }
                    };

                    // Always log the validation metrics so users can see them.
                    if let Some(store) = log_store.clone() {
                        let vr = &validation_result;
                        store.with_current(|log| {
                            log.debug(format!(
                                "OCR: hallucination check (variance={}, threshold={}, mean_rgb=({},{},{}))",
                                vr.variance, vr.threshold, vr.mean_rgb.0, vr.mean_rgb.1, vr.mean_rgb.2
                            ));
                        });
                    }

                    if !validation_result.validation.is_valid() {
                        let reason = validation_result
                            .validation
                            .reason()
                            .unwrap_or_else(|| "unknown".to_string());
                        if let Some(store) = log_store.clone() {
                            store.with_current(|log| {
                                log.ocr_status = Some("skipped".to_string());
                                log.ocr_failed_reason = Some(reason.clone());
                                let duration_ms = (chrono::Utc::now() - ocr_started_at)
                                    .num_milliseconds()
                                    .max(0) as u64;
                                log.ocr_duration_ms = Some(duration_ms);
                                log.warn(format!(
                                    "OCR: skipped due to hallucination protection ({}, variance={} < threshold={})",
                                    reason, validation_result.variance, validation_result.threshold
                                ));
                            });
                        }
                        emit_overlay_ocr_context_unavailable(
                            &app_handle,
                            Some(format!("OCR skipped: {}", reason)),
                        );
                        return Err(format!("OCR skipped: {}", reason));
                    }
                }
                if let Some(store) = log_store.clone() {
                    store.with_current(|log| {
                        let base_url_for_log = base_url_trimmed.clone();
                        let model_for_log = model.clone();
                        let auth_mode_for_log = auth_mode.clone();
                        let prompt_for_log = prompt.clone();

                        log.ocr_request_json = Some(serde_json::json!({
                            "provider": "openai_compatible",
                            "base_url": base_url_for_log,
                            "endpoint": "/v1/chat/completions",
                            "model": model_for_log,
                            "auth_mode": auth_mode_for_log,
                            "timeout_ms": timeout_ms,
                            "prompt": prompt_for_log,
                            "capture_target": capture_target_json,
                            "max_tokens": max_tokens,
                            "temperature": temperature,
                            "top_p": top_p,
                            "image": {
                                "format": "png",
                                "bytes": capture.image_png_bytes.len(),
                                "width_px": capture.image_width_px,
                                "height_px": capture.image_height_px,
                            },
                        }));
                    });
                }

                let ocr_result = crate::ocr::openai_compatible::request_ocr_text(
                    crate::ocr::openai_compatible::OcrRequestParams {
                        base_url: base_url_trimmed.as_str(),
                        model: model.as_str(),
                        image_png: &capture.image_png_bytes,
                        api_key: api_key.as_deref(),
                        timeout_ms,
                        prompt: prompt.as_str(),
                        max_tokens,
                        temperature,
                        top_p,
                    },
                )
                .await;

                let (result, response_json) = match ocr_result {
                    Ok(ok) => ok,
                    Err(err) => {
                        if let Some(store) = log_store.clone() {
                            store.with_current(|log| {
                                log.ocr_status = Some("failed".to_string());
                                log.ocr_failed_reason = Some(err.clone());
                                log.ocr_response_json = Some(serde_json::json!({
                                    "ok": false,
                                    "error": err,
                                }));
                                let duration_ms = (chrono::Utc::now() - ocr_started_at)
                                    .num_milliseconds()
                                    .max(0)
                                    as u64;
                                log.ocr_duration_ms = Some(duration_ms);
                                log.warn("OCR: failed".to_string());
                            });
                        }
                        return Err(err);
                    }
                };

                if let Some(store) = log_store.clone() {
                    store.with_current(|log| {
                        log.ocr_response_json = Some(response_json);
                        let duration_ms = (chrono::Utc::now() - ocr_started_at)
                            .num_milliseconds()
                            .max(0) as u64;
                        log.ocr_duration_ms = Some(duration_ms);
                        log.debug(format!(
                            "OCR: response received ({} chars)",
                            result.text.chars().count()
                        ));

                        // Warn if OCR result looks like the model echoed the system prompt
                        // (common when using a non-vision model for OCR)
                        let lower = result.text.to_lowercase();
                        if lower.contains("ocr") && (lower.contains("engine") || lower.contains("extract")) {
                            log.warn("OCR: response looks like prompt echo (model may not support vision)".to_string());
                        }
                    });
                }

                let (text, _truncated) =
                    crate::ocr::truncate_ocr_text(&result.text, context_max_chars);

                Ok(crate::ocr::OcrResult {
                    text,
                    provider: result.provider,
                    model: result.model,
                })
            });

            inner.ocr_task = Some(OcrTaskHandle { handle });
        }
    }

    pub(crate) fn cancel_ocr_task(&self) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.cancel_ocr_task(true);
        }

        if let Ok(app_guard) = self.app_handle.lock() {
            if let Some(app) = app_guard.as_ref() {
                if let Some(store) = app.try_state::<RequestLogStore>() {
                    store.with_current(|log| {
                        log.ocr_status = Some("cancelled".to_string());
                        log.info("OCR: cancelled".to_string());
                    });
                }
            }
        }
    }

    pub(crate) async fn finalize_ocr_task_if_finished(&self) {
        let handle = {
            let mut inner = match self.inner.lock() {
                Ok(g) => g,
                Err(_) => return,
            };

            let Some(task) = inner.ocr_task.as_ref() else {
                return;
            };

            if !task.handle.is_finished() {
                return;
            }

            log::debug!("finalize_ocr_task_if_finished: task finished, taking handle");
            // Task is finished: take ownership so we can await and store the outcome.
            inner.ocr_task.take().map(|t| t.handle)
        };

        let Some(handle) = handle else {
            return;
        };

        match handle.await {
            Ok(Ok(result)) => {
                if let Ok(mut inner) = self.inner.lock() {
                    inner.ocr_result = Some(result);
                    inner.ocr_failed_reason = None;
                    inner.ocr_cancelled = false;
                }

                if let Ok(app_guard) = self.app_handle.lock() {
                    if let Some(app) = app_guard.as_ref() {
                        if let Some(store) = app.try_state::<RequestLogStore>() {
                            store.with_current(|log| {
                                log.ocr_status = Some("done".to_string());
                                log.info("OCR: done".to_string());
                            });
                        }
                    }
                }
            }
            Ok(Err(err)) => {
                let reason = err.clone();
                if let Ok(mut inner) = self.inner.lock() {
                    inner.ocr_failed_reason = Some(reason.clone());
                    inner.ocr_result = None;
                    inner.ocr_cancelled = false;
                }

                if let Ok(app_guard) = self.app_handle.lock() {
                    if let Some(app) = app_guard.as_ref() {
                        emit_overlay_ocr_context_unavailable(app, Some(reason.clone()));
                        if let Some(store) = app.try_state::<RequestLogStore>() {
                            store.with_current(|log| {
                                log.ocr_status = Some("failed".to_string());
                                log.ocr_failed_reason = Some(reason);
                                log.warn("OCR: failed".to_string());
                            });
                        }
                    }
                }
            }
            Err(join_err) => {
                // Aborted/cancelled tasks surface as a JoinError.
                if let Ok(mut inner) = self.inner.lock() {
                    inner.ocr_result = None;
                    inner.ocr_failed_reason = Some(join_err.to_string());
                    inner.ocr_cancelled = join_err.is_cancelled();
                }

                if let Ok(app_guard) = self.app_handle.lock() {
                    if let Some(app) = app_guard.as_ref() {
                        if !join_err.is_cancelled() {
                            emit_overlay_ocr_context_unavailable(app, Some(join_err.to_string()));
                        }
                        if let Some(store) = app.try_state::<RequestLogStore>() {
                            let cancelled = join_err.is_cancelled();
                            let reason = join_err.to_string();
                            store.with_current(|log| {
                                log.ocr_status = Some(
                                    if cancelled { "cancelled" } else { "failed" }.to_string(),
                                );
                                log.ocr_failed_reason = Some(reason);
                                log.warn("OCR: task aborted".to_string());
                            });
                        }
                    }
                }
            }
        }
    }

    pub(crate) fn get_ocr_status(&self) -> String {
        let inner = match self.inner.lock() {
            Ok(g) => g,
            Err(_) => return "failed".to_string(),
        };

        if inner.ocr_cancelled {
            return "cancelled".to_string();
        }

        if inner.ocr_result.is_some() {
            return "done".to_string();
        }

        if inner.ocr_task.is_some() || inner.ocr_awaiting {
            return "running".to_string();
        }

        if inner.ocr_failed_reason.is_some() {
            return "failed".to_string();
        }

        "not_started".to_string()
    }

    /// Returns whether the STT portion has completed for the current session.
    ///
    /// Used by the overlay to distinguish "transcribing (STT in progress)" from
    /// "transcribing (waiting for OCR)" when `ocr_status == "running"`.
    pub(crate) fn is_stt_complete(&self) -> bool {
        self.inner.lock().map(|g| g.stt_complete).unwrap_or(false)
    }

    pub(crate) fn peek_session_profile_override(&self) -> Option<String> {
        self.session_profile_override
            .lock()
            .ok()
            .and_then(|g| g.clone())
    }

    pub(crate) async fn get_ocr_result_with_timeout(
        &self,
        timeout: Duration,
    ) -> Option<crate::ocr::OcrResult> {
        // IMPORTANT: Do not permanently take/drop the OCR task handle when timing out.
        // If we drop the JoinHandle on timeout, the OCR task will keep running in the background
        // (and may even log "response received"), but the pipeline can no longer consume/store
        // the result.
        let mut handle = {
            let mut inner = self.inner.lock().ok()?;
            if let Some(result) = inner.ocr_result.as_ref() {
                return Some(result.clone());
            }
            // Mark that we're awaiting the OCR result so get_ocr_status() still returns "running".
            inner.ocr_awaiting = true;
            inner.ocr_task.take()?.handle
        };

        let res = tokio::select! {
            r = &mut handle => r,
            _ = tokio::time::sleep(timeout) => {
                // Put the handle back so future callers (or overlay polling) can still consume it.
                if let Ok(mut inner) = self.inner.lock() {
                    // Only restore if we didn't end up with a result while waiting.
                    if inner.ocr_result.is_none() {
                        inner.ocr_task = Some(OcrTaskHandle { handle });
                    }
                    inner.ocr_awaiting = false;
                }

                if let Ok(app_guard) = self.app_handle.lock() {
                    if let Some(app) = app_guard.as_ref() {
                        if let Some(store) = app.try_state::<RequestLogStore>() {
                            store.with_current(|log| {
                                // Keep status as running; this is "not ready in time", not a failure.
                                if log.ocr_status.is_none() {
                                    log.ocr_status = Some("running".to_string());
                                }
                                log.info(format!(
                                    "OCR: still running (not ready before timeout {}ms)",
                                    timeout.as_millis()
                                ));
                            });
                        }
                    }
                }

                return None;
            }
        };

        match res {
            Ok(Ok(result)) => {
                if let Ok(mut inner) = self.inner.lock() {
                    inner.ocr_result = Some(result.clone());
                    inner.ocr_failed_reason = None;
                    inner.ocr_cancelled = false;
                    inner.ocr_awaiting = false;
                }

                if let Ok(app_guard) = self.app_handle.lock() {
                    if let Some(app) = app_guard.as_ref() {
                        if let Some(store) = app.try_state::<RequestLogStore>() {
                            store.with_current(|log| {
                                log.ocr_status = Some("done".to_string());
                                log.info("OCR: done".to_string());
                            });
                        }
                    }
                }
                Some(result)
            }
            Ok(Err(err)) => {
                if let Ok(mut inner) = self.inner.lock() {
                    inner.ocr_failed_reason = Some(err.clone());
                    inner.ocr_cancelled = false;
                    inner.ocr_awaiting = false;
                }

                if let Ok(app_guard) = self.app_handle.lock() {
                    if let Some(app) = app_guard.as_ref() {
                        emit_overlay_ocr_context_unavailable(app, Some(err.clone()));
                        if let Some(store) = app.try_state::<RequestLogStore>() {
                            store.with_current(|log| {
                                log.ocr_status = Some("failed".to_string());
                                log.ocr_failed_reason = Some(err.clone());
                                log.warn("OCR: failed".to_string());
                            });
                        }
                    }
                }
                None
            }
            Err(err) => {
                if let Ok(mut inner) = self.inner.lock() {
                    inner.ocr_failed_reason = Some(err.to_string());
                    inner.ocr_cancelled = err.is_cancelled();
                    inner.ocr_awaiting = false;
                }

                if let Ok(app_guard) = self.app_handle.lock() {
                    if let Some(app) = app_guard.as_ref() {
                        if !err.is_cancelled() {
                            emit_overlay_ocr_context_unavailable(app, Some(err.to_string()));
                        }
                        if let Some(store) = app.try_state::<RequestLogStore>() {
                            store.with_current(|log| {
                                log.ocr_status = Some(
                                    if err.is_cancelled() {
                                        "cancelled"
                                    } else {
                                        "failed"
                                    }
                                    .to_string(),
                                );
                                log.ocr_failed_reason = Some(err.to_string());
                                log.warn("OCR: task aborted".to_string());
                            });
                        }
                    }
                }
                None
            }
        }
    }

    pub(crate) fn get_ocr_failed_reason(&self) -> Option<String> {
        self.inner
            .lock()
            .ok()
            .and_then(|inner| inner.ocr_failed_reason.clone())
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

    /// Begin (or switch) the OCR session associated with the current user request.
    ///
    /// We use the current Request Log id as a stable session identifier so OCR can remain
    /// consumable even if the pipeline returns to Idle while post-processing continues
    /// (e.g., Quick Ask / Quick Replace).
    pub fn begin_ocr_session(&self, session_id: String) {
        let mut inner = match self.inner.lock() {
            Ok(g) => g,
            Err(_) => return,
        };

        // If we're already on this session id, do nothing.
        if inner.ocr_session_id.as_deref() == Some(session_id.as_str()) {
            log::debug!(
                "begin_ocr_session: already on session {}, skipping",
                session_id
            );
            return;
        }

        // Supersede any previous session.
        if inner.ocr_task.is_some() || inner.ocr_result.is_some() {
            log::debug!(
                "begin_ocr_session: superseding previous session {:?} with {}",
                inner.ocr_session_id,
                session_id
            );
            inner.cancel_ocr_task(true);
        } else {
            log::debug!(
                "begin_ocr_session: starting new session {} (no previous)",
                session_id
            );
        }

        inner.ocr_session_id = Some(session_id);
        inner.ocr_cancelled = false;
        inner.ocr_failed_reason = None;
        inner.ocr_result = None;
        inner.ocr_task = None;
        inner.ocr_awaiting = false;
    }

    /// End the OCR session if it matches the provided session id.
    ///
    /// This should be called once all flows that might consume OCR (Quick Ask answer,
    /// Quick Replace extra LLM step, etc.) have completed.
    pub fn end_ocr_session_if_matches(&self, session_id: &str) {
        let mut inner = match self.inner.lock() {
            Ok(g) => g,
            Err(_) => return,
        };

        if inner.ocr_session_id.as_deref() != Some(session_id) {
            log::debug!(
                "end_ocr_session_if_matches: session_id={} does not match current {:?}",
                session_id,
                inner.ocr_session_id
            );
            return;
        }

        log::debug!(
            "end_ocr_session_if_matches: session_id={} matches, clearing OCR",
            session_id
        );
        inner.cancel_ocr_task(false);
        inner.ocr_session_id = None;
    }

    /// Read the current OCR session id (if any).
    pub fn ocr_session_id(&self) -> Option<String> {
        self.inner
            .lock()
            .ok()
            .and_then(|g| g.ocr_session_id.clone())
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
        if !recording::can_start_recording(inner.state) {
            return Err(PipelineError::AlreadyRecording);
        }

        // Create a new cancellation token for this session
        let cancel_token = CancellationToken::new();
        inner.cancel_token = Some(cancel_token);

        let max_duration = inner.config.max_duration_secs;
        // Clone out of the config to avoid borrowing `inner` immutably while calling into
        // `audio_capture` mutably.
        let input_device_name = inner.config.input_device_name.clone();
        match recording::start_recording_session(
            inner.audio_capture.as_mut(),
            max_duration,
            input_device_name.as_deref(),
        ) {
            Ok(()) => {
                inner.stt_complete = false;
                log::debug!("stt_complete reset to false (start_recording)");
                inner.transition_to(PipelineState::Recording, "start_recording");
                log::info!("Pipeline: Recording started");
                Ok(())
            }
            Err(e) => {
                inner.set_error(&format!("Failed to start recording: {}", e));
                Err(e)
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

        if !recording::can_stop_recording(inner.state) {
            return Err(PipelineError::NotRecording);
        }

        let encode_cfg = inner.config.audio_encode_config();
        match recording::stop_recording_session(inner.audio_capture.as_mut(), encode_cfg) {
            Ok(outcome) => {
                inner.last_recording_diagnostics = Some(outcome.diagnostics);

                // Check size limit
                let max_bytes = inner.config.max_recording_bytes;
                if let Err(e) =
                    recording::validate_recording_size(outcome.wav_bytes.len(), max_bytes)
                {
                    inner.set_error(&format!(
                        "Recording too large: {} bytes",
                        outcome.wav_bytes.len()
                    ));
                    return Err(e);
                }

                // Keep a copy for STT testing/debugging UI.
                inner.last_wav_bytes = Some(outcome.wav_bytes.clone());

                inner.reset_to_idle();
                log::info!(
                    "Pipeline: Recording stopped, {} bytes captured",
                    outcome.wav_bytes.len()
                );
                Ok(outcome.wav_bytes)
            }
            Err(e) => {
                inner.set_error(&format!("Failed to stop recording: {}", e));
                Err(e)
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

        if !recording::can_stop_recording(inner.state) {
            return Err(PipelineError::NotRecording);
        }

        let encode_cfg = inner.config.audio_encode_config();
        match recording::stop_recording_before_after(inner.audio_capture.as_mut(), encode_cfg) {
            Ok(outcome) => {
                inner.last_recording_diagnostics = Some(outcome.diagnostics);

                // Check size limit (both, to avoid surprising huge payloads)
                let max_bytes = inner.config.max_recording_bytes;
                if let Err(e) =
                    recording::validate_recording_size(outcome.before_wav.len(), max_bytes)
                {
                    inner.set_error(&format!(
                        "Recording too large: {} bytes",
                        outcome.before_wav.len()
                    ));
                    return Err(e);
                }
                if let Err(e) =
                    recording::validate_recording_size(outcome.after_wav.len(), max_bytes)
                {
                    inner.set_error(&format!(
                        "Recording too large: {} bytes",
                        outcome.after_wav.len()
                    ));
                    return Err(e);
                }

                // Keep a copy of the processed output for STT test + debugging.
                inner.last_wav_bytes = Some(outcome.after_wav.clone());

                inner.reset_to_idle();
                Ok((outcome.before_wav, outcome.after_wav))
            }
            Err(e) => {
                inner.set_error(&format!("Failed to stop recording: {}", e));
                Err(e)
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
            ocr_config,
            rewrite_ocr_mode,
        ) = {
            let mut inner = self
                .inner
                .lock()
                .map_err(|e| PipelineError::Lock(e.to_string()))?;

            if !recording::can_stop_recording(inner.state) {
                return Err(PipelineError::NotRecording);
            }

            let encode_cfg = inner.config.audio_encode_config();
            let outcome =
                match recording::stop_recording_session(inner.audio_capture.as_mut(), encode_cfg) {
                    Ok(out) => out,
                    Err(e) => {
                        inner.set_error(&format!("Failed to stop recording: {}", e));
                        return Err(e);
                    }
                };

            // Persist diagnostics for UI readout.
            inner.last_recording_diagnostics = Some(outcome.diagnostics);

            // Check size limit before cloning/storing debug audio.
            let max_bytes = inner.config.max_recording_bytes;
            if let Err(e) = recording::validate_recording_size(outcome.wav_bytes.len(), max_bytes) {
                inner.set_error(&format!(
                    "Recording too large: {} bytes",
                    outcome.wav_bytes.len()
                ));
                return Err(e);
            }

            // Keep a copy for STT testing/debugging UI.
            inner.last_wav_bytes = Some(outcome.wav_bytes.clone());

            // Evaluate quiet audio gate (VAD-based speech detection + amplitude thresholds).
            let gate_config = recording::QuietAudioGateConfig {
                enabled: inner.config.quiet_audio_gate_enabled,
                require_speech: inner.config.quiet_audio_require_speech,
                min_duration_secs: inner.config.quiet_audio_min_duration_secs,
                rms_dbfs_threshold: inner.config.quiet_audio_rms_dbfs_threshold,
                peak_dbfs_threshold: inner.config.quiet_audio_peak_dbfs_threshold,
            };
            match recording::evaluate_quiet_audio_gate(&outcome.diagnostics, gate_config) {
                recording::QuietAudioGateResult::NoSpeechDetected => {
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
                recording::QuietAudioGateResult::Quiet => {
                    inner.reset_to_idle();
                    return Ok(TranscriptionResult {
                        stt_text: String::new(),
                        final_text: String::new(),
                        stt_duration_ms: 0,
                        llm_duration_ms: None,
                        llm_provider_used: None,
                        llm_model_used: None,
                        llm_outcome: LlmOutcome::NotAttempted(
                            LlmNotAttemptedReason::QuietAudioGate,
                        ),
                    });
                }
                recording::QuietAudioGateResult::NotQuiet => {}
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

            let default_profile = select_default_profile(&llm_config);

            let default_rewrite_include_clipboard_context = default_profile
                .as_ref()
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

            let rewrite_ocr_mode = resolve_rewrite_active_window_ocr_mode(
                active_profile.as_ref(),
                default_profile.as_ref(),
                inner.config.ocr_config.rewrite_mode.as_str(),
            )
            .to_string();

            (
                outcome.wav_bytes,
                stt_provider,
                retry_config,
                desired_timeout,
                cancel_token,
                active_profile,
                default_rewrite_include_clipboard_context,
                inner.config.ocr_config.clone(),
                rewrite_ocr_mode,
            )
        };

        self.start_ocr_task_if_auto(&ocr_config, rewrite_ocr_mode == "auto");

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
            Ok(result) => {
                // STT portion is done. Mark this so the overlay can show "waiting for OCR"
                // instead of "transcribing" if OCR is still running.
                if let Ok(mut inner) = self.inner.lock() {
                    inner.stt_complete = true;
                    log::debug!("stt_complete set to true (stop_and_transcribe_detailed)");
                }
                (result.text, result.duration_ms)
            }
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

        // OCR modes:
        // - auto: start + wait
        // - manual: user may have triggered OCR via overlay; if so, wait for it here
        // - off: never wait
        let ocr_result = if rewrite_ocr_mode != "off" {
            self.get_ocr_result_with_timeout(Duration::from_millis(ocr_config.request_timeout_ms))
                .await
        } else {
            None
        };
        let ocr_text = ocr_result.as_ref().map(|r| r.text.clone());

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
            active_window_ocr_text: ocr_text,
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
            ocr_config,
            rewrite_ocr_mode,
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

            let default_profile = select_default_profile(&llm_config);

            let default_rewrite_include_clipboard_context = default_profile
                .as_ref()
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

            let rewrite_ocr_mode = resolve_rewrite_active_window_ocr_mode(
                active_profile.as_ref(),
                default_profile.as_ref(),
                inner.config.ocr_config.rewrite_mode.as_str(),
            )
            .to_string();

            (
                stt_provider,
                retry_config,
                desired_timeout,
                cancel_token,
                active_profile,
                default_rewrite_include_clipboard_context,
                inner.config.ocr_config.clone(),
                rewrite_ocr_mode,
            )
        };

        self.start_ocr_task_if_auto(&ocr_config, rewrite_ocr_mode == "auto");

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
            Ok(result) => {
                // STT portion is done. Mark this so the overlay can show "waiting for OCR"
                // instead of "transcribing" if OCR is still running.
                if let Ok(mut inner) = self.inner.lock() {
                    inner.stt_complete = true;
                    log::debug!("stt_complete set to true (retry_transcription)");
                }
                (result.text, result.duration_ms)
            }
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

        let ocr_result = if rewrite_ocr_mode != "off" {
            self.get_ocr_result_with_timeout(Duration::from_millis(ocr_config.request_timeout_ms))
                .await
        } else {
            None
        };
        let ocr_text = ocr_result.as_ref().map(|r| r.text.clone());

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
            active_window_ocr_text: ocr_text,
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

        let ocr_enabled = |ocr: &OcrConfig| {
            let base_url_ok = ocr
                .base_url
                .as_deref()
                .map(|s| !s.trim().is_empty())
                .unwrap_or(false);
            if !base_url_ok {
                return false;
            }
            ocr.rewrite_mode != "off"
                || ocr.quick_replace_mode != "off"
                || ocr.quick_ask_mode != "off"
        };

        let old_ocr_enabled = ocr_enabled(&inner.config.ocr_config);
        let new_ocr_enabled = ocr_enabled(&config.ocr_config);

        // If the local-whisper model path changed, evict cached models.
        // Otherwise switching models could keep multiple large GGML files resident.
        let old_local_whisper_key = inner.local_whisper_model_key_for_cache();
        let old_stt_prompt = inner.config.stt_transcription_prompt.clone();

        // Don't update config while recording - could cause issues
        if inner.state == PipelineState::Recording {
            log::warn!("Pipeline: Config update requested while recording, will take effect after current session");
        }

        inner.config = config.clone();

        // If OCR was enabled and is now disabled (URL cleared or modes all off), cancel any
        // in-flight OCR work for the current session.
        if old_ocr_enabled && !new_ocr_enabled {
            inner.cancel_ocr_task(true);
        }

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

        let enabled_profile_ids: HashSet<String> = inner
            .config
            .llm_config
            .program_prompt_profiles
            .iter()
            .map(|p| p.id.clone())
            .collect();

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

        // If the active override/lock refers to a disabled (filtered) profile, clear it.
        if let Ok(mut override_guard) = self.session_profile_override.lock() {
            if override_guard
                .as_deref()
                .map(|id| !enabled_profile_ids.contains(id))
                .unwrap_or(false)
            {
                *override_guard = None;
            }
        }

        if let Ok(mut lock_guard) = self.session_preset_lock.lock() {
            if lock_guard
                .as_ref()
                .and_then(|lock| lock.profile_id.as_deref())
                .map(|id| !enabled_profile_ids.contains(id))
                .unwrap_or(false)
            {
                *lock_guard = None;
            }
        }
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

            inner.ocr_task = None;
            inner.ocr_result = None;
            inner.ocr_failed_reason = None;
            inner.ocr_awaiting = false;

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
