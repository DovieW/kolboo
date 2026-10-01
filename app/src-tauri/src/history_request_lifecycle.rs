//! App-level helpers for request-row lifecycle updates.
//!
//! `history.rs` owns persistence and the low-level transition methods. This Module owns the
//! app-facing glue that applies those updates and emits `history-changed` exactly once per
//! successful request-row mutation.

use tauri::{AppHandle, Emitter, Manager, Runtime};

use crate::events;
use crate::history::{HistoryStorage, RequestHistoryUpdate};
use crate::request_log::{RequestLogStore, RequestStatus};

fn emit_history_changed(app: &AppHandle) {
    let _ = app.emit(events::EVENT_HISTORY_CHANGED, ());
}

pub(crate) fn apply_request_history_update(
    app: &AppHandle,
    update: RequestHistoryUpdate,
) -> Result<(), String> {
    let Some(history) = app.try_state::<HistoryStorage>() else {
        return Ok(());
    };

    history.apply_request_update(update)?;
    emit_history_changed(app);
    Ok(())
}

/// A newer operation owns the pipeline and overlay now. Preserve this request's
/// result without inspecting the pipeline, completing a newer log, or emitting
/// any pipeline/window event. Archived request logs were cancelled by start_request.
pub(crate) fn complete_superseded_request<R: Runtime>(
    app: &AppHandle<R>,
    request_id: Option<&str>,
    result: Result<&str, &str>,
) -> Result<(), String> {
    let Some(id) = request_id else {
        return Ok(());
    };
    if let Some(logs) = app.try_state::<RequestLogStore>() {
        logs.with_current_id(id, |log| {
            if log.status == RequestStatus::InProgress {
                log.complete_cancelled();
            }
        });
    }
    if let Some(history) = app.try_state::<HistoryStorage>() {
        let completion = history.complete_pending_request(id, result);
        if completion.is_err() {
            log::warn!("Could not persist a superseded History result; row remains recoverable");
        }
        if completion? {
            let _ = app.emit(events::EVENT_HISTORY_CHANGED, ());
        }
    }
    Ok(())
}

/// Copy the current request log's resolved profile metadata into History.
///
/// This intentionally uses `with_current_id(...)` so late async work cannot stamp profile chips
/// onto the wrong row after a newer request becomes current.
pub(crate) fn sync_request_profile_from_current_log(
    app: &AppHandle,
    request_id: Option<&str>,
) -> Result<(), String> {
    let Some(request_id) = request_id else {
        return Ok(());
    };

    let Some(log_store) = app.try_state::<RequestLogStore>() else {
        return Ok(());
    };

    let Some((profile_id, profile_name)) = log_store.with_current_id(request_id, |log| {
        (log.profile_id.clone(), log.profile_name.clone())
    }) else {
        return Ok(());
    };

    apply_request_history_update(
        app,
        RequestHistoryUpdate::SetProfile {
            request_id: request_id.to_string(),
            profile_id,
            profile_name,
        },
    )
}

#[cfg(test)]
#[path = "tests/history_request_lifecycle.rs"]
pub(crate) mod tests;
