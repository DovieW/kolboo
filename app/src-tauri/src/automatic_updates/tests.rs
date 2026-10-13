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
    start(app.handle(), true, InstallTarget::Tauri);
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

#[tokio::test]
async fn status_and_checks_share_exclusive_ownership_and_fail_closed() {
    let server = MockServer::start().await;
    let app = test_app(&server);
    setup(app.handle(), true); // Raw test executables must not self-replace.
    assert!(!status(app.handle()).enabled);
    assert_eq!(request_check(app.handle()).unwrap_err(), DISABLED);
    app.state::<PendingUpdate>().status.lock().unwrap().enabled = true;
    release(&server, "99.0.0", PAYLOAD).await;
    stage(app.handle()).await.unwrap();
    assert!(
        !status(app.handle()).can_install,
        "no pipeline means no installation"
    );
    app.manage(crate::pipeline::SharedPipeline::default());
    assert!(status(app.handle()).can_install);
    let lease = InstallLease::acquire(
        app.state::<crate::pipeline::SharedPipeline>()
            .inner()
            .clone(),
    )
    .unwrap();
    assert!(!status(app.handle()).can_install);
    assert!(InstallLease::acquire(
        app.state::<crate::pipeline::SharedPipeline>()
            .inner()
            .clone()
    )
    .is_err());
    drop(lease);
    let permit = OperationPermit::acquire(&app.state::<PendingUpdate>()).unwrap();
    assert!(!status(app.handle()).can_install);
    assert_eq!(
        request_check(app.handle()).unwrap().phase,
        UpdatePhase::Ready
    );
    check_once(app.handle()).await;
    assert_eq!(server.received_requests().await.unwrap().len(), 2);
    drop(permit);
    server.reset().await;
    let result = request_check(app.handle()).unwrap();
    assert_eq!(result.phase, UpdatePhase::Checking);
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while app
        .state::<PendingUpdate>()
        .operation
        .load(Ordering::SeqCst)
    {
        assert!(std::time::Instant::now() < deadline);
        tokio::task::yield_now().await;
    }
    let result = status(app.handle());
    assert_eq!(result.error.as_deref(), Some(CHECK_FAILED));
    assert!(
        result.can_install,
        "a failed check preserves a previously verified update"
    );
    server.reset().await;
    app.state::<PendingUpdate>().shutdown.cancel();
    request_check(app.handle()).unwrap();
    while app
        .state::<PendingUpdate>()
        .operation
        .load(Ordering::SeqCst)
    {
        assert!(std::time::Instant::now() < deadline);
        tokio::task::yield_now().await;
    }
    assert_eq!(status(app.handle()).phase, UpdatePhase::Ready);
    assert!(status(app.handle()).can_install);
    assert!(
        server.received_requests().await.unwrap().is_empty(),
        "shutdown cancels a manual check without discarding verified bytes"
    );
}

#[cfg(target_os = "linux")]
fn native_invoke(app: &tauri::AppHandle, command: &str) -> tauri::ipc::InvokeResponse {
    let window = app.get_webview_window("updater-command-test").unwrap();
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    let webview: &tauri::Webview = window.as_ref();
    webview.clone().on_message(
        tauri::webview::InvokeRequest {
            cmd: command.into(),
            callback: tauri::ipc::CallbackFn(0),
            error: tauri::ipc::CallbackFn(1),
            url: "tauri://localhost".parse().unwrap(),
            body: tauri::ipc::InvokeBody::Json(serde_json::json!({})),
            headers: Default::default(),
            invoke_key: app.invoke_key().into(),
        },
        Box::new(move |_, _, response, _, _| {
            tx.send(response).unwrap();
        }),
    );
    rx.recv_timeout(Duration::from_secs(5)).unwrap()
}

#[cfg(target_os = "linux")]
fn assert_ipc_error(app: &tauri::AppHandle, command: &str, expected: &str) {
    match native_invoke(app, command) {
        tauri::ipc::InvokeResponse::Err(error) => {
            assert!(error.0.to_string().contains(expected), "{error:?}")
        }
        _ => panic!("Unsafe updater IPC unexpectedly succeeded"),
    }
}

#[cfg(target_os = "linux")]
fn native_manual(
    app: &tauri::AppHandle,
    updater_app: &tauri::App<MockRuntime>,
    executable: &std::path::Path,
) {
    let pending = app.state::<PendingUpdate>();
    tauri::WebviewWindowBuilder::new(app, "updater-command-test", Default::default())
        .visible(false)
        .build()
        .unwrap();
    assert!(matches!(
        native_invoke(app, "get_update_status"),
        tauri::ipc::InvokeResponse::Ok(_)
    ));
    pending.status.lock().unwrap().enabled = false;
    assert_ipc_error(app, "check_for_updates", DISABLED);
    assert_ipc_error(app, "install_update", DISABLED);
    pending.status.lock().unwrap().enabled = true;
    assert_ipc_error(app, "install_update", "Recording status is unavailable");
    let pipeline = crate::pipeline::SharedPipeline::default();
    app.manage(pipeline.clone());
    assert_ipc_error(app, "install_update", "No verified update");
    let permit = OperationPermit::acquire(&pending).unwrap();
    assert_ipc_error(app, "install_update", "already in progress");
    drop(permit);
    pipeline.begin_recovery().unwrap();
    assert_ipc_error(app, "install_update", "Finish recording");
    pipeline.end_recovery();
    assert!(matches!(
        native_invoke(app, "check_for_updates"),
        tauri::ipc::InvokeResponse::Ok(_)
    ));
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while pending.operation.load(Ordering::SeqCst) {
        assert!(std::time::Instant::now() < deadline);
        std::thread::yield_now();
    }
    // Discard an update aimed at the test runner. Repeated failing native
    // attempts below use only signed synthetic bytes and private targets.
    pending.download.lock().unwrap().take();
    for target in [
        InstallTarget::Unsupported,
        InstallTarget::Deb,
        InstallTarget::MacBundle(executable.into()),
        InstallTarget::Tauri,
    ] {
        let updater = updater_app
            .updater_builder()
            .executable_path(executable.with_file_name("missing.AppImage"))
            .build()
            .unwrap();
        tauri::async_runtime::block_on(stage_update(updater, &pending)).unwrap();
        *pending.target.lock().unwrap() = target;
        assert!(matches!(
            native_invoke(app, "install_update"),
            tauri::ipc::InvokeResponse::Ok(_)
        ));
        while pending.operation.load(Ordering::SeqCst) {
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        let result = status(app);
        assert_eq!(result.phase, UpdatePhase::Ready);
        assert_eq!(result.error.as_deref(), Some(INSTALL_FAILED));
        assert!(result.can_install);
        assert!(!pipeline.is_recovering());
        assert_eq!(
            pending.download.lock().unwrap().as_ref().unwrap().1,
            PAYLOAD
        );
        pending.download.lock().unwrap().take();
    }
    *pending.target.lock().unwrap() = InstallTarget::Tauri;
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
    start(app.handle(), true, InstallTarget::Tauri);
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
    *app.state::<PendingUpdate>().installing.lock().unwrap() = Some(false);
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
    assert_eq!(
        *app.state::<PendingUpdate>().installing.lock().unwrap(),
        Some(true)
    );
    *app.state::<PendingUpdate>().installing.lock().unwrap() = None;
    native_manual(app.handle(), &updater_app, &executable);
    tauri::async_runtime::block_on(release(server, "99.0.0", PAYLOAD));
    let updater = updater_app
        .updater_builder()
        .executable_path(&executable)
        .build()
        .unwrap();
    tauri::async_runtime::block_on(stage_update(updater, &app.state::<PendingUpdate>())).unwrap();
    assert!(matches!(
        native_invoke(app.handle(), "install_update"),
        tauri::ipc::InvokeResponse::Ok(_)
    ));
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    let quit_events = Arc::new(AtomicUsize::new(0));
    let deadline = Instant::now() + Duration::from_secs(5);
    while quit_events.load(Ordering::SeqCst) < 1 {
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
    while quit_events.load(Ordering::SeqCst) < 3 {
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
