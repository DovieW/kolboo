use crate::llm::*;
use serde_json::{json, Value};
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

fn sse(events: &[Value]) -> String {
    events
        .iter()
        .map(|value| format!("data: {value}\n\n"))
        .collect()
}

fn provider(
    kind: &str,
    url: &str,
    key: &str,
    store: Option<crate::request_log::RequestLogStore>,
) -> Box<dyn LlmProvider> {
    match kind {
        "openai" => Box::new(
            OpenAiLlmProvider::with_model(key.into(), "gpt-5.1".into())
                .with_api_base_url(url.into())
                .with_structured_outputs(false)
                .with_reasoning_effort(Some("high".into()))
                .with_request_log_store(store),
        ),
        "anthropic" => Box::new(
            AnthropicLlmProvider::with_model(key.into(), "claude-3-7-sonnet".into())
                .with_api_base_url(url.into())
                .with_thinking_budget(Some(1024))
                .with_request_log_store(store),
        ),
        "gemini" => Box::new(
            GeminiLlmProvider::with_client(
                reqwest::Client::new(),
                key.into(),
                Some("gemini-3-flash-preview".into()),
            )
            .with_api_base_url(url.into())
            .with_structured_outputs(false)
            .with_thinking_level(Some("high".into()))
            .with_request_log_store(store),
        ),
        "cohere" => Box::new(
            CohereLlmProvider::new(key.into())
                .with_api_base_url(url.into())
                .with_request_log_store(store),
        ),
        "ollama" => Box::new(
            OllamaLlmProvider::with_url(url.into(), Some("fixture".into()))
                .with_request_log_store(store),
        ),
        "groq" => Box::new(
            GroqLlmProvider::new(key.into())
                .with_api_base_url(url.into())
                .with_request_log_store(store),
        ),
        "custom" => Box::new(
            GroqLlmProvider::new(key.into())
                .for_custom_endpoint(url.into())
                .with_request_log_store(store),
        ),
        "cerebras" => Box::new(
            CerebrasLlmProvider::with_model(key.into(), "gpt-oss-120b".into())
                .with_api_base_url(url.into())
                .with_reasoning_effort(Some("high".into()))
                .with_request_log_store(store),
        ),
        "fireworks" => Box::new(
            FireworksLlmProvider::new(key.into())
                .with_api_base_url(url.into())
                .with_request_log_store(store),
        ),
        _ => Box::new(
            ManagedLlmProvider::new(key.into(), None, url.into())
                .with_selected_provider(Some("groq".into()))
                .with_request_log_store(store),
        ),
    }
}

fn successful(kind: &str) -> String {
    match kind {
        "openai" => sse(&[
            json!({"type":"response.created"}),
            json!({"type":"response.reasoning_text.delta","delta":"private thinking"}),
            json!({"type":"response.output_text.delta","output_index":1,"delta":"שלום"}),
            json!({"type":"response.output_text.delta","output_index":1,"delta":" world"}),
            json!({"type":"response.completed","response":{"status":"completed","usage":{"input_tokens":3,"output_tokens":4,"total_tokens":7}}}),
        ]),
        "anthropic" => sse(&[
            json!({"type":"ping"}),
            json!({"type":"message_start","message":{"usage":{"input_tokens":3}}}),
            json!({"type":"message_delta","usage":{"output_tokens":0}}),
            json!({"type":"content_block_start","content_block":{"type":"thinking"}}),
            json!({"type":"content_block_delta","delta":{"type":"thinking_delta","thinking":"private"}}),
            json!({"type":"content_block_start","content_block":{"type":"text","text":"שלום"}}),
            json!({"type":"content_block_delta","delta":{"type":"text_delta","text":" world"}}),
            json!({"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":4}}),
            json!({"type":"message_stop"}),
        ]),
        "gemini" => sse(&[
            json!({}),
            json!({"candidates":[{}]}),
            json!({"candidates":[{"content":{"parts":[{"thought":true,"text":"private thinking"},{"text":"שלום"},{}]}}]}),
            json!({"candidates":[{"content":{"parts":[{"text":" world"}]},"finishReason":"STOP"}],"usageMetadata":{"promptTokenCount":3,"candidatesTokenCount":4}}),
        ]),
        "cohere" => sse(&[
            json!({"type":"message-start"}),
            json!({"type":"content-delta","delta":{"message":{"content":{"text":"שלום"}}}}),
            json!({"type":"content-delta","delta":{"message":{"content":{"text":" world"}}}}),
            json!({"type":"message-end","delta":{"finish_reason":"COMPLETE","usage":{"tokens":{"input_tokens":3,"output_tokens":4}}}}),
        ]),
        "ollama" => format!(
            "{}\n{}\n{}",
            json!({"message":{"content":"שלום","thinking":"private"},"done":false}),
            json!({"message":{"content":" world"},"done":false}),
            json!({"done":true,"done_reason":"stop","prompt_eval_count":3,"eval_count":4})
        ),
        "managed" => {
            json!({"choices":[{"message":{"content":"שלום world"}}],"usage":{"total_tokens":7}})
                .to_string()
        }
        _ => format!(
            "{}data: [DONE]\n\n",
            sse(&[
                json!({"choices":[{"index":0,"delta":{"role":"assistant","reasoning_content":"private"}}]}),
                json!({"choices":[{"index":0,"delta":{"content":"שלום"}},{"index":1,"delta":{"content":"ignore"}}]}),
                json!({"choices":[{"index":0,"delta":{"content":" world"}}]}),
                json!({"choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}),
                json!({"choices":[],"usage":{"prompt_tokens":3,"completion_tokens":4,"total_tokens":7}}),
            ])
        ),
    }
}

#[tokio::test]
async fn managed_discovery_streams_when_advertised_and_never_retries_after_dispatch() {
    use wiremock::matchers::path;
    for (status, protocol, stream_status, succeeds, sends) in [
        (404, "legacy", 200, true, true),
        (200, "kolboo-quick-ask-stream-v1", 200, true, true),
        (200, "kolboo-quick-ask-stream-v1", 503, false, true),
        (200, "future-unknown", 200, false, false),
        (200, "malformed", 200, false, false),
        (200, "oversized", 200, false, false),
        (403, "unavailable", 200, false, false),
    ] {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/quick-ask/stream"))
            .respond_with(ResponseTemplate::new(status).set_body_string(
                if protocol == "oversized" {
                    "x".repeat(4097)
                } else if protocol == "malformed" {
                    "invalid JSON".into()
                } else {
                    json!({"protocol":protocol}).to_string()
                },
            ))
            .expect(1)
            .mount(&server)
            .await;
        if sends {
            Mock::given(method("POST"))
                .and(path(if status == 404 {
                    "/v1/chat/completions"
                } else {
                    "/v1/quick-ask/stream"
                }))
                .respond_with(
                    ResponseTemplate::new(stream_status).set_body_string(successful(
                        if status == 404 { "managed" } else { "groq" },
                    )),
                )
                .expect(1)
                .mount(&server)
                .await;
        }
        let provider = ManagedLlmProvider::new(
            "fixture-session".into(),
            None,
            format!("{}/v1/chat/completions", server.uri()),
        )
        .with_selected_provider(Some("groq".into()));
        let mut updates = vec![];
        let result = provider
            .complete_streaming("sys", "user", &mut |text| updates.push(text.to_string()))
            .await;
        assert_eq!(result.is_ok(), succeeds);
        if succeeds {
            assert_eq!(result.unwrap(), "שלום world");
        }
        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 1 + usize::from(sends));
        if status == 404 {
            assert!(updates.is_empty());
        }
        if status == 200 && stream_status == 200 && sends {
            assert_eq!(updates, ["שלום", "שלום world"]);
            let body: Value = serde_json::from_slice(&requests[1].body).unwrap();
            assert_eq!(body["stream"], true);
            assert_eq!(
                requests[1].headers["authorization"],
                "Bearer fixture-session"
            );
            assert_eq!(requests[1].headers["x-kolboo-model-provider"], "groq");
            assert!(requests[1].headers.contains_key("x-idempotency-key"));
        }
    }
    let provider = ManagedLlmProvider::new(
        "fixture-session".into(),
        None,
        "http://127.0.0.1:1/v1/chat/completions".into(),
    );
    assert!(provider
        .complete_streaming("sys", "user", &mut |_| {})
        .await
        .unwrap_err()
        .to_string()
        .contains("no inference was sent"));
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!(
        "http://{}/v1/chat/completions",
        listener.local_addr().unwrap()
    );
    let task = tokio::spawn(async move {
        let (mut connection, _) = listener.accept().await.unwrap();
        let mut request = [0; 4096];
        assert!(connection.read(&mut request).await.unwrap() > 0);
        connection
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 9999\r\n\r\n{\"protocol\":")
            .await
            .unwrap();
    });
    let provider = ManagedLlmProvider::new("fixture".into(), None, url);
    assert!(provider
        .complete_streaming("sys", "user", &mut |_| panic!("discovery must not publish"))
        .await
        .unwrap_err()
        .to_string()
        .contains("interrupted"));
    task.await.unwrap();
}

#[tokio::test]
async fn native_adapters_bound_accumulated_answers_before_completion() {
    for kind in ["openai", "anthropic-start", "anthropic", "cohere", "gemini"] {
        let text = "x".repeat(65536);
        let event = match kind {
            "openai" => json!({"type":"response.output_text.delta","delta":text}),
            "anthropic-start" => {
                json!({"type":"content_block_start","content_block":{"type":"text","text":text}})
            }
            "anthropic" => {
                json!({"type":"content_block_delta","delta":{"type":"text_delta","text":text}})
            }
            "cohere" => {
                json!({"type":"content-delta","delta":{"message":{"content":{"text":text}}}})
            }
            _ => json!({"candidates":[{"content":{"parts":[{"text":text}]}}]}),
        };
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_string(sse(&vec![event; 17])))
            .mount(&server)
            .await;
        let mut previews = 0;
        let provider = provider(
            if kind == "anthropic-start" {
                "anthropic"
            } else {
                kind
            },
            &server.uri(),
            "fixture",
            None,
        );
        let error = provider
            .complete_streaming("sys", "user", &mut |snapshot| {
                assert!(snapshot.len() <= 1024 * 1024);
                previews += 1;
            })
            .await
            .unwrap_err();
        assert_eq!(previews, 16, "{kind}");
        assert!(
            error.to_string().contains("supported size"),
            "{kind}: {error}"
        );
    }
}

#[tokio::test]
async fn provider_contracts_stream_only_answer_text_preserve_controls_and_usage() {
    for kind in [
        "openai",
        "anthropic",
        "gemini",
        "cohere",
        "ollama",
        "groq",
        "custom",
        "cerebras",
        "fireworks",
        "managed",
    ] {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_string(successful(kind)))
            .expect(1)
            .mount(&server)
            .await;
        let store = crate::request_log::RequestLogStore::new();
        store.start_request("fixture".into(), None);
        let provider = provider(kind, &server.uri(), "fixture-key", Some(store.clone()));
        let mut updates = vec![];
        let answer = provider
            .complete_streaming("Synthetic instruction", "Synthetic question", &mut |text| {
                updates.push(text.to_string())
            })
            .await
            .unwrap();
        assert_eq!(answer, "שלום world");
        assert_eq!(
            updates,
            if kind == "managed" {
                vec![]
            } else {
                vec!["שלום", "שלום world"]
            }
        );
        let requests = server.received_requests().await.unwrap();
        let request = &requests[0];
        let body: Value = serde_json::from_slice(&request.body).unwrap();
        if !matches!(kind, "managed" | "gemini") {
            assert_eq!(body["stream"], true, "{kind}");
        }
        let metadata = store.get_logs(Some(1))[0]
            .llm_response_json
            .clone()
            .unwrap();
        if kind != "managed" {
            assert!(!metadata.to_string().contains("שלום"));
            assert!(!metadata.to_string().contains("private"));
        }
        match kind {
            "openai" => {
                assert_eq!(body["reasoning"]["effort"], "high");
                assert_eq!(body["store"], false);
                assert!(body.get("temperature").is_none());
                assert_eq!(request.url.path(), "/v1/responses");
            }
            "anthropic" => {
                assert_eq!(body["thinking"]["budget_tokens"], 1024);
                assert_eq!(body["max_tokens"], 5120);
                assert_eq!(request.headers["x-api-key"], "fixture-key");
                assert_eq!(metadata["usage"]["input_tokens"], 3);
                assert_eq!(metadata["usage"]["output_tokens"], 4);
            }
            "gemini" => {
                assert_eq!(
                    body["generationConfig"]["thinkingConfig"]["thinkingLevel"],
                    "high"
                );
                assert!(body["generationConfig"].get("temperature").is_none());
                assert_eq!(request.url.query(), Some("alt=sse"));
            }
            "cerebras" => assert_eq!(body["reasoning_effort"], "high"),
            "custom" => {
                assert!(body.get("temperature").is_none());
                assert!(body.get("max_tokens").is_none());
            }
            "managed" => {
                assert!(body.get("stream").is_none());
                assert_eq!(request.headers["x-kolboo-model-provider"], "groq");
                assert!(request.headers.contains_key("x-idempotency-key"));
            }
            _ => {}
        }
        if !matches!(kind, "ollama" | "anthropic" | "gemini") {
            assert_eq!(request.headers["authorization"], "Bearer fixture-key");
        }
    }
}

#[tokio::test]
async fn missing_keys_never_send_a_streaming_request() {
    for kind in [
        "openai",
        "anthropic",
        "gemini",
        "cohere",
        "groq",
        "custom",
        "cerebras",
        "fireworks",
        "managed",
    ] {
        let provider = provider(kind, "http://127.0.0.1:1", "", None);
        assert!(
            matches!(
                provider
                    .complete_streaming("sys", "user", &mut |_| panic!(
                        "no preview without credentials"
                    ))
                    .await,
                Err(LlmError::NoApiKey(_))
            ),
            "{kind}"
        );
    }
}

#[tokio::test]
async fn failed_truncated_and_malformed_protocols_are_not_completed_answers() {
    let cases = [
        (
            "openai",
            sse(&[json!({"type":"response.output_text.delta"})]),
        ),
        (
            "openai",
            sse(&[json!({"type":"response.completed","response":{"status":"incomplete"}})]),
        ),
        (
            "openai",
            sse(&[json!({"type":"response.incomplete","error":{"message":"private"}})]),
        ),
        (
            "anthropic",
            sse(&[json!({"type":"content_block_delta","delta":{"type":"text_delta"}})]),
        ),
        (
            "anthropic",
            sse(&[json!({"type":"message_delta","delta":{"stop_reason":"max_tokens"}})]),
        ),
        ("anthropic", sse(&[json!({"type":"message_stop"})])),
        (
            "anthropic",
            sse(&[json!({"type":"error","error":{"message":"private"}})]),
        ),
        ("gemini", sse(&[json!({"error":{"message":"private"}})])),
        (
            "gemini",
            sse(&[json!({"promptFeedback":{"blockReason":"SAFETY"}})]),
        ),
        (
            "gemini",
            sse(&[json!({"candidates":[{"finishReason":"MAX_TOKENS"}]})]),
        ),
        ("cohere", sse(&[json!({"type":"content-delta","delta":{}})])),
        (
            "cohere",
            sse(&[json!({"type":"message-end","delta":{"finish_reason":"MAX_TOKENS"}})]),
        ),
        (
            "cohere",
            sse(&[json!({"type":"error","message":"private"})]),
        ),
        ("ollama", json!({"error":"private"}).to_string()),
        (
            "ollama",
            json!({"message":{"content":"partial"},"done":true,"done_reason":"length"}).to_string(),
        ),
        ("groq", "data: [DONE]\n\n".into()),
        ("groq", sse(&[json!({"error":{"message":"private"}})])),
        (
            "groq",
            sse(&[json!({"choices":[{"index":0,"finish_reason":"length"}]})]),
        ),
        (
            "groq",
            sse(&[
                json!({"choices":[{"index":0,"finish_reason":"stop"}]}),
                json!({"choices":[{"index":0,"delta":{"content":"late"}}]}),
            ]),
        ),
        ("groq", "data: {malformed}\n\n".into()),
    ];
    for (kind, body) in cases {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .expect(1)
            .mount(&server)
            .await;
        let error = provider(kind, &server.uri(), "fixture", None)
            .complete_streaming("sys", "user", &mut |_| {})
            .await
            .unwrap_err();
        assert!(!error.to_string().contains("private"), "{kind}");
    }
}
