//! Performance benchmarks for the Kolboo pipeline.
//!
//! These benchmarks measure:
//! - VAD frame processing latency
//! - Audio resampling performance
//! - WAV encoding overhead
//! - Pipeline state transitions
//!
//! Run with: cargo test --release benchmark_ -- --nocapture

use std::time::{Duration, Instant};

/// Benchmark helper to measure execution time
fn benchmark<F, R>(name: &str, iterations: u32, mut f: F) -> Duration
where
    F: FnMut() -> R,
{
    // Warmup
    for _ in 0..10 {
        let _ = f();
    }

    let start = Instant::now();
    for _ in 0..iterations {
        let _ = f();
    }
    let total = start.elapsed();
    let per_iter = total / iterations;

    println!(
        "[BENCH] {}: {:?} total, {:?} per iteration ({} iterations)",
        name, total, per_iter, iterations
    );

    per_iter
}

#[cfg(test)]
mod vad_benchmarks {
    use super::*;
    use crate::vad::{VadConfig, VoiceActivityDetector};

    /// Benchmark VAD frame processing with silence.
    #[test]
    fn benchmark_vad_process_frame_silence() {
        let config = VadConfig::default();
        let mut vad = VoiceActivityDetector::new(config);

        // 10ms frame at 16kHz = 160 samples
        let silence = vec![0i16; 160];

        let per_iter = benchmark("VAD process_frame (silence)", 10000, || {
            vad.process_frame(&silence)
        });

        // VAD should process a frame in < 1ms to not introduce latency
        assert!(
            per_iter < Duration::from_millis(1),
            "VAD frame processing too slow: {:?}",
            per_iter
        );
    }

    /// Benchmark VAD frame processing with noise.
    #[test]
    fn benchmark_vad_process_frame_noise() {
        let config = VadConfig::default();
        let mut vad = VoiceActivityDetector::new(config);

        // Generate pseudo-random noise
        let noise: Vec<i16> = (0..160)
            .map(|i| ((i * 12345 + 6789) % 32768) as i16 - 16384)
            .collect();

        let per_iter = benchmark("VAD process_frame (noise)", 10000, || {
            vad.process_frame(&noise)
        });

        assert!(
            per_iter < Duration::from_millis(1),
            "VAD frame processing too slow: {:?}",
            per_iter
        );
    }

    /// Benchmark VAD creation.
    #[test]
    fn benchmark_vad_creation() {
        let per_iter = benchmark("VAD creation", 1000, || {
            let config = VadConfig::default();
            VoiceActivityDetector::new(config)
        });

        // VAD creation should be fast
        assert!(
            per_iter < Duration::from_millis(10),
            "VAD creation too slow: {:?}",
            per_iter
        );
    }
}

#[cfg(test)]
mod audio_benchmarks {
    use super::*;
    use crate::vad::resample_to_16khz;

    /// Benchmark resampling from 48kHz to 16kHz.
    /// Note: Run with `cargo test --release` for accurate results.
    /// Debug mode is ~20x slower due to lack of optimizations in rubato.
    #[test]
    fn benchmark_resample_48k_to_16k() {
        // 1 second of audio at 48kHz
        let samples_48k: Vec<f32> = (0..48000).map(|i| (i as f32 * 0.001).sin()).collect();

        let per_iter = benchmark("Resample 48kHz→16kHz (1s)", 100, || {
            resample_to_16khz(&samples_48k, 48000)
        });

        // Only assert in release mode - debug mode is ~20x slower
        #[cfg(not(debug_assertions))]
        assert!(
            per_iter < Duration::from_millis(100),
            "Resampling too slow: {:?}",
            per_iter
        );
        #[cfg(debug_assertions)]
        println!("Note: Skipping assertion in debug mode (use --release for accurate benchmarks)");
    }

    /// Benchmark resampling from 44.1kHz to 16kHz.
    /// Note: Run with `cargo test --release` for accurate results.
    #[test]
    fn benchmark_resample_44k_to_16k() {
        // 1 second of audio at 44.1kHz
        let samples_44k: Vec<f32> = (0..44100).map(|i| (i as f32 * 0.001).sin()).collect();

        let per_iter = benchmark("Resample 44.1kHz→16kHz (1s)", 100, || {
            resample_to_16khz(&samples_44k, 44100)
        });

        // Only assert in release mode - debug mode is ~20x slower
        #[cfg(not(debug_assertions))]
        assert!(
            per_iter < Duration::from_millis(100),
            "Resampling too slow: {:?}",
            per_iter
        );
        #[cfg(debug_assertions)]
        println!("Note: Skipping assertion in debug mode (use --release for accurate benchmarks)");
    }

    /// Measure resampling overhead for short chunks.
    /// Note: Rubato has initialization overhead per call, so short chunks are less efficient.
    /// In production, we resample the full recording at once, not per-chunk.
    #[test]
    fn benchmark_resample_short_chunk_informational() {
        // 10ms chunk at 48kHz = 480 samples
        let chunk: Vec<f32> = (0..480).map(|i| (i as f32 * 0.001).sin()).collect();

        let per_iter = benchmark("Resample 48kHz→16kHz (10ms) [INFO]", 100, || {
            resample_to_16khz(&chunk, 48000)
        });

        // Informational only - rubato has per-call overhead
        println!(
            "Note: Short chunk resampling has rubato initialization overhead."
        );
        println!(
            "In production, we resample the full recording at once ({:?} per 10ms chunk).",
            per_iter
        );
    }
}

#[cfg(test)]
mod pipeline_benchmarks {
    use super::*;
    use crate::pipeline::{PipelineConfig, PipelineState, SharedPipeline};

    /// Benchmark pipeline state queries.
    #[test]
    fn benchmark_pipeline_state_query() {
        let config = PipelineConfig::default();
        let pipeline = SharedPipeline::new(config);

        let per_iter = benchmark("Pipeline state query", 100000, || pipeline.state());

        // State query should be very fast (just a lock + read)
        assert!(
            per_iter < Duration::from_micros(100),
            "State query too slow: {:?}",
            per_iter
        );
    }

    /// Benchmark pipeline state guards.
    #[test]
    fn benchmark_state_guards() {
        let states = [
            PipelineState::Idle,
            PipelineState::Recording,
            PipelineState::Transcribing,
            PipelineState::Error,
        ];

        let per_iter = benchmark("State guard checks", 100000, || {
            for state in &states {
                let _ = state.can_start_recording();
                let _ = state.can_stop_recording();
                let _ = state.can_cancel();
            }
        });

        // Guard checks are just match expressions, should be nanoseconds
        assert!(
            per_iter < Duration::from_micros(10),
            "Guard checks too slow: {:?}",
            per_iter
        );
    }

    /// Benchmark pipeline creation.
    #[test]
    fn benchmark_pipeline_creation() {
        let _per_iter = benchmark("Pipeline creation", 100, || {
            let config = PipelineConfig::default();
            SharedPipeline::new(config)
        });

        // Pipeline creation involves audio device setup, allow more time
        println!("Note: Pipeline creation includes audio device initialization");
    }
}

#[cfg(test)]
mod provider_benchmarks {
    use super::*;
    use crate::llm::PromptSections;
    use crate::stt::{AudioEncoding, AudioFormat};

    /// Measure provider creation time (informational).
    /// Note: Provider creation includes HTTP client setup with TLS, which is intentionally slow.
    /// Providers are created once at app startup, not per-request.
    #[test]
    fn benchmark_provider_creation_informational() {
        use crate::llm::OpenAiLlmProvider;
        use crate::stt::GroqSttProvider;

        println!("\n[INFO] Provider creation benchmarks:");
        println!("Note: Providers include HTTP client with TLS setup.");
        println!("This is a one-time cost at app startup.\n");

        // Just measure a few iterations
        let start = Instant::now();
        for _ in 0..10 {
            let _ = GroqSttProvider::new("test_key".to_string(), None, None);
        }
        let stt_time = start.elapsed() / 10;
        println!("[BENCH] STT provider creation: {:?} per provider", stt_time);

        let start = Instant::now();
        for _ in 0..10 {
            let _ = OpenAiLlmProvider::new("test_key".to_string());
        }
        let llm_time = start.elapsed() / 10;
        println!("[BENCH] LLM provider creation: {:?} per provider", llm_time);

        println!("\nNote: In production, providers are created once and reused.");
    }

    /// Benchmark AudioFormat creation.
    #[test]
    fn benchmark_audio_format_creation() {
        let per_iter = benchmark("AudioFormat creation", 100000, || AudioFormat {
            sample_rate: 16000,
            channels: 1,
            encoding: AudioEncoding::Wav,
        });

        assert!(
            per_iter < Duration::from_micros(1),
            "AudioFormat creation too slow: {:?}",
            per_iter
        );
    }

    /// Benchmark PromptSections default.
    #[test]
    fn benchmark_prompt_sections_default() {
        let per_iter = benchmark("PromptSections::default()", 10000, PromptSections::default);

        assert!(
            per_iter < Duration::from_micros(100),
            "PromptSections default too slow: {:?}",
            per_iter
        );
    }
}

#[cfg(test)]
mod memory_benchmarks {
    /// Measure memory for typical audio buffer sizes.
    #[test]
    fn benchmark_audio_buffer_memory() {
        println!("\n[MEMORY] Audio buffer sizes:");

        // 1 second at 16kHz mono i16
        let one_sec_16k = std::mem::size_of::<i16>() * 16000;
        println!(
            "  1s @ 16kHz mono i16: {} bytes ({:.2} KB)",
            one_sec_16k,
            one_sec_16k as f64 / 1024.0
        );

        // 1 second at 48kHz stereo f32
        let one_sec_48k_stereo = std::mem::size_of::<f32>() * 48000 * 2;
        println!(
            "  1s @ 48kHz stereo f32: {} bytes ({:.2} KB)",
            one_sec_48k_stereo,
            one_sec_48k_stereo as f64 / 1024.0
        );

        // 30 seconds at 48kHz mono f32 (typical max recording)
        let thirty_sec = std::mem::size_of::<f32>() * 48000 * 30;
        println!(
            "  30s @ 48kHz mono f32: {} bytes ({:.2} MB)",
            thirty_sec,
            thirty_sec as f64 / 1024.0 / 1024.0
        );

        // 60 seconds at 48kHz mono f32
        let sixty_sec = std::mem::size_of::<f32>() * 48000 * 60;
        println!(
            "  60s @ 48kHz mono f32: {} bytes ({:.2} MB)",
            sixty_sec,
            sixty_sec as f64 / 1024.0 / 1024.0
        );

        // Max recording size (50MB limit)
        let max_recording = 50 * 1024 * 1024;
        let max_duration_48k =
            max_recording as f64 / (std::mem::size_of::<f32>() as f64 * 48000.0);
        println!(
            "  Max recording (50MB) @ 48kHz mono f32: {:.1} seconds",
            max_duration_48k
        );
    }

    /// Measure struct sizes.
    #[test]
    fn benchmark_struct_sizes() {
        use crate::pipeline::PipelineConfig;
        use crate::stt::{AudioFormat, RetryConfig};
        use crate::vad::VadConfig;

        println!("\n[MEMORY] Struct sizes:");
        println!("  VadConfig: {} bytes", std::mem::size_of::<VadConfig>());
        println!("  AudioFormat: {} bytes", std::mem::size_of::<AudioFormat>());
        println!("  RetryConfig: {} bytes", std::mem::size_of::<RetryConfig>());
        println!(
            "  PipelineConfig: {} bytes",
            std::mem::size_of::<PipelineConfig>()
        );
    }
}
