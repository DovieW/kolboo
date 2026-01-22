use std::sync::Once;

use tracing_subscriber::prelude::*;
use tracing_subscriber::{fmt, EnvFilter};

static INIT: Once = Once::new();

/// Initialize structured tracing.
///
/// - Routes `log` crate events into tracing (so existing `log::info!` calls get span context).
/// - Uses `RUST_LOG` for filtering (same as env_logger), defaulting to `info`.
/// - Emits JSON logs so we can correlate by `request_id` and other span fields.
pub fn init() {
    INIT.call_once(|| {
        // Best-effort: if another logger was already set, don't crash.
        let _ = tracing_log::LogTracer::init();

        let filter_layer = EnvFilter::try_from_default_env()
            .or_else(|_| EnvFilter::try_new("info"))
            .unwrap_or_else(|_| EnvFilter::new("info"));

        let fmt_layer = fmt::layer().json().flatten_event(true).with_target(false);

        // Best-effort: tests or embedded environments may initialize tracing elsewhere.
        let _ = tracing_subscriber::registry()
            .with(filter_layer)
            .with(fmt_layer)
            .try_init();
    });
}
