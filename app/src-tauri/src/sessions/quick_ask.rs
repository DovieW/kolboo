use tauri::{AppHandle, Emitter, Manager};

/// Emit an event intended for the Quick Ask window.
///
/// If the `quick_ask` window exists, emit directly to it (so only that surface updates).
/// Otherwise, fall back to emitting at the app level.
pub(crate) fn emit_to_quick_ask<T: serde::Serialize>(app: &AppHandle, event: &str, payload: T) {
    let Ok(value) = serde_json::to_value(payload) else {
        return;
    };
    if let Some(win) = app.get_webview_window("quick_ask") {
        let _ = win.emit(event, value);
    } else {
        let _ = app.emit(event, value);
    }
}

/// Best-effort: make sure the Quick Ask window is visible and focused.
///
/// This is used when we want the user to actually see streaming/error states.
pub(crate) fn ensure_quick_ask_window_visible(app: &AppHandle) {
    let _ = crate::commands::overlay::position_quick_ask_to_target_monitor(app);
    if let Some(win) = app.get_webview_window("quick_ask") {
        let _ = win.set_always_on_top(true);
        let _ = win.show();
        let _ = win.set_focus();
    }
}
