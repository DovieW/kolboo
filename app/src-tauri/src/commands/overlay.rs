use std::sync::atomic::Ordering;
use std::time::{SystemTime, UNIX_EPOCH};

use tauri::{AppHandle, Emitter, Manager};

#[cfg(desktop)]
use tauri::window::Monitor;

use crate::commands::CommandResult;
use crate::events;
#[cfg(desktop)]
use crate::overlay::layout::{self, LogicalSize, PhysicalRect, WidgetAnchor, WidgetLayout};
use crate::pipeline;
use crate::state::AppState;

#[cfg(desktop)]
use tauri_plugin_store::StoreExt;

#[cfg(desktop)]
fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(desktop)]
fn overlay_frontend_needs_reload(
    last_ready_ms: u64,
    age_ms: u64,
    stale_after_ms: Option<u64>,
) -> bool {
    last_ready_ms == 0
        || stale_after_ms
            .map(|threshold_ms| age_ms > threshold_ms)
            .unwrap_or(false)
}

#[cfg(desktop)]
pub fn maybe_reload_overlay_webview(
    app: &AppHandle,
    label: &str,
    last_ready_ms: u64,
    stale_after_ms: Option<u64>,
    reason: &str,
) {
    let age_ms = now_ms().saturating_sub(last_ready_ms);
    let stale = overlay_frontend_needs_reload(last_ready_ms, age_ms, stale_after_ms);
    if !stale {
        return;
    }

    let Some(window) = app.get_webview_window(label) else {
        return;
    };

    log::warn!(
        "[overlay] frontend stale, reloading (label={}, last_ready_ms={}, age_ms={}, reason={})",
        label,
        last_ready_ms,
        age_ms,
        reason
    );
    let _ = window.eval("window.location.reload()");
}

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

#[cfg(all(desktop, target_os = "windows"))]
fn monitor_contains_point(monitor: &Monitor, x: i32, y: i32) -> bool {
    let pos = monitor.position();
    let size = monitor.size();
    let left = pos.x;
    let top = pos.y;
    let right = left.saturating_add(size.width as i32);
    let bottom = top.saturating_add(size.height as i32);
    x >= left && x < right && y >= top && y < bottom
}

#[cfg(all(desktop, target_os = "windows"))]
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

#[cfg(desktop)]
fn monitor_work_area(monitor: &Monitor) -> PhysicalRect {
    let work_area = monitor.work_area();
    if work_area.size.width > 0 && work_area.size.height > 0 {
        return PhysicalRect::new(
            work_area.position.x,
            work_area.position.y,
            work_area.size.width,
            work_area.size.height,
        );
    }

    // Defensive fallback for window managers that do not report a work area.
    PhysicalRect::new(
        monitor.position().x,
        monitor.position().y,
        monitor.size().width,
        monitor.size().height,
    )
}

#[cfg(desktop)]
fn format_monitor_summary(monitor: Option<&Monitor>) -> String {
    match monitor {
        Some(m) => format!(
            "name={:?}, pos={:?}, size={:?}, work_area={:?}, scale={}",
            m.name(),
            m.position(),
            m.size(),
            m.work_area(),
            m.scale_factor()
        ),
        None => "<none>".to_string(),
    }
}

#[cfg(all(desktop, target_os = "windows"))]
fn raise_overlay_without_focus(window: &tauri::WebviewWindow) -> Result<(), String> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        SetWindowPos, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOOWNERZORDER, SWP_NOSIZE,
        SWP_SHOWWINDOW,
    };

    let hwnd: HWND = window.hwnd().map_err(|e| e.to_string())?;

    let flags = SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW | SWP_NOOWNERZORDER;

    unsafe {
        SetWindowPos(hwnd, Some(HWND_TOPMOST), 0, 0, 0, 0, flags).map_err(|err| err.to_string())
    }
}

#[cfg(desktop)]
fn saved_widget_anchor(app: &AppHandle) -> CommandResult<WidgetAnchor> {
    let value: String = get_setting_from_store(app, "widget_position", "bottom-center".to_string());
    WidgetAnchor::parse(value.as_str())
        .ok_or_else(|| format!("Invalid widget position: {value}").into())
}

/// Apply one complete native layout transaction for the main overlay.
///
/// This is the only backend path that changes the main overlay's native size or
/// position. Geometry is calculated from semantic state, monitor work area and
/// DPI, never from the previous window rectangle.
#[cfg(desktop)]
pub(crate) fn apply_overlay_layout(
    app: &AppHandle,
    widget_layout: WidgetLayout,
) -> CommandResult<()> {
    let anchor = saved_widget_anchor(app)?;
    apply_overlay_layout_at_anchor(app, widget_layout, anchor, false)
}

#[cfg(desktop)]
fn apply_overlay_layout_at_anchor(
    app: &AppHandle,
    widget_layout: WidgetLayout,
    anchor: WidgetAnchor,
    force: bool,
) -> CommandResult<()> {
    let Some(window) = app.get_webview_window("overlay") else {
        return Err("Overlay window not found".to_string().into());
    };

    let app_state = app.state::<AppState>();
    let _layout_guard = app_state
        .overlay_layout_lock
        .lock()
        .map_err(|_| "Overlay layout lock poisoned".to_string())?;

    let monitor = resolve_target_monitor(&window, app).ok_or("No monitor found")?;
    let scale = monitor.scale_factor();
    let work_area = monitor_work_area(&monitor);
    let rect = layout::widget_rect(work_area, scale, widget_layout, anchor);

    app_state
        .overlay_expanded
        .store(widget_layout == WidgetLayout::Expanded, Ordering::SeqCst);

    let rect_key = (rect.x, rect.y, rect.width, rect.height);
    {
        let last_rect = app_state
            .overlay_last_applied_rect
            .lock()
            .map_err(|_| "Overlay rectangle cache lock poisoned".to_string())?;
        if !force && last_rect.as_ref() == Some(&rect_key) {
            log::trace!(
                "[overlay] identical layout skipped (layout={:?}, anchor={:?}, rect={:?})",
                widget_layout,
                anchor,
                rect
            );
            return Ok(());
        }
    }

    // Physical size and position share the same coordinate system and are
    // issued under one lock. Borderless utility windows have no intended
    // decoration delta, so the semantic CSS size maps directly through DPI.
    window
        .set_size(tauri::Size::Physical(tauri::PhysicalSize {
            width: rect.width,
            height: rect.height,
        }))
        .map_err(|e| e.to_string())?;
    window
        .set_position(tauri::Position::Physical(tauri::PhysicalPosition {
            x: rect.x,
            y: rect.y,
        }))
        .map_err(|e| e.to_string())?;

    *app_state
        .overlay_last_applied_rect
        .lock()
        .map_err(|_| "Overlay rectangle cache lock poisoned".to_string())? = Some(rect_key);

    log::debug!(
        "[overlay] layout applied (layout={:?}, anchor={:?}, rect={:?}, monitor={})",
        widget_layout,
        anchor,
        rect,
        format_monitor_summary(Some(&monitor))
    );
    Ok(())
}

/// Show the overlay window and, if the current mode is not "always", reset the window
/// back to the saved preset position.
#[cfg(desktop)]
pub fn show_overlay_with_reset_if_not_always(app: &AppHandle) -> CommandResult<()> {
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

        let last_ready_ms = app
            .state::<AppState>()
            .overlay_frontend_ready_at_ms
            .load(Ordering::SeqCst);
        // The main overlay intentionally reports readiness once when mounted.
        // Reload only if it never mounted; elapsed time alone does not mean it is stale.
        maybe_reload_overlay_webview(app, "overlay", last_ready_ms, None, "show");

        // Resolve the complete rectangle while hidden. Non-always modes are
        // intentionally expanded whenever they are force-shown for recording
        // or an error; always mode follows the frontend's semantic state.
        let widget_layout = if overlay_mode == "always" {
            WidgetLayout::from_expanded(
                app.state::<AppState>()
                    .overlay_expanded
                    .load(Ordering::SeqCst),
            )
        } else {
            WidgetLayout::Expanded
        };
        let anchor = saved_widget_anchor(app)?;
        let force_layout = overlay_mode != "always" || visible_before != Some(true);
        apply_overlay_layout_at_anchor(app, widget_layout, anchor, force_layout)?;

        let _ = window.unminimize();
        let _ = window.set_always_on_top(true);
        window.show().map_err(|e| e.to_string())?;

        #[cfg(all(desktop, target_os = "windows"))]
        let raise_status = match raise_overlay_without_focus(&window) {
            Ok(()) => "ok",
            Err(err) => {
                log::warn!("[overlay] raise without focus failed: {}", err);
                "err"
            }
        };
        #[cfg(not(all(desktop, target_os = "windows")))]
        let raise_status = "n/a";

        let pos = window.outer_position().ok();
        let outer = window.outer_size().ok();
        let inner = window.inner_size().ok();
        let scale = window.scale_factor().ok();
        let mon = window.current_monitor().ok().flatten();
        let visible_after = window.is_visible().ok();
        log::info!(
            "[overlay] show complete (layout={:?}, visible_after={:?}, pos={:?}, outer={:?}, inner={:?}, scale={:?}, monitor={}, raise_without_focus={})",
            widget_layout,
            visible_after,
            pos,
            outer,
            inner,
            scale,
            format_monitor_summary(mon.as_ref()),
            raise_status
        );
    }

    Ok(())
}

#[tauri::command]
pub async fn set_overlay_layout(app: AppHandle, expanded: bool) -> CommandResult<()> {
    #[cfg(desktop)]
    {
        let overlay_mode: String =
            get_setting_from_store(&app, "overlay_mode", "recording_only".to_string());
        let effective_expanded = expanded || overlay_mode == "recording_only";
        return apply_overlay_layout(&app, WidgetLayout::from_expanded(effective_expanded));
    }

    #[cfg(not(desktop))]
    {
        let _ = (app, expanded);
        Ok(())
    }
}

#[tauri::command]
pub async fn show_overlay(app: AppHandle) -> CommandResult<()> {
    #[cfg(desktop)]
    {
        log::debug!("[overlay] show_overlay command invoked");
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
pub async fn hide_overlay(app: AppHandle) -> CommandResult<()> {
    if let Some(window) = app.get_webview_window("overlay") {
        #[cfg(desktop)]
        let overlay_mode: String =
            get_setting_from_store(&app, "overlay_mode", "recording_only".to_string());
        #[cfg(not(desktop))]
        let overlay_mode: &str = "<n/a>";

        let pipeline_state = app
            .try_state::<crate::pipeline::SharedPipeline>()
            .map(|p| p.state());

        let visible_before = window.is_visible().ok();
        log::debug!(
            "[overlay] hide_overlay command invoked (visible_before={:?}, overlay_mode={}, pipeline_state={:?})",
            visible_before,
            overlay_mode,
            pipeline_state
        );
        window.hide().map_err(|e| e.to_string())?;
        let visible_after = window.is_visible().ok();
        log::debug!(
            "[overlay] hide_overlay complete (visible_after={:?})",
            visible_after
        );
    }

    // Best-effort: also hide the hover panel window so it doesn't orphan.
    if let Some(window) = app.get_webview_window("overlay_hover") {
        let visible_before = window.is_visible().ok();
        log::debug!(
            "[overlay_hover] hide via hide_overlay (visible_before={:?})",
            visible_before
        );
        let _ = window.hide();
    }
    Ok(())
}

/// Toggle Escape shortcut while the Quick Ask overlay is visible.
#[tauri::command]
pub async fn set_quick_ask_escape_enabled(app: AppHandle, enabled: bool) -> CommandResult<()> {
    #[cfg(desktop)]
    {
        crate::set_escape_cancel_shortcut_enabled(&app, enabled);
    }

    Ok(())
}

/// Notify backend that the overlay frontend has mounted.
///
/// This gives us a chance to re-assert visibility/position when recording starts
/// before the webview is ready (common source of "overlay didn't show" reports).
#[tauri::command]
pub async fn overlay_frontend_ready(app: AppHandle) -> CommandResult<()> {
    #[cfg(desktop)]
    {
        let now_ms = now_ms();
        app.state::<AppState>()
            .overlay_frontend_ready_at_ms
            .store(now_ms, Ordering::SeqCst);

        let overlay_mode: String =
            get_setting_from_store(&app, "overlay_mode", "recording_only".to_string());
        let pipeline_state = app
            .try_state::<pipeline::SharedPipeline>()
            .map(|p| p.state());

        log::info!(
            "[overlay] frontend ready (overlay_mode={}, pipeline_state={:?})",
            overlay_mode,
            pipeline_state
        );

        let should_show = match overlay_mode.as_str() {
            "always" => true,
            "recording_only" => matches!(
                pipeline_state,
                Some(pipeline::PipelineState::Recording)
                    | Some(pipeline::PipelineState::Routing)
                    | Some(pipeline::PipelineState::Transcribing)
                    | Some(pipeline::PipelineState::Rewriting)
                    | Some(pipeline::PipelineState::Error)
            ),
            _ => false,
        };

        if should_show {
            let _ = show_overlay_with_reset_if_not_always(&app);
        }
    }

    Ok(())
}

/// Notify backend that the overlay hover frontend has mounted.
#[tauri::command]
pub async fn overlay_hover_frontend_ready(app: AppHandle) -> CommandResult<()> {
    #[cfg(desktop)]
    {
        let now_ms = now_ms();
        app.state::<AppState>()
            .overlay_hover_frontend_ready_at_ms
            .store(now_ms, Ordering::SeqCst);
        log::trace!("[overlay_hover] frontend ready (ts={})", now_ms);
    }

    Ok(())
}

/// Notify backend that the Quick Ask frontend has mounted.
#[tauri::command]
pub async fn quick_ask_frontend_ready(app: AppHandle) -> CommandResult<()> {
    #[cfg(desktop)]
    {
        let now_ms = now_ms();
        app.state::<AppState>()
            .quick_ask_frontend_ready_at_ms
            .store(now_ms, Ordering::SeqCst);
        log::trace!("[quick_ask] frontend ready (ts={})", now_ms);
    }

    Ok(())
}

#[tauri::command]
pub async fn show_overlay_hover(app: AppHandle) -> CommandResult<()> {
    let Some(overlay) = app.get_webview_window("overlay") else {
        return Err("Overlay window not found".to_string().into());
    };
    let Some(hover) = app.get_webview_window("overlay_hover") else {
        return Err("Overlay hover window not found".to_string().into());
    };

    app.state::<AppState>()
        .overlay_hover_epoch
        .fetch_add(1, Ordering::SeqCst);

    log::debug!("[overlay_hover] show requested");

    let last_ready_ms = app
        .state::<AppState>()
        .overlay_hover_frontend_ready_at_ms
        .load(Ordering::SeqCst);
    maybe_reload_overlay_webview(
        &app,
        "overlay_hover",
        last_ready_ms,
        Some(45_000),
        "show_overlay_hover",
    );

    let overlay_pos = overlay.outer_position().map_err(|e| e.to_string())?;
    let overlay_size = overlay.outer_size().map_err(|e| e.to_string())?;
    let monitor = overlay
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| resolve_target_monitor(&overlay, &app))
        .ok_or("No monitor found for overlay hover")?;
    let scale = monitor.scale_factor();
    let widget_rect = PhysicalRect::new(
        overlay_pos.x,
        overlay_pos.y,
        overlay_size.width,
        overlay_size.height,
    );
    let rect = layout::adjacent_panel_rect(
        monitor_work_area(&monitor),
        widget_rect,
        LogicalSize::new(320.0, 220.0),
        scale,
        10.0,
        12.0,
    );

    hover
        .set_size(tauri::Size::Physical(tauri::PhysicalSize {
            width: rect.width,
            height: rect.height,
        }))
        .map_err(|e| e.to_string())?;

    hover
        .set_position(tauri::Position::Physical(tauri::PhysicalPosition {
            x: rect.x,
            y: rect.y,
        }))
        .map_err(|e| e.to_string())?;

    // Keep above other windows.
    let _ = hover.set_always_on_top(true);
    hover.show().map_err(|e| e.to_string())?;

    let visible_after = hover.is_visible().ok();
    log::debug!(
        "[overlay_hover] show complete (visible_after={:?}, rect={:?}, monitor={})",
        visible_after,
        rect,
        format_monitor_summary(Some(&monitor))
    );

    Ok(())
}

#[tauri::command]
pub async fn hide_overlay_hover(app: AppHandle) -> CommandResult<()> {
    if let Some(window) = app.get_webview_window("overlay_hover") {
        let visible_before = window.is_visible().ok();
        log::trace!(
            "[overlay_hover] hide requested (visible_before={:?})",
            visible_before
        );
        window.hide().map_err(|e| e.to_string())?;
        let visible_after = window.is_visible().ok();
        log::trace!(
            "[overlay_hover] hide complete (visible_after={:?})",
            visible_after
        );
    }

    // Bump epoch so any pending hides are considered stale.
    app.state::<AppState>()
        .overlay_hover_epoch
        .fetch_add(1, Ordering::SeqCst);

    Ok(())
}

#[tauri::command]
pub async fn schedule_hide_overlay_hover(app: AppHandle, delay_ms: u64) -> CommandResult<()> {
    let expected_epoch = app
        .state::<AppState>()
        .overlay_hover_epoch
        .load(Ordering::SeqCst);

    log::trace!(
        "[overlay_hover] schedule hide requested (delay_ms={}, expected_epoch={})",
        delay_ms,
        expected_epoch
    );

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
            let visible_before = window.is_visible().ok();
            log::debug!(
                "[overlay_hover] schedule hide firing (visible_before={:?}, epoch={})",
                visible_before,
                current_epoch
            );
            let _ = window.hide();
        }
    });

    Ok(())
}

/// Set overlay mode: "always", "never", or "recording_only"
#[tauri::command]
pub async fn set_overlay_mode(app: AppHandle, mode: String) -> CommandResult<()> {
    if let Some(window) = app.get_webview_window("overlay") {
        match mode.as_str() {
            "always" => {
                #[cfg(desktop)]
                {
                    show_overlay_with_reset_if_not_always(&app)?;
                }
                #[cfg(not(desktop))]
                {
                    window.show().map_err(|e| e.to_string())?;
                }
            }
            "never" => {
                // Ask the frontend to animate out before we hide.
                let _ = app.emit(events::EVENT_OVERLAY_HIDE_REQUESTED, ());

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
                    let pipeline_state = app_clone
                        .try_state::<crate::pipeline::SharedPipeline>()
                        .map(|p| p.state());
                    log::debug!(
                        "[overlay] set_overlay_mode fallback hide check (mode_req=never, current_mode={}, expected_epoch={}, current_epoch={}, pipeline_state={:?})",
                        current_mode,
                        expected_epoch,
                        current_epoch,
                        pipeline_state
                    );
                    if current_mode == "never" && current_epoch == expected_epoch {
                        let visible_before = window_clone.is_visible().ok();
                        log::debug!(
                            "[overlay] set_overlay_mode fallback hide firing (mode_req=never, visible_before={:?})",
                            visible_before
                        );
                        let _ = window_clone.hide();
                    }
                });
            }
            "recording_only" => {
                // Hide initially, will be shown when recording starts
                let _ = app.emit(events::EVENT_OVERLAY_HIDE_REQUESTED, ());
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
                    let pipeline_state = app_clone
                        .try_state::<crate::pipeline::SharedPipeline>()
                        .map(|p| p.state());
                    log::debug!(
                        "[overlay] set_overlay_mode fallback hide check (mode_req=recording_only, current_mode={}, expected_epoch={}, current_epoch={}, pipeline_state={:?})",
                        current_mode,
                        expected_epoch,
                        current_epoch,
                        pipeline_state
                    );
                    if current_mode == "recording_only" && current_epoch == expected_epoch {
                        let visible_before = window_clone.is_visible().ok();
                        log::debug!(
                            "[overlay] set_overlay_mode fallback hide firing (mode_req=recording_only, visible_before={:?})",
                            visible_before
                        );
                        let _ = window_clone.hide();
                    }
                });
            }
            _ => {
                return Err(format!("Invalid overlay mode: {}", mode).into());
            }
        }
    }
    Ok(())
}

/// Set overlay widget position on screen
#[tauri::command]
pub async fn set_widget_position(app: AppHandle, position: String) -> CommandResult<()> {
    #[cfg(desktop)]
    {
        let anchor = WidgetAnchor::parse(position.as_str())
            .ok_or_else(|| format!("Invalid widget position: {position}"))?;
        let expanded = app
            .state::<AppState>()
            .overlay_expanded
            .load(Ordering::SeqCst);
        return apply_overlay_layout_at_anchor(
            &app,
            WidgetLayout::from_expanded(expanded),
            anchor,
            true,
        );
    }

    #[cfg(not(desktop))]
    {
        let _ = (app, position);
        Ok(())
    }
}

/// Best-effort: position the Quick Ask overlay window to the configured monitor.
///
/// Quick Ask is implemented as a transparent always-on-top window sized to the panel,
/// so it doesn't block interaction with the rest of the screen.
#[cfg(desktop)]
pub fn position_quick_ask_to_target_monitor(app: &AppHandle) -> CommandResult<()> {
    let Some(win) = app.get_webview_window("quick_ask") else {
        return Err("Quick Ask window not found".to_string().into());
    };

    let monitor = resolve_target_monitor(&win, app).ok_or("No monitor found")?;
    let scale = monitor.scale_factor();
    let rect = layout::anchored_rect(
        monitor_work_area(&monitor),
        LogicalSize::new(520.0, 260.0),
        scale,
        WidgetAnchor::BottomCenter,
        4.0,
    );

    win.set_size(tauri::Size::Physical(tauri::PhysicalSize {
        width: rect.width,
        height: rect.height,
    }))
    .map_err(|e| e.to_string())?;
    win.set_position(tauri::Position::Physical(tauri::PhysicalPosition {
        x: rect.x,
        y: rect.y,
    }))
    .map_err(|e| e.to_string())?;

    Ok(())
}

#[cfg(all(test, desktop))]
mod tests {
    use super::overlay_frontend_needs_reload;

    #[test]
    fn one_shot_ready_overlay_does_not_expire() {
        assert!(!overlay_frontend_needs_reload(1, 120_000, None));
    }

    #[test]
    fn heartbeat_overlay_expires_after_threshold() {
        assert!(!overlay_frontend_needs_reload(1, 45_000, Some(45_000)));
        assert!(overlay_frontend_needs_reload(1, 45_001, Some(45_000)));
    }

    #[test]
    fn overlay_that_never_reported_ready_is_reloaded() {
        assert!(overlay_frontend_needs_reload(0, 0, None));
    }
}
