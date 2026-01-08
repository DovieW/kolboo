//! Groq pricing tables and cost estimation helpers.
//!
//! Sources (fetched 2025-12-28):
//! - https://groq.com/pricing
//! - https://console.groq.com/docs/models
//! - https://console.groq.com/docs/speech-to-text
//!
//! Notes:
//! - Groq LLM pricing is listed as USD per 1,000,000 tokens (input/output).
//! - Groq ASR pricing is listed as USD per hour, with a minimum billed length of 10 seconds.
//! - Groq responses use an OpenAI-compatible schema; for chat completions we typically get
//!   `usage.prompt_tokens` and `usage.completion_tokens`.

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

/// Returns Groq LLM token rates for the given model.
///
/// Prices are per 1,000,000 tokens.
pub fn text_token_rates(model: &str) -> Option<TokenRates> {
    // Helper: $X.xx -> micros
    const fn usd_micros(dollars_times_1_000_000: u64) -> UsdMicros {
        dollars_times_1_000_000
    }

    let m = model.trim();

    Some(match m {
        // Meta Llama
        "llama-3.1-8b-instant" => TokenRates {
            input_usd_micros_per_1m: usd_micros(50_000),
            cached_input_usd_micros_per_1m: None,
            output_usd_micros_per_1m: usd_micros(80_000),
        },
        "llama-3.3-70b-versatile" => TokenRates {
            input_usd_micros_per_1m: usd_micros(590_000),
            cached_input_usd_micros_per_1m: None,
            output_usd_micros_per_1m: usd_micros(790_000),
        },

        // Meta Llama 4 (Preview)
        "meta-llama/llama-4-scout-17b-16e-instruct" => TokenRates {
            input_usd_micros_per_1m: usd_micros(110_000),
            cached_input_usd_micros_per_1m: None,
            output_usd_micros_per_1m: usd_micros(340_000),
        },
        "meta-llama/llama-4-maverick-17b-128e-instruct" => TokenRates {
            input_usd_micros_per_1m: usd_micros(200_000),
            cached_input_usd_micros_per_1m: None,
            output_usd_micros_per_1m: usd_micros(600_000),
        },

        // OpenAI GPT-OSS
        "openai/gpt-oss-120b" => TokenRates {
            input_usd_micros_per_1m: usd_micros(150_000),
            cached_input_usd_micros_per_1m: None,
            output_usd_micros_per_1m: usd_micros(600_000),
        },
        "openai/gpt-oss-20b" => TokenRates {
            input_usd_micros_per_1m: usd_micros(75_000),
            cached_input_usd_micros_per_1m: None,
            output_usd_micros_per_1m: usd_micros(300_000),
        },

        // Qwen (Preview)
        "qwen/qwen3-32b" => TokenRates {
            input_usd_micros_per_1m: usd_micros(290_000),
            cached_input_usd_micros_per_1m: None,
            output_usd_micros_per_1m: usd_micros(590_000),
        },

        // Moonshot (Preview)
        "moonshotai/kimi-k2-instruct-0905" => TokenRates {
            input_usd_micros_per_1m: usd_micros(1_000_000),
            cached_input_usd_micros_per_1m: None,
            output_usd_micros_per_1m: usd_micros(3_000_000),
        },

        _ => return None,
    })
}

/// Estimate Groq LLM cost based on OpenAI-compatible usage counts.
///
/// For Groq Chat Completions, we treat:
/// - `usage.input_tokens` as input tokens
/// - `usage.output_tokens` as output tokens
pub fn estimate_llm_cost_from_usage(model: &str, usage: OpenAiUsage) -> Option<UsdMicros> {
    let rates = text_token_rates(model)?;

    let input = usage.input_tokens;
    let output = usage.output_tokens;

    let input_micros = cost_from_tokens_micros(rates.input_usd_micros_per_1m, input);
    let output_micros = cost_from_tokens_micros(rates.output_usd_micros_per_1m, output);

    Some(input_micros.saturating_add(output_micros))
}

/// Returns Groq ASR pricing in USD micros per hour.
///
/// Models:
/// - whisper-large-v3: $0.111 / hour
/// - whisper-large-v3-turbo: $0.04 / hour
pub fn asr_usd_micros_per_hour(model: &str) -> Option<UsdMicros> {
    match model.trim() {
        "whisper-large-v3" => Some(111_000),
        "whisper-large-v3-turbo" => Some(40_000),
        _ => None,
    }
}

/// Estimate Groq ASR cost from audio duration in seconds.
///
/// Groq applies a minimum billed length of 10 seconds.
pub fn estimate_stt_cost_from_audio_secs(model: &str, audio_secs: f64) -> Option<UsdMicros> {
    let rate_per_hour = asr_usd_micros_per_hour(model)? as u128;

    if !(audio_secs.is_finite() && audio_secs >= 0.0) {
        return None;
    }

    // Billing granularity is not explicitly stated; we bill by milliseconds and apply
    // the minimum billed length of 10 seconds.
    let audio_millis = (audio_secs * 1000.0).round().max(0.0) as u128;
    let billed_millis = audio_millis.max(10_000);

    let micros = mul_div_round(rate_per_hour, billed_millis, 3_600_000);
    Some(micros.min(u128::from(u64::MAX)) as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rates_exist_for_known_models() {
        assert!(text_token_rates("llama-3.3-70b-versatile").is_some());
        assert!(asr_usd_micros_per_hour("whisper-large-v3-turbo").is_some());
    }

    #[test]
    fn stt_minimum_billing_is_applied() {
        // 1 second should bill as 10 seconds.
        let cost_1s = estimate_stt_cost_from_audio_secs("whisper-large-v3-turbo", 1.0).unwrap();
        let cost_10s = estimate_stt_cost_from_audio_secs("whisper-large-v3-turbo", 10.0).unwrap();
        assert_eq!(cost_1s, cost_10s);
    }
}
