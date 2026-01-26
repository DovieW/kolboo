//! Unified selection probe for Quick Ask and Quick Replace.
//!
//! Both features need to capture the user's currently highlighted text at recording
//! start. This module provides a single implementation that works for both.

use std::sync::atomic::Ordering;

use tauri::{AppHandle, Manager};

use crate::commands::text::ContextGrabMethod;
use crate::windows_uia::types::WindowsTextContextSource;

/// Which feature is requesting the selection probe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeKind {
    QuickAsk,
    QuickReplace,
}

#[derive(Debug, Clone)]
pub struct SelectionProbeContext {
    pub selection_text: Option<String>,
    pub surrounding_text: Option<String>,
    #[allow(dead_code)]
    pub source: WindowsTextContextSource,
}

/// Spawn a background selection probe.
///
/// Returns the epoch for this probe (used to match results later), or `0` if the probe
/// was not started (e.g., feature disabled or no grab method).
///
/// The probe runs on a blocking thread and updates the corresponding probe state when done.
pub fn spawn_probe(
    app: &AppHandle,
    kind: ProbeKind,
    context_grab_method: ContextGrabMethod,
) -> u64 {
    if context_grab_method == ContextGrabMethod::None {
        return 0;
    }

    let state = app.state::<crate::state::AppState>();

    // Bump epoch and initialize the probe slot based on kind
    let epoch = match kind {
        ProbeKind::QuickAsk => {
            let epoch = state
                .quick_ask_probe_epoch
                .fetch_add(1, Ordering::SeqCst)
                .saturating_add(1);
            if let Ok(mut probe) = state.quick_ask_probe.lock() {
                probe.epoch = epoch;
                probe.ready = false;
                probe.selection_text = None;
                probe.surrounding_text = None;
                probe.source = WindowsTextContextSource::None;
            }
            epoch
        }
        ProbeKind::QuickReplace => {
            let epoch = state
                .quick_replace_probe_epoch
                .fetch_add(1, Ordering::SeqCst)
                .saturating_add(1);
            if let Ok(mut probe) = state.quick_replace_probe.lock() {
                probe.epoch = epoch;
                probe.ready = false;
                probe.selection_text = None;
                probe.surrounding_text = None;
                probe.source = WindowsTextContextSource::None;
            }
            epoch
        }
    };

    // Spawn the blocking probe task
    let app_for_probe = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let max_chars = 8_000usize;
        let mut selection: Option<String> = None;
        let mut surrounding: Option<String> = None;
        let mut source = WindowsTextContextSource::None;
        let blocked_by_safety: bool;

        #[cfg(target_os = "windows")]
        {
            let allow_context_capture = crate::windows_uia::snapshot::capture_focused_snapshot()
                .ok()
                .map(|snapshot| crate::windows_uia::safety::allow_context_capture(&snapshot))
                .unwrap_or(true);
            blocked_by_safety = !allow_context_capture;

            if allow_context_capture {
                let uia_context =
                    crate::windows_uia::context::capture_focused_text_context(max_chars)
                        .ok()
                        .flatten();

                if let Some(context) = uia_context {
                    selection = context.selection_text;
                    surrounding = context.surrounding_text;
                    source = context.source;
                }
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            blocked_by_safety = false;
            selection = crate::commands::text::probe_selected_text_via_copy_with_app(
                &app_for_probe,
                context_grab_method,
            )
            .ok()
            .flatten();

            if selection.is_some() {
                source = WindowsTextContextSource::Clipboard;
            }
        }

        log::info!(
            "Selection probe: source={:?} selection_len={} surrounding_len={} blocked_by_safety={}",
            source,
            selection.as_ref().map(|s| s.chars().count()).unwrap_or(0),
            surrounding.as_ref().map(|s| s.chars().count()).unwrap_or(0),
            blocked_by_safety
        );

        let state = app_for_probe.state::<crate::state::AppState>();
        match kind {
            ProbeKind::QuickAsk => {
                if let Ok(mut probe) = state.quick_ask_probe.lock() {
                    if probe.epoch == epoch {
                        probe.selection_text = selection;
                        probe.surrounding_text = surrounding;
                        probe.source = source;
                        probe.ready = true;
                    }
                }
            }
            ProbeKind::QuickReplace => {
                if let Ok(mut probe) = state.quick_replace_probe.lock() {
                    if probe.epoch == epoch {
                        probe.selection_text = selection;
                        probe.surrounding_text = surrounding;
                        probe.source = source;
                        probe.ready = true;
                    }
                }
            }
        }
    });

    epoch
}

/// Wait for a probe to complete and return the captured selection text.
///
/// Returns `None` if the probe times out, was never started (epoch=0), or captured nothing.
pub async fn await_probe_result(
    app: &AppHandle,
    kind: ProbeKind,
    epoch: u64,
    timeout_ms: u64,
) -> Option<SelectionProbeContext> {
    use std::time::{Duration, Instant};

    if epoch == 0 {
        return None;
    }

    let deadline = Instant::now() + Duration::from_millis(timeout_ms);

    loop {
        let (ready, selection, surrounding, source) = {
            let state = app.state::<crate::state::AppState>();
            match kind {
                ProbeKind::QuickAsk => match state.quick_ask_probe.lock() {
                    Ok(probe) if probe.epoch == epoch => (
                        probe.ready,
                        probe.selection_text.clone(),
                        probe.surrounding_text.clone(),
                        probe.source.clone(),
                    ),
                    _ => (true, None, None, WindowsTextContextSource::None),
                },
                ProbeKind::QuickReplace => match state.quick_replace_probe.lock() {
                    Ok(probe) if probe.epoch == epoch => (
                        probe.ready,
                        probe.selection_text.clone(),
                        probe.surrounding_text.clone(),
                        probe.source.clone(),
                    ),
                    _ => (true, None, None, WindowsTextContextSource::None),
                },
            }
        };

        if ready {
            if selection.is_none() && surrounding.is_none() {
                return None;
            }
            return Some(SelectionProbeContext {
                selection_text: selection,
                surrounding_text: surrounding,
                source,
            });
        }

        if Instant::now() >= deadline {
            return None;
        }

        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}
