use super::*;
pub(super) fn native_test(app: &tauri::AppHandle) {
    use dbus::channel::{MatchingReceiver, Sender};
    use std::sync::{
        atomic::{AtomicBool, AtomicU8, Ordering},
        Arc,
    };
    let server = Connection::new_session().unwrap();
    server
        .request_name("org.a11y.Bus", false, true, false)
        .unwrap();
    server
        .request_name("org.a11y.atspi.Registry", false, true, false)
        .unwrap();
    let address = std::env::var("DBUS_SESSION_BUS_ADDRESS").unwrap();
    let mode = Arc::new(AtomicU8::new(0));
    let fixture = mode.clone();
    server.start_receive(
        dbus::message::MatchRule::new_method_call(),
        Box::new(move |message, connection| {
            let mode = fixture.load(Ordering::SeqCst);
            let path = message.path().unwrap().to_string();
            let member = message.member().unwrap().to_string();
            let object = |path: &str| {
                (
                    "org.a11y.atspi.Registry".to_string(),
                    dbus::Path::from(path.to_string()),
                )
            };
            let response = match member.as_str() {
                "GetAddress" => message.method_return().append1(if mode == 7 {
                    "invalid address".to_string()
                } else {
                    address.clone()
                }),
                "GetState" => {
                    let bits: u64 = match path.as_str() {
                        "/window" if mode != 2 && mode != 12 => 1 << 1,
                        "/editor" if mode != 1 && mode != 4 && mode != 10 => {
                            (1 << 7)
                                | (1 << 8)
                                | (1 << 12)
                                | if mode == 11 { 1 << 43 } else { 0 }
                        }
                        _ => 0,
                    };
                    message.method_return().append1(if mode == 3 {
                        vec![]
                    } else {
                        vec![bits as u32, (bits >> 32) as u32]
                    })
                }
                "GetChildren" if mode == 6 => {
                    message.method_return().append1("invalid children")
                }
                "GetChildren" => message.method_return().append1(match path.as_str() {
                    "/org/a11y/atspi/accessible/root" => {
                        if mode == 5 {
                            (0..300).map(|n| object(&format!("/node{n}"))).collect()
                        } else {
                            vec![object("/window")]
                        }
                    }
                    "/window" => vec![object("/editor")],
                    "/editor" if mode == 4 => vec![object("/window")],
                    _ => vec![],
                }),
                _ => panic!("unexpected fixture call"),
            };
            connection.send(response).unwrap();
            true
        }),
    );
    let stop = Arc::new(AtomicBool::new(false));
    let stopped = stop.clone();
    let worker = std::thread::spawn(move || {
        while !stopped.load(Ordering::SeqCst) {
            server.process(Duration::from_millis(10)).unwrap();
        }
    });
    let target = Arc::new(Target::capture().expect("synthetic focused text control"));
    assert!(target.is_current());
    assert!(!Target { address: "invalid address".into(), object: target.object.clone(), window: target.window.clone() }.is_current());
    for state in [1, 2, 3, 4, 5, 6, 7, 10, 11, 12] {
        mode.store(state, Ordering::SeqCst);
        assert_eq!(
            target.is_current(),
            matches!(state, 5 | 6 | 7),
            "mode {state}"
        );
        assert!(Target::capture().is_none(), "mode {state}");
    }
    mode.store(0, Ordering::SeqCst);
    tauri::async_runtime::block_on(async {
        crate::pipeline::native_live_output(app, &|focused| {
            mode.store(if focused { 0 } else { 1 }, Ordering::SeqCst)
        })
        .await;
        let captured = crate::text::inject::capture_live_output_target(app)
            .await
            .unwrap();
        assert!(captured.is_current());
        let pipeline =
            crate::pipeline::SharedPipeline::new(crate::pipeline::PipelineConfig::default());
        let lease = crate::text::inject::LiveOutputLease {
            epoch: pipeline.session_epoch().unwrap(),
            pipeline: pipeline.clone(),
            target: captured,
        };
        mode.store(1, Ordering::SeqCst);
        assert!(crate::text::inject::output_live_text_with_app_async(
            app.clone(),
            "must not paste".into(),
            crate::text::inject::OutputMode::Clipboard,
            false,
            crate::text::key_inject::PasteShortcut::System,
            &lease
        )
        .await
        .is_err());
        mode.store(0, Ordering::SeqCst);
        let output_session = crate::text::inject::OutputSession {
            pipeline: pipeline.clone(),
            epoch: pipeline.session_epoch(),
        };
        assert!(output_session.ensure_current().is_ok());
        let request = |owner| crate::windows_uia::insert::WindowsInsertRequest {
            text: "synthetic fixture",
            initial_snapshot: None,
            allow_paste: true,
            allow_typing: true,
            smart_paste_protection: true,
            paste_shortcut: crate::text::key_inject::PasteShortcut::System,
            owner,
        };
        assert_eq!(
            crate::windows_uia::insert::insert_text_with_snapshot(app, request(None)).unwrap(),
            crate::windows_uia::types::WindowsInsertMethod::None
        );
        assert_eq!(
            crate::windows_uia::insert::insert_text_with_snapshot(
                app,
                request(Some(&output_session))
            )
            .unwrap(),
            crate::windows_uia::types::WindowsInsertMethod::None
        );
        pipeline.force_reset();
        assert!(output_session.ensure_current().is_err());
        assert!(crate::windows_uia::insert::insert_text_with_snapshot(
            app,
            request(Some(&output_session))
        )
        .is_err());
        let intent = crate::core::output_settings::resolve_output_intent(
            crate::text::inject::OutputMode::Clipboard,
            false,
            None,
            None,
            true,
            false,
        );
        assert!(crate::text::inject::output_text_for_session_async(
            app.clone(),
            "must not paste".into(),
            intent,
            &output_session
        )
        .await
        .is_err());
        assert!(crate::text::inject::output_live_text_with_app_async(
            app.clone(),
            "must not paste".into(),
            crate::text::inject::OutputMode::Clipboard,
            false,
            crate::text::key_inject::PasteShortcut::System,
            &lease
        )
        .await
        .is_err());
    });
    stop.store(true, Ordering::SeqCst);
    worker.join().unwrap();
}
