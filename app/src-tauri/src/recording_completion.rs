//! Shared command-facing recording completion helpers.
//!
//! The pipeline state machine still owns real transitions, and `sessions/recording_finalization.rs`
//! still owns request-log/cost/OCR cleanup. This Module owns the remaining boring-but-repeated
//! terminal side effects for recording commands: saved-WAV persistence and final UI event shapes.

use tauri::{AppHandle, Emitter, Manager};

use crate::commands::event_sink::EventSink;
use crate::event_payloads::PipelineErrorPayload;
use crate::events;
use crate::history::RequestHistoryUpdate;
use crate::history_request_lifecycle;
use crate::recordings::RecordingStore;
use crate::PipelineStateEvent;

/// Persist the final WAV under the request id and mirror that availability into History.
///
/// Returns `Ok(true)` when a WAV was saved, `Ok(false)` when there was nothing to save (or no
/// recording store was available), and `Err(...)` when the save itself failed.
pub(crate) fn persist_request_recording(
    app: &AppHandle,
    request_id: Option<&str>,
    wav_bytes: Option<&[u8]>,
    max_saved_recordings: usize,
) -> Result<bool, String> {
    let Some(request_id) = request_id else {
        return Ok(false);
    };
    let Some(wav_bytes) = wav_bytes else {
        return Ok(false);
    };
    let Some(store) = app.try_state::<RecordingStore>() else {
        return Ok(false);
    };

    store
        .save_wav(request_id, wav_bytes)
        .map_err(|e| format!("Failed to persist audio for retry: {e}"))?;

    if let Err(e) = store.prune_to_max_files(max_saved_recordings) {
        log::warn!("Failed to prune saved recordings after persisting {request_id}: {e}");
    }

    history_request_lifecycle::apply_request_history_update(
        app,
        RequestHistoryUpdate::SetRecordingSource {
            request_id: request_id.to_string(),
            recording_request_id: Some(request_id.to_string()),
        },
    )?;

    Ok(true)
}

/// Emit the shared "recording started" frontend contract.
///
/// Keeping the event pair together avoids subtle drift between command and hotkey paths.
pub(crate) fn emit_pipeline_recording_started<S: EventSink>(sink: &S) {
    sink.emit(events::EVENT_PIPELINE_RECORDING_STARTED, &());
    sink.emit(
        events::EVENT_PIPELINE_STATE_CHANGED,
        &PipelineStateEvent::Recording,
    );
}

pub(crate) fn emit_transcript_ready(app: &AppHandle, final_text: &str) {
    let _ = app.emit(events::EVENT_PIPELINE_TRANSCRIPT_READY, final_text);
    let _ = app.emit(
        events::EVENT_PIPELINE_STATE_CHANGED,
        PipelineStateEvent::Idle,
    );
}

pub(crate) fn emit_cancelled(app: &AppHandle) {
    let _ = app.emit(events::EVENT_PIPELINE_CANCELLED, ());
    let _ = app.emit(
        events::EVENT_PIPELINE_STATE_CHANGED,
        PipelineStateEvent::Idle,
    );
}

pub(crate) fn emit_pipeline_error(app: &AppHandle, message: &str, request_id: Option<&str>) {
    let _ = app.emit(
        events::EVENT_PIPELINE_ERROR,
        PipelineErrorPayload {
            message: message.to_string(),
            request_id: request_id.map(str::to_string),
        },
    );
    let _ = app.emit(
        events::EVENT_PIPELINE_STATE_CHANGED,
        PipelineStateEvent::Error,
    );
}
