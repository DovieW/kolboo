//! Integration tests for STT providers.
//!
//! These tests verify that STT providers can be created and configured correctly.
//! Note: Actual API calls require API keys - run with `cargo test -- --ignored`
//! when you have `GROQ_API_KEY`, `OPENAI_API_KEY`, or `DEEPGRAM_API_KEY` set.

use crate::stt::{
    AudioEncoding, AudioFormat, DeepgramSttProvider, GroqSttProvider, OpenAiSttProvider,
    SpeechmaticsSttProvider, SttProvider,
};

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
