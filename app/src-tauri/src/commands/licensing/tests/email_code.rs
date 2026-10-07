// Deterministic OTP protocol and safe-error regression tests.
use super::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[test]
fn email_normalization_and_safe_errors() {
    assert_eq!(
        validated_email("  Person@Example.test ").unwrap(),
        "person@example.test"
    );
    for value in ["", "missing-at", "x @example.test"] {
        assert_eq!(
            validated_email(value).unwrap_err().code.as_deref(),
            Some("auth_email_invalid")
        );
    }
    assert!(validated_email(&format!("{}@example.test", "a".repeat(321))).is_err());
    assert_eq!(
        email_code_error(StatusCode::TOO_MANY_REQUESTS, false)
            .code
            .as_deref(),
        Some("auth_code_rate_limited")
    );
    for status in [
        StatusCode::BAD_REQUEST,
        StatusCode::UNAUTHORIZED,
        StatusCode::FORBIDDEN,
    ] {
        assert_eq!(
            email_code_error(status, true).code.as_deref(),
            Some("auth_code_invalid")
        );
    }
    assert_eq!(
        email_code_error(StatusCode::FORBIDDEN, false)
            .code
            .as_deref(),
        Some("auth_code_request_failed")
    );
    assert_eq!(
        email_code_error(StatusCode::INTERNAL_SERVER_ERROR, true).retryable,
        Some(true)
    );
}

async fn mock_server(status: u16) -> (String, tokio::task::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = Vec::new();
        let mut buffer = [0_u8; 4096];
        loop {
            let length = socket.read(&mut buffer).await.unwrap();
            assert!(length > 0);
            request.extend_from_slice(&buffer[..length]);
            if let Some(header_end) = request.windows(4).position(|part| part == b"\r\n\r\n") {
                let headers = String::from_utf8_lossy(&request[..header_end]).to_lowercase();
                let content_length: usize = headers
                    .lines()
                    .find_map(|line| {
                        line.strip_prefix("content-length: ")
                            .and_then(|value| value.parse().ok())
                    })
                    .unwrap();
                if request.len() >= header_end + 4 + content_length {
                    break;
                }
            }
        }
        socket
            .write_all(
                format!(
                    "HTTP/1.1 {status} Test\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{{}}"
                )
                .as_bytes(),
            )
            .await
            .unwrap();
        String::from_utf8(request).unwrap()
    });
    (format!("http://{address}"), task)
}

#[tokio::test]
async fn otp_and_verification_requests_use_only_the_required_supabase_fields() {
    let client = reqwest::Client::new();
    for code in [None, Some("123456")] {
        let (base, server) = mock_server(200).await;
        assert!(send_email_code_request(
            &client,
            &base,
            "synthetic-public-key",
            "person@example.test",
            code
        )
        .await
        .is_ok());
        let request = server.await.unwrap();
        let (headers, body) = request.split_once("\r\n\r\n").unwrap();
        assert!(headers.starts_with(if code.is_some() {
            "POST /auth/v1/verify "
        } else {
            "POST /auth/v1/otp "
        }));
        assert!(headers
            .to_lowercase()
            .contains("apikey: synthetic-public-key"));
        assert!(!headers.to_lowercase().contains("authorization:"));
        let body: serde_json::Value = serde_json::from_str(body).unwrap();
        assert_eq!(
            body,
            match code {
                Some(code) =>
                    json!({"email": "person@example.test", "token": code, "type": "email"}),
                None => json!({"email": "person@example.test", "create_user": true}),
            }
        );
    }
    let (base, server) = mock_server(403).await;
    assert_eq!(
        send_email_code_request(
            &client,
            &base,
            "public-key",
            "person@example.test",
            Some("000000")
        )
        .await
        .unwrap_err()
        .code
        .as_deref(),
        Some("auth_code_invalid")
    );
    server.await.unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    assert_eq!(
        send_email_code_request(
            &client,
            &format!("http://{address}"),
            "public-key",
            "person@example.test",
            None
        )
        .await
        .unwrap_err()
        .code
        .as_deref(),
        Some("auth_service_unavailable")
    );
}
