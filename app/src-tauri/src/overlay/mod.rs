use std::time::{Duration, Instant};

use tauri::{App, AppHandle, Emitter, Manager};
use tauri_utils::config::BackgroundThrottlingPolicy;

use crate::commands;
use crate::events;
use crate::pipeline;
use crate::{get_setting_from_store, OverlayAudioLevelPayload};

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
    let overlay = tauri::WebviewWindowBuilder::new(
        app,
        "overlay",
        tauri::WebviewUrl::App("overlay.html".into()),
    )
    .title("Kolboo Overlay")
    .inner_size(48.0, 48.0)
    .decorations(false)
    .transparent(true)
    .shadow(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .resizable(false)
    .focused(false)
    .focusable(false)
    .accept_first_mouse(true)
    .visible(true)
    .visible_on_all_workspaces(true)
    .background_throttling(BackgroundThrottlingPolicy::Disabled)
    .build()?;

    // Create hover panel window (hidden by default).
    // This avoids resizing the main overlay window on hover, which can cause
    // cursor flicker and position drift on Windows.
    let _overlay_hover = tauri::WebviewWindowBuilder::new(
        app,
        "overlay_hover",
        tauri::WebviewUrl::App("overlay-hover.html".into()),
    )
    .title("Kolboo Presets")
    .inner_size(320.0, 220.0)
    .decorations(false)
    .transparent(true)
    .shadow(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .resizable(false)
    .focused(false)
    .focusable(false)
    .accept_first_mouse(true)
    .visible(false)
    .visible_on_all_workspaces(true)
    .background_throttling(BackgroundThrottlingPolicy::Disabled)
    .build()?;

    // Create Quick Ask answer window (hidden by default).
    // This is a separate transparent webview that renders an answer + copy button.
    let _quick_ask = tauri::WebviewWindowBuilder::new(
        app,
        "quick_ask",
        tauri::WebviewUrl::App("quick-ask.html".into()),
    )
    .title("Kolboo Quick Ask")
    .inner_size(520.0, 340.0)
    .decorations(false)
    .transparent(true)
    .shadow(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .resizable(false)
    .focused(false)
    .focusable(false)
    .accept_first_mouse(true)
    .visible(false)
    .visible_on_all_workspaces(true)
    .background_throttling(BackgroundThrottlingPolicy::Disabled)
    .build()?;

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

    // Best-effort: position overlay + quick-ask windows using persisted settings.
    // This includes monitor targeting (main/cursor/active window).
    if let Err(e) = commands::overlay::snap_overlay_to_saved_position(app.handle()) {
        log::warn!("Failed to position overlay at startup: {}", e);
    }
    if let Err(e) = commands::overlay::position_quick_ask_to_target_monitor(app.handle()) {
        log::warn!("Failed to position Quick Ask at startup: {}", e);
    }

    // Set initial overlay visibility based on saved settings
    let overlay_mode: String =
        get_setting_from_store(app.handle(), "overlay_mode", "recording_only".to_string());
    match overlay_mode.as_str() {
        "never" | "recording_only" => {
            let _ = overlay.hide();
        }
        _ => {} // "always" - keep visible (default)
    }

    Ok(())
}
