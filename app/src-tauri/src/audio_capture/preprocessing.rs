//! Stop-time audio preprocessing helpers for captured microphone buffers.
//!
//! Keep these helpers separate from `audio_normalization.rs`: normalization owns
//! provider-independent format conversion/resampling, while this module owns the
//! user-facing capture cleanup controls (high-pass, AGC, suppression, and gate).

fn clamp_u8_0_100(v: u8) -> u8 {
    v.min(100)
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn db_to_amp(db: f32) -> f32 {
    // db is dBFS; 0 dBFS == full-scale amplitude (1.0).
    10.0_f32.powf(db / 20.0)
}

pub(super) fn noise_gate_threshold_dbfs_from_strength(strength: u8) -> Option<f32> {
    let strength = clamp_u8_0_100(strength);
    if strength == 0 {
        None
    } else {
        let t = strength as f32 / 100.0;
        Some(lerp(-75.0, -30.0, t))
    }
}

pub(super) fn apply_highpass_dc_block(samples: &mut [f32], sample_rate: u32) {
    // Simple DC-blocking high-pass filter.
    // Good enough to reduce rumble / DC offset without heavy DSP.
    let sr = sample_rate.max(1) as f32;
    // Choose r based on a rough cutoff. Keep stable across SR.
    // r close to 1.0 => lower cutoff.
    let cutoff_hz = 80.0_f32;
    let r = (-2.0 * std::f32::consts::PI * cutoff_hz / sr).exp();
    let mut y_prev = 0.0_f32;
    let mut x_prev = 0.0_f32;
    for x in samples.iter_mut() {
        let y = *x - x_prev + r * y_prev;
        x_prev = *x;
        y_prev = y;
        *x = y;
    }
}

pub(super) fn apply_agc(samples: &mut [f32]) {
    // Lightweight gain normalization.
    // Target a strong peak while capping max gain to avoid crazy amplification.
    let mut peak = 0.0_f32;
    let mut sum_sq = 0.0_f64;
    for &s in samples.iter() {
        peak = peak.max(s.abs());
        sum_sq += (s as f64) * (s as f64);
    }
    if samples.is_empty() {
        return;
    }
    let rms = (sum_sq / samples.len() as f64).sqrt() as f32;

    // Avoid amplifying true silence.
    if peak < 1e-6 && rms < 1e-6 {
        return;
    }

    let target_peak = 0.90_f32;
    let target_rms = 0.10_f32; // ~ -20 dBFS
    let max_gain = 8.0_f32;

    let gain_peak = if peak > 0.0 { target_peak / peak } else { 1.0 };
    let gain_rms = if rms > 0.0 { target_rms / rms } else { 1.0 };
    let gain = gain_peak.min(gain_rms).clamp(0.1, max_gain);

    for s in samples.iter_mut() {
        *s = (*s * gain).clamp(-1.0, 1.0);
    }
}

pub(super) fn apply_light_noise_suppression(samples: &mut [f32], sample_rate: u32) {
    // Extremely lightweight noise suppression:
    // estimate a noise floor from the first ~200ms and apply soft subtraction.
    if samples.is_empty() {
        return;
    }

    let sr = sample_rate.max(1) as usize;
    let window = (sr as f32 * 0.20) as usize; // ~200ms
    let n = window.clamp(1, samples.len());

    let mut sum_sq = 0.0_f64;
    for &s in samples.iter().take(n) {
        sum_sq += (s as f64) * (s as f64);
    }
    let floor_rms = (sum_sq / n as f64).sqrt() as f32;
    if !floor_rms.is_finite() || floor_rms <= 0.0 {
        return;
    }

    // Subtract most of the estimated floor; keep some to avoid pumping.
    let subtract = floor_rms * 0.8;
    for s in samples.iter_mut() {
        let a = s.abs();
        let sign = if *s >= 0.0 { 1.0 } else { -1.0 };
        let out = (a - subtract).max(0.0);
        *s = (sign * out).clamp(-1.0, 1.0);
    }
}

/// Apply a simple noise gate to interleaved samples.
///
/// - `samples` are interleaved f32 in [-1, 1]
/// - `channels` is the interleaving width
/// - `threshold_dbfs` is a negative dBFS value (e.g. -60). `None` => bypass.
///
/// This is intentionally lightweight and runs at stop-time (offline), not in the
/// real-time capture callback.
pub(super) fn apply_noise_gate_interleaved(
    samples: &[f32],
    sample_rate: u32,
    channels: u16,
    threshold_dbfs: Option<f32>,
) -> Vec<f32> {
    let Some(threshold_dbfs) = threshold_dbfs else {
        return samples.to_vec();
    };
    if samples.is_empty() {
        return samples.to_vec();
    }

    // UI-range clamp. Keep conservative.
    let threshold_dbfs = if threshold_dbfs.is_finite() {
        threshold_dbfs.clamp(-75.0, -30.0)
    } else {
        -60.0
    };

    let channels_usize = channels.max(1) as usize;
    let frames = samples.len() / channels_usize;

    let threshold_amp = db_to_amp(threshold_dbfs);

    // Hysteresis reduces "chattering" around the threshold.
    let close_threshold_amp = threshold_amp * 0.85;

    // Attack/release smoothing on the *gain* to avoid clicks.
    let fs = sample_rate.max(1) as f32;
    let attack_s = 0.005_f32;
    let release_s = 0.120_f32;

    let attack_alpha = (-1.0 / (attack_s * fs)).exp();
    let release_alpha = (-1.0 / (release_s * fs)).exp();

    let mut out = Vec::with_capacity(samples.len());
    let mut gate_open = false;
    let mut gain: f32 = 0.0;

    for frame_idx in 0..frames {
        let base = frame_idx * channels_usize;

        // Envelope: max abs across channels for this frame.
        let mut env = 0.0_f32;
        for c in 0..channels_usize {
            env = env.max(samples[base + c].abs());
        }

        if gate_open {
            if env < close_threshold_amp {
                gate_open = false;
            }
        } else if env > threshold_amp {
            gate_open = true;
        }

        let target = if gate_open { 1.0_f32 } else { 0.0_f32 };
        let alpha = if target > gain {
            attack_alpha
        } else {
            release_alpha
        };
        gain = target + alpha * (gain - target);

        for c in 0..channels_usize {
            out.push(samples[base + c] * gain);
        }
    }

    // If there were trailing samples not forming a whole frame, just copy them.
    // (Shouldn't happen in normal capture, but keep it safe.)
    let consumed = frames * channels_usize;
    if consumed < samples.len() {
        out.extend_from_slice(&samples[consumed..]);
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: f32, b: f32) -> bool {
        (a - b).abs() < 0.0001
    }

    #[test]
    fn noise_gate_strength_maps_to_expected_threshold_range() {
        assert_eq!(noise_gate_threshold_dbfs_from_strength(0), None);
        assert!(approx_eq(
            noise_gate_threshold_dbfs_from_strength(1).unwrap(),
            -74.55
        ));
        assert!(approx_eq(
            noise_gate_threshold_dbfs_from_strength(50).unwrap(),
            -52.5
        ));
        assert!(approx_eq(
            noise_gate_threshold_dbfs_from_strength(100).unwrap(),
            -30.0
        ));
    }

    #[test]
    fn noise_gate_none_is_bypass() {
        let samples = [0.1, -0.2, 0.3];
        assert_eq!(
            apply_noise_gate_interleaved(&samples, 16_000, 1, None),
            samples
        );
    }

    #[test]
    fn agc_leaves_true_silence_unchanged() {
        let mut samples = [0.0_f32; 8];
        apply_agc(&mut samples);
        assert_eq!(samples, [0.0_f32; 8]);
    }

    #[test]
    fn highpass_reduces_constant_dc_offset() {
        let mut samples = vec![0.5_f32; 256];
        apply_highpass_dc_block(&mut samples, 16_000);
        let tail = samples.last().copied().unwrap_or_default().abs();
        assert!(tail < 0.1, "tail should decay toward zero, got {tail}");
    }

    #[test]
    fn light_noise_suppression_reduces_floor_without_changing_sign() {
        let mut samples = vec![0.05_f32, -0.05, 0.10, -0.10];
        apply_light_noise_suppression(&mut samples, 16_000);
        assert!(samples[0].abs() < 0.05);
        assert!(samples[1].is_sign_negative());
    }
}
