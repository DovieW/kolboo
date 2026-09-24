use super::*;
use sentry::protocol::{Envelope, EnvelopeItem, Event};
use serde_json::json;
use std::sync::Mutex;

#[test]
fn removes_unmarked_content_without_losing_stack_locations_and_symbols() {
    let event: Event<'static> = serde_json::from_value(json!({
        "level": "fatal", "release": "kolboo@1.2.3", "environment": "beta",
        "message": "PRIVATE meeting notes", "server_name": "PRIVATE laptop",
        "user": { "email": "PRIVATE@example.test" },
        "request": { "url": "https://private.example.test" },
        "extra": { "response": "PRIVATE text" },
        "breadcrumbs": {"values": [{ "message": "PRIVATE text" }]},
        "transaction": "PRIVATE operation", "fingerprint": ["PRIVATE group"],
        "tags": { "surface": "main", "event_kind": "smoke_test", "other": "PRIVATE" },
        "contexts": {
            "os": { "type": "os", "name": "Linux", "version": "6.8", "other_field": "PRIVATE" },
            "runtime": { "type": "runtime", "name": "rustc", "version": "1.98", "other_field": "PRIVATE" },
            "device": { "type": "device", "name": "PRIVATE laptop" }
        },
        "exception": {"values": [{ "type": "panic", "value": "PRIVATE notes", "thread_id": "PRIVATE thread",
            "mechanism": { "type": "panic", "handled": false, "description": "PRIVATE description", "data": {"body": "PRIVATE"} },
            "stacktrace": { "frames": [{ "filename": "/home/PRIVATE/main.rs", "abs_path": "/home/PRIVATE/main.rs", "function": "kolboo::record", "lineno": 42, "in_app": true, "vars": {"body": "PRIVATE"}, "pre_context": ["PRIVATE"], "post_context": ["PRIVATE"], "context_line": "PRIVATE" }], "registers": {"rax": "0x1234"} },
            "raw_stacktrace": { "frames": [] }
        }, { "type": "PRIVATE exception", "value": "PRIVATE" }]},
        "stacktrace": { "frames": [{ "filename": "C:\\Users\\PRIVATE\\helper.rs" }] },
        "debug_meta": { "images": [
            { "type": "symbolic", "name": "/home/PRIVATE/kolboo", "arch": "x86_64", "image_addr": "0x1000", "image_size": 10, "id": "00000000-0000-0000-0000-000000000001", "debug_file": "C:\\Users\\PRIVATE\\kolboo.pdb" },
            { "type": "apple", "name": "/Users/PRIVATE/kolboo", "image_addr": "0x1000", "image_size": 10, "uuid": "00000000-0000-0000-0000-000000000002" },
            { "type": "wasm", "name": "/PRIVATE/code.wasm", "debug_id": "00000000-0000-0000-0000-000000000003", "code_file": "/PRIVATE/code.wasm", "debug_file": "/PRIVATE/code.debug" },
            { "type": "proguard", "uuid": "00000000-0000-0000-0000-000000000004" }
        ] }
    })).unwrap();
    let safe = scrub_event(event).unwrap();
    let encoded = serde_json::to_string(&safe).unwrap();
    assert!(!encoded.to_lowercase().contains("private"), "{encoded}");
    assert_eq!(safe.level, sentry::Level::Fatal);
    assert_eq!(safe.release.as_deref(), Some("kolboo@1.2.3"));
    let frame = &safe.exception.values[0].stacktrace.as_ref().unwrap().frames[0];
    assert_eq!(frame.filename.as_deref(), Some("main.rs"));
    assert_eq!(frame.function.as_deref(), Some("kolboo::record"));
    assert_eq!(frame.lineno, Some(42));
    assert_eq!(safe.exception.values[0].ty, "panic");
    assert_eq!(safe.exception.values[1].ty, "Error");
    assert_eq!(
        safe.exception.values[0].mechanism.as_ref().unwrap().handled,
        Some(false)
    );
    assert_eq!(safe.debug_meta.images.len(), 4);
    assert!(encoded.contains("00000000-0000-0000-0000-000000000001"));
    assert_eq!(safe.tags["runtime"], "tauri-backend");
    assert_eq!(safe.tags["service"], "desktop");
    assert_eq!(safe.tags["surface"], "main");
    assert_eq!(safe.tags["os"], std::env::consts::OS);
    assert_eq!(safe.contexts.len(), 2);
    assert_eq!(scrub_event(Event::default()).unwrap().message, None);
}

#[test]
fn config_normalization_and_cloud_free_initialization() {
    assert_eq!(compiled_non_empty(None), None);
    assert_eq!(compiled_non_empty(Some("  ")), None);
    assert_eq!(compiled_non_empty(Some(" beta ")), Some("beta".into()));
    assert_eq!(
        sentry_config_value("KOLBOO_TEST_NONEXISTENT_SENTRY_CONFIG", Some(" fallback ")),
        Some("fallback".into())
    );
    let guard = OnceLock::new();
    init_once(&guard, None, "release".into(), "test".into());
    init_once(
        &guard,
        Some("not-a-dsn".into()),
        "release".into(),
        "test".into(),
    );
    assert!(guard.get().is_none());
    // The public entry point is also exercised without mutating process env.
    sentry::Hub::run(Arc::new(sentry::Hub::new(None, Arc::default())), || {
        init();
        assert_eq!(
            SENTRY_GUARD.get().is_some(),
            sentry::Hub::current().client().is_some()
        );
    });
}

#[test]
fn initialization_is_idempotent_and_error_only() {
    sentry::Hub::run(Arc::new(sentry::Hub::new(None, Arc::default())), || {
        let guard = OnceLock::new();
        init_once(
            &guard,
            Some("https://public@example.invalid/1".into()),
            "kolboo@test".into(),
            "test".into(),
        );
        let client = sentry::Hub::current().client().unwrap();
        let options = client.options();
        assert!(!options.send_default_pii);
        assert!(!options.enable_logs);
        assert!(!options.enable_metrics);
        assert_eq!(options.max_breadcrumbs, 0);
        assert_eq!(options.release.as_deref(), Some("kolboo@test"));
        assert_eq!(options.environment.as_deref(), Some("test"));
        assert!(options.before_send.as_ref().unwrap()(Event::default()).is_some());
        init_once(&guard, None, "changed".into(), "production".into());
        assert!(Arc::ptr_eq(
            &client,
            &sentry::Hub::current().client().unwrap()
        ));
    });
}

struct TestTransport {
    envelopes: Mutex<Vec<Envelope>>,
    flush_result: bool,
}

impl sentry::Transport for TestTransport {
    fn send_envelope(&self, envelope: Envelope) {
        self.envelopes.lock().unwrap().push(envelope);
    }
    fn flush(&self, _timeout: std::time::Duration) -> bool {
        self.flush_result
    }
}

#[test]
fn smoke_requires_nonproduction_valid_surface_and_successful_flush() {
    sentry::Hub::run(Arc::new(sentry::Hub::new(None, Arc::default())), || {
        assert!(!capture_backend_smoke("main"));
    });
    for (environment, surface, flush_result, expected_count, expected) in [
        ("test", "main", true, 1, true),
        ("test", "main", false, 1, false),
        ("production", "main", true, 0, false),
        ("prod", "main", true, 0, false),
        ("unknown", "main", true, 0, false),
        ("test", "PRIVATE arbitrary surface", true, 0, false),
    ] {
        let transport = Arc::new(TestTransport {
            envelopes: Mutex::default(),
            flush_result,
        });
        let mut options = sentry::ClientOptions::default();
        options.dsn = Some("https://public@example.invalid/1".parse().unwrap());
        options.environment = Some(Cow::Borrowed(environment));
        options.transport = Some(Arc::new(transport.clone()));
        options.before_send = Some(Arc::new(scrub_event));
        let client = sentry::Client::from_config(options);
        let hub = Arc::new(sentry::Hub::new(Some(Arc::new(client)), Arc::default()));
        sentry::Hub::run(hub, || assert_eq!(capture_backend_smoke(surface), expected));
        let envelopes = transport.envelopes.lock().unwrap();
        let events: Vec<_> = envelopes
            .iter()
            .flat_map(Envelope::items)
            .filter_map(|item| match item {
                EnvelopeItem::Event(event) => Some(event),
                _ => None,
            })
            .collect();
        assert_eq!(events.len(), expected_count);
        if let Some(event) = events.first() {
            assert_eq!(event.tags["event_kind"], "smoke_test");
            assert_eq!(event.tags["surface"], "main");
            assert_eq!(
                event.message.as_deref(),
                Some("Backend error (details withheld)")
            );
        }
    }
}
