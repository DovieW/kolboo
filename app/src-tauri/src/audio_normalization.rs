//! Shared audio format normalization helpers.
//!
//! Keep pure sample-format behavior here so STT providers, VAD, and capture
//! code do not each carry their own tiny interpretation of mono/downmix/PCM or
//! resampling rules. The Module intentionally preserves two resampling paths:
//! a latency-friendly linear helper for streaming providers, and the existing
//! higher-quality `rubato` helper for VAD/offline 16 kHz normalization.

/// Convert interleaved f32 samples to mono by averaging each full frame.
///
/// Incomplete trailing frames are ignored. That matches the previous capture
/// behavior and avoids inventing samples when a device callback delivers an odd
/// buffer length.
pub(crate) fn downmix_interleaved_to_mono(samples: &[f32], channels: usize) -> Vec<f32> {
    let channels = channels.max(1);
    if channels == 1 {
        return samples.to_vec();
    }

    let frames = samples.len() / channels;
    let mut mono = Vec::with_capacity(frames);
    for frame_idx in 0..frames {
        let base = frame_idx * channels;
        let mut sum = 0.0_f32;
        for c in 0..channels {
            sum += samples[base + c];
        }
        mono.push(sum / channels as f32);
    }
    mono
}

/// Allocation-reusing variant of [`downmix_interleaved_to_mono`].
///
/// Capture workers use this in hot paths so VAD chunks can be recycled through
/// a small buffer pool instead of allocating for every callback.
pub(crate) fn downmix_interleaved_chunk_to_mono_into(
    samples: &[f32],
    channels: usize,
    out: &mut Vec<f32>,
) {
    let channels = channels.max(1);
    out.clear();
    if samples.is_empty() {
        return;
    }

    if channels == 1 {
        out.extend_from_slice(samples);
        return;
    }

    let frames = samples.len() / channels;
    // Callers normally provision enough capacity from a pool. Reserve only when
    // needed so the common capture path remains allocation-free.
    out.reserve(frames);
    for frame_idx in 0..frames {
        let base = frame_idx * channels;
        let mut sum = 0.0_f32;
        for c in 0..channels {
            sum += samples[base + c];
        }
        out.push(sum / channels as f32);
    }
}

/// Compute a chunk size (in bytes) for PCM s16le audio.
///
/// The result is clamped and aligned upward to a whole sample frame. Streaming
/// providers share this so buffer sizing does not drift by provider.
pub(crate) fn chunk_size_bytes_for_pcm_s16le(
    sample_rate: u32,
    channels: u8,
    target_chunk_ms: u32,
    min_bytes: usize,
    max_bytes: usize,
) -> usize {
    let channels = channels.max(1) as usize;
    let bytes_per_frame = channels.saturating_mul(2); // i16 per channel
    let bytes_per_second = (sample_rate as usize)
        .saturating_mul(channels)
        .saturating_mul(2);

    let mut chunk = bytes_per_second.saturating_mul(target_chunk_ms as usize) / 1000;

    chunk = chunk.clamp(min_bytes, max_bytes);

    if bytes_per_frame > 0 {
        let rem = chunk % bytes_per_frame;
        if rem != 0 {
            chunk = chunk.saturating_add(bytes_per_frame - rem);
        }
    }

    chunk
}

/// Convert mono f32 samples into little-endian PCM s16le bytes.
///
/// Capture represents samples as f32 in `[-1.0, 1.0]`. Clamp before converting
/// so provider adapters cannot drift on clipping behavior.
pub(crate) fn f32_to_pcm_s16le(samples: &[f32]) -> Vec<u8> {
    let mut pcm = Vec::with_capacity(samples.len() * 2);
    for &s in samples {
        let clamped = s.clamp(-1.0, 1.0);
        let val = (clamped * i16::MAX as f32).round() as i16;
        pcm.extend_from_slice(&val.to_le_bytes());
    }
    pcm
}

/// Resample mono f32 audio with simple linear interpolation.
///
/// This is intentionally latency-friendly and dependency-free for live
/// streaming providers. Higher-quality offline/VAD conversion uses
/// [`resample_to_16khz_vad_quality`] instead.
pub(crate) fn resample_linear(input: &[f32], input_rate: u32, output_rate: u32) -> Vec<f32> {
    if input_rate == output_rate || input.is_empty() {
        return input.to_vec();
    }

    // Avoid division by zero while preserving previous "return input" fallback
    // semantics for invalid sample-rate data.
    if input_rate == 0 || output_rate == 0 {
        log::warn!(
            "resample_linear: invalid sample rate input_rate={} output_rate={}; returning input unchanged",
            input_rate,
            output_rate
        );
        return input.to_vec();
    }

    let ratio = input_rate as f64 / output_rate as f64;
    let output_len = (input.len() as f64 / ratio).ceil() as usize;
    let mut output = Vec::with_capacity(output_len);

    for i in 0..output_len {
        let src_idx = i as f64 * ratio;
        let idx0 = src_idx.floor() as usize;
        let idx1 = (idx0 + 1).min(input.len() - 1);
        let frac = (src_idx - idx0 as f64) as f32;
        output.push(input[idx0] * (1.0 - frac) + input[idx1] * frac);
    }

    output
}

/// Resample mono audio to 16 kHz using the existing VAD/offline quality path.
///
/// Do not substitute this into live streaming paths without measuring latency:
/// `rubato` is intentionally higher quality than the lightweight linear helper.
pub(crate) fn resample_to_16khz_vad_quality(samples: &[f32], source_sample_rate: u32) -> Vec<f32> {
    use audioadapter_buffers::direct::InterleavedSlice;
    use rubato::{
        Async, FixedAsync, Resampler, SincInterpolationParameters, SincInterpolationType,
        WindowFunction,
    };

    if source_sample_rate == 16_000 {
        return samples.to_vec();
    }

    if samples.is_empty() {
        return Vec::new();
    }

    if source_sample_rate == 0 {
        log::warn!(
            "resample_to_16khz_vad_quality: source_sample_rate=0; returning input unchanged"
        );
        return samples.to_vec();
    }

    let params = SincInterpolationParameters {
        sinc_len: 256,
        f_cutoff: 0.95,
        interpolation: SincInterpolationType::Linear,
        oversampling_factor: 256,
        window: WindowFunction::BlackmanHarris2,
    };

    let resample_ratio = 16_000.0 / source_sample_rate as f64;
    let input_len_frames = samples.len();
    let chunk_size_frames = input_len_frames.clamp(1, 1024);

    // rubato v1 uses AudioAdapter-based input/output buffers. We only use mono
    // data here, so the adapter channel count is fixed to 1.
    let mut resampler = match Async::<f32>::new_sinc(
        resample_ratio,
        2.0, // max relative ratio (allows some drift if reused for streams)
        &params,
        chunk_size_frames,
        1,
        FixedAsync::Input,
    ) {
        Ok(r) => r,
        Err(e) => {
            log::error!("Failed to create resampler: {}", e);
            return samples.to_vec();
        }
    };

    let input_adapter = match InterleavedSlice::new(samples, 1, input_len_frames) {
        Ok(a) => a,
        Err(e) => {
            log::error!("Failed to create input adapter: {}", e);
            return samples.to_vec();
        }
    };

    let output_len_frames = resampler.process_all_needed_output_len(input_len_frames);
    let mut out = vec![0.0_f32; output_len_frames];
    let out_capacity_frames = out.len();
    let mut output_adapter = match InterleavedSlice::new_mut(&mut out, 1, out_capacity_frames) {
        Ok(a) => a,
        Err(e) => {
            log::error!("Failed to create output adapter: {}", e);
            return samples.to_vec();
        }
    };

    match resampler.process_all_into_buffer(
        &input_adapter,
        &mut output_adapter,
        input_len_frames,
        None,
    ) {
        Ok((_frames_read, frames_written)) => {
            out.truncate(frames_written);
            out
        }
        Err(e) => {
            log::error!("Resampling failed: {}", e);
            samples.to_vec()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn downmix_interleaved_averages_complete_frames() {
        let samples = [1.0_f32, -1.0, 0.5, 0.25, 10.0];
        assert_eq!(downmix_interleaved_to_mono(&samples, 2), vec![0.0, 0.375]);
    }

    #[test]
    fn downmix_into_reuses_output_buffer() {
        let mut out = vec![99.0_f32];
        downmix_interleaved_chunk_to_mono_into(&[1.0, 3.0, -1.0, 1.0], 2, &mut out);
        assert_eq!(out, vec![2.0, 0.0]);

        downmix_interleaved_chunk_to_mono_into(&[], 2, &mut out);
        assert!(out.is_empty());
    }

    #[test]
    fn chunk_size_respects_target_and_alignment() {
        let mono = chunk_size_bytes_for_pcm_s16le(16_000, 1, 100, 2_048, 32_768);
        assert_eq!(mono, 3_200);
        assert_eq!(mono % 2, 0);

        let stereo = chunk_size_bytes_for_pcm_s16le(16_000, 2, 100, 2_048, 32_768);
        assert_eq!(stereo % 4, 0);
    }

    #[test]
    fn chunk_size_clamps() {
        assert_eq!(
            chunk_size_bytes_for_pcm_s16le(16_000, 1, 1, 2_048, 32_768),
            2_048
        );
        assert_eq!(
            chunk_size_bytes_for_pcm_s16le(48_000, 1, 10_000, 2_048, 32_768),
            32_768
        );
    }

    #[test]
    fn f32_to_pcm_s16le_converts_and_clamps_samples() {
        let samples = vec![0.0_f32, 1.0, -1.0, 0.5, 2.0, -2.0];
        let pcm = f32_to_pcm_s16le(&samples);

        assert_eq!(pcm.len(), samples.len() * 2);
        assert_eq!(i16::from_le_bytes([pcm[0], pcm[1]]), 0);
        assert_eq!(i16::from_le_bytes([pcm[2], pcm[3]]), i16::MAX);
        assert_eq!(i16::from_le_bytes([pcm[4], pcm[5]]), -i16::MAX);
        assert_eq!(i16::from_le_bytes([pcm[6], pcm[7]]), 16_384);
        assert_eq!(i16::from_le_bytes([pcm[8], pcm[9]]), i16::MAX);
        assert_eq!(i16::from_le_bytes([pcm[10], pcm[11]]), -i16::MAX);
    }

    #[test]
    fn resample_linear_passthrough_for_same_rate_empty_or_invalid_rates() {
        let input = vec![1.0_f32, 2.0, 3.0];
        assert_eq!(resample_linear(&input, 16_000, 16_000), input);
        assert!(resample_linear(&[], 48_000, 16_000).is_empty());
        assert_eq!(resample_linear(&input, 0, 16_000), input);
        assert_eq!(resample_linear(&input, 16_000, 0), input);
    }

    #[test]
    fn resample_linear_downsamples_with_existing_provider_math() {
        let input: Vec<f32> = (0..300).map(|i| i as f32).collect();
        let output = resample_linear(&input, 48_000, 16_000);

        assert_eq!(output.len(), 100);
        assert_eq!(output[0], 0.0);
        assert_eq!(output[1], 3.0);
        assert_eq!(output[99], 297.0);
    }

    #[test]
    fn resample_linear_upsamples_with_interpolation() {
        let input = vec![0.0_f32, 10.0];
        let output = resample_linear(&input, 1, 4);

        assert_eq!(output, vec![0.0, 2.5, 5.0, 7.5, 10.0, 10.0, 10.0, 10.0]);
    }

    #[test]
    fn vad_quality_resample_preserves_empty_and_invalid_rate_fallbacks() {
        assert!(resample_to_16khz_vad_quality(&[], 48_000).is_empty());
        let input = vec![0.1_f32, -0.2, 0.3];
        assert_eq!(resample_to_16khz_vad_quality(&input, 0), input);
        assert_eq!(resample_to_16khz_vad_quality(&input, 16_000), input);
    }
}
