use std::path::PathBuf;
use std::sync::{Mutex, Once, OnceLock};

use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{fmt, EnvFilter};

static INIT: Once = Once::new();
static LOG_DIR: OnceLock<PathBuf> = OnceLock::new();

/// Must be held for the app's lifetime so the non-blocking writer flushes on exit.
static FILE_LOG_GUARD: Mutex<Option<WorkerGuard>> = Mutex::new(None);
static FILE_LOG_ERRORS: OnceLock<tracing_appender::non_blocking::ErrorCounter> = OnceLock::new();
// The dependency default is 128,000 queued records. A stalled disk must not
// retain a large backlog of formatted diagnostic strings in the GUI process.
const FILE_LOG_QUEUE_CAPACITY: usize = 1024;

pub(crate) fn dropped_lines() -> u64 {
    FILE_LOG_ERRORS
        .get()
        .map_or(0, |counter| counter.dropped_lines() as u64)
}

pub(crate) fn shutdown() {
    // Static values are not dropped automatically at process exit.
    drop(FILE_LOG_GUARD.lock().unwrap().take());
}

#[cfg(test)]
pub(crate) fn overflowing_test_writer() -> (std::sync::mpsc::Sender<()>, WorkerGuard) {
    use std::io::Write;
    struct PausedSink(std::sync::mpsc::Sender<()>, std::sync::mpsc::Receiver<()>);
    impl Write for PausedSink {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            let _ = self.0.send(());
            let _ = self.1.recv();
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let (entered_tx, entered_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let (mut writer, guard) = tracing_appender::non_blocking::NonBlockingBuilder::default()
        .buffered_lines_limit(FILE_LOG_QUEUE_CAPACITY)
        .finish(PausedSink(entered_tx, release_rx));
    writer.write_all(b"first").unwrap();
    entered_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    for _ in 0..FILE_LOG_QUEUE_CAPACITY {
        writer.write_all(b"queued").unwrap();
    }
    writer.write_all(b"dropped").unwrap();
    assert_eq!(writer.error_counter().dropped_lines(), 1);
    assert!(FILE_LOG_ERRORS.set(writer.error_counter()).is_ok());
    (release_tx, guard)
}

/// Returns the directory where rolling log files are written, if file logging was
/// successfully initialized.  Available immediately after [`init`] returns.
pub fn log_dir() -> Option<&'static PathBuf> {
    LOG_DIR.get()
}

/// Resolve the log directory without the Tauri runtime.
///
/// 1. `KOLBOO_APP_DATA_DIR` env var (same override as [`crate::app_paths`]) + `/logs`
/// 2. Platform-specific default matching Tauri 2's `app_data_dir` + `/logs`
fn resolve_log_dir() -> Option<PathBuf> {
    if let Ok(val) = std::env::var(crate::app_paths::APP_DATA_DIR_ENV) {
        let trimmed = val.trim();
        if !trimmed.is_empty() {
            return Some(PathBuf::from(trimmed).join("logs"));
        }
    }
    platform_app_data_dir().map(|d| d.join("logs"))
}

#[cfg(target_os = "windows")]
fn platform_app_data_dir() -> Option<PathBuf> {
    std::env::var("APPDATA")
        .ok()
        .map(|d| PathBuf::from(d).join("com.kolboo.app"))
}

#[cfg(target_os = "macos")]
fn platform_app_data_dir() -> Option<PathBuf> {
    std::env::var("HOME")
        .ok()
        .map(|h| PathBuf::from(h).join("Library/Application Support/com.kolboo.app"))
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn platform_app_data_dir() -> Option<PathBuf> {
    let data = std::env::var("XDG_DATA_HOME")
        .ok()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var("HOME")
                .ok()
                .map(|h| PathBuf::from(h).join(".local/share"))
        });
    data.map(|d| d.join("com.kolboo.app"))
}

/// Build an [`EnvFilter`] from an explicit directive string, used as a fallback
/// when `RUST_LOG` is not set.
fn make_filter(default: &str) -> EnvFilter {
    EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(default))
        .unwrap_or_else(|_| EnvFilter::new(default))
}

/// Default file log filter: info for all crates, debug for our own code.
/// This avoids noisy debug output from third-party crates (keyring, hyper,
/// reqwest, etc.) while keeping detailed troubleshooting data from kolboo.
const FILE_LOG_DEFAULT: &str = "info,kolboo=debug";

/// Initialize structured tracing.
///
/// - Routes `log` crate events into tracing (so existing `log::info!` calls get span context).
/// - Console: defaults to `info` (override via `RUST_LOG`).
/// - File: defaults to `info,kolboo=debug` — our code at debug, dependencies at info.
///   Override with `KOLBOO_FILE_LOG` env var if needed (e.g. `debug` to see everything).
/// - Console output format controlled by `KOLBOO_LOG_FORMAT` (`json` (default) or `pretty`/`text`).
/// - Writes daily-rotated log files to `<app-data>/logs/` (7 day retention).
pub fn init() {
    INIT.call_once(|| {
        // Best-effort: if another logger was already set, don't crash.
        let _ = tracing_log::LogTracer::init();

        // File filter: KOLBOO_FILE_LOG env var, or default to info + kolboo=debug.
        let file_filter_directive =
            std::env::var("KOLBOO_FILE_LOG").unwrap_or_else(|_| FILE_LOG_DEFAULT.to_string());
        let file_filter = EnvFilter::try_new(&file_filter_directive)
            .unwrap_or_else(|_| EnvFilter::new(FILE_LOG_DEFAULT));

        let is_pretty = {
            let fmt = std::env::var("KOLBOO_LOG_FORMAT")
                .unwrap_or_else(|_| "json".to_string())
                .to_lowercase();
            matches!(fmt.as_str(), "pretty" | "text")
        };
        let is_cli_invocation = crate::cli::is_cli_invocation();

        // Console layers (only one will be Some based on format preference).
        // Each layer gets its own filter so console and file can differ.
        // CLI mode reserves stdout for the final machine-readable envelope.
        // Progress that is intentionally user-visible should use stderr from
        // the CLI command itself; structured tracing still goes to the file log.
        let console_pretty = (!is_cli_invocation && is_pretty).then(|| {
            fmt::layer()
                .pretty()
                .with_target(false)
                .with_ansi(true)
                .with_filter(make_filter("info"))
        });
        let console_json = (!is_cli_invocation && !is_pretty).then(|| {
            fmt::layer()
                .json()
                .flatten_event(true)
                .with_target(false)
                .with_filter(make_filter("info"))
        });

        // File layer: daily rotation, 7-day retention, plain text for readability.
        let file_layer = resolve_log_dir().and_then(|log_dir| {
            if let Err(e) = std::fs::create_dir_all(&log_dir) {
                eprintln!(
                    "Warning: could not create log directory {}: {e}",
                    log_dir.display()
                );
                return None;
            }

            let appender = tracing_appender::rolling::RollingFileAppender::builder()
                .rotation(tracing_appender::rolling::Rotation::DAILY)
                .filename_prefix("kolboo")
                .filename_suffix("log")
                .max_log_files(7)
                .build(&log_dir)
                .ok()?;

            let (non_blocking, guard) =
                tracing_appender::non_blocking::NonBlockingBuilder::default()
                    .buffered_lines_limit(FILE_LOG_QUEUE_CAPACITY)
                    .finish(appender);
            let _ = FILE_LOG_ERRORS.set(non_blocking.error_counter());
            *FILE_LOG_GUARD.lock().unwrap() = Some(guard);
            let _ = LOG_DIR.set(log_dir);

            Some(
                fmt::layer()
                    .with_target(false)
                    .with_ansi(false)
                    .with_writer(non_blocking)
                    .with_filter(file_filter),
            )
        });

        // Best-effort: tests or embedded environments may initialize tracing elsewhere.
        let _ = tracing_subscriber::registry()
            .with(console_pretty)
            .with(console_json)
            .with(file_layer)
            .try_init();
    });
}
