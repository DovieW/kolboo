//! Request-owned OCR Session state.
//!
//! This module owns the small state machine around active-window OCR results:
//! session identity, in-flight task ownership, timeout restore, cancellation,
//! reusable results, and sanitized failure status.

pub(super) struct OcrTaskHandle {
    pub(super) session_id: Option<String>,
    pub(super) request_log_id: Option<String>,
    pub(super) handle: tokio::task::JoinHandle<Result<crate::ocr::OcrResult, String>>,
}

impl OcrTaskHandle {
    pub(super) fn new(
        session_id: Option<String>,
        request_log_id: Option<String>,
        handle: tokio::task::JoinHandle<Result<crate::ocr::OcrResult, String>>,
    ) -> Self {
        Self {
            session_id,
            request_log_id,
            handle,
        }
    }
}

#[derive(Default)]
pub(super) struct OcrSessionState {
    pub(super) session_id: Option<String>,
    pub(super) task: Option<OcrTaskHandle>,
    pub(super) abort_handle: Option<tokio::task::AbortHandle>,
    pub(super) result: Option<crate::ocr::OcrResult>,
    pub(super) failed_reason: Option<String>,
    pub(super) cancelled: bool,
    pub(super) awaiting: bool,
}

impl OcrSessionState {
    pub(super) fn session_id(&self) -> Option<&str> {
        self.session_id.as_deref()
    }

    pub(super) fn session_id_owned(&self) -> Option<String> {
        self.session_id.clone()
    }

    pub(super) fn request_log_id_for_cancel(&self) -> Option<String> {
        self.task
            .as_ref()
            .and_then(|task| task.request_log_id.clone())
            .or_else(|| self.session_id.clone())
    }

    pub(super) fn ensure_session(&mut self, session_id: Option<String>) {
        if self.session_id.is_none() {
            self.session_id = session_id;
        }
    }

    pub(super) fn can_start_task(&self) -> bool {
        self.task.is_none() && self.result.is_none() && !self.awaiting
    }

    pub(super) fn prepare_task_start(&mut self) -> Option<String> {
        self.failed_reason = None;
        self.cancelled = false;
        self.session_id.clone()
    }

    pub(super) fn install_task(&mut self, task: OcrTaskHandle) {
        self.abort_handle = Some(task.handle.abort_handle());
        self.task = Some(task);
    }

    pub(super) fn take_finished_task(&mut self) -> Option<OcrTaskHandle> {
        let task = self.task.as_ref()?;
        if !task.handle.is_finished() {
            return None;
        }
        self.task.take()
    }

    pub(super) fn cached_result(&self) -> Option<crate::ocr::OcrResult> {
        self.result.clone()
    }

    pub(super) fn take_task_for_await(&mut self) -> Option<OcrTaskHandle> {
        let task = self.task.take()?;
        self.awaiting = true;
        Some(task)
    }

    pub(super) fn clear_awaiting_without_task(&mut self) {
        self.awaiting = false;
    }

    pub(super) fn restore_task_after_timeout(
        &mut self,
        expected_session_id: Option<&str>,
        task: OcrTaskHandle,
    ) -> Result<(), OcrTaskHandle> {
        if !session_matches(self.session_id.as_deref(), expected_session_id) {
            return Err(task);
        }

        self.awaiting = false;
        if self.result.is_none() && self.task.is_none() {
            self.task = Some(task);
            Ok(())
        } else {
            Err(task)
        }
    }

    pub(super) fn complete_success(
        &mut self,
        expected_session_id: Option<&str>,
        result: crate::ocr::OcrResult,
    ) -> bool {
        if !session_matches(self.session_id.as_deref(), expected_session_id) {
            return false;
        }

        self.result = Some(result);
        self.failed_reason = None;
        self.cancelled = false;
        self.awaiting = false;
        self.abort_handle = None;
        true
    }

    pub(super) fn complete_failure(
        &mut self,
        expected_session_id: Option<&str>,
        reason: String,
    ) -> bool {
        if !session_matches(self.session_id.as_deref(), expected_session_id) {
            return false;
        }

        self.failed_reason = Some(reason);
        self.result = None;
        self.cancelled = false;
        self.awaiting = false;
        self.abort_handle = None;
        true
    }

    pub(super) fn complete_join_error(
        &mut self,
        expected_session_id: Option<&str>,
        join_err: &tokio::task::JoinError,
    ) -> bool {
        if !session_matches(self.session_id.as_deref(), expected_session_id) {
            return false;
        }

        self.result = None;
        self.failed_reason = Some(join_err.to_string());
        self.cancelled = join_err.is_cancelled();
        self.awaiting = false;
        self.abort_handle = None;
        true
    }

    pub(super) fn cancel_task(&mut self, mark_cancelled: bool) {
        if let Some(task) = self.task.take() {
            log::debug!(
                "cancel_ocr_task called: mark_cancelled={}, aborting task",
                mark_cancelled
            );
            task.handle.abort();
        } else if let Some(abort_handle) = self.abort_handle.take() {
            log::debug!(
                "cancel_ocr_task called: mark_cancelled={}, aborting awaited task",
                mark_cancelled
            );
            abort_handle.abort();
        } else {
            log::debug!(
                "cancel_ocr_task called: mark_cancelled={}, no task to abort",
                mark_cancelled
            );
        }

        self.abort_handle = None;
        self.result = None;
        self.failed_reason = None;
        self.cancelled = mark_cancelled;
        self.awaiting = false;
    }

    pub(super) fn begin_session(&mut self, session_id: String) -> bool {
        if self.session_id.as_deref() == Some(session_id.as_str()) {
            return false;
        }

        if self.has_active_or_reusable_state() {
            self.cancel_task(true);
        }

        self.session_id = Some(session_id);
        self.cancelled = false;
        self.failed_reason = None;
        self.result = None;
        self.task = None;
        self.awaiting = false;
        true
    }

    pub(super) fn end_session_if_matches(&mut self, session_id: &str) -> bool {
        if self.session_id.as_deref() != Some(session_id) {
            return false;
        }

        self.cancel_task(false);
        self.session_id = None;
        true
    }

    pub(super) fn status(&self) -> &'static str {
        if self.cancelled {
            return "cancelled";
        }
        if self.result.is_some() {
            return "done";
        }
        if self.task.is_some() || self.awaiting {
            return "running";
        }
        if self.failed_reason.is_some() {
            return "failed";
        }
        "not_started"
    }

    pub(super) fn failed_reason(&self) -> Option<String> {
        self.failed_reason.clone()
    }

    fn has_active_or_reusable_state(&self) -> bool {
        self.task.is_some() || self.abort_handle.is_some() || self.result.is_some() || self.awaiting
    }
}

fn session_matches(current: Option<&str>, expected: Option<&str>) -> bool {
    current == expected
}

#[cfg(test)]
mod tests {
    use super::{OcrSessionState, OcrTaskHandle};

    fn result(text: &str) -> crate::ocr::OcrResult {
        crate::ocr::OcrResult {
            text: text.to_string(),
            provider: "fixture".to_string(),
            model: "fixture-model".to_string(),
        }
    }

    fn pending_task(session_id: &str) -> OcrTaskHandle {
        let handle = tokio::spawn(std::future::pending::<Result<crate::ocr::OcrResult, String>>());
        OcrTaskHandle::new(
            Some(session_id.to_string()),
            Some(session_id.to_string()),
            handle,
        )
    }

    #[test]
    fn stale_success_and_failure_do_not_mutate_current_session() {
        let mut state = OcrSessionState::default();
        state.begin_session("request-b".to_string());

        assert!(!state.complete_success(Some("request-a"), result("stale")));
        assert_eq!(state.status(), "not_started");
        assert!(state.cached_result().is_none());

        assert!(!state.complete_failure(Some("request-a"), "stale failure".to_string()));
        assert_eq!(state.status(), "not_started");
        assert_eq!(state.failed_reason(), None);
    }

    #[tokio::test]
    async fn await_timeout_restores_running_task_for_current_session() {
        let mut state = OcrSessionState::default();
        state.begin_session("request-a".to_string());
        state.install_task(pending_task("request-a"));

        let task = state.take_task_for_await().expect("task for await");
        assert_eq!(state.status(), "running");

        assert!(state
            .restore_task_after_timeout(Some("request-a"), task)
            .is_ok());
        assert_eq!(state.status(), "running");
        state.cancel_task(true);
    }

    #[tokio::test]
    async fn start_task_preparation_tracks_session_and_cancel_request_log() {
        let mut state = OcrSessionState::default();

        state.ensure_session(Some("request-a".to_string()));
        state.ensure_session(Some("request-b".to_string()));

        assert_eq!(state.session_id(), Some("request-a"));
        assert_eq!(state.session_id_owned().as_deref(), Some("request-a"));
        assert!(state.can_start_task());

        state.failed_reason = Some("old failure".to_string());
        state.cancelled = true;

        assert_eq!(state.prepare_task_start().as_deref(), Some("request-a"));
        assert_eq!(state.failed_reason(), None);
        assert!(!state.cancelled);

        state.install_task(pending_task("request-a"));

        assert!(!state.can_start_task());
        assert_eq!(state.status(), "running");
        assert_eq!(
            state.request_log_id_for_cancel().as_deref(),
            Some("request-a"),
        );

        state.cancel_task(true);

        assert_eq!(
            state.request_log_id_for_cancel().as_deref(),
            Some("request-a"),
        );
    }

    #[tokio::test]
    async fn take_finished_task_only_takes_completed_tasks() {
        let mut state = OcrSessionState::default();
        state.begin_session("request-a".to_string());

        assert!(state.take_finished_task().is_none());

        state.install_task(pending_task("request-a"));
        assert!(state.take_finished_task().is_none());
        state.cancel_task(true);

        let handle = tokio::spawn(async { Ok(result("ready")) });
        for _ in 0..16 {
            if handle.is_finished() {
                break;
            }
            tokio::task::yield_now().await;
        }
        state.install_task(OcrTaskHandle::new(
            Some("request-a".to_string()),
            Some("request-a".to_string()),
            handle,
        ));

        assert!(state.take_finished_task().is_some());
        assert!(state.task.is_none());
    }

    #[tokio::test]
    async fn timeout_restore_returns_task_when_state_cannot_reuse_it() {
        let mut state = OcrSessionState::default();
        state.begin_session("request-current".to_string());

        let stale_task = pending_task("request-old");
        let stale_task = state
            .restore_task_after_timeout(Some("request-old"), stale_task)
            .expect_err("stale session should hand task back to caller for abort");
        stale_task.handle.abort();

        assert!(state.complete_success(Some("request-current"), result("already done")));
        state.awaiting = true;
        let redundant_task = pending_task("request-current");
        let redundant_task = state
            .restore_task_after_timeout(Some("request-current"), redundant_task)
            .expect_err("completed state should hand task back to caller for abort");
        redundant_task.handle.abort();
        assert!(!state.awaiting);
        assert_eq!(state.status(), "done");
    }

    #[tokio::test]
    async fn join_error_completion_marks_cancelled_for_current_session_only() {
        let mut state = OcrSessionState::default();
        state.begin_session("request-a".to_string());

        let handle = tokio::spawn(std::future::pending::<Result<crate::ocr::OcrResult, String>>());
        handle.abort();
        let join_err = handle
            .await
            .expect_err("aborted task should return join error");

        assert!(!state.complete_join_error(Some("request-b"), &join_err));
        assert_eq!(state.status(), "not_started");

        assert!(state.complete_join_error(Some("request-a"), &join_err));
        assert_eq!(state.status(), "cancelled");
        assert!(state.failed_reason().is_some());
    }

    #[tokio::test]
    async fn begin_and_end_session_report_noop_and_matching_transitions() {
        let mut state = OcrSessionState::default();

        assert!(state.begin_session("request-a".to_string()));
        assert!(!state.begin_session("request-a".to_string()));
        assert!(!state.end_session_if_matches("request-b"));

        state.install_task(pending_task("request-a"));
        assert!(state.begin_session("request-b".to_string()));
        assert_eq!(state.session_id(), Some("request-b"));
        assert_eq!(state.status(), "not_started");

        assert!(state.end_session_if_matches("request-b"));
        assert_eq!(state.session_id(), None);
        assert_eq!(state.status(), "not_started");
    }

    #[tokio::test]
    async fn explicit_cancellation_clears_reusable_result_and_is_idempotent() {
        let mut state = OcrSessionState::default();
        state.begin_session("request-a".to_string());
        assert!(state.complete_success(Some("request-a"), result("usable")));

        state.cancel_task(true);
        assert_eq!(state.status(), "cancelled");
        assert!(state.cached_result().is_none());

        state.cancel_task(true);
        assert_eq!(state.status(), "cancelled");
    }

    #[test]
    fn failure_reason_is_request_specific_status() {
        let mut state = OcrSessionState::default();
        state.begin_session("request-a".to_string());

        assert!(
            state.complete_failure(Some("request-a"), "sanitized provider failure".to_string(),)
        );

        assert_eq!(state.status(), "failed");
        assert_eq!(
            state.failed_reason().as_deref(),
            Some("sanitized provider failure"),
        );
    }
}
