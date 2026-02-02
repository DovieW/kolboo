use tauri::AppHandle;
#[cfg(target_os = "windows")]
use tauri::Manager;

use crate::commands::{CommandError, CommandResult};

#[allow(unused_imports)]
pub use crate::text::inject::{
    copy_to_clipboard, output_text_with_mode, output_text_with_mode_options,
    paste_and_keep_clipboard, type_as_keystrokes, type_text_blocking_with_options, OutputMode,
};

#[cfg(not(target_os = "windows"))]
#[allow(unused_imports)]
pub use crate::text::inject::type_text_blocking;

#[cfg(desktop)]
#[allow(unused_imports)]
pub use crate::text::selection_probe::{
    probe_selected_text_via_copy, probe_selected_text_via_copy_with_app, ContextGrabMethod,
};

const SERVER_URL: &str = "http://127.0.0.1:8765";

#[tauri::command]
pub async fn get_server_url() -> String {
    SERVER_URL.to_string()
}

#[cfg(target_os = "windows")]
#[tauri::command]
pub async fn type_text(app: AppHandle, text: String) -> CommandResult<()> {
    // Windows: use UIA-first insertion ladder (with fallbacks).
    let snapshot = app
        .state::<crate::state::AppState>()
        .windows_text_target_snapshot
        .lock()
        .ok()
        .and_then(|mut g| g.take());
    let smart_paste_protection: bool =
        crate::get_setting_from_store(&app, "output_smart_paste_protection", false);

    crate::windows_uia::insert::insert_text_with_snapshot(
        &app,
        &text,
        snapshot,
        true,
        true,
        smart_paste_protection,
    )
    .map_err(CommandError::from)?;

    Ok(())
}

#[cfg(not(target_os = "windows"))]
#[tauri::command]
pub async fn type_text(app: AppHandle, text: String) -> CommandResult<()> {
    use crate::text::inject::run_with_output_injection_lock;
    use std::sync::mpsc;

    // macOS HIToolbox APIs (used by enigo) must run on the main thread
    // Use a channel to get the result back from the main thread
    let (tx, rx) = mpsc::channel::<Result<(), CommandError>>();

    app.run_on_main_thread(move || {
        // Serialize output across all modes to avoid interleaving key events.
        let _ = tx.send(
            run_with_output_injection_lock(|| type_text_blocking(&text, false))
                .map_err(CommandError::from),
        );
    })
    .map_err(|e| CommandError::from(e.to_string()))?;

    // Wait for result from main thread
    let result = rx.recv().map_err(|e| CommandError::from(e.to_string()))?;
    match result {
        Ok(()) => Ok(()),
        Err(error) => Err(error),
    }
}
