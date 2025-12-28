//! Aquavoice (Avalon) pricing tables and cost estimation helpers.
//!
//! Source: https://aquavoice.com/avalon-api (fetched 2025-12-28)
//! - Pricing: $0.39 / hour of audio
//! - Billed per second
//! - No minimums

use crate::cost::openai::UsdMicros;

fn mul_div_round(n: u128, mul: u128, div: u128) -> u128 {
    if div == 0 {
        return 0;
    }
    // Round half up.
    (n.saturating_mul(mul).saturating_add(div / 2)) / div
}

/// Returns Avalon STT pricing in USD micros per hour.
pub fn asr_usd_micros_per_hour(model: &str) -> Option<UsdMicros> {
    // $0.39/hour -> 390_000 micros
    const fn usd_micros(dollars_times_1_000_000: u64) -> UsdMicros {
        dollars_times_1_000_000
    }

    let m = model.trim();

    Some(match m {
        "avalon-1" => usd_micros(390_000),
        _ => return None,
    })
}

/// Estimate Aquavoice STT cost from audio duration in seconds.
///
/// Aquavoice pricing is per hour; we bill proportionally by milliseconds.
pub fn estimate_stt_cost_from_audio_secs(model: &str, audio_secs: f64) -> Option<UsdMicros> {
    let rate_per_hour = asr_usd_micros_per_hour(model)? as u128;

    if !(audio_secs.is_finite() && audio_secs >= 0.0) {
        return None;
    }

    // Bill per-second; we compute on millisecond granularity with rounding.
    let audio_millis = (audio_secs * 1000.0).round().max(0.0) as u128;
    let micros = mul_div_round(rate_per_hour, audio_millis, 3_600_000);
    Some(micros.min(u128::from(u64::MAX)) as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rates_exist_for_known_models() {
        assert!(asr_usd_micros_per_hour("avalon-1").is_some());
        assert!(asr_usd_micros_per_hour("unknown").is_none());
    }

    #[test]
    fn estimate_zero_is_zero() {
        assert_eq!(estimate_stt_cost_from_audio_secs("avalon-1", 0.0), Some(0));
    }
}
