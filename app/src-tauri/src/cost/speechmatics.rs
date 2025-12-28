//! Speechmatics pricing tables and cost estimation helpers.
//!
//! Sources (fetched 2025-12-28):
//! - https://www.speechmatics.com/pricing
//!
//! Notes:
//! - Speechmatics STT pricing is listed as USD per hour.
//! - Billing is described as per-second ("billed to the second").
//! - The public pricing page distinguishes "Standard" vs "Enhanced" models.

use crate::cost::openai::UsdMicros;

fn mul_div_round(n: u128, mul: u128, div: u128) -> u128 {
    if div == 0 {
        return 0;
    }
    // Round half up.
    (n.saturating_mul(mul).saturating_add(div / 2)) / div
}

/// Returns Speechmatics STT pricing in USD micros per hour.
///
/// `model` corresponds to Speechmatics `operating_point`.
///
/// Known models:
/// - `standard`: $0.24 / hour
/// - `enhanced`: $0.40 / hour
pub fn asr_usd_micros_per_hour(model: &str) -> Option<UsdMicros> {
    match model.trim().to_lowercase().as_str() {
        "standard" => Some(240_000),
        "enhanced" => Some(400_000),
        _ => None,
    }
}

/// Estimate Speechmatics STT cost from audio duration in seconds.
///
/// Speechmatics bills Pro usage to the second; we round to milliseconds.
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
        assert_eq!(asr_usd_micros_per_hour("standard"), Some(240_000));
        assert_eq!(asr_usd_micros_per_hour("enhanced"), Some(400_000));
    }

    #[test]
    fn cost_is_zero_for_zero_audio() {
        let c = estimate_stt_cost_from_audio_secs("standard", 0.0).unwrap();
        assert_eq!(c, 0);
    }
}
