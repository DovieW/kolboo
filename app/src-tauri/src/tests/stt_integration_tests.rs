//! Integration tests for STT providers.
//!
//! These tests verify that STT providers can be created and configured correctly.
//! Note: Actual API calls require API keys - run with `cargo test -- --ignored`
//! when you have `GROQ_API_KEY`, `OPENAI_API_KEY`, or `DEEPGRAM_API_KEY` set.

use crate::stt::{
    AquavoiceSttProvider, AudioEncoding, AudioFormat, DeepgramSttProvider, ElevenLabsSttProvider,
    FireworksSttProvider, GroqSttProvider, OpenAiSttProvider, SpeechmaticsSttProvider, SttError,
    SttProvider, WhisperServerSttProvider,
};

use serde_json::json;
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[test]
fn test_groq_provider_implements_trait() {
    let provider = GroqSttProvider::new("test_key".to_string(), None, None);
    assert_eq!(provider.name(), "groq");
}

#[test]
fn test_openai_provider_implements_trait() {
    let provider = OpenAiSttProvider::new("test_key".to_string(), None, None);
    assert_eq!(provider.name(), "openai");
}

#[test]
fn test_deepgram_provider_implements_trait() {
    let provider = DeepgramSttProvider::new("test_key".to_string(), None);
    assert_eq!(provider.name(), "deepgram");
}

#[test]
fn test_speechmatics_provider_implements_trait() {
    let provider = SpeechmaticsSttProvider::new("test_key".to_string(), None);
    assert_eq!(provider.name(), "speechmatics");
}

#[test]
fn test_groq_provider_with_custom_model() {
    let provider = GroqSttProvider::new(
        "test_key".to_string(),
        Some("distil-whisper-large-v3-en".to_string()),
        None,
    );
    assert_eq!(provider.name(), "groq");
}

#[test]
fn test_openai_provider_with_custom_model() {
    let provider =
        OpenAiSttProvider::new("test_key".to_string(), Some("whisper-1".to_string()), None);
    assert_eq!(provider.name(), "openai");
}

#[test]
fn test_deepgram_provider_with_custom_model() {
    let provider = DeepgramSttProvider::new("test_key".to_string(), Some("nova-2".to_string()));
    assert_eq!(provider.name(), "deepgram");
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
    let provider = GroqSttProvider::new(api_key, None, None);
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

    let provider = OpenAiSttProvider::new(api_key, None, None);
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

    let provider = DeepgramSttProvider::new(api_key, None);
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

    let provider =
        DeepgramSttProvider::with_client(reqwest::Client::new(), "test_key".to_string(), None)
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

    let provider =
        DeepgramSttProvider::with_client(reqwest::Client::new(), "test_key".to_string(), None)
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

    let provider =
        GroqSttProvider::with_client(reqwest::Client::new(), "test_key".to_string(), None, None)
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

    let provider =
        ElevenLabsSttProvider::with_client(reqwest::Client::new(), "test_key".to_string(), None)
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
async fn test_elevenlabs_non_success_is_surface_as_api_error() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/speech-to-text"))
        .respond_with(ResponseTemplate::new(401).set_body_string("nope"))
        .expect(1)
        .mount(&mock_server)
        .await;

    let provider =
        ElevenLabsSttProvider::with_client(reqwest::Client::new(), "test_key".to_string(), None)
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

    let provider =
        WhisperServerSttProvider::new(format!("{}/v1", mock_server.uri()), None, Some(prompt_raw))
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

    let provider = WhisperServerSttProvider::new(format!("{}/v1", mock_server.uri()), None, None)
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
