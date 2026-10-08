use super::*;
use tauri::test::{mock_builder, mock_context, noop_assets, MockRuntime};
use wiremock::{matchers::path, Mock, MockServer, ResponseTemplate};

// Synthetic fixture key only; never the release key. The private key is not
// retained. The payload is deliberately not an installer or user content.
const PUBLIC_KEY: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDc1MkM0NDA3NDBCNDNFMjAKUldRZ1ByUkFCMFFzZFNvQkRQSEwwU2RrcklvVm1LL2thT2xFb05RampMQVIyZkw4RGNsdWp0Q0oK";
const SIGNATURE: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IHNpZ25hdHVyZSBmcm9tIHRhdXJpIHNlY3JldCBrZXkKUlVRZ1ByUkFCMFFzZFFVYm5IYXNQTktDeXk4ZWFuMnFPZWZEbVowWHhTWUFDNWJNR1RHRkFkN2FKQkw1Y20vdHNSa3ZRbTAxajhKdHZpM3ZOcDcvQkVyV1Y0b1ZZVDhUWHdBPQp0cnVzdGVkIGNvbW1lbnQ6IHRpbWVzdGFtcDoxNzkxNDIzMDY2CWZpbGU6a29sYm9vLXVwZGF0ZXItZml4dHVyZS50eHQKdERTVzBHMGsyQW5ma21RdXNyUW5zVkFZWU56dzB2SHJ1bURxOVhONDhXM2hIc1dIeE5LdzVzS2t6RWNmRktxRmloLzVON1hsTnRuYVh4cTE0blFKQkE9PQo=";
const PAYLOAD: &[u8] = b"synthetic updater payload, not an executable\n";

fn test_app(server: &MockServer) -> tauri::App<MockRuntime> {
    let mut context = mock_context(noop_assets());
    configure_context(&mut context, server);
    mock_builder()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(PendingUpdate::default())
        .build(context)
        .unwrap()
}

fn configure_context<R: Runtime>(context: &mut tauri::Context<R>, server: &MockServer) {
    context.config_mut().plugins.0.insert(
        "updater".into(),
        serde_json::json!({
            "pubkey": PUBLIC_KEY,
            "endpoints": [format!("{}/latest", server.uri())],
            "dangerousInsecureTransportProtocol": true,
        }),
    );
}

#[cfg(target_os = "linux")]
pub(crate) fn native_context() -> (tauri::Context<tauri::Wry>, MockServer) {
    let server = tauri::async_runtime::block_on(MockServer::start());
    let mut context = mock_context(noop_assets());
    configure_context(&mut context, &server);
    (context, server)
}

async fn release(server: &MockServer, version: &str, body: &[u8]) {
    server.reset().await;
    Mock::given(path("/latest"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "version": version,
            "platforms": { tauri_plugin_updater::target().unwrap(): {
                "url": format!("{}/installer", server.uri()),
                "signature": SIGNATURE,
            }},
        })))
        .mount(server)
        .await;
    Mock::given(path("/installer"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(body))
        .mount(server)
        .await;
}

#[tokio::test]
async fn stages_only_signed_newer_versions_and_deduplicates_downloads() {
    let server = MockServer::start().await;
    let app = test_app(&server);
    setup(app.handle(), false);
    let pending = app.state::<PendingUpdate>();
    assert!(pending.download.lock().unwrap().is_none());
    release(&server, "0.0.0", PAYLOAD).await;
    stage(app.handle()).await.unwrap();
    assert!(pending.download.lock().unwrap().is_none());
    release(&server, "99.0.0", PAYLOAD).await;
    stage(app.handle()).await.unwrap();
    stage(app.handle()).await.unwrap();
    assert_eq!(
        server
            .received_requests()
            .await
            .unwrap()
            .iter()
            .filter(|request| request.url.path() == "/installer")
            .count(),
        1
    );
    assert_eq!(
        pending.download.lock().unwrap().as_ref().unwrap().1,
        PAYLOAD
    );
    release(&server, "99.0.1", b"tampered payload").await;
    assert!(stage(app.handle()).await.is_err());
    assert_eq!(
        pending.download.lock().unwrap().as_ref().unwrap().0.version,
        "99.0.0"
    );
    server.reset().await;
    assert!(stage(app.handle()).await.is_err());
    assert_eq!(
        pending.download.lock().unwrap().as_ref().unwrap().1,
        PAYLOAD
    );
    assert!(!pending.shutdown.is_cancelled());
    pending.shutdown.cancel();
    assert!(pending.shutdown.is_cancelled());
}

#[tokio::test]
async fn background_checks_retry_and_cancellation_stops_the_worker() {
    let server = MockServer::start().await;
    let app = test_app(&server);
    setup(app.handle(), false);
    let worker = tokio::spawn(run(app.handle().clone()));
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    // Observe actual local I/O, rather than waiting an arbitrary duration.
    while server.received_requests().await.unwrap().is_empty() {
        assert!(
            std::time::Instant::now() < deadline,
            "initial check must reach the local server"
        );
        tokio::task::yield_now().await;
    }
    tokio::time::pause();
    while server.received_requests().await.unwrap().len() < 2 {
        assert!(
            std::time::Instant::now() < deadline,
            "failed checks must be retried"
        );
        tokio::time::advance(CHECK_INTERVAL).await;
        tokio::task::yield_now().await;
    }
    app.state::<PendingUpdate>().shutdown.cancel();
    worker.await.unwrap();
    let count = server.received_requests().await.unwrap().len();
    tokio::time::advance(CHECK_INTERVAL).await;
    assert_eq!(server.received_requests().await.unwrap().len(), count);
    // Cancelling before the first request is also a normal shutdown path.
    run(app.handle().clone()).await;
}

#[tokio::test]
async fn enabled_setup_starts_checks_without_creating_a_window() {
    let server = MockServer::start().await;
    let app = test_app(&server);
    release(&server, "99.0.0", PAYLOAD).await;
    setup(app.handle(), true);
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while app
        .state::<PendingUpdate>()
        .download
        .lock()
        .unwrap()
        .is_none()
    {
        assert!(
            std::time::Instant::now() < deadline,
            "enabled checks must reach the local server"
        );
        tokio::task::yield_now().await;
    }
    assert!(app.webview_windows().is_empty());
    app.state::<PendingUpdate>().shutdown.cancel();
    assert!(app.state::<PendingUpdate>().shutdown.is_cancelled());
    assert_eq!(
        app.state::<PendingUpdate>()
            .download
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .1,
        PAYLOAD,
        "worker cancellation does not consume the staged update"
    );
}

// Called from the isolated Wry fixture, where actual Quit events can be pumped
// and intercepted. Installation targets only this test's temporary fake app.
#[cfg(target_os = "linux")]
#[allow(deprecated)] // Bounded isolated fixture event-loop pumping, never production.
pub(crate) fn native_quit(app: &mut tauri::App<tauri::Wry>, server: &MockServer) {
    use std::time::Instant;
    let directory = tempfile::tempdir().unwrap();
    let executable = directory.path().join("fixture.AppImage");
    std::fs::write(&executable, b"old synthetic app").unwrap();
    // Private endpoint overrides exist only on the synthetic test AppHandle.
    // No production endpoint or installed application is mutated.
    let updater_app = test_app(server);
    on_event(app.handle(), &RunEvent::Ready);
    assert!(!app.state::<PendingUpdate>().shutdown.is_cancelled());
    tauri::async_runtime::block_on(release(server, "99.0.0", PAYLOAD));
    setup(app.handle(), true);
    let deadline = Instant::now() + Duration::from_secs(5);
    while app
        .state::<PendingUpdate>()
        .download
        .lock()
        .unwrap()
        .is_none()
    {
        assert!(
            Instant::now() < deadline,
            "real Wry worker must stage a verified download"
        );
        std::thread::yield_now();
    }
    // The live producer's Update points at the fixture process. Discard it;
    // installation tests below use an explicitly private executable path.
    app.state::<PendingUpdate>().download.lock().unwrap().take();
    app.state::<PendingUpdate>()
        .installing
        .store(true, Ordering::SeqCst);
    app.handle().exit(0);
    let duplicate_seen = std::sync::Arc::new(AtomicBool::new(false));
    let deadline = Instant::now() + Duration::from_secs(5);
    while !duplicate_seen.load(Ordering::SeqCst) {
        assert!(
            Instant::now() < deadline,
            "duplicate Quit must be delivered"
        );
        let seen = duplicate_seen.clone();
        app.run_iteration(move |_, event| {
            if let RunEvent::ExitRequested {
                code: Some(0), api, ..
            } = &event
            {
                api.prevent_exit();
                seen.store(true, Ordering::SeqCst);
            }
        });
    }
    assert!(app
        .state::<PendingUpdate>()
        .installing
        .load(Ordering::SeqCst));
    app.state::<PendingUpdate>()
        .installing
        .store(false, Ordering::SeqCst);
    tauri::async_runtime::block_on(release(server, "99.0.0", PAYLOAD));
    let updater = updater_app
        .updater_builder()
        .executable_path(&executable)
        .build()
        .unwrap();
    tauri::async_runtime::block_on(stage_update(updater, &app.state::<PendingUpdate>())).unwrap();
    app.handle().exit(0);
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    let quit_events = Arc::new(AtomicUsize::new(0));
    let deadline = Instant::now() + Duration::from_secs(5);
    while quit_events.load(Ordering::SeqCst) < 2 {
        assert!(
            Instant::now() < deadline,
            "installer must complete the deferred Quit"
        );
        let quit_events = quit_events.clone();
        app.run_iteration(move |_, event| {
            if let RunEvent::ExitRequested {
                code: Some(0), api, ..
            } = &event
            {
                api.prevent_exit(); // Keep the isolated fixture's loop alive.
                quit_events.fetch_add(1, Ordering::SeqCst);
            }
        });
    }
    assert_eq!(std::fs::read(&executable).unwrap(), PAYLOAD);
    assert!(app
        .state::<PendingUpdate>()
        .download
        .lock()
        .unwrap()
        .is_none());
    let updater = updater_app
        .updater_builder()
        .executable_path(directory.path().join("missing.AppImage"))
        .build()
        .unwrap();
    tauri::async_runtime::block_on(stage_update(updater, &app.state::<PendingUpdate>())).unwrap();
    app.handle().exit(0);
    while quit_events.load(Ordering::SeqCst) < 4 {
        assert!(
            Instant::now() < deadline,
            "failed installer must also complete Quit"
        );
        let quit_events = quit_events.clone();
        app.run_iteration(move |_, event| {
            if let RunEvent::ExitRequested {
                code: Some(0), api, ..
            } = &event
            {
                api.prevent_exit();
                quit_events.fetch_add(1, Ordering::SeqCst);
            }
        });
    }
    assert_eq!(std::fs::read(&executable).unwrap(), PAYLOAD);
    // An OS/unexpected exit must not run an installer.
    on_event(app.handle(), &RunEvent::Exit);
    assert!(app.state::<PendingUpdate>().shutdown.is_cancelled());
}
