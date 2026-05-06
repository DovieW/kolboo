//! Cost Reporting assembly.
//!
//! Provider pricing tables and formulas stay in their provider-specific Modules.
//! This Module owns the common "turn request telemetry into a cost report" shape
//! so callers do not need a long provider-specific match every time they emit a
//! stats event.

use serde_json::Value as JsonValue;

use crate::cost::anthropic;
use crate::cost::aquavoice;
use crate::cost::assemblyai;
use crate::cost::deepgram;
use crate::cost::fireworks;
use crate::cost::gemini;
use crate::cost::groq;
use crate::cost::openai;
use crate::cost::speechmatics;

/// Provider-neutral token counters used by Stats cost events.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CostTokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cached_input_tokens: u64,
    pub input_audio_tokens: u64,
    pub output_audio_tokens: u64,
}

impl CostTokenUsage {
    fn from_openai_usage(usage: openai::OpenAiUsage) -> Self {
        Self {
            input_tokens: usage.input_tokens,
            output_tokens: usage.output_tokens,
            cached_input_tokens: usage.cached_input_tokens,
            input_audio_tokens: usage.input_audio_tokens,
            output_audio_tokens: usage.output_audio_tokens,
        }
    }
}

/// Cost Reporting output for either STT or LLM events.
#[derive(Debug, Clone, Default)]
pub struct ProviderCostReport {
    pub audio_duration_secs: Option<f64>,
    pub tokens: Option<CostTokenUsage>,
    pub estimated_cost_usd_micros: Option<openai::UsdMicros>,
    pub estimated_cost_breakdown_openai: Option<openai::OpenAiCostBreakdown>,
}

/// Estimate STT cost from provider telemetry.
///
/// WAV-derived duration is preferred by the caller and passed in as
/// `wav_audio_secs`. Provider response duration is a fallback for paths where
/// WAV bytes are unavailable (for example, some diagnostic/test flows).
pub fn report_stt_cost(
    provider: &str,
    model: Option<&str>,
    response_json: Option<&JsonValue>,
    wav_audio_secs: Option<f64>,
) -> ProviderCostReport {
    let audio_duration_secs = wav_audio_secs.or_else(|| match provider {
        "openai" => response_json.and_then(parse_openai_stt_duration_secs_from_response_json),
        "deepgram" => response_json.and_then(parse_deepgram_stt_duration_secs_from_response_json),
        _ => None,
    });

    let mut report = ProviderCostReport {
        audio_duration_secs,
        ..ProviderCostReport::default()
    };

    match provider {
        "openai" => {
            if let (Some(model), Some(resp)) = (model, response_json) {
                if let Some(usage) = parse_openai_usage_from_response_json(resp) {
                    report.tokens = Some(CostTokenUsage::from_openai_usage(usage));
                    if let Some(breakdown) = openai::estimate_cost_from_usage(model, usage) {
                        report.estimated_cost_usd_micros = Some(breakdown.total_usd_micros);
                        report.estimated_cost_breakdown_openai = Some(breakdown);
                    }
                }
            }

            // Whisper-style transcription endpoints price by audio duration rather than
            // by token usage, so keep the duration fallback after the token attempt.
            if report.estimated_cost_usd_micros.is_none() {
                if let (Some(model), Some(secs)) = (model, audio_duration_secs) {
                    report.estimated_cost_usd_micros =
                        openai::estimate_transcription_cost_from_audio_secs(model, secs);
                }
            }
        }
        "groq" => {
            if let (Some(model), Some(secs)) = (model, audio_duration_secs) {
                report.estimated_cost_usd_micros =
                    groq::estimate_stt_cost_from_audio_secs(model, secs);
            }
        }
        "deepgram" => {
            if let (Some(model), Some(secs)) = (model, audio_duration_secs) {
                report.estimated_cost_usd_micros =
                    deepgram::estimate_stt_cost_from_audio_secs(model, secs);
            }
        }
        "aquavoice" => {
            if let (Some(model), Some(secs)) = (model, audio_duration_secs) {
                report.estimated_cost_usd_micros =
                    aquavoice::estimate_stt_cost_from_audio_secs(model, secs);
            }
        }
        "assemblyai" => {
            if let (Some(model), Some(secs)) = (model, audio_duration_secs) {
                report.estimated_cost_usd_micros =
                    assemblyai::estimate_stt_cost_from_audio_secs(model, secs);
            }
        }
        "speechmatics" => {
            if let (Some(model), Some(secs)) = (model, audio_duration_secs) {
                report.estimated_cost_usd_micros =
                    speechmatics::estimate_stt_cost_from_audio_secs(model, secs);
            }
        }
        "fireworks" => {
            if let (Some(model), Some(secs)) = (model, audio_duration_secs) {
                report.estimated_cost_usd_micros =
                    fireworks::estimate_stt_cost_from_audio_secs(model, secs);
            }
        }
        _ => {}
    }

    report
}

/// Estimate LLM cost from provider response telemetry.
pub fn report_llm_cost(
    provider: &str,
    model: &str,
    response_json: Option<&JsonValue>,
) -> ProviderCostReport {
    let mut report = ProviderCostReport::default();
    let Some(resp) = response_json else {
        return report;
    };

    match provider {
        "openai" => {
            if let Some(usage) = parse_openai_usage_from_response_json(resp) {
                report.tokens = Some(CostTokenUsage::from_openai_usage(usage));
                if let Some(breakdown) = openai::estimate_cost_from_usage(model, usage) {
                    report.estimated_cost_usd_micros = Some(breakdown.total_usd_micros);
                    report.estimated_cost_breakdown_openai = Some(breakdown);
                }
            }
        }
        "groq" => {
            if let Some(usage) = parse_openai_usage_from_response_json(resp) {
                report.tokens = Some(CostTokenUsage::from_openai_usage(usage));
                report.estimated_cost_usd_micros = groq::estimate_llm_cost_from_usage(model, usage);
            }
        }
        "gemini" => {
            if let Some(usage) = parse_gemini_usage_from_response_json(resp) {
                report.tokens = Some(CostTokenUsage {
                    input_tokens: usage.input_tokens,
                    output_tokens: usage.output_tokens,
                    ..CostTokenUsage::default()
                });
                report.estimated_cost_usd_micros =
                    gemini::estimate_llm_cost_from_usage(model, usage);
            }
        }
        "anthropic" => {
            if let Some(usage) = parse_anthropic_usage_from_response_json(resp) {
                report.tokens = Some(CostTokenUsage {
                    input_tokens: usage.total_input_tokens_for_tier(),
                    output_tokens: usage.output_tokens,
                    cached_input_tokens: usage.cache_read_input_tokens,
                    ..CostTokenUsage::default()
                });
                report.estimated_cost_usd_micros =
                    anthropic::estimate_llm_cost_from_usage(model, usage);
            }
        }
        "fireworks" => {
            if let Some(usage) = parse_openai_usage_from_response_json(resp) {
                report.tokens = Some(CostTokenUsage::from_openai_usage(usage));
                report.estimated_cost_usd_micros =
                    fireworks::estimate_llm_cost_from_usage(model, usage);
            }
        }
        _ => {}
    }

    report
}

/// Parse OpenAI usage information out of a response JSON.
///
/// Supports both:
/// - Responses API: usage.input_tokens/output_tokens + *_details
/// - Chat Completions API: usage.prompt_tokens/completion_tokens
pub fn parse_openai_usage_from_response_json(v: &JsonValue) -> Option<openai::OpenAiUsage> {
    let usage = v.get("usage")?;

    // Chat Completions shape
    if usage.get("prompt_tokens").is_some() || usage.get("completion_tokens").is_some() {
        let prompt = usage
            .get("prompt_tokens")
            .and_then(|x| x.as_u64())
            .unwrap_or(0);
        let completion = usage
            .get("completion_tokens")
            .and_then(|x| x.as_u64())
            .unwrap_or(0);
        return Some(openai::OpenAiUsage {
            input_tokens: prompt,
            output_tokens: completion,
            cached_input_tokens: 0,
            input_audio_tokens: 0,
            output_audio_tokens: 0,
        });
    }

    // Responses shape
    let input_tokens = usage
        .get("input_tokens")
        .and_then(|x| x.as_u64())
        .unwrap_or(0);
    let output_tokens = usage
        .get("output_tokens")
        .and_then(|x| x.as_u64())
        .unwrap_or(0);

    let cached_input_tokens = usage
        .get("input_tokens_details")
        .and_then(|d| d.get("cached_tokens"))
        .and_then(|x| x.as_u64())
        .unwrap_or(0);

    let input_audio_tokens = usage
        .get("input_tokens_details")
        .and_then(|d| d.get("audio_tokens"))
        .and_then(|x| x.as_u64())
        .unwrap_or(0);

    let output_audio_tokens = usage
        .get("output_tokens_details")
        .and_then(|d| d.get("audio_tokens"))
        .and_then(|x| x.as_u64())
        .unwrap_or(0);

    Some(openai::OpenAiUsage {
        input_tokens,
        output_tokens,
        cached_input_tokens,
        input_audio_tokens,
        output_audio_tokens,
    })
}

/// Parse Gemini token usage information out of a Gemini `models.generateContent` response JSON.
pub fn parse_gemini_usage_from_response_json(v: &JsonValue) -> Option<openai::OpenAiUsage> {
    let usage = v.get("usageMetadata").or_else(|| v.get("usage_metadata"))?;

    let prompt = usage
        .get("promptTokenCount")
        .or_else(|| usage.get("prompt_token_count"))
        .and_then(|x| x.as_u64())
        .unwrap_or(0);

    let candidates = usage
        .get("candidatesTokenCount")
        .or_else(|| usage.get("candidates_token_count"))
        .and_then(|x| x.as_u64())
        .unwrap_or(0);

    Some(openai::OpenAiUsage {
        input_tokens: prompt,
        output_tokens: candidates,
        cached_input_tokens: 0,
        input_audio_tokens: 0,
        output_audio_tokens: 0,
    })
}

/// Parse Anthropic Claude Messages API token usage out of a response JSON.
pub fn parse_anthropic_usage_from_response_json(
    v: &JsonValue,
) -> Option<anthropic::AnthropicUsage> {
    let usage = v.get("usage")?;

    let input_tokens = usage
        .get("input_tokens")
        .and_then(|x| x.as_u64())
        .unwrap_or(0);
    let output_tokens = usage
        .get("output_tokens")
        .and_then(|x| x.as_u64())
        .unwrap_or(0);

    let cache_read_input_tokens = usage
        .get("cache_read_input_tokens")
        .and_then(|x| x.as_u64())
        .unwrap_or(0);

    let cache_creation_total = usage
        .get("cache_creation_input_tokens")
        .and_then(|x| x.as_u64())
        .unwrap_or(0);

    let mut cache_creation_5m_input_tokens = usage
        .get("cache_creation")
        .and_then(|c| c.get("ephemeral_5m_input_tokens"))
        .and_then(|x| x.as_u64())
        .unwrap_or(0);

    let cache_creation_1h_input_tokens = usage
        .get("cache_creation")
        .and_then(|c| c.get("ephemeral_1h_input_tokens"))
        .and_then(|x| x.as_u64())
        .unwrap_or(0);

    // If the split isn't present, fall back to the aggregated count.
    if cache_creation_5m_input_tokens == 0 && cache_creation_1h_input_tokens == 0 {
        cache_creation_5m_input_tokens = cache_creation_total;
    } else {
        // If the totals don't match (API evolution), assign any remainder to 5m.
        let split_sum =
            cache_creation_5m_input_tokens.saturating_add(cache_creation_1h_input_tokens);
        if cache_creation_total > split_sum {
            cache_creation_5m_input_tokens = cache_creation_5m_input_tokens
                .saturating_add(cache_creation_total.saturating_sub(split_sum));
        }
    }

    Some(anthropic::AnthropicUsage {
        input_tokens,
        output_tokens,
        cache_creation_5m_input_tokens,
        cache_creation_1h_input_tokens,
        cache_read_input_tokens,
    })
}

/// Parse OpenAI STT duration (seconds) from transcription responses.
pub fn parse_openai_stt_duration_secs_from_response_json(v: &JsonValue) -> Option<f64> {
    let usage = v.get("usage")?;
    let ty = usage.get("type").and_then(|x| x.as_str()).unwrap_or("");
    if ty != "duration" {
        return None;
    }

    usage
        .get("seconds")
        .and_then(|x| x.as_f64().or_else(|| x.as_u64().map(|u| u as f64)))
        .filter(|s| s.is_finite() && *s >= 0.0)
}

/// Parse Deepgram STT duration (seconds) from a `/v1/listen` response.
pub fn parse_deepgram_stt_duration_secs_from_response_json(v: &JsonValue) -> Option<f64> {
    v.get("metadata")?
        .get("duration")
        .and_then(|x| x.as_f64().or_else(|| x.as_u64().map(|u| u as f64)))
        .filter(|s| s.is_finite() && *s >= 0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openai_stt_report_uses_provider_duration_when_wav_is_missing() {
        let response = serde_json::json!({
            "usage": { "type": "duration", "seconds": 61.0 }
        });

        let report = report_stt_cost("openai", Some("whisper-1"), Some(&response), None);

        assert_eq!(report.audio_duration_secs, Some(61.0));
        assert_eq!(
            report.estimated_cost_usd_micros,
            openai::estimate_transcription_cost_from_audio_secs("whisper-1", 61.0)
        );
    }

    #[test]
    fn groq_stt_report_uses_shared_audio_duration_shape() {
        let report = report_stt_cost("groq", Some("whisper-large-v3-turbo"), None, Some(30.0));

        assert_eq!(report.audio_duration_secs, Some(30.0));
        assert_eq!(
            report.estimated_cost_usd_micros,
            groq::estimate_stt_cost_from_audio_secs("whisper-large-v3-turbo", 30.0)
        );
    }

    #[test]
    fn openai_and_groq_llm_reports_share_usage_mapping_without_sharing_rates() {
        let response = serde_json::json!({
            "usage": {
                "prompt_tokens": 1_000,
                "completion_tokens": 500
            }
        });

        let openai = report_llm_cost("openai", "gpt-4o-mini", Some(&response));
        let groq = report_llm_cost("groq", "llama-3.1-8b-instant", Some(&response));

        assert_eq!(
            openai.tokens,
            Some(CostTokenUsage {
                input_tokens: 1_000,
                output_tokens: 500,
                ..CostTokenUsage::default()
            })
        );
        assert_eq!(openai.tokens, groq.tokens);
        assert!(openai.estimated_cost_usd_micros.is_some());
        assert!(groq.estimated_cost_usd_micros.is_some());
        assert_ne!(
            openai.estimated_cost_usd_micros,
            groq.estimated_cost_usd_micros
        );
    }

    #[test]
    fn anthropic_report_preserves_cache_tokens_for_stats() {
        let response = serde_json::json!({
            "usage": {
                "input_tokens": 10,
                "output_tokens": 5,
                "cache_read_input_tokens": 3,
                "cache_creation_input_tokens": 7
            }
        });

        let report = report_llm_cost("anthropic", "claude-sonnet-4-5", Some(&response));

        assert_eq!(
            report.tokens,
            Some(CostTokenUsage {
                input_tokens: 20,
                output_tokens: 5,
                cached_input_tokens: 3,
                ..CostTokenUsage::default()
            })
        );
    }
}
