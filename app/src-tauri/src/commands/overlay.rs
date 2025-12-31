use std::sync::atomic::Ordering;

use tauri::{AppHandle, Emitter, Manager};

use crate::state::AppState;

#[cfg(desktop)]
use tauri_plugin_store::StoreExt;

#[cfg(desktop)]
fn get_setting_from_store<T: serde::de::DeserializeOwned>(app: &AppHandle, key: &str, default: T) -> T {
    app.store("settings.json")
        .ok()
        .and_then(|store| store.get(key))
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or(default)
}

fn set_widget_position_impl(app: &AppHandle, position: &str) -> Result<(), String> {
    let Some(window) = app.get_webview_window("overlay") else {
        return Err("Overlay window not found".to_string());
    };

    let monitor = window
        .current_monitor()
        .map_err(|e| e.to_string())?
        .ok_or("No monitor found")?;

    let screen_size = monitor.size();
    let scale = monitor.scale_factor();
    let screen_width = screen_size.width as f64 / scale;
    let screen_height = screen_size.height as f64 / scale;

    // Get current window size
    let window_size = window.outer_size().map_err(|e| e.to_string())?;
    let window_width = window_size.width as f64 / scale;
    let window_height = window_size.height as f64 / scale;

    // Calculate margins (pixels from edge)
    let margin = 50.0;

    let (x, y) = match position {
        "top-left" => (margin, margin),
        "top-center" => ((screen_width - window_width) / 2.0, margin),
        "top-right" => (screen_width - window_width - margin, margin),
        "center" => (
            (screen_width - window_width) / 2.0,
            (screen_height - window_height) / 2.0,
        ),
        "bottom-left" => (margin, screen_height - window_height - margin),
        "bottom-center" => (
            (screen_width - window_width) / 2.0,
            screen_height - window_height - margin,
        ),
        "bottom-right" => (
            screen_width - window_width - margin,
            screen_height - window_height - margin,
        ),
        _ => return Err(format!("Invalid widget position: {}", position)),
    };

    window
        .set_position(tauri::Position::Logical(tauri::LogicalPosition { x, y }))
        .map_err(|e| e.to_string())?;

    log::info!("Widget position set to {} at ({}, {})", position, x, y);
    Ok(())
}

/// Best-effort: snap the overlay window back to the saved preset position.
///
/// Intended for cases where the overlay is not always visible (recording-only/never) and
/// the user may have dragged it away since the last time it was shown.
#[cfg(desktop)]
pub fn snap_overlay_to_saved_position(app: &AppHandle) -> Result<(), String> {
    let position: String =
        get_setting_from_store(app, "widget_position", "bottom-center".to_string());
    set_widget_position_impl(app, position.as_str())
}

/// Show the overlay window and, if the current mode is not "always", reset the window
/// back to the saved preset position.
#[cfg(desktop)]
pub fn show_overlay_with_reset_if_not_always(app: &AppHandle) -> Result<(), String> {
    let overlay_mode: String =
        get_setting_from_store(app, "overlay_mode", "recording_only".to_string());

    if overlay_mode != "always" {
        if let Err(e) = snap_overlay_to_saved_position(app) {
            log::warn!("Failed to snap overlay position on show: {}", e);
        }
    }

    if let Some(window) = app.get_webview_window("overlay") {
        // Bump the epoch so any previously-scheduled delayed hides know they are outdated.
        app.state::<AppState>()
            .overlay_visibility_epoch
            .fetch_add(1, Ordering::SeqCst);
        window.show().map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[tauri::command]
pub async fn resize_overlay(app: AppHandle, width: f64, height: f64) -> Result<(), String> {
    // Enforce minimum dimensions to prevent invisible window
    let min_size = 48.0;
    let width = width.max(min_size);
    let height = height.max(min_size);

    if let Some(window) = app.get_webview_window("overlay") {
        // We position using *outer* geometry (position + size), because that's what the OS
        // uses for window placement. On Windows/macOS the outer size includes decorations,
        // so using the requested logical size directly can cause subtle drift.
        let prev = if let (Ok(pos), Ok(outer_size), Ok(inner_size)) =
            (window.outer_position(), window.outer_size(), window.inner_size())
        {
            let scale = window.scale_factor().unwrap_or(1.0);
            let x = pos.x as f64 / scale;
            let y = pos.y as f64 / scale;
            let w = outer_size.width as f64 / scale;
            let h = outer_size.height as f64 / scale;
            let inner_w = inner_size.width as f64 / scale;
            let inner_h = inner_size.height as f64 / scale;
            Some((x, y, w, h, inner_w, inner_h, scale))
        } else {
            None
        };

        // Set the new size
        window
            .set_size(tauri::Size::Logical(tauri::LogicalSize { width, height }))
            .map_err(|e| e.to_string())?;

        // For the fixed collapsed/expanded toggle sizes (56x56 <-> 264x56), keep the window
        // center fixed so the expanded state grows out from the collapsed widget's location.
        // Avoid clamping in this path: users prefer slight off-screen over "push away" drift.
        let is_fixed_toggle_size = (height - 56.0).abs() < 0.5
            && ((width - 56.0).abs() < 0.5 || (width - 264.0).abs() < 0.5);

        if let Some((prev_x, prev_y, prev_outer_w, prev_outer_h, prev_inner_w, prev_inner_h, scale)) =
            prev
        {
            // Try to use the actual outer size after resize (most accurate).
            // Fall back to estimating using the previous decoration delta if needed.
            let (new_outer_w, new_outer_h) = match window.outer_size() {
                Ok(sz) => (sz.width as f64 / scale, sz.height as f64 / scale),
                Err(_) => {
                    // Estimate decoration delta using the pre-resize outer vs inner sizes.
                    let decor_w = (prev_outer_w - prev_inner_w).max(0.0);
                    let decor_h = (prev_outer_h - prev_inner_h).max(0.0);
                    (width + decor_w, height + decor_h)
                }
            };

            let mut x;
            let mut y;

            if is_fixed_toggle_size {
                let cx = prev_x + prev_outer_w / 2.0;
                let cy = prev_y + prev_outer_h / 2.0;
                x = cx - new_outer_w / 2.0;
                y = cy - new_outer_h / 2.0;
            } else {
                // Default: preserve top-left and clamp to screen bounds.
                x = prev_x;
                y = prev_y;

                if let Ok(Some(monitor)) = window.current_monitor() {
                    let screen_size = monitor.size();
                    let scale = monitor.scale_factor();
                    let screen_width = screen_size.width as f64 / scale;
                    let screen_height = screen_size.height as f64 / scale;

                    let margin = 12.0;
                    let max_x = (screen_width - new_outer_w - margin).max(margin);
                    let max_y = (screen_height - new_outer_h - margin).max(margin);

                    x = x.clamp(margin, max_x);
                    y = y.clamp(margin, max_y);
                }
            }

            window
                .set_position(tauri::Position::Logical(tauri::LogicalPosition { x, y }))
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn show_overlay(app: AppHandle) -> Result<(), String> {
    #[cfg(desktop)]
    {
        return show_overlay_with_reset_if_not_always(&app);
    }

    #[cfg(not(desktop))]
    {
        if let Some(window) = app.get_webview_window("overlay") {
            window.show().map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}

#[tauri::command]
pub async fn hide_overlay(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("overlay") {
        window.hide().map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Set overlay mode: "always", "never", or "recording_only"
#[tauri::command]
pub async fn set_overlay_mode(app: AppHandle, mode: String) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("overlay") {
        match mode.as_str() {
            "always" => {
                window.show().map_err(|e| e.to_string())?;
            }
            "never" => {
                // Ask the frontend to animate out before we hide.
                let _ = app.emit("overlay-hide-requested", ());

                // Fallback hide (in case the overlay UI isn't ready to handle the event).
                let window_clone = window.clone();
                let app_clone = app.clone();
                let expected_epoch = app
                    .state::<AppState>()
                    .overlay_visibility_epoch
                    .load(Ordering::SeqCst);
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_millis(220)).await;
                    let current_mode: String = get_setting_from_store(
                        &app_clone,
                        "overlay_mode",
                        "recording_only".to_string(),
                    );
                    let current_epoch = app_clone
                        .state::<AppState>()
                        .overlay_visibility_epoch
                        .load(Ordering::SeqCst);
                    if current_mode == "never" && current_epoch == expected_epoch {
                        let _ = window_clone.hide();
                    }
                });
            }
            "recording_only" => {
                // Hide initially, will be shown when recording starts
                let _ = app.emit("overlay-hide-requested", ());
                let window_clone = window.clone();
                let app_clone = app.clone();
                let expected_epoch = app
                    .state::<AppState>()
                    .overlay_visibility_epoch
                    .load(Ordering::SeqCst);
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_millis(220)).await;
                    let current_mode: String = get_setting_from_store(
                        &app_clone,
                        "overlay_mode",
                        "recording_only".to_string(),
                    );
                    let current_epoch = app_clone
                        .state::<AppState>()
                        .overlay_visibility_epoch
                        .load(Ordering::SeqCst);
                    if current_mode == "recording_only" && current_epoch == expected_epoch {
                        let _ = window_clone.hide();
                    }
                });
            }
            _ => {
                return Err(format!("Invalid overlay mode: {}", mode));
            }
        }
    }
    Ok(())
}

/// Set overlay widget position on screen
#[tauri::command]
pub async fn set_widget_position(app: AppHandle, position: String) -> Result<(), String> {
    set_widget_position_impl(&app, position.as_str())
}
