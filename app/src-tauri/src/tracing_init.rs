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

        let log_format = std::env::var("KOLBOO_LOG_FORMAT")
            .unwrap_or_else(|_| "json".to_string())
            .to_lowercase();

        // Best-effort: tests or embedded environments may initialize tracing elsewhere.
        let _ = if matches!(log_format.as_str(), "pretty" | "text") {
            tracing_subscriber::registry()
                .with(filter_layer.clone())
                .with(fmt::layer().pretty().with_target(false).with_ansi(true))
                .try_init()
        } else {
            tracing_subscriber::registry()
                .with(filter_layer)
                .with(fmt::layer().json().flatten_event(true).with_target(false))
                .try_init()
        };
    });
}
