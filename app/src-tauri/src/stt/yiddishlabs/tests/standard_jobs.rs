use super::*;
use crate::stt::UploadPurpose;
use std::fs::OpenOptions;
use wiremock::{
    matchers::{method, path},
    Mock, MockServer, ResponseTemplate,
};

fn provider(server: &MockServer, managed: bool, token: Option<String>) -> YiddishLabsSttProvider {
    let client = Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let mut provider = YiddishLabsSttProvider::with_client(
        client,
        token.unwrap_or_else(|| "synthetic-standard-key".into()),
        Some(STANDARD_MODEL.into()),
        None,
        Some("synthetic context".into()),
        managed.then(|| server.uri()),
    );
    provider.standard_endpoint = format!("{}/api/v1/transcriptions", server.uri());
    provider
}
fn managed_token(subject: &str, issued: u64) -> String {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
    format!(
        "header.{}.signature",
        URL_SAFE_NO_PAD.encode(
            serde_json::to_vec(
                &serde_json::json!({"sub":subject,"iss":"synthetic-issuer","iat":issued})
            )
            .unwrap()
        )
    )
}
fn context(directory: &tempfile::TempDir, purpose: UploadPurpose) -> SttUploadContext {
    SttUploadContext {
        purpose,
        chunk_offset: 0,
        checkpoint_path: directory.path().join("chunk.yiddish-job"),
    }
}

#[test]
fn receipts_reject_invalid_job_ids_oversized_files_and_foreign_checkpoint_names() {
    let directory = tempfile::tempdir().unwrap();
    let base = directory.path().join("recording.transcripts");
    let path = checkpoint_path(&base, false, b"audio", 0).unwrap();
    let mut checkpoint = Checkpoint::open(&path, "synthetic-fingerprint".into()).unwrap();
    checkpoint.receipt.job_id = Some("../../private".into());
    checkpoint.save().unwrap();
    drop(checkpoint);
    assert!(Checkpoint::open(&path, "synthetic-fingerprint".into()).is_err());
    let file = OpenOptions::new().write(true).open(&path).unwrap();
    file.set_len(65537).unwrap();
    drop(file);
    assert!(Checkpoint::open(&path, "synthetic-fingerprint".into()).is_err());
    std::fs::remove_file(&path).unwrap();
    for name in [
        "byok.bad.job".to_owned(),
        format!("managed.{}.job", "z".repeat(64)),
    ] {
        let foreign = path.parent().unwrap().join(name);
        std::fs::write(&foreign, b"retain").unwrap();
        assert!(delete_owned_checkpoints(&base).is_err());
        assert_eq!(std::fs::read(&foreign).unwrap(), b"retain");
        std::fs::remove_file(foreign).unwrap();
    }
    let parent_file = directory.path().join("not-directory");
    std::fs::write(&parent_file, b"retain").unwrap();
    let invalid = parent_file.join("child.transcripts");
    assert!(checkpoint_path(&invalid, false, b"audio", 0).is_err());
    assert!(delete_owned_checkpoints(&invalid).is_err());
}

#[cfg(unix)]
#[test]
fn receipt_directory_sync_rejects_files_links_and_missing_directories_privately() {
    use std::os::unix::fs::symlink;
    let directory = tempfile::tempdir().unwrap();
    sync_directory(directory.path()).unwrap();
    let file = directory.path().join("private-file");
    std::fs::write(&file, b"retain").unwrap();
    let link = directory.path().join("private-link");
    symlink(directory.path(), &link).unwrap();
    for path in [file, link, directory.path().join("missing")] {
        let error = sync_directory(&path).unwrap_err().to_string();
        assert!(!error.contains(path.to_str().unwrap()));
        assert!(error.contains("No new job will be submitted"));
    }
}

#[tokio::test]
async fn managed_owner_identity_is_private_stable_and_includes_nested_organization() {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
    let server = MockServer::start().await;
    let token = |claims: Value| {
        format!(
            "header.{}.signature",
            URL_SAFE_NO_PAD.encode(claims.to_string())
        )
    };
    for claims in [
        serde_json::json!({}),
        serde_json::json!({"sub":"owner"}),
        serde_json::json!({"sub":"owner","iss":""}),
    ] {
        assert!(credential_owner(&provider(&server, true, Some(token(claims)))).is_err());
    }
    let invalid_json = format!(
        "header.{}.signature",
        URL_SAFE_NO_PAD.encode("private invalid json")
    );
    assert!(
        !credential_owner(&provider(&server, true, Some(invalid_json)))
            .unwrap_err()
            .to_string()
            .contains("private invalid")
    );
    let owner = |claims| credential_owner(&provider(&server, true, Some(token(claims)))).unwrap();
    let direct = owner(serde_json::json!({"sub":"owner","iss":"issuer","organization_id":"org"}));
    assert_eq!(
        direct,
        owner(serde_json::json!({"sub":"owner","iss":"issuer","app_metadata":{"org_id":"org"}}))
    );
    assert_eq!(
        direct,
        owner(
            serde_json::json!({"sub":"owner","iss":"issuer","app_metadata":{"organization_id":"org"},"iat":99})
        )
    );
    assert_ne!(
        direct,
        owner(serde_json::json!({"sub":"owner","iss":"issuer","app_metadata":{"org_id":"other"}}))
    );
    assert!(!direct.contains("owner"));
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn pending_jobs_poll_locally_without_resubmission_and_unknown_states_are_recoverable() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(201)
                .set_body_json(serde_json::json!({"id":"known-job","status":"queued"})),
        )
        .expect(1)
        .mount(&server)
        .await;
    let polls = AtomicUsize::new(0);
    Mock::given(method("GET"))
        .respond_with(move |_: &wiremock::Request| {
            ResponseTemplate::new(200).set_body_json(match polls.fetch_add(1, Ordering::SeqCst) {
                0 => serde_json::json!({"id":"known-job","status":"processing"}),
                1 => serde_json::json!({"id":"known-job","status":"pending"}),
                _ => serde_json::json!({"id":"known-job","status":"completed","text":"שלום"}),
            })
        })
        .expect(3)
        .mount(&server)
        .await;
    let directory = tempfile::tempdir().unwrap();
    let upload = context(&directory, UploadPurpose::Meeting);
    assert_eq!(
        transcribe_with_poll_interval(
            &provider(&server, false, None),
            b"audio",
            &upload,
            Duration::ZERO
        )
        .await
        .unwrap(),
        "שלום"
    );
    assert!(terminal_result(&serde_json::json!({"status":"unknown-private"})).is_err());
}

#[tokio::test]
async fn managed_auth_failures_preserve_the_upload_key_or_known_job_for_refresh() {
    for during_poll in [false, true] {
        let server = MockServer::start().await;
        let job = "00000000-0000-0000-0000-000000000123";
        Mock::given(method("POST"))
            .respond_with(if during_poll {
                ResponseTemplate::new(201)
                    .set_body_json(serde_json::json!({"id":job,"status":"queued"}))
            } else {
                ResponseTemplate::new(401)
            })
            .expect(1)
            .mount(&server)
            .await;
        if during_poll {
            Mock::given(method("GET"))
                .respond_with(ResponseTemplate::new(401))
                .expect(1)
                .mount(&server)
                .await;
        }
        let directory = tempfile::tempdir().unwrap();
        let upload = context(&directory, UploadPurpose::File);
        let error = provider(&server, true, Some(managed_token("owner", 1)))
            .transcribe_upload(b"audio", &AudioFormat::default(), &upload)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("401 auth_invalid_token"));
        let receipt = std::fs::read_to_string(&upload.checkpoint_path).unwrap();
        assert_eq!(receipt.contains(job), during_poll);
        assert!(receipt.contains("\"dispatched\":true"));
    }
}

#[tokio::test]
async fn network_loss_retains_dispatch_claims_and_known_job_ids_without_paid_resubmission() {
    for known in [false, true] {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}/jobs", listener.local_addr().unwrap());
        let transport = tokio::spawn(async move {
            let (socket, _) = listener.accept().await.unwrap();
            drop(socket); // A real transport failure, not a vendor HTTP error.
        });
        let mut adapter = YiddishLabsSttProvider::with_client(
            Client::new(),
            "synthetic-key".into(),
            Some(STANDARD_MODEL.into()),
            None,
            Some("synthetic context".into()),
            None,
        );
        adapter.standard_endpoint = endpoint.clone();
        let directory = tempfile::tempdir().unwrap();
        let upload = context(&directory, UploadPurpose::File);
        if known {
            // Installed receipt format: recovery must GET this job, not POST.
            let fingerprint = format!(
                "{:x}",
                Sha256::digest(
                    serde_json::to_vec(&(
                        endpoint,
                        credential_owner(&adapter).unwrap(),
                        "auto",
                        &adapter.prompt,
                        "file",
                        0_u64,
                        format!("{:x}", Sha256::digest(b"audio")),
                    ))
                    .unwrap()
                )
            );
            let mut checkpoint = Checkpoint::open(&upload.checkpoint_path, fingerprint).unwrap();
            checkpoint.receipt.dispatched = true;
            checkpoint.receipt.job_id = Some("known-job".into());
            checkpoint.save().unwrap();
        }
        let error = adapter
            .transcribe_upload(b"audio", &AudioFormat::default(), &upload)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("submission is uncertain"));
        let receipt = std::fs::read_to_string(&upload.checkpoint_path).unwrap();
        assert!(receipt.contains("\"dispatched\":true"));
        assert_eq!(receipt.contains("known-job"), known);
        transport.await.unwrap();
    }
}

#[tokio::test]
async fn managed_job_ids_are_opaque_kolboo_uuids_not_vendor_identifiers() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(201)
                .set_body_json(serde_json::json!({"id":"vendor-id","status":"queued"})),
        )
        .expect(1)
        .mount(&server)
        .await;
    let directory = tempfile::tempdir().unwrap();
    let upload = context(&directory, UploadPurpose::File);
    let error = provider(&server, true, Some(managed_token("owner", 1)))
        .transcribe_upload(b"audio", &AudioFormat::default(), &upload)
        .await
        .unwrap_err();
    assert!(!error.to_string().contains("vendor-id"));
    assert!(!std::fs::read_to_string(&upload.checkpoint_path)
        .unwrap()
        .contains("vendor-id"));
}

#[tokio::test]
async fn identical_audio_at_different_chunk_positions_has_independent_resumable_jobs() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(201)
                .set_body_json(serde_json::json!({"id":"known-job","status":"queued"})),
        )
        .expect(2)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v1/transcriptions/known-job"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            serde_json::json!({"id":"known-job","status":"completed","text":"שלום"}),
        ))
        .expect(4)
        .mount(&server)
        .await;
    let directory = tempfile::tempdir().unwrap();
    let base = directory.path().join("recording.transcripts");
    for _attempt in 0..2 {
        for chunk_offset in [0, 16000] {
            let upload = SttUploadContext {
                purpose: UploadPurpose::Meeting,
                chunk_offset,
                checkpoint_path: checkpoint_path(&base, false, b"same audio", chunk_offset)
                    .unwrap(),
            };
            let response = provider(&server, false, None)
                .transcribe_upload(b"same audio", &AudioFormat::default(), &upload)
                .await
                .unwrap();
            assert_eq!(response.text, "שלום");
        }
    }
    assert_eq!(
        server
            .received_requests()
            .await
            .unwrap()
            .iter()
            .filter(|r| r.method == "POST")
            .count(),
        2
    );
}

#[tokio::test]
async fn invalid_upload_identity_and_receipt_storage_never_dispatch_paid_work() {
    let server = MockServer::start().await;
    let directory = tempfile::tempdir().unwrap();
    let upload = context(&directory, UploadPurpose::File);
    for token in [
        String::new(),
        "invalid".into(),
        "header.not-base64.signature".into(),
        managed_token("", 1),
        "x".repeat(65537),
    ] {
        let error = provider(&server, true, Some(token))
            .transcribe_upload(b"audio", &AudioFormat::default(), &upload)
            .await
            .unwrap_err();
        assert!(matches!(error, SttError::Config(_)));
        assert!(!error.to_string().contains("not-base64"));
    }
    let mut adapter = provider(&server, false, None);
    assert!(adapter
        .transcribe_upload(b"", &AudioFormat::default(), &upload)
        .await
        .is_err());
    adapter.prompt = Some("ש".repeat(4001));
    assert!(adapter
        .transcribe_upload(b"audio", &AudioFormat::default(), &upload)
        .await
        .is_err());
    let mut missing_parent = upload.clone();
    missing_parent.checkpoint_path = directory.path().join("missing/receipt.job");
    assert!(provider(&server, false, None)
        .transcribe_upload(b"audio", &AudioFormat::default(), &missing_parent)
        .await
        .is_err());
    for bytes in [
        b"private invalid receipt\n".as_slice(),
        b"partial without newline",
        b"",
    ] {
        let path = &upload.checkpoint_path;
        let file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(path)
            .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            file.set_permissions(std::fs::Permissions::from_mode(0o600))
                .unwrap();
        }
        use std::io::Write;
        (&file).write_all(bytes).unwrap();
        if bytes.is_empty() {
            file.try_lock().unwrap();
        }
        let error = provider(&server, false, None)
            .transcribe_upload(b"audio", &AudioFormat::default(), &upload)
            .await
            .unwrap_err();
        assert!(matches!(error, SttError::Config(_)));
        assert!(!error.to_string().contains("private invalid"));
        drop(file);
    }
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn known_jobs_keep_their_receipt_after_failed_or_mismatched_results() {
    for response in [
        ResponseTemplate::new(503).set_body_string("private failure"),
        ResponseTemplate::new(200).set_body_json(
            serde_json::json!({"id":"other-job","status":"completed","text":"private text"}),
        ),
        ResponseTemplate::new(200)
            .set_body_json(serde_json::json!({"id":"known-job","status":"failed"})),
        ResponseTemplate::new(200)
            .set_body_json(serde_json::json!({"id":"known-job","status":"completed"})),
    ] {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(201)
                    .set_body_json(serde_json::json!({"id":"known-job","status":"queued"})),
            )
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .respond_with(response)
            .expect(2)
            .mount(&server)
            .await;
        let directory = tempfile::tempdir().unwrap();
        let upload = context(&directory, UploadPurpose::Meeting);
        for _ in 0..2 {
            let error = provider(&server, false, None)
                .transcribe_upload(b"audio", &AudioFormat::default(), &upload)
                .await
                .unwrap_err();
            assert!(!error.to_string().contains("private"));
        }
        assert_eq!(
            server
                .received_requests()
                .await
                .unwrap()
                .iter()
                .filter(|request| request.method == "POST")
                .count(),
            1
        );
        assert!(std::fs::read_to_string(upload.checkpoint_path)
            .unwrap()
            .contains("known-job"));
    }
}

#[tokio::test]
async fn owned_checkpoint_cleanup_is_narrow_and_refuses_in_flight_or_foreign_files() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            serde_json::json!({"id":"synthetic-job","status":"completed","text":"שלום"}),
        ))
        .expect(1)
        .mount(&server)
        .await;
    let directory = tempfile::tempdir().unwrap();
    let base = directory.path().join("recording.transcripts");
    assert!(delete_owned_checkpoints(&base).unwrap().is_empty());
    let path = checkpoint_path(&base, false, b"audio", 0).unwrap();
    assert_eq!(path, checkpoint_path(&base, false, b"audio", 0).unwrap());
    assert_ne!(path, checkpoint_path(&base, true, b"audio", 0).unwrap());
    assert_ne!(
        path,
        checkpoint_path(&base, false, b"audio", 16000).unwrap()
    );
    let context = SttUploadContext {
        purpose: UploadPurpose::Meeting,
        chunk_offset: 0,
        checkpoint_path: path.clone(),
    };
    provider(&server, false, None)
        .transcribe_upload(b"audio", &AudioFormat::default(), &context)
        .await
        .unwrap();
    let unrelated = directory.path().join("unrelated.txt");
    std::fs::write(&unrelated, b"retain me").unwrap();
    let locked = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&path)
        .unwrap();
    locked.try_lock().unwrap();
    assert!(delete_owned_checkpoints(&base).is_err());
    assert!(path.exists());
    drop(locked);
    let foreign = path.parent().unwrap().join("foreign.txt");
    std::fs::write(&foreign, b"not owned").unwrap();
    assert!(delete_owned_checkpoints(&base).is_err());
    assert!(path.exists());
    std::fs::remove_file(foreign).unwrap();
    let guards = delete_owned_checkpoints(&base).unwrap();
    assert_eq!(guards.len(), 1);
    assert!(!path.exists());
    assert!(!base.with_extension("yiddish-jobs").exists());
    assert_eq!(std::fs::read(unrelated).unwrap(), b"retain me");
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
}

#[cfg(unix)]
#[test]
fn checkpoint_directories_cannot_follow_links_or_reuse_world_readable_storage() {
    use std::os::unix::fs::{symlink, PermissionsExt};
    let directory = tempfile::tempdir().unwrap();
    let base = directory.path().join("recording.transcripts");
    let owned = base.with_extension("yiddish-jobs");
    let outside = tempfile::tempdir().unwrap();
    symlink(outside.path(), &owned).unwrap();
    assert!(checkpoint_path(&base, false, b"audio", 0).is_err());
    assert!(delete_owned_checkpoints(&base).is_err());
    std::fs::remove_file(&owned).unwrap();
    let path = checkpoint_path(&base, false, b"audio", 0).unwrap();
    std::fs::set_permissions(
        path.parent().unwrap(),
        std::fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    assert!(checkpoint_path(&base, false, b"audio", 0).is_err());
}

#[tokio::test]
async fn standard_resumes_known_jobs_and_keeps_receipts_free_of_audio_text_and_credentials() {
    for managed in [false, true] {
        let server = MockServer::start().await;
        let endpoint = if managed {
            "/v1/managed/transcription/jobs"
        } else {
            "/api/v1/transcriptions"
        };
        let id = if managed {
            "00000000-0000-0000-0000-000000000123"
        } else {
            "synthetic-vendor-123"
        };
        Mock::given(method("POST"))
            .and(path(endpoint))
            .respond_with(
                ResponseTemplate::new(201)
                    .set_body_json(serde_json::json!({"id":id,"status":"queued"})),
            )
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path(format!("{endpoint}/{id}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({"id":id,"status":"completed","text":"שלום — hello"}),
            ))
            .expect(2)
            .mount(&server)
            .await;
        let directory = tempfile::tempdir().unwrap();
        let context = context(
            &directory,
            if managed {
                UploadPurpose::File
            } else {
                UploadPurpose::Meeting
            },
        );
        for issued in [1, 2] {
            let provider = provider(
                &server,
                managed,
                managed.then(|| managed_token("same-owner", issued)),
            );
            assert_eq!(
                provider
                    .transcribe_upload(b"synthetic audio", &AudioFormat::default(), &context)
                    .await
                    .unwrap()
                    .text,
                "שלום — hello"
            );
        }
        let requests = server.received_requests().await.unwrap();
        let submission = &requests[0];
        let body = String::from_utf8_lossy(&submission.body);
        assert!(body.contains("name=\"language\"\r\n\r\nauto"));
        assert!(body.contains("name=\"context\"\r\n\r\nsynthetic context"));
        if managed {
            assert!(submission.headers.contains_key("authorization"));
            assert!(submission.headers.contains_key("idempotency-key"));
            assert!(body.contains(STANDARD_MODEL));
            assert!(body.contains("name=\"purpose\"\r\n\r\nfile"));
            assert!(!submission.headers.contains_key("x-api-key"));
        } else {
            assert_eq!(submission.headers["x-api-key"], "synthetic-standard-key");
            assert!(!submission.headers.contains_key("authorization"));
            assert!(!body.contains("name=\"model\""));
        }
        let receipt = std::fs::read_to_string(&context.checkpoint_path).unwrap();
        for private in [
            "synthetic audio",
            "שלום",
            "synthetic-standard-key",
            "same-owner",
            "synthetic context",
        ] {
            assert!(!receipt.contains(private));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&context.checkpoint_path)
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
    }
}

#[tokio::test]
async fn uncertain_byok_submission_is_never_automatically_sent_again() {
    for response in [
        ResponseTemplate::new(503).set_body_string("private vendor failure"),
        ResponseTemplate::new(200).set_body_string("private invalid response"),
    ] {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(response)
            .expect(1)
            .mount(&server)
            .await;
        let directory = tempfile::tempdir().unwrap();
        let context = context(&directory, UploadPurpose::File);
        for _ in 0..2 {
            let error = provider(&server, false, None)
                .transcribe_upload(b"audio", &AudioFormat::default(), &context)
                .await
                .unwrap_err();
            assert!(!crate::stt::is_retryable_error(&error));
            assert!(!error.to_string().contains("private"));
        }
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }
}

#[tokio::test]
async fn managed_recovery_reuses_the_same_upload_nonce_after_an_uncertain_response() {
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    let server = MockServer::start().await;
    let calls = Arc::new(AtomicUsize::new(0));
    let responder_calls = calls.clone();
    Mock::given(method("POST")).respond_with(move |_: &wiremock::Request| {
        if responder_calls.fetch_add(1, Ordering::SeqCst) == 0 { ResponseTemplate::new(503) }
        else { ResponseTemplate::new(202).set_body_json(serde_json::json!({"id":"00000000-0000-0000-0000-000000000123","status":"completed","text":"שלום"})) }
    }).expect(2).mount(&server).await;
    let directory = tempfile::tempdir().unwrap();
    let context = context(&directory, UploadPurpose::Meeting);
    assert!(
        provider(&server, true, Some(managed_token("same-owner", 1)))
            .transcribe_upload(b"audio", &AudioFormat::default(), &context)
            .await
            .is_err()
    );
    assert_eq!(
        provider(&server, true, Some(managed_token("same-owner", 2)))
            .transcribe_upload(b"audio", &AudioFormat::default(), &context)
            .await
            .unwrap()
            .text,
        "שלום"
    );
    let requests = server.received_requests().await.unwrap();
    assert_eq!(
        requests[0].headers["idempotency-key"],
        requests[1].headers["idempotency-key"]
    );
    assert_ne!(
        requests[0].headers["authorization"],
        requests[1].headers["authorization"]
    );
}

#[tokio::test]
async fn cancellation_after_dispatch_preserves_the_claim_and_never_claims_remote_cancellation() {
    let server = MockServer::start().await;
    let submitted = std::sync::Arc::new(tokio::sync::Notify::new());
    let signal = submitted.clone();
    Mock::given(method("POST"))
        .respond_with(move |_: &wiremock::Request| {
            signal.notify_one();
            ResponseTemplate::new(201).set_delay(Duration::from_secs(60))
        })
        .expect(1)
        .mount(&server)
        .await;
    let directory = tempfile::tempdir().unwrap();
    let context = context(&directory, UploadPurpose::Meeting);
    let saved = context.clone();
    let first = provider(&server, false, None);
    let task = tokio::spawn(async move {
        first
            .transcribe_upload(b"audio", &AudioFormat::default(), &saved)
            .await
    });
    submitted.notified().await;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    let error = provider(&server, false, None)
        .transcribe_upload(b"audio", &AudioFormat::default(), &context)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("may have started"));
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
}

#[tokio::test]
async fn changed_key_owner_audio_language_or_context_never_reuses_a_paid_receipt() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            serde_json::json!({"id":"synthetic-job","status":"completed","text":"שלום"}),
        ))
        .expect(1)
        .mount(&server)
        .await;
    let directory = tempfile::tempdir().unwrap();
    let context = context(&directory, UploadPurpose::Meeting);
    assert!(provider(&server, false, None)
        .transcribe_upload(b"audio", &AudioFormat::default(), &context)
        .await
        .is_ok());
    for variant in 0..4 {
        let mut changed = provider(&server, false, None);
        match variant {
            0 => changed.api_key = "different-key".into(),
            1 => changed.language = Some("he".into()),
            2 => changed.prompt = Some("different context".into()),
            _ => {}
        }
        let error = changed
            .transcribe_upload(
                if variant == 3 {
                    b"different audio"
                } else {
                    b"audio"
                },
                &AudioFormat::default(),
                &context,
            )
            .await
            .unwrap_err();
        assert!(matches!(error, SttError::Config(_)));
    }
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
}
