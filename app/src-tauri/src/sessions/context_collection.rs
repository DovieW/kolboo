//! Context collection for Quick Ask and Quick Replace.
//!
//! This Module coordinates existing context-source adapters without replacing them. Selection
//! probing still lives in `selection_probe`, clipboard transport still lives in `clipboard_context`,
//! and OCR ownership still lives in the pipeline. Keeping the orchestration here gives Quick Action
//! execution a smaller Interface: it asks for context instead of knowing probe/clipboard timing.

use std::time::Duration;

use tauri::AppHandle;

use crate::clipboard_context;
use crate::pipeline::{OcrConfig, SharedPipeline};
use crate::sessions::quick_action_lifecycle::{
    is_selection_probe_sentinel, QuickActionContext, QuickActionProbePlan, QuickAskEffectiveConfig,
};
use crate::sessions::selection_probe;

const QUICK_ACTION_CLIPBOARD_CONTEXT_MAX_CHARS: usize = 8_000;
const SELECTION_PROBE_SENTINEL_RETRY_DELAY_MS: u64 = 120;

/// Collect the optional selection and clipboard context for a Quick Ask request.
///
/// Quick Ask can include both highlighted text and clipboard context. We await the selection probe
/// first so a Ctrl+C based probe has a chance to restore the user's clipboard before we read it as
/// additional context. A single targeted sentinel retry preserves the current timing behavior.
pub(crate) async fn collect_quick_ask_context(
    app: &AppHandle,
    probe_plan: QuickActionProbePlan,
    quick_ask_config: &QuickAskEffectiveConfig,
) -> QuickActionContext {
    let selection_context = await_selection_context(app, probe_plan).await;
    let mut context = QuickActionContext::from_probe_result(selection_context);

    if quick_ask_config.include_clipboard_context {
        context = context.with_clipboard_context(read_quick_ask_clipboard_context().await);
    }

    context
}

/// Collect the selected/surrounding text context that can make Quick Replace eligible.
///
/// Clipboard and OCR context intentionally stay as separate helper calls because current behavior
/// only reads clipboard after provider readiness succeeds, while OCR is read once a selection exists.
pub(crate) async fn collect_quick_replace_selection_context(
    app: &AppHandle,
    probe_plan: QuickActionProbePlan,
) -> QuickActionContext {
    QuickActionContext::from_probe_result(await_selection_context(app, probe_plan).await)
}

/// Read optional clipboard context for Quick Replace without sentinel retry.
///
/// Quick Replace reads clipboard later than Quick Ask, after the selection probe has completed and
/// provider readiness has succeeded. Preserve that behavior: do not add extra delay or filtering
/// here unless a future bug proves it is needed. This stays async even though the branching is tiny
/// because the clipboard adapter uses `spawn_blocking` to avoid pinning the async runtime.
pub(crate) async fn read_clipboard_context_if_enabled(
    include_clipboard_context: bool,
) -> Option<String> {
    if include_clipboard_context {
        clipboard_context::read_clipboard_text_best_effort_async(
            QUICK_ACTION_CLIPBOARD_CONTEXT_MAX_CHARS,
        )
        .await
    } else {
        None
    }
}

/// Fetch OCR text for a Quick Action if its effective mode allows OCR.
///
/// Keep the timeout at this boundary so execution code does not need to know whether OCR was still
/// pending, already cached, or unavailable for this request.
pub(crate) async fn collect_quick_action_ocr_text(
    pipeline: &SharedPipeline,
    ocr_mode: &str,
    ocr_config: &OcrConfig,
) -> Option<String> {
    if ocr_mode == "off" {
        return None;
    }

    pipeline
        .get_ocr_result_with_timeout(Duration::from_millis(ocr_config.request_timeout_ms))
        .await
        .map(|result| result.text)
}

async fn await_selection_context(
    app: &AppHandle,
    probe_plan: QuickActionProbePlan,
) -> Option<selection_probe::SelectionProbeContext> {
    if probe_plan.should_await() {
        selection_probe::await_probe_result(
            app,
            probe_plan.probe_kind(),
            probe_plan.epoch(),
            probe_plan.timeout_ms(),
        )
        .await
    } else {
        None
    }
}

async fn read_quick_ask_clipboard_context() -> Option<String> {
    let first = clipboard_context::read_clipboard_text_best_effort_async(
        QUICK_ACTION_CLIPBOARD_CONTEXT_MAX_CHARS,
    )
    .await;

    let value = if should_retry_after_clipboard_read(first.as_deref()) {
        // The selection probe wait should usually outlive the temporary clipboard sentinel, but
        // keep one targeted retry for rare slow clipboard restores instead of imposing a second
        // fixed wait on every Quick Ask clipboard-context request.
        tokio::time::sleep(Duration::from_millis(
            SELECTION_PROBE_SENTINEL_RETRY_DELAY_MS,
        ))
        .await;
        clipboard_context::read_clipboard_text_best_effort_async(
            QUICK_ACTION_CLIPBOARD_CONTEXT_MAX_CHARS,
        )
        .await
    } else {
        first
    };

    filter_selection_probe_sentinel(value)
}

fn should_retry_after_clipboard_read(text: Option<&str>) -> bool {
    text.is_some_and(is_selection_probe_sentinel)
}

fn filter_selection_probe_sentinel(text: Option<String>) -> Option<String> {
    text.filter(|value| !is_selection_probe_sentinel(value.as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sentinel_clipboard_text_requests_one_retry() {
        assert!(should_retry_after_clipboard_read(Some(
            "   __kolboo_selection_probe__abc"
        )));
        assert!(!should_retry_after_clipboard_read(Some(
            "actual clipboard text"
        )));
        assert!(!should_retry_after_clipboard_read(None));
    }

    #[test]
    fn sentinel_clipboard_text_is_filtered_after_retry() {
        assert_eq!(
            filter_selection_probe_sentinel(Some("actual clipboard text".into())).as_deref(),
            Some("actual clipboard text")
        );
        assert_eq!(
            filter_selection_probe_sentinel(Some("__kolboo_selection_probe__abc".into())),
            None
        );
    }
}
