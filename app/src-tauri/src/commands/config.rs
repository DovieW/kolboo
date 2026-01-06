//! Tauri commands for configuration endpoints.
//!
//! This module provides commands that replace the Python server's config API,
//! including default prompt sections and available providers.

use serde::Serialize;
use tauri::AppHandle;

use crate::request_log::RequestLogStore;

#[cfg(desktop)]
use tauri_plugin_store::StoreExt;

// ============================================================================
// Default Prompt Sections
// ============================================================================

/// System prompt - Core formatting rules
pub const SYSTEM_PROMPT_DEFAULT: &str = r#"You are a dictation formatting assistant. Your task is to format transcribed speech.

## Core Rules
- Remove filler words (um, uh, err, erm, etc.)
- Use punctuation where appropriate
- Capitalize sentences properly
- Keep the original meaning and tone intact
- Do NOT add any new information or change the intent
- Do NOT condense, summarize, or make sentences more concise - preserve the speaker's full expression
- Do NOT answer questions - if the user dictates a question, output the cleaned question, not an answer
- Do NOT respond conversationally or engage with the content - you are a text processor, not a conversational assistant
- Output ONLY the cleaned text, nothing else - no explanations, no quotes, no prefixes

### Good Example
Input: "um so basically I was like thinking we should uh you know update the readme file"
Output: "So basically, I was thinking we should update the readme file."

### Bad Examples

1. Condensing/summarizing (preserve full expression):
   Input: "I really think that we should probably consider maybe going to the store to pick up some groceries"
   Bad: "We should go grocery shopping."
   Good: "I really think that we should probably consider going to the store to pick up some groceries."

2. Answering questions (just clean the question):
   Input: "what is the capital of France"
   Bad: "The capital of France is Paris."
   Good: "What is the capital of France?"

3. Responding conversationally (format, don't engage):
   Input: "hey how are you doing today"
   Bad: "I'm doing well, thank you for asking!"
   Good: "Hey, how are you doing today?"

4. Adding information (keep original intent only):
   Input: "send the email to john"
   Bad: "Send the email to John as soon as possible."
   Good: "Send the email to John."

## Punctuation
Convert spoken punctuation to symbols:
- "comma" = ,
- "period" or "full stop" = .
- "question mark" = ?
- "exclamation point" or "exclamation mark" = !
- "dash" = -
- "em dash" = —
- "quotation mark" or "quote" or "end quote" = "
- "colon" = :
- "semicolon" = ;
- "open parenthesis" or "open paren" = (
- "close parenthesis" or "close paren" = )

Example:
Input: "I can't wait exclamation point Let's meet at seven period"
Output: "I can't wait! Let's meet at seven."

## New Line and Paragraph
- "new line" = Insert a line break
- "new paragraph" = Insert a paragraph break (blank line)

Example:
Input: "Hello, new line, world, new paragraph, bye"
Output: "Hello
world

bye""#;

/// Response containing default prompt sections
#[derive(Debug, Serialize)]
pub struct DefaultSectionsResponse {
    pub system: String,
}

/// Get default prompts for each section
#[tauri::command]
pub fn get_default_sections() -> DefaultSectionsResponse {
    DefaultSectionsResponse {
        system: SYSTEM_PROMPT_DEFAULT.to_string(),
    }
}

// ============================================================================
// Available Providers
// ============================================================================

/// Information about a provider
#[derive(Debug, Serialize)]
pub struct ProviderInfo {
    pub value: String,
    pub label: String,
    pub is_local: bool,
}

/// Response listing available providers
#[derive(Debug, Serialize)]
pub struct AvailableProvidersResponse {
    pub stt: Vec<ProviderInfo>,
    pub llm: Vec<ProviderInfo>,
}

/// STT provider definitions
const STT_PROVIDERS: &[(&str, &str, bool)] = &[
    ("openai", "OpenAI", false),
    ("aquavoice", "Aquavoice (Avalon)", false),
    ("groq", "Groq", false),
    ("assemblyai", "AssemblyAI", false),
    ("speechmatics", "Speechmatics", false),
    ("deepgram", "Deepgram", false),
    ("whisper", "Local Whisper", true),
];

/// LLM provider definitions
const LLM_PROVIDERS: &[(&str, &str, bool)] = &[
    ("openai", "OpenAI", false),
    ("gemini", "Google AI Studio", false),
    ("anthropic", "Anthropic", false),
    ("cohere", "Cohere", false),
    ("groq", "Groq", false),
    ("ollama", "Ollama", true),
];

/// Helper to check if an API key is configured in the store
#[cfg(desktop)]
fn has_api_key(app: &AppHandle, key: &str) -> bool {
    app.store("settings.json")
        .ok()
        .and_then(|store| store.get(key))
        .and_then(|v| v.as_str().map(|s| !s.is_empty()))
        .unwrap_or(false)
}

/// Get list of available STT and LLM providers (those with API keys configured)
#[cfg(desktop)]
#[tauri::command]
pub fn get_available_providers(app: AppHandle) -> AvailableProvidersResponse {
    let mut stt_providers = Vec::new();
    let mut llm_providers = Vec::new();

    // Check which STT providers have API keys
    for (id, label, is_local) in STT_PROVIDERS {
        let key_name = format!("{}_api_key", id);
        // Local providers don't need API keys, remote ones do
        if *is_local || has_api_key(&app, &key_name) {
            stt_providers.push(ProviderInfo {
                value: id.to_string(),
                label: label.to_string(),
                is_local: *is_local,
            });
        }
    }

    // Check which LLM providers have API keys
    for (id, label, is_local) in LLM_PROVIDERS {
        let key_name = format!("{}_api_key", id);
        // Local providers don't need API keys, remote ones do
        if *is_local || has_api_key(&app, &key_name) {
            llm_providers.push(ProviderInfo {
                value: id.to_string(),
                label: label.to_string(),
                is_local: *is_local,
            });
        }
    }

    AvailableProvidersResponse {
        stt: stt_providers,
        llm: llm_providers,
    }
}

/// Stub for non-desktop platforms
#[cfg(not(desktop))]
#[tauri::command]
pub fn get_available_providers(_app: AppHandle) -> AvailableProvidersResponse {
    AvailableProvidersResponse {
        stt: vec![],
        llm: vec![],
    }
}

// ============================================================================
// Pipeline Configuration Updates
// ============================================================================

/// Update the pipeline configuration when settings change
/// This re-initializes the STT provider based on current settings
#[cfg(desktop)]
#[tauri::command]
pub fn sync_pipeline_config(app: AppHandle) -> Result<(), String> {
    use crate::pipeline::{PipelineConfig, SharedPipeline};
    use crate::stt::RetryConfig;
    use tauri::Manager;

    // Read STT settings from store
    let stt_provider: String = app
        .store("settings.json")
        .ok()
        .and_then(|store| store.get("stt_provider"))
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_else(|| "groq".to_string());

    // Read STT model from store
    let stt_model: Option<String> = app
        .store("settings.json")
        .ok()
        .and_then(|store| store.get("stt_model"))
        .and_then(|v| serde_json::from_value(v).ok());

    // Read global STT transcription prompt from store
    let stt_transcription_prompt: Option<String> = app
        .store("settings.json")
        .ok()
        .and_then(|store| store.get("stt_transcription_prompt"))
        .and_then(|v| serde_json::from_value(v).ok());

    // Get the appropriate API key based on provider
    let stt_api_key: String = {
        let key_name = format!("{}_api_key", stt_provider);
        app.store("settings.json")
            .ok()
            .and_then(|store| store.get(&key_name))
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default()
    };

    // Read all available STT API keys (for per-profile provider overrides at runtime)
    let mut stt_api_keys: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    for provider in [
        "openai",
        "aquavoice",
        "groq",
        "assemblyai",
        "speechmatics",
        "deepgram",
    ] {
        let key_name = format!("{}_api_key", provider);
        let key: String = app
            .store("settings.json")
            .ok()
            .and_then(|store| store.get(&key_name))
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default();
        if !key.is_empty() {
            stt_api_keys.insert(provider.to_string(), key);
        }
    }

    // Read STT timeout from store (seconds)
    let stt_timeout_seconds_raw: f64 = app
        .store("settings.json")
        .ok()
        .and_then(|store| store.get("stt_timeout_seconds"))
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or(10.0);

    // Guard against invalid values (NaN/inf/<=0) to avoid Duration::from_secs_f64 panics.
    let stt_timeout_seconds: f64 = if stt_timeout_seconds_raw.is_finite() && stt_timeout_seconds_raw > 0.0 {
        stt_timeout_seconds_raw
    } else {
        log::warn!(
            "Invalid stt_timeout_seconds value in store ({}); falling back to 10s",
            stt_timeout_seconds_raw
        );
        10.0
    };

    // Read LLM settings from store
    // NOTE: If the user has not selected an LLM provider yet, keep LLM disabled.
    let rewrite_llm_enabled: bool = app
        .store("settings.json")
        .ok()
        .and_then(|store| store.get("rewrite_llm_enabled"))
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or(false);

    let llm_provider_setting: Option<String> = app
        .store("settings.json")
        .ok()
        .and_then(|store| store.get("llm_provider"))
        .and_then(|v| serde_json::from_value(v).ok());

    let llm_model_setting: Option<String> = app
        .store("settings.json")
        .ok()
        .and_then(|store| store.get("llm_model"))
        .and_then(|v| serde_json::from_value(v).ok());

    // Optional provider-specific reasoning/thinking knobs.
    // These are ignored unless the selected provider/model supports them.
    let openai_reasoning_effort: Option<String> = app
        .store("settings.json")
        .ok()
        .and_then(|store| store.get("openai_reasoning_effort"))
        .and_then(|v| serde_json::from_value(v).ok());
    let gemini_thinking_budget: Option<i64> = app
        .store("settings.json")
        .ok()
        .and_then(|store| store.get("gemini_thinking_budget"))
        .and_then(|v| serde_json::from_value(v).ok());
    let gemini_thinking_level: Option<String> = app
        .store("settings.json")
        .ok()
        .and_then(|store| store.get("gemini_thinking_level"))
        .and_then(|v| serde_json::from_value(v).ok());

    let anthropic_thinking_budget: Option<i64> = app
        .store("settings.json")
        .ok()
        .and_then(|store| store.get("anthropic_thinking_budget"))
        .and_then(|v| serde_json::from_value(v).ok());

    // If the user never explicitly selected a model, treat "default" as the provider's
    // concrete default model so request logs can display the exact model used.
    let llm_provider_effective = llm_provider_setting
        .clone()
        .unwrap_or_else(|| "openai".to_string());
    let llm_model_effective: Option<String> = llm_model_setting.or_else(|| {
        if rewrite_llm_enabled {
            crate::llm::default_llm_model_for_provider(llm_provider_effective.as_str())
                .map(|m| m.to_string())
        } else {
            None
        }
    });

    let llm_api_key: String = llm_provider_setting
        .as_deref()
        .map(|provider| {
            let key_name = format!("{}_api_key", provider);
            app.store("settings.json")
                .ok()
                .and_then(|store| store.get(&key_name))
                .and_then(|v| serde_json::from_value(v).ok())
                .unwrap_or_default()
        })
        .unwrap_or_default();

    // IMPORTANT: `enabled` is only the global toggle. The effective provider/key is resolved
    // per transcription based on the active profile.
    let llm_enabled = rewrite_llm_enabled;

    // Read all available LLM API keys (for per-profile provider overrides at runtime)
    let mut llm_api_keys: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    for provider in ["openai", "anthropic", "groq", "gemini", "cohere"] {
        let key_name = format!("{}_api_key", provider);
        let key: String = app
            .store("settings.json")
            .ok()
            .and_then(|store| store.get(&key_name))
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default();
        if !key.is_empty() {
            llm_api_keys.insert(provider.to_string(), key);
        }
    }

    // Read rewrite prompt sections + per-program profiles from store
    let cleanup_prompt_sections: Option<crate::settings::CleanupPromptSectionsSetting> = app
        .store("settings.json")
        .ok()
        .and_then(|store| store.get("cleanup_prompt_sections"))
        .and_then(|v| serde_json::from_value(v).ok());

    let base_prompts: crate::llm::PromptSections = cleanup_prompt_sections
        .as_ref()
        .map(|o| o.apply_to(&crate::llm::PromptSections::default()))
        .unwrap_or_else(crate::llm::PromptSections::default);

    let rewrite_program_prompt_profiles: Vec<crate::settings::RewriteProgramPromptProfile> = app
        .store("settings.json")
        .ok()
        .and_then(|store| store.get("rewrite_program_prompt_profiles"))
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();

    let program_prompt_profiles: Vec<crate::llm::ProgramPromptProfile> = rewrite_program_prompt_profiles
        .into_iter()
        .map(|p| {
            let profile_prompts = p
                .cleanup_prompt_sections
                .as_ref()
                .map(|o| o.apply_to(&base_prompts))
                .unwrap_or_else(|| base_prompts.clone());

            let presets = p
                .presets
                .into_iter()
                .map(|preset| {
                    let preset_prompts = preset
                        .cleanup_prompt_sections
                        .as_ref()
                        .map(|o| o.apply_to(&profile_prompts))
                        .unwrap_or_else(|| profile_prompts.clone());

                    crate::llm::ProgramPreset {
                        id: preset.id,
                        name: preset.name,
                        description: preset.description,
                        routing_hints: preset.routing_hints,
                        prompts: preset_prompts,
                        rewrite_llm_enabled: preset.rewrite_llm_enabled,
                        stt_provider: preset.stt_provider,
                        stt_model: preset.stt_model,
                        stt_timeout_seconds: preset.stt_timeout_seconds,
                        llm_provider: preset.llm_provider,
                        llm_model: preset.llm_model,
                        openai_reasoning_effort: preset.openai_reasoning_effort,
                        gemini_thinking_budget: preset.gemini_thinking_budget,
                        gemini_thinking_level: preset.gemini_thinking_level,
                        anthropic_thinking_budget: preset.anthropic_thinking_budget,
                    }
                })
                .collect();

            crate::llm::ProgramPromptProfile {
                id: p.id,
                name: p.name,
                program_paths: p.program_paths,
                prompts: profile_prompts,

                presets,
                default_preset_id: p.default_preset_id,
                default_preset_description: p.default_preset_description,
                active_preset_id: p.active_preset_id,
                router: p.router,

                rewrite_llm_enabled: p.rewrite_llm_enabled,
                stt_provider: p.stt_provider,
                stt_model: p.stt_model,
                stt_timeout_seconds: p.stt_timeout_seconds,
                llm_provider: p.llm_provider,
                llm_model: p.llm_model,
                openai_reasoning_effort: p.openai_reasoning_effort,
                gemini_thinking_budget: p.gemini_thinking_budget,
                gemini_thinking_level: p.gemini_thinking_level,
                anthropic_thinking_budget: p.anthropic_thinking_budget,
            }
        })
        .collect();

    // Read VAD settings from store
    let vad_settings: VadSettings = app
        .store("settings.json")
        .ok()
        .and_then(|store| store.get("vad_settings"))
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();

    // Read selected input device from store.
    // NOTE: The frontend setting key is historically named `selected_mic_id`.
    // Newer builds store a backend-generated selection token (unique per enumerated device).
    // Older builds stored a CPAL device *name*.
    // The audio capture layer accepts both.
    let input_device_name: Option<String> = app
        .store("settings.json")
        .ok()
        .and_then(|store| store.get("selected_mic_id"))
        .and_then(|v| serde_json::from_value(v).ok())
        .and_then(|s: String| {
            let t = s.trim().to_string();
            if t.is_empty() || t == "default" { None } else { Some(t) }
        });

    // Capture behavior (Hot Mic + recovery)
    let hot_mic_enabled: bool = app
        .store("settings.json")
        .ok()
        .and_then(|store| store.get("hot_mic_enabled"))
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or(false);

    let hot_mic_pre_roll_ms: u32 = app
        .store("settings.json")
        .ok()
        .and_then(|store| store.get("hot_mic_pre_roll_ms"))
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or(1500)
        .min(5000);

    let mic_auto_recover_enabled: bool = app
        .store("settings.json")
        .ok()
        .and_then(|store| store.get("mic_auto_recover_enabled"))
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or(false);

    // Read quiet-audio gate settings from store
    let default_pipeline_config = PipelineConfig::default();
    let quiet_audio_gate_enabled: bool = app
        .store("settings.json")
        .ok()
        .and_then(|store| store.get("quiet_audio_gate_enabled"))
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or(default_pipeline_config.quiet_audio_gate_enabled);

    let quiet_audio_min_duration_secs: f32 = app
        .store("settings.json")
        .ok()
        .and_then(|store| store.get("quiet_audio_min_duration_secs"))
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or(default_pipeline_config.quiet_audio_min_duration_secs);

    let quiet_audio_rms_dbfs_threshold: f32 = app
        .store("settings.json")
        .ok()
        .and_then(|store| store.get("quiet_audio_rms_dbfs_threshold"))
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or(default_pipeline_config.quiet_audio_rms_dbfs_threshold);

    let quiet_audio_peak_dbfs_threshold: f32 = app
        .store("settings.json")
        .ok()
        .and_then(|store| store.get("quiet_audio_peak_dbfs_threshold"))
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or(default_pipeline_config.quiet_audio_peak_dbfs_threshold);

    // Read experimental noise gate settings from store.
    // New: `noise_gate_threshold_dbfs` (Option<f32>), with legacy fallback to
    // `noise_gate_strength` (0..=100 mapped to -75..-30 dBFS).
    let noise_gate_threshold_dbfs: Option<f32> = app
        .store("settings.json")
        .ok()
        .and_then(|store| store.get("noise_gate_threshold_dbfs"))
        .and_then(|v| serde_json::from_value(v).ok())
        .and_then(|v: f32| if v.is_finite() { Some(v.clamp(-75.0, -30.0)) } else { None })
        .or_else(|| {
            let strength_raw: u64 = app
                .store("settings.json")
                .ok()
                .and_then(|store| store.get("noise_gate_strength"))
                .and_then(|v| serde_json::from_value(v).ok())
                .unwrap_or(0);
            let s = (strength_raw.min(100) as u8) as f32;
            if s <= 0.0 {
                None
            } else {
                let t = s / 100.0;
                Some((-75.0 + (-30.0 + 75.0) * t).clamp(-75.0, -30.0))
            }
        });

    // Voice pickup preprocessing toggles
    let audio_downmix_to_mono: bool = app
        .store("settings.json")
        .ok()
        .and_then(|store| store.get("audio_downmix_to_mono"))
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or(default_pipeline_config.audio_downmix_to_mono);
    let audio_resample_to_16khz: bool = app
        .store("settings.json")
        .ok()
        .and_then(|store| store.get("audio_resample_to_16khz"))
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or(default_pipeline_config.audio_resample_to_16khz);
    let audio_highpass_enabled: bool = app
        .store("settings.json")
        .ok()
        .and_then(|store| store.get("audio_highpass_enabled"))
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or(default_pipeline_config.audio_highpass_enabled);
    let audio_agc_enabled: bool = app
        .store("settings.json")
        .ok()
        .and_then(|store| store.get("audio_agc_enabled"))
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or(default_pipeline_config.audio_agc_enabled);
    let audio_noise_suppression_enabled: bool = app
        .store("settings.json")
        .ok()
        .and_then(|store| store.get("audio_noise_suppression_enabled"))
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or(default_pipeline_config.audio_noise_suppression_enabled);

    // Extra hallucination protection
    let quiet_audio_require_speech: bool = app
        .store("settings.json")
        .ok()
        .and_then(|store| store.get("quiet_audio_require_speech"))
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or(default_pipeline_config.quiet_audio_require_speech);

    // Network / proxy settings
    let proxy_settings: crate::settings::ProxySettings = app
        .store("settings.json")
        .ok()
        .and_then(|store| store.get("proxy_settings"))
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();

    let config = PipelineConfig {
        input_device_name,
        stt_provider: stt_provider.clone(),
        stt_api_key,
        stt_api_keys,
        stt_model: stt_model.clone(),
        stt_transcription_prompt,
        max_duration_secs: 300.0,
        retry_config: RetryConfig::default(),
        vad_config: vad_settings.to_vad_auto_stop_config(),
        transcription_timeout: std::time::Duration::from_secs_f64(stt_timeout_seconds),
        max_recording_bytes: 50 * 1024 * 1024, // 50MB

        proxy_settings,

        quiet_audio_gate_enabled,
        quiet_audio_min_duration_secs,
        quiet_audio_rms_dbfs_threshold,
        quiet_audio_peak_dbfs_threshold,

        noise_gate_threshold_dbfs,

        audio_downmix_to_mono,
        audio_resample_to_16khz,
        audio_highpass_enabled,
        audio_agc_enabled,
        audio_noise_suppression_enabled,

        quiet_audio_require_speech,

        hot_mic_enabled,
        hot_mic_pre_roll_ms,
        mic_auto_recover_enabled,

        llm_config: crate::llm::LlmConfig {
            enabled: llm_enabled,
            provider: llm_provider_effective,
            api_key: llm_api_key,
            model: llm_model_effective.clone(),
            openai_reasoning_effort,
            gemini_thinking_budget,
            gemini_thinking_level,
            anthropic_thinking_budget,
            prompts: base_prompts,
            program_prompt_profiles,
            ..Default::default()
        },
        llm_api_keys,

        // Preserve provider payload logging across config sync.
        request_log_store: app.try_state::<RequestLogStore>().map(|s| s.inner().clone()),
    };

    // Update the pipeline
    if let Some(pipeline) = app.try_state::<SharedPipeline>() {
        pipeline
            .update_config(config)
            .map_err(|e| format!("Failed to update pipeline config: {}", e))?;
        log::info!(
            "Pipeline config synced - STT: {} ({}), LLM: {} ({}), VAD: {}, program_profiles: {}",
            stt_provider,
            stt_model.as_deref().unwrap_or("default"),
            llm_provider_setting.clone().unwrap_or_else(|| "disabled".to_string()),
            llm_model_effective.as_deref().unwrap_or("default"),
            vad_settings.enabled,
            pipeline.config().llm_config.program_prompt_profiles.len()
        );
    }

    Ok(())
}

/// Stub for non-desktop platforms
#[cfg(not(desktop))]
#[tauri::command]
pub fn sync_pipeline_config(_app: AppHandle) -> Result<(), String> {
    Ok(())
}

// ============================================================================
// VAD Settings
// ============================================================================

use crate::settings::VadSettings;

/// Get current VAD settings from the store
#[cfg(desktop)]
#[tauri::command]
pub fn get_vad_settings(app: AppHandle) -> VadSettings {
    app.store("settings.json")
        .ok()
        .and_then(|store| store.get("vad_settings"))
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default()
}

/// Stub for non-desktop platforms
#[cfg(not(desktop))]
#[tauri::command]
pub fn get_vad_settings(_app: AppHandle) -> VadSettings {
    VadSettings::default()
}

/// Save VAD settings to the store
#[cfg(desktop)]
#[tauri::command]
pub fn set_vad_settings(app: AppHandle, settings: VadSettings) -> Result<(), String> {
    let store = app
        .store("settings.json")
        .map_err(|e| format!("Failed to get store: {}", e))?;

    store
        .set(
            "vad_settings",
            serde_json::to_value(&settings).map_err(|e| format!("Failed to serialize: {}", e))?,
        );

    store
        .save()
        .map_err(|e| format!("Failed to save store: {}", e))?;

    log::info!("VAD settings updated: enabled={}, auto_stop={}", settings.enabled, settings.auto_stop);
    Ok(())
}

/// Stub for non-desktop platforms
#[cfg(not(desktop))]
#[tauri::command]
pub fn set_vad_settings(_app: AppHandle, _settings: VadSettings) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_default_sections() {
        let response = get_default_sections();
        assert!(!response.system.is_empty());
        assert!(response.system.contains("dictation formatting"));
    }
}
