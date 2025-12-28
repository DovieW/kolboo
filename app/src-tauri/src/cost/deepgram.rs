//! Deepgram pricing tables and cost estimation helpers.
//!
//! Source (fetched 2025-12-28):
//! - https://deepgram.com/pricing
//! - https://developers.deepgram.com/docs/models-languages-overview
//! - https://developers.deepgram.com/docs/pre-recorded-audio
//!
//! Notes:
//! - Deepgram Speech-to-Text pricing is listed as USD per minute.
//! - `model=nova-3` is described in the docs as a multilingual model option, so we
//!   map it to the Nova-3 (Multilingual) pricing tier.

use crate::cost::openai::UsdMicros;

fn mul_div_round(n: u128, mul: u128, div: u128) -> u128 {
    if div == 0 {
        return 0;
    }
    // Round half up.
    (n.saturating_mul(mul).saturating_add(div / 2)) / div
}

/// Returns Deepgram STT pricing in USD micros per minute.
///
/// Models used in this app:
/// - nova-3
/// - nova-2
/// - nova
/// - enhanced
/// - base
pub fn stt_usd_micros_per_minute(model: &str) -> Option<UsdMicros> {
    // $X.xx/min -> micros
    const fn usd_micros(dollars_times_1_000_000: u64) -> UsdMicros {
        dollars_times_1_000_000
    }

    let m = model.trim();

    Some(match m {
        // Nova-3 pricing (Pay As You Go)
        // Pricing page lists Nova-3 Monolingual and Multilingual. Docs say `nova-3`
        // is multilingual, so we use the multilingual rate.
        "nova-3" | "nova-3-general" | "nova-3-general-nova" => usd_micros(9_200),

        // A best-effort mapping for an English-only Nova-3 variant, if used.
        // (Not currently in the app model list.)
        "nova-3-general-en" => usd_micros(7_700),

        // Nova-1 & 2 pricing (Pay As You Go)
        "nova-2" | "nova-2-general" | "nova" | "nova-general" => usd_micros(5_800),

        // Legacy models
        "enhanced" | "enhanced-general" => usd_micros(16_500),
        "base" | "base-general" => usd_micros(14_500),

        _ => return None,
    })
}

/// Estimate Deepgram STT cost from audio duration in seconds.
///
/// Deepgram pricing is per minute; we bill proportionally by milliseconds.
pub fn estimate_stt_cost_from_audio_secs(model: &str, audio_secs: f64) -> Option<UsdMicros> {
    let rate_per_min = stt_usd_micros_per_minute(model)? as u128;

    if !(audio_secs.is_finite() && audio_secs >= 0.0) {
        return None;
    }

    let audio_millis = (audio_secs * 1000.0).round().max(0.0) as u128;
    let micros = mul_div_round(rate_per_min, audio_millis, 60_000);
    Some(micros.min(u128::from(u64::MAX)) as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rates_exist_for_known_models() {
        assert!(stt_usd_micros_per_minute("nova-3").is_some());
        assert!(stt_usd_micros_per_minute("nova-2").is_some());
        assert!(stt_usd_micros_per_minute("enhanced").is_some());
        assert!(stt_usd_micros_per_minute("base").is_some());
    }
}
