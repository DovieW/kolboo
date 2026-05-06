//! Integration tests for STT providers.
//!
//! These tests verify that STT providers can be created and configured correctly.
//! Note: Actual API calls require API keys - run with `cargo test -- --ignored`
//! when you have `GROQ_API_KEY`, `OPENAI_API_KEY`, or `DEEPGRAM_API_KEY` set.

use crate::stt::{
    is_retryable_error, AquavoiceSttProvider, AssemblyAiSttProvider, AudioEncoding, AudioFormat,
    DeepgramSttProvider, ElevenLabsSttProvider, FireworksSttProvider, GroqSttProvider,
    OpenAiSttProvider, SpeechmaticsSttProvider, SttError, SttProvider, WhisperServerSttProvider,
};

use serde_json::json;
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[test]
fn test_groq_provider_implements_trait() {
    let provider = GroqSttProvider::new("test_key".to_string(), None, None, None);
    assert_eq!(provider.name(), "groq");
}

#[test]
fn test_openai_provider_implements_trait() {
    let provider = OpenAiSttProvider::new("test_key".to_string(), None, None, None);
    assert_eq!(provider.name(), "openai");
}

#[test]
fn test_deepgram_provider_implements_trait() {
    let provider = DeepgramSttProvider::new("test_key".to_string(), None, None);
    assert_eq!(provider.name(), "deepgram");
}

#[test]
fn test_speechmatics_provider_implements_trait() {
    let provider = SpeechmaticsSttProvider::new("test_key".to_string(), None, None);
    assert_eq!(provider.name(), "speechmatics");
}

#[test]
fn test_groq_provider_with_custom_model() {
    let provider = GroqSttProvider::new(
        "test_key".to_string(),
        Some("distil-whisper-large-v3-en".to_string()),
        None,
        None,
    );
    assert_eq!(provider.name(), "groq");
}

#[test]
fn test_openai_provider_with_custom_model() {
    let provider = OpenAiSttProvider::new(
        "test_key".to_string(),
        Some("whisper-1".to_string()),
        None,
        None,
    );
    assert_eq!(provider.name(), "openai");
}

#[test]
fn test_deepgram_provider_with_custom_model() {
    let provider =
        DeepgramSttProvider::new("test_key".to_string(), Some("nova-2".to_string()), None);
    assert_eq!(provider.name(), "deepgram");
}

#[test]
fn provider_error_classification_preserves_retryable_status_semantics() {
    assert!(is_retryable_error(&SttError::Api(
        "Groq API error 429: rate limit".to_string()
    )));
    assert!(is_retryable_error(&SttError::Api(
        "Deepgram API error 503: upstream unavailable".to_string()
    )));
    assert!(!is_retryable_error(&SttError::Api(
        "OpenAI API error 401: unauthorized".to_string()
    )));
    assert!(!is_retryable_error(&SttError::Config(
        "missing provider API key".to_string()
    )));
}

/// Integration test for Groq STT provider.
/// Only runs if GROQ_API_KEY is set.
#[tokio::test]
#[ignore] // Run with `cargo test -- --ignored` when you have API keys
async fn test_groq_transcription_integration() {
    let api_key = match std::env::var("GROQ_API_KEY") {
        Ok(key) if !key.is_empty() => key,
        _ => {
            eprintln!("Skipping Groq integration test: GROQ_API_KEY not set");
            return;
        }
    };
    let provider = GroqSttProvider::new(api_key, None, None, None);
    let wav_data = create_test_wav_silence(1.0); // 1 second of silence
    let format = AudioFormat {
        sample_rate: 16000,
        channels: 1,
        encoding: AudioEncoding::Wav,
    };

    let result = provider.transcribe(&wav_data, &format).await;

    // Should succeed (may return empty string for silence)
    assert!(result.is_ok(), "Groq transcription failed: {:?}", result);
}

/// Integration test for OpenAI STT provider.
/// Only runs if OPENAI_API_KEY is set.
#[tokio::test]
#[ignore]
async fn test_openai_transcription_integration() {
    let api_key = match std::env::var("OPENAI_API_KEY") {
        Ok(key) if !key.is_empty() => key,
        _ => {
            eprintln!("Skipping OpenAI integration test: OPENAI_API_KEY not set");
            return;
        }
    };

    let provider = OpenAiSttProvider::new(api_key, None, None, None);
    let wav_data = create_test_wav_silence(1.0);
    let format = AudioFormat {
        sample_rate: 16000,
        channels: 1,
        encoding: AudioEncoding::Wav,
    };

    let result = provider.transcribe(&wav_data, &format).await;
    assert!(result.is_ok(), "OpenAI transcription failed: {:?}", result);
}

/// Integration test for Deepgram STT provider.
/// Only runs if DEEPGRAM_API_KEY is set.
#[tokio::test]
#[ignore]
async fn test_deepgram_transcription_integration() {
    let api_key = match std::env::var("DEEPGRAM_API_KEY") {
        Ok(key) if !key.is_empty() => key,
        _ => {
            eprintln!("Skipping Deepgram integration test: DEEPGRAM_API_KEY not set");
            return;
        }
    };

    let provider = DeepgramSttProvider::new(api_key, None, None);
    let wav_data = create_test_wav_silence(1.0);
    let format = AudioFormat {
        sample_rate: 16000,
        channels: 1,
        encoding: AudioEncoding::Wav,
    };

    let result = provider.transcribe(&wav_data, &format).await;
    assert!(
        result.is_ok(),
        "Deepgram transcription failed: {:?}",
        result
    );
}

/// Creates a minimal WAV file with silence for testing.
fn create_test_wav_silence(duration_secs: f32) -> Vec<u8> {
    use std::io::Write;

    let sample_rate: u32 = 16000;
    let channels: u16 = 1;
    let bits_per_sample: u16 = 16;
    let num_samples = (sample_rate as f32 * duration_secs) as u32;
    let byte_rate = sample_rate * channels as u32 * bits_per_sample as u32 / 8;
    let block_align = channels * bits_per_sample / 8;
    let data_size = num_samples * channels as u32 * bits_per_sample as u32 / 8;
    let file_size = 36 + data_size;

    let mut buffer = Vec::with_capacity(44 + data_size as usize);

    // RIFF header
    buffer.write_all(b"RIFF").unwrap();
    buffer.write_all(&file_size.to_le_bytes()).unwrap();
    buffer.write_all(b"WAVE").unwrap();

    // fmt chunk
    buffer.write_all(b"fmt ").unwrap();
    buffer.write_all(&16u32.to_le_bytes()).unwrap(); // chunk size
    buffer.write_all(&1u16.to_le_bytes()).unwrap(); // PCM format
    buffer.write_all(&channels.to_le_bytes()).unwrap();
    buffer.write_all(&sample_rate.to_le_bytes()).unwrap();
    buffer.write_all(&byte_rate.to_le_bytes()).unwrap();
    buffer.write_all(&block_align.to_le_bytes()).unwrap();
    buffer.write_all(&bits_per_sample.to_le_bytes()).unwrap();

    // data chunk
    buffer.write_all(b"data").unwrap();
    buffer.write_all(&data_size.to_le_bytes()).unwrap();

    // Silence (zeros)
    buffer.resize(44 + data_size as usize, 0);

    buffer
}

#[tokio::test]
async fn test_deepgram_transcribe_sends_expected_request() {
    let mock_server = MockServer::start().await;

    let guard = Mock::given(method("POST"))
        .and(path("/v1/listen"))
        .and(query_param("model", "nova-2"))
        .and(query_param("smart_format", "true"))
        .and(query_param("punctuate", "true"))
        .and(header("authorization", "Token test_key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "results": {
                "channels": [{
                    "alternatives": [{
                        "transcript": "hello deepgram"
                    }]
                }]
            }
        })))
        .expect(1)
        .mount_as_scoped(&mock_server)
        .await;

    let provider = DeepgramSttProvider::with_client(
        reqwest::Client::new(),
        "test_key".to_string(),
        None,
        None,
    )
    .with_api_base_url(mock_server.uri());

    let wav_data = create_test_wav_silence(0.1);
    let format = AudioFormat {
        sample_rate: 16000,
        channels: 1,
        encoding: AudioEncoding::Wav,
    };

    let result = provider.transcribe(&wav_data, &format).await;
    assert_eq!(result.unwrap(), "hello deepgram");

    let received = guard.received_requests().await;
    assert_eq!(received.len(), 1);

    let req = &received[0];
    let content_type = req
        .headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(content_type, "audio/wav");
}

#[tokio::test]
async fn test_deepgram_non_success_is_surface_as_api_error() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/listen"))
        .respond_with(ResponseTemplate::new(503).set_body_string("nope"))
        .expect(1)
        .mount(&mock_server)
        .await;

    let provider = DeepgramSttProvider::with_client(
        reqwest::Client::new(),
        "test_key".to_string(),
        None,
        None,
    )
    .with_api_base_url(mock_server.uri());

    let wav_data = create_test_wav_silence(0.1);
    let format = AudioFormat {
        sample_rate: 16000,
        channels: 1,
        encoding: AudioEncoding::Wav,
    };

    let err = provider
        .transcribe(&wav_data, &format)
        .await
        .expect_err("expected error");

    match err {
        SttError::Api(msg) => {
            assert!(msg.contains("503"), "expected status in message: {msg}");
            assert!(msg.contains("nope"), "expected body in message: {msg}");
        }
        other => panic!("expected SttError::Api, got: {other:?}"),
    }
}

#[tokio::test]
async fn test_assemblyai_transcribe_sends_expected_requests() {
    let mock_server = MockServer::start().await;
    let upload_url = format!("{}/upload/abc", mock_server.uri());

    let upload_guard = Mock::given(method("POST"))
        .and(path("/v2/upload"))
        .and(header("authorization", "test_key"))
        .and(header("content-type", "application/octet-stream"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "upload_url": upload_url
        })))
        .expect(1)
        .mount_as_scoped(&mock_server)
        .await;

    let submit_guard = Mock::given(method("POST"))
        .and(path("/v2/transcript"))
        .and(header("authorization", "test_key"))
        .and(header("content-type", "application/json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "transcript_123"
        })))
        .expect(1)
        .mount_as_scoped(&mock_server)
        .await;

    let get_guard = Mock::given(method("GET"))
        .and(path("/v2/transcript/transcript_123"))
        .and(header("authorization", "test_key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status": "completed",
            "text": "hello assemblyai"
        })))
        .expect(1)
        .mount_as_scoped(&mock_server)
        .await;

    let provider = AssemblyAiSttProvider::with_client(
        reqwest::Client::new(),
        "test_key".to_string(),
        None,
        None,
    )
    .with_api_base_url(mock_server.uri());

    let wav_data = create_test_wav_silence(0.1);
    let format = AudioFormat {
        sample_rate: 16000,
        channels: 1,
        encoding: AudioEncoding::Wav,
    };

    let result = provider.transcribe(&wav_data, &format).await;
    assert_eq!(result.unwrap(), "hello assemblyai");

    let upload_received = upload_guard.received_requests().await;
    assert_eq!(upload_received.len(), 1);

    let upload_req = &upload_received[0];
    let content_type = upload_req
        .headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(content_type, "application/octet-stream");
    assert_eq!(upload_req.body.len(), wav_data.len());

    let submit_received = submit_guard.received_requests().await;
    assert_eq!(submit_received.len(), 1);

    let submit_body: serde_json::Value =
        serde_json::from_slice(&submit_received[0].body).expect("valid JSON body");
    assert_eq!(
        submit_body["audio_url"].as_str(),
        Some(format!("{}/upload/abc", mock_server.uri()).as_str())
    );
    assert_eq!(
        submit_body["speech_models"].get(0).and_then(|v| v.as_str()),
        Some("universal")
    );
    assert_eq!(submit_body["punctuate"].as_bool(), Some(true));
    assert_eq!(submit_body["format_text"].as_bool(), Some(true));

    let get_received = get_guard.received_requests().await;
    assert_eq!(get_received.len(), 1);
}

#[tokio::test]
async fn test_assemblyai_upload_non_success_is_surface_as_api_error() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v2/upload"))
        .respond_with(ResponseTemplate::new(502).set_body_string("nope"))
        .expect(1)
        .mount(&mock_server)
        .await;

    let provider = AssemblyAiSttProvider::with_client(
        reqwest::Client::new(),
        "test_key".to_string(),
        None,
        None,
    )
    .with_api_base_url(mock_server.uri());

    let wav_data = create_test_wav_silence(0.1);
    let format = AudioFormat {
        sample_rate: 16000,
        channels: 1,
        encoding: AudioEncoding::Wav,
    };

    let err = provider
        .transcribe(&wav_data, &format)
        .await
        .expect_err("expected error");

    match err {
        SttError::Api(msg) => {
            assert!(msg.contains("502"), "expected status in message: {msg}");
            assert!(msg.contains("nope"), "expected body in message: {msg}");
        }
        other => panic!("expected SttError::Api, got: {other:?}"),
    }
}

#[tokio::test]
async fn test_groq_transcribe_sends_expected_request() {
    let mock_server = MockServer::start().await;

    let guard = Mock::given(method("POST"))
        .and(path("/openai/v1/audio/transcriptions"))
        .and(header("authorization", "Bearer test_key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "text": "hello groq"
        })))
        .expect(1)
        .mount_as_scoped(&mock_server)
        .await;

    let provider = GroqSttProvider::with_client(
        reqwest::Client::new(),
        "test_key".to_string(),
        Some("whisper-large-v3-turbo".to_string()),
        None,
        Some("hello prompt".to_string()),
    )
    .with_api_base_url(mock_server.uri());

    let wav_data = create_test_wav_silence(0.1);
    let format = AudioFormat {
        sample_rate: 16000,
        channels: 1,
        encoding: AudioEncoding::Wav,
    };

    let result = provider.transcribe(&wav_data, &format).await;
    assert_eq!(result.unwrap(), "hello groq");

    let received = guard.received_requests().await;
    assert_eq!(received.len(), 1);

    let req = &received[0];
    let content_type = req
        .headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        content_type.contains("multipart/form-data"),
        "expected multipart/form-data, got: {content_type}"
    );

    let body = String::from_utf8_lossy(&req.body);
    assert!(body.contains("name=\"model\""));
    assert!(body.contains("whisper-large-v3-turbo"));
    assert!(body.contains("name=\"prompt\""));
    assert!(body.contains("hello prompt"));
    assert!(body.contains("filename=\"audio.wav\""));
}

#[tokio::test]
async fn test_groq_non_success_is_surface_as_api_error() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/openai/v1/audio/transcriptions"))
        .respond_with(ResponseTemplate::new(429).set_body_string("nope"))
        .expect(1)
        .mount(&mock_server)
        .await;

    let provider = GroqSttProvider::with_client(
        reqwest::Client::new(),
        "test_key".to_string(),
        None,
        None,
        None,
    )
    .with_api_base_url(mock_server.uri());

    let wav_data = create_test_wav_silence(0.1);
    let format = AudioFormat {
        sample_rate: 16000,
        channels: 1,
        encoding: AudioEncoding::Wav,
    };

    let err = provider
        .transcribe(&wav_data, &format)
        .await
        .expect_err("expected error");

    match err {
        SttError::Api(msg) => {
            assert!(msg.contains("429"), "expected status in message: {msg}");
            assert!(msg.contains("nope"), "expected body in message: {msg}");
        }
        other => panic!("expected SttError::Api, got: {other:?}"),
    }
}

#[tokio::test]
async fn test_fireworks_transcribe_sends_expected_request() {
    let mock_server = MockServer::start().await;

    let guard = Mock::given(method("POST"))
        .and(path("/v1/audio/transcriptions"))
        .and(header("authorization", "test_key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "text": "hello fireworks"
        })))
        .expect(1)
        .mount_as_scoped(&mock_server)
        .await;

    let provider = FireworksSttProvider::with_client(
        reqwest::Client::new(),
        "test_key".to_string(),
        Some("whisper-v3-turbo".to_string()),
        None,
        Some("hello prompt".to_string()),
    )
    .with_api_base_url(mock_server.uri());

    let wav_data = create_test_wav_silence(0.1);
    let format = AudioFormat {
        sample_rate: 16000,
        channels: 1,
        encoding: AudioEncoding::Wav,
    };

    let result = provider.transcribe(&wav_data, &format).await;
    assert_eq!(result.unwrap(), "hello fireworks");

    let received = guard.received_requests().await;
    assert_eq!(received.len(), 1);

    let req = &received[0];
    let content_type = req
        .headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        content_type.contains("multipart/form-data"),
        "expected multipart/form-data, got: {content_type}"
    );

    let body = String::from_utf8_lossy(&req.body);
    assert!(body.contains("name=\"model\""));
    assert!(body.contains("whisper-v3-turbo"));
    assert!(body.contains("name=\"prompt\""));
    assert!(body.contains("hello prompt"));
    assert!(body.contains("filename=\"audio.wav\""));
}

#[tokio::test]
async fn test_fireworks_non_success_is_surface_as_api_error() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/audio/transcriptions"))
        .respond_with(ResponseTemplate::new(500).set_body_string("nope"))
        .expect(1)
        .mount(&mock_server)
        .await;

    let provider = FireworksSttProvider::with_client(
        reqwest::Client::new(),
        "test_key".to_string(),
        None,
        None,
        None,
    )
    .with_api_base_url(mock_server.uri());

    let wav_data = create_test_wav_silence(0.1);
    let format = AudioFormat {
        sample_rate: 16000,
        channels: 1,
        encoding: AudioEncoding::Wav,
    };

    let err = provider
        .transcribe(&wav_data, &format)
        .await
        .expect_err("expected error");

    match err {
        SttError::Api(msg) => {
            assert!(msg.contains("500"), "expected status in message: {msg}");
            assert!(msg.contains("nope"), "expected body in message: {msg}");
        }
        other => panic!("expected SttError::Api, got: {other:?}"),
    }
}

#[tokio::test]
async fn test_aquavoice_transcribe_sends_expected_request() {
    let mock_server = MockServer::start().await;

    let guard = Mock::given(method("POST"))
        .and(path("/api/v1/audio/transcriptions"))
        .and(header("authorization", "Bearer test_key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "text": "hello aquavoice"
        })))
        .expect(1)
        .mount_as_scoped(&mock_server)
        .await;

    let provider = AquavoiceSttProvider::with_client(
        reqwest::Client::new(),
        "test_key".to_string(),
        None,
        None,
        Some("hello prompt".to_string()),
    )
    .with_api_base_url(format!("{}/api/v1", mock_server.uri()));

    let wav_data = create_test_wav_silence(0.1);
    let format = AudioFormat {
        sample_rate: 16000,
        channels: 1,
        encoding: AudioEncoding::Wav,
    };

    let result = provider.transcribe(&wav_data, &format).await;
    assert_eq!(result.unwrap(), "hello aquavoice");

    let received = guard.received_requests().await;
    assert_eq!(received.len(), 1);

    let req = &received[0];
    let content_type = req
        .headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        content_type.contains("multipart/form-data"),
        "expected multipart/form-data, got: {content_type}"
    );

    let body = String::from_utf8_lossy(&req.body);
    assert!(body.contains("name=\"model\""));
    assert!(body.contains("avalon-v1-en"));
    assert!(body.contains("name=\"prompt\""));
    assert!(body.contains("hello prompt"));
    assert!(body.contains("filename=\"audio.wav\""));
}

#[tokio::test]
async fn test_aquavoice_non_success_is_surface_as_api_error() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/v1/audio/transcriptions"))
        .respond_with(ResponseTemplate::new(403).set_body_string("nope"))
        .expect(1)
        .mount(&mock_server)
        .await;

    let provider = AquavoiceSttProvider::with_client(
        reqwest::Client::new(),
        "test_key".to_string(),
        None,
        None,
        None,
    )
    .with_api_base_url(format!("{}/api/v1", mock_server.uri()));

    let wav_data = create_test_wav_silence(0.1);
    let format = AudioFormat {
        sample_rate: 16000,
        channels: 1,
        encoding: AudioEncoding::Wav,
    };

    let err = provider
        .transcribe(&wav_data, &format)
        .await
        .expect_err("expected error");

    match err {
        SttError::Api(msg) => {
            assert!(msg.contains("403"), "expected status in message: {msg}");
            assert!(msg.contains("nope"), "expected body in message: {msg}");
        }
        other => panic!("expected SttError::Api, got: {other:?}"),
    }
}

#[tokio::test]
async fn test_elevenlabs_transcribe_sends_expected_multipart() {
    let mock_server = MockServer::start().await;

    let guard = Mock::given(method("POST"))
        .and(path("/v1/speech-to-text"))
        .and(header("xi-api-key", "test_key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "text": "hello elevenlabs"
        })))
        .expect(1)
        .mount_as_scoped(&mock_server)
        .await;

    let provider = ElevenLabsSttProvider::with_client(
        reqwest::Client::new(),
        "test_key".to_string(),
        // Use a legacy model so the provider stays on the batch multipart endpoint.
        Some("scribe_v1".to_string()),
        None,
    )
    .with_api_base_url(mock_server.uri());

    let wav_data = create_test_wav_silence(0.1);
    let format = AudioFormat {
        sample_rate: 16000,
        channels: 1,
        encoding: AudioEncoding::Wav,
    };

    let result = provider.transcribe(&wav_data, &format).await;
    assert_eq!(result.unwrap(), "hello elevenlabs");

    let received = guard.received_requests().await;
    assert_eq!(received.len(), 1);

    let req = &received[0];
    let content_type = req
        .headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        content_type.contains("multipart/form-data"),
        "expected multipart/form-data, got: {content_type}"
    );

    let body = String::from_utf8_lossy(&req.body);
    assert!(body.contains("name=\"model_id\""));
    assert!(body.contains("scribe_v1"));
    assert!(body.contains("name=\"file\""));
    assert!(body.contains("filename=\"audio.wav\""));
    assert!(body.contains("Content-Type: audio/wav"));
}

#[tokio::test]
#[allow(clippy::result_large_err)]
// The websocket test server's handshake callback uses tungstenite's required
// response type directly; boxing it would add noise to the fixture without
// improving the behavior under test.
async fn test_elevenlabs_scribe_v2_uses_realtime_ws_and_commits_final_transcript() {
    use futures_util::{SinkExt, StreamExt};
    use serde_json::Value as JsonValue;
    use std::sync::{Arc, Mutex};
    use tokio::net::TcpListener;
    use tokio_tungstenite::tungstenite::handshake::server::{Request, Response};
    use tokio_tungstenite::tungstenite::Message;
    use tokio_tungstenite::{accept_hdr_async, WebSocketStream};

    #[derive(Debug, Default)]
    struct Capture {
        uri: Option<String>,
        api_key: Option<String>,
        received: Vec<JsonValue>,
    }

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind ws listener");
    let addr = listener.local_addr().expect("local addr");

    let capture: Arc<Mutex<Capture>> = Arc::new(Mutex::new(Capture::default()));
    let capture_for_server = capture.clone();

    let server_task = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("accept");

        let capture_for_hdr = capture_for_server.clone();
        let ws: WebSocketStream<_> =
            accept_hdr_async(stream, move |req: &Request, resp: Response| {
                let mut cap = capture_for_hdr.lock().expect("capture lock");
                cap.uri = Some(req.uri().to_string());
                cap.api_key = req
                    .headers()
                    .get("xi-api-key")
                    .and_then(|v| v.to_str().ok())
                    .map(|s| s.to_string());
                Ok(resp)
            })
            .await
            .expect("ws accept");

        let (mut write, mut read) = ws.split();

        // Optional, but makes the interaction look like the real API.
        let session_started = json!({
            "message_type": "session_started",
            "session_id": "test",
            "config": {
                "model_id": "scribe_v2_realtime"
            }
        });
        write
            .send(Message::Text(session_started.to_string().into()))
            .await
            .expect("send session_started");

        // Read audio chunk messages until we see a commit.
        let mut saw_commit = false;
        while let Some(msg) = read.next().await {
            let msg = msg.expect("ws msg ok");
            if let Message::Text(t) = msg {
                let v: JsonValue = serde_json::from_str(&t).expect("valid json");
                {
                    let mut cap = capture_for_server.lock().expect("capture lock");
                    cap.received.push(v.clone());
                }

                if v.get("message_type").and_then(|m| m.as_str()) == Some("input_audio_chunk")
                    && v.get("commit").and_then(|c| c.as_bool()) == Some(true)
                {
                    saw_commit = true;
                    break;
                }
            }
        }

        assert!(saw_commit, "expected a committed final chunk");

        let committed = json!({
            "message_type": "committed_transcript",
            "text": "hello realtime"
        });
        write
            .send(Message::Text(committed.to_string().into()))
            .await
            .expect("send committed_transcript");

        let _ = write.send(Message::Close(None)).await;
    });

    let provider = ElevenLabsSttProvider::with_client(
        reqwest::Client::new(),
        "test_key".to_string(),
        Some("scribe_v2".to_string()),
        Some("en".to_string()),
    )
    .with_api_base_url(format!("http://{}", addr));

    let wav_data = create_test_wav_silence(0.2);
    let format = AudioFormat {
        sample_rate: 16000,
        channels: 1,
        encoding: AudioEncoding::Wav,
    };

    let result = provider
        .transcribe(&wav_data, &format)
        .await
        .expect("transcribe");
    assert_eq!(result, "hello realtime");

    server_task.await.expect("server task");

    let cap = capture.lock().expect("capture lock");
    assert_eq!(cap.api_key.as_deref(), Some("test_key"));
    let uri = cap.uri.as_deref().unwrap_or("");
    assert!(
        uri.contains("/v1/speech-to-text/realtime"),
        "expected realtime path in uri, got: {uri}"
    );
    assert!(
        uri.contains("model_id=scribe_v2_realtime"),
        "expected mapped model_id in uri, got: {uri}"
    );
    assert!(
        uri.contains("commit_strategy=manual"),
        "expected commit_strategy in uri, got: {uri}"
    );
    assert!(
        uri.contains("audio_format=pcm_16000"),
        "expected audio_format in uri, got: {uri}"
    );

    assert!(
        cap.received
            .iter()
            .any(|v| v.get("message_type").and_then(|m| m.as_str()) == Some("input_audio_chunk")),
        "expected at least one input_audio_chunk message"
    );
}

#[tokio::test]
async fn test_elevenlabs_non_success_is_surface_as_api_error() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/speech-to-text"))
        .respond_with(ResponseTemplate::new(401).set_body_string("nope"))
        .expect(1)
        .mount(&mock_server)
        .await;

    let provider = ElevenLabsSttProvider::with_client(
        reqwest::Client::new(),
        "test_key".to_string(),
        // Force the legacy HTTP endpoint so we can deterministically assert status/body handling.
        Some("scribe_v1".to_string()),
        None,
    )
    .with_api_base_url(mock_server.uri());

    let wav_data = create_test_wav_silence(0.1);
    let format = AudioFormat {
        sample_rate: 16000,
        channels: 1,
        encoding: AudioEncoding::Wav,
    };

    let err = provider
        .transcribe(&wav_data, &format)
        .await
        .expect_err("expected error");

    match err {
        SttError::Api(msg) => {
            assert!(msg.contains("401"), "expected status in message: {msg}");
            assert!(msg.contains("nope"), "expected body in message: {msg}");
        }
        other => panic!("expected SttError::Api, got: {other:?}"),
    }
}

#[tokio::test]
async fn test_whisper_server_transcribe_sends_expected_multipart_and_prompt_is_clamped() {
    let mock_server = MockServer::start().await;

    let guard = Mock::given(method("POST"))
        .and(path("/v1/audio/transcriptions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "text": "hello from mock"
        })))
        .expect(1)
        .mount_as_scoped(&mock_server)
        .await;

    let prompt_raw = format!("   {}   ", "a".repeat(300));
    let expected_prompt = "a".repeat(224);

    let provider = WhisperServerSttProvider::new(
        format!("{}/v1", mock_server.uri()),
        None,
        None,
        Some(prompt_raw),
    )
    .expect("provider should be constructible");

    let wav_data = create_test_wav_silence(0.1);
    let format = AudioFormat {
        sample_rate: 16000,
        channels: 1,
        encoding: AudioEncoding::Wav,
    };

    let result = provider.transcribe(&wav_data, &format).await;
    assert_eq!(result.unwrap(), "hello from mock");

    let received = guard.received_requests().await;
    assert_eq!(received.len(), 1);

    let req = &received[0];
    let content_type = req
        .headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        content_type.contains("multipart/form-data"),
        "expected multipart/form-data, got: {content_type}"
    );

    let body = String::from_utf8_lossy(&req.body);
    assert!(body.contains("name=\"model\""));
    assert!(body.contains("whisper-1"));
    assert!(body.contains("name=\"prompt\""));
    assert!(body.contains(&expected_prompt));
    assert!(body.contains("filename=\"audio.wav\""));
}

#[tokio::test]
async fn test_whisper_server_transcribe_omits_prompt_when_empty_or_whitespace() {
    let mock_server = MockServer::start().await;

    let guard = Mock::given(method("POST"))
        .and(path("/v1/audio/transcriptions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "text": "ok" })))
        .expect(1)
        .mount_as_scoped(&mock_server)
        .await;

    let provider = WhisperServerSttProvider::new(
        format!("{}/v1", mock_server.uri()),
        None,
        None,
        Some("   ".to_string()),
    )
    .expect("provider should be constructible");

    let wav_data = create_test_wav_silence(0.1);
    let format = AudioFormat {
        sample_rate: 16000,
        channels: 1,
        encoding: AudioEncoding::Wav,
    };

    let result = provider.transcribe(&wav_data, &format).await;
    assert_eq!(result.unwrap(), "ok");

    let received = guard.received_requests().await;
    assert_eq!(received.len(), 1);

    let body = String::from_utf8_lossy(&received[0].body);
    assert!(!body.contains("name=\"prompt\""));
}

#[tokio::test]
async fn test_openai_whisper_transcribe_sends_expected_multipart_and_prompt_is_clamped() {
    let mock_server = MockServer::start().await;

    let guard = Mock::given(method("POST"))
        .and(path("/v1/audio/transcriptions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "text": "hello from openai mock"
        })))
        .expect(1)
        .mount_as_scoped(&mock_server)
        .await;

    let prompt_raw = format!("   {}   ", "a".repeat(300));
    let expected_prompt = "a".repeat(224);

    let client = reqwest::Client::new();
    let provider = OpenAiSttProvider::with_client(
        client,
        "test_key".to_string(),
        Some("whisper-1".to_string()),
        None,
        Some(prompt_raw),
    )
    .with_api_base_url(mock_server.uri());

    let wav_data = create_test_wav_silence(0.1);
    let format = AudioFormat {
        sample_rate: 16000,
        channels: 1,
        encoding: AudioEncoding::Wav,
    };

    let result = provider.transcribe(&wav_data, &format).await;
    assert_eq!(result.unwrap(), "hello from openai mock");

    let received = guard.received_requests().await;
    assert_eq!(received.len(), 1);

    let req = &received[0];
    let content_type = req
        .headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        content_type.contains("multipart/form-data"),
        "expected multipart/form-data, got: {content_type}"
    );

    let body = String::from_utf8_lossy(&req.body);
    assert!(body.contains("name=\"model\""));
    assert!(body.contains("whisper-1"));
    assert!(body.contains("name=\"prompt\""));
    assert!(body.contains(&expected_prompt));
    assert!(body.contains("filename=\"audio.wav\""));
}

#[tokio::test]
async fn test_whisper_server_non_success_is_surface_as_api_error() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/audio/transcriptions"))
        .respond_with(ResponseTemplate::new(401).set_body_string("nope"))
        .expect(1)
        .mount(&mock_server)
        .await;

    let provider =
        WhisperServerSttProvider::new(format!("{}/v1", mock_server.uri()), None, None, None)
            .expect("provider should be constructible");

    let wav_data = create_test_wav_silence(0.1);
    let format = AudioFormat {
        sample_rate: 16000,
        channels: 1,
        encoding: AudioEncoding::Wav,
    };

    let err = provider
        .transcribe(&wav_data, &format)
        .await
        .expect_err("expected error");

    match err {
        SttError::Api(msg) => {
            assert!(msg.contains("401"), "expected status in message: {msg}");
            assert!(msg.contains("nope"), "expected body in message: {msg}");
        }
        other => panic!("expected SttError::Api, got: {other:?}"),
    }
}

/// Integration test for OpenAI Realtime STT streaming.
/// Only runs if OPENAI_API_KEY is set.
///
/// Connects to the real OpenAI Realtime API via WebSocket, sends a short sine
/// tone (to verify actual audio processing), then finalizes and checks the result.
///
/// Run with: cargo test -- --ignored test_openai_realtime_streaming_integration
#[tokio::test]
#[ignore]
async fn test_openai_realtime_streaming_integration() {
    let api_key = match std::env::var("OPENAI_API_KEY") {
        Ok(key) if !key.is_empty() => key,
        _ => {
            eprintln!("Skipping OpenAI realtime integration test: OPENAI_API_KEY not set");
            return;
        }
    };

    // Test both realtime model variants.
    for model in [
        "gpt-4o-realtime-transcribe",
        "gpt-4o-mini-realtime-transcribe",
    ] {
        eprintln!("--- Testing model: {model} ---");

        let provider = OpenAiSttProvider::new(
            api_key.clone(),
            Some(model.to_string()),
            Some("en".to_string()),
            None,
        );

        // Verify provider trait flags.
        assert!(
            provider.supports_streaming(),
            "{model}: should support streaming"
        );
        assert!(
            provider.requires_streaming(),
            "{model}: should require streaming"
        );

        // Start streaming at a typical capture rate (48 kHz).
        let capture_sample_rate = 48_000u32;
        let session = provider
            .start_streaming(capture_sample_rate)
            .await
            .unwrap_or_else(|e| panic!("{model}: start_streaming failed: {e}"));

        // Send ~1 second of a 440 Hz sine tone so VAD has something to detect.
        let duration_secs = 1.0f32;
        let num_samples = (capture_sample_rate as f32 * duration_secs) as usize;
        let mut samples = Vec::with_capacity(num_samples);
        for i in 0..num_samples {
            let t = i as f32 / capture_sample_rate as f32;
            samples.push((t * 440.0 * 2.0 * std::f32::consts::PI).sin() * 0.5);
        }

        // Send in ~100ms chunks with small delays to allow server events to interleave.
        let chunk_size = capture_sample_rate as usize / 10;
        for chunk in samples.chunks(chunk_size) {
            if session.audio_tx.send(chunk.to_vec()).await.is_err() {
                break; // receiver might have closed
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }

        // Finalize — drops audio_tx and waits for the committed transcript.
        let result = session.finalize().await;
        match &result {
            Ok(text) => {
                eprintln!("{model}: transcript = {text:?}");
                // Silence/tone may produce empty text — that's fine; we just
                // need the WebSocket flow to succeed without errors.
            }
            Err(e) => {
                panic!("{model}: finalize failed: {e}");
            }
        }
    }
}
