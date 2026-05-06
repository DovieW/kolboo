//! Normal dictation final output handling.
//!
//! Quick Ask and Quick Replace own their LLM-specific request lifecycle elsewhere. This Module owns
//! the normal post-transcription output decision: Quick Replace failure display, live-output
//! de-duplication, the platform-specific paste/type/clipboard path, and non-empty normal success
//! finalization after output warnings have had a chance to land in the request log.

use tauri::{AppHandle, Emitter, Manager};

use crate::commands;
use crate::event_payloads::{PipelineErrorPayload, PipelineStateEvent};
use crate::history::HistoryStorage;
use crate::pipeline::{SharedPipeline, TranscriptionResult};
use crate::request_log::{RequestLogStore, RequestStatus};
use crate::sessions::recording_finalization;
use crate::stats;

/// Inputs needed to decide and perform final normal dictation output.
pub(crate) struct NormalDictationOutputRequest<'a> {
    pub(crate) output_value: &'a str,
    pub(crate) output_intent: crate::core::output_settings::ResolvedOutputIntent,
    pub(crate) live_output_completed: bool,
    pub(crate) quick_replace_failure: Option<&'a str>,
    pub(crate) request_id: Option<&'a str>,
}

/// Inputs needed after normal dictation output has had a chance to record output warnings.
pub(crate) struct NormalDictationFinalizationRequest<'a> {
    pub(crate) pipeline: &'a SharedPipeline,
    pub(crate) request_id: Option<&'a str>,
    pub(crate) result: &'a TranscriptionResult,
    pub(crate) output_value: &'a str,
    pub(crate) quick_replace_failure: Option<&'a str>,
    pub(crate) complete_request_log_after_output: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NormalDictationOutputDecision {
    QuickReplaceFailure,
    LiveOutputAlreadyCompleted,
    Output,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NormalDictationOutputResult {
    pub(crate) decision: NormalDictationOutputDecision,
    pub(crate) output_error: Option<String>,
}

/// Execute final output for a non-Quick-Ask dictation request.
pub(crate) async fn execute_normal_dictation_output(
    app: &AppHandle,
    request: NormalDictationOutputRequest<'_>,
) -> NormalDictationOutputResult {
    match decide_normal_dictation_output(
        request.quick_replace_failure,
        request.live_output_completed,
    ) {
        NormalDictationOutputDecision::QuickReplaceFailure => {
            if let Some(err) = request.quick_replace_failure {
                // In hotkey-triggered flows, the overlay webview may not be created (or visible)
                // yet. Show it so the error state + Retry UI is actually seen.
                let _ = commands::overlay::show_overlay(app.clone()).await;

                // Emit pipeline-error so overlay shows error state + retry affordance.
                let payload = PipelineErrorPayload {
                    message: err.to_string(),
                    request_id: request.request_id.map(str::to_string),
                };
                let _ = app.emit(crate::events::EVENT_PIPELINE_ERROR, payload);
                let _ = app.emit(
                    crate::events::EVENT_PIPELINE_STATE_CHANGED,
                    PipelineStateEvent::Error,
                );
            }

            NormalDictationOutputResult {
                decision: NormalDictationOutputDecision::QuickReplaceFailure,
                output_error: request.quick_replace_failure.map(str::to_string),
            }
        }
        NormalDictationOutputDecision::LiveOutputAlreadyCompleted => {
            // Live output already pasted committed chunks during recording — skip the final paste
            // to avoid duplication.
            log::info!("Pipeline: skipping final paste (live output completed)");

            NormalDictationOutputResult {
                decision: NormalDictationOutputDecision::LiveOutputAlreadyCompleted,
                output_error: None,
            }
        }
        NormalDictationOutputDecision::Output => {
            let output_error =
                output_text_for_platform(app, request.output_value, request.output_intent);

            NormalDictationOutputResult {
                decision: NormalDictationOutputDecision::Output,
                output_error,
            }
        }
    }
}

/// Finalize a non-empty normal dictation request after output execution.
///
/// Keep this ordering aligned with the old `lib.rs` implementation: finish the request log after
/// output warnings are recorded, emit stats/cost, end the matching OCR session, update History,
/// then apply retention. Quick Ask intentionally does not use this path.
pub(crate) fn finalize_normal_dictation_request(
    app: &AppHandle,
    request: NormalDictationFinalizationRequest<'_>,
) {
    finalize_request_log_after_output(app, &request);
    update_history_after_output(app, &request);

    // Time-based retention (best-effort). This path is used by global shortcuts. Quick Ask calls
    // retention from `lib.rs` after its answer flow and intentionally never reaches this helper.
    commands::recording::apply_transcription_retention(app);
}

fn decide_normal_dictation_output(
    quick_replace_failure: Option<&str>,
    live_output_completed: bool,
) -> NormalDictationOutputDecision {
    if quick_replace_failure.is_some() {
        return NormalDictationOutputDecision::QuickReplaceFailure;
    }

    if live_output_completed {
        return NormalDictationOutputDecision::LiveOutputAlreadyCompleted;
    }

    NormalDictationOutputDecision::Output
}

fn finalize_request_log_after_output(
    app: &AppHandle,
    request: &NormalDictationFinalizationRequest<'_>,
) {
    if !request.complete_request_log_after_output {
        return;
    }

    if let Some(log_store) = app.try_state::<RequestLogStore>() {
        log_store.with_current(|log| {
            if log.status == RequestStatus::InProgress {
                log.complete_success();
            }
        });
    }

    recording_finalization::complete_current_request_with_pipeline_wav(
        app,
        request.pipeline,
        request.request_id,
        stats::EventStatus::Success,
    );
}

fn update_history_after_output(app: &AppHandle, request: &NormalDictationFinalizationRequest<'_>) {
    let Some(req_id) = request.request_id else {
        return;
    };

    let Some(history) = app.try_state::<HistoryStorage>() else {
        return;
    };

    // Store the actual output (Quick Replace may have changed it).
    if let Some(err) = request.quick_replace_failure {
        let _ = history.complete_request_error(req_id, err.to_string());
    } else if let Err(e) =
        history.complete_request_success(req_id, request.output_value.to_string())
    {
        log::warn!("Failed to update history: {}", e);
    }

    recording_finalization::persist_history_llm_metadata(app, Some(req_id), request.result);
}

#[cfg(target_os = "windows")]
fn output_text_for_platform(
    app: &AppHandle,
    output_value: &str,
    output_intent: crate::core::output_settings::ResolvedOutputIntent,
) -> Option<String> {
    if matches!(output_intent.mode(), commands::text::OutputMode::Paste) {
        let snapshot = app
            .state::<crate::state::AppState>()
            .windows_text_target_snapshot
            .lock()
            .ok()
            .and_then(|mut g| g.take());

        if let Err(e) = crate::windows_uia::insert::insert_text_with_snapshot(
            app,
            output_value,
            snapshot,
            true,
            true,
            output_intent.smart_paste_protection(),
        ) {
            log::error!("Failed to output transcript (UIA ladder): {}", e);
            record_output_failure(app, &e);
            return Some(e);
        }

        return None;
    }

    let safe_to_insert = if output_intent.smart_paste_protection() {
        // Preserve the existing safety-first behavior: non-paste modes re-check the currently
        // focused target instead of relying on the recording-stop snapshot. LLM latency, overlay
        // focus changes, or user navigation can move focus between stop-recording and final output.
        crate::windows_uia::snapshot::capture_focused_snapshot()
            .ok()
            .map(|snapshot| {
                crate::windows_uia::safety::allow_insert_with_protection(
                    &snapshot,
                    output_intent.smart_paste_protection(),
                )
            })
            .unwrap_or(true)
    } else {
        true
    };

    if !safe_to_insert {
        if let Err(e) = crate::text::inject::copy_to_clipboard_and_notify(app, output_value) {
            // Preserve current behavior: safe-fallback failure is logged, but it does not add the
            // request-log warning used by normal output-mode failures.
            log::error!("Failed to output transcript (safe fallback): {}", e);
        }
        return None;
    }

    if let Err(e) = commands::text::output_text_with_mode_options(
        output_value,
        output_intent.mode(),
        output_intent.hit_enter(),
        !output_intent.clipboard_privacy_mode(),
    ) {
        log::error!("Failed to output transcript: {}", e);
        record_output_failure(app, &e);
        return Some(e);
    }

    None
}

#[cfg(not(target_os = "windows"))]
fn output_text_for_platform(
    app: &AppHandle,
    output_value: &str,
    output_intent: crate::core::output_settings::ResolvedOutputIntent,
) -> Option<String> {
    if let Err(e) = commands::text::output_text_with_mode_options(
        output_value,
        output_intent.mode(),
        output_intent.hit_enter(),
        !output_intent.clipboard_privacy_mode(),
    ) {
        log::error!("Failed to output transcript: {}", e);
        record_output_failure(app, &e);
        return Some(e);
    }

    None
}

fn record_output_failure(app: &AppHandle, error: &str) {
    if let Some(log_store) = app.try_state::<RequestLogStore>() {
        log_store.with_current(|log| {
            log.warn(format!("Output failed: {}", error));
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_decision_prioritizes_quick_replace_failure() {
        assert_eq!(
            decide_normal_dictation_output(Some("rewrite failed"), true),
            NormalDictationOutputDecision::QuickReplaceFailure
        );
    }

    #[test]
    fn output_decision_skips_when_live_output_completed() {
        assert_eq!(
            decide_normal_dictation_output(None, true),
            NormalDictationOutputDecision::LiveOutputAlreadyCompleted
        );
    }

    #[test]
    fn output_decision_outputs_for_normal_dictation() {
        assert_eq!(
            decide_normal_dictation_output(None, false),
            NormalDictationOutputDecision::Output
        );
    }
}
