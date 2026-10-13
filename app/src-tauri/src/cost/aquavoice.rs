//! Aquavoice pricing tables and cost estimation helpers (Avalon API).
//!
//! Source: https://aquavoice.com/api (reviewed 2026-10-08)
//! - Pricing: $0.39 / hour of audio
//! - Billed per second
//! - Current Avalon v1.5 has a 10-second minimum; legacy estimates are retained.

use crate::cost::math::mul_div_round_u128;
use crate::cost::openai::UsdMicros;

/// Returns Avalon STT pricing in USD micros per hour.
pub fn asr_usd_micros_per_hour(model: &str) -> Option<UsdMicros> {
    // $0.39/hour -> 390_000 micros
    const fn usd_micros(dollars_times_1_000_000: u64) -> UsdMicros {
        dollars_times_1_000_000
    }

    let m = model.trim();

    Some(match m {
        // Current dashboard/API model IDs
        "avalon-v1-en" | "avalon-v1.5" => usd_micros(390_000),
        _ => return None,
    })
}

/// Estimate Aquavoice STT cost from audio duration in seconds.
///
/// Estimate at millisecond granularity, applying Avalon v1.5's ten-second
/// minimum for non-empty audio. This is not an invoice or a billing operation.
pub fn estimate_stt_cost_from_audio_secs(model: &str, audio_secs: f64) -> Option<UsdMicros> {
    let rate_per_hour = asr_usd_micros_per_hour(model)? as u128;

    if !(audio_secs.is_finite() && audio_secs >= 0.0) {
        return None;
    }

    // Do not round up to an unpublished per-second billing increment.
    let billable_secs = if model.trim() == "avalon-v1.5" && audio_secs > 0.0 {
        audio_secs.max(10.0)
    } else {
        audio_secs
    };
    let audio_millis = (billable_secs * 1000.0).round().max(0.0) as u128;
    let micros = mul_div_round_u128(rate_per_hour, audio_millis, 3_600_000);
    Some(micros.min(u128::from(u64::MAX)) as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rates_exist_for_known_models() {
        assert!(asr_usd_micros_per_hour("avalon-v1-en").is_some());
        assert!(asr_usd_micros_per_hour("unknown").is_none());
    }

    #[test]
    fn estimate_zero_is_zero() {
        assert_eq!(
            estimate_stt_cost_from_audio_secs("avalon-v1-en", 0.0),
            Some(0)
        );
    }

    #[test]
    fn current_model_applies_minimum_without_rounding_up_unpublished_increments() {
        assert_eq!(
            estimate_stt_cost_from_audio_secs("avalon-v1.5", 0.0),
            Some(0)
        );
        assert_eq!(
            estimate_stt_cost_from_audio_secs("avalon-v1.5", 0.1),
            Some(1083)
        );
        assert_eq!(
            estimate_stt_cost_from_audio_secs("avalon-v1.5", 10.1),
            Some(1094)
        );
        assert_eq!(
            estimate_stt_cost_from_audio_secs("avalon-v1.5", 3600.0),
            Some(390000)
        );
    }
}
