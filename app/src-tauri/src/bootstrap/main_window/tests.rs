use super::*;
use crate::overlay::layout::effective_layout_scale;
use tauri::test::{mock_builder, mock_context, noop_assets};

fn sample(width: u32, height: u32, scale: f64) -> WindowSample {
    WindowSample {
        size: PhysicalSize::new(width, height),
        scale,
        maximized: false,
        minimized: false,
        fullscreen: false,
    }
}

#[test]
fn fractional_xwayland_uses_content_dpi_not_integer_native_scale() {
    let scale = effective_layout_scale(1.0, Some(1.75));
    let bounds = SavedWindow::default().bounds(PhysicalSize::new(2880, 1758), scale);
    assert_eq!(bounds.size, PhysicalSize::new(2240, 1400));
    assert_eq!(bounds.minimum, PhysicalSize::new(1120, 700));
    assert_eq!(f64::from(bounds.size.width) / scale, 1280.0);
}

#[test]
fn missing_work_area_falls_back_to_monitor_without_discarding_valid_panels() {
    let monitor = PhysicalSize::new(2880, 1800);
    let area = PhysicalSize::new(2880, 1758);
    assert_eq!(work_area_size(area, monitor), area);
    assert_eq!(work_area_size(PhysicalSize::new(0, 1758), monitor), monitor);
    assert_eq!(work_area_size(PhysicalSize::new(2880, 0), monitor), monitor);
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires an isolated X11 display and window manager; see cargo:test:window-native"]
#[allow(deprecated)] // Bounded native polling; never used in production/default unit tests.
fn native_window_manager_integration() {
    use crate::state::AppState;
    use std::time::{Duration, Instant};
    use tauri::{Manager, WebviewWindowBuilder, WindowEvent};

    assert_eq!(
        std::env::var("KOLBOO_NATIVE_WINDOW_TEST").as_deref(),
        Ok("1"),
        "Run only through the isolated native-window test runner"
    );
    crate::text::selection_probe::tests::native_selection_probe_copy_transaction();
    let history_directory = tempfile::tempdir().unwrap();
    let mut app = tauri::Builder::<tauri::Wry>::default()
        .any_thread()
        .manage(AppState::default())
        .manage(crate::history::HistoryStorage::new(
            history_directory.path().into(),
        ))
        .manage(crate::request_log::RequestLogStore::new())
        .plugin(tauri_plugin_store::Builder::new().build())
        .build(mock_context(noop_assets()))
        .unwrap();
    crate::history_request_lifecycle::tests::native_superseded_history(app.handle());
    crate::sessions::normal_dictation_output::tests::native_output_warning(app.handle());
    // A real GTK screen with controlled fractional DPI, not a fake Window.
    gdk::Screen::default().unwrap().set_resolution(168.0);
    let window = WebviewWindowBuilder::new(&app, "main", Default::default())
        .visible(false)
        .build()
        .unwrap();
    super::native::setup(app.handle());
    window.show().unwrap();
    let store = open_store(app.handle(), STORE_FILE).unwrap();
    let pump = |app: &mut tauri::App<tauri::Wry>,
                predicate: &dyn Fn(&tauri::App<tauri::Wry>) -> bool| {
        let deadline = Instant::now() + Duration::from_secs(8);
        while !predicate(app) {
            assert!(
                Instant::now() < deadline,
                "native window operation did not settle"
            );
            app.run_iteration(|_, event| {
                if let tauri::RunEvent::ExitRequested { api, .. } = event {
                    api.prevent_exit();
                }
            });
        }
    };
    pump(&mut app, &|_| {
        window.inner_size().unwrap() == PhysicalSize::new(2240, 1400)
    });
    window
        .set_size(tauri::Size::Physical(PhysicalSize::new(1925, 1225)))
        .unwrap();
    pump(&mut app, &|_| {
        read_state(&store).width == 1100.0 && read_state(&store).height == 700.0
    });
    assert_eq!(read_state(&store).width, 1100.0);
    window.maximize().unwrap();
    pump(&mut app, &|_| read_state(&store).maximized);
    assert_eq!(
        (read_state(&store).width, read_state(&store).height),
        (1100.0, 700.0)
    );
    store.save().unwrap();
    window.close().unwrap();
    pump(&mut app, &|app| app.get_webview_window("main").is_none());
    crate::bootstrap::show_main_window(app.handle(), "native-window-test", None);
    pump(&mut app, &|app| {
        app.get_webview_window("main")
            .is_some_and(|window| window.is_maximized().ok() == Some(true))
    });
    let recreated = app.get_webview_window("main").unwrap();
    recreated.unmaximize().unwrap();
    pump(&mut app, &|_| {
        !read_state(&store).maximized
            && recreated.inner_size().unwrap() == PhysicalSize::new(1925, 1225)
    });
    assert_eq!(
        (read_state(&store).width, read_state(&store).height),
        (1100.0, 700.0)
    );

    // Corrupt only this test's Store resource registry. The real failed-open
    // path must still restore usable default dimensions rather than block UI.
    let rid = app
        .resources_table()
        .names()
        .find(|(_, name)| name.contains("tauri_plugin_store") && name.contains("Store"))
        .unwrap()
        .0;
    let taken = app.resources_table().take_any(rid).unwrap();
    super::native::configure(&recreated);
    pump(&mut app, &|_| {
        recreated.inner_size().unwrap() == PhysicalSize::new(2240, 1400)
    });
    drop(taken);
    let repaired = app.store_builder(STORE_FILE).create_new().build().unwrap();
    let path = app.path().app_data_dir().unwrap().join(STORE_FILE);
    if path.is_file() {
        std::fs::remove_file(&path).unwrap();
    }
    std::fs::create_dir(&path).unwrap();
    super::native::handle_event(&recreated, &repaired, &WindowEvent::Destroyed);
    assert_eq!(read_state(&repaired).width, 1280.0);
    std::fs::remove_dir(&path).unwrap();
    repaired.save().unwrap();
    recreated.close().unwrap();
    pump(&mut app, &|app| app.get_webview_window("main").is_none());
    // Destroyed handles return an OS error. Capture must keep the last valid
    // preference and failed restore must not panic while attaching cleanup.
    super::native::handle_event(&recreated, &repaired, &WindowEvent::Destroyed);
    super::native::configure(&recreated);
    assert_eq!(read_state(&repaired).width, 1280.0);
}

#[test]
fn windows_and_retina_native_scale_is_not_applied_twice() {
    for scale in [1.0, 1.25, 1.5, 1.75, 2.0] {
        let bounds = SavedWindow::default().bounds(
            PhysicalSize::new(5120, 2880),
            effective_layout_scale(scale, None),
        );
        assert_eq!(f64::from(bounds.size.width), 1280.0 * scale);
        assert_eq!(f64::from(bounds.size.height), 800.0 * scale);
    }
    assert_eq!(effective_layout_scale(2.0, Some(1.75)), 2.0);
}

#[test]
fn resized_preferences_survive_scaling_changes_and_small_screens() {
    let state = SavedWindow::default()
        .capture(sample(2100, 1400, 1.75))
        .unwrap();
    assert_eq!((state.width, state.height), (1200.0, 800.0));
    assert_eq!(
        state.bounds(PhysicalSize::new(1920, 1080), 1.0).size,
        PhysicalSize::new(1200, 800)
    );
    let small = state.bounds(PhysicalSize::new(1024, 768), 2.0);
    assert_eq!(small.size, PhysicalSize::new(928, 672));
    assert_eq!(small.minimum, PhysicalSize::new(928, 672));
    assert_eq!(
        state.bounds(PhysicalSize::new(0, 0), 1.0).size,
        PhysicalSize::new(1, 1)
    );
}

#[test]
fn maximizing_minimizing_and_fullscreen_never_erase_normal_dimensions() {
    let original = SavedWindow {
        width: 1100.0,
        height: 700.0,
        maximized: false,
    };
    let mut maximized = sample(2880, 1758, 1.75);
    maximized.maximized = true;
    let remembered = original.capture(maximized).unwrap();
    assert_eq!(
        remembered,
        SavedWindow {
            maximized: true,
            ..original
        }
    );
    let mut minimized = sample(0, 0, 1.75);
    minimized.minimized = true;
    assert_eq!(remembered.capture(minimized), None);
    let mut fullscreen = sample(2880, 1800, 1.75);
    fullscreen.fullscreen = true;
    assert_eq!(remembered.capture(fullscreen), None);
    let restored = remembered.capture(sample(1925, 1225, 1.75)).unwrap();
    assert_eq!(restored, original);
    assert_eq!(original.capture(sample(1100, 700, 1.0)), None);
}

#[test]
fn transient_zero_dimensions_or_invalid_measurements_are_ignored() {
    for value in [
        sample(0, 0, 1.0),
        sample(100, 500, 1.0),
        sample(500, 100, 1.0),
        sample(20000, 500, 1.0),
        sample(500, 20000, 1.0),
        sample(1280, 800, 0.0),
        sample(1280, 800, f64::NAN),
    ] {
        assert_eq!(SavedWindow::default().capture(value), None);
    }
}

#[test]
fn actual_store_persists_size_and_maximize_across_recreation_without_settings() {
    let directory = tempfile::tempdir().unwrap();
    let app = mock_builder()
        .plugin(tauri_plugin_store::Builder::new().build())
        .build(mock_context(noop_assets()))
        .unwrap();
    let path = directory.path().join(STORE_FILE);
    let store = open_store(app.handle(), &path).unwrap();
    assert_eq!(read_state(&store), SavedWindow::default());
    remember(&store, sample(2100, 1400, 1.75));
    let mut maximized = sample(2880, 1758, 1.75);
    maximized.maximized = true;
    remember(&store, maximized);
    remember(&store, sample(0, 0, 1.75));
    store.save().unwrap();
    // A fresh application/store is the real disk boundary, not a mocked saver.
    let restarted = mock_builder()
        .plugin(tauri_plugin_store::Builder::new().build())
        .build(mock_context(noop_assets()))
        .unwrap();
    let loaded = open_store(restarted.handle(), &path).unwrap();
    let state = read_state(&loaded);
    assert_eq!(
        state,
        SavedWindow {
            width: 1200.0,
            height: 800.0,
            maximized: true
        }
    );
    assert_eq!(
        state.bounds(PhysicalSize::new(5120, 2880), 2.0).size,
        PhysicalSize::new(2400, 1600)
    );
    assert!(!directory.path().join("settings.json").exists());
    assert_eq!(loaded.keys(), vec![STATE_KEY]);
}

#[test]
fn malformed_preferences_fall_back_and_write_errors_do_not_erase_in_memory_state() {
    let directory = tempfile::tempdir().unwrap();
    let app = mock_builder()
        .plugin(tauri_plugin_store::Builder::new().build())
        .build(mock_context(noop_assets()))
        .unwrap();
    let path = directory.path().join(STORE_FILE);
    let store = open_store(app.handle(), &path).unwrap();
    for raw in [
        serde_json::json!("bad"),
        serde_json::json!({"width": 0, "height": 800, "maximized": true}),
        serde_json::json!({"width": 1280, "height": 1, "maximized": false}),
    ] {
        store.set(STATE_KEY, raw);
        assert_eq!(read_state(&store), SavedWindow::default());
    }
    remember(&store, sample(1000, 650, 1.0));
    std::fs::create_dir(&path).unwrap();
    assert!(store.save().is_err());
    assert_eq!(read_state(&store).width, 1000.0);
    std::fs::remove_dir(&path).unwrap();
    store.save().unwrap();
    assert!(path.is_file());
}
