//! The only native boundary for main-window geometry. No renderer IPC or
//! frontend timing dependency: initialize before showing, including recreation.

use super::*;
use crate::overlay::layout::effective_layout_scale;
#[cfg(target_os = "linux")]
use crate::state::AppState;
use tauri::{AppHandle, Manager, WebviewWindow, WindowEvent};

pub(crate) fn setup(app: &AppHandle) {
    #[cfg(target_os = "linux")]
    {
        // GTK resolution must be queried on the initialized setup/main thread,
        // not the worker that recreates a destroyed window.
        if let Ok(mut cached) = app.state::<AppState>().overlay_linux_desktop_scale.lock() {
            *cached = crate::platform_capabilities::current_linux_desktop_scale();
        }
    }
    if let Some(window) = app.get_webview_window("main") {
        configure(&window);
    }
}

fn scale(window: &WebviewWindow) -> f64 {
    #[cfg(target_os = "linux")]
    let desktop_scale = window
        .state::<AppState>()
        .overlay_linux_desktop_scale
        .lock()
        .ok()
        .and_then(|cached| *cached);
    #[cfg(not(target_os = "linux"))]
    let desktop_scale = None;
    effective_layout_scale(window.scale_factor().unwrap_or(1.0), desktop_scale)
}

fn capture(window: &WebviewWindow, store: &Store<tauri::Wry>) {
    // Failed OS reads are not a new preference.
    let (Ok(size), Ok(maximized), Ok(minimized), Ok(fullscreen)) = (
        window.inner_size(),
        window.is_maximized(),
        window.is_minimized(),
        window.is_fullscreen(),
    ) else {
        return;
    };
    remember(
        store,
        WindowSample {
            size,
            scale: scale(window),
            maximized,
            minimized,
            fullscreen,
        },
    );
}

fn restore(window: &WebviewWindow, state: SavedWindow) -> tauri::Result<()> {
    let monitor = window.current_monitor()?.or(window.primary_monitor()?);
    let work_area = monitor
        .map(|monitor| work_area_size(monitor.work_area().size, *monitor.size()))
        .unwrap_or(PhysicalSize::new(16384, 16384));
    let bounds = state.bounds(work_area, scale(window));
    window.set_min_size(Some(tauri::Size::Physical(bounds.minimum)))?;
    window.set_size(tauri::Size::Physical(bounds.size))?;
    window.center()?;
    if state.maximized {
        window.maximize()?;
    }
    Ok(())
}

pub(crate) fn configure(window: &WebviewWindow) {
    let store = match open_store(window.app_handle(), STORE_FILE) {
        Ok(store) => store,
        Err(_) => {
            // Storage failure must not prevent opening the app or correcting
            // the DPI mismatch in its default dimensions.
            let _ = restore(window, SavedWindow::default());
            log::warn!("Main window preferences unavailable");
            return;
        }
    };
    if restore(window, read_state(&store)).is_err() {
        log::warn!("Main window geometry could not be fully restored");
    }
    let tracked = window.clone();
    window.on_window_event(move |event| handle_event(&tracked, &store, event));
}

pub(super) fn handle_event(window: &WebviewWindow, store: &Store<tauri::Wry>, event: &WindowEvent) {
    match event {
        WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => capture(window, store),
        WindowEvent::CloseRequested { .. } | WindowEvent::Destroyed => {
            capture(window, store);
            if store.save().is_err() {
                log::warn!("Main window preferences could not be saved");
            }
        }
        _ => {}
    }
}
