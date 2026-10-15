use std::borrow::Cow;
use std::sync::Arc;
use std::sync::OnceLock;

static SENTRY_GUARD: OnceLock<sentry::ClientInitGuard> = OnceLock::new();
static ACCOUNT_REPORTS: std::sync::Mutex<Vec<(String, std::time::Instant)>> =
    std::sync::Mutex::new(Vec::new());

fn safe_account_code(code: &str) -> &'static str {
    match code {
        "refresh_token_not_found" => "refresh_token_not_found",
        "refresh_token_already_used" => "refresh_token_already_used",
        "session_not_found" => "session_not_found",
        "session_expired" => "session_expired",
        "user_banned" => "user_banned",
        "reauth_required" => "reauth_required",
        "network" => "network",
        "timeout" => "timeout",
        "invalid_response" => "invalid_response",
        "auth_session_save_failed" => "auth_session_save_failed",
        "http_rejected" => "http_rejected",
        _ => "unknown",
    }
}

fn safe_account_tag(key: &str, value: &str) -> bool {
    match key {
        "account_stage" => matches!(
            value,
            "session_refresh" | "entitlement" | "model_catalog" | "session_storage"
        ),
        "account_code" => value == safe_account_code(value),
        "http_status" => {
            value == "none"
                || value.parse::<u16>().is_ok_and(|status| {
                    (100..=599).contains(&status) && status.to_string() == value
                })
        }
        _ => false,
    }
}

/// Explicit handled-failure reporting. No arbitrary text, identity, URL, token,
/// request or response body enters logs/Sentry. At most one event per finite
/// category/status every five minutes; ordinary logs remain local.
pub(crate) fn report_account_failure(stage: &str, code: &str, status: Option<u16>) {
    if !safe_account_tag("account_stage", stage) {
        return;
    }
    let code = safe_account_code(code);
    let status = status
        .filter(|status| (100..=599).contains(status))
        .map(|status| status.to_string())
        .unwrap_or_else(|| "none".into());
    log::warn!(
        "Account operation failed: stage={} code={} http_status={}",
        stage,
        code,
        status
    );
    let Some(client) = sentry::Hub::current().client() else {
        return;
    };
    let key = format!("{stage}:{code}:{status}");
    let now = std::time::Instant::now();
    let mut reports = ACCOUNT_REPORTS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    reports.retain(|(_, at)| now.duration_since(*at) < std::time::Duration::from_secs(300));
    if reports.iter().any(|(existing, _)| existing == &key) {
        return;
    }
    reports.push((key, now));
    drop(reports);
    let mut event = sentry::protocol::Event {
        level: sentry::Level::Error,
        message: Some("Account operation failed".into()),
        ..Default::default()
    };
    event
        .tags
        .insert("event_kind".into(), "account_failure".into());
    event.tags.insert("account_stage".into(), stage.into());
    event.tags.insert("account_code".into(), code.into());
    event.tags.insert("http_status".into(), status);
    client.capture_event(event, None);
}

fn env_non_empty(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

fn compiled_non_empty(value: Option<&'static str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn sentry_config_value(key: &str, compiled_value: Option<&'static str>) -> Option<String> {
    env_non_empty(key).or_else(|| compiled_non_empty(compiled_value))
}

fn code_filename(value: &str) -> String {
    value
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or_default()
        .to_string()
}

fn scrub_stack(stack: &mut sentry::protocol::Stacktrace) {
    stack.registers.clear();
    for frame in &mut stack.frames {
        frame.filename = frame.filename.as_deref().map(code_filename);
        frame.package = frame.package.as_deref().map(code_filename);
        frame.abs_path = None;
        frame.vars.clear();
        frame.pre_context.clear();
        frame.post_context.clear();
        frame.context_line = None;
    }
}

fn scrub_event(
    mut event: sentry::protocol::Event<'static>,
) -> Option<sentry::protocol::Event<'static>> {
    use sentry::protocol::{Context, DebugImage, Event, Mechanism};
    // Panic payloads and errors may include arbitrary provider/user content,
    // even when no sensitive keyword occurs. Retain code, not message bodies.
    for exception in &mut event.exception.values {
        exception.value = Some("Error details withheld for privacy".into());
        if exception.ty != "panic" {
            exception.ty = "Error".into();
        }
        exception.thread_id = None;
        if let Some(mechanism) = exception.mechanism.take() {
            exception.mechanism = Some(Mechanism {
                ty: mechanism.ty,
                handled: mechanism.handled,
                ..Default::default()
            });
        }
        for stack in [&mut exception.stacktrace, &mut exception.raw_stacktrace]
            .into_iter()
            .flatten()
        {
            scrub_stack(stack);
        }
    }
    if let Some(stack) = &mut event.stacktrace {
        scrub_stack(stack);
    }
    event.tags.retain(|key, value| {
        matches!(
            (key.as_str(), value.as_str()),
            (
                "surface",
                "main" | "overlay" | "overlay_hover" | "quick_ask"
            ) | ("event_kind", "smoke_test" | "account_failure")
        ) || safe_account_tag(key, value)
    });
    event.tags.insert("service".into(), "desktop".into());
    event.tags.insert("runtime".into(), "tauri-backend".into());
    event.tags.insert("os".into(), std::env::consts::OS.into());
    event
        .tags
        .insert("arch".into(), std::env::consts::ARCH.into());
    event
        .contexts
        .retain(|key, context| match (key.as_str(), context) {
            ("os", Context::Os(os)) => {
                os.other.clear();
                true
            }
            ("runtime", Context::Runtime(runtime)) => {
                runtime.other.clear();
                true
            }
            _ => false,
        });
    for image in &mut event.debug_meta.to_mut().images {
        match image {
            DebugImage::Apple(image) => image.name = code_filename(&image.name),
            DebugImage::Symbolic(image) => {
                image.name = code_filename(&image.name);
                image.debug_file = image.debug_file.as_deref().map(code_filename);
            }
            DebugImage::Wasm(image) => {
                image.name = code_filename(&image.name);
                image.code_file = code_filename(&image.code_file);
                image.debug_file = image.debug_file.as_deref().map(code_filename);
            }
            DebugImage::Proguard(_) => {}
        }
    }
    let account_failure = event
        .tags
        .get("event_kind")
        .is_some_and(|value| value == "account_failure");
    let fingerprint = if account_failure {
        ["account_stage", "account_code", "http_status"]
            .into_iter()
            .map(|key| {
                Cow::Owned(
                    event
                        .tags
                        .get(key)
                        .cloned()
                        .unwrap_or_else(|| "unknown".into()),
                )
            })
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    Some(Event {
        event_id: event.event_id,
        timestamp: event.timestamp,
        level: event.level,
        platform: event.platform,
        release: event.release,
        environment: event.environment,
        dist: event.dist,
        sdk: event.sdk,
        fingerprint: Cow::Owned(fingerprint),
        message: if account_failure {
            Some("Account operation failed".into())
        } else {
            event
                .message
                .map(|_| "Backend error (details withheld)".into())
        },
        exception: event.exception,
        stacktrace: event.stacktrace,
        tags: event.tags,
        contexts: event.contexts,
        debug_meta: event.debug_meta,
        ..Default::default()
    })
}

fn sentry_environment() -> String {
    sentry_config_value("TAURI_SENTRY_ENV", option_env!("TAURI_SENTRY_ENV")).unwrap_or_else(|| {
        if cfg!(debug_assertions) {
            "development".to_string()
        } else {
            "production".to_string()
        }
    })
}

pub fn init() {
    init_once(
        &SENTRY_GUARD,
        sentry_config_value("TAURI_SENTRY_DSN", option_env!("TAURI_SENTRY_DSN")),
        sentry_config_value("TAURI_SENTRY_RELEASE", option_env!("TAURI_SENTRY_RELEASE"))
            .unwrap_or_else(|| format!("kolboo@{}", env!("CARGO_PKG_VERSION"))),
        sentry_environment(),
    );
}

fn init_once(
    guard_slot: &OnceLock<sentry::ClientInitGuard>,
    dsn_raw: Option<String>,
    release: String,
    environment: String,
) {
    if guard_slot.get().is_some() {
        return;
    }

    let Some(dsn_raw) = dsn_raw else {
        log::info!("Backend Sentry disabled (no TAURI_SENTRY_DSN)");
        return;
    };

    let dsn = match dsn_raw.parse() {
        Ok(parsed) => parsed,
        Err(_) => {
            log::warn!("Backend Sentry disabled (invalid DSN)");
            return;
        }
    };

    let mut options = sentry::ClientOptions::default();
    options.dsn = Some(dsn);
    options.release = Some(Cow::Owned(release));
    options.environment = Some(Cow::Owned(environment));
    options.send_default_pii = false;
    options.enable_logs = false;
    options.enable_metrics = false;
    options.max_breadcrumbs = 0;
    options.before_send = Some(Arc::new(scrub_event));

    let guard = sentry::init(options);

    let _ = guard_slot.set(guard);
    log::info!("Backend Sentry initialized");
}

pub fn capture_backend_smoke(surface: &str) -> bool {
    let Some(client) = sentry::Hub::current().client() else {
        return false;
    };
    if !matches!(surface, "main" | "overlay" | "overlay_hover" | "quick_ask")
        || !matches!(
            client.options().environment.as_deref(),
            Some("development" | "test" | "preview" | "beta")
        )
    {
        return false;
    }

    sentry::with_scope(
        |scope| {
            scope.set_tag("runtime", "tauri-backend");
            scope.set_tag("surface", surface);
            scope.set_tag("event_kind", "smoke_test");
        },
        || {
            sentry::capture_message("backend-sentry-smoke", sentry::Level::Info);
        },
    );

    client.flush(Some(std::time::Duration::from_secs(2)))
}

#[cfg(test)]
#[path = "sentry_init/tests.rs"]
mod tests;
