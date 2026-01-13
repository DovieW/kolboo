//! Google Gemini (Gemini Developer API / AI Studio) pricing tables and cost estimation helpers.
//!
//! Sources (fetched 2025-12-28):
//! - https://ai.google.dev/gemini-api/docs/pricing
//!
//! Notes:
//! - Prices for text models are listed as USD per 1,000,000 tokens (input/output).
//! - Some models have a higher price tier when prompt length exceeds 200k tokens.
//!   We approximate this by checking `usage.input_tokens`.
//! - The app currently uses Gemini primarily for text rewrite, so we only model text token pricing.

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

fn normalize_model(model: &str) -> &str {
    let m = model.trim();
    m.strip_prefix("models/").unwrap_or(m)
}

/// Returns Gemini LLM **text** token rates for the given model.
///
/// This function returns the baseline price tier ("prompts <= 200k tokens") when
/// the pricing table has multiple prompt-length tiers.
pub fn text_token_rates(model: &str) -> Option<TokenRates> {
    text_token_rates_for_prompt_tokens(model, 0)
}

/// Returns Gemini LLM **text** token rates for the given model, selecting the
/// appropriate tier based on prompt token count.
///
/// For models with two tiers, we use the higher tier when `prompt_tokens > 200_000`.
pub fn text_token_rates_for_prompt_tokens(model: &str, prompt_tokens: u64) -> Option<TokenRates> {
    // Helper: $X.xx -> micros
    const fn usd_micros(dollars_times_1_000_000: u64) -> UsdMicros {
        dollars_times_1_000_000
    }

    let m = normalize_model(model);
    let long_prompt = prompt_tokens > 200_000;

    Some(match m {
        // Gemini 2.5 (stable)
        "gemini-2.5-pro" => {
            if long_prompt {
                TokenRates {
                    input_usd_micros_per_1m: usd_micros(2_500_000),
                    cached_input_usd_micros_per_1m: None,
                    output_usd_micros_per_1m: usd_micros(15_000_000),
                }
            } else {
                TokenRates {
                    input_usd_micros_per_1m: usd_micros(1_250_000),
                    cached_input_usd_micros_per_1m: None,
                    output_usd_micros_per_1m: usd_micros(10_000_000),
                }
            }
        }
        "gemini-2.5-flash" => TokenRates {
            // Text/image/video input pricing.
            input_usd_micros_per_1m: usd_micros(300_000),
            cached_input_usd_micros_per_1m: None,
            output_usd_micros_per_1m: usd_micros(2_500_000),
        },
        "gemini-2.5-flash-lite" => TokenRates {
            // Text/image/video input pricing.
            input_usd_micros_per_1m: usd_micros(100_000),
            cached_input_usd_micros_per_1m: None,
            output_usd_micros_per_1m: usd_micros(400_000),
        },

        // Gemini 3 (preview)
        "gemini-3-pro-preview" => {
            if long_prompt {
                TokenRates {
                    input_usd_micros_per_1m: usd_micros(4_000_000),
                    cached_input_usd_micros_per_1m: None,
                    output_usd_micros_per_1m: usd_micros(18_000_000),
                }
            } else {
                TokenRates {
                    input_usd_micros_per_1m: usd_micros(2_000_000),
                    cached_input_usd_micros_per_1m: None,
                    output_usd_micros_per_1m: usd_micros(12_000_000),
                }
            }
        }
        "gemini-3-flash-preview" => TokenRates {
            // Text/image/video input pricing.
            input_usd_micros_per_1m: usd_micros(500_000),
            cached_input_usd_micros_per_1m: None,
            output_usd_micros_per_1m: usd_micros(3_000_000),
        },

        _ => return None,
    })
}

/// Estimate Gemini LLM cost based on token usage.
///
/// Gemini `usageMetadata` reports prompt + candidates tokens. We map those into
/// `OpenAiUsage` and compute:
/// - input cost from `usage.input_tokens`
/// - output cost from `usage.output_tokens`
pub fn estimate_llm_cost_from_usage(model: &str, usage: OpenAiUsage) -> Option<UsdMicros> {
    let rates = text_token_rates_for_prompt_tokens(model, usage.input_tokens)?;

    let input_micros = cost_from_tokens_micros(rates.input_usd_micros_per_1m, usage.input_tokens);
    let output_micros =
        cost_from_tokens_micros(rates.output_usd_micros_per_1m, usage.output_tokens);

    Some(input_micros.saturating_add(output_micros))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rates_exist_for_known_models() {
        assert!(text_token_rates("gemini-2.5-flash").is_some());
        assert!(text_token_rates("models/gemini-3-flash-preview").is_some());
    }

    #[test]
    fn tiered_rates_switch_after_200k_prompt_tokens() {
        let short = text_token_rates_for_prompt_tokens("gemini-2.5-pro", 200_000).unwrap();
        let long = text_token_rates_for_prompt_tokens("gemini-2.5-pro", 200_001).unwrap();
        assert!(long.input_usd_micros_per_1m > short.input_usd_micros_per_1m);
        assert!(long.output_usd_micros_per_1m > short.output_usd_micros_per_1m);
    }
}
