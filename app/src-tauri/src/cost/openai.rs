//! OpenAI pricing tables and cost estimation helpers.
//!
//! Source: https://platform.openai.com/docs/pricing (fetched 2025-12-26)

use serde::{Deserialize, Serialize};

/// USD microdollars (1 USD = 1_000_000 micros).
pub type UsdMicros = u64;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TokenRates {
    /// Price per 1,000,000 **text input** tokens.
    pub input_usd_micros_per_1m: UsdMicros,
    /// Price per 1,000,000 **cached text input** tokens.
    pub cached_input_usd_micros_per_1m: Option<UsdMicros>,
    /// Price per 1,000,000 **text output** tokens.
    pub output_usd_micros_per_1m: UsdMicros,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct AudioTokenRates {
    /// Price per 1,000,000 **audio input** tokens.
    pub input_usd_micros_per_1m: UsdMicros,
    /// Price per 1,000,000 **cached audio input** tokens.
    pub cached_input_usd_micros_per_1m: Option<UsdMicros>,
    /// Price per 1,000,000 **audio output** tokens.
    pub output_usd_micros_per_1m: UsdMicros,
}

/// Returns standard processing text token rates for the given model.
///
/// Notes:
/// - This is a best-effort mapping. If a model isn't found, return `None`.
/// - We treat "standard" rates as the baseline for cost estimation.
pub fn text_token_rates(model: &str) -> Option<TokenRates> {
    // Helper: $X.xx -> micros
    const fn usd_micros(dollars_times_1_000_000: u64) -> UsdMicros {
        dollars_times_1_000_000
    }

    let m = model.trim();

    // IMPORTANT: Keep this list in sync with the pricing source.
    // Prices per 1M tokens.
    Some(match m {
        // GPT-5 family
        "gpt-5.2" | "gpt-5.2-chat-latest" => TokenRates {
            input_usd_micros_per_1m: usd_micros(1_750_000),
            cached_input_usd_micros_per_1m: Some(usd_micros(175_000)),
            output_usd_micros_per_1m: usd_micros(14_000_000),
        },
        "gpt-5.1" | "gpt-5.1-chat-latest" => TokenRates {
            input_usd_micros_per_1m: usd_micros(1_250_000),
            cached_input_usd_micros_per_1m: Some(usd_micros(125_000)),
            output_usd_micros_per_1m: usd_micros(10_000_000),
        },
        "gpt-5" | "gpt-5-chat-latest" => TokenRates {
            input_usd_micros_per_1m: usd_micros(1_250_000),
            cached_input_usd_micros_per_1m: Some(usd_micros(125_000)),
            output_usd_micros_per_1m: usd_micros(10_000_000),
        },
        "gpt-5-mini" => TokenRates {
            input_usd_micros_per_1m: usd_micros(250_000),
            cached_input_usd_micros_per_1m: Some(usd_micros(25_000)),
            output_usd_micros_per_1m: usd_micros(2_000_000),
        },
        "gpt-5-nano" => TokenRates {
            input_usd_micros_per_1m: usd_micros(50_000),
            cached_input_usd_micros_per_1m: Some(usd_micros(5_000)),
            output_usd_micros_per_1m: usd_micros(400_000),
        },

        // GPT-5 pro variants
        "gpt-5.2-pro" => TokenRates {
            input_usd_micros_per_1m: usd_micros(21_000_000),
            cached_input_usd_micros_per_1m: None,
            output_usd_micros_per_1m: usd_micros(168_000_000),
        },
        "gpt-5-pro" => TokenRates {
            input_usd_micros_per_1m: usd_micros(15_000_000),
            cached_input_usd_micros_per_1m: None,
            output_usd_micros_per_1m: usd_micros(120_000_000),
        },

        // GPT-4.1 family
        "gpt-4.1" => TokenRates {
            input_usd_micros_per_1m: usd_micros(2_000_000),
            cached_input_usd_micros_per_1m: Some(usd_micros(500_000)),
            output_usd_micros_per_1m: usd_micros(8_000_000),
        },
        "gpt-4.1-mini" => TokenRates {
            input_usd_micros_per_1m: usd_micros(400_000),
            cached_input_usd_micros_per_1m: Some(usd_micros(100_000)),
            output_usd_micros_per_1m: usd_micros(1_600_000),
        },
        "gpt-4.1-nano" => TokenRates {
            input_usd_micros_per_1m: usd_micros(100_000),
            cached_input_usd_micros_per_1m: Some(usd_micros(25_000)),
            output_usd_micros_per_1m: usd_micros(400_000),
        },

        // GPT-4o family
        "gpt-4o" | "gpt-audio" | "gpt-4o-audio-preview" => TokenRates {
            input_usd_micros_per_1m: usd_micros(2_500_000),
            cached_input_usd_micros_per_1m: Some(usd_micros(1_250_000)),
            output_usd_micros_per_1m: usd_micros(10_000_000),
        },
        "gpt-4o-mini" | "gpt-audio-mini" | "gpt-4o-mini-audio-preview" => TokenRates {
            input_usd_micros_per_1m: usd_micros(150_000),
            cached_input_usd_micros_per_1m: Some(usd_micros(75_000)),
            output_usd_micros_per_1m: usd_micros(600_000),
        },

        // Older, dated GPT-4o model (no cached pricing listed on docs page snippet)
        "gpt-4o-2024-05-13" => TokenRates {
            input_usd_micros_per_1m: usd_micros(5_000_000),
            cached_input_usd_micros_per_1m: None,
            output_usd_micros_per_1m: usd_micros(15_000_000),
        },

        // Realtime
        "gpt-realtime" | "gpt-4o-realtime-preview" => TokenRates {
            input_usd_micros_per_1m: usd_micros(4_000_000),
            cached_input_usd_micros_per_1m: Some(usd_micros(400_000)),
            output_usd_micros_per_1m: usd_micros(16_000_000),
        },
        "gpt-realtime-mini" | "gpt-4o-mini-realtime-preview" => TokenRates {
            input_usd_micros_per_1m: usd_micros(600_000),
            cached_input_usd_micros_per_1m: Some(usd_micros(60_000)),
            output_usd_micros_per_1m: usd_micros(2_400_000),
        },

        // o-series (reasoning)
        "o1" => TokenRates {
            input_usd_micros_per_1m: usd_micros(15_000_000),
            cached_input_usd_micros_per_1m: Some(usd_micros(7_500_000)),
            output_usd_micros_per_1m: usd_micros(60_000_000),
        },
        "o1-pro" => TokenRates {
            input_usd_micros_per_1m: usd_micros(150_000_000),
            cached_input_usd_micros_per_1m: None,
            output_usd_micros_per_1m: usd_micros(600_000_000),
        },
        "o3" => TokenRates {
            input_usd_micros_per_1m: usd_micros(2_000_000),
            cached_input_usd_micros_per_1m: Some(usd_micros(500_000)),
            output_usd_micros_per_1m: usd_micros(8_000_000),
        },
        "o3-pro" => TokenRates {
            input_usd_micros_per_1m: usd_micros(20_000_000),
            cached_input_usd_micros_per_1m: None,
            output_usd_micros_per_1m: usd_micros(80_000_000),
        },
        "o3-deep-research" => TokenRates {
            input_usd_micros_per_1m: usd_micros(10_000_000),
            cached_input_usd_micros_per_1m: Some(usd_micros(2_500_000)),
            output_usd_micros_per_1m: usd_micros(40_000_000),
        },
        "o4-mini" => TokenRates {
            input_usd_micros_per_1m: usd_micros(1_100_000),
            cached_input_usd_micros_per_1m: Some(usd_micros(275_000)),
            output_usd_micros_per_1m: usd_micros(4_400_000),
        },
        "o4-mini-deep-research" => TokenRates {
            input_usd_micros_per_1m: usd_micros(2_000_000),
            cached_input_usd_micros_per_1m: Some(usd_micros(500_000)),
            output_usd_micros_per_1m: usd_micros(8_000_000),
        },
        "o3-mini" | "o1-mini" => TokenRates {
            input_usd_micros_per_1m: usd_micros(1_100_000),
            cached_input_usd_micros_per_1m: Some(usd_micros(550_000)),
            output_usd_micros_per_1m: usd_micros(4_400_000),
        },

        // Legacy models (batch+standard; cached not listed)
        "chatgpt-4o-latest" => TokenRates {
            input_usd_micros_per_1m: usd_micros(5_000_000),
            cached_input_usd_micros_per_1m: None,
            output_usd_micros_per_1m: usd_micros(15_000_000),
        },
        "gpt-4-turbo-2024-04-09" | "gpt-4-0125-preview" | "gpt-4-1106-preview" | "gpt-4-1106-vision-preview" => TokenRates {
            input_usd_micros_per_1m: usd_micros(10_000_000),
            cached_input_usd_micros_per_1m: None,
            output_usd_micros_per_1m: usd_micros(30_000_000),
        },
        "gpt-4-0613" | "gpt-4-0314" => TokenRates {
            input_usd_micros_per_1m: usd_micros(30_000_000),
            cached_input_usd_micros_per_1m: None,
            output_usd_micros_per_1m: usd_micros(60_000_000),
        },
        "gpt-4-32k" => TokenRates {
            input_usd_micros_per_1m: usd_micros(60_000_000),
            cached_input_usd_micros_per_1m: None,
            output_usd_micros_per_1m: usd_micros(120_000_000),
        },
        "gpt-3.5-turbo" | "gpt-3.5-turbo-0125" => TokenRates {
            input_usd_micros_per_1m: usd_micros(500_000),
            cached_input_usd_micros_per_1m: None,
            output_usd_micros_per_1m: usd_micros(1_500_000),
        },
        "gpt-3.5-turbo-1106" => TokenRates {
            input_usd_micros_per_1m: usd_micros(1_000_000),
            cached_input_usd_micros_per_1m: None,
            output_usd_micros_per_1m: usd_micros(2_000_000),
        },
        "gpt-3.5-turbo-0613" | "gpt-3.5-0301" | "gpt-3.5-turbo-instruct" => TokenRates {
            input_usd_micros_per_1m: usd_micros(1_500_000),
            cached_input_usd_micros_per_1m: None,
            output_usd_micros_per_1m: usd_micros(2_000_000),
        },
        "gpt-3.5-turbo-16k-0613" => TokenRates {
            input_usd_micros_per_1m: usd_micros(3_000_000),
            cached_input_usd_micros_per_1m: None,
            output_usd_micros_per_1m: usd_micros(4_000_000),
        },
        "davinci-002" => TokenRates {
            input_usd_micros_per_1m: usd_micros(2_000_000),
            cached_input_usd_micros_per_1m: None,
            output_usd_micros_per_1m: usd_micros(2_000_000),
        },
        "babbage-002" => TokenRates {
            input_usd_micros_per_1m: usd_micros(400_000),
            cached_input_usd_micros_per_1m: None,
            output_usd_micros_per_1m: usd_micros(400_000),
        },

        _ => return None,
    })
}

/// Returns standard processing audio token rates for the given model.
pub fn audio_token_rates(model: &str) -> Option<AudioTokenRates> {
    const fn usd_micros(dollars_times_1_000_000: u64) -> UsdMicros {
        dollars_times_1_000_000
    }

    let m = model.trim();

    Some(match m {
        "gpt-realtime" => AudioTokenRates {
            input_usd_micros_per_1m: usd_micros(32_000_000),
            cached_input_usd_micros_per_1m: Some(usd_micros(400_000)),
            output_usd_micros_per_1m: usd_micros(64_000_000),
        },
        "gpt-realtime-mini" => AudioTokenRates {
            input_usd_micros_per_1m: usd_micros(10_000_000),
            cached_input_usd_micros_per_1m: Some(usd_micros(300_000)),
            output_usd_micros_per_1m: usd_micros(20_000_000),
        },
        "gpt-4o-realtime-preview" => AudioTokenRates {
            input_usd_micros_per_1m: usd_micros(40_000_000),
            cached_input_usd_micros_per_1m: Some(usd_micros(2_500_000)),
            output_usd_micros_per_1m: usd_micros(80_000_000),
        },
        "gpt-4o-mini-realtime-preview" => AudioTokenRates {
            input_usd_micros_per_1m: usd_micros(10_000_000),
            cached_input_usd_micros_per_1m: Some(usd_micros(300_000)),
            output_usd_micros_per_1m: usd_micros(20_000_000),
        },
        "gpt-audio" => AudioTokenRates {
            input_usd_micros_per_1m: usd_micros(32_000_000),
            cached_input_usd_micros_per_1m: None,
            output_usd_micros_per_1m: usd_micros(64_000_000),
        },
        "gpt-audio-mini" => AudioTokenRates {
            input_usd_micros_per_1m: usd_micros(10_000_000),
            cached_input_usd_micros_per_1m: None,
            output_usd_micros_per_1m: usd_micros(20_000_000),
        },
        "gpt-4o-audio-preview" => AudioTokenRates {
            input_usd_micros_per_1m: usd_micros(40_000_000),
            cached_input_usd_micros_per_1m: None,
            output_usd_micros_per_1m: usd_micros(80_000_000),
        },
        "gpt-4o-mini-audio-preview" => AudioTokenRates {
            input_usd_micros_per_1m: usd_micros(10_000_000),
            cached_input_usd_micros_per_1m: None,
            output_usd_micros_per_1m: usd_micros(20_000_000),
        },
        _ => return None,
    })
}

/// Returns transcription pricing in USD micros per minute.
///
/// Models:
/// - Whisper / whisper-1: $0.006 / minute
/// - gpt-4o-transcribe (+ diarize): $0.006 / minute
/// - gpt-4o-mini-transcribe: $0.003 / minute
pub fn transcription_usd_micros_per_minute(model: &str) -> Option<UsdMicros> {
    match model.trim() {
        "whisper" | "whisper-1" => Some(6_000),
        "gpt-4o-transcribe" | "gpt-4o-transcribe-diarize" => Some(6_000),
        "gpt-4o-mini-transcribe" => Some(3_000),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct OpenAiUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cached_input_tokens: u64,
    pub input_audio_tokens: u64,
    pub output_audio_tokens: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct OpenAiCostBreakdown {
    pub total_usd_micros: UsdMicros,
    pub text_input_usd_micros: UsdMicros,
    pub text_cached_input_usd_micros: UsdMicros,
    pub text_output_usd_micros: UsdMicros,
    pub audio_input_usd_micros: UsdMicros,
    pub audio_output_usd_micros: UsdMicros,
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

pub fn estimate_cost_from_usage(model: &str, usage: OpenAiUsage) -> Option<OpenAiCostBreakdown> {
    let text_rates = text_token_rates(model)?;

    let cached = usage.cached_input_tokens;
    let input_audio = usage.input_audio_tokens;
    let output_audio = usage.output_audio_tokens;

    // OpenAI reports totals plus per-category details. We compute text tokens as the residual.
    let input_text = usage
        .input_tokens
        .saturating_sub(cached)
        .saturating_sub(input_audio);
    let output_text = usage.output_tokens.saturating_sub(output_audio);

    let text_input_usd_micros = cost_from_tokens_micros(text_rates.input_usd_micros_per_1m, input_text);
    let text_output_usd_micros = cost_from_tokens_micros(text_rates.output_usd_micros_per_1m, output_text);

    let text_cached_input_usd_micros = match text_rates.cached_input_usd_micros_per_1m {
        Some(rate) => cost_from_tokens_micros(rate, cached),
        None => 0,
    };

    let (audio_input_usd_micros, audio_output_usd_micros) = match audio_token_rates(model) {
        Some(audio_rates) => (
            cost_from_tokens_micros(audio_rates.input_usd_micros_per_1m, input_audio),
            cost_from_tokens_micros(audio_rates.output_usd_micros_per_1m, output_audio),
        ),
        None => (0, 0),
    };

    let total_usd_micros = text_input_usd_micros
        .saturating_add(text_cached_input_usd_micros)
        .saturating_add(text_output_usd_micros)
        .saturating_add(audio_input_usd_micros)
        .saturating_add(audio_output_usd_micros);

    Some(OpenAiCostBreakdown {
        total_usd_micros,
        text_input_usd_micros,
        text_cached_input_usd_micros,
        text_output_usd_micros,
        audio_input_usd_micros,
        audio_output_usd_micros,
    })
}

pub fn estimate_transcription_cost_from_audio_secs(
    model: &str,
    audio_secs: f64,
) -> Option<UsdMicros> {
    let rate_per_min = transcription_usd_micros_per_minute(model)? as u128;
    if !(audio_secs.is_finite() && audio_secs >= 0.0) {
        return None;
    }

    let audio_millis = (audio_secs * 1000.0).round().max(0.0) as u128;
    let micros = mul_div_round(rate_per_min, audio_millis, 60_000);
    Some(micros.min(u128::from(u64::MAX)) as u64)
}
