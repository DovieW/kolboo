//! Shared command-facing recording lifecycle helpers.
//!
//! This Module owns the repeated setup work around the recording commands' request lifecycle:
//! request-log recovery/creation, request-id tracing, initial History updates, OCR Session
//! binding, and watcher-bundle startup delegation. It intentionally reuses the narrower
//! Modules that already own request-log seed shaping, History mutation application, and watcher
//! implementation details instead of collapsing them back into one large command file.

use tauri::AppHandle;

use crate::history::RequestHistoryUpdate;
use crate::history_request_lifecycle;
use crate::pipeline::{OcrConfig, SharedPipeline};
use crate::recording_orchestration::{spawn_recording_phase_watchers, RecordingPhaseWatcherBundle};
use crate::recording_request_initialization::{
    create_in_progress_history_update, record_request_id_on_current_span,
    start_request_log_with_seed, HistorySelectionMode, LogLlmSeedMode, RecordingRequestSeed,
};
use crate::request_log::{RequestLog, RequestLogStore};

#[derive(Debug, Clone, Default)]
pub(crate) struct PreparedRecordingCommandRequest {
    pub(crate) request_id: Option<String>,
    pub(crate) history_updates: Vec<RequestHistoryUpdate>,
}

impl PreparedRecordingCommandRequest {
    #[cfg(test)]
    pub(crate) fn request_id(&self) -> Option<&str> {
        self.request_id.as_deref()
    }

    pub(crate) fn apply_history_updates(&self, app: &AppHandle) {
        for update in self.history_updates.iter().cloned() {
            let _ = history_request_lifecycle::apply_request_history_update(app, update);
        }
    }

    pub(crate) fn bind_ocr_session(&self, pipeline: &SharedPipeline) {
        if let Some(request_id) = self.request_id.as_ref() {
            pipeline.begin_ocr_session(request_id.clone());
        }
    }

    pub(crate) fn bind_ocr_session_for_transcription(
        &self,
        pipeline: &SharedPipeline,
        ocr_config: &OcrConfig,
    ) {
        self.bind_ocr_session(pipeline);

        // Starting OCR eagerly is a command-facing policy decision; keep it here so stop/
        // transcribe callers do not have to re-remember the timing/mode guard on every edit.
        if self.request_id.is_some()
            && ocr_config.auto_capture_timing == "on_start"
            && ocr_config.has_any_auto_mode()
        {
            pipeline.start_ocr_task(ocr_config);
        }
    }

    pub(crate) fn spawn_watchers(
        &self,
        app: AppHandle,
        pipeline: SharedPipeline,
        bundle: RecordingPhaseWatcherBundle,
    ) {
        let _ = self;
        spawn_recording_phase_watchers(app, pipeline, bundle);
    }
}

pub(crate) fn start_recording_request<F>(
    log_store: Option<&RequestLogStore>,
    request_seed: &RecordingRequestSeed,
    llm_mode: LogLlmSeedMode,
    after_seed: F,
) -> Option<String>
where
    F: FnOnce(&mut RequestLog),
{
    let request_id = log_store.map(|log_store| {
        start_request_log_with_seed(log_store, request_seed, llm_mode, after_seed)
    });

    record_request_id_on_current_span(request_id.as_deref());
    request_id
}

pub(crate) fn ensure_current_transcription_request(
    log_store: Option<&RequestLogStore>,
    request_seed: &RecordingRequestSeed,
    selection_mode: HistorySelectionMode,
    max_history_entries: Option<usize>,
    missing_log_message: &str,
) -> PreparedRecordingCommandRequest {
    let mut request_id = log_store.and_then(|store| store.with_current(|log| log.id.clone()));

    if request_id.is_none() {
        request_id = start_recording_request(
            log_store,
            request_seed,
            LogLlmSeedMode::PreserveConfigured,
            |log| {
                log.warn(missing_log_message);
            },
        );
    } else {
        record_request_id_on_current_span(request_id.as_deref());
    }

    PreparedRecordingCommandRequest {
        history_updates: in_progress_history_updates(
            request_id.as_deref(),
            request_seed,
            selection_mode,
            max_history_entries,
        ),
        request_id,
    }
}

pub(crate) fn start_retry_transcription_request(
    log_store: Option<&RequestLogStore>,
    request_seed: &RecordingRequestSeed,
    recording_source_id: &str,
    max_history_entries: Option<usize>,
) -> PreparedRecordingCommandRequest {
    let request_id = start_recording_request(
        log_store,
        request_seed,
        LogLlmSeedMode::OmitConfigured,
        |_| {},
    );

    let mut history_updates = in_progress_history_updates(
        request_id.as_deref(),
        request_seed,
        HistorySelectionMode::PreserveSeededSelection,
        max_history_entries,
    );

    if let Some(request_id) = request_id.as_deref() {
        history_updates.push(RequestHistoryUpdate::SetRecordingSource {
            request_id: request_id.to_string(),
            recording_request_id: Some(recording_source_id.to_string()),
        });
    }

    PreparedRecordingCommandRequest {
        request_id,
        history_updates,
    }
}

fn in_progress_history_updates(
    request_id: Option<&str>,
    request_seed: &RecordingRequestSeed,
    selection_mode: HistorySelectionMode,
    max_history_entries: Option<usize>,
) -> Vec<RequestHistoryUpdate> {
    let Some(request_id) = request_id else {
        return Vec::new();
    };

    vec![create_in_progress_history_update(
        request_id,
        request_seed,
        selection_mode,
        max_history_entries,
    )]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_recording_request_seeds_new_request_log() {
        let store = RequestLogStore::new();
        let seed = RecordingRequestSeed::new("openai".to_string(), Some("whisper-1".to_string()))
            .with_profile(Some("default".to_string()), Some("Default".to_string()));

        let request_id = start_recording_request(
            Some(&store),
            &seed,
            LogLlmSeedMode::PreserveConfigured,
            |log| {
                log.info("Recording started");
            },
        )
        .expect("request id should be created");

        store.with_current(|log| {
            assert_eq!(log.id, request_id);
            assert_eq!(log.profile_id.as_deref(), Some("default"));
            assert_eq!(log.profile_name.as_deref(), Some("Default"));
            assert_eq!(log.entries.len(), 1);
        });
    }

    #[test]
    fn ensure_current_transcription_request_reuses_current_log() {
        let store = RequestLogStore::new();
        let existing_id = store.start_request("groq".to_string(), Some("whisper".to_string()));
        let seed = RecordingRequestSeed::new("groq".to_string(), Some("whisper".to_string()));

        let prepared = ensure_current_transcription_request(
            Some(&store),
            &seed,
            HistorySelectionMode::OmitSeededSelection,
            Some(25),
            "missing log",
        );

        assert_eq!(prepared.request_id(), Some(existing_id.as_str()));
        assert_eq!(prepared.history_updates.len(), 1);
    }

    #[test]
    fn ensure_current_transcription_request_starts_missing_log_with_warning() {
        let store = RequestLogStore::new();
        let seed = RecordingRequestSeed::new("openai".to_string(), Some("whisper-1".to_string()));

        let prepared = ensure_current_transcription_request(
            Some(&store),
            &seed,
            HistorySelectionMode::PreserveSeededSelection,
            Some(10),
            "Request log was missing at stop; started a new request log entry",
        );

        let request_id = prepared.request_id().expect("request id should be present");
        store.with_current(|log| {
            assert_eq!(log.id, request_id);
            assert_eq!(log.entries.len(), 1);
            assert!(log.entries[0]
                .message
                .contains("Request log was missing at stop"));
        });
        assert_eq!(prepared.history_updates.len(), 1);
    }

    #[test]
    fn start_retry_transcription_request_adds_recording_source_update() {
        let store = RequestLogStore::new();
        let seed = RecordingRequestSeed {
            llm_provider: Some("anthropic".to_string()),
            llm_model: Some("claude".to_string()),
            ..RecordingRequestSeed::new("openai".to_string(), Some("whisper-1".to_string()))
        };

        let prepared =
            start_retry_transcription_request(Some(&store), &seed, "original-request", Some(50));

        let request_id = prepared
            .request_id()
            .expect("retry request id should be present")
            .to_string();
        assert_eq!(prepared.history_updates.len(), 2);
        assert!(matches!(
            prepared.history_updates[1],
            RequestHistoryUpdate::SetRecordingSource {
                request_id: ref update_request_id,
                ref recording_request_id,
            } if update_request_id == &request_id && recording_request_id.as_deref() == Some("original-request")
        ));

        store.with_current(|log| {
            assert_eq!(log.id, request_id);
            assert_eq!(log.llm_provider, None);
            assert_eq!(log.llm_model, None);
        });
    }
}
