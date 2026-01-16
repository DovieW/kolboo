use std::sync::atomic::Ordering;

use tauri::{AppHandle, Emitter, Manager};

#[cfg(desktop)]
use tauri::window::Monitor;

use crate::state::AppState;

#[cfg(desktop)]
use tauri_plugin_store::StoreExt;

#[cfg(desktop)]
fn get_setting_from_store<T: serde::de::DeserializeOwned>(
    app: &AppHandle,
    key: &str,
    default: T,
) -> T {
    app.store("settings.json")
        .ok()
        .and_then(|store| store.get(key))
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or(default)
}

// ----------------------------------------------------------------------------
// Overlay monitor targeting
// ----------------------------------------------------------------------------

#[cfg(desktop)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OverlayMonitorTarget {
    Main,
    Cursor,
    ActiveWindow,
}

#[cfg(desktop)]
fn parse_overlay_monitor_target(raw: &str) -> OverlayMonitorTarget {
    match raw.trim() {
        "cursor" => OverlayMonitorTarget::Cursor,
        "active_window" => OverlayMonitorTarget::ActiveWindow,
        // Default + unknown values
        _ => OverlayMonitorTarget::Main,
    }
}

#[cfg(desktop)]
fn get_overlay_monitor_target(app: &AppHandle) -> OverlayMonitorTarget {
    let raw: String = get_setting_from_store(app, "overlay_monitor_target", "main".to_string());
    parse_overlay_monitor_target(raw.as_str())
}

#[cfg(desktop)]
fn monitor_contains_point(monitor: &Monitor, x: i32, y: i32) -> bool {
    let pos = monitor.position();
    let size = monitor.size();
    let left = pos.x;
    let top = pos.y;
    let right = left.saturating_add(size.width as i32);
    let bottom = top.saturating_add(size.height as i32);
    x >= left && x < right && y >= top && y < bottom
}

#[cfg(desktop)]
fn find_monitor_by_point(window: &tauri::WebviewWindow, x: i32, y: i32) -> Option<Monitor> {
    window.available_monitors().ok().and_then(|monitors| {
        monitors
            .into_iter()
            .find(|m| monitor_contains_point(m, x, y))
    })
}

#[cfg(all(desktop, target_os = "windows"))]
fn get_cursor_pos_px() -> Option<(i32, i32)> {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

    unsafe {
        let mut pt = POINT::default();
        if GetCursorPos(&mut pt).is_ok() {
            Some((pt.x, pt.y))
        } else {
            None
        }
    }
}

#[cfg(all(desktop, target_os = "windows"))]
fn get_foreground_window_center_px() -> Option<(i32, i32)> {
    use windows::Win32::Foundation::RECT;
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowRect};

    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() {
            return None;
        }

        let mut rect = RECT::default();
        if GetWindowRect(hwnd, &mut rect).is_err() {
            return None;
        }

        let cx = rect.left.saturating_add(rect.right).saturating_div(2);
        let cy = rect.top.saturating_add(rect.bottom).saturating_div(2);
        Some((cx, cy))
    }
}

#[cfg(desktop)]
fn resolve_target_monitor(window: &tauri::WebviewWindow, app: &AppHandle) -> Option<Monitor> {
    let target = get_overlay_monitor_target(app);

    // Primary intent: honor requested target.
    let preferred = match target {
        OverlayMonitorTarget::Main => window.primary_monitor().ok().flatten(),

        OverlayMonitorTarget::Cursor => {
            #[cfg(all(desktop, target_os = "windows"))]
            {
                if let Some((x, y)) = get_cursor_pos_px() {
                    if let Some(m) = find_monitor_by_point(window, x, y) {
                        return Some(m);
                    }
                }
            }

            None
        }

        OverlayMonitorTarget::ActiveWindow => {
            #[cfg(all(desktop, target_os = "windows"))]
            {
                if let Some((x, y)) = get_foreground_window_center_px() {
                    if let Some(m) = find_monitor_by_point(window, x, y) {
                        return Some(m);
                    }
                }
            }

            None
        }
    };

    if preferred.is_some() {
        return preferred;
    }

    // Fallbacks: preserve previous behavior.
    window
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| window.primary_monitor().ok().flatten())
        .or_else(|| window.available_monitors().ok().and_then(|mut m| m.pop()))
}

fn set_widget_position_impl(app: &AppHandle, position: &str) -> Result<(), String> {
    let Some(window) = app.get_webview_window("overlay") else {
        return Err("Overlay window not found".to_string());
    };

    // Select a monitor based on settings (main/cursor/active window), with fallbacks.
    let monitor = resolve_target_monitor(&window, app).ok_or("No monitor found")?;

    // Use PHYSICAL coordinates for all window placement.
    // This avoids DPI conversion edge cases where logical math + LogicalPosition
    // can lead to double-scaling on Windows (window ends up off-screen).
    let screen_size = monitor.size();
    let screen_pos = monitor.position();
    let scale = monitor.scale_factor();
    let screen_width_px = screen_size.width as f64;
    let screen_height_px = screen_size.height as f64;
    let origin_x_px = screen_pos.x as f64;
    let origin_y_px = screen_pos.y as f64;

    // Get current window size
    let window_size = window.outer_size().map_err(|e| e.to_string())?;
    let mut window_width_px = window_size.width as f64;
    let mut window_height_px = window_size.height as f64;

    // When the window is hidden (recording_only/never), some platforms can report
    // a near-zero outer_size. Using that would place the window partially or fully
    // off-screen. Prefer a conservative estimate that matches the overlay UI.
    if window_width_px < 20.0 || window_height_px < 20.0 {
        window_width_px = (224.0 * scale).round();
        window_height_px = (56.0 * scale).round();
    }

    // Calculate margins (pixels from edge)
    let margin_px = (50.0 * scale).round();

    let (mut x_px, mut y_px) = match position {
        "top-left" => (origin_x_px + margin_px, origin_y_px + margin_px),
        "top-center" => (
            origin_x_px + (screen_width_px - window_width_px) / 2.0,
            origin_y_px + margin_px,
        ),
        "top-right" => (
            origin_x_px + screen_width_px - window_width_px - margin_px,
            origin_y_px + margin_px,
        ),
        "center" => (
            origin_x_px + (screen_width_px - window_width_px) / 2.0,
            origin_y_px + (screen_height_px - window_height_px) / 2.0,
        ),
        "bottom-left" => (
            origin_x_px + margin_px,
            origin_y_px + screen_height_px - window_height_px - margin_px,
        ),
        "bottom-center" => (
            origin_x_px + (screen_width_px - window_width_px) / 2.0,
            origin_y_px + screen_height_px - window_height_px - margin_px,
        ),
        "bottom-right" => (
            origin_x_px + screen_width_px - window_width_px - margin_px,
            origin_y_px + screen_height_px - window_height_px - margin_px,
        ),
        _ => return Err(format!("Invalid widget position: {}", position)),
    };

    // Clamp to screen bounds with a small margin. This avoids cases where size estimates
    // (or unusual DPI/taskbar geometry) would push the window slightly off-screen.
    let clamp_margin_px = (12.0 * scale).round();
    let min_x_px = origin_x_px + clamp_margin_px;
    let min_y_px = origin_y_px + clamp_margin_px;
    let max_x_px =
        (origin_x_px + screen_width_px - window_width_px - clamp_margin_px).max(min_x_px);
    let max_y_px =
        (origin_y_px + screen_height_px - window_height_px - clamp_margin_px).max(min_y_px);
    x_px = x_px.clamp(min_x_px, max_x_px);
    y_px = y_px.clamp(min_y_px, max_y_px);

    window
        .set_position(tauri::Position::Physical(tauri::PhysicalPosition {
            x: x_px.round() as i32,
            y: y_px.round() as i32,
        }))
        .map_err(|e| e.to_string())?;

    log::info!(
        "Widget position set to {} at logical=({:.1}, {:.1}) physical=({:.0}, {:.0}) [monitor_origin_logical=({:.1}, {:.1}) physical=({:.0}, {:.0}), scale={}]",
        position,
        x_px / scale,
        y_px / scale,
        x_px,
        y_px,
        origin_x_px / scale,
        origin_y_px / scale,
        origin_x_px,
        origin_y_px,
        scale
    );
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

    if let Some(window) = app.get_webview_window("overlay") {
        // Bump the epoch so any previously-scheduled delayed hides know they are outdated.
        app.state::<AppState>()
            .overlay_visibility_epoch
            .fetch_add(1, Ordering::SeqCst);

        let visible_before = window.is_visible().ok();
        log::info!(
            "[overlay] show requested (mode={}, visible_before={:?})",
            overlay_mode,
            visible_before
        );

        window.show().map_err(|e| e.to_string())?;

        // If the frontend hasn't mounted yet (or resize calls were missed), the overlay can
        // remain at its tiny initial size and appear "missing" near the screen edge.
        // In recording-only mode we always want the expanded pill while visible.
        if overlay_mode == "recording_only" {
            let _ = window.set_size(tauri::Size::Logical(tauri::LogicalSize {
                width: 224.0,
                height: 56.0,
            }));
        }

        // Recovery: if the overlay was previously on a disconnected monitor, show() may succeed
        // but the window can still be effectively invisible. Centering is a good best-effort
        // recovery before we snap to the user's preferred position.
        let _ = window.unminimize();
        let _ = window.center();

        // Best-effort: some environments can lose the always-on-top flag across hide/show.
        let _ = window.set_always_on_top(true);

        // For non-always modes, snap after show/center so current_monitor() resolves reliably.
        if overlay_mode != "always" {
            if let Err(e) = snap_overlay_to_saved_position(app) {
                log::warn!("Failed to snap overlay position on show: {}", e);
            }
        }

        // Diagnostics: log final geometry + monitor. On Windows with DPI/multi-monitor,
        // a window can be technically visible but placed somewhere unexpected.
        let pos = window.outer_position().ok();
        let outer = window.outer_size().ok();
        let inner = window.inner_size().ok();
        let scale = window.scale_factor().ok();
        let mon = window.current_monitor().ok().flatten();
        if let Some(m) = mon {
            log::info!(
                "[overlay] final geom pos={:?} outer={:?} inner={:?} scale={:?} monitor={{name={:?}, pos={:?}, size={:?}, scale={}}}",
                pos,
                outer,
                inner,
                scale,
                m.name(),
                m.position(),
                m.size(),
                m.scale_factor()
            );
        } else {
            log::info!(
                "[overlay] final geom pos={:?} outer={:?} inner={:?} scale={:?} monitor=<none>",
                pos,
                outer,
                inner,
                scale
            );
        }

        let visible_after = window.is_visible().ok();
        log::info!(
            "[overlay] show complete (visible_after={:?})",
            visible_after
        );
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
        // We position using *outer* geometry (position + size) in PHYSICAL pixels.
        // Using logical floats here can accumulate rounding error across repeated
        // size transitions (notably hover open/close), causing the overlay to drift.
        let prev = if let (Ok(pos), Ok(outer_size), Ok(inner_size)) = (
            window.outer_position(),
            window.outer_size(),
            window.inner_size(),
        ) {
            let scale = window.scale_factor().unwrap_or(1.0);
            let inner_w = inner_size.width as f64 / scale;
            let inner_h = inner_size.height as f64 / scale;
            Some((pos, outer_size, inner_w, inner_h))
        } else {
            None
        };

        // Set the new size
        window
            .set_size(tauri::Size::Logical(tauri::LogicalSize { width, height }))
            .map_err(|e| e.to_string())?;

        // For the fixed collapsed/expanded toggle sizes (56x56 <-> expanded x 56), keep the window
        // center fixed so the expanded state grows out from the collapsed widget's location.
        // Avoid clamping in this path: users prefer slight off-screen over "push away" drift.
        let is_fixed_toggle_size = (height - 56.0).abs() < 0.5
            && ((width - 56.0).abs() < 0.5
                || (width - 224.0).abs() < 0.5
                || (width - 264.0).abs() < 0.5);

        if let Some((prev_pos_px, prev_outer_px, prev_inner_w, prev_inner_h)) = prev {
            // Use actual outer size after resize when possible; otherwise estimate from the
            // previous decoration delta.
            let new_outer_px = match window.outer_size() {
                Ok(sz) => sz,
                Err(_) => {
                    let scale = window.scale_factor().unwrap_or(1.0);
                    let prev_outer_w = prev_outer_px.width as f64 / scale;
                    let prev_outer_h = prev_outer_px.height as f64 / scale;
                    let decor_w = (prev_outer_w - prev_inner_w).max(0.0);
                    let decor_h = (prev_outer_h - prev_inner_h).max(0.0);
                    tauri::PhysicalSize {
                        width: ((width + decor_w) * scale).round().max(1.0) as u32,
                        height: ((height + decor_h) * scale).round().max(1.0) as u32,
                    }
                }
            };

            let mut x_px: i32;
            let mut y_px: i32;

            // When temporarily showing a hover panel above the overlay widget we resize the window
            // to a taller size. Preserve the widget's *bottom-center* across that transition so
            // it doesn't jump away from the cursor (which would instantly cancel hover).
            let was_compact = (prev_inner_h - 56.0).abs() < 1.2;
            let is_compact = (height - 56.0).abs() < 0.8;
            let is_hover_panel_transition =
                (was_compact && !is_compact) || (!was_compact && is_compact);

            if is_fixed_toggle_size {
                let cx_px = prev_pos_px.x + (prev_outer_px.width as i32 / 2);
                let cy_px = prev_pos_px.y + (prev_outer_px.height as i32 / 2);
                x_px = cx_px - (new_outer_px.width as i32 / 2);
                y_px = cy_px - (new_outer_px.height as i32 / 2);
            } else if is_hover_panel_transition {
                let bc_x_px = prev_pos_px.x + (prev_outer_px.width as i32 / 2);
                let bc_y_px = prev_pos_px.y + prev_outer_px.height as i32;
                x_px = bc_x_px - (new_outer_px.width as i32 / 2);
                y_px = bc_y_px - new_outer_px.height as i32;
            } else {
                // Default: preserve top-left and clamp to screen bounds.
                x_px = prev_pos_px.x;
                y_px = prev_pos_px.y;

                if let Ok(Some(monitor)) = window.current_monitor() {
                    let screen_size = monitor.size();
                    let screen_pos = monitor.position();
                    let scale = monitor.scale_factor();
                    let screen_w_px = (screen_size.width as f64).round() as i32;
                    let screen_h_px = (screen_size.height as f64).round() as i32;
                    let margin_px = (12.0 * scale).round() as i32;

                    let min_x_px = screen_pos.x + margin_px;
                    let min_y_px = screen_pos.y + margin_px;
                    let max_x_px =
                        (screen_pos.x + screen_w_px - new_outer_px.width as i32 - margin_px)
                            .max(min_x_px);
                    let max_y_px =
                        (screen_pos.y + screen_h_px - new_outer_px.height as i32 - margin_px)
                            .max(min_y_px);

                    x_px = x_px.clamp(min_x_px, max_x_px);
                    y_px = y_px.clamp(min_y_px, max_y_px);
                }
            }

            window
                .set_position(tauri::Position::Physical(tauri::PhysicalPosition {
                    x: x_px,
                    y: y_px,
                }))
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn show_overlay(app: AppHandle) -> Result<(), String> {
    #[cfg(desktop)]
    {
        show_overlay_with_reset_if_not_always(&app)
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

    // Best-effort: also hide the hover panel window so it doesn't orphan.
    if let Some(window) = app.get_webview_window("overlay_hover") {
        let _ = window.hide();
    }
    Ok(())
}

#[tauri::command]
pub async fn show_overlay_hover(app: AppHandle) -> Result<(), String> {
    let Some(overlay) = app.get_webview_window("overlay") else {
        return Err("Overlay window not found".to_string());
    };
    let Some(hover) = app.get_webview_window("overlay_hover") else {
        return Err("Overlay hover window not found".to_string());
    };

    app.state::<AppState>()
        .overlay_hover_epoch
        .fetch_add(1, Ordering::SeqCst);

    // Fixed hover panel size (logical).
    let hover_w = 320.0;
    let hover_h = 220.0;
    let gap = 10.0;

    // Compute in physical px to avoid cumulative rounding drift on Windows.
    let scale = overlay.scale_factor().unwrap_or(1.0);
    let overlay_pos = overlay.outer_position().map_err(|e| e.to_string())?;
    let overlay_size = overlay.outer_size().map_err(|e| e.to_string())?;

    // Ensure hover window is the expected size.
    let _ = hover.set_size(tauri::Size::Logical(tauri::LogicalSize {
        width: hover_w,
        height: hover_h,
    }));

    let hover_w_px = (hover_w * scale).round() as i32;
    let hover_h_px = (hover_h * scale).round() as i32;
    let gap_px = (gap * scale).round() as i32;

    let overlay_center_x_px = overlay_pos.x + (overlay_size.width as i32 / 2);
    let overlay_top_y_px = overlay_pos.y;
    let overlay_bottom_y_px = overlay_pos.y + overlay_size.height as i32;

    // Preferred: above the overlay.
    let mut x_px = overlay_center_x_px - (hover_w_px / 2);
    let mut y_px = overlay_top_y_px - hover_h_px - gap_px;

    // Clamp to the monitor bounds (and flip below if we don't fit above).
    if let Ok(Some(monitor)) = overlay.current_monitor() {
        let screen_size = monitor.size();
        let screen_pos = monitor.position();
        let screen_w_px = screen_size.width as i32;
        let screen_h_px = screen_size.height as i32;
        let origin_x_px = screen_pos.x;
        let origin_y_px = screen_pos.y;

        // A small margin so the panel doesn't hug the monitor edge.
        let margin_px = (12.0 * scale).round() as i32;

        let min_x = origin_x_px + margin_px;
        let max_x = (origin_x_px + screen_w_px - hover_w_px - margin_px).max(min_x);

        // If the "above" placement would go off the top, try placing below.
        let min_y = origin_y_px + margin_px;
        let max_y = (origin_y_px + screen_h_px - hover_h_px - margin_px).max(min_y);

        if y_px < min_y {
            // Place below the overlay widget.
            y_px = overlay_bottom_y_px + gap_px;
        }

        x_px = x_px.clamp(min_x, max_x);
        y_px = y_px.clamp(min_y, max_y);
    }

    hover
        .set_position(tauri::Position::Physical(tauri::PhysicalPosition {
            x: x_px,
            y: y_px,
        }))
        .map_err(|e| e.to_string())?;

    // Keep above other windows.
    let _ = hover.set_always_on_top(true);
    hover.show().map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn hide_overlay_hover(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("overlay_hover") {
        window.hide().map_err(|e| e.to_string())?;
    }

    // Bump epoch so any pending hides are considered stale.
    app.state::<AppState>()
        .overlay_hover_epoch
        .fetch_add(1, Ordering::SeqCst);

    Ok(())
}

#[tauri::command]
pub async fn schedule_hide_overlay_hover(app: AppHandle, delay_ms: u64) -> Result<(), String> {
    let expected_epoch = app
        .state::<AppState>()
        .overlay_hover_epoch
        .load(Ordering::SeqCst);

    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move {
        let delay = std::time::Duration::from_millis(delay_ms);
        tokio::time::sleep(delay).await;

        let current_epoch = app_clone
            .state::<AppState>()
            .overlay_hover_epoch
            .load(Ordering::SeqCst);

        if current_epoch != expected_epoch {
            return;
        }

        if let Some(window) = app_clone.get_webview_window("overlay_hover") {
            let _ = window.hide();
        }
    });

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

/// Best-effort: position the Quick Ask overlay window to the configured monitor.
///
/// Quick Ask is implemented as a transparent always-on-top window that covers a full monitor.
#[cfg(desktop)]
pub fn position_quick_ask_to_target_monitor(app: &AppHandle) -> Result<(), String> {
    let Some(win) = app.get_webview_window("quick_ask") else {
        return Err("Quick Ask window not found".to_string());
    };

    let monitor = resolve_target_monitor(&win, app).ok_or("No monitor found")?;
    let size = monitor.size();
    let pos = monitor.position();

    // Use PHYSICAL coordinates to avoid DPI conversion edge cases (especially on Windows).
    win.set_size(tauri::Size::Physical(tauri::PhysicalSize {
        width: size.width,
        height: size.height,
    }))
    .map_err(|e| e.to_string())?;
    win.set_position(tauri::Position::Physical(tauri::PhysicalPosition {
        x: pos.x,
        y: pos.y,
    }))
    .map_err(|e| e.to_string())?;

    Ok(())
}
