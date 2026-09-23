//! Tauri commands for OS window/process information.
//!
//! Used by the per-program rewrite prompt profiles UI.

use crate::windows_apps;

/// List currently open top-level windows.
///
/// By default this minimizes sensitive data and does not return window titles.
///
/// To include window titles, pass `include_titles: true`.
#[tauri::command]
pub async fn list_open_windows(
    include_titles: Option<bool>,
) -> Result<Vec<windows_apps::OpenWindowInfo>, String> {
    tokio::task::spawn_blocking(move || {
        windows_apps::list_open_windows(include_titles.unwrap_or(false))
    })
    .await
    .map_err(|_| "Open-program query stopped unexpectedly.".to_string())?
}

/// Get the executable path of the current foreground process (active window).
#[tauri::command]
pub async fn get_foreground_process_path() -> Option<String> {
    tokio::task::spawn_blocking(windows_apps::get_foreground_process_path)
        .await
        .ok()
        .flatten()
}
