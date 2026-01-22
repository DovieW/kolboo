//! Shared math helpers for cost estimation.
//!
//! These helpers are intentionally small and provider-agnostic.

/// Multiply then divide with "round half up" behavior.
///
/// Returns 0 when `div == 0`.
pub fn mul_div_round_u128(n: u128, mul: u128, div: u128) -> u128 {
    if div == 0 {
        return 0;
    }
    // Round half up.
    (n.saturating_mul(mul).saturating_add(div / 2)) / div
}

/// Convert token counts to USD micros, given a USD-micros-per-1M-tokens rate.
///
/// - `rate_per_1m`: price per 1,000,000 tokens, in USD micros.
/// - `tokens`: token count.
pub fn cost_from_tokens_micros(rate_per_1m: u64, tokens: u64) -> u64 {
    let micros = mul_div_round_u128(rate_per_1m as u128, tokens as u128, 1_000_000);
    micros.min(u128::from(u64::MAX)) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mul_div_round_rounds_half_up() {
        assert_eq!(mul_div_round_u128(10, 1, 2), 5);
        // 1/2 rounds up.
        assert_eq!(mul_div_round_u128(1, 1, 2), 1);
        assert_eq!(mul_div_round_u128(0, 999, 1), 0);
    }

    #[test]
    fn mul_div_round_div_zero_is_zero() {
        assert_eq!(mul_div_round_u128(123, 456, 0), 0);
    }

    #[test]
    fn cost_from_tokens_micros_basic_cases() {
        // $1.00 per 1M tokens; 1M tokens => $1.00
        assert_eq!(cost_from_tokens_micros(1_000_000, 1_000_000), 1_000_000);
        // $0.000001 per 1M tokens; 1M tokens => 1 micro
        assert_eq!(cost_from_tokens_micros(1, 1_000_000), 1);
        // 0 tokens => 0 cost
        assert_eq!(cost_from_tokens_micros(123_456, 0), 0);
    }
}
