//! Shared OCR usage policy helpers.
//!
//! `pipeline::ocr_session_state` still owns the actual OCR session/task state machine. This module
//! only centralizes the small policy decisions that multiple flows were rebuilding by hand:
//! whether stop-recording should auto-start OCR, and how Quick Ask / Quick Replace should consume
//! OCR context with a timeout while keeping status/failure details together.

use std::time::Duration;

use crate::pipeline::{OcrConfig, SharedPipeline};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CollectedOcrContext {
    requested: bool,
    status: String,
    failed_reason: Option<String>,
    text: Option<String>,
    timeout_ms: u64,
}

impl CollectedOcrContext {
    pub(crate) fn requested(&self) -> bool {
        self.requested
    }

    pub(crate) fn status(&self) -> &str {
        self.status.as_str()
    }

    pub(crate) fn failed_reason(&self) -> Option<&str> {
        self.failed_reason.as_deref()
    }

    pub(crate) fn text(&self) -> Option<&str> {
        self.text.as_deref()
    }

    pub(crate) fn text_len(&self) -> Option<usize> {
        self.text.as_deref().map(str::len)
    }

    pub(crate) fn timeout_ms(&self) -> u64 {
        self.timeout_ms
    }
}

/// Start OCR at stop time only when the request's policy allows auto OCR and the current session
/// has not already started or finished OCR.
pub(crate) fn ensure_stop_time_ocr_started(
    pipeline: &SharedPipeline,
    ocr_config: &OcrConfig,
    should_auto_ocr: bool,
) {
    if should_start_stop_time_ocr(pipeline.get_ocr_status().as_str(), should_auto_ocr) {
        pipeline.start_ocr_task_if_auto(ocr_config, true);
    }
}

/// Collect OCR context for a flow that may consume OCR text.
///
/// The caller gets a single value describing whether OCR was requested, whether text was actually
/// attached, and what status/failure state remained after the wait. This keeps request-log/UI code
/// from having to re-query the pipeline in subtly different ways.
pub(crate) async fn collect_ocr_context(
    pipeline: &SharedPipeline,
    ocr_mode: &str,
    ocr_config: &OcrConfig,
) -> CollectedOcrContext {
    if !ocr_mode_requests_context(ocr_mode) {
        return CollectedOcrContext {
            requested: false,
            status: pipeline.get_ocr_status(),
            failed_reason: None,
            text: None,
            timeout_ms: ocr_config.request_timeout_ms,
        };
    }

    let text = pipeline
        .get_ocr_result_with_timeout(Duration::from_millis(ocr_config.request_timeout_ms))
        .await
        .map(|result| result.text);

    let status = pipeline.get_ocr_status();
    let failed_reason = if text.is_some() {
        None
    } else {
        pipeline.get_ocr_failed_reason()
    };

    CollectedOcrContext {
        requested: true,
        status,
        failed_reason,
        text,
        timeout_ms: ocr_config.request_timeout_ms,
    }
}

fn should_start_stop_time_ocr(current_status: &str, should_auto_ocr: bool) -> bool {
    should_auto_ocr && !matches!(current_status, "running" | "done")
}

fn ocr_mode_requests_context(ocr_mode: &str) -> bool {
    ocr_mode != "off"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stop_time_ocr_only_starts_when_auto_and_not_already_started() {
        assert!(should_start_stop_time_ocr("not_started", true));
        assert!(should_start_stop_time_ocr("failed", true));
        assert!(!should_start_stop_time_ocr("running", true));
        assert!(!should_start_stop_time_ocr("done", true));
        assert!(!should_start_stop_time_ocr("not_started", false));
    }

    #[test]
    fn off_mode_does_not_request_ocr_context() {
        assert!(!ocr_mode_requests_context("off"));
        assert!(ocr_mode_requests_context("auto"));
        assert!(ocr_mode_requests_context("manual"));
    }
}
