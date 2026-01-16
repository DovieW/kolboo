//! Contract tests for embeddings providers.

use crate::embeddings::cohere::{self, CohereEmbeddingsError};
use crate::embeddings::fireworks::{self, FireworksEmbeddingsError};
use crate::embeddings::openai::{self, OpenAiEmbeddingsError};

use serde_json::json;
use wiremock::matchers::{body_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn test_openai_embeddings_request_shape_and_headers() {
    let mock_server = MockServer::start().await;

    let expected_request = json!({
        "model": "text-embedding-3-small",
        "input": "hello world"
    });

    Mock::given(method("POST"))
        .and(path("/v1/embeddings"))
        .and(header("authorization", "Bearer test_key"))
        .and(body_json(&expected_request))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [{ "embedding": [0.1, 0.2] }]
        })))
        .expect(1)
        .mount(&mock_server)
        .await;

    let client = reqwest::Client::new();
    let embedding = openai::embed_text_with_url(
        &client,
        "test_key",
        "text-embedding-3-small",
        "hello world",
        &format!("{}/v1/embeddings", mock_server.uri()),
    )
    .await
    .unwrap();

    assert_eq!(embedding.len(), 2);
}

#[tokio::test]
async fn test_openai_embeddings_parses_json_error() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/embeddings"))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({
            "error": { "message": "bad request" }
        })))
        .expect(1)
        .mount(&mock_server)
        .await;

    let client = reqwest::Client::new();
    let err = openai::embed_text_with_url(
        &client,
        "test_key",
        "text-embedding-3-small",
        "hello world",
        &format!("{}/v1/embeddings", mock_server.uri()),
    )
    .await
    .expect_err("expected error");

    match err {
        OpenAiEmbeddingsError::Api(msg) => {
            assert!(msg.contains("400"), "expected status in message: {msg}");
            assert!(
                msg.contains("bad request"),
                "expected error in message: {msg}"
            );
        }
        other => panic!("expected OpenAiEmbeddingsError::Api, got: {other:?}"),
    }
}

#[tokio::test]
async fn test_cohere_embeddings_request_shape_and_headers() {
    let mock_server = MockServer::start().await;

    let expected_request = json!({
        "model": "embed-english-v3.0",
        "texts": ["hello world"],
        "input_type": "search_query",
        "embedding_types": ["float"]
    });

    Mock::given(method("POST"))
        .and(path("/v2/embed"))
        .and(header("authorization", "Bearer test_key"))
        .and(body_json(&expected_request))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "embeddings": { "float": [[0.25, 0.5]] }
        })))
        .expect(1)
        .mount(&mock_server)
        .await;

    let client = reqwest::Client::new();
    let inputs = vec!["hello world".to_string()];
    let embeddings = cohere::embed_texts_with_url(
        &client,
        "test_key",
        "embed-english-v3.0",
        "search_query",
        &inputs,
        &format!("{}/v2/embed", mock_server.uri()),
    )
    .await
    .unwrap();

    assert_eq!(embeddings.len(), 1);
    assert_eq!(embeddings[0].len(), 2);
}

#[tokio::test]
async fn test_cohere_embeddings_parses_json_error() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v2/embed"))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({
            "message": "bad request"
        })))
        .expect(1)
        .mount(&mock_server)
        .await;

    let client = reqwest::Client::new();
    let inputs = vec!["hello world".to_string()];
    let err = cohere::embed_texts_with_url(
        &client,
        "test_key",
        "embed-english-v3.0",
        "search_query",
        &inputs,
        &format!("{}/v2/embed", mock_server.uri()),
    )
    .await
    .expect_err("expected error");

    match err {
        CohereEmbeddingsError::Api(msg) => {
            assert!(msg.contains("400"), "expected status in message: {msg}");
            assert!(
                msg.contains("bad request"),
                "expected error in message: {msg}"
            );
        }
        other => panic!("expected CohereEmbeddingsError::Api, got: {other:?}"),
    }
}

#[tokio::test]
async fn test_fireworks_embeddings_request_shape_and_headers() {
    let mock_server = MockServer::start().await;

    let expected_request = json!({
        "model": "nomic-ai/nomic-embed-text-v1.5",
        "input": "hello world"
    });

    Mock::given(method("POST"))
        .and(path("/inference/v1/embeddings"))
        .and(header("authorization", "Bearer test_key"))
        .and(body_json(&expected_request))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [{ "embedding": [0.3, 0.6] }]
        })))
        .expect(1)
        .mount(&mock_server)
        .await;

    let client = reqwest::Client::new();
    let embedding = fireworks::embed_text_with_url(
        &client,
        "test_key",
        "nomic-ai/nomic-embed-text-v1.5",
        "hello world",
        &format!("{}/inference/v1/embeddings", mock_server.uri()),
    )
    .await
    .unwrap();

    assert_eq!(embedding.len(), 2);
}

#[tokio::test]
async fn test_fireworks_embeddings_parses_json_error() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/inference/v1/embeddings"))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({
            "error": { "message": "bad request" }
        })))
        .expect(1)
        .mount(&mock_server)
        .await;

    let client = reqwest::Client::new();
    let err = fireworks::embed_text_with_url(
        &client,
        "test_key",
        "nomic-ai/nomic-embed-text-v1.5",
        "hello world",
        &format!("{}/inference/v1/embeddings", mock_server.uri()),
    )
    .await
    .expect_err("expected error");

    match err {
        FireworksEmbeddingsError::Api(msg) => {
            assert!(msg.contains("400"), "expected status in message: {msg}");
            assert!(
                msg.contains("bad request"),
                "expected error in message: {msg}"
            );
        }
        other => panic!("expected FireworksEmbeddingsError::Api, got: {other:?}"),
    }
}
