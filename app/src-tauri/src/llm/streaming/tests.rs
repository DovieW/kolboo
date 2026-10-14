use super::*;
mod providers;
use serde_json::json;
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

#[test]
fn framing_handles_every_byte_boundary_crlf_multiline_and_unicode() {
    let input =
        ": ping\r\nevent: delta\r\ndata: {\"text\":\r\ndata: \"שלום\"}\r\n\r\ndata:[DONE]\n\n";
    for width in 1..=input.len() {
        let mut frames = Frames::new(true);
        let output: Vec<_> = input
            .as_bytes()
            .chunks(width)
            .flat_map(|chunk| frames.feed(chunk).unwrap())
            .collect();
        assert_eq!(output, ["{\"text\":\n\"שלום\"}", "[DONE]"]);
    }
    let mut frames = Frames::new(false);
    assert_eq!(frames.feed(b"\n{}\r\n\n").unwrap(), ["{}"]);
    assert!(Frames::new(true).feed(&[0xff, b'\n']).is_err());
    assert!(Frames::new(true).feed(&vec![b'x'; MAX_FRAME + 1]).is_err());
    let mut frames = Frames::new(true);
    frames.feed(b"data: partial\n").unwrap();
    assert!(frames.feed(&vec![b'x'; MAX_FRAME]).is_err());
}

#[test]
fn answers_are_bounded_and_billing_metadata_cannot_contain_content() {
    let mut output = Output::default();
    let mut updates = vec![];
    output
        .push("", &mut |text| updates.push(text.to_string()))
        .unwrap();
    output
        .push("Hello", &mut |text| updates.push(text.to_string()))
        .unwrap();
    output
        .push(" world", &mut |text| updates.push(text.to_string()))
        .unwrap();
    assert_eq!(updates, ["Hello", "Hello world"]);
    assert!(output
        .push(&"x".repeat(MAX_TEXT), &mut |_| panic!(
            "oversized content must not publish"
        ))
        .is_err());
    let store = crate::request_log::RequestLogStore::new();
    store.start_request("mock".into(), None);
    output.usage = Some(
        json!({"usage":usage(&json!({"input_tokens":3,"output_tokens":"secret","prompt":"private","input_tokens_details":{"cached_tokens":2,"text":"private"},"tokens":{"output_tokens":4},"cache_creation":{"ephemeral_5m_input_tokens":1},"billed_units":{"input_tokens":3}}))}),
    );
    assert_eq!(output.finish(Some(&store)).unwrap(), "Hello world");
    let value = store.get_logs(Some(1))[0]
        .llm_response_json
        .clone()
        .unwrap();
    assert_eq!(value["usage"]["input_tokens"], 3);
    assert_eq!(value["usage"]["input_tokens_details"]["cached_tokens"], 2);
    assert!(!value.to_string().contains("private"));
    assert!(!value.to_string().contains("secret"));
    assert_eq!(
        Output {
            text: "answer".into(),
            usage: None
        }
        .finish(None)
        .unwrap(),
        "answer"
    );
    assert!(Output::default().finish(None).is_err());
    assert!(super::json("private invalid text")
        .unwrap_err()
        .to_string()
        .find("private")
        .is_none());
}

#[tokio::test]
async fn transport_requires_terminal_event_and_sanitizes_http_errors() {
    for (status, body, sse, terminal, expected) in [
        (200, "data: ok\n\n", true, "ok", true),
        (200, "data: ok\n", true, "ok", false),
        (200, "{}", false, "{}", true),
        (200, "{}", false, "stop", false),
        (200, "\n", false, "stop", false),
        (200, "{bad}", false, "error", false),
        (401, "private provider details", true, "ok", false),
    ] {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(status).set_body_string(body))
            .expect(1)
            .mount(&server)
            .await;
        let result = read(
            reqwest::Client::new().post(server.uri()),
            Some(Duration::from_secs(5)),
            sse,
            &mut |data| {
                if data == "{bad}" {
                    return Err(invalid("Fixture parse failure"));
                }
                Ok(data == terminal)
            },
        )
        .await;
        assert_eq!(result.is_ok(), expected);
        if let Err(error) = result {
            assert!(!error.to_string().contains("private"));
        }
    }
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    drop(listener);
    assert!(
        read(reqwest::Client::new().post(url), None, true, &mut |_| Ok(
            true
        ))
        .await
        .unwrap_err()
        .to_string()
        .contains("Could not connect")
    );
}

#[tokio::test]
async fn deadline_bounds_a_connected_stream_without_timing_sleeps() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let (preview, seen) = tokio::sync::oneshot::channel();
    let mut preview = Some(preview);
    let task = tokio::spawn(async move {
        read(
            reqwest::Client::new().post(url),
            Some(Duration::from_secs(90)),
            true,
            &mut move |_| {
                if let Some(sender) = preview.take() {
                    sender.send(()).unwrap();
                }
                Ok(false)
            },
        )
        .await
    });
    let (mut connection, _) = listener.accept().await.unwrap();
    let mut request = [0; 4096];
    assert!(connection.read(&mut request).await.unwrap() > 0);
    connection.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: 9999\r\n\r\ndata: partial\n\n").await.unwrap();
    seen.await.unwrap();
    tokio::time::pause();
    tokio::time::advance(Duration::from_secs(91)).await;
    assert!(matches!(task.await.unwrap(), Err(LlmError::Timeout(_))));
    tokio::time::resume();
}

#[tokio::test]
async fn interrupted_oversized_and_invalid_eof_bodies_fail_without_publishing() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        let (mut connection, _) = listener.accept().await.unwrap();
        let mut request = [0; 4096];
        assert!(connection.read(&mut request).await.unwrap() > 0);
        connection
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 9999\r\n\r\ndata: partial\n\n")
            .await
            .unwrap();
    });
    let mut previews = vec![];
    let error = read(reqwest::Client::new().post(url), None, true, &mut |frame| {
        previews.push(frame.to_string());
        Ok(false)
    })
    .await
    .unwrap_err();
    server.await.unwrap();
    assert_eq!(previews, ["partial"]);
    assert!(error.to_string().contains("interrupted"));
    for (body, sse, expected) in [
        (vec![0xff], false, "UTF-8"),
        (
            b": heartbeat\n\n".repeat(MAX_BODY / 13 + 1),
            true,
            "supported size",
        ),
    ] {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(body))
            .mount(&server)
            .await;
        let error = read(
            reqwest::Client::new().post(server.uri()),
            None,
            sse,
            &mut |_| panic!("invalid body cannot publish answer frames"),
        )
        .await
        .unwrap_err();
        assert!(error.to_string().contains(expected), "{error}");
    }
}
