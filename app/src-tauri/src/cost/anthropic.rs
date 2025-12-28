//! Anthropic (Claude API) pricing tables and cost estimation helpers.
//!
//! Source (fetched 2025-12-28):
//! - https://platform.claude.com/docs/en/about-claude/pricing
//!
//! Notes:
//! - Prices are represented in USD micros per 1,000,000 tokens.
//! - Prompt caching:
//!   - cache read tokens are 0.1x the base input token price
//!   - 5-minute cache write tokens are 1.25x the base input token price
//!   - 1-hour cache write tokens are 2.0x the base input token price
//! - Long context pricing:
//!   - For Claude Sonnet 4 / Sonnet 4.5 with the 1M context enabled, if total input
//!     tokens (input + cache read + cache write) exceeds 200k, *all* tokens are billed
//!     at premium rates.
//!   - The app does not control/know whether the 1M flag is enabled, but the pricing
//!     rule is deterministic from the returned `usage` object, so we apply it when we
//!     see `total_input_tokens > 200_000`.

use crate::cost::openai::{TokenRates, UsdMicros};

#[derive(Debug, Clone, Copy)]
pub struct AnthropicUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,

    /// Prompt caching: cache creation tokens (5-minute TTL).
    pub cache_creation_5m_input_tokens: u64,
    /// Prompt caching: cache creation tokens (1-hour TTL).
    pub cache_creation_1h_input_tokens: u64,
    /// Prompt caching: cache read tokens.
    pub cache_read_input_tokens: u64,
}

impl AnthropicUsage {
    pub fn cache_creation_input_tokens(&self) -> u64 {
        self.cache_creation_5m_input_tokens
            .saturating_add(self.cache_creation_1h_input_tokens)
    }

    pub fn total_input_tokens_for_tier(&self) -> u64 {
        self.input_tokens
            .saturating_add(self.cache_creation_input_tokens())
            .saturating_add(self.cache_read_input_tokens)
    }
}

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

fn normalize_model(model: &str) -> String {
    let m = model.trim();

    // Common patterns:
    // - "claude-sonnet-4-5" (UI)
    // - "claude-sonnet-4-5-20250929" (API)
    // - "claude-3-5-sonnet-latest" (UI)
    // - "claude-3-5-sonnet-20240620" (API)
    let base = m.strip_suffix("-latest").unwrap_or(m);

    // Strip a trailing date/version suffix (e.g. -20250929 or -20240620), if present.
    if let Some((prefix, suffix)) = base.rsplit_once('-') {
        if suffix.len() == 8 && suffix.chars().all(|c| c.is_ascii_digit()) {
            return prefix.to_string();
        }
    }

    base.to_string()
}

/// Returns Claude LLM **text** token rates for the given model.
///
/// This returns the baseline tier (<=200k total input tokens) for tiered Sonnet models.
pub fn text_token_rates(model: &str) -> Option<TokenRates> {
    text_token_rates_for_total_input_tokens(model, 0)
}

/// Returns Claude LLM **text** token rates for the given model, selecting the tier based
/// on total input tokens (including prompt caching read/write tokens).
pub fn text_token_rates_for_total_input_tokens(model: &str, total_input_tokens: u64) -> Option<TokenRates> {
    const fn usd_micros(dollars_times_1_000_000: u64) -> UsdMicros {
        dollars_times_1_000_000
    }

    let m = normalize_model(model);
    let long_input = total_input_tokens > 200_000;

    // Cache read tokens are always 0.1x base input rate per docs.
    fn cache_hit_rate(base_input_rate: UsdMicros) -> UsdMicros {
        // 0.1x
        mul_div_round(base_input_rate as u128, 1, 10) as u64
    }

    Some(match m.as_str() {
        // Claude 4.5 family
        "claude-sonnet-4-5" => {
            // Long-context tiering applies to Sonnet 4/4.5 when using the 1M context.
            // We apply the premium tier whenever total input tokens exceed 200k.
            let base_input = if long_input { usd_micros(6_000_000) } else { usd_micros(3_000_000) };
            let base_output = if long_input { usd_micros(22_500_000) } else { usd_micros(15_000_000) };
            TokenRates {
                input_usd_micros_per_1m: base_input,
                cached_input_usd_micros_per_1m: Some(cache_hit_rate(base_input)),
                output_usd_micros_per_1m: base_output,
            }
        }
        "claude-haiku-4-5" => TokenRates {
            input_usd_micros_per_1m: usd_micros(1_000_000),
            cached_input_usd_micros_per_1m: Some(usd_micros(100_000)),
            output_usd_micros_per_1m: usd_micros(5_000_000),
        },
        "claude-opus-4-5" => TokenRates {
            input_usd_micros_per_1m: usd_micros(5_000_000),
            cached_input_usd_micros_per_1m: Some(usd_micros(500_000)),
            output_usd_micros_per_1m: usd_micros(25_000_000),
        },

        // Claude 3.x families
        "claude-3-5-haiku" => TokenRates {
            input_usd_micros_per_1m: usd_micros(800_000),
            cached_input_usd_micros_per_1m: Some(usd_micros(80_000)),
            output_usd_micros_per_1m: usd_micros(4_000_000),
        },

        // The current docs table does not list a dedicated "Claude Sonnet 3.5" row.
        // Historically (and consistent with Sonnet 4 standard tier), Sonnet pricing is $3/MTok input,
        // $15/MTok output, with cache hits at 0.1x.
        "claude-3-5-sonnet" => TokenRates {
            input_usd_micros_per_1m: usd_micros(3_000_000),
            cached_input_usd_micros_per_1m: Some(usd_micros(300_000)),
            output_usd_micros_per_1m: usd_micros(15_000_000),
        },

        // Opus 3 (deprecated in docs, but still in app model list)
        "claude-3-opus" => TokenRates {
            input_usd_micros_per_1m: usd_micros(15_000_000),
            cached_input_usd_micros_per_1m: Some(usd_micros(1_500_000)),
            output_usd_micros_per_1m: usd_micros(75_000_000),
        },

        // Some API callers may send Sonnet 4 without the "-latest" alias.
        "claude-sonnet-4" => {
            let base_input = if long_input { usd_micros(6_000_000) } else { usd_micros(3_000_000) };
            let base_output = if long_input { usd_micros(22_500_000) } else { usd_micros(15_000_000) };
            TokenRates {
                input_usd_micros_per_1m: base_input,
                cached_input_usd_micros_per_1m: Some(cache_hit_rate(base_input)),
                output_usd_micros_per_1m: base_output,
            }
        }

        _ => return None,
    })
}

/// Estimates a Claude API request cost (including prompt caching tokens when present).
///
/// The `usage` object can report:
/// - input_tokens
/// - output_tokens
/// - cache_creation_input_tokens (optionally further split into 5m/1h)
/// - cache_read_input_tokens
pub fn estimate_llm_cost_from_usage(model: &str, usage: AnthropicUsage) -> Option<UsdMicros> {
    let total_input = usage.total_input_tokens_for_tier();
    let rates = text_token_rates_for_total_input_tokens(model, total_input)?;

    // Base input tokens.
    let input_micros = cost_from_tokens_micros(rates.input_usd_micros_per_1m, usage.input_tokens);

    // Cache hits (read) are represented by `cached_input_usd_micros_per_1m`.
    let cache_read_rate = rates.cached_input_usd_micros_per_1m.unwrap_or(0);
    let cache_read_micros = cost_from_tokens_micros(cache_read_rate, usage.cache_read_input_tokens);

    // Cache writes use multipliers on the base input rate.
    let cache_write_5m_rate = mul_div_round(rates.input_usd_micros_per_1m as u128, 125, 100) as u64;
    let cache_write_1h_rate = mul_div_round(rates.input_usd_micros_per_1m as u128, 2, 1) as u64;

    let cache_write_5m_micros =
        cost_from_tokens_micros(cache_write_5m_rate, usage.cache_creation_5m_input_tokens);
    let cache_write_1h_micros =
        cost_from_tokens_micros(cache_write_1h_rate, usage.cache_creation_1h_input_tokens);

    let output_micros =
        cost_from_tokens_micros(rates.output_usd_micros_per_1m, usage.output_tokens);

    Some(
        input_micros
            .saturating_add(cache_read_micros)
            .saturating_add(cache_write_5m_micros)
            .saturating_add(cache_write_1h_micros)
            .saturating_add(output_micros),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rates_exist_for_app_models() {
        assert!(text_token_rates("claude-sonnet-4-5").is_some());
        assert!(text_token_rates("claude-haiku-4-5").is_some());
        assert!(text_token_rates("claude-opus-4-5").is_some());
        assert!(text_token_rates("claude-3-5-haiku-latest").is_some());
        assert!(text_token_rates("claude-3-5-sonnet-latest").is_some());
        assert!(text_token_rates("claude-3-opus-latest").is_some());
    }

    #[test]
    fn sonnet_45_switches_to_premium_after_200k_total_input() {
        let short = text_token_rates_for_total_input_tokens("claude-sonnet-4-5", 200_000).unwrap();
        let long = text_token_rates_for_total_input_tokens("claude-sonnet-4-5", 200_001).unwrap();
        assert!(long.input_usd_micros_per_1m > short.input_usd_micros_per_1m);
        assert!(long.output_usd_micros_per_1m > short.output_usd_micros_per_1m);
    }

    #[test]
    fn estimator_counts_cache_tokens() {
        let usage = AnthropicUsage {
            input_tokens: 1_000,
            output_tokens: 500,
            cache_creation_5m_input_tokens: 100,
            cache_creation_1h_input_tokens: 0,
            cache_read_input_tokens: 200,
        };

        let cost = estimate_llm_cost_from_usage("claude-haiku-4-5", usage).unwrap();
        assert!(cost > 0);
    }
}
