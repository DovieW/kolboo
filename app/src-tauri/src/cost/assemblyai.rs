//! AssemblyAI pricing tables and cost estimation helpers.
//!
//! Source (reviewed 2026-10-08):
//! - https://www.assemblyai.com/pricing
//!
//! Notes:
//! - AssemblyAI Speech-to-Text pricing is listed as USD per hour (billed per second).
//! - Base rates only, without optional paid features.
//! - Legacy aliases can route between models; their price is unknown until the
//!   server reports the model actually used. Never present them as free.

use crate::cost::math::mul_div_round_u128;
use crate::cost::openai::UsdMicros;

/// Returns AssemblyAI pre-recorded STT pricing in USD micros per hour.
///
/// Returns None for unpriced or server-selected legacy models.
pub fn asr_usd_micros_per_hour(model: &str) -> Option<UsdMicros> {
    match model.trim() {
        "universal-3-5-pro" => Some(210_000),
        "universal-2" | "universal-streaming-english" | "universal-streaming-multilingual" => {
            Some(150_000)
        }
        "universal-3-6-pro" => Some(450_000),
        _ => None,
    }
}

/// Estimate AssemblyAI STT cost from audio duration in seconds.
///
/// Pricing is per hour; we bill proportionally by milliseconds.
pub fn estimate_stt_cost_from_audio_secs(model: &str, audio_secs: f64) -> Option<UsdMicros> {
    let rate_per_hour = asr_usd_micros_per_hour(model)? as u128;

    if !(audio_secs.is_finite() && audio_secs >= 0.0) {
        return None;
    }

    let audio_millis = (audio_secs * 1000.0).round().max(0.0) as u128;
    let micros = mul_div_round_u128(rate_per_hour, audio_millis, 3_600_000);
    Some(micros.min(u128::from(u64::MAX)) as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rates_exist_for_known_models() {
        assert_eq!(asr_usd_micros_per_hour("universal-3-5-pro"), Some(210_000));
        assert_eq!(asr_usd_micros_per_hour("universal-2"), Some(150_000));
        assert_eq!(
            asr_usd_micros_per_hour("universal-streaming-english"),
            Some(150_000)
        );
        assert_eq!(
            asr_usd_micros_per_hour("universal-streaming-multilingual"),
            Some(150_000)
        );
        assert_eq!(asr_usd_micros_per_hour("universal-3-6-pro"), Some(450_000));
        for alias in ["universal", "slam-1", "best"] {
            assert!(asr_usd_micros_per_hour(alias).is_none());
        }
        assert!(asr_usd_micros_per_hour("unknown").is_none());
    }

    #[test]
    fn estimate_scales_with_duration() {
        let c1 = estimate_stt_cost_from_audio_secs("universal-3-5-pro", 10.0).unwrap();
        let c2 = estimate_stt_cost_from_audio_secs("universal-3-5-pro", 20.0).unwrap();
        assert!(c2 >= c1);
    }
}
