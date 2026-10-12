use super::*;
use wiremock::{
    matchers::{method, path},
    Mock, MockServer, ResponseTemplate,
};

fn provider(server: &MockServer, managed: bool) -> YiddishLabsSttProvider {
    let client = Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let mut provider = YiddishLabsSttProvider::with_client(
        client,
        "synthetic-key".into(),
        None,
        None,
        None,
        managed.then(|| server.uri()),
    );
    provider.flash_endpoint = format!("{}/v1/audio/transcriptions", server.uri());
    provider
}

#[tokio::test]
async fn flash_uses_the_documented_model_auth_and_auto_language_without_mutating_rtl_text() {
    for managed in [false, true] {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/audio/transcriptions"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"text":"שלום — hello"})),
            )
            .expect(1)
            .mount(&server)
            .await;
        let mut provider = provider(&server, managed);
        provider.prompt = Some("synthetic context".into());
        assert!(!provider.supports_streaming());
        assert_eq!(provider.name(), "yiddishlabs");
        assert_eq!(
            provider
                .transcribe(b"synthetic audio", &AudioFormat::default())
                .await
                .unwrap(),
            "שלום — hello"
        );
        let request = server.received_requests().await.unwrap().pop().unwrap();
        assert_eq!(request.headers["authorization"], "Bearer synthetic-key");
        assert_eq!(
            request.headers.contains_key("x-kolboo-model-provider"),
            managed
        );
        assert_eq!(request.headers.contains_key("x-idempotency-key"), managed);
        let body = String::from_utf8_lossy(&request.body);
        for field in [
            "yiddishlabs-3-turbo",
            "name=\"language\"\r\n\r\nauto",
            "name=\"prompt\"\r\n\r\nsynthetic context",
        ] {
            assert!(body.contains(field), "missing contract field: {field}");
        }
        assert!(!body.contains("synthetic-key"));
    }
}

#[tokio::test]
async fn flash_fails_closed_before_upload_for_invalid_model_key_audio_or_hints() {
    let server = MockServer::start().await;
    for model in [STANDARD_MODEL, "unknown-model"] {
        let mut provider = provider(&server, false);
        provider.model = model.into();
        assert!(matches!(
            provider.transcribe(b"audio", &AudioFormat::default()).await,
            Err(SttError::Config(_))
        ));
    }
    let mut empty_key = provider(&server, false);
    empty_key.api_key.clear();
    assert!(empty_key
        .transcribe(b"audio", &AudioFormat::default())
        .await
        .is_err());
    assert!(provider(&server, false)
        .transcribe(b"", &AudioFormat::default())
        .await
        .is_err());
    let mut invalid_hint = provider(&server, false);
    invalid_hint.language = Some("lk".into());
    assert!(invalid_hint
        .transcribe(b"audio", &AudioFormat::default())
        .await
        .is_err());
    let mut long_prompt = provider(&server, false);
    long_prompt.prompt = Some("x".repeat(8001));
    assert!(long_prompt
        .transcribe(b"audio", &AudioFormat::default())
        .await
        .is_err());
    assert!(server.received_requests().await.unwrap().is_empty());
    for value in [
        None,
        Some(""),
        Some("  "),
        Some("auto"),
        Some("yi"),
        Some("en"),
        Some("he"),
    ] {
        assert!(language_hint(value, false).is_ok());
    }
    assert_eq!(language_hint(Some("lk"), true).unwrap(), "lk");
    assert!(language_hint(Some("xx"), true).is_err());
}

#[tokio::test]
async fn vendor_failures_and_malformed_results_never_expose_private_response_bodies() {
    for response in [
        ResponseTemplate::new(401).set_body_string("private provider response"),
        ResponseTemplate::new(200).set_body_string("private invalid JSON"),
        ResponseTemplate::new(200).set_body_json(serde_json::json!({"private":"response"})),
        ResponseTemplate::new(200).set_body_string("x".repeat(MAX_RESPONSE_BYTES + 1)),
        ResponseTemplate::new(302)
            .insert_header("location", "http://must-not-receive-credentials.invalid"),
    ] {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(response)
            .expect(1)
            .mount(&server)
            .await;
        let error = provider(&server, false)
            .transcribe(b"audio", &AudioFormat::default())
            .await
            .unwrap_err()
            .to_string();
        assert!(!error.contains("private"));
        assert!(!error.contains("synthetic-key"));
    }
}

#[tokio::test]
async fn incomplete_http_bodies_and_request_failures_have_content_free_errors() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let peer = tokio::spawn(async move {
        let (mut connection, _) = listener.accept().await.unwrap();
        let mut request = [0; 1024];
        let received = connection.read(&mut request).await.unwrap();
        assert!(received > 0, "the synthetic client sent an HTTP request");
        connection.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\nConnection: close\r\n\r\n{\"private\":\"response\"}").await.unwrap();
    });
    let response = Client::new()
        .get(format!("http://{address}"))
        .send()
        .await
        .unwrap();
    let error = response_json(response).await.unwrap_err().to_string();
    assert!(!error.contains("private"));
    assert!(!error.contains(&address.to_string()));
    peer.await.unwrap();
    let server = MockServer::start().await;
    let mut adapter = provider(&server, false);
    adapter.flash_endpoint = "http://[invalid-synthetic-address".into();
    let error = adapter
        .transcribe(b"audio", &AudioFormat::default())
        .await
        .unwrap_err()
        .to_string();
    assert!(!error.contains("invalid-synthetic-address"));
    assert!(server.received_requests().await.unwrap().is_empty());
}
