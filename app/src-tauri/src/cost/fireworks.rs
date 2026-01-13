//! Fireworks pricing tables and cost estimation helpers.
//!
//! Source: https://fireworks.ai/pricing (fetched 2026-01-08)
//!
//! Notes:
//! - Fireworks' Serverless Pricing for "Text and Vision" includes:
//!   - tiered single-rate pricing by parameter count (applies to both input + output tokens)
//!   - a handful of special-case families with explicit input/output rates
//!   - cached input tokens are billed at 50% of the input token rate
//! - Speech-to-text is priced per second of audio input.

use crate::cost::openai::{OpenAiUsage, TokenRates, UsdMicros};

fn mul_div_round(n: u128, mul: u128, div: u128) -> u128 {
    if div == 0 {
        return 0;
    }
    // Round half up.
    (n.saturating_mul(mul).saturating_add(div / 2)) / div
}

fn cost_from_tokens_micros(rate_per_1m: UsdMicros, tokens: u64) -> UsdMicros {
    let micros = mul_div_round(rate_per_1m as u128, tokens as u128, 1_000_000);
    micros.min(u128::from(u64::MAX)) as u64
}

fn normalize_model_id(model: &str) -> String {
    let m = model.trim();
    if let Some(rest) = m.strip_prefix("accounts/fireworks/models/") {
        return rest.to_string();
    }
    if let Some(rest) = m.strip_prefix("fireworks/") {
        return rest.to_string();
    }
    m.to_string()
}

fn parse_params_b_from_id(model_id: &str) -> Option<f64> {
    // Extract tokens like "8b", "70b", "405b" from the model id.
    // We keep this intentionally conservative.
    for part in model_id.split(|c: char| c == '-' || c == '_' || c == '/' || c == '.') {
        let p = part.trim();
        if p.len() < 2 || !p.ends_with('b') {
            continue;
        }
        let num = &p[..p.len() - 1];
        if let Ok(n) = num.parse::<f64>() {
            return Some(n);
        }
    }
    None
}

fn tier_rate_usd_micros_per_1m_by_params(params_b: f64) -> Option<UsdMicros> {
    // From pricing page:
    // - <4B: $0.10 / 1M tokens
    // - 4B-16B: $0.20 / 1M tokens
    // - >16B: $0.90 / 1M tokens
    if !(params_b.is_finite() && params_b > 0.0) {
        return None;
    }

    if params_b < 4.0 {
        Some(100_000)
    } else if params_b <= 16.0 {
        Some(200_000)
    } else {
        Some(900_000)
    }
}

/// Returns Fireworks LLM token rates for the given model.
///
/// Prices are per 1,000,000 tokens.
pub fn text_token_rates(model: &str) -> Option<TokenRates> {
    let model_id = normalize_model_id(model);
    let m = model_id.as_str();

    // Special-case families with explicit input/output rates.
    // We key off substrings because Fireworks model ids vary widely.
    let special = if m.contains("deepseek") && m.contains("v3") {
        Some((560_000u64, 1_680_000u64))
    } else if m.contains("deepseek") && m.contains("r1") {
        // "DeepSeek R1 0528" on the pricing page.
        Some((1_350_000u64, 5_400_000u64))
    } else if m.contains("deepseek") && m.contains("reason") {
        // "DeepSeek R1 0528" on the pricing page.
        Some((1_350_000u64, 5_400_000u64))
    } else if m.contains("glm-4.5") || m.contains("glm-4.6") {
        Some((550_000u64, 2_190_000u64))
    } else if m.contains("glm-4.7") {
        Some((600_000u64, 2_200_000u64))
    } else if m.contains("qwen3") && m.contains("235b") {
        Some((220_000u64, 880_000u64))
    } else if m.contains("qwen3") && m.contains("vl") && m.contains("30b") {
        Some((150_000u64, 600_000u64))
    } else if m.contains("kimi") && m.contains("k2") {
        Some((600_000u64, 2_500_000u64))
    } else if m.contains("qwen3") && m.contains("coder") && m.contains("480b") {
        Some((450_000u64, 1_800_000u64))
    } else if m.contains("gpt-oss-120b") {
        Some((150_000u64, 600_000u64))
    } else if m.contains("gpt-oss-20b") {
        Some((70_000u64, 300_000u64))
    } else if m.contains("minimax") && m.contains("m2.1") {
        Some((300_000u64, 1_200_000u64))
    } else if m.contains("minimax") && m.contains("m2") {
        Some((300_000u64, 1_200_000u64))
    } else {
        None
    };

    if let Some((input, output)) = special {
        return Some(TokenRates {
            input_usd_micros_per_1m: input,
            cached_input_usd_micros_per_1m: Some(input / 2),
            output_usd_micros_per_1m: output,
        });
    }

    // Generic tiered rate: a single rate applies to both input and output.
    let params_b = parse_params_b_from_id(m)?;
    let rate = tier_rate_usd_micros_per_1m_by_params(params_b)?;

    Some(TokenRates {
        input_usd_micros_per_1m: rate,
        cached_input_usd_micros_per_1m: Some(rate / 2),
        output_usd_micros_per_1m: rate,
    })
}

/// Estimate Fireworks LLM cost based on OpenAI-compatible usage counts.
pub fn estimate_llm_cost_from_usage(model: &str, usage: OpenAiUsage) -> Option<UsdMicros> {
    let rates = text_token_rates(model)?;

    let input = usage.input_tokens;
    let output = usage.output_tokens;
    let cached_input = usage.cached_input_tokens;

    let input_billable = input.saturating_sub(cached_input);
    let input_micros = cost_from_tokens_micros(rates.input_usd_micros_per_1m, input_billable);
    let cached_input_micros = rates
        .cached_input_usd_micros_per_1m
        .map(|r| cost_from_tokens_micros(r, cached_input))
        .unwrap_or(0);
    let output_micros = cost_from_tokens_micros(rates.output_usd_micros_per_1m, output);

    Some(
        input_micros
            .saturating_add(cached_input_micros)
            .saturating_add(output_micros),
    )
}

/// Returns Fireworks STT pricing in USD micros per minute.
///
/// Fireworks' STT API uses model ids `whisper-v3` and `whisper-v3-turbo`.
///
/// Pricing source: https://fireworks.ai/pricing ("Speech to Text (STT)").
///
/// NOTE: The pricing table labels STT as "Pay per second", but the numeric
/// values are listed as:
/// - Whisper-v3-large: $0.0015
/// - Whisper-v3-large-turbo: $0.0009
///
/// In-app STT pricing is shown as per-minute (to match other providers), and
/// treating these values as per-second produces unexpectedly high costs.
/// We therefore interpret these figures as per-minute list prices and derive
/// per-second rates from them for per-call estimation.
pub fn stt_usd_micros_per_minute(model: &str) -> Option<UsdMicros> {
    let model_id = normalize_model_id(model);
    match model_id.trim() {
        // $0.0015 per minute
        "whisper-v3" | "whisper-v3-large" => Some(1_500),
        // $0.0009 per minute
        "whisper-v3-turbo" | "whisper-v3-large-turbo" => Some(900),
        _ => None,
    }
}

pub fn stt_usd_micros_per_second(model: &str) -> Option<UsdMicros> {
    stt_usd_micros_per_minute(model).map(|per_min| per_min / 60)
}

/// Estimate Fireworks STT cost from audio duration in seconds.
pub fn estimate_stt_cost_from_audio_secs(model: &str, audio_secs: f64) -> Option<UsdMicros> {
    let rate_per_sec = stt_usd_micros_per_second(model)? as u128;

    if !(audio_secs.is_finite() && audio_secs >= 0.0) {
        return None;
    }

    // Bill by milliseconds (best-effort). Pricing is per second.
    let audio_millis = (audio_secs * 1000.0).round().max(0.0) as u128;
    let micros = mul_div_round(rate_per_sec, audio_millis, 1000);
    Some(micros.min(u128::from(u64::MAX)) as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rates_exist_for_llama_suffixes() {
        let r = text_token_rates("accounts/fireworks/models/llama-v3p1-8b-instruct").unwrap();
        assert_eq!(r.input_usd_micros_per_1m, 200_000);
        assert_eq!(r.output_usd_micros_per_1m, 200_000);
        assert_eq!(r.cached_input_usd_micros_per_1m, Some(100_000));
    }

    #[test]
    fn stt_rates_exist_for_whisper_v3() {
        assert!(stt_usd_micros_per_second("whisper-v3").is_some());
        assert!(stt_usd_micros_per_second("whisper-v3-turbo").is_some());
    }
}
