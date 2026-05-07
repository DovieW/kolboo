//! Runtime execution for Quick Ask and Quick Replace.
//!
//! `quick_action_lifecycle` owns the pure vocabulary and fallback rules, while
//! `context_collection` owns selection/clipboard/OCR source mechanics. This module owns the
//! side-effectful request lifecycle around those inputs: provider readiness, request-log updates,
//! stats emission, and OCR cleanup. Keeping the Quick Action execution paths together prevents
//! future changes from updating Quick Ask while forgetting Quick Replace (or vice versa).

use std::time::Instant;

use serde_json::json;
use tauri::{AppHandle, Manager};

use crate::clipboard_context;
use crate::event_payloads::{
    QuickAskAnswerErrorPayload, QuickAskAnswerOkPayload, QuickAskAnswerPayload,
    QuickAskStartedPayload,
};
use crate::llm;
use crate::pipeline::{OcrConfig, SharedPipeline, TranscriptionResult};
use crate::prompt_builders;
use crate::request_log::{RequestLogStore, RequestStatus};
use crate::sessions::quick_action_lifecycle::{
    QuickActionKind, QuickActionProbePlan, QuickAskEffectiveConfig, QuickAskGlobalConfig,
    QuickAskProfileConfig, QuickReplaceConfig,
};
use crate::sessions::recording_finalization;
use crate::sessions::{context_collection, quick_ask};
use crate::settings::store::SettingsReadMode;
use crate::settings_view;
use crate::state::QuickAskConversationMemory;
use crate::stats;

/// Input bundle for the Quick Ask answer step.
///
/// The request has already completed STT at this point. Quick Ask claims ownership of the
/// request and intentionally prevents normal dictation output from running.
pub(crate) struct QuickAskExecution<'a> {
    pub(crate) app: &'a AppHandle,
    pub(crate) pipeline: &'a SharedPipeline,
    pub(crate) request_id: Option<&'a str>,
    pub(crate) result: &'a TranscriptionResult,
    pub(crate) fallback_text: &'a str,
    pub(crate) profile_config: &'a QuickAskProfileConfig,
    pub(crate) probe_plan: QuickActionProbePlan,
    pub(crate) ocr_mode: &'a str,
    pub(crate) ocr_config: &'a OcrConfig,
}

/// Output from the Quick Replace rewrite attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct QuickReplaceExecutionResult {
    pub(crate) output_value: String,
    pub(crate) failure: Option<String>,
}

/// Input bundle for the Quick Replace rewrite step.
pub(crate) struct QuickReplaceExecution<'a> {
    pub(crate) app: &'a AppHandle,
    pub(crate) pipeline: &'a SharedPipeline,
    pub(crate) request_id: Option<&'a str>,
    pub(crate) config: &'a QuickReplaceConfig,
    pub(crate) probe_plan: QuickActionProbePlan,
    pub(crate) ocr_mode: &'a str,
    pub(crate) ocr_config: &'a OcrConfig,
    pub(crate) output_value: String,
}

/// Complete the current request log, emit cost stats, and end the OCR session.
///
/// This is the lifecycle cleanup shape shared by Quick Ask, Quick Replace, normal success, and
/// errors. Keeping it tiny and boring is deliberate: lifecycle bugs here show up as stuck logs,
/// missing cost events, or OCR text leaking into the next request.
pub(crate) fn complete_current_request_with_cost(
    app: &AppHandle,
    pipeline: &SharedPipeline,
    request_id: Option<&str>,
    status: stats::EventStatus,
) {
    recording_finalization::complete_current_request_with_pipeline_wav(
        app, pipeline, request_id, status,
    );
}

/// Resolve the settings-store side of Quick Ask config.
///
/// The pure precedence rule lives in `QuickAskEffectiveConfig::resolve`; this helper deliberately
/// only reads persisted settings so `lib.rs` no longer repeats the same long list of keys.
pub(crate) fn resolve_quick_ask_config(
    app: &AppHandle,
    profile_config: &QuickAskProfileConfig,
) -> QuickAskEffectiveConfig {
    let global: QuickAskGlobalConfig =
        settings_view::read_quick_ask_global_config(app, SettingsReadMode::Cached);
    QuickAskEffectiveConfig::resolve(profile_config, global)
}

pub(crate) async fn answer_quick_ask(input: QuickAskExecution<'_>) {
    let question = crate::sanitize_transcript(&input.result.stt_text)
        .unwrap_or_else(|| input.fallback_text.to_string())
        .trim()
        .to_string();

    if question.is_empty() {
        complete_quick_ask_empty_transcript_error(input.app, input.pipeline, input.request_id);
        return;
    }

    let quick_ask_config = resolve_quick_ask_config(input.app, input.profile_config);
    let provider = quick_ask_config.provider.clone();
    let model = quick_ask_config.model.clone();
    let system_prompt = quick_ask_config.system_prompt.clone();

    if let Some(log_store) = input.app.try_state::<RequestLogStore>() {
        log_store.with_current(|log| {
            log.kind = QuickActionKind::QuickAsk.request_kind();
            log.quick_ask_question = Some(question.clone());
            log.quick_ask_provider = Some(provider.clone());
            log.quick_ask_model = model.clone();
            log.quick_ask_request_json = Some(json!({
                "system_prompt": system_prompt.clone(),
                "question": question.clone(),
                "ocr_mode": input.ocr_mode,
                "provider": provider.clone(),
                "model": model.clone(),
            }));
            log.info("Quick Ask: starting answer generation");
        });
    }

    quick_ask::emit_to_quick_ask(
        input.app,
        quick_ask::EVENT_QUICK_ASK_STARTED,
        QuickAskStartedPayload {
            question: Some(question.clone()),
            provider: Some(provider.clone()),
            model: model.clone(),
        },
    );

    let pipeline_config = input.pipeline.config();
    let provider_impl =
        match crate::pipeline::llm_provider::create_one_off_llm_provider_unstructured(
            &pipeline_config.llm_config,
            &pipeline_config.llm_api_keys,
            provider.as_str(),
            crate::pipeline::llm_provider::LlmProviderParams {
                model: model.clone(),
                timeout: pipeline_config.llm_config.timeout,
                ollama_url: pipeline_config.llm_config.ollama_url.clone(),
                openai_reasoning_effort: quick_ask_config.openai_reasoning_effort.clone(),
                gemini_thinking_budget: quick_ask_config.gemini_thinking_budget,
                gemini_thinking_level: quick_ask_config.gemini_thinking_level.clone(),
                anthropic_thinking_budget: quick_ask_config.anthropic_thinking_budget,
            },
        ) {
            Ok(provider) => provider,
            Err(e) => {
                let err = e.to_string();
                if let Some(log_store) = input.app.try_state::<RequestLogStore>() {
                    log_store.with_current(|log| {
                        log.kind = QuickActionKind::QuickAsk.request_kind();
                        log.error(format!("Quick Ask failed: {}", err));
                        log.complete_error(err.clone());
                    });
                }

                complete_current_request_with_cost(
                    input.app,
                    input.pipeline,
                    input.request_id,
                    stats::EventStatus::Error,
                );

                quick_ask::emit_to_quick_ask(
                    input.app,
                    quick_ask::EVENT_QUICK_ASK_ANSWER,
                    QuickAskAnswerPayload::Err(QuickAskAnswerErrorPayload {
                        ok: false,
                        error: err,
                    }),
                );
                return;
            }
        };

    let request_context = context_collection::collect_quick_ask_context(
        input.app,
        input.probe_plan,
        &quick_ask_config,
    )
    .await;
    let selected_context_trimmed = request_context.selected_text_for_prompt();
    let surrounding_context_trimmed = request_context.surrounding_text_for_prompt();
    let clipboard_trimmed = request_context.clipboard_context_for_prompt();

    let cap = 8_000usize;
    let selected_context_capped =
        clipboard_context::cap_optional_context_text_for_prompt(selected_context_trimmed, cap);
    let surrounding_context_capped =
        clipboard_context::cap_optional_context_text_for_prompt(surrounding_context_trimmed, cap);
    let clipboard_context_capped =
        clipboard_context::cap_optional_context_text_for_prompt(clipboard_trimmed, cap);

    // This is the exact context text (if any) attached to the question and surfaced in logs/UI.
    let quick_ask_context_text_for_log = selected_context_capped
        .clone()
        .or_else(|| surrounding_context_capped.clone());
    let quick_ask_clipboard_context_for_log = clipboard_context_capped.clone();

    let ocr_context = crate::sessions::ocr_usage::collect_ocr_context(
        input.pipeline,
        input.ocr_mode,
        input.ocr_config,
    )
    .await;
    let ocr_text = ocr_context.text().map(str::to_string);

    record_quick_ask_ocr_start_context(input.app, &ocr_context);

    let question_with_context = prompt_builders::build_quick_ask_user_message_with_context(
        question.as_str(),
        selected_context_capped.as_deref(),
        surrounding_context_capped.as_deref(),
        clipboard_context_capped.as_deref(),
        ocr_text.as_deref(),
    );

    let question_with_context = if quick_ask_config.conversation_history_enabled {
        prepend_quick_ask_history(
            input.app,
            question_with_context,
            quick_ask_config.conversation_history_count,
        )
    } else {
        question_with_context
    };

    record_quick_ask_prompt_context(
        input.app,
        input.pipeline,
        &quick_ask_config,
        &question_with_context,
        quick_ask_context_text_for_log,
        quick_ask_clipboard_context_for_log,
        selected_context_trimmed,
        surrounding_context_trimmed,
        clipboard_trimmed,
        ocr_text.clone(),
    );

    let t0 = Instant::now();
    match provider_impl
        .complete(system_prompt.as_str(), question_with_context.as_str())
        .await
    {
        Ok(answer) => {
            let answer = answer.trim().to_string();
            let duration_ms = t0.elapsed().as_millis() as u64;

            if let Some(memory) = input.app.try_state::<QuickAskConversationMemory>() {
                memory.push_turn(question.clone(), answer.clone());
            }

            if let Some(log_store) = input.app.try_state::<RequestLogStore>() {
                log_store.with_current(|log| {
                    log.kind = QuickActionKind::QuickAsk.request_kind();
                    log.quick_ask_answer = Some(answer.clone());
                    log.quick_ask_provider = Some(provider_impl.name().to_string());
                    log.quick_ask_model = Some(provider_impl.model().to_string());
                    log.quick_ask_duration_ms = Some(duration_ms);
                    log.quick_ask_response_json = Some(json!({
                        "ok": true,
                        "answer": answer.clone(),
                        "provider_used": provider_impl.name(),
                        "model_used": provider_impl.model(),
                        "duration_ms": duration_ms,
                    }));
                    log.complete_success();
                });
            }

            complete_current_request_with_cost(
                input.app,
                input.pipeline,
                input.request_id,
                stats::EventStatus::Success,
            );

            quick_ask::emit_to_quick_ask(
                input.app,
                quick_ask::EVENT_QUICK_ASK_ANSWER,
                QuickAskAnswerPayload::Ok(QuickAskAnswerOkPayload {
                    ok: true,
                    answer,
                    provider_used: Some(provider_impl.name().to_string()),
                    model_used: Some(provider_impl.model().to_string()),
                    duration_ms: Some(duration_ms),
                }),
            );

            // Show the Quick Ask window only once we have something to display.
            quick_ask::ensure_quick_ask_window_visible(input.app);
        }
        Err(e) => {
            let err = e.to_string();
            if let Some(log_store) = input.app.try_state::<RequestLogStore>() {
                log_store.with_current(|log| {
                    log.kind = QuickActionKind::QuickAsk.request_kind();
                    log.quick_ask_answer = None;
                    log.quick_ask_response_json = Some(json!({
                        "ok": false,
                        "error": err.clone(),
                    }));
                    log.error(format!("Quick Ask failed: {}", err.clone()));
                    log.complete_error(err.clone());
                });
            }

            complete_current_request_with_cost(
                input.app,
                input.pipeline,
                input.request_id,
                stats::EventStatus::Error,
            );

            quick_ask::emit_to_quick_ask(
                input.app,
                quick_ask::EVENT_QUICK_ASK_ANSWER,
                QuickAskAnswerPayload::Err(QuickAskAnswerErrorPayload {
                    ok: false,
                    error: err,
                }),
            );
        }
    }
}

pub(crate) fn complete_quick_ask_empty_transcript_error(
    app: &AppHandle,
    pipeline: &SharedPipeline,
    request_id: Option<&str>,
) {
    if let Some(log_store) = app.try_state::<RequestLogStore>() {
        log_store.with_current(|log| {
            log.kind = QuickActionKind::QuickAsk.request_kind();
            log.quick_ask_question = Some(String::new());
            log.quick_ask_answer = None;
            log.error("Quick Ask failed: no transcript to answer (empty)");
            log.complete_error("No transcript to answer (empty)");
        });
    }

    complete_current_request_with_cost(app, pipeline, request_id, stats::EventStatus::Error);

    quick_ask::emit_to_quick_ask(
        app,
        quick_ask::EVENT_QUICK_ASK_STARTED,
        QuickAskStartedPayload {
            question: Some(String::new()),
            provider: None,
            model: None,
        },
    );

    quick_ask::emit_to_quick_ask(
        app,
        quick_ask::EVENT_QUICK_ASK_ANSWER,
        QuickAskAnswerPayload::Err(QuickAskAnswerErrorPayload {
            ok: false,
            error: "No transcript to answer (empty)".to_string(),
        }),
    );
}

pub(crate) async fn try_quick_replace(
    input: QuickReplaceExecution<'_>,
) -> QuickReplaceExecutionResult {
    let mut output_value = input.output_value;
    let mut failure = None;

    if input.config.enabled && input.probe_plan.should_await() {
        let quick_replace_context = context_collection::collect_quick_replace_selection_context(
            input.app,
            input.probe_plan,
        )
        .await;
        let selected_text = quick_replace_context.selected_text_for_prompt();
        let surrounding_text = quick_replace_context.surrounding_text_for_prompt();

        if let Some(selected) = selected_text {
            let ocr_context = crate::sessions::ocr_usage::collect_ocr_context(
                input.pipeline,
                input.ocr_mode,
                input.ocr_config,
            )
            .await;
            let ocr_text = ocr_context.text().map(str::to_string);

            record_quick_replace_selection_start(input.app, selected, output_value.as_str());

            let pipeline_config = input.pipeline.config();
            let provider = input
                .config
                .provider
                .clone()
                .unwrap_or_else(|| pipeline_config.llm_config.provider.clone());
            let model = input
                .config
                .model
                .clone()
                .or_else(|| pipeline_config.llm_config.model.clone())
                .or_else(|| {
                    llm::default_llm_model_for_provider(provider.as_str()).map(|m| m.to_string())
                });

            let system_prompt = input.config.system_prompt.clone();
            let instructions_text = output_value.trim().to_string();
            let clipboard_text = context_collection::read_clipboard_context_if_enabled(
                input.config.include_clipboard_context,
            )
            .await;

            let user_prompt = prompt_builders::build_quick_replace_user_message(
                instructions_text.as_str(),
                selected,
                surrounding_text,
                clipboard_text.as_deref(),
                ocr_text.as_deref(),
            );

            record_quick_replace_context(input.app, clipboard_text.clone(), &ocr_context);

            let provider_impl =
                match crate::pipeline::llm_provider::create_one_off_llm_provider_unstructured(
                    &pipeline_config.llm_config,
                    &pipeline_config.llm_api_keys,
                    provider.as_str(),
                    crate::pipeline::llm_provider::LlmProviderParams {
                        model: model.clone(),
                        timeout: pipeline_config.llm_config.timeout,
                        ollama_url: pipeline_config.llm_config.ollama_url.clone(),
                        openai_reasoning_effort: pipeline_config
                            .llm_config
                            .openai_reasoning_effort
                            .clone(),
                        gemini_thinking_budget: pipeline_config.llm_config.gemini_thinking_budget,
                        gemini_thinking_level: pipeline_config
                            .llm_config
                            .gemini_thinking_level
                            .clone(),
                        anthropic_thinking_budget: pipeline_config
                            .llm_config
                            .anthropic_thinking_budget,
                    },
                ) {
                    Ok(provider_impl) => provider_impl,
                    Err(_) => {
                        let err = format!(
                            "Quick Replace failed: no API key configured for provider: {}",
                            provider
                        );
                        record_quick_replace_missing_key(input.app, &provider, input.config, &err);
                        failure = Some(err);
                        finalize_quick_replace_attempt(
                            input.app,
                            input.pipeline,
                            input.request_id,
                            failure.as_ref(),
                        );
                        return QuickReplaceExecutionResult {
                            output_value,
                            failure,
                        };
                    }
                };
            let t0 = Instant::now();

            match provider_impl.complete(&system_prompt, &user_prompt).await {
                Ok(rewritten) => {
                    let rewritten = rewritten.trim().to_string();
                    if rewritten.is_empty() {
                        let err = "Quick Replace failed: model returned empty output".to_string();
                        record_quick_replace_empty_output(input.app, &provider_impl, &err);
                        failure = Some(err);
                    } else {
                        output_value = rewritten;
                        record_quick_replace_success(
                            input.app,
                            &provider,
                            &provider_impl,
                            &system_prompt,
                            &instructions_text,
                            selected,
                            &output_value,
                            t0.elapsed().as_millis() as u64,
                        );
                    }
                }
                Err(e) => {
                    let err = e.to_string();
                    record_quick_replace_provider_error(
                        input.app,
                        input.config,
                        &provider,
                        &system_prompt,
                        selected,
                        &err,
                    );
                    failure = Some(err);
                }
            }
        }
    }

    finalize_quick_replace_attempt(
        input.app,
        input.pipeline,
        input.request_id,
        failure.as_ref(),
    );

    QuickReplaceExecutionResult {
        output_value,
        failure,
    }
}

fn record_quick_ask_ocr_start_context(
    app: &AppHandle,
    ocr_context: &crate::sessions::ocr_usage::CollectedOcrContext,
) {
    let Some(log_store) = app.try_state::<RequestLogStore>() else {
        return;
    };

    let ocr_chars = ocr_context.text_len();

    log_store.with_current(|log| {
        if let Some(n) = ocr_chars {
            log.info(format!("Quick Ask: OCR context attached ({} chars)", n));
        } else if ocr_context.requested() {
            match ocr_context.status() {
                "running" => log.warn(format!(
                    "Quick Ask: proceeding without OCR (OCR still running; timeout={}ms)",
                    ocr_context.timeout_ms()
                )),
                "failed" => log.warn(format!(
                    "Quick Ask: proceeding without OCR (OCR failed: {})",
                    ocr_context.failed_reason().unwrap_or("unknown")
                )),
                "cancelled" => {
                    log.info("Quick Ask: proceeding without OCR (OCR cancelled)".to_string())
                }
                _ => log.info(format!(
                    "Quick Ask: proceeding without OCR (status={})",
                    ocr_context.status()
                )),
            }
        }

        if let Some(serde_json::Value::Object(map)) = log.quick_ask_request_json.as_mut() {
            map.insert(
                "ocr_status".to_string(),
                serde_json::Value::String(ocr_context.status().to_string()),
            );
            map.insert(
                "ocr_context_present".to_string(),
                serde_json::Value::Bool(ocr_chars.is_some()),
            );
            map.insert(
                "ocr_context_chars".to_string(),
                ocr_chars
                    .map(|n| serde_json::Value::Number(serde_json::Number::from(n as u64)))
                    .unwrap_or(serde_json::Value::Null),
            );
        }
    });
}

#[allow(clippy::too_many_arguments)]
fn record_quick_ask_prompt_context(
    app: &AppHandle,
    pipeline: &SharedPipeline,
    quick_ask_config: &QuickAskEffectiveConfig,
    question_with_context: &str,
    quick_ask_context_text_for_log: Option<String>,
    quick_ask_clipboard_context_for_log: Option<String>,
    selected_context_trimmed: Option<&str>,
    surrounding_context_trimmed: Option<&str>,
    clipboard_trimmed: Option<&str>,
    ocr_text: Option<String>,
) {
    let Some(log_store) = app.try_state::<RequestLogStore>() else {
        return;
    };

    let context_chars = selected_context_trimmed
        .map(str::len)
        .or_else(|| surrounding_context_trimmed.map(str::len));
    let clipboard_chars = clipboard_trimmed.map(str::len);
    let ocr_chars = ocr_text.as_deref().map(str::len);
    let ocr_failed_reason = pipeline.get_ocr_failed_reason();

    log_store.with_current(|log| {
        log.quick_ask_context_text = quick_ask_context_text_for_log;
        log.quick_ask_clipboard_context = quick_ask_clipboard_context_for_log;
        log.ocr_context_present = ocr_chars.is_some();
        log.ocr_context_chars = ocr_chars.map(|n| n as u64);
        log.ocr_context_text = ocr_text;
        if ocr_chars.is_none() {
            log.ocr_failed_reason = ocr_failed_reason;
        }

        if let Some(serde_json::Value::Object(map)) = log.quick_ask_request_json.as_mut() {
            if !quick_ask_config.request_logs_privacy_mode {
                map.insert(
                    "user_message".to_string(),
                    serde_json::Value::String(question_with_context.to_string()),
                );
            }
            map.insert(
                "context_present".to_string(),
                serde_json::Value::Bool(context_chars.is_some()),
            );
            map.insert(
                "context_chars".to_string(),
                context_chars
                    .map(|n| serde_json::Value::Number(serde_json::Number::from(n as u64)))
                    .unwrap_or(serde_json::Value::Null),
            );
            map.insert(
                "clipboard_context_present".to_string(),
                serde_json::Value::Bool(clipboard_chars.is_some()),
            );
            map.insert(
                "clipboard_context_chars".to_string(),
                clipboard_chars
                    .map(|n| serde_json::Value::Number(serde_json::Number::from(n as u64)))
                    .unwrap_or(serde_json::Value::Null),
            );
        }
    });
}

fn prepend_quick_ask_history(
    app: &AppHandle,
    question_with_context: String,
    history_count: usize,
) -> String {
    let turns = app
        .try_state::<QuickAskConversationMemory>()
        .map(|memory| memory.snapshot_last(history_count))
        .unwrap_or_default();

    if turns.is_empty() {
        return question_with_context;
    }

    let mut message = String::new();
    message.push_str("Previous Quick Ask conversation (most recent last):\n\n");

    for turn in turns.iter() {
        let question = turn.question.trim();
        let answer = turn.answer.trim();
        if question.is_empty() && answer.is_empty() {
            continue;
        }

        // Keep each historical turn bounded. The history is useful context, not a license to
        // blow up prompt size or accidentally log long prior answers.
        let cap = 1_500usize;
        let question_capped = if question.len() > cap {
            format!("{}…", truncate_utf8_to_byte_cap(question, cap))
        } else {
            question.to_string()
        };
        let answer_capped = if answer.len() > cap {
            format!("{}…", truncate_utf8_to_byte_cap(answer, cap))
        } else {
            answer.to_string()
        };

        message.push_str("User: ");
        message.push_str(&question_capped);
        message.push_str("\nAssistant: ");
        message.push_str(&answer_capped);
        message.push_str("\n\n");
    }

    message.push_str("---\n\n");
    message.push_str(&question_with_context);
    message
}

fn truncate_utf8_to_byte_cap(s: &str, cap_bytes: usize) -> &str {
    if s.len() <= cap_bytes {
        return s;
    }

    let mut idx = cap_bytes;
    while idx > 0 && !s.is_char_boundary(idx) {
        idx -= 1;
    }

    &s[..idx]
}

fn record_quick_replace_selection_start(app: &AppHandle, selected: &str, output_value: &str) {
    if let Some(log_store) = app.try_state::<RequestLogStore>() {
        log_store.with_current(|log| {
            log.kind = QuickActionKind::QuickReplace.request_kind();
            log.info(format!(
                "Quick replace: rewriting selection ({} chars)",
                selected.len()
            ));

            // Best-effort: keep these bounded so request logs stay usable.
            let cap = 8_000usize;
            log.quick_replace_selected_text = Some(clipboard_context::cap_context_text_for_prompt(
                selected, cap,
            ));
            log.quick_replace_instructions = Some(clipboard_context::cap_context_text_for_prompt(
                output_value.trim(),
                cap,
            ));
        });
    }
}

fn record_quick_replace_missing_key(
    app: &AppHandle,
    provider: &str,
    config: &QuickReplaceConfig,
    err: &str,
) {
    if let Some(log_store) = app.try_state::<RequestLogStore>() {
        log_store.with_current(|log| {
            log.warn(format!(
                "Quick replace: skipped (no API key configured for provider: {})",
                provider
            ));
            log.quick_replace_provider = Some(provider.to_string());
            log.quick_replace_model = config.model.clone();
            log.quick_replace_response_json = Some(json!({
                "ok": false,
                "error": err,
            }));
            log.error(err.to_string());
            log.complete_error(err.to_string());
        });
    }
}

fn record_quick_replace_context(
    app: &AppHandle,
    clipboard_text: Option<String>,
    ocr_context: &crate::sessions::ocr_usage::CollectedOcrContext,
) {
    if let Some(log_store) = app.try_state::<RequestLogStore>() {
        let ocr_chars = ocr_context.text_len();
        log_store.with_current(|log| {
            if let Some(cb_text) = clipboard_text {
                log.quick_replace_clipboard_context = Some(cb_text);
            }
            log.ocr_context_present = ocr_chars.is_some();
            log.ocr_context_chars = ocr_chars.map(|n| n as u64);
            log.ocr_context_text = ocr_context.text().map(str::to_string);
            if ocr_chars.is_none() {
                log.ocr_failed_reason = ocr_context.failed_reason().map(str::to_string);
            }
        });
    }
}

fn record_quick_replace_empty_output(
    app: &AppHandle,
    provider_impl: &std::sync::Arc<dyn llm::LlmProvider>,
    err: &str,
) {
    if let Some(log_store) = app.try_state::<RequestLogStore>() {
        log_store.with_current(|log| {
            log.kind = QuickActionKind::QuickReplace.request_kind();
            log.quick_replace_provider = Some(provider_impl.name().to_string());
            log.quick_replace_model = Some(provider_impl.model().to_string());
            log.quick_replace_response_json = Some(json!({
                "ok": false,
                "error": err,
            }));
            log.error(err.to_string());
            log.complete_error(err.to_string());
        });
    }
}

#[allow(clippy::too_many_arguments)]
fn record_quick_replace_success(
    app: &AppHandle,
    provider: &str,
    provider_impl: &std::sync::Arc<dyn llm::LlmProvider>,
    system_prompt: &str,
    instructions_text: &str,
    selected: &str,
    output_value: &str,
    duration_ms: u64,
) {
    if let Some(log_store) = app.try_state::<RequestLogStore>() {
        log_store.with_current(|log| {
            log.kind = QuickActionKind::QuickReplace.request_kind();
            log.info(format!(
                "Quick replace: rewrite succeeded in {}ms ({} chars)",
                duration_ms,
                output_value.len()
            ));
            log.quick_replace_provider = Some(provider_impl.name().to_string());
            log.quick_replace_model = Some(provider_impl.model().to_string());
            log.quick_replace_duration_ms = Some(duration_ms);
            log.quick_replace_output_text = Some(output_value.to_string());
            log.quick_replace_request_json = Some(json!({
                "provider": provider,
                "model": provider_impl.model(),
                "system_prompt": system_prompt,
                "instructions_chars": instructions_text.len(),
                "selected_text_chars": selected.len(),
            }));
            log.quick_replace_response_json = Some(json!({
                "ok": true,
                "provider_used": provider_impl.name(),
                "model_used": provider_impl.model(),
                "duration_ms": duration_ms,
                "output_chars": output_value.len(),
            }));

            // The effective final output for this request.
            log.formatted_transcript = Some(output_value.to_string());
        });
    }
}

fn record_quick_replace_provider_error(
    app: &AppHandle,
    config: &QuickReplaceConfig,
    provider: &str,
    system_prompt: &str,
    selected: &str,
    err: &str,
) {
    if let Some(log_store) = app.try_state::<RequestLogStore>() {
        log_store.with_current(|log| {
            log.kind = QuickActionKind::QuickReplace.request_kind();
            log.quick_replace_provider = Some(provider.to_string());
            log.quick_replace_model = config.model.clone();
            log.quick_replace_request_json = Some(json!({
                "provider": provider,
                "system_prompt": system_prompt,
                "selected_text_chars": selected.len(),
            }));
            log.quick_replace_response_json = Some(json!({
                "ok": false,
                "error": err,
            }));
            log.warn(format!("Quick replace: rewrite failed ({})", err));
            log.error(format!("Quick Replace failed: {}", err));
            log.complete_error(err.to_string());
        });
    }
}

fn finalize_quick_replace_attempt(
    app: &AppHandle,
    pipeline: &SharedPipeline,
    request_id: Option<&str>,
    failure: Option<&String>,
) {
    if let Some(log_store) = app.try_state::<RequestLogStore>() {
        log_store.with_current(|log| {
            if log.status == RequestStatus::InProgress && failure.is_none() {
                log.complete_success();
            }
        });
    }

    complete_current_request_with_cost(
        app,
        pipeline,
        request_id,
        if failure.is_some() {
            stats::EventStatus::Error
        } else {
            stats::EventStatus::Success
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf8_truncation_preserves_character_boundaries() {
        assert_eq!(truncate_utf8_to_byte_cap("hello", 99), "hello");
        assert_eq!(truncate_utf8_to_byte_cap("שלום", 5), "של");
    }

    #[test]
    fn quick_replace_result_tracks_output_and_failure() {
        let result = QuickReplaceExecutionResult {
            output_value: "text".into(),
            failure: Some("no key".into()),
        };

        assert_eq!(result.failure.as_deref(), Some("no key"));
        assert_eq!(result.output_value, "text");
    }
}
