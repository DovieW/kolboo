use super::*;

#[cfg(target_os = "linux")]
type ConfigurationBarrier = (
    std::sync::mpsc::SyncSender<()>,
    std::sync::mpsc::Receiver<()>,
);
#[cfg(target_os = "linux")]
static QUICK_ASK_CONFIGURATION_BARRIER: std::sync::Mutex<Option<ConfigurationBarrier>> =
    std::sync::Mutex::new(None);

/// Pause the real Wry factory after native registration and before GTK setup.
/// This does not replace a native operation or depend on scheduling sleeps.
#[cfg(target_os = "linux")]
pub(crate) fn pause_next_quick_ask_configuration() -> (
    std::sync::mpsc::Receiver<()>,
    std::sync::mpsc::SyncSender<()>,
) {
    let (arrived, received) = std::sync::mpsc::sync_channel(1);
    let (resume, released) = std::sync::mpsc::sync_channel(1);
    let mut barrier = QUICK_ASK_CONFIGURATION_BARRIER.lock().unwrap();
    assert!(barrier.is_none());
    *barrier = Some((arrived, released));
    (received, resume)
}

#[cfg(target_os = "linux")]
pub(super) fn quick_ask_configuration_barrier(preset: OverlayWindowPreset) {
    if preset != OverlayWindowPreset::QuickAsk {
        return;
    }
    let barrier = QUICK_ASK_CONFIGURATION_BARRIER.lock().unwrap().take();
    if let Some((arrived, released)) = barrier {
        arrived.send(()).unwrap();
        released
            .recv_timeout(std::time::Duration::from_secs(8))
            .unwrap();
    }
}

#[test]
fn waveform_uses_fast_capture_and_slow_idle_cadence() {
    use crate::pipeline::PipelineState::*;
    assert_eq!(waveform_interval(None), Duration::from_millis(33));
    assert_eq!(
        waveform_interval(Some(Recording)),
        Duration::from_millis(33)
    );
    for state in [Idle, Transcribing, Routing, Rewriting, Error] {
        assert_eq!(waveform_interval(Some(state)), Duration::from_secs(5));
    }
}

#[test]
fn stale_hide_cannot_conceal_an_active_or_unknown_pipeline() {
    use crate::pipeline::PipelineState::*;
    for state in [
        None,
        Some(Recording),
        Some(Transcribing),
        Some(Routing),
        Some(Rewriting),
        Some(Error),
    ] {
        assert!(!may_hide("recording_only", state));
        assert!(!may_hide("always", state));
        assert!(may_hide("never", state));
    }
    assert!(may_hide("recording_only", Some(Idle)));
    assert!(!may_hide("always", Some(Idle)));
    assert!(!may_hide("invalid", Some(Idle)));
}

#[test]
fn overlay_window_presets_are_stable() {
    for (preset, label, html, title, size, focusable) in [
        (
            OverlayWindowPreset::Overlay,
            "overlay",
            "overlay.html",
            "Kolboo Overlay",
            (48.0, 48.0),
            false,
        ),
        (
            OverlayWindowPreset::Hover,
            "overlay_hover",
            "overlay-hover.html",
            "Kolboo Presets",
            (320.0, 220.0),
            false,
        ),
        (
            OverlayWindowPreset::QuickAsk,
            "quick_ask",
            "quick-ask.html",
            "Kolboo Quick Ask",
            (520.0, 340.0),
            true,
        ),
    ] {
        assert_eq!(preset.label(), label);
        assert_eq!(preset.html_path(), html);
        assert_eq!(preset.title(), title);
        assert_eq!(preset.size(), size);
        assert!(!preset.visible());
        assert_eq!(preset.focusable(), focusable);
    }
}

#[cfg(target_os = "linux")]
pub(crate) fn native_utility_errors(app: &AppHandle, hover: tauri::WebviewWindow) {
    // A real duplicate-label build exercises the native error boundary without
    // replacing the implementation. Destroyed handles must fail GTK setup
    // cleanly rather than leave a partially configured utility registered.
    let duplicate = build_utility_window(app, OverlayWindowPreset::Hover);
    assert!(duplicate.is_err());
    assert!(report_hover_creation_result(app, duplicate.is_ok()));
    assert!(
        !report_hover_creation_result(app, false),
        "repeated pointer retries do not flood diagnostics"
    );
    assert!(!report_hover_creation_result(app, true));
    assert!(!app
        .state::<crate::state::AppState>()
        .overlay_hover_failure_reported
        .load(Ordering::SeqCst));
    hover.destroy().unwrap();
    let destroyed = configure_utility_window(hover, OverlayWindowPreset::Hover);
    assert!(destroyed.is_err());
    assert!(
        report_hover_creation_result(app, destroyed.is_ok()),
        "a failed native configuration is reported again after recovery"
    );
}
