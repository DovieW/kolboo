use std::sync::{Arc, Mutex};
use tokio_util::sync::CancellationToken;

use tauri::{AppHandle, Emitter, Manager};

use crate::events;
use crate::QuickAskStatePayload;

pub(crate) const QUICK_ASK_WINDOW_LABEL: &str = "quick_ask";

pub(crate) const EVENT_QUICK_ASK_STARTED: &str = events::EVENT_QUICK_ASK_STARTED;
pub(crate) const EVENT_QUICK_ASK_ANSWER: &str = events::EVENT_QUICK_ASK_ANSWER;

/// One owner for the answer task. Cancellation and replacement invalidate all
/// later previews/final output; dropping the lease never cancels a newer task.
#[derive(Default)]
pub(crate) struct QuickAskSessions(
    Arc<Mutex<Option<(String, CancellationToken)>>>,
    Arc<Mutex<QuickAskSurface>>,
);

#[derive(Default)]
struct QuickAskSurface {
    request_id: Option<String>,
    revision: u64,
    snapshot: Option<QuickAskStatePayload>,
    /// One presenter owns construction through the GUI commit. Cancellation
    /// leaves its registered-but-not-yet-configured window for that owner.
    presenting: bool,
}

impl QuickAskSurface {
    fn replace(&mut self, id: String) {
        self.request_id = Some(id);
        self.clear();
    }

    fn clear(&mut self) {
        self.revision += 1;
        self.snapshot = Some(QuickAskStatePayload {
            revision: self.revision,
            started: None,
            partial: None,
            answer: None,
        });
    }

    fn update(&mut self, event: &str, value: serde_json::Value) -> Option<QuickAskStatePayload> {
        let id = value.get("request_id")?.as_str()?;
        if self.request_id.as_deref() != Some(id) {
            return None;
        }
        match event {
            EVENT_QUICK_ASK_STARTED => {
                self.snapshot = Some(QuickAskStatePayload {
                    revision: 0,
                    started: Some(serde_json::from_value(value).ok()?),
                    partial: None,
                    answer: None,
                });
            }
            events::EVENT_QUICK_ASK_PARTIAL => {
                let snapshot = self.snapshot.as_mut()?;
                snapshot.started.as_ref()?;
                let partial: crate::QuickAskPartialPayload = serde_json::from_value(value).ok()?;
                if snapshot.answer.is_some()
                    || snapshot
                        .partial
                        .as_ref()
                        .is_some_and(|old| old.seq >= partial.seq)
                {
                    return None;
                }
                snapshot.partial = Some(partial);
            }
            EVENT_QUICK_ASK_ANSWER => {
                let snapshot = self.snapshot.as_mut()?;
                snapshot.started.as_ref()?;
                if snapshot.answer.is_some() {
                    return None;
                }
                snapshot.answer = Some(serde_json::from_value(value).ok()?);
            }
            _ => return None,
        }
        self.revision += 1;
        let snapshot = self.snapshot.as_mut()?;
        snapshot.revision = self.revision;
        Some(snapshot.clone())
    }
}

pub(crate) struct AnswerSession {
    owner: Arc<Mutex<Option<(String, CancellationToken)>>>,
    surface: Arc<Mutex<QuickAskSurface>>,
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
        self.1
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .replace(id.clone());
        AnswerSession {
            owner: self.0.clone(),
            surface: self.1.clone(),
            id,
            cancel,
        }
    }
    fn cancel(&self, id: &str) -> Option<QuickAskStatePayload> {
        let current = self.0.lock().unwrap_or_else(|error| error.into_inner());
        if let Some((_, token)) = current.as_ref().filter(|(current_id, _)| current_id == id) {
            token.cancel();
        }
        let mut surface = self.1.lock().unwrap_or_else(|error| error.into_inner());
        if surface.request_id.as_deref() == Some(id)
            && surface
                .snapshot
                .as_ref()
                .is_some_and(|snapshot| snapshot.started.is_some())
        {
            surface.clear();
            return surface.snapshot.clone();
        }
        None
    }

    pub(crate) fn snapshot(&self) -> Option<QuickAskStatePayload> {
        self.1
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .snapshot
            .clone()
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
            // The surface owns completed display state, not the answer lease.
            let mut surface = self
                .surface
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            if surface
                .snapshot
                .as_ref()
                .is_some_and(|snapshot| snapshot.answer.is_none())
            {
                surface.clear();
            }
        }
    }
}

#[tauri::command]
pub(crate) fn cancel_quick_ask(app: AppHandle, request_id: String, dismiss: Option<bool>) {
    if let Some(snapshot) = app.state::<QuickAskSessions>().cancel(&request_id) {
        if let Some(window) = app.get_webview_window(QUICK_ASK_WINDOW_LABEL) {
            let _ = window.emit(events::EVENT_QUICK_ASK_STATE, snapshot);
        }
    }
    if dismiss == Some(true) {
        let handle = app.clone();
        let _ = app.run_on_main_thread(move || {
            let sessions = handle.state::<QuickAskSessions>();
            let mut surface = sessions.1.lock().unwrap_or_else(|error| error.into_inner());
            if surface.request_id.as_deref() == Some(&request_id) {
                surface.request_id = None;
                #[cfg(desktop)]
                crate::set_escape_cancel_shortcut_enabled(&handle, false);
                if !surface.presenting {
                    if let Some(window) = handle.get_webview_window(QUICK_ASK_WINDOW_LABEL) {
                        let _ = window.destroy();
                    }
                }
            }
        });
    }
}

/// Recording owns invalidation even while a lazy renderer has no listeners.
pub(crate) fn recording_started(app: &AppHandle) {
    if let Some(id) = recording_request_id(app.try_state::<QuickAskSessions>()) {
        cancel_quick_ask(app.clone(), id, Some(true));
    }
}

fn recording_request_id(sessions: Option<tauri::State<'_, QuickAskSessions>>) -> Option<String> {
    let sessions = sessions?;
    let id = sessions
        .1
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .request_id
        .clone();
    id
}

/// Emit an event intended for the Quick Ask window.
///
/// Retain only the newest request state before lazy construction. Window event
/// delivery and listener-ready replay share a monotonic revision.
pub(crate) fn emit_to_quick_ask<T: serde::Serialize>(app: &AppHandle, event: &str, payload: T) {
    let Ok(value) = serde_json::to_value(payload) else {
        return;
    };
    emit_quick_ask_value(app, event, value);
}

fn emit_quick_ask_value(app: &AppHandle, event: &str, value: serde_json::Value) {
    let sessions = app.state::<QuickAskSessions>();
    let current = sessions.0.lock().unwrap_or_else(|error| error.into_inner());
    if !current.as_ref().is_some_and(|(id, token)| {
        value.get("request_id").and_then(serde_json::Value::as_str) == Some(id)
            && !token.is_cancelled()
    }) {
        return;
    }
    let snapshot = sessions
        .1
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .update(event, value.clone());
    drop(current);
    let Some(snapshot) = snapshot else {
        return;
    };
    if event == EVENT_QUICK_ASK_STARTED {
        ensure_quick_ask_window_visible(app);
    }
    // Never hold the ownership lock while emitting: native listeners may cancel
    // synchronously. Revisions make delivery order independent of task ordering.
    if let Some(win) = app.get_webview_window(QUICK_ASK_WINDOW_LABEL) {
        let _ = win.emit(events::EVENT_QUICK_ASK_STATE, snapshot);
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
    let sessions = app.state::<QuickAskSessions>();
    let mut surface = sessions.1.lock().unwrap_or_else(|error| error.into_inner());
    if surface.presenting
        || !surface
            .snapshot
            .as_ref()
            .is_some_and(|snapshot| snapshot.started.is_some())
    {
        return;
    }
    surface.presenting = true;
    let request_id = surface.request_id.clone();
    drop(surface);
    let app = app.clone();
    // Webview construction must not block a GUI event handler (notably WebView2).
    tauri::async_runtime::spawn_blocking(move || {
        let window = crate::overlay::ensure_quick_ask_window(&app);
        let mut failed = window.is_err();
        if let Ok(win) = &window {
            failed |= position_presentation_window(&app, win);
        }
        let handle = app.clone();
        let dispatch = app.run_on_main_thread(move || {
            complete_presentation(&handle, window.ok(), failed, request_id.as_deref());
        });
        complete_presentation_dispatch(&app, dispatch);
    });
}

fn complete_presentation_dispatch(app: &AppHandle, dispatch: tauri::Result<()>) {
    if dispatch.is_err() {
        presentation_failed(app);
    }
}

fn position_presentation_window(app: &AppHandle, window: &tauri::WebviewWindow) -> bool {
    let failed = crate::commands::overlay::position_quick_ask_to_target_monitor(app).is_err();
    if failed {
        // Monitor changes must not strand a usable answer window.
        let _ = window.center();
    }
    failed
}

/// GUI completion retains creation ownership until native cleanup/show is done.
/// It serves the latest request, including a replacement that began mid-build.
fn complete_presentation(
    app: &AppHandle,
    window: Option<tauri::WebviewWindow>,
    mut failed: bool,
    attempted_request_id: Option<&str>,
) {
    let sessions = app.state::<QuickAskSessions>();
    let current = sessions.0.lock().unwrap_or_else(|error| error.into_inner());
    let mut surface = sessions.1.lock().unwrap_or_else(|error| error.into_inner());
    let pending = current.as_ref().is_some_and(|(id, cancel)| {
        surface.request_id.as_deref() == Some(id.as_str()) && !cancel.is_cancelled()
    });
    drop(current);
    let wanted = surface
        .snapshot
        .as_ref()
        .is_some_and(|snapshot| snapshot.started.is_some());
    let retry_replacement =
        window.is_none() && wanted && surface.request_id.as_deref() != attempted_request_id;
    if !wanted && !pending {
        if let Some(window) = window.or_else(|| app.get_webview_window(QUICK_ASK_WINDOW_LABEL)) {
            let _ = window.destroy();
        }
    } else if wanted {
        if let Some(window) = window {
            // Try every affordance even if one platform operation fails.
            failed |= [
                window.set_always_on_top(true),
                window.show(),
                window.set_focus(),
            ]
            .iter()
            .any(Result::is_err);
        }
    }
    surface.presenting = false;
    drop(surface);
    if failed {
        record_window_failure();
    }
    // A replacement coalesced into a failed attempt still owns one fresh try.
    // The same failed request never loops or polls a broken native boundary.
    if retry_replacement {
        ensure_quick_ask_window_visible(app);
    }
}

fn presentation_failed(app: &AppHandle) {
    app.state::<QuickAskSessions>()
        .1
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .presenting = false;
    record_window_failure();
}

fn record_window_failure() {
    log::warn!("Quick Ask window presentation failed");
    crate::runtime_diagnostics::record(crate::runtime_diagnostics::Event::Failure(
        crate::runtime_diagnostics::Failure::QuickAskWindow,
    ));
}
