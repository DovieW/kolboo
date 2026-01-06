//! Tauri commands for LLM formatting configuration.

use crate::llm::{LlmConfig, PromptSections, SYSTEM_PROMPT_DEFAULT};
use crate::llm::{
    format_text, AnthropicLlmProvider, CohereLlmProvider, GeminiLlmProvider, GroqLlmProvider,
    LlmProvider, OllamaLlmProvider, OpenAiLlmProvider,
};
use crate::pipeline::SharedPipeline;
use crate::request_log::RequestLogStore;
use std::sync::Arc;
use std::time::Instant;
use std::time::Duration;
use tauri::{AppHandle, Manager, State};
use crate::stats::EventStatus;

/// Error type for LLM commands
#[derive(Debug, serde::Serialize)]
pub struct LlmCommandError {
    pub message: String,
}

impl From<String> for LlmCommandError {
    fn from(message: String) -> Self {
        Self { message }
    }
}

/// LLM configuration payload from frontend
#[derive(Debug, serde::Deserialize)]
pub struct LlmConfigPayload {
    /// Whether LLM formatting is enabled
    pub enabled: bool,
    /// Provider name: "openai", "anthropic", or "ollama"
    pub provider: String,
    /// API key (not needed for ollama)
    pub api_key: Option<String>,
    /// Model to use (optional, uses provider default if not specified)
    pub model: Option<String>,
    /// Base URL for Ollama (optional)
    pub ollama_url: Option<String>,
    /// Timeout in seconds (optional, default 30)
    pub timeout_secs: Option<u64>,
}

#[derive(Debug, serde::Serialize)]
pub struct TestLlmRewriteResponse {
    pub output: String,
    pub provider_used: String,
    pub model_used: String,
}

#[derive(Debug, serde::Serialize)]
pub struct IterateRewritePromptResponse {
    pub improved_prompt: String,
    pub provider_used: String,
    pub model_used: String,
}

#[derive(Debug, serde::Serialize)]
pub struct TestRewriteWithPromptResponse {
    pub output: String,
    pub provider_used: String,
    pub model_used: String,
}

#[derive(Debug, serde::Serialize)]
pub struct LlmCompleteResponse {
    pub output: String,
    pub provider_used: String,
    pub model_used: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct LlmCompleteArgs {
    pub provider: String,
    pub model: Option<String>,

    // Historical/UI naming: accept both camelCase and snake_case.
    #[serde(alias = "systemPrompt")]
    pub system_prompt: String,
    #[serde(alias = "userPrompt")]
    pub user_prompt: String,
}

fn create_llm_provider_unstructured(config: &LlmConfig) -> Arc<dyn LlmProvider> {
    // IMPORTANT:
    // This is used for one-off ad-hoc completions (e.g. History "Analyze transcripts" → "Send to LLM").
    // We intentionally disable rewrite-oriented structured outputs so the model can return free-form text.
    match config.provider.as_str() {
        "anthropic" => {
            let provider = if let Some(model) = &config.model {
                AnthropicLlmProvider::with_model(config.api_key.clone(), model.clone())
            } else {
                AnthropicLlmProvider::new(config.api_key.clone())
            };
            Arc::new(
                provider
                    .with_timeout(config.timeout)
                    .with_thinking_budget(config.anthropic_thinking_budget),
            )
        }
        "groq" => {
            let provider = if let Some(model) = &config.model {
                GroqLlmProvider::with_model(config.api_key.clone(), model.clone())
            } else {
                GroqLlmProvider::new(config.api_key.clone())
            };
            Arc::new(provider.with_timeout(config.timeout))
        }
        "gemini" => {
            let provider = if let Some(model) = &config.model {
                GeminiLlmProvider::with_model(config.api_key.clone(), model.clone())
            } else {
                GeminiLlmProvider::new(config.api_key.clone())
            };

            Arc::new(
                provider
                    .with_timeout(config.timeout)
                    .with_thinking_budget(config.gemini_thinking_budget)
                    .with_thinking_level(config.gemini_thinking_level.clone())
                    .with_structured_outputs(false),
            )
        }
        "cohere" => {
            let provider = if let Some(model) = &config.model {
                CohereLlmProvider::with_model(config.api_key.clone(), model.clone())
            } else {
                CohereLlmProvider::new(config.api_key.clone())
            };
            Arc::new(provider.with_timeout(config.timeout))
        }
        "ollama" => {
            let provider = OllamaLlmProvider::with_url(
                config
                    .ollama_url
                    .clone()
                    .unwrap_or_else(|| "http://localhost:11434".to_string()),
                config.model.clone(),
            );
            Arc::new(provider.with_timeout(config.timeout))
        }
        _ => {
            // Default to OpenAI
            let provider = if let Some(model) = &config.model {
                OpenAiLlmProvider::with_model(config.api_key.clone(), model.clone())
            } else {
                OpenAiLlmProvider::new(config.api_key.clone())
            };
            Arc::new(
                provider
                    .with_timeout(config.timeout)
                    .with_reasoning_effort(config.openai_reasoning_effort.clone())
                    .with_structured_outputs(false),
            )
        }
    }
}

fn create_llm_provider_without_timeout(
    config: &LlmConfig,
    request_log_store: Option<RequestLogStore>,
) -> Arc<dyn LlmProvider> {
    match config.provider.as_str() {
        "anthropic" => {
            let provider = if let Some(model) = &config.model {
                AnthropicLlmProvider::with_model(config.api_key.clone(), model.clone())
            } else {
                AnthropicLlmProvider::new(config.api_key.clone())
            };
            Arc::new(
                provider
                    .without_timeout()
                    .with_request_log_store(request_log_store.clone())
                    .with_thinking_budget(config.anthropic_thinking_budget),
            )
        }
        "groq" => {
            let provider = if let Some(model) = &config.model {
                GroqLlmProvider::with_model(config.api_key.clone(), model.clone())
            } else {
                GroqLlmProvider::new(config.api_key.clone())
            };
            Arc::new(
                provider
                    .without_timeout()
                    .with_request_log_store(request_log_store.clone()),
            )
        }
        "gemini" => {
            let provider = if let Some(model) = &config.model {
                GeminiLlmProvider::with_model(config.api_key.clone(), model.clone())
            } else {
                GeminiLlmProvider::new(config.api_key.clone())
            };

            Arc::new(
                provider
                    .without_timeout()
                    .with_request_log_store(request_log_store.clone())
                    .with_thinking_budget(config.gemini_thinking_budget)
                    .with_thinking_level(config.gemini_thinking_level.clone()),
            )
        }
        "cohere" => {
            let provider = if let Some(model) = &config.model {
                CohereLlmProvider::with_model(config.api_key.clone(), model.clone())
            } else {
                CohereLlmProvider::new(config.api_key.clone())
            };
            Arc::new(
                provider
                    .without_timeout()
                    .with_request_log_store(request_log_store.clone()),
            )
        }
        "ollama" => {
            let provider = OllamaLlmProvider::with_url(
                config
                    .ollama_url
                    .clone()
                    .unwrap_or_else(|| "http://localhost:11434".to_string()),
                config.model.clone(),
            );
            Arc::new(
                provider
                    .without_timeout()
                    .with_request_log_store(request_log_store.clone()),
            )
        }
        _ => {
            // Default to OpenAI
            let provider = if let Some(model) = &config.model {
                OpenAiLlmProvider::with_model(config.api_key.clone(), model.clone())
            } else {
                OpenAiLlmProvider::new(config.api_key.clone())
            };
            Arc::new(
                provider
                    .without_timeout()
                    .with_request_log_store(request_log_store.clone())
                    .with_reasoning_effort(config.openai_reasoning_effort.clone()),
            )
        }
    }
}

/// Prompt configuration payload from frontend
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct PromptConfigPayload {
    /// Custom system prompt (null to use default)
    pub system_custom: Option<String>,
}

impl From<PromptConfigPayload> for PromptSections {
    fn from(payload: PromptConfigPayload) -> Self {
        Self {
            system_custom: payload.system_custom,
        }
    }
}

impl From<PromptSections> for PromptConfigPayload {
    fn from(sections: PromptSections) -> Self {
        Self {
            system_custom: sections.system_custom,
        }
    }
}

/// Get the default prompt templates
#[tauri::command]
pub fn get_llm_default_prompts() -> DefaultPromptsResponse {
    DefaultPromptsResponse {
        system: SYSTEM_PROMPT_DEFAULT.to_string(),
    }
}

/// Response containing default prompts
#[derive(Debug, serde::Serialize)]
pub struct DefaultPromptsResponse {
    pub system: String,
}

/// Get available LLM providers
#[tauri::command]
pub fn get_llm_providers() -> Vec<LlmProviderInfo> {
    vec![
        LlmProviderInfo {
            id: "openai".to_string(),
            name: "OpenAI".to_string(),
            requires_api_key: true,
            default_model: "gpt-5".to_string(),
            models: vec![
                "gpt-5.2".to_string(),
                "gpt-5.1".to_string(),
                "gpt-5".to_string(),
                "gpt-5-mini".to_string(),
                "gpt-5-nano".to_string(),
                "gpt-4.1".to_string(),
                "gpt-4.1-mini".to_string(),
                "gpt-4.1-nano".to_string(),
                // Older models kept for backwards compatibility.
                "gpt-4o-mini".to_string(),
                "gpt-4o".to_string(),
                "gpt-4-turbo".to_string(),
                "gpt-3.5-turbo".to_string(),
            ],
        },
        LlmProviderInfo {
            id: "gemini".to_string(),
            name: "Google AI Studio".to_string(),
            requires_api_key: true,
            default_model: "gemini-2.5-flash".to_string(),
            models: vec![
                // Gemini 3 (preview) - uses full model path IDs
                "models/gemini-3-pro-preview".to_string(),
                "models/gemini-3-flash-preview".to_string(),
                // Gemini 2.5 (stable)
                "gemini-2.5-pro".to_string(),
                "gemini-2.5-flash".to_string(),
                "gemini-2.5-flash-lite".to_string(),
            ],
        },
        LlmProviderInfo {
            id: "anthropic".to_string(),
            name: "Anthropic".to_string(),
            requires_api_key: true,
            default_model: "claude-3-haiku-20240307".to_string(),
            models: vec![
                "claude-sonnet-4-5".to_string(),
                "claude-haiku-4-5".to_string(),
                "claude-opus-4-5".to_string(),
                "claude-3-haiku-20240307".to_string(),
                "claude-3-sonnet-20240229".to_string(),
                "claude-3-opus-20240229".to_string(),
                "claude-3-5-sonnet-20241022".to_string(),
            ],
        },
        LlmProviderInfo {
            id: "groq".to_string(),
            name: "Groq".to_string(),
            requires_api_key: true,
            default_model: "llama-3.3-70b-versatile".to_string(),
            models: vec![
                "llama-3.3-70b-versatile".to_string(),
                "llama-3.1-8b-instant".to_string(),
                "openai/gpt-oss-120b".to_string(),
                "openai/gpt-oss-20b".to_string(),
            ],
        },
        LlmProviderInfo {
            id: "cohere".to_string(),
            name: "Cohere".to_string(),
            requires_api_key: true,
            default_model: "command-r-08-2024".to_string(),
            models: vec![
                "command-a-03-2025".to_string(),
                "command-r-plus-08-2024".to_string(),
                "command-r-08-2024".to_string(),
            ],
        },
        LlmProviderInfo {
            id: "ollama".to_string(),
            name: "Ollama".to_string(),
            requires_api_key: false,
            default_model: "llama3.2".to_string(),
            models: vec![
                "llama3.2".to_string(),
                "llama3.1".to_string(),
                "mistral".to_string(),
                "codellama".to_string(),
            ],
        },
    ]
}

/// Test LLM rewrite for the given transcript.
///
/// Uses the effective provider/model/prompts as configured in the pipeline config.
/// If `profile_id` matches a program prompt profile, its overrides are applied; otherwise
/// the Default profile is used.
#[tauri::command]
pub async fn test_llm_rewrite(
    app: AppHandle,
    pipeline: State<'_, SharedPipeline>,
    transcript: String,
    profile_id: Option<String>,
) -> Result<TestLlmRewriteResponse, LlmCommandError> {
    // Create a dedicated request-log entry for this test action.
    // This is intentionally *rewrite-only* (no STT step), and is the only way to have
    // a request log without an STT request/response.
    let llm_started_at = Instant::now();

    let request_log_store = app
        .try_state::<RequestLogStore>()
        .map(|s| s.inner().clone());

    if let Some(store) = request_log_store.as_ref() {
        store.start_request("rewrite-only".to_string(), None);
        store.with_current(|log| {
            log.raw_transcript = Some(transcript.clone());
            log.info("Test rewrite started");
        });
    }

    let config = pipeline.config();

    // Resolve the requested profile (if any). For unknown ids we error instead of silently
    // falling back to Default; this prevents confusing test results.
    let resolved_profile = profile_id
        .as_deref()
        .and_then(|id| if id == "default" { None } else { Some(id) })
        .map(|id| {
            config
                .llm_config
                .program_prompt_profiles
                .iter()
                .find(|p| p.id == id)
                .cloned()
                .ok_or_else(|| LlmCommandError::from(format!("Unknown profile_id: {}", id)))
        })
        .transpose()?;

    // Persist which profile was used into the request log so the Logs UI and raw payloads
    // are easier to interpret.
    if let Some(store) = request_log_store.as_ref() {
        let (used_id, used_name) = if let Some(p) = resolved_profile.as_ref() {
            (Some(p.id.clone()), Some(p.name.clone()))
        } else {
            (Some("default".to_string()), Some("Default".to_string()))
        };

        store.with_current(|log| {
            log.profile_id = used_id;
            log.profile_name = used_name;
        });
    }

    // IMPORTANT: This is a *test* endpoint. It intentionally ignores the
    // "Rewrite Transcription" enable toggle so users can validate prompts/
    // provider/model without changing runtime behavior.
    let (desired_provider, desired_model, prompts) = if let Some(profile) = resolved_profile.as_ref() {
        let provider = profile
            .llm_provider
            .clone()
            .unwrap_or_else(|| config.llm_config.provider.clone());
        let model = profile.llm_model.clone().or_else(|| config.llm_config.model.clone());

        (provider, model, profile.prompts.clone())
    } else {
        (
            config.llm_config.provider.clone(),
            config.llm_config.model.clone(),
            config.llm_config.prompts.clone(),
        )
    };

    let api_key = if desired_provider == "ollama" {
        String::new()
    } else {
        config
            .llm_api_keys
            .get(desired_provider.as_str())
            .cloned()
            .unwrap_or_default()
    };

    // Apply provider-specific thinking/reasoning knobs (profile overrides -> global defaults).
    let effective_openai_reasoning_effort = resolved_profile
        .as_ref()
        .and_then(|p| p.openai_reasoning_effort.clone())
        .or_else(|| config.llm_config.openai_reasoning_effort.clone());
    let effective_gemini_thinking_budget = resolved_profile
        .as_ref()
        .and_then(|p| p.gemini_thinking_budget)
        .or(config.llm_config.gemini_thinking_budget);
    let effective_gemini_thinking_level = resolved_profile
        .as_ref()
        .and_then(|p| p.gemini_thinking_level.clone())
        .or_else(|| config.llm_config.gemini_thinking_level.clone());
    let effective_anthropic_thinking_budget = resolved_profile
        .as_ref()
        .and_then(|p| p.anthropic_thinking_budget)
        .or(config.llm_config.anthropic_thinking_budget);

    let provider_cfg = LlmConfig {
        enabled: true,
        provider: desired_provider,
        api_key,
        model: desired_model,
        ollama_url: config.llm_config.ollama_url.clone(),
        openai_reasoning_effort: effective_openai_reasoning_effort,
        gemini_thinking_budget: effective_gemini_thinking_budget,
        gemini_thinking_level: effective_gemini_thinking_level,
        anthropic_thinking_budget: effective_anthropic_thinking_budget,
        prompts: PromptSections::default(),
        program_prompt_profiles: Vec::new(),
        timeout: config.llm_config.timeout,
    };

    // This is a *test* endpoint: do not enforce request timeouts.
    let provider = create_llm_provider_without_timeout(&provider_cfg, request_log_store.clone());

    let output_res = format_text(provider.as_ref(), &transcript, &prompts).await;

    match output_res {
        Ok(output) => {
            if let Some(store) = request_log_store.as_ref() {
                let llm_duration_ms = llm_started_at.elapsed().as_millis() as u64;
                let provider_used = provider.name().to_string();
                let model_used = provider.model().to_string();

                store.with_current(|log| {
                    log.llm_provider = Some(provider_used.clone());
                    log.llm_model = Some(model_used.clone());
                    log.formatted_transcript = Some(output.clone());
                    log.llm_duration_ms = Some(llm_duration_ms);
                    log.info(format!(
                        "Test rewrite completed in {}ms ({} -> {} chars)",
                        llm_duration_ms,
                        transcript.len(),
                        output.len()
                    ));
                    log.complete_success();
                });

                // Best-effort: emit LLM cost event for this request log.
                crate::stats::emit_cost_events_for_current_request(
                    &app,
                    EventStatus::Success,
                    None,
                );

                store.complete_current();

                return Ok(TestLlmRewriteResponse {
                    output,
                    provider_used,
                    model_used,
                });
            }

            // Best-effort: emit LLM cost event for the current request log (if any).
            crate::stats::emit_cost_events_for_current_request(&app, EventStatus::Success, None);

            Ok(TestLlmRewriteResponse {
                output,
                provider_used: provider.name().to_string(),
                model_used: provider.model().to_string(),
            })
        }
        Err(e) => {
            if let Some(store) = request_log_store.as_ref() {
                let llm_duration_ms = llm_started_at.elapsed().as_millis() as u64;
                store.with_current(|log| {
                    log.llm_duration_ms = Some(llm_duration_ms);
                    log.error(format!("Test rewrite failed: {}", e));
                    log.complete_error(e.to_string());
                });

                crate::stats::emit_cost_events_for_current_request(
                    &app,
                    EventStatus::Error,
                    None,
                );

                store.complete_current();
            } else {
                crate::stats::emit_cost_events_for_current_request(&app, EventStatus::Error, None);
            }

            Err(LlmCommandError::from(e.to_string()))
        }
    }
}

/// Iterate on the rewrite system prompt using an example transcript + before/after outputs.
///
/// This command is designed for the Settings "Prompt lab" UI.
/// It creates a dedicated request-log entry so provider/model + request/response payloads
/// are visible in Request Logs.
#[tauri::command]
pub async fn iterate_rewrite_prompt(
    app: AppHandle,
    pipeline: State<'_, SharedPipeline>,
    transcript: String,
    problem_output: String,
    desired_output: Option<String>,
    current_prompt: String,
    profile_id: Option<String>,
    mode: Option<String>,

    // Optional Prompt Lab overrides (do not persist; only affect this invocation).
    llm_provider: Option<String>,
    llm_model: Option<String>,
    open_ai_reasoning_effort: Option<String>,
    gemini_thinking_level: Option<String>,
    gemini_thinking_budget: Option<i64>,
    anthropic_thinking_budget: Option<i64>,
) -> Result<IterateRewritePromptResponse, LlmCommandError> {
    let llm_started_at = Instant::now();

    let request_log_store = app
        .try_state::<RequestLogStore>()
        .map(|s| s.inner().clone());

    if let Some(store) = request_log_store.as_ref() {
        store.start_request("prompt-iter".to_string(), None);
        store.with_current(|log| {
            log.raw_transcript = Some(transcript.clone());
            let mode_label = mode
                .as_deref()
                .unwrap_or("fixed")
                .to_string();
            log.info(format!("Prompt iteration started (mode: {})", mode_label));
        });
    }

    let config = pipeline.config();

    let resolved_profile = profile_id
        .as_deref()
        .and_then(|id| if id == "default" { None } else { Some(id) })
        .map(|id| {
            config
                .llm_config
                .program_prompt_profiles
                .iter()
                .find(|p| p.id == id)
                .cloned()
                .ok_or_else(|| LlmCommandError::from(format!("Unknown profile_id: {}", id)))
        })
        .transpose()?;

    if let Some(store) = request_log_store.as_ref() {
        let (used_id, used_name) = if let Some(p) = resolved_profile.as_ref() {
            (Some(p.id.clone()), Some(p.name.clone()))
        } else {
            (Some("default".to_string()), Some("Default".to_string()))
        };

        store.with_current(|log| {
            log.profile_id = used_id;
            log.profile_name = used_name;
        });
    }

    let (base_provider, base_model) = if let Some(profile) = resolved_profile.as_ref() {
        let provider = profile
            .llm_provider
            .clone()
            .unwrap_or_else(|| config.llm_config.provider.clone());
        let model = profile.llm_model.clone().or_else(|| config.llm_config.model.clone());
        (provider, model)
    } else {
        (config.llm_config.provider.clone(), config.llm_config.model.clone())
    };

    let desired_provider = llm_provider
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or(base_provider);

    let desired_model = llm_model
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| Some(s.to_string()))
        .unwrap_or(base_model);

    let api_key = if desired_provider == "ollama" {
        String::new()
    } else {
        config
            .llm_api_keys
            .get(desired_provider.as_str())
            .cloned()
            .unwrap_or_default()
    };

    // Apply provider-specific thinking/reasoning knobs.
    // Precedence: Prompt Lab override -> profile override -> global defaults.
    let effective_openai_reasoning_effort = open_ai_reasoning_effort
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .or_else(|| {
            resolved_profile
                .as_ref()
                .and_then(|p| p.openai_reasoning_effort.clone())
        })
        .or_else(|| config.llm_config.openai_reasoning_effort.clone());

    let effective_gemini_thinking_budget = gemini_thinking_budget
        .or_else(|| resolved_profile.as_ref().and_then(|p| p.gemini_thinking_budget))
        .or(config.llm_config.gemini_thinking_budget);

    let effective_gemini_thinking_level = gemini_thinking_level
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .or_else(|| {
            resolved_profile
                .as_ref()
                .and_then(|p| p.gemini_thinking_level.clone())
        })
        .or_else(|| config.llm_config.gemini_thinking_level.clone());

    let effective_anthropic_thinking_budget = anthropic_thinking_budget
        .or_else(|| resolved_profile.as_ref().and_then(|p| p.anthropic_thinking_budget))
        .or(config.llm_config.anthropic_thinking_budget);

    let provider_cfg = LlmConfig {
        enabled: true,
        provider: desired_provider,
        api_key,
        model: desired_model,
        ollama_url: config.llm_config.ollama_url.clone(),
        openai_reasoning_effort: effective_openai_reasoning_effort,
        gemini_thinking_budget: effective_gemini_thinking_budget,
        gemini_thinking_level: effective_gemini_thinking_level,
        anthropic_thinking_budget: effective_anthropic_thinking_budget,
        prompts: PromptSections::default(),
        program_prompt_profiles: Vec::new(),
        timeout: config.llm_config.timeout,
    };

    // This is a Settings UI helper: do not enforce request timeouts.
    let provider = create_llm_provider_without_timeout(&provider_cfg, request_log_store.clone());

    let mode = mode.as_deref().unwrap_or("fixed");

    // Desired output is optional for "new" mode, but required for "fixed" mode.
    let desired_output_trimmed = desired_output
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());

    let transcript_trimmed = transcript.trim();
    let goal_trimmed = problem_output.trim();

    if mode != "new" && desired_output_trimmed.is_none() {
        return Err(LlmCommandError::from(
            "Desired output is required when improving an existing prompt".to_string(),
        ));
    }

    if mode == "new" {
        let has_goal = !goal_trimmed.is_empty();
        let has_example_pair = !transcript_trimmed.is_empty() && desired_output_trimmed.is_some();

        if !has_goal && !has_example_pair {
            return Err(LlmCommandError::from(
                "For New prompt, provide either a prompt goal/description, or both transcript and desired output".to_string(),
            ));
        }
    }

    let system_prompt = match mode {
        "new" => "You write system prompts for rewriting dictation transcripts.\n\nReturn ONLY the system prompt text.\n- No markdown\n- No quotes\n- No greetings\n- No explanations\n- The result must be usable as a system prompt as-is",
        _ => "You improve system prompts for rewriting dictation transcripts.\n\nReturn ONLY the improved system prompt text.\n- No markdown\n- No quotes\n- No greetings\n- No explanations\n- The result must be usable as a system prompt as-is",
    };

    let user_message = match mode {
        "new" => {
            let mut msg = String::new();

            if !transcript_trimmed.is_empty() {
                msg.push_str("Transcript (input):\n<<<\n");
                msg.push_str(transcript_trimmed);
                msg.push_str("\n>>>\n\n");
            }

            if !goal_trimmed.is_empty() {
                msg.push_str("Prompt goal / description:\n<<<\n");
                msg.push_str(goal_trimmed);
                msg.push_str("\n>>>\n\n");
            }

            if let Some(desired) = desired_output_trimmed {
                msg.push_str("Desired output:\n<<<\n");
                msg.push_str(desired);
                msg.push_str("\n>>>\n\n");
            }

            msg.push_str("Existing prompt (reference; may be ignored):\n<<<\n");
            msg.push_str(current_prompt.trim());
            msg.push_str("\n>>>\n\nTask:\nWrite a NEW system prompt from scratch for rewriting dictation transcripts. Use the goal/description and/or the example transcript/output (if provided) as guidance. Be specific and include clear rules. Produce a complete prompt.");

            msg
        }
        _ => format!(
            "Current system prompt:\n<<<\n{current_prompt}\n>>>\n\nTranscript (input):\n<<<\n{transcript}\n>>>\n\nProblem output (current prompt produced):\n<<<\n{problem_output}\n>>>\n\nDesired output:\n<<<\n{desired_output}\n>>>\n\nTask:\nWrite an improved system prompt that would transform the transcript into the desired output more reliably. Preserve the original style and scope. Add or adjust rules only as needed.",
            current_prompt = current_prompt.trim(),
            transcript = transcript_trimmed,
            problem_output = problem_output.trim(),
            desired_output = desired_output_trimmed.unwrap_or(""),
        ),
    };

    let improved_res = provider.complete(system_prompt, &user_message).await;

    match improved_res {
        Ok(improved_raw) => {
            let improved = improved_raw.trim().to_string();

            if let Some(store) = request_log_store.as_ref() {
                let llm_duration_ms = llm_started_at.elapsed().as_millis() as u64;
                let provider_used = provider.name().to_string();
                let model_used = provider.model().to_string();

                store.with_current(|log| {
                    log.llm_provider = Some(provider_used.clone());
                    log.llm_model = Some(model_used.clone());
                    // Use the standard rewrite fields so the Logs UI can show the content.
                    log.formatted_transcript = Some(improved.clone());
                    log.llm_duration_ms = Some(llm_duration_ms);
                    log.info(format!(
                        "Prompt iteration completed in {}ms ({} chars)",
                        llm_duration_ms,
                        improved.len()
                    ));
                    log.complete_success();
                });

                crate::stats::emit_cost_events_for_current_request(
                    &app,
                    EventStatus::Success,
                    None,
                );
                store.complete_current();

                return Ok(IterateRewritePromptResponse {
                    improved_prompt: improved,
                    provider_used,
                    model_used,
                });
            }

            crate::stats::emit_cost_events_for_current_request(&app, EventStatus::Success, None);

            Ok(IterateRewritePromptResponse {
                improved_prompt: improved,
                provider_used: provider.name().to_string(),
                model_used: provider.model().to_string(),
            })
        }
        Err(e) => {
            if let Some(store) = request_log_store.as_ref() {
                let llm_duration_ms = llm_started_at.elapsed().as_millis() as u64;
                store.with_current(|log| {
                    log.llm_duration_ms = Some(llm_duration_ms);
                    log.error(format!("Prompt iteration failed: {}", e));
                    log.complete_error(e.to_string());
                });

                crate::stats::emit_cost_events_for_current_request(
                    &app,
                    EventStatus::Error,
                    None,
                );
                store.complete_current();
            } else {
                crate::stats::emit_cost_events_for_current_request(&app, EventStatus::Error, None);
            }

            Err(LlmCommandError::from(e.to_string()))
        }
    }
}

/// Test a custom system prompt against a transcript.
///
/// This mirrors the rewrite step (system prompt + transcript -> output) but allows passing an
/// arbitrary system prompt string.
#[tauri::command]
pub async fn test_rewrite_with_prompt(
    app: AppHandle,
    pipeline: State<'_, SharedPipeline>,
    transcript: String,
    prompt: String,
    profile_id: Option<String>,
) -> Result<TestRewriteWithPromptResponse, LlmCommandError> {
    let llm_started_at = Instant::now();

    let request_log_store = app
        .try_state::<RequestLogStore>()
        .map(|s| s.inner().clone());

    if let Some(store) = request_log_store.as_ref() {
        store.start_request("prompt-test".to_string(), None);
        store.with_current(|log| {
            log.raw_transcript = Some(transcript.clone());
            log.info("Prompt test started");
        });
    }

    let config = pipeline.config();

    let resolved_profile = profile_id
        .as_deref()
        .and_then(|id| if id == "default" { None } else { Some(id) })
        .map(|id| {
            config
                .llm_config
                .program_prompt_profiles
                .iter()
                .find(|p| p.id == id)
                .cloned()
                .ok_or_else(|| LlmCommandError::from(format!("Unknown profile_id: {}", id)))
        })
        .transpose()?;

    if let Some(store) = request_log_store.as_ref() {
        let (used_id, used_name) = if let Some(p) = resolved_profile.as_ref() {
            (Some(p.id.clone()), Some(p.name.clone()))
        } else {
            (Some("default".to_string()), Some("Default".to_string()))
        };

        store.with_current(|log| {
            log.profile_id = used_id;
            log.profile_name = used_name;
        });
    }

    let (desired_provider, desired_model) = if let Some(profile) = resolved_profile.as_ref() {
        let provider = profile
            .llm_provider
            .clone()
            .unwrap_or_else(|| config.llm_config.provider.clone());
        let model = profile.llm_model.clone().or_else(|| config.llm_config.model.clone());
        (provider, model)
    } else {
        (config.llm_config.provider.clone(), config.llm_config.model.clone())
    };

    let api_key = if desired_provider == "ollama" {
        String::new()
    } else {
        config
            .llm_api_keys
            .get(desired_provider.as_str())
            .cloned()
            .unwrap_or_default()
    };

    let effective_openai_reasoning_effort = resolved_profile
        .as_ref()
        .and_then(|p| p.openai_reasoning_effort.clone())
        .or_else(|| config.llm_config.openai_reasoning_effort.clone());
    let effective_gemini_thinking_budget = resolved_profile
        .as_ref()
        .and_then(|p| p.gemini_thinking_budget)
        .or(config.llm_config.gemini_thinking_budget);
    let effective_gemini_thinking_level = resolved_profile
        .as_ref()
        .and_then(|p| p.gemini_thinking_level.clone())
        .or_else(|| config.llm_config.gemini_thinking_level.clone());
    let effective_anthropic_thinking_budget = resolved_profile
        .as_ref()
        .and_then(|p| p.anthropic_thinking_budget)
        .or(config.llm_config.anthropic_thinking_budget);

    let provider_cfg = LlmConfig {
        enabled: true,
        provider: desired_provider,
        api_key,
        model: desired_model,
        ollama_url: config.llm_config.ollama_url.clone(),
        openai_reasoning_effort: effective_openai_reasoning_effort,
        gemini_thinking_budget: effective_gemini_thinking_budget,
        gemini_thinking_level: effective_gemini_thinking_level,
        anthropic_thinking_budget: effective_anthropic_thinking_budget,
        prompts: PromptSections::default(),
        program_prompt_profiles: Vec::new(),
        timeout: config.llm_config.timeout,
    };

    let provider = create_llm_provider_without_timeout(&provider_cfg, request_log_store.clone());

    let output_res = provider.complete(prompt.as_str(), transcript.as_str()).await;

    match output_res {
        Ok(output_raw) => {
            let output = output_raw.trim().to_string();

            if let Some(store) = request_log_store.as_ref() {
                let llm_duration_ms = llm_started_at.elapsed().as_millis() as u64;
                let provider_used = provider.name().to_string();
                let model_used = provider.model().to_string();

                store.with_current(|log| {
                    log.llm_provider = Some(provider_used.clone());
                    log.llm_model = Some(model_used.clone());
                    log.formatted_transcript = Some(output.clone());
                    log.llm_duration_ms = Some(llm_duration_ms);
                    log.info(format!(
                        "Prompt test completed in {}ms ({} -> {} chars)",
                        llm_duration_ms,
                        transcript.len(),
                        output.len()
                    ));
                    log.complete_success();
                });

                crate::stats::emit_cost_events_for_current_request(
                    &app,
                    EventStatus::Success,
                    None,
                );
                store.complete_current();

                return Ok(TestRewriteWithPromptResponse {
                    output,
                    provider_used,
                    model_used,
                });
            }

            crate::stats::emit_cost_events_for_current_request(&app, EventStatus::Success, None);

            Ok(TestRewriteWithPromptResponse {
                output,
                provider_used: provider.name().to_string(),
                model_used: provider.model().to_string(),
            })
        }
        Err(e) => {
            if let Some(store) = request_log_store.as_ref() {
                let llm_duration_ms = llm_started_at.elapsed().as_millis() as u64;
                store.with_current(|log| {
                    log.llm_duration_ms = Some(llm_duration_ms);
                    log.error(format!("Prompt test failed: {}", e));
                    log.complete_error(e.to_string());
                });

                crate::stats::emit_cost_events_for_current_request(
                    &app,
                    EventStatus::Error,
                    None,
                );
                store.complete_current();
            } else {
                crate::stats::emit_cost_events_for_current_request(&app, EventStatus::Error, None);
            }

            Err(LlmCommandError::from(e.to_string()))
        }
    }
}

/// Run a one-off LLM completion with explicit provider/model and explicit prompts.
///
/// This is used by the History UI to send analysis instructions as the *system prompt*
/// and the transcript bundle as the *user prompt*.
#[tauri::command]
pub async fn llm_complete(
    app: AppHandle,
    pipeline: State<'_, SharedPipeline>,
    args: LlmCompleteArgs,
) -> Result<LlmCompleteResponse, LlmCommandError> {
    let config = pipeline.config();

    let desired_provider = args.provider;
    let desired_model = args.model;

    let api_key = if desired_provider == "ollama" {
        String::new()
    } else {
        config
            .llm_api_keys
            .get(desired_provider.as_str())
            .cloned()
            .unwrap_or_default()
    };

    if desired_provider != "ollama" && api_key.trim().is_empty() {
        return Err(LlmCommandError::from(format!(
            "No API key configured for provider: {}",
            desired_provider
        )));
    }

    let provider_cfg = LlmConfig {
        enabled: true,
        provider: desired_provider,
        api_key,
        model: desired_model,
        ollama_url: config.llm_config.ollama_url.clone(),
        openai_reasoning_effort: config.llm_config.openai_reasoning_effort.clone(),
        gemini_thinking_budget: config.llm_config.gemini_thinking_budget,
        gemini_thinking_level: config.llm_config.gemini_thinking_level.clone(),
        anthropic_thinking_budget: config.llm_config.anthropic_thinking_budget,
        prompts: PromptSections::default(),
        program_prompt_profiles: Vec::new(),
        timeout: config.llm_config.timeout,
    };

    let provider = create_llm_provider_unstructured(&provider_cfg);
    let output = provider
        .complete(args.system_prompt.as_str(), args.user_prompt.as_str())
        .await
        .map_err(|e| LlmCommandError::from(e.to_string()))?;

    // Best-effort: emit LLM cost event for the current request log (if any).
    crate::stats::emit_cost_events_for_current_request(&app, EventStatus::Success, None);

    Ok(LlmCompleteResponse {
        output: output.trim().to_string(),
        provider_used: provider.name().to_string(),
        model_used: provider.model().to_string(),
    })
}

/// LLM provider information for the frontend
#[derive(Debug, serde::Serialize)]
pub struct LlmProviderInfo {
    pub id: String,
    pub name: String,
    pub requires_api_key: bool,
    pub default_model: String,
    pub models: Vec<String>,
}

/// Update LLM configuration on the pipeline
#[tauri::command]
pub fn update_llm_config(
    pipeline: State<'_, SharedPipeline>,
    config: LlmConfigPayload,
) -> Result<(), LlmCommandError> {
    // Get current pipeline config and update just the LLM portion
    // Note: This is a simplified approach - in a full implementation,
    // we'd want to preserve other config and only update LLM settings
    let llm_config = LlmConfig {
        enabled: config.enabled,
        provider: config.provider,
        api_key: config.api_key.unwrap_or_default(),
        model: config.model,
        ollama_url: config.ollama_url,
        openai_reasoning_effort: None,
        gemini_thinking_budget: None,
        gemini_thinking_level: None,
        anthropic_thinking_budget: None,
        prompts: PromptSections::default(),
        program_prompt_profiles: Vec::new(),
        timeout: Duration::from_secs(config.timeout_secs.unwrap_or(30)),
    };

    // Get current config from pipeline and update LLM portion
    let current_config = get_current_pipeline_config(&pipeline)?;
    let new_config = crate::pipeline::PipelineConfig {
        llm_config,
        ..current_config
    };

    pipeline
        .update_config(new_config)
        .map_err(|e| LlmCommandError::from(e.to_string()))?;

    log::info!("LLM configuration updated");
    Ok(())
}

/// Update LLM prompt configuration
#[tauri::command]
pub fn update_llm_prompts(
    pipeline: State<'_, SharedPipeline>,
    prompts: PromptConfigPayload,
) -> Result<(), LlmCommandError> {
    let current_config = get_current_pipeline_config(&pipeline)?;
    let mut llm_config = current_config.llm_config.clone();
    llm_config.prompts = prompts.into();

    let new_config = crate::pipeline::PipelineConfig {
        llm_config,
        ..current_config
    };

    pipeline
        .update_config(new_config)
        .map_err(|e| LlmCommandError::from(e.to_string()))?;

    log::info!("LLM prompts updated");
    Ok(())
}

/// Get current LLM configuration
#[tauri::command]
pub fn get_llm_config(pipeline: State<'_, SharedPipeline>) -> Result<LlmConfigResponse, LlmCommandError> {
    let config = get_current_pipeline_config(&pipeline)?;
    Ok(LlmConfigResponse {
        enabled: config.llm_config.enabled,
        provider: config.llm_config.provider,
        model: config.llm_config.model,
        ollama_url: config.llm_config.ollama_url,
        timeout_secs: config.llm_config.timeout.as_secs(),
        prompts: config.llm_config.prompts.into(),
    })
}

/// Response containing current LLM configuration
#[derive(Debug, serde::Serialize)]
pub struct LlmConfigResponse {
    pub enabled: bool,
    pub provider: String,
    pub model: Option<String>,
    pub ollama_url: Option<String>,
    pub timeout_secs: u64,
    pub prompts: PromptConfigPayload,
}

/// Helper to get current pipeline config (placeholder - needs proper implementation)
fn get_current_pipeline_config(
    _pipeline: &State<'_, SharedPipeline>,
) -> Result<crate::pipeline::PipelineConfig, LlmCommandError> {
    // Note: The current SharedPipeline doesn't expose config reading
    // For now, return default config. In a full implementation, we'd
    // add a get_config() method to SharedPipeline.
    Ok(crate::pipeline::PipelineConfig::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_llm_providers() {
        let providers = get_llm_providers();
        assert_eq!(providers.len(), 6);
        assert!(providers.iter().any(|p| p.id == "openai"));
        assert!(providers.iter().any(|p| p.id == "gemini"));
        assert!(providers.iter().any(|p| p.id == "anthropic"));
        assert!(providers.iter().any(|p| p.id == "groq"));
        assert!(providers.iter().any(|p| p.id == "cohere"));
        assert!(providers.iter().any(|p| p.id == "ollama"));
    }

    #[test]
    fn test_get_default_prompts() {
        let prompts = get_llm_default_prompts();
        assert!(!prompts.system.is_empty());
    }
}
