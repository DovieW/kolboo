use std::time::{Duration, Instant};

use tauri::{App, AppHandle, Emitter, Manager};
use tauri_utils::config::BackgroundThrottlingPolicy;

use crate::commands;
use crate::events;
use crate::pipeline;
use crate::{get_setting_from_store, OverlayAudioLevelPayload};

pub(crate) mod layout;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OverlayWindowPreset {
    Overlay,
    Hover,
    QuickAsk,
}

impl OverlayWindowPreset {
    fn label(self) -> &'static str {
        match self {
            Self::Overlay => "overlay",
            Self::Hover => "overlay_hover",
            Self::QuickAsk => "quick_ask",
        }
    }

    fn html_path(self) -> &'static str {
        match self {
            Self::Overlay => "overlay.html",
            Self::Hover => "overlay-hover.html",
            Self::QuickAsk => "quick-ask.html",
        }
    }

    fn title(self) -> &'static str {
        match self {
            Self::Overlay => "Kolboo Overlay",
            Self::Hover => "Kolboo Presets",
            Self::QuickAsk => "Kolboo Quick Ask",
        }
    }

    fn size(self) -> (f64, f64) {
        match self {
            Self::Overlay => (56.0, 56.0),
            Self::Hover => (320.0, 220.0),
            Self::QuickAsk => (520.0, 340.0),
        }
    }

    fn visible(self) -> bool {
        // Position and size every utility window before its first show. This
        // prevents a startup flash at the window manager's default location.
        false
    }

    fn focusable(self) -> bool {
        match self {
            Self::QuickAsk => true,
            Self::Overlay | Self::Hover => false,
        }
    }
}

fn overlay_window_builder<'a, R: tauri::Runtime, M: tauri::Manager<R>>(
    app: &'a M,
    label: &str,
    html_path: &str,
    title: &str,
    (width, height): (f64, f64),
    visible: bool,
    focusable: bool,
) -> tauri::webview::WebviewWindowBuilder<'a, R, M> {
    tauri::WebviewWindowBuilder::new(app, label, tauri::WebviewUrl::App(html_path.into()))
        .title(title)
        .inner_size(width, height)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .focused(false)
        .focusable(focusable)
        .accept_first_mouse(true)
        .visible(visible)
        .visible_on_all_workspaces(true)
        .background_throttling(BackgroundThrottlingPolicy::Disabled)
}

fn overlay_window_builder_preset<'a, R: tauri::Runtime, M: tauri::Manager<R>>(
    app: &'a M,
    preset: OverlayWindowPreset,
) -> tauri::webview::WebviewWindowBuilder<'a, R, M> {
    let builder = overlay_window_builder(
        app,
        preset.label(),
        preset.html_path(),
        preset.title(),
        preset.size(),
        preset.visible(),
        preset.focusable(),
    );

    match preset {
        OverlayWindowPreset::QuickAsk => builder.focused(true),
        _ => builder,
    }
}

/// Apply Linux window-manager hints at the native map boundary.
///
/// GTK can accept `keep_above` while a hidden window is being built, but some
/// X11/XWayland window managers do not persist that request when the window is
/// subsequently mapped. Reasserting it from the map signal gives the window
/// manager a realized surface to update, while `raise` changes stacking without
/// activating the non-focusable recording overlay.
#[cfg(target_os = "linux")]
fn configure_linux_overlay_stacking(
    window: &tauri::WebviewWindow,
    preset: OverlayWindowPreset,
) -> Result<(), Box<dyn std::error::Error>> {
    use gtk::prelude::{GtkWindowExt, WidgetExt};

    let gtk_window = window.gtk_window()?;
    let type_hint = match preset {
        OverlayWindowPreset::Overlay => gdk::WindowTypeHint::Notification,
        OverlayWindowPreset::Hover | OverlayWindowPreset::QuickAsk => gdk::WindowTypeHint::Utility,
    };

    gtk_window.set_type_hint(type_hint);
    gtk_window.set_keep_above(true);
    gtk_window.connect_map(|gtk_window| {
        gtk_window.set_keep_above(true);
        if let Some(surface) = gtk_window.window() {
            surface.set_keep_above(true);
            surface.raise();
        }
    });

    Ok(())
}

/// Backend-driven overlay waveform publisher.
///
/// This avoids browser getUserMedia startup latency and stays aligned with the
/// actual CPAL capture stream.
#[cfg(desktop)]
pub(crate) fn spawn_overlay_waveform_publisher(app: &AppHandle) {
    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut last_seq: u64 = 0;
        let mut last_emit = Instant::now();
        let mut last_priming_emit: Option<Instant> = None;

        loop {
            // 60Hz-ish. If this is too chatty we can reduce to 30Hz later.
            tokio::time::sleep(Duration::from_millis(16)).await;

            let Some(pipeline) = app_handle.try_state::<pipeline::SharedPipeline>() else {
                continue;
            };

            // Prefer a non-blocking state check: during pipeline start-up the
            // mutex may be held while CPAL capture already begins.
            //
            // If we can prove we're NOT recording, don't publish. Otherwise,
            // allow publishing once the meter seq starts moving.
            if let Some(state) = pipeline.try_state() {
                if state != pipeline::PipelineState::Recording {
                    last_seq = 0;
                    last_priming_emit = None;
                    continue;
                }
            }

            // Read the latest snapshots without locking the pipeline.
            // Drive emission from the level meter so the overlay stays alive
            // even if waveform buckets are temporarily unavailable.
            let levels = pipeline.audio_level_snapshot_fast();

            // If the capture stream has started but we haven't seen a callback yet,
            // send a one-time "priming" event so the overlay can render immediately
            // (baseline waveform) instead of waiting for the first buffer.
            if levels.seq == 0 {
                // Haven't observed any callbacks yet.
                // Keep sending priming frames for a short while so the overlay
                // doesn't miss the first event if its listener registers late.
                let should_emit = match last_priming_emit {
                    None => true,
                    Some(t) => t.elapsed() >= Duration::from_millis(50),
                };
                if should_emit {
                    last_priming_emit = Some(Instant::now());
                    let payload = OverlayAudioLevelPayload {
                        seq: 0,
                        rms: 0.0,
                        peak: 0.0,
                        wave_seq: Some(0),
                        mins: Some(Vec::<f32>::new()),
                        maxes: Some(Vec::<f32>::new()),
                    };
                    if let Some(overlay) = app_handle.get_webview_window("overlay") {
                        let _ = overlay.emit(events::EVENT_OVERLAY_AUDIO_LEVEL, payload);
                    } else {
                        let _ = app_handle.emit(events::EVENT_OVERLAY_AUDIO_LEVEL, payload);
                    }
                }
                continue;
            }

            if levels.seq == last_seq {
                continue;
            }
            last_seq = levels.seq;

            // Waveform buckets (may be all-zeros early or on some devices).
            let wave = pipeline.audio_waveform_snapshot_fast();

            // Throttle slightly if needed (defensive).
            if last_emit.elapsed() < Duration::from_millis(8) {
                continue;
            }
            last_emit = Instant::now();

            // Emit directly to the overlay window when available.
            // This avoids any ambiguity around app-wide vs window event targets.
            let payload = OverlayAudioLevelPayload {
                seq: levels.seq,
                rms: levels.rms,
                peak: levels.peak,
                wave_seq: Some(wave.seq),
                mins: Some(wave.mins),
                maxes: Some(wave.maxes),
            };
            if let Some(overlay) = app_handle.get_webview_window("overlay") {
                let _ = overlay.emit(events::EVENT_OVERLAY_AUDIO_LEVEL, payload);
            } else {
                let _ = app_handle.emit(events::EVENT_OVERLAY_AUDIO_LEVEL, payload);
            }
        }
    });
}

/// Create overlay, hover, and quick-ask windows, and apply startup positioning/visibility.
pub(crate) fn create_overlay_windows(app: &App) -> Result<(), Box<dyn std::error::Error>> {
    // Create overlay window
    let overlay = overlay_window_builder_preset(app, OverlayWindowPreset::Overlay).build()?;

    // Create hover panel window (hidden by default).
    // This avoids resizing the main overlay window on hover, which can cause
    // cursor flicker and position drift on Windows.
    let overlay_hover = overlay_window_builder_preset(app, OverlayWindowPreset::Hover).build()?;

    // Create Quick Ask answer window (hidden by default).
    // This is a separate transparent webview that renders an answer + copy button.
    let quick_ask = overlay_window_builder_preset(app, OverlayWindowPreset::QuickAsk).build()?;

    #[cfg(target_os = "linux")]
    {
        configure_linux_overlay_stacking(&overlay, OverlayWindowPreset::Overlay)?;
        configure_linux_overlay_stacking(&overlay_hover, OverlayWindowPreset::Hover)?;
        configure_linux_overlay_stacking(&quick_ask, OverlayWindowPreset::QuickAsk)?;
    }

    #[cfg(target_os = "linux")]
    {
        let desktop_scale = crate::platform_capabilities::current_linux_desktop_scale();
        *app.state::<crate::state::AppState>()
            .overlay_linux_desktop_scale
            .lock()
            .map_err(|_| "Overlay desktop scale lock poisoned")? = desktop_scale;
        log::info!(
            "Linux overlay desktop scale: {:?} (native_window_scale={})",
            desktop_scale,
            overlay.scale_factor().unwrap_or(1.0)
        );
    }

    // On macOS, convert to NSPanel for better fullscreen app behavior
    #[cfg(target_os = "macos")]
    {
        use tauri_nspanel::{CollectionBehavior, PanelLevel, WebviewWindowExt};
        match overlay.to_panel::<crate::OverlayPanel>() {
            Ok(panel) => {
                // Configure panel to float above fullscreen apps
                panel.set_level(PanelLevel::ScreenSaver.value());
                panel.set_floating_panel(true);

                // Set collection behavior to appear on all spaces including fullscreen
                let behavior = CollectionBehavior::new()
                    .can_join_all_spaces()
                    .full_screen_auxiliary();
                panel.set_collection_behavior(behavior.value());

                // Set style mask to non-activating panel
                let style = tauri_nspanel::StyleMask::empty().nonactivating_panel();
                panel.set_style_mask(style.value());

                log::info!("[NSPanel] Successfully converted overlay to NSPanel");
            }
            Err(e) => {
                log::error!("[NSPanel] Failed to convert overlay to NSPanel: {:?}", e);
            }
        }
    }

    // Position hidden windows before their first show. The layout controller
    // owns both size and position, including monitor targeting and DPI.
    if let Err(e) = commands::overlay::position_quick_ask_to_target_monitor(app.handle()) {
        log::warn!("Failed to position Quick Ask at startup: {}", e);
    }

    // Set initial overlay visibility based on saved settings
    let overlay_mode: String =
        get_setting_from_store(app.handle(), "overlay_mode", "recording_only".to_string());
    let initial_layout = if overlay_mode == "recording_only" {
        layout::WidgetLayout::Expanded
    } else {
        layout::WidgetLayout::Compact
    };
    if let Err(e) = commands::overlay::apply_overlay_layout(app.handle(), initial_layout) {
        log::warn!("Failed to lay out overlay at startup: {}", e);
    }

    match overlay_mode.as_str() {
        "never" | "recording_only" => {
            let _ = overlay.hide();
        }
        _ => {
            let _ = overlay.show();
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::OverlayWindowPreset;

    #[test]
    fn overlay_window_presets_are_stable() {
        let overlay = OverlayWindowPreset::Overlay;
        assert_eq!(overlay.label(), "overlay");
        assert_eq!(overlay.html_path(), "overlay.html");
        assert_eq!(overlay.title(), "Kolboo Overlay");
        assert_eq!(overlay.size(), (56.0, 56.0));
        assert!(!overlay.visible());

        let hover = OverlayWindowPreset::Hover;
        assert_eq!(hover.label(), "overlay_hover");
        assert_eq!(hover.html_path(), "overlay-hover.html");
        assert_eq!(hover.title(), "Kolboo Presets");
        assert_eq!(hover.size(), (320.0, 220.0));
        assert!(!hover.visible());

        let quick_ask = OverlayWindowPreset::QuickAsk;
        assert_eq!(quick_ask.label(), "quick_ask");
        assert_eq!(quick_ask.html_path(), "quick-ask.html");
        assert_eq!(quick_ask.title(), "Kolboo Quick Ask");
        assert_eq!(quick_ask.size(), (520.0, 340.0));
        assert!(!quick_ask.visible());
    }
}
