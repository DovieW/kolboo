use std::borrow::Cow;
use std::sync::Arc;
use std::sync::OnceLock;

static SENTRY_GUARD: OnceLock<sentry::ClientInitGuard> = OnceLock::new();

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
            ) | ("event_kind", "smoke_test")
        )
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
    Some(Event {
        event_id: event.event_id,
        timestamp: event.timestamp,
        level: event.level,
        platform: event.platform,
        release: event.release,
        environment: event.environment,
        dist: event.dist,
        sdk: event.sdk,
        message: event
            .message
            .map(|_| "Backend error (details withheld)".into()),
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
