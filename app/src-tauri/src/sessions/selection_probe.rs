//! Unified selection probe for Quick Ask and Quick Replace.
//!
//! Both features need to capture the user's currently highlighted text at recording
//! start. This module provides a single implementation that works for both.

use std::sync::atomic::Ordering;

use tauri::{AppHandle, Manager};

use crate::commands::text::ContextGrabMethod;

/// Which feature is requesting the selection probe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeKind {
    QuickAsk,
    QuickReplace,
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
            }
            epoch
        }
    };

    // Spawn the blocking probe task
    let app_for_probe = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let selection = crate::commands::text::probe_selected_text_via_copy_with_app(
            &app_for_probe,
            context_grab_method,
        )
        .ok()
        .flatten();

        let state = app_for_probe.state::<crate::state::AppState>();
        match kind {
            ProbeKind::QuickAsk => {
                if let Ok(mut probe) = state.quick_ask_probe.lock() {
                    if probe.epoch == epoch {
                        probe.selection_text = selection;
                        probe.ready = true;
                    }
                }
            }
            ProbeKind::QuickReplace => {
                if let Ok(mut probe) = state.quick_replace_probe.lock() {
                    if probe.epoch == epoch {
                        probe.selection_text = selection;
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
) -> Option<String> {
    use std::time::{Duration, Instant};

    if epoch == 0 {
        return None;
    }

    let deadline = Instant::now() + Duration::from_millis(timeout_ms);

    loop {
        let (ready, selection) = {
            let state = app.state::<crate::state::AppState>();
            match kind {
                ProbeKind::QuickAsk => match state.quick_ask_probe.lock() {
                    Ok(probe) if probe.epoch == epoch => {
                        (probe.ready, probe.selection_text.clone())
                    }
                    _ => (true, None),
                },
                ProbeKind::QuickReplace => match state.quick_replace_probe.lock() {
                    Ok(probe) if probe.epoch == epoch => {
                        (probe.ready, probe.selection_text.clone())
                    }
                    _ => (true, None),
                },
            }
        };

        if ready {
            return selection;
        }

        if Instant::now() >= deadline {
            return None;
        }

        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// Check if a probe is ready without blocking.
///
/// Returns `(ready, selection_text)`. If epoch is 0 or mismatched, returns `(true, None)`.
pub fn check_probe_ready(app: &AppHandle, kind: ProbeKind, epoch: u64) -> (bool, Option<String>) {
    if epoch == 0 {
        return (true, None);
    }

    let state = app.state::<crate::state::AppState>();
    match kind {
        ProbeKind::QuickAsk => match state.quick_ask_probe.lock() {
            Ok(probe) if probe.epoch == epoch => (probe.ready, probe.selection_text.clone()),
            _ => (true, None),
        },
        ProbeKind::QuickReplace => match state.quick_replace_probe.lock() {
            Ok(probe) if probe.epoch == epoch => (probe.ready, probe.selection_text.clone()),
            _ => (true, None),
        },
    }
}
