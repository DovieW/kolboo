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
pub fn list_open_windows(include_titles: Option<bool>) -> Vec<windows_apps::OpenWindowInfo> {
    windows_apps::list_open_windows(include_titles.unwrap_or(false))
}

/// Get the executable path of the current foreground process (active window).
#[tauri::command]
pub fn get_foreground_process_path() -> Option<String> {
    windows_apps::get_foreground_process_path()
}
