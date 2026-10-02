//! Machine-local main-window preferences. Store logical content dimensions,
//! never monitor coordinates, overlay state, account settings or display zoom.

use std::{sync::Arc, time::Duration};
use tauri::{PhysicalSize, Runtime};
use tauri_plugin_store::{Store, StoreExt};

mod native;
pub(crate) use native::{configure, setup};

const STORE_FILE: &str = "main-window.json";
const STATE_KEY: &str = "main";

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
struct SavedWindow {
    width: f64,
    height: f64,
    maximized: bool,
}

impl Default for SavedWindow {
    fn default() -> Self {
        Self {
            width: 1280.0,
            height: 800.0,
            maximized: false,
        }
    }
}

impl SavedWindow {
    fn valid(self) -> bool {
        self.width.is_finite()
            && self.height.is_finite()
            && (200.0..=16384.0).contains(&self.width)
            && (150.0..=16384.0).contains(&self.height)
    }

    fn bounds(self, work_area: PhysicalSize<u32>, scale: f64) -> WindowBounds {
        // Reserve space for decorations and desktop panels. Even a small or
        // newly connected screen must retain reachable window controls.
        let dimension = |logical: f64, maximum: u32| {
            let available = (f64::from(maximum) - 48.0 * scale).max(1.0);
            (logical * scale).min(available).round().max(1.0) as u32
        };
        WindowBounds {
            size: PhysicalSize::new(
                dimension(self.width, work_area.width),
                dimension(self.height, work_area.height),
            ),
            minimum: PhysicalSize::new(
                dimension(640.0, work_area.width),
                dimension(400.0, work_area.height),
            ),
        }
    }

    fn capture(self, sample: WindowSample) -> Option<Self> {
        // Minimization, fullscreen and maximize-generated resize events must
        // not replace the normal-size preference with transient screen bounds.
        if sample.minimized || sample.fullscreen {
            return None;
        }
        let mut next = Self {
            maximized: sample.maximized,
            ..self
        };
        if !sample.maximized {
            next.width = f64::from(sample.size.width) / sample.scale;
            next.height = f64::from(sample.size.height) / sample.scale;
            if !next.valid() {
                return None;
            }
        }
        (next != self).then_some(next)
    }
}

#[derive(Debug, PartialEq)]
struct WindowBounds {
    size: PhysicalSize<u32>,
    minimum: PhysicalSize<u32>,
}

struct WindowSample {
    size: PhysicalSize<u32>,
    scale: f64,
    maximized: bool,
    minimized: bool,
    fullscreen: bool,
}

fn open_store<R: Runtime>(
    app: &tauri::AppHandle<R>,
    path: impl AsRef<std::path::Path>,
) -> tauri_plugin_store::Result<Arc<Store<R>>> {
    app.store_builder(path)
        .auto_save(Duration::from_millis(300))
        .build()
}

fn read_state<R: Runtime>(store: &Store<R>) -> SavedWindow {
    store
        .get(STATE_KEY)
        .and_then(|value| serde_json::from_value::<SavedWindow>(value).ok())
        .filter(|state| state.valid())
        .unwrap_or_default()
}

fn remember<R: Runtime>(store: &Store<R>, sample: WindowSample) {
    if let Some(next) = read_state(store).capture(sample) {
        store.set(STATE_KEY, serde_json::json!(next));
    }
}

fn work_area_size(area: PhysicalSize<u32>, monitor: PhysicalSize<u32>) -> PhysicalSize<u32> {
    if area.width > 0 && area.height > 0 {
        area
    } else {
        monitor
    }
}

#[cfg(test)]
mod tests;
