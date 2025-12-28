use serde::Serialize;

use crate::cost::groq as groq_cost;
use crate::cost::openai as openai_cost;
use crate::cost::aquavoice as aquavoice_cost;
use crate::cost::gemini as gemini_cost;
use crate::cost::anthropic as anthropic_cost;
use crate::cost::deepgram as deepgram_cost;

#[derive(Debug, Clone, Serialize)]
pub struct SttModelPricing {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usd_micros_per_minute: Option<openai_cost::UsdMicros>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usd_micros_per_hour: Option<openai_cost::UsdMicros>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_billed_secs: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LlmModelPricing {
    pub input_usd_micros_per_1m: openai_cost::UsdMicros,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cached_input_usd_micros_per_1m: Option<openai_cost::UsdMicros>,
    pub output_usd_micros_per_1m: openai_cost::UsdMicros,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelPricingResponse {
    pub kind: String,
    pub provider: String,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stt: Option<SttModelPricing>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub llm: Option<LlmModelPricing>,
}

#[tauri::command]
pub fn get_model_pricing(provider: String, kind: String, model: String) -> Option<ModelPricingResponse> {
    let provider_norm = provider.trim().to_lowercase();
    let kind_norm = kind.trim().to_lowercase();
    let model_norm = model.trim().to_string();

    if provider_norm.is_empty() || kind_norm.is_empty() || model_norm.is_empty() {
        return None;
    }

    match kind_norm.as_str() {
        "stt" => {
            if provider_norm == "openai" {
                let per_min = openai_cost::transcription_usd_micros_per_minute(&model_norm)?;
                return Some(ModelPricingResponse {
                    kind: "stt".into(),
                    provider: provider_norm,
                    model: model_norm,
                    stt: Some(SttModelPricing {
                        usd_micros_per_minute: Some(per_min),
                        usd_micros_per_hour: None,
                        min_billed_secs: None,
                    }),
                    llm: None,
                });
            }

            if provider_norm == "groq" {
                let per_hour = groq_cost::asr_usd_micros_per_hour(&model_norm)?;
                return Some(ModelPricingResponse {
                    kind: "stt".into(),
                    provider: provider_norm,
                    model: model_norm,
                    stt: Some(SttModelPricing {
                        usd_micros_per_minute: None,
                        usd_micros_per_hour: Some(per_hour),
                        min_billed_secs: Some(10),
                    }),
                    llm: None,
                });
            }

            if provider_norm == "deepgram" {
                let per_min = deepgram_cost::stt_usd_micros_per_minute(&model_norm)?;
                return Some(ModelPricingResponse {
                    kind: "stt".into(),
                    provider: provider_norm,
                    model: model_norm,
                    stt: Some(SttModelPricing {
                        usd_micros_per_minute: Some(per_min),
                        usd_micros_per_hour: None,
                        min_billed_secs: None,
                    }),
                    llm: None,
                });
            }

            if provider_norm == "aquavoice" {
                let per_hour = aquavoice_cost::asr_usd_micros_per_hour(&model_norm)?;
                return Some(ModelPricingResponse {
                    kind: "stt".into(),
                    provider: provider_norm,
                    model: model_norm,
                    stt: Some(SttModelPricing {
                        usd_micros_per_minute: None,
                        usd_micros_per_hour: Some(per_hour),
                        min_billed_secs: None,
                    }),
                    llm: None,
                });
            }

            None
        }
        "llm" => {
            if provider_norm == "openai" {
                let rates = openai_cost::text_token_rates(&model_norm)?;
                return Some(ModelPricingResponse {
                    kind: "llm".into(),
                    provider: provider_norm,
                    model: model_norm,
                    stt: None,
                    llm: Some(LlmModelPricing {
                        input_usd_micros_per_1m: rates.input_usd_micros_per_1m,
                        cached_input_usd_micros_per_1m: rates.cached_input_usd_micros_per_1m,
                        output_usd_micros_per_1m: rates.output_usd_micros_per_1m,
                    }),
                });
            }

            if provider_norm == "groq" {
                let rates = groq_cost::text_token_rates(&model_norm)?;
                return Some(ModelPricingResponse {
                    kind: "llm".into(),
                    provider: provider_norm,
                    model: model_norm,
                    stt: None,
                    llm: Some(LlmModelPricing {
                        input_usd_micros_per_1m: rates.input_usd_micros_per_1m,
                        cached_input_usd_micros_per_1m: rates.cached_input_usd_micros_per_1m,
                        output_usd_micros_per_1m: rates.output_usd_micros_per_1m,
                    }),
                });
            }

            if provider_norm == "gemini" {
                let rates = gemini_cost::text_token_rates(&model_norm)?;
                return Some(ModelPricingResponse {
                    kind: "llm".into(),
                    provider: provider_norm,
                    model: model_norm,
                    stt: None,
                    llm: Some(LlmModelPricing {
                        input_usd_micros_per_1m: rates.input_usd_micros_per_1m,
                        cached_input_usd_micros_per_1m: rates.cached_input_usd_micros_per_1m,
                        output_usd_micros_per_1m: rates.output_usd_micros_per_1m,
                    }),
                });
            }

            if provider_norm == "anthropic" {
                let rates = anthropic_cost::text_token_rates(&model_norm)?;
                return Some(ModelPricingResponse {
                    kind: "llm".into(),
                    provider: provider_norm,
                    model: model_norm,
                    stt: None,
                    llm: Some(LlmModelPricing {
                        input_usd_micros_per_1m: rates.input_usd_micros_per_1m,
                        cached_input_usd_micros_per_1m: rates.cached_input_usd_micros_per_1m,
                        output_usd_micros_per_1m: rates.output_usd_micros_per_1m,
                    }),
                });
            }

            None
        }
        _ => None,
    }
}
