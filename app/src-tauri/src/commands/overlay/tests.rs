use super::*;

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

#[test]
fn interrupted_hover_transition_does_not_permanently_disable_visibility() {
    let state = AppState::default();
    let _ = std::panic::catch_unwind(|| {
        let _guard = state.overlay_hover_lifecycle_lock.lock().unwrap();
        panic!("synthetic interrupted visibility transition");
    });
    let _guard = lock_hover_lifecycle(&state);
    assert!(state.overlay_hover_lifecycle_lock.is_poisoned());
}

#[cfg(target_os = "linux")]
#[allow(deprecated)]
pub(crate) fn native_hover_lifecycle(app: &mut tauri::App) {
    use std::time::{Duration, Instant};
    fn worker(app: &mut tauri::App, operation: impl FnOnce(AppHandle) + Send + 'static) {
        let handle = app.handle().clone();
        let (sent, received) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            operation(handle);
            sent.send(()).unwrap();
        });
        let deadline = Instant::now() + Duration::from_secs(8);
        loop {
            match received.try_recv() {
                Ok(()) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    // Preserve the worker's original assertion, not a misleading
                    // subsequent deadline failure in the GUI pump.
                    worker.join().unwrap();
                    panic!("hover worker disconnected without a completion");
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    assert!(Instant::now() < deadline, "hover native worker stalled");
                    app.run_iteration(|_, _| {});
                }
            }
        }
        worker.join().unwrap();
    }
    fn mapped_at(app: &mut tauri::App, label: &str, rect: PhysicalRect) {
        let deadline = Instant::now() + Duration::from_secs(8);
        loop {
            let window = app.get_webview_window(label).unwrap();
            let observed = (
                window.is_visible().unwrap(),
                window.outer_position().unwrap(),
                window.outer_size().unwrap(),
            );
            if observed.0
                && observed.1 == tauri::PhysicalPosition::new(rect.x, rect.y)
                && observed.2 == tauri::PhysicalSize::new(rect.width, rect.height)
            {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "{label} did not map at its requested rectangle: expected={rect:?}, observed={observed:?}"
            );
            app.run_iteration(|_, _| {});
        }
    }
    fn mapped_parent(app: &mut tauri::App, requested: PhysicalRect) {
        use gtk::prelude::{GtkWindowExt, WidgetExt};
        let deadline = Instant::now() + Duration::from_secs(8);
        loop {
            let window = app.get_webview_window("overlay").unwrap();
            let native = window.gtk_window().unwrap();
            let native_size = native.size();
            let scale = window.scale_factor().unwrap();
            let expected_native =
                tauri::LogicalSize::new(native_size.0 as u32, native_size.1 as u32)
                    .to_physical::<u32>(scale);
            let size = window.outer_size().unwrap();
            if native.is_mapped()
                && window.is_visible().unwrap()
                && size == expected_native
                && size.width == requested.width
                && size.height >= requested.height
            {
                eprintln!("Synthetic parent mapping: requested={requested:?}, actual_position={:?}, actual_size={size:?}, GTK_preferred_height={:?}, GTK_size_request={:?}", window.outer_position().unwrap(), native.preferred_height(), native.size_request());
                return;
            }
            assert!(Instant::now() < deadline, "recording parent did not finish native mapping: requested={requested:?}, cached_size={size:?}, GTK_size={native_size:?}");
            app.run_iteration(|_, _| {});
        }
    }
    // The preceding overlap test closes its page asynchronously.
    let deadline = Instant::now() + Duration::from_secs(8);
    while app.get_webview_window("overlay").is_some() {
        assert!(Instant::now() < deadline);
        app.run_iteration(|_, _| {});
    }
    worker(app, |app| {
        assert!(!app
            .state::<AppState>()
            .overlay_hover_requested
            .load(Ordering::SeqCst));
        tauri::async_runtime::block_on(hide_overlay_hover(app.clone())).unwrap();
        tauri::async_runtime::block_on(overlay_hover_frontend_ready(app.clone())).unwrap();
        app.state::<AppState>()
            .overlay_hover_requested
            .store(true, Ordering::SeqCst);
        assert!(
            show_ready_hover(&app).is_err(),
            "missing recording overlay cannot be shown"
        );
    });
    let settings = app.store("settings.json").unwrap();
    settings.set("overlay_mode", serde_json::json!("recording_only"));
    // The overlap test deliberately destroyed the previous primary page.
    // Its successful native-layout cache belongs to that old instance, not
    // the newly constructed utility. Production preloads this page only once.
    *app.state::<AppState>()
        .overlay_last_applied_rect
        .lock()
        .unwrap() = None;
    crate::overlay::create_overlay_windows(app).unwrap();
    assert!(app.get_webview_window("overlay_hover").is_none());
    worker(app, |app| {
        // Hidden parent invalidates even a pending ready show.
        show_ready_hover(&app).unwrap();
        assert!(!app
            .state::<AppState>()
            .overlay_hover_requested
            .load(Ordering::SeqCst));
        // Follow recording's real force-layout/show path, rather than mapping
        // the hidden page directly with a prior test's cached native geometry.
        show_overlay_with_reset_if_not_always(&app).unwrap();
        app.state::<AppState>()
            .overlay_hover_requested
            .store(true, Ordering::SeqCst);
        assert!(
            show_ready_hover(&app).is_err(),
            "missing hover is reported, not silently shown"
        );
    });
    // Visibility APIs acknowledge the map request before the window manager
    // applies its ConfigureNotify geometry. A real pointer cannot enter the
    // parent until that map is complete; reproduce that boundary explicitly.
    let (x, y, width, height) = app
        .state::<AppState>()
        .overlay_last_applied_rect
        .lock()
        .unwrap()
        .unwrap();
    // The asset-free Wry fixture can map at GTK's content natural height rather
    // than the recording widget's requested height. Wait for its real mapped
    // allocation/ConfigureNotify, then position hover from that actual parent
    // rectangle, as production does. Hover's own exact geometry is checked below.
    mapped_parent(app, PhysicalRect::new(x, y, width, height));
    worker(app, |app| {
        app.state::<AppState>()
            .overlay_hover_creating
            .store(true, Ordering::SeqCst);
        assert!(tauri::async_runtime::block_on(show_overlay_hover(app.clone())).is_err());
        app.state::<AppState>()
            .overlay_hover_creating
            .store(false, Ordering::SeqCst);
        tauri::async_runtime::block_on(show_overlay_hover(app.clone())).unwrap();
        let hover = app.get_webview_window("overlay_hover").unwrap();
        assert!(
            !hover.is_visible().unwrap(),
            "new page waits for actual listener readiness"
        );
        assert!(!app
            .state::<AppState>()
            .overlay_hover_creating
            .load(Ordering::SeqCst));
        assert_eq!(
            app.state::<AppState>()
                .overlay_hover_frontend_ready_at_ms
                .load(Ordering::SeqCst),
            0
        );
        tauri::async_runtime::block_on(hide_overlay_hover(app.clone())).unwrap();
        tauri::async_runtime::block_on(overlay_hover_frontend_ready(app.clone())).unwrap();
        assert!(
            !hover.is_visible().unwrap(),
            "late readiness cannot resurrect a canceled hover"
        );
        tauri::async_runtime::block_on(show_overlay_hover(app.clone())).unwrap();
        assert!(hover.is_visible().unwrap());
    });
    let overlay = app.get_webview_window("overlay").unwrap();
    let monitor = overlay.current_monitor().unwrap().unwrap();
    let position = overlay.outer_position().unwrap();
    let size = overlay.outer_size().unwrap();
    let (_, scale) = overlay_display_scale(app.handle(), &monitor);
    let hover_rect = layout::adjacent_panel_rect(
        monitor_work_area(&monitor),
        PhysicalRect::new(position.x, position.y, size.width, size.height),
        LogicalSize::new(320.0, 220.0),
        scale,
        10.0,
        12.0,
    );
    mapped_at(app, "overlay_hover", hover_rect);
    worker(app, |app| {
        let hover = app.get_webview_window("overlay_hover").unwrap();
        let before = (hover.outer_position().unwrap(), hover.outer_size().unwrap());
        tauri::async_runtime::block_on(show_overlay_hover(app.clone())).unwrap();
        assert_eq!(
            before,
            (hover.outer_position().unwrap(), hover.outer_size().unwrap())
        );
        tauri::async_runtime::block_on(hide_overlay_hover(app.clone())).unwrap();
        assert!(!hover.is_visible().unwrap());
        tauri::async_runtime::block_on(show_overlay_hover(app.clone())).unwrap();
        assert!(hover.is_visible().unwrap());
        tauri::async_runtime::block_on(schedule_hide_overlay_hover(app.clone(), 0)).unwrap();
    });
    let deadline = Instant::now() + Duration::from_secs(8);
    while app
        .get_webview_window("overlay_hover")
        .unwrap()
        .is_visible()
        .unwrap()
    {
        assert!(Instant::now() < deadline);
        app.run_iteration(|_, _| {});
    }
    worker(app, |app| {
        tauri::async_runtime::block_on(show_overlay_hover(app.clone())).unwrap();
        let hover = app.get_webview_window("overlay_hover").unwrap();
        app.store("settings.json")
            .unwrap()
            .set("overlay_mode", serde_json::json!("never"));
        tauri::async_runtime::block_on(hide_overlay(app.clone())).unwrap();
        assert!(!hover.is_visible().unwrap());
        assert!(!app
            .state::<AppState>()
            .overlay_hover_requested
            .load(Ordering::SeqCst));
        crate::overlay::tests::native_utility_errors(&app, hover);
        let overlay = app.get_webview_window("overlay").unwrap();
        overlay.destroy().unwrap();
        let deadline = Instant::now() + Duration::from_secs(8);
        let native_error = loop {
            match overlay.is_visible() {
                Err(error) => break error,
                Ok(_) => {
                    assert!(
                        Instant::now() < deadline,
                        "parent destruction did not reach the native getter boundary"
                    );
                    std::thread::yield_now();
                }
            }
        };
        assert!(
            hover_target_monitor(&overlay, &app).is_none(),
            "a lost native parent exhausts monitor fallbacks without a stale monitor"
        );
        let expected = native_error.to_string();
        let command_error = window_command_error(native_error);
        assert_eq!(command_error.message.as_ref(), expected);
        assert_eq!(command_error.error_type.as_ref(), "unknown");
        assert!(command_error.details.is_none() && command_error.code.is_none());
    });
}
