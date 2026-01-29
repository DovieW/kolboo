use crate::commands::CommandError;
use crate::pipeline::{PipelineState, SharedPipeline};
use crate::request_log::RequestLogStore;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_store::StoreExt;

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub struct OverlayOcrProviderStatus {
    pub available: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub struct OverlayPipelineState {
    pub pipeline_state: String,
    pub ocr_session_id: Option<String>,
    pub ocr_status: String,
    pub ocr_manual_available: bool,
    pub ocr_provider: OverlayOcrProviderStatus,
    /// True when the STT portion is done (before LLM / output).
    /// Use in combination with `ocr_status == "running"` to show "waiting for OCR".
    pub stt_complete: bool,
}

fn get_ocr_provider_status(app: &AppHandle) -> OverlayOcrProviderStatus {
    let base_url_raw = app
        .store("settings.json")
        .ok()
        .and_then(|store| store.get("ocr_base_url"))
        .and_then(|v| v.as_str().map(|s| s.trim().to_string()))
        .unwrap_or_default();

    if base_url_raw.is_empty() {
        return OverlayOcrProviderStatus {
            available: false,
            reason: Some("OCR base URL not set".to_string()),
        };
    }

    if reqwest::Url::parse(&base_url_raw).is_err() {
        return OverlayOcrProviderStatus {
            available: false,
            reason: Some("OCR base URL is invalid".to_string()),
        };
    }

    let auth_mode = app
        .store("settings.json")
        .ok()
        .and_then(|store| store.get("ocr_auth_mode"))
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .unwrap_or_else(|| "none".to_string());

    if auth_mode == "bearer_api_key" && !crate::secrets::has_api_key(app, "ocr_api_key") {
        return OverlayOcrProviderStatus {
            available: false,
            reason: Some("OCR API key not set".to_string()),
        };
    }

    OverlayOcrProviderStatus {
        available: true,
        reason: None,
    }
}

fn is_manual_ocr_available_for_current_session(pipeline: &SharedPipeline) -> bool {
    let config = pipeline.config();

    let session_profile_id = pipeline.peek_session_profile_override();
    let active_profile = session_profile_id
        .as_deref()
        .and_then(|id| if id == "default" { None } else { Some(id) })
        .and_then(|id| {
            config
                .llm_config
                .program_prompt_profiles
                .iter()
                .find(|p| p.id == id)
        });

    let default_profile = config
        .llm_config
        .program_prompt_profiles
        .iter()
        .find(|p| p.id == "default");

    let rewrite_mode = crate::pipeline::resolve_rewrite_active_window_ocr_mode(
        active_profile,
        default_profile,
        config.ocr_config.rewrite_mode.as_str(),
    );
    let quick_replace_mode = crate::pipeline::resolve_quick_replace_active_window_ocr_mode(
        active_profile,
        default_profile,
        config.ocr_config.quick_replace_mode.as_str(),
    );
    let quick_ask_mode = crate::pipeline::resolve_quick_ask_active_window_ocr_mode(
        active_profile,
        default_profile,
        config.ocr_config.quick_ask_mode.as_str(),
    );

    rewrite_mode == "manual" || quick_replace_mode == "manual" || quick_ask_mode == "manual"
}

fn pipeline_state_string(pipeline: &SharedPipeline) -> String {
    match pipeline.state() {
        PipelineState::Idle => "idle",
        PipelineState::Recording => "recording",
        PipelineState::Routing => "routing",
        PipelineState::Transcribing => "transcribing",
        PipelineState::Rewriting => "rewriting",
        PipelineState::Error => "error",
    }
    .to_string()
}

/// Trigger active-window OCR for the current session.
///
/// This is intended for "manual" OCR mode: OCR is only started when the user
/// clicks the overlay OCR button.
#[tauri::command]
pub async fn pipeline_trigger_active_window_ocr(
    app: AppHandle,
    pipeline: State<'_, SharedPipeline>,
) -> Result<bool, CommandError> {
    if let Some(store) = app.try_state::<RequestLogStore>() {
        store.with_current(|log| {
            log.ocr_effective_mode = Some("manual".to_string());
            log.ocr_not_attempted_reason = None;
            log.info("OCR: manually triggered".to_string());
        });
    }
    // Best-effort: finalize any previous completed task so repeated clicks behave sanely.
    pipeline.finalize_ocr_task_if_finished().await;

    let before = pipeline.get_ocr_status();

    let cfg = pipeline.config();
    pipeline.start_ocr_task(&cfg.ocr_config);

    let after = pipeline.get_ocr_status();

    Ok(before != after && (after == "running" || after == "done"))
}

/// Composite overlay poll: pipeline phase + OCR task status.
#[tauri::command]
pub async fn pipeline_get_overlay_state(
    app: AppHandle,
    pipeline: State<'_, SharedPipeline>,
) -> Result<OverlayPipelineState, CommandError> {
    pipeline.finalize_ocr_task_if_finished().await;

    let state = OverlayPipelineState {
        pipeline_state: pipeline_state_string(&pipeline),
        ocr_session_id: pipeline.ocr_session_id(),
        ocr_status: pipeline.get_ocr_status(),
        ocr_manual_available: is_manual_ocr_available_for_current_session(&pipeline),
        ocr_provider: get_ocr_provider_status(&app),
        stt_complete: pipeline.is_stt_complete(),
    };

    // Debug logging for overlay state (only at debug level)
    log::debug!(
        "overlay_state: pipeline={}, ocr_status={}, stt_complete={}, ocr_blocking={}",
        state.pipeline_state,
        state.ocr_status,
        state.stt_complete,
        state.stt_complete && state.ocr_status == "running"
    );

    Ok(state)
}

/// Cancel any in-flight OCR work for the current session.
#[tauri::command]
pub fn pipeline_cancel_active_window_ocr(
    pipeline: State<'_, SharedPipeline>,
) -> Result<(), CommandError> {
    pipeline.cancel_ocr_task();
    Ok(())
}
