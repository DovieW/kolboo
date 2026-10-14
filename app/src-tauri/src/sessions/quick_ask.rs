use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use tokio_util::sync::CancellationToken;

use tauri::{AppHandle, Emitter, Manager};

use crate::events;
use crate::state::AppState;

pub(crate) const QUICK_ASK_WINDOW_LABEL: &str = "quick_ask";

pub(crate) const EVENT_QUICK_ASK_STARTED: &str = events::EVENT_QUICK_ASK_STARTED;
pub(crate) const EVENT_QUICK_ASK_ANSWER: &str = events::EVENT_QUICK_ASK_ANSWER;

/// One owner for the answer task. Cancellation and replacement invalidate all
/// later previews/final output; dropping the lease never cancels a newer task.
#[derive(Default)]
pub(crate) struct QuickAskSessions(Arc<Mutex<Option<(String, CancellationToken)>>>);

pub(crate) struct AnswerSession {
    owner: Arc<Mutex<Option<(String, CancellationToken)>>>,
    pub(crate) id: String,
    pub(crate) cancel: CancellationToken,
}

impl QuickAskSessions {
    pub(crate) fn begin(&self, id: String, parent: Option<CancellationToken>) -> AnswerSession {
        let cancel = parent.map_or_else(CancellationToken::new, |parent| parent.child_token());
        let mut current = self.0.lock().unwrap_or_else(|error| error.into_inner());
        if let Some((_, old)) = current.take() {
            old.cancel();
        }
        *current = Some((id.clone(), cancel.clone()));
        AnswerSession {
            owner: self.0.clone(),
            id,
            cancel,
        }
    }
    fn cancel(&self, id: &str) {
        let current = self.0.lock().unwrap_or_else(|error| error.into_inner());
        if let Some((_, token)) = current.as_ref().filter(|(current_id, _)| current_id == id) {
            token.cancel();
        }
    }
}

impl AnswerSession {
    pub(crate) fn current(&self) -> bool {
        !self.cancel.is_cancelled()
            && self
                .owner
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .as_ref()
                .is_some_and(|(id, _)| id == &self.id)
    }
}
impl Drop for AnswerSession {
    fn drop(&mut self) {
        self.cancel.cancel();
        let mut current = self.owner.lock().unwrap_or_else(|error| error.into_inner());
        if current
            .as_ref()
            .is_some_and(|(id, token)| id == &self.id && token.is_cancelled())
        {
            *current = None;
        }
    }
}

#[tauri::command]
pub(crate) fn cancel_quick_ask(app: AppHandle, request_id: String) {
    app.state::<QuickAskSessions>().cancel(&request_id);
}

/// Emit an event intended for the Quick Ask window.
///
/// If the `quick_ask` window exists, emit directly to it (so only that surface updates).
/// Otherwise, fall back to emitting at the app level.
pub(crate) fn emit_to_quick_ask<T: serde::Serialize>(app: &AppHandle, event: &str, payload: T) {
    let Ok(value) = serde_json::to_value(payload) else {
        return;
    };
    // Important: ensure the Quick Ask window is visible/focused *before* emitting.
    // The window's visibility/positioning can change based on monitors and settings;
    // doing this first avoids emitting streaming/error events to a hidden or off-screen window.
    if event == EVENT_QUICK_ASK_STARTED {
        ensure_quick_ask_window_visible(app);
    }
    if let Some(win) = app.get_webview_window(QUICK_ASK_WINDOW_LABEL) {
        let _ = win.emit(event, value);
    } else {
        let _ = app.emit(event, value);
    }
}

#[cfg(test)]
pub(crate) mod tests;

/// Best-effort: make sure the Quick Ask window is visible and focused.
///
/// This is used when we want the user to actually see streaming/error states.
pub(crate) fn ensure_quick_ask_window_visible(app: &AppHandle) {
    let last_ready_ms = app
        .state::<AppState>()
        .quick_ask_frontend_ready_at_ms
        .load(Ordering::SeqCst);
    #[cfg(desktop)]
    {
        crate::commands::overlay::maybe_reload_overlay_webview(
            app,
            QUICK_ASK_WINDOW_LABEL,
            last_ready_ms,
            Some(45_000),
            "quick_ask_show",
        );
    }
    let _ = crate::commands::overlay::position_quick_ask_to_target_monitor(app);
    if let Some(win) = app.get_webview_window(QUICK_ASK_WINDOW_LABEL) {
        let _ = win.set_always_on_top(true);
        let _ = win.show();
        let _ = win.set_focus();
    }
}
