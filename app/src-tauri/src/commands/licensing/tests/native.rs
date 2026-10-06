//! Real command/session/store wiring, with loopback HTTP and synthetic keyring.
use super::*;
use std::os::unix::fs::PermissionsExt;
use tauri::Manager;

struct Environment(Vec<(&'static str, Option<String>)>);
impl Environment {
    fn new() -> Self {
        Self(
            [
                "TAURI_SUPABASE_URL",
                "TAURI_SUPABASE_PUBLISHABLE_KEY",
                "TAURI_API_BASE_URL",
                "TAURI_MANAGED_INFERENCE_GATEWAY_URL",
                "TAURI_CLOUDFLARE_ACCESS_CLIENT_ID",
                "TAURI_CLOUDFLARE_ACCESS_CLIENT_SECRET",
                "TAURI_PUBLIC_AUTH_PAGE_URL",
                "PATH",
            ]
            .into_iter()
            .map(|key| (key, std::env::var(key).ok()))
            .collect(),
        )
    }
    fn configure(&self, base: &str) {
        std::env::set_var("TAURI_SUPABASE_URL", base);
        std::env::set_var("TAURI_SUPABASE_PUBLISHABLE_KEY", "synthetic-public-key");
        std::env::set_var("TAURI_API_BASE_URL", base);
        std::env::set_var("TAURI_MANAGED_INFERENCE_GATEWAY_URL", base);
        std::env::set_var("TAURI_CLOUDFLARE_ACCESS_CLIENT_ID", "synthetic-access-id");
        std::env::set_var(
            "TAURI_CLOUDFLARE_ACCESS_CLIENT_SECRET",
            "synthetic-access-secret",
        );
    }
}
impl Drop for Environment {
    fn drop(&mut self) {
        for (key, value) in &self.0 {
            match value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
    }
}

async fn server(responses: Vec<(u16, String)>) -> (String, tokio::task::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        let mut captured = Vec::new();
        for (status, body) in responses {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            let mut buffer = [0; 4096];
            loop {
                let count = socket.read(&mut buffer).await.unwrap();
                assert!(count > 0);
                request.extend_from_slice(&buffer[..count]);
                if let Some(end) = request.windows(4).position(|value| value == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&request[..end]).to_lowercase();
                    let length = headers
                        .lines()
                        .find_map(|line| {
                            line.strip_prefix("content-length: ")
                                .and_then(|value| value.parse::<usize>().ok())
                        })
                        .unwrap_or(0);
                    if request.len() >= end + 4 + length {
                        break;
                    }
                }
            }
            captured.push(String::from_utf8(request).unwrap());
            socket.write_all(format!("HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()).await.unwrap();
        }
        captured
    });
    (format!("http://{address}"), task)
}

pub(crate) fn email_code_session_lifecycle(
    app: &AppHandle,
    fail_credential: impl Fn(&str),
    fail_write: impl Fn(&str),
) {
    assert_eq!(
        std::env::var("KOLBOO_NATIVE_WINDOW_TEST").as_deref(),
        Ok("1")
    );
    let environment = Environment::new();
    // Exercise generated IPC handlers as well as their Rust bodies. Malformed
    // frontend arguments must be rejected before any external request.
    let window = tauri::WebviewWindowBuilder::new(app, "auth-command-test", Default::default())
        .visible(false)
        .build()
        .unwrap();
    let invoke = |command: &str, body: serde_json::Value| {
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        let webview: &tauri::Webview = window.as_ref();
        webview.clone().on_message(
            tauri::webview::InvokeRequest {
                cmd: command.into(),
                callback: tauri::ipc::CallbackFn(0),
                error: tauri::ipc::CallbackFn(1),
                url: "tauri://localhost".parse().unwrap(),
                body: tauri::ipc::InvokeBody::Json(body),
                headers: Default::default(),
                invoke_key: app.invoke_key().into(),
            },
            Box::new(move |_, _, response, _, _| {
                tx.send(response).unwrap();
            }),
        );
        rx.recv_timeout(Duration::from_secs(5)).unwrap()
    };
    for (command, body) in [
        ("license_request_email_code", json!({"email":"invalid"})),
        (
            "license_verify_email_code",
            json!({"email":"person@example.test","code":"bad"}),
        ),
    ] {
        match invoke(command, body) {
            tauri::ipc::InvokeResponse::Err(error) => {
                assert!(error.0.to_string().contains("auth_"))
            }
            _ => panic!("Malformed auth IPC request succeeded"),
        }
    }
    assert!(matches!(
        invoke("license_cancel_login", json!({})),
        tauri::ipc::InvokeResponse::Ok(_)
    ));
    tauri::async_runtime::block_on(async {
        std::env::remove_var("TAURI_SUPABASE_URL");
        std::env::remove_var("TAURI_SUPABASE_PUBLISHABLE_KEY");
        assert_eq!(
            supabase_auth_config().unwrap_err().code.as_deref(),
            Some("auth_not_configured")
        );
        std::env::set_var("TAURI_SUPABASE_URL", "http://127.0.0.1");
        assert_eq!(
            supabase_auth_config().unwrap_err().code.as_deref(),
            Some("auth_not_configured")
        );
        std::env::set_var("TAURI_PUBLIC_AUTH_PAGE_URL", "https://auth.example.test");
        assert_eq!(
            public_auth_page_url().as_deref(),
            Some("https://auth.example.test")
        );
        // All calls remain on loopback; response bodies deliberately contain
        // private-looking content to verify safe user-facing error boundaries.
        for (status, body) in [(503, "private upstream content"), (200, "not-json")] {
            let (base, task) = server(vec![(status, body.into())]).await;
            environment.configure(&base);
            let error =
                fetch_license_state_from_api("synthetic-access", Method::GET, "/v1/license/state")
                    .await
                    .unwrap_err();
            assert!(!error.message.contains("private"));
            task.await.unwrap();
            let (base, task) = server(vec![(status, body.into())]).await;
            environment.configure(&base);
            let error = fetch_license_portal_url_from_api("synthetic-access")
                .await
                .unwrap_err();
            assert!(!error.message.contains("private"));
            task.await.unwrap();
            let (base, task) = server(vec![(status, body.into())]).await;
            environment.configure(&base);
            let error = exchange_supabase_auth_code("synthetic-code", "synthetic-verifier")
                .await
                .unwrap_err();
            assert!(!error.message.contains("private"));
            task.await.unwrap();
        }
        for status in [400, 401, 503, 200] {
            let (base, task) = server(vec![(status, "not-json private upstream".into())]).await;
            environment.configure(&base);
            let error = refresh_supabase_session(&SessionMaterial {
                access_token: "synthetic-access".into(),
                refresh_token: "synthetic-refresh".into(),
            })
            .await
            .unwrap_err();
            assert!(!error.message.contains("private"));
            assert_eq!(
                refresh_error_requires_reauthentication(&error.message),
                status == 400 || status == 401
            );
            task.await.unwrap();
        }
        for signup in [false, true] {
            let (base, task) = server(vec![(200, "not-json private upstream".into())]).await;
            environment.configure(&base);
            let error = if signup {
                sign_up_supabase_with_password("person@example.test", "synthetic-password")
                    .await
                    .unwrap_err()
            } else {
                sign_in_supabase_with_password("person@example.test", "synthetic-password")
                    .await
                    .unwrap_err()
            };
            assert_eq!(error.code.as_deref(), Some("auth_response_parse_failed"));
            task.await.unwrap();
        }
        let unavailable = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = unavailable.local_addr().unwrap();
        drop(unavailable);
        environment.configure(&format!("http://{address}"));
        assert_eq!(
            fetch_license_state_from_api("synthetic-access", Method::GET, "/v1/license/state")
                .await
                .unwrap_err()
                .code
                .as_deref(),
            Some("license_state_unavailable")
        );
        assert!(fetch_license_portal_url_from_api("synthetic-access")
            .await
            .is_err());
        assert!(
            exchange_supabase_auth_code("synthetic-code", "synthetic-verifier")
                .await
                .is_err()
        );
        assert!(refresh_supabase_session(&SessionMaterial {
            access_token: "synthetic-access".into(),
            refresh_token: "synthetic-refresh".into()
        })
        .await
        .is_err());
        assert!(email_code::license_request_email_code("invalid".into())
            .await
            .is_err());
        assert!(email_code::license_verify_email_code(
            app.clone(),
            "invalid".into(),
            "123456".into()
        )
        .await
        .is_err());
        for code in ["12345", "abcdef"] {
            assert_eq!(
                email_code::license_verify_email_code(
                    app.clone(),
                    "person@example.test".into(),
                    code.into()
                )
                .await
                .unwrap_err()
                .code
                .as_deref(),
                Some("auth_code_invalid")
            );
        }
        let (base, task) = server(vec![(200, "{}".into())]).await;
        environment.configure(&base);
        email_code::license_request_email_code("Person@Example.test".into())
            .await
            .unwrap();
        let requests = task.await.unwrap();
        assert!(requests[0].starts_with("POST /auth/v1/otp "));
        assert!(requests[0].contains("person@example.test"));

        let (base, task) = server(vec![(200, "{}".into())]).await;
        environment.configure(&base);
        assert_eq!(
            email_code::license_verify_email_code(
                app.clone(),
                "person@example.test".into(),
                "123456".into()
            )
            .await
            .unwrap_err()
            .code
            .as_deref(),
            Some("auth_response_parse_failed")
        );
        task.await.unwrap();
        assert!(load_session_material(app).is_none());

        let auth = json!({"access_token":"synthetic-access", "refresh_token":"synthetic-refresh", "user":{"id":"synthetic-user","email":"person@example.test"}}).to_string();
        let (base, task) =
            server(vec![(200, auth), (503, "private upstream content".into())]).await;
        environment.configure(&base);
        let state = email_code::license_verify_email_code(
            app.clone(),
            "person@example.test".into(),
            "123456".into(),
        )
        .await
        .unwrap();
        assert_eq!(state.tier, LicenseTier::Community);
        assert_eq!(state.status, LicenseStatus::Active);
        assert_eq!(
            load_session_material(app).unwrap().access_token,
            "synthetic-access"
        );
        assert!(app
            .store("settings.json")
            .unwrap()
            .get(crate::secrets::AUTH_SESSION_ACCESS_TOKEN_KEY)
            .is_none());
        let requests = task.await.unwrap();
        assert!(requests[0].starts_with("POST /auth/v1/verify "));
        assert!(requests[1].starts_with("GET /v1/license/state "));
        assert!(requests[1]
            .to_lowercase()
            .contains("authorization: bearer synthetic-access"));
        // Password signup/login and both browser handoffs use the same session
        // ownership path. Exercise them without launching a browser or email.
        let auth = json!({"access_token":"synthetic-access", "refresh_token":"synthetic-refresh", "user":{"id":"synthetic-user","email":"person@example.test"}}).to_string();
        let community = build_signed_in_fallback_state(
            "synthetic-user".into(),
            Some("person@example.test".into()),
            Utc::now(),
        );
        for browser in [false, true] {
            let (base, task) = server(vec![
                (200, auth.clone()),
                (200, serde_json::to_string(&community).unwrap()),
            ])
            .await;
            environment.configure(&base);
            let state = if browser {
                complete_browser_auth(
                    app,
                    BrowserAuthCallback::AuthorizationCode("synthetic-code".into()),
                    "synthetic-verifier",
                    SESSION_OWNER.begin(),
                )
                .await
                .unwrap()
            } else {
                license_start_login(
                    app.clone(),
                    Some(LoginRequest {
                        provider_hint: None,
                        auth_provider: None,
                        email: Some("person@example.test".into()),
                        password: Some("synthetic-password".into()),
                    }),
                )
                .await
                .unwrap()
            };
            assert_eq!(state.tier, LicenseTier::Community);
            task.await.unwrap();
        }
        let (base, task) = server(vec![
            (200, auth),
            (200, serde_json::to_string(&community).unwrap()),
        ])
        .await;
        environment.configure(&base);
        assert!(
            !license_sign_up(
                app.clone(),
                SignupRequest {
                    email: "person@example.test".into(),
                    password: "synthetic-password".into()
                }
            )
            .await
            .unwrap()
            .confirmation_required
        );
        task.await.unwrap();
        let (base, task) = server(vec![(200, serde_json::to_string(&community).unwrap())]).await;
        environment.configure(&base);
        let callback = PublicAuthSessionCallback {
            session: SessionMaterial {
                access_token: "synthetic-access".into(),
                refresh_token: "synthetic-refresh".into(),
            },
            user_id: "synthetic-user".into(),
            email: Some("person@example.test".into()),
        };
        assert_eq!(
            complete_browser_auth(
                app,
                BrowserAuthCallback::Session(callback),
                "unused",
                SESSION_OWNER.begin()
            )
            .await
            .unwrap()
            .tier,
            LicenseTier::Community
        );
        task.await.unwrap();

        // Capture the OS browser launch without opening a real browser. The
        // callback still traverses the complete production loopback flow.
        let capture = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = capture.local_addr().unwrap();
        let launcher = tempfile::tempdir().unwrap();
        let executable = launcher.path().join("xdg-open");
        std::fs::write(&executable, format!("#!/usr/bin/python3\nimport socket,sys\nwith socket.create_connection(('127.0.0.1', {})) as connection:\n connection.sendall(sys.argv[1].encode())\n", address.port())).unwrap();
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();
        let original_path = std::env::var_os("PATH").unwrap();
        let paths = std::iter::once(launcher.path().to_path_buf())
            .chain(std::env::split_paths(&original_path));
        std::env::set_var("PATH", std::env::join_paths(paths).unwrap());
        let callback_task = tokio::spawn(async move {
            let (mut socket, _) = capture.accept().await.unwrap();
            let mut bytes = Vec::new();
            socket.read_to_end(&mut bytes).await.unwrap();
            let url = Url::parse(std::str::from_utf8(&bytes).unwrap()).unwrap();
            assert_eq!(url.host_str(), Some("auth.example.test"));
            let parameters = url
                .query_pairs()
                .collect::<std::collections::HashMap<_, _>>();
            let callback = parameters.get("desktop_callback_url").unwrap();
            assert!(callback.starts_with("http://127.0.0.1:"));
            let response = reqwest::Client::new().post(callback.as_ref()).json(&json!({
                "state": parameters.get("desktop_state").unwrap(), "access_token":"synthetic-access",
                "refresh_token":"synthetic-refresh", "user_id":"synthetic-user", "email":"person@example.test"
            })).send().await.unwrap();
            assert!(response.status().is_success());
        });
        let (base, task) = server(vec![(200, serde_json::to_string(&community).unwrap())]).await;
        environment.configure(&base);
        assert_eq!(
            timeout(
                Duration::from_secs(10),
                license_start_login(app.clone(), None)
            )
            .await
            .unwrap()
            .unwrap()
            .tier,
            LicenseTier::Community
        );
        callback_task.await.unwrap();
        task.await.unwrap();
        std::env::set_var("PATH", original_path);

        let store = app.store("settings.json").unwrap();
        crate::settings::managed_defaults::initialize(&store);
        store.set(crate::settings::managed_defaults::PENDING, json!(true));
        let models = serde_json::from_str::<crate::managed_inference::ManagedModelCatalogResponse>(
            &catalog_for_test(),
        )
        .unwrap()
        .models;
        // Custom BYOK keys have the same ownership protection as built-ins.
        store.set("custom_providers", json!([{"id":"custom_fixture", "name":"Fixture", "base_url":"https://provider.example.test", "stt_models":["speech"], "llm_models":[]}]));
        crate::secrets::set_api_key(app, "custom_fixture_api_key", "synthetic-custom-key").unwrap();
        let previous = store.get("stt_model");
        try_apply_first_install_models(app, Some(&models)).unwrap();
        assert_eq!(store.get("stt_model"), previous);
        assert_eq!(
            store.get(crate::settings::managed_defaults::PENDING),
            Some(json!(true))
        );
        crate::secrets::clear_api_key(app, "custom_fixture_api_key").unwrap();
        store.delete("custom_providers");
        store.set("custom_providers", json!("invalid saved providers"));
        assert!(try_apply_first_install_models(app, Some(&models)).is_err());
        store.delete("custom_providers");
        fail_credential("groq_api_key");
        apply_first_install_models(app, Some(&models));
        assert_eq!(store.get("stt_model"), previous);
        assert_eq!(
            store.get(crate::settings::managed_defaults::PENDING),
            Some(json!(true))
        );
        // Simulate an actual settings-file write failure, not a mocked save.
        let path = app.path().app_data_dir().unwrap().join("settings.json");
        store.save().unwrap();
        let backup = path.with_extension("fixture-backup");
        std::fs::rename(&path, &backup).unwrap();
        std::fs::create_dir(&path).unwrap();
        assert!(try_apply_first_install_models(app, Some(&models)).is_err());
        assert_eq!(store.get("stt_model"), previous);
        assert_eq!(
            store.get(crate::settings::managed_defaults::PENDING),
            Some(json!(true))
        );
        std::fs::remove_dir(&path).unwrap();
        std::fs::rename(&backup, &path).unwrap();
        let mut pro = LicenseState::signed_out(Utc::now());
        pro.tier = LicenseTier::Personal;
        pro.status = LicenseStatus::Active;
        pro.user_id = Some("synthetic-user".into());
        pro.last_validated_at = Some(Utc::now());
        let auth = json!({"access_token":"synthetic-pro-access", "refresh_token":"synthetic-pro-refresh", "user":{"id":"synthetic-user","email":"person@example.test"}}).to_string();
        let catalog = json!({"request_id":"synthetic-request", "models":[
            {"id":"speech-default", "provider":"groq", "display_name":"Speech", "capabilities":["transcription"], "default_for_provider":true},
            {"id":"text-default", "provider":"openai", "display_name":"Text", "capabilities":["chat_completions"], "default_for_provider":true}
        ]}).to_string();
        let (base, task) = server(vec![
            (200, auth),
            (200, serde_json::to_string(&pro).unwrap()),
            (200, catalog),
        ])
        .await;
        environment.configure(&base);
        let state = email_code::license_verify_email_code(
            app.clone(),
            "person@example.test".into(),
            "123456".into(),
        )
        .await
        .unwrap();
        assert_eq!(state.tier, LicenseTier::Personal);
        assert_eq!(store.get("stt_model"), Some(json!("speech-default")));
        assert_eq!(store.get("llm_model"), Some(json!("text-default")));
        assert_eq!(
            store.get(crate::settings::managed_defaults::PENDING),
            Some(json!(false))
        );
        assert!(task.await.unwrap()[2].starts_with("GET /v1/managed/models "));
        let old_session = load_session_material(app).unwrap();
        // Wallet errors must reject the refresh safely without destroying the
        // previous token pair or granting unverified account access.
        let auth = json!({"access_token":"failed-access", "refresh_token":"failed-refresh", "user":{"id":"synthetic-user","email":"person@example.test"}}).to_string();
        let (base, task) = server(vec![(200, auth)]).await;
        environment.configure(&base);
        fail_write(crate::secrets::AUTH_SESSION_ACCESS_TOKEN_KEY);
        assert_eq!(
            license_refresh_entitlement(app.clone(), None)
                .await
                .unwrap_err()
                .code
                .as_deref(),
            Some("auth_session_save_failed")
        );
        task.await.unwrap();
        assert_eq!(
            load_session_material(app).unwrap().access_token,
            old_session.access_token
        );
        let auth = json!({"access_token":"failed-access", "refresh_token":"failed-refresh", "user":{"id":"synthetic-user","email":"person@example.test"}}).to_string();
        let (base, task) = server(vec![
            (200, auth),
            (200, serde_json::to_string(&pro).unwrap()),
        ])
        .await;
        environment.configure(&base);
        fail_write(crate::secrets::AUTH_SESSION_ACCESS_TOKEN_KEY);
        assert_eq!(
            email_code::license_verify_email_code(
                app.clone(),
                "person@example.test".into(),
                "123456".into()
            )
            .await
            .unwrap_err()
            .code
            .as_deref(),
            Some("auth_session_save_failed")
        );
        task.await.unwrap();
        assert_eq!(
            load_session_material(app).unwrap().access_token,
            old_session.access_token
        );
        assert!(
            license_get_auth_context(app.clone())
                .await
                .unwrap()
                .authenticated
        );
        assert!(
            crate::commands::sync::sync_get_status(app.clone())
                .await
                .unwrap()
                .endpoint_configured
        );
        assert!(first_install_catalog(app, &state, "synthetic-access")
            .await
            .is_none());
        let (base, task) = server(vec![(200, catalog_for_test())]).await;
        environment.configure(&base);
        assert_eq!(
            crate::managed_inference::managed_inference_get_models(app.clone())
                .await
                .unwrap()
                .models
                .len(),
            1
        );
        task.await.unwrap();
        // A rotated session plus verified revocation must downgrade immediately.
        let rotation = json!({"access_token":"rotated-access", "refresh_token":"rotated-refresh", "user":{"id":"synthetic-user","email":"person@example.test"}}).to_string();
        let community = build_signed_in_fallback_state(
            "synthetic-user".into(),
            Some("person@example.test".into()),
            Utc::now(),
        );
        let (base, task) = server(vec![
            (200, rotation),
            (200, serde_json::to_string(&community).unwrap()),
        ])
        .await;
        environment.configure(&base);
        assert_eq!(
            license_refresh_entitlement(app.clone(), None)
                .await
                .unwrap()
                .tier,
            LicenseTier::Community
        );
        assert_eq!(
            load_session_material(app).unwrap().access_token,
            "rotated-access"
        );
        assert!(!license_get_auth_context(app.clone())
            .await
            .unwrap()
            .entitlements
            .contains(&"managed_inference".into()));
        task.await.unwrap();
        save_license_state(app, &state, "synthetic-fixture").unwrap();
        assert_eq!(
            license_refresh_entitlement(app.clone(), Some(true))
                .await
                .unwrap()
                .tier,
            LicenseTier::Personal
        );

        let ticket = SESSION_OWNER.current();
        license_cancel_login();
        assert!(SESSION_OWNER.commit(ticket, || Ok(())).is_err());
        assert_eq!(
            license_logout(app.clone()).await.unwrap().status,
            LicenseStatus::SignedOut
        );
        std::env::remove_var("TAURI_MANAGED_INFERENCE_GATEWAY_URL");
        std::env::remove_var("TAURI_API_BASE_URL");
        assert_eq!(
            crate::managed_inference::managed_inference_get_models(app.clone())
                .await
                .unwrap_err(),
            "Managed inference gateway URL is not configured"
        );
        crate::settings::patch::apply_settings_patch(
            &store,
            Default::default(),
            vec!["llm_model".into()],
        )
        .unwrap();
        assert_eq!(
            store.get(crate::settings::managed_defaults::PENDING),
            Some(json!(false))
        );
        assert!(load_session_material(app).is_none());
        assert_eq!(
            license_refresh_entitlement(app.clone(), None)
                .await
                .unwrap()
                .status,
            LicenseStatus::SignedOut
        );
        // Exercise a genuinely invalid Store resource registry during teardown,
        // then repair this isolated fixture before other native checks run.
        let entries = store.entries();
        let resources = app.resources_table();
        let rid = resources
            .names()
            .find_map(|(rid, _)| {
                resources
                    .get::<tauri_plugin_store::Store<tauri::Wry>>(rid)
                    .ok()
                    .filter(|candidate| std::sync::Arc::ptr_eq(candidate, &store))
                    .map(|_| rid)
            })
            .unwrap();
        drop(resources);
        let removed = app.resources_table().take_any(rid).unwrap();
        assert!(try_apply_first_install_models(app, Some(&models)).is_err());
        let repaired = app
            .store_builder("settings.json")
            .create_new()
            .build()
            .unwrap();
        for (key, value) in entries {
            repaired.set(key, value);
        }
        repaired.save().unwrap();
        drop(removed);
    });
}

fn catalog_for_test() -> String {
    json!({"request_id":"synthetic-request", "models":[{"id":"speech-default", "provider":"groq", "display_name":"Speech", "capabilities":["transcription"], "default_for_provider":true}]}).to_string()
}
