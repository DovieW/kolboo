use std::borrow::Cow;
use std::sync::Arc;
use std::sync::OnceLock;

static SENTRY_GUARD: OnceLock<sentry::ClientInitGuard> = OnceLock::new();

const SENSITIVE_MARKERS: &[&str] = &[
    "api_key",
    "apikey",
    "token",
    "authorization",
    "bearer ",
    "password",
    "secret",
    "transcript",
    "ocr",
    "audio",
    "prompt",
];

fn env_non_empty(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

fn looks_sensitive(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    SENSITIVE_MARKERS
        .iter()
        .any(|marker| lower.contains(marker))
}

fn scrub_text(value: &str) -> String {
    if looks_sensitive(value) {
        "[REDACTED]".to_string()
    } else {
        value.to_string()
    }
}

fn scrub_event(
    mut event: sentry::protocol::Event<'static>,
) -> Option<sentry::protocol::Event<'static>> {
    // Never send request/user identity by default.
    event.user = None;
    event.request = None;
    event.server_name = None;

    if let Some(message) = event.message.as_mut() {
        *message = scrub_text(message);
    }

    for exception in event.exception.values.iter_mut() {
        if let Some(value) = exception.value.as_mut() {
            *value = scrub_text(value);
        }
    }

    for (key, value) in event.tags.iter_mut() {
        if looks_sensitive(key) || looks_sensitive(value) {
            *value = "[REDACTED]".to_string();
        }
    }

    Some(event)
}

fn sentry_environment() -> String {
    env_non_empty("TAURI_SENTRY_ENV").unwrap_or_else(|| {
        if cfg!(debug_assertions) {
            "development".to_string()
        } else {
            "production".to_string()
        }
    })
}

pub fn init() {
    if SENTRY_GUARD.get().is_some() {
        return;
    }

    let Some(dsn_raw) = env_non_empty("TAURI_SENTRY_DSN") else {
        log::info!("Backend Sentry disabled (no TAURI_SENTRY_DSN)");
        return;
    };

    let dsn = match dsn_raw.parse() {
        Ok(parsed) => parsed,
        Err(err) => {
            log::warn!("Backend Sentry disabled (invalid DSN): {err}");
            return;
        }
    };

    let release = env_non_empty("TAURI_SENTRY_RELEASE")
        .or_else(|| env_non_empty("TAURI_APP_VERSION"))
        .map(Cow::Owned);

    let options = sentry::ClientOptions {
        dsn: Some(dsn),
        release,
        environment: Some(Cow::Owned(sentry_environment())),
        before_send: Some(Arc::new(scrub_event)),
        ..Default::default()
    };

    let guard = sentry::init(options);

    if sentry::Hub::current().client().is_some() {
        let _ = SENTRY_GUARD.set(guard);
        log::info!("Backend Sentry initialized");
    } else {
        log::warn!("Backend Sentry initialization did not attach a client");
    }
}

pub fn capture_backend_smoke(surface: &str) -> bool {
    if sentry::Hub::current().client().is_none() {
        return false;
    }

    sentry::with_scope(
        |scope| {
            scope.set_tag("runtime", "tauri-backend");
            scope.set_tag("surface", scrub_text(surface));
            scope.set_tag("event_kind", "smoke_test");
        },
        || {
            sentry::capture_message("backend-sentry-smoke", sentry::Level::Info);
        },
    );

    true
}

#[cfg(test)]
mod tests {
    use super::{looks_sensitive, scrub_text};

    #[test]
    fn scrub_text_redacts_sensitive_markers() {
        assert_eq!(scrub_text("authorization bearer token"), "[REDACTED]");
        assert_eq!(scrub_text("user transcript sample"), "[REDACTED]");
    }

    #[test]
    fn scrub_text_keeps_safe_values() {
        assert_eq!(scrub_text("startup health check"), "startup health check");
        assert!(!looks_sensitive("normal-error-category"));
    }
}
