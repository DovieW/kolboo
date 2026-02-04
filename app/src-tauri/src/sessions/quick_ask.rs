use std::sync::atomic::Ordering;

use tauri::{AppHandle, Emitter, Manager};

use crate::events;
use crate::state::AppState;

pub(crate) const QUICK_ASK_WINDOW_LABEL: &str = "quick_ask";

pub(crate) const EVENT_QUICK_ASK_STARTED: &str = events::EVENT_QUICK_ASK_STARTED;
pub(crate) const EVENT_QUICK_ASK_ANSWER: &str = events::EVENT_QUICK_ASK_ANSWER;

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
    ensure_quick_ask_window_visible(app);
    if let Some(win) = app.get_webview_window(QUICK_ASK_WINDOW_LABEL) {
        let _ = win.emit(event, value);
    } else {
        let _ = app.emit(event, value);
    }
}

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
            45_000,
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
