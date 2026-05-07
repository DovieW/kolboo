//! Shared command-facing request initialization helpers.
//!
//! `RequestLogStore::start_request_with(...)` remains the atomic primitive for creating a new
//! current request log. This Module owns the repeated metadata shaping around that primitive for
//! recording command flows: request-log seed fields, in-progress History payloads, and tracing
//! request-id stamping. It intentionally does **not** own terminal completion, retention, or
//! platform output behavior.

use crate::history::{RequestHistoryUpdate, RequestModelInfo};
use crate::pipeline::PipelineConfig;
use crate::request_log::{RequestLog, RequestLogStore};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HistorySelectionMode {
    /// Preserve the caller-supplied profile/preset selection in the initial History row.
    PreserveSeededSelection,
    /// Omit profile/preset chips until later request-log mirroring resolves the effective values.
    OmitSeededSelection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LogLlmSeedMode {
    /// Seed configured LLM provider/model onto the request log immediately.
    PreserveConfigured,
    /// Leave LLM provider/model unset until later flow stages stamp the effective values.
    OmitConfigured,
    /// Keep any existing LLM provider/model values already present on the request log.
    LeaveExisting,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct RecordingRequestSeed {
    pub(crate) stt_provider: String,
    pub(crate) stt_model: Option<String>,
    pub(crate) llm_provider: Option<String>,
    pub(crate) llm_model: Option<String>,
    pub(crate) profile_id: Option<String>,
    pub(crate) profile_name: Option<String>,
    pub(crate) preset_id: Option<String>,
    pub(crate) preset_name: Option<String>,
}

impl RecordingRequestSeed {
    pub(crate) fn new(stt_provider: String, stt_model: Option<String>) -> Self {
        Self {
            stt_provider,
            stt_model,
            ..Self::default()
        }
    }

    pub(crate) fn from_config(config: &PipelineConfig) -> Self {
        let mut seed = Self::new(config.stt_provider.clone(), config.stt_model.clone());
        if config.llm_config.enabled {
            seed.llm_provider = Some(config.llm_config.provider.clone());
            seed.llm_model = config.llm_config.model.clone();
        }
        seed
    }

    pub(crate) fn with_profile(
        mut self,
        profile_id: Option<String>,
        profile_name: Option<String>,
    ) -> Self {
        self.profile_id = profile_id;
        self.profile_name = profile_name;
        self
    }

    pub(crate) fn with_preset(
        mut self,
        preset_id: Option<String>,
        preset_name: Option<String>,
    ) -> Self {
        self.preset_id = preset_id;
        self.preset_name = preset_name;
        self
    }

    pub(crate) fn seed_log(&self, log: &mut RequestLog, llm_mode: LogLlmSeedMode) {
        log.profile_id = self.profile_id.clone();
        log.profile_name = self.profile_name.clone();
        log.preset_id = self.preset_id.clone();
        log.preset_name = self.preset_name.clone();

        match llm_mode {
            LogLlmSeedMode::PreserveConfigured => {
                log.llm_provider = self.llm_provider.clone();
                log.llm_model = self.llm_model.clone();
            }
            LogLlmSeedMode::OmitConfigured => {
                log.llm_provider = None;
                log.llm_model = None;
            }
            LogLlmSeedMode::LeaveExisting => {}
        }
    }

    pub(crate) fn to_history_model_info(
        &self,
        selection_mode: HistorySelectionMode,
    ) -> RequestModelInfo {
        let (profile_id, profile_name, preset_id, preset_name) = match selection_mode {
            HistorySelectionMode::PreserveSeededSelection => (
                self.profile_id.clone(),
                self.profile_name.clone(),
                self.preset_id.clone(),
                self.preset_name.clone(),
            ),
            HistorySelectionMode::OmitSeededSelection => (None, None, None, None),
        };

        RequestModelInfo {
            stt_provider: Some(self.stt_provider.clone()),
            stt_model: self.stt_model.clone(),
            llm_provider: self.llm_provider.clone(),
            llm_model: self.llm_model.clone(),
            profile_id,
            profile_name,
            preset_id,
            preset_name,
        }
    }
}

pub(crate) fn start_request_log_with_seed<F>(
    log_store: &RequestLogStore,
    seed: &RecordingRequestSeed,
    llm_mode: LogLlmSeedMode,
    after_seed: F,
) -> String
where
    F: FnOnce(&mut RequestLog),
{
    log_store.start_request_with(seed.stt_provider.clone(), seed.stt_model.clone(), |log| {
        seed.seed_log(log, llm_mode);
        after_seed(log);
    })
}

pub(crate) fn create_in_progress_history_update(
    request_id: &str,
    seed: &RecordingRequestSeed,
    selection_mode: HistorySelectionMode,
    max_entries: Option<usize>,
) -> RequestHistoryUpdate {
    RequestHistoryUpdate::CreateInProgress {
        request_id: request_id.to_string(),
        model_info: seed.to_history_model_info(selection_mode),
        max_entries,
    }
}

pub(crate) fn record_request_id_on_current_span(request_id: Option<&str>) {
    if let Some(request_id) = request_id {
        tracing::Span::current().record("request_id", tracing::field::display(request_id));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::RequestHistoryUpdate;
    use crate::pipeline::PipelineConfig;

    #[test]
    fn request_seed_from_config_only_keeps_llm_when_enabled() {
        let mut config = PipelineConfig {
            stt_provider: "openai".to_string(),
            stt_model: Some("whisper-1".to_string()),
            ..PipelineConfig::default()
        };
        config.llm_config.provider = "anthropic".to_string();
        config.llm_config.model = Some("claude-3-7-sonnet".to_string());

        config.llm_config.enabled = false;
        let disabled = RecordingRequestSeed::from_config(&config);
        assert_eq!(disabled.stt_provider, "openai");
        assert_eq!(disabled.stt_model.as_deref(), Some("whisper-1"));
        assert_eq!(disabled.llm_provider, None);
        assert_eq!(disabled.llm_model, None);

        config.llm_config.enabled = true;
        let enabled = RecordingRequestSeed::from_config(&config);
        assert_eq!(enabled.llm_provider.as_deref(), Some("anthropic"));
        assert_eq!(enabled.llm_model.as_deref(), Some("claude-3-7-sonnet"));
    }

    #[test]
    fn history_update_can_omit_seeded_profile_and_preset() {
        let seed = RecordingRequestSeed {
            llm_provider: Some("anthropic".to_string()),
            llm_model: Some("claude-3-7-sonnet".to_string()),
            ..RecordingRequestSeed::new("openai".to_string(), Some("whisper-1".to_string()))
        }
        .with_profile(
            Some("profile-1".to_string()),
            Some("Profile One".to_string()),
        )
        .with_preset(Some("preset-1".to_string()), Some("Preset One".to_string()));

        let update = create_in_progress_history_update(
            "req-1",
            &seed,
            HistorySelectionMode::OmitSeededSelection,
            Some(25),
        );

        match update {
            RequestHistoryUpdate::CreateInProgress {
                request_id,
                model_info,
                max_entries,
            } => {
                assert_eq!(request_id, "req-1");
                assert_eq!(max_entries, Some(25));
                assert_eq!(model_info.stt_provider.as_deref(), Some("openai"));
                assert_eq!(model_info.stt_model.as_deref(), Some("whisper-1"));
                assert_eq!(model_info.llm_provider.as_deref(), Some("anthropic"));
                assert_eq!(model_info.llm_model.as_deref(), Some("claude-3-7-sonnet"));
                assert_eq!(model_info.profile_id, None);
                assert_eq!(model_info.profile_name, None);
                assert_eq!(model_info.preset_id, None);
                assert_eq!(model_info.preset_name, None);
            }
            other => panic!("unexpected history update: {other:?}"),
        }
    }

    #[test]
    fn start_request_log_with_seed_keeps_atomic_seeding_behavior() {
        let store = RequestLogStore::new();
        let seed = RecordingRequestSeed {
            llm_provider: Some("anthropic".to_string()),
            llm_model: Some("claude-3-7-sonnet".to_string()),
            ..RecordingRequestSeed::new("openai".to_string(), Some("whisper-1".to_string()))
        }
        .with_profile(Some("default".to_string()), Some("Default".to_string()))
        .with_preset(Some("preset-1".to_string()), Some("Preset One".to_string()));

        let id =
            start_request_log_with_seed(&store, &seed, LogLlmSeedMode::PreserveConfigured, |log| {
                log.info("seeded");
            });

        store.with_current(|log| {
            assert_eq!(log.id, id);
            assert_eq!(log.profile_id.as_deref(), Some("default"));
            assert_eq!(log.profile_name.as_deref(), Some("Default"));
            assert_eq!(log.preset_id.as_deref(), Some("preset-1"));
            assert_eq!(log.preset_name.as_deref(), Some("Preset One"));
            assert_eq!(log.llm_provider.as_deref(), Some("anthropic"));
            assert_eq!(log.llm_model.as_deref(), Some("claude-3-7-sonnet"));
            assert_eq!(log.entries.len(), 1);
        });
    }

    #[test]
    fn start_request_log_with_seed_can_omit_initial_llm_metadata() {
        let store = RequestLogStore::new();
        let seed = RecordingRequestSeed {
            llm_provider: Some("anthropic".to_string()),
            llm_model: Some("claude-3-7-sonnet".to_string()),
            ..RecordingRequestSeed::new("openai".to_string(), Some("whisper-1".to_string()))
        };

        let _ = start_request_log_with_seed(&store, &seed, LogLlmSeedMode::OmitConfigured, |_| {});

        store.with_current(|log| {
            assert_eq!(log.llm_provider, None);
            assert_eq!(log.llm_model, None);
        });
    }

    #[test]
    fn seed_log_can_leave_existing_llm_metadata_untouched() {
        let mut log = RequestLog::new("openai".to_string(), Some("whisper-1".to_string()));
        log.llm_provider = Some("existing-provider".to_string());
        log.llm_model = Some("existing-model".to_string());

        RecordingRequestSeed {
            llm_provider: Some("new-provider".to_string()),
            llm_model: Some("new-model".to_string()),
            ..RecordingRequestSeed::new("openai".to_string(), Some("whisper-1".to_string()))
        }
        .seed_log(&mut log, LogLlmSeedMode::LeaveExisting);

        assert_eq!(log.llm_provider.as_deref(), Some("existing-provider"));
        assert_eq!(log.llm_model.as_deref(), Some("existing-model"));
    }
}
