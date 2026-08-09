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
    "cookie",
    "password",
    "secret",
    "clipboard",
    "completion",
    "transcript",
    "ocr",
    "audio",
    "wav",
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

fn scrub_protocol_value_map(map: &mut sentry::protocol::Map<String, sentry::protocol::Value>) {
    for (key, value) in map.iter_mut() {
        if looks_sensitive(key) {
            *value = sentry::protocol::Value::String("[REDACTED]".to_string());
            continue;
        }
        scrub_json_value(value);
    }
}

fn scrub_json_object(map: &mut sentry::protocol::value::Map<String, sentry::protocol::Value>) {
    for (key, value) in map.iter_mut() {
        if looks_sensitive(key) {
            *value = sentry::protocol::Value::String("[REDACTED]".to_string());
            continue;
        }
        scrub_json_value(value);
    }
}

fn scrub_json_value(value: &mut sentry::protocol::Value) {
    match value {
        sentry::protocol::Value::String(text) => {
            *text = scrub_text(text);
        }
        sentry::protocol::Value::Array(entries) => {
            for entry in entries {
                scrub_json_value(entry);
            }
        }
        sentry::protocol::Value::Object(map) => {
            scrub_json_object(map);
        }
        _ => {}
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

    scrub_protocol_value_map(&mut event.extra);

    for breadcrumb in &mut event.breadcrumbs.values {
        if let Some(message) = breadcrumb.message.as_mut() {
            *message = scrub_text(message);
        }
        scrub_protocol_value_map(&mut breadcrumb.data);
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

    let mut options = sentry::ClientOptions::default();
    options.dsn = Some(dsn);
    options.release = release;
    options.environment = Some(Cow::Owned(sentry_environment()));
    options.before_send = Some(Arc::new(scrub_event));

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
    use super::{looks_sensitive, scrub_event, scrub_text};
    use sentry::protocol::{
        value::Map as JsonMap, Breadcrumb, Event, Exception, Map, Request, User, Value,
    };

    #[test]
    fn scrub_text_redacts_sensitive_markers() {
        assert_eq!(scrub_text("authorization bearer token"), "[REDACTED]");
        assert_eq!(scrub_text("user transcript sample"), "[REDACTED]");
        assert_eq!(scrub_text("clipboard restore payload"), "[REDACTED]");
        assert_eq!(scrub_text("completion preview"), "[REDACTED]");
    }

    #[test]
    fn scrub_text_keeps_safe_values() {
        assert_eq!(scrub_text("startup health check"), "startup health check");
        assert!(!looks_sensitive("normal-error-category"));
    }

    #[test]
    fn scrub_event_removes_identity_and_redacts_nested_payloads() {
        let mut extra = Map::new();
        extra.insert(
            "clipboard_contents".to_string(),
            Value::String("copied text".to_string()),
        );
        extra.insert(
            "safe_nested".to_string(),
            Value::Object({
                let mut nested = JsonMap::new();
                nested.insert(
                    "completion_text".to_string(),
                    Value::String("rewritten answer".to_string()),
                );
                nested.insert("safe_value".to_string(), Value::String("ok".to_string()));
                nested
            }),
        );

        let breadcrumb = Breadcrumb {
            message: Some("prompt payload".to_string()),
            data: {
                let mut data = Map::new();
                data.insert(
                    "ocr_payload".to_string(),
                    Value::String("screen text".to_string()),
                );
                data.insert("safe_flag".to_string(), Value::Bool(true));
                data
            },
            ..Breadcrumb::default()
        };

        let event = Event {
            user: Some(User::default()),
            request: Some(Request::default()),
            server_name: Some("desktop-host".into()),
            message: Some("user transcript sample".to_string()),
            exception: vec![Exception {
                value: Some("authorization bearer token".to_string()),
                ..Exception::default()
            }]
            .into(),
            tags: {
                let mut tags = Map::new();
                tags.insert("clipboard_state".to_string(), "present".to_string());
                tags.insert("safe".to_string(), "ok".to_string());
                tags
            },
            extra,
            breadcrumbs: vec![breadcrumb].into(),
            ..Event::default()
        };

        let safe = scrub_event(event).expect("event should survive redaction");

        assert!(safe.user.is_none());
        assert!(safe.request.is_none());
        assert!(safe.server_name.is_none());
        assert_eq!(safe.message.as_deref(), Some("[REDACTED]"));
        assert_eq!(
            safe.exception.values[0].value.as_deref(),
            Some("[REDACTED]")
        );
        assert_eq!(
            safe.tags.get("clipboard_state").map(String::as_str),
            Some("[REDACTED]")
        );
        assert_eq!(safe.tags.get("safe").map(String::as_str), Some("ok"));
        assert_eq!(
            safe.extra.get("clipboard_contents"),
            Some(&Value::String("[REDACTED]".to_string()))
        );

        let nested = safe
            .extra
            .get("safe_nested")
            .and_then(Value::as_object)
            .expect("nested extra object should stay present");
        assert_eq!(
            nested.get("completion_text"),
            Some(&Value::String("[REDACTED]".to_string()))
        );
        assert_eq!(
            nested.get("safe_value"),
            Some(&Value::String("ok".to_string()))
        );

        let breadcrumb = &safe.breadcrumbs.values[0];
        assert_eq!(breadcrumb.message.as_deref(), Some("[REDACTED]"));
        assert_eq!(
            breadcrumb.data.get("ocr_payload"),
            Some(&Value::String("[REDACTED]".to_string()))
        );
        assert_eq!(breadcrumb.data.get("safe_flag"), Some(&Value::Bool(true)));
    }
}
