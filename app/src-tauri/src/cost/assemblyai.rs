//! AssemblyAI pricing tables and cost estimation helpers.
//!
//! Source (fetched 2025-12-28):
//! - https://www.assemblyai.com/pricing
//!
//! Notes:
//! - AssemblyAI Speech-to-Text pricing is listed as USD per hour (billed per second).
//! - The API supports `speech_models` with enum values `universal`, `slam-1`, and `best`.
//!   The pricing page lists Universal and Slam-1. `best` is treated as a legacy alias
//!   and mapped to the Universal rate for estimation.

use crate::cost::openai::UsdMicros;

fn mul_div_round(n: u128, mul: u128, div: u128) -> u128 {
    if div == 0 {
        return 0;
    }
    // Round half up.
    (n.saturating_mul(mul).saturating_add(div / 2)) / div
}

/// Returns AssemblyAI pre-recorded STT pricing in USD micros per hour.
///
/// Models used in this app:
/// - universal: $0.15 / hour
/// - slam-1: $0.27 / hour
/// - best: legacy alias (mapped to universal)
pub fn asr_usd_micros_per_hour(model: &str) -> Option<UsdMicros> {
    match model.trim() {
        "universal" => Some(150_000),
        "slam-1" => Some(270_000),
        "best" => Some(150_000),
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
    let micros = mul_div_round(rate_per_hour, audio_millis, 3_600_000);
    Some(micros.min(u128::from(u64::MAX)) as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rates_exist_for_known_models() {
        assert!(asr_usd_micros_per_hour("universal").is_some());
        assert!(asr_usd_micros_per_hour("slam-1").is_some());
        assert!(asr_usd_micros_per_hour("best").is_some());
        assert!(asr_usd_micros_per_hour("unknown").is_none());
    }

    #[test]
    fn estimate_scales_with_duration() {
        let c1 = estimate_stt_cost_from_audio_secs("universal", 10.0).unwrap();
        let c2 = estimate_stt_cost_from_audio_secs("universal", 20.0).unwrap();
        assert!(c2 >= c1);
    }
}
