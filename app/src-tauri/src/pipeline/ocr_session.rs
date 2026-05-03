//! OCR session/task orchestration for the recording pipeline.
//!
//! This module keeps active-window OCR concerns out of the root pipeline module:
//! - validating OCR runtime readiness before screen capture
//! - spawning and finalizing the async OCR task
//! - preserving OCR session state across internal pipeline transitions
//! - translating OCR task state into overlay/request-log status

use super::{OcrConfig, SharedPipeline};
use crate::event_payloads::OverlayOcrContextUnavailablePayload;
use crate::events;
use crate::request_log::{RequestLog, RequestLogStore};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

pub(super) struct OcrTaskHandle {
    pub(super) session_id: Option<String>,
    pub(super) request_log_id: Option<String>,
    pub(super) handle: tokio::task::JoinHandle<Result<crate::ocr::OcrResult, String>>,
}

impl OcrTaskHandle {
    pub(super) fn new(
        session_id: Option<String>,
        request_log_id: Option<String>,
        handle: tokio::task::JoinHandle<Result<crate::ocr::OcrResult, String>>,
    ) -> Self {
        Self {
            session_id,
            request_log_id,
            handle,
        }
    }
}

#[derive(Clone, Default)]
struct OcrRequestLog {
    store: Option<RequestLogStore>,
    request_id: Option<String>,
}

impl OcrRequestLog {
    fn capture(app: &AppHandle) -> Self {
        let store = app
            .try_state::<RequestLogStore>()
            .map(|s| s.inner().clone());
        let request_id = store
            .as_ref()
            .and_then(|s| s.with_current(|log| log.id.clone()));

        Self { store, request_id }
    }

    fn for_request_id(app: &AppHandle, request_id: Option<String>) -> Self {
        let store = app
            .try_state::<RequestLogStore>()
            .map(|s| s.inner().clone());
        Self { store, request_id }
    }

    fn with_request<F>(&self, f: F)
    where
        F: FnOnce(&mut RequestLog),
    {
        let (Some(store), Some(request_id)) = (&self.store, &self.request_id) else {
            return;
        };

        let _ = store.with_current_id(request_id, f);
    }

    fn request_id(&self) -> Option<String> {
        self.request_id.clone()
    }
}

fn session_matches(current: Option<&str>, expected: Option<&str>) -> bool {
    current == expected
}

fn truncate_overlay_reason(reason: &str) -> String {
    // Keep overlay messages short; users can inspect request logs for full details.
    const CAP: usize = 220;
    let trimmed = reason.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if trimmed.chars().count() <= CAP {
        return trimmed.to_string();
    }
    let mut out = String::new();
    for (i, ch) in trimmed.chars().enumerate() {
        if i >= CAP.saturating_sub(1) {
            break;
        }
        out.push(ch);
    }
    out.push('…');
    out
}

fn emit_overlay_ocr_context_unavailable(
    app: &AppHandle,
    request_id: Option<String>,
    reason: Option<String>,
) {
    let reason = reason.and_then(|r| {
        let t = truncate_overlay_reason(&r);
        if t.is_empty() {
            None
        } else {
            Some(t)
        }
    });

    let payload = OverlayOcrContextUnavailablePayload {
        message: "OCR context unavailable".to_string(),
        reason,
        request_id,
    };

    if let Some(overlay) = app.get_webview_window("overlay") {
        let _ = overlay.emit(events::EVENT_OVERLAY_OCR_CONTEXT_UNAVAILABLE, payload);
    } else {
        let _ = app.emit(events::EVENT_OVERLAY_OCR_CONTEXT_UNAVAILABLE, payload);
    }
}

impl SharedPipeline {
    pub(crate) fn start_ocr_task_if_auto(&self, ocr_config: &OcrConfig, should_run: bool) {
        if !should_run {
            // Best-effort request-log breadcrumb. (Detailed mode reasoning is recorded at callsites.)
            if let Ok(app_guard) = self.app_handle.lock() {
                if let Some(app) = app_guard.as_ref() {
                    OcrRequestLog::capture(app).with_request(|log| {
                        if log.ocr_status.is_none() {
                            log.ocr_status = Some("not_started".to_string());
                        }
                        if log.ocr_not_attempted_reason.is_none() {
                            log.ocr_not_attempted_reason = Some("not_triggered".to_string());
                        }
                        log.debug("OCR: auto-start skipped".to_string());
                    });
                }
            }
            return;
        }

        self.start_ocr_task(ocr_config);
    }

    pub(crate) fn start_ocr_task(&self, ocr_config: &OcrConfig) {
        let app_handle = self.app_handle.lock().ok().and_then(|g| g.clone());
        let Some(app_handle) = app_handle else {
            return;
        };

        let request_log = OcrRequestLog::capture(&app_handle);

        // Best-effort: bind OCR to the current request log id as a stable session id.
        // This makes OCR consumption resilient to internal pipeline transitions.
        let request_id_for_session = request_log.request_id();

        let base_url = ocr_config.base_url.clone();
        let model = ocr_config.model.clone();
        let auth_mode = ocr_config.auth_mode.clone();
        let prompt = ocr_config.prompt.clone();
        let max_tokens = ocr_config.max_tokens;
        let temperature = ocr_config.temperature;
        let top_p = ocr_config.top_p;
        let timeout_ms = ocr_config.request_timeout_ms;
        let context_max_chars = ocr_config.context_max_chars;
        let hallucination_protection = ocr_config.hallucination_protection;
        let hallucination_threshold = ocr_config.hallucination_threshold;
        let resize_max_dimension = ocr_config.resize_max_dimension;
        let resize_filter = ocr_config.resize_filter.clone();

        let base_url_trimmed = base_url.as_deref().unwrap_or("").trim().to_string();
        if base_url_trimmed.is_empty() {
            request_log.with_request(|log| {
                log.ocr_status = Some("not_started".to_string());
                log.ocr_not_attempted_reason = Some("provider_unavailable".to_string());
                log.info("OCR: not started (OCR base URL not set)".to_string());
            });

            emit_overlay_ocr_context_unavailable(
                &app_handle,
                request_log.request_id(),
                Some("OCR base URL not set".to_string()),
            );
            return;
        }

        if reqwest::Url::parse(&base_url_trimmed).is_err() {
            request_log.with_request(|log| {
                log.ocr_status = Some("not_started".to_string());
                log.ocr_not_attempted_reason = Some("invalid_base_url".to_string());
                log.info("OCR: not started (OCR base URL is invalid)".to_string());
            });

            emit_overlay_ocr_context_unavailable(
                &app_handle,
                request_log.request_id(),
                Some("OCR base URL is invalid".to_string()),
            );
            return;
        }

        // Validate auth before we capture a screenshot.
        // If we can't call the provider (missing key), we should *not* capture.
        let api_key = if auth_mode == "bearer_api_key" {
            crate::secrets::get_api_key(&app_handle, "ocr_api_key")
        } else {
            None
        };

        if auth_mode == "bearer_api_key"
            && api_key
                .as_deref()
                .map(|s| s.trim())
                .unwrap_or("")
                .is_empty()
        {
            request_log.with_request(|log| {
                log.ocr_status = Some("not_started".to_string());
                log.ocr_not_attempted_reason = Some("missing_api_key".to_string());
                log.info("OCR: not started (OCR API key not set)".to_string());
            });

            emit_overlay_ocr_context_unavailable(
                &app_handle,
                request_log.request_id(),
                Some("OCR API key not set".to_string()),
            );
            return;
        }

        if let Ok(mut inner) = self.inner.lock() {
            if inner.ocr_session_id.is_none() {
                inner.ocr_session_id = request_id_for_session.clone();
            }

            if inner.ocr_task.is_some() || inner.ocr_result.is_some() || inner.ocr_awaiting {
                return;
            }

            inner.ocr_failed_reason = None;
            inner.ocr_cancelled = false;

            let task_session_id = inner.ocr_session_id.clone();
            let request_log = OcrRequestLog {
                store: request_log.store.clone(),
                request_id: task_session_id
                    .clone()
                    .or_else(|| request_id_for_session.clone()),
            };
            let task_request_log_id = request_log.request_id();

            request_log.with_request(|log| {
                log.ocr_status = Some("running".to_string());
                log.ocr_started_at = Some(chrono::Utc::now());
                log.info("OCR: started".to_string());
            });

            let handle = tokio::spawn(async move {
                let ocr_started_at = chrono::Utc::now();

                #[cfg(target_os = "windows")]
                let capture_target = crate::windows_apps::get_foreground_window_capture_target();

                #[cfg(target_os = "windows")]
                let (capture_hwnd_raw, capture_process_path, capture_target_json) = {
                    let hwnd_raw = capture_target.as_ref().map(|t| t.hwnd_raw);
                    let process_path = capture_target.as_ref().map(|t| t.process_path.clone());
                    let json = capture_target.as_ref().map(|t| {
                        serde_json::json!({
                            "process": crate::app_shared::basename_for_log(&t.process_path),
                            "external_fallback": t.used_external_fallback,
                        })
                    });
                    (hwnd_raw, process_path, json)
                };

                #[cfg(not(target_os = "windows"))]
                let capture_target: Option<()> = None;

                #[cfg(not(target_os = "windows"))]
                let (capture_hwnd_raw, capture_process_path, capture_target_json): (
                    Option<usize>,
                    Option<String>,
                    Option<serde_json::Value>,
                ) = (None, None, None);

                #[cfg(target_os = "windows")]
                if let Some(target) = capture_target.as_ref() {
                    request_log.with_request(|log| {
                        log.debug(format!(
                            "OCR: capture target selected (process={}, external_fallback={})",
                            crate::app_shared::basename_for_log(&target.process_path),
                            target.used_external_fallback
                        ));
                    });
                }

                let capture = match tokio::task::spawn_blocking(move || {
                    #[cfg(target_os = "windows")]
                    {
                        if let (Some(hwnd_raw), Some(process_path)) =
                            (capture_hwnd_raw, capture_process_path)
                        {
                            // Guardrail: if we somehow ended up with our own window, refuse to OCR.
                            // This prevents "OCR reading Kolboo" when focus flips during capture.
                            // (Profile matching already tries to avoid this; capture must too.)
                            // If Kolboo is still the chosen window (very rare), bail out.
                            // We can't perfectly detect this from HWND alone here, but we can cheaply
                            // sanity-check by process basename.
                            let base =
                                crate::app_shared::basename_for_log(&process_path).to_lowercase();
                            if base.contains("kolboo") {
                                return Err(
                                    "Refused OCR capture: target window appears to be Kolboo"
                                        .to_string(),
                                );
                            }

                            return crate::active_window_capture::capture_window_png(
                                windows::Win32::Foundation::HWND(
                                    hwnd_raw as *mut core::ffi::c_void,
                                ),
                                resize_max_dimension,
                                &resize_filter,
                            );
                        }

                        crate::active_window_capture::capture_active_window_png(
                            resize_max_dimension,
                            &resize_filter,
                        )
                    }

                    #[cfg(not(target_os = "windows"))]
                    {
                        crate::active_window_capture::capture_active_window_png(
                            resize_max_dimension,
                            &resize_filter,
                        )
                    }
                })
                .await
                {
                    Ok(Ok(capture)) => capture,
                    Ok(Err(err)) => {
                        request_log.with_request(|log| {
                            log.ocr_status = Some("failed".to_string());
                            log.ocr_failed_reason = Some(err.clone());
                            let duration_ms = (chrono::Utc::now() - ocr_started_at)
                                .num_milliseconds()
                                .max(0) as u64;
                            log.ocr_duration_ms = Some(duration_ms);
                            log.warn("OCR: capture failed".to_string());
                        });

                        // If the capture failed because we refused to OCR Kolboo, emit a friendly hint.
                        if err.to_lowercase().contains("refused ocr capture") {
                            emit_overlay_ocr_context_unavailable(
                                &app_handle,
                                request_log.request_id(),
                                Some("OCR can’t run while Kolboo is focused. Switch back to the target app and try again.".to_string()),
                            );
                        }
                        return Err(err);
                    }
                    Err(join_err) => {
                        let err = format!("OCR capture failed: {}", join_err);
                        request_log.with_request(|log| {
                            log.ocr_status = Some("failed".to_string());
                            log.ocr_failed_reason = Some(err.clone());
                            let duration_ms = (chrono::Utc::now() - ocr_started_at)
                                .num_milliseconds()
                                .max(0) as u64;
                            log.ocr_duration_ms = Some(duration_ms);
                            log.warn("OCR: capture join failed".to_string());
                        });
                        return Err(err);
                    }
                };

                request_log.with_request(|log| {
                    log.debug(format!(
                        "OCR: captured active window ({}x{}, png_bytes={})",
                        capture.image_width_px,
                        capture.image_height_px,
                        capture.image_png_bytes.len()
                    ));
                });

                // OCR hallucination protection: validate image quality before sending to API.
                // This catches uniform-color images that would cause the model to hallucinate.
                if hallucination_protection {
                    // Decode the PNG to validate its contents.
                    let validation_result = match image::load_from_memory(&capture.image_png_bytes)
                    {
                        Ok(img) => {
                            let rgba = img.to_rgba8();
                            crate::active_window_capture::validate_image_for_ocr(
                                rgba.as_raw(),
                                hallucination_threshold,
                            )
                        }
                        Err(e) => {
                            log::warn!(
                                "OCR: Failed to decode captured image for validation: {}",
                                e
                            );
                            // If we can't decode it, skip validation and let the API handle it.
                            crate::active_window_capture::ImageValidationResult {
                                validation: crate::active_window_capture::ImageValidation::Valid,
                                variance: 0,
                                threshold: hallucination_threshold,
                                mean_rgb: (0, 0, 0),
                            }
                        }
                    };

                    // Always log the validation metrics so users can see them.
                    let vr = &validation_result;
                    request_log.with_request(|log| {
                        log.debug(format!(
                            "OCR: hallucination check (variance={}, threshold={}, mean_rgb=({},{},{}))",
                            vr.variance, vr.threshold, vr.mean_rgb.0, vr.mean_rgb.1, vr.mean_rgb.2
                        ));
                    });

                    if !validation_result.validation.is_valid() {
                        let reason = validation_result
                            .validation
                            .reason()
                            .unwrap_or_else(|| "unknown".to_string());
                        request_log.with_request(|log| {
                            log.ocr_status = Some("skipped".to_string());
                            log.ocr_failed_reason = Some(reason.clone());
                            let duration_ms = (chrono::Utc::now() - ocr_started_at)
                                .num_milliseconds()
                                .max(0) as u64;
                            log.ocr_duration_ms = Some(duration_ms);
                            log.warn(format!(
                                "OCR: skipped due to hallucination protection ({}, variance={} < threshold={})",
                                reason, validation_result.variance, validation_result.threshold
                            ));
                        });
                        emit_overlay_ocr_context_unavailable(
                            &app_handle,
                            request_log.request_id(),
                            Some(format!("OCR skipped: {}", reason)),
                        );
                        return Err(format!("OCR skipped: {}", reason));
                    }
                }
                request_log.with_request(|log| {
                    let base_url_for_log = base_url_trimmed.clone();
                    let model_for_log = model.clone();
                    let auth_mode_for_log = auth_mode.clone();
                    let prompt_for_log = prompt.clone();

                    log.ocr_request_json = Some(serde_json::json!({
                        "provider": "openai_compatible",
                        "base_url": base_url_for_log,
                        "endpoint": "/v1/chat/completions",
                        "model": model_for_log,
                        "auth_mode": auth_mode_for_log,
                        "timeout_ms": timeout_ms,
                        "prompt": prompt_for_log,
                        "capture_target": capture_target_json,
                        "max_tokens": max_tokens,
                        "temperature": temperature,
                        "top_p": top_p,
                        "image": {
                            "format": "png",
                            "bytes": capture.image_png_bytes.len(),
                            "width_px": capture.image_width_px,
                            "height_px": capture.image_height_px,
                        },
                    }));
                });

                let ocr_result = crate::ocr::openai_compatible::request_ocr_text(
                    crate::ocr::openai_compatible::OcrRequestParams {
                        base_url: base_url_trimmed.as_str(),
                        model: model.as_str(),
                        image_png: &capture.image_png_bytes,
                        api_key: api_key.as_deref(),
                        timeout_ms,
                        prompt: prompt.as_str(),
                        max_tokens,
                        temperature,
                        top_p,
                    },
                )
                .await;

                let (result, response_json) = match ocr_result {
                    Ok(ok) => ok,
                    Err(err) => {
                        request_log.with_request(|log| {
                            log.ocr_status = Some("failed".to_string());
                            log.ocr_failed_reason = Some(err.clone());
                            log.ocr_response_json = Some(serde_json::json!({
                                "ok": false,
                                "error": err,
                            }));
                            let duration_ms = (chrono::Utc::now() - ocr_started_at)
                                .num_milliseconds()
                                .max(0) as u64;
                            log.ocr_duration_ms = Some(duration_ms);
                            log.warn("OCR: failed".to_string());
                        });
                        return Err(err);
                    }
                };

                request_log.with_request(|log| {
                    log.ocr_response_json = Some(response_json);
                    let duration_ms = (chrono::Utc::now() - ocr_started_at)
                        .num_milliseconds()
                        .max(0) as u64;
                    log.ocr_duration_ms = Some(duration_ms);
                    log.debug(format!(
                        "OCR: response received ({} chars)",
                        result.text.chars().count()
                    ));

                    // Warn if OCR result looks like the model echoed the system prompt
                    // (common when using a non-vision model for OCR)
                    let lower = result.text.to_lowercase();
                    if lower.contains("ocr")
                        && (lower.contains("engine") || lower.contains("extract"))
                    {
                        log.warn(
                            "OCR: response looks like prompt echo (model may not support vision)"
                                .to_string(),
                        );
                    }
                });

                let (text, _truncated) =
                    crate::ocr::truncate_ocr_text(&result.text, context_max_chars);

                Ok(crate::ocr::OcrResult {
                    text,
                    provider: result.provider,
                    model: result.model,
                })
            });

            inner.ocr_abort_handle = Some(handle.abort_handle());
            inner.ocr_task = Some(OcrTaskHandle::new(
                task_session_id,
                task_request_log_id,
                handle,
            ));
        }
    }

    pub(crate) fn cancel_ocr_task(&self) {
        let request_log_id = self.inner.lock().ok().and_then(|inner| {
            inner
                .ocr_task
                .as_ref()
                .and_then(|task| task.request_log_id.clone())
                .or_else(|| inner.ocr_session_id.clone())
        });

        if let Ok(mut inner) = self.inner.lock() {
            inner.cancel_ocr_task(true);
        }

        if let Ok(app_guard) = self.app_handle.lock() {
            if let Some(app) = app_guard.as_ref() {
                OcrRequestLog::for_request_id(app, request_log_id).with_request(|log| {
                    log.ocr_status = Some("cancelled".to_string());
                    log.info("OCR: cancelled".to_string());
                });
            }
        }
    }

    pub(crate) async fn finalize_ocr_task_if_finished(&self) {
        let task = {
            let mut inner = match self.inner.lock() {
                Ok(g) => g,
                Err(_) => return,
            };

            let Some(task) = inner.ocr_task.as_ref() else {
                return;
            };

            if !task.handle.is_finished() {
                return;
            }

            log::debug!("finalize_ocr_task_if_finished: task finished, taking handle");
            // Task is finished: take ownership so we can await and store the outcome.
            inner.ocr_task.take()
        };

        let Some(task) = task else {
            return;
        };
        let task_session_id = task.session_id.clone();
        let request_log_id = task.request_log_id.clone();
        let request_log = self
            .app_handle
            .lock()
            .ok()
            .and_then(|g| g.clone())
            .map(|app| OcrRequestLog::for_request_id(&app, request_log_id.clone()))
            .unwrap_or_default();

        match task.handle.await {
            Ok(Ok(result)) => {
                if let Ok(mut inner) = self.inner.lock() {
                    if !session_matches(inner.ocr_session_id.as_deref(), task_session_id.as_deref())
                    {
                        log::debug!(
                            "finalize_ocr_task_if_finished: ignoring stale OCR result for session {:?}; current={:?}",
                            task_session_id,
                            inner.ocr_session_id
                        );
                        return;
                    }
                    inner.ocr_result = Some(result);
                    inner.ocr_failed_reason = None;
                    inner.ocr_cancelled = false;
                    inner.ocr_abort_handle = None;
                }

                request_log.with_request(|log| {
                    log.ocr_status = Some("done".to_string());
                    log.info("OCR: done".to_string());
                });
            }
            Ok(Err(err)) => {
                let reason = err.clone();
                if let Ok(mut inner) = self.inner.lock() {
                    if !session_matches(inner.ocr_session_id.as_deref(), task_session_id.as_deref())
                    {
                        log::debug!(
                            "finalize_ocr_task_if_finished: ignoring stale OCR failure for session {:?}; current={:?}",
                            task_session_id,
                            inner.ocr_session_id
                        );
                        return;
                    }
                    inner.ocr_failed_reason = Some(reason.clone());
                    inner.ocr_result = None;
                    inner.ocr_cancelled = false;
                    inner.ocr_abort_handle = None;
                }

                if let Ok(app_guard) = self.app_handle.lock() {
                    if let Some(app) = app_guard.as_ref() {
                        emit_overlay_ocr_context_unavailable(
                            app,
                            request_log.request_id(),
                            Some(reason.clone()),
                        );
                    }
                }
                request_log.with_request(|log| {
                    log.ocr_status = Some("failed".to_string());
                    log.ocr_failed_reason = Some(reason);
                    log.warn("OCR: failed".to_string());
                });
            }
            Err(join_err) => {
                // Aborted/cancelled tasks surface as a JoinError.
                if let Ok(mut inner) = self.inner.lock() {
                    if !session_matches(inner.ocr_session_id.as_deref(), task_session_id.as_deref())
                    {
                        log::debug!(
                            "finalize_ocr_task_if_finished: ignoring stale OCR join error for session {:?}; current={:?}",
                            task_session_id,
                            inner.ocr_session_id
                        );
                        return;
                    }
                    inner.ocr_result = None;
                    inner.ocr_failed_reason = Some(join_err.to_string());
                    inner.ocr_cancelled = join_err.is_cancelled();
                    inner.ocr_abort_handle = None;
                }

                if let Ok(app_guard) = self.app_handle.lock() {
                    if let Some(app) = app_guard.as_ref() {
                        if !join_err.is_cancelled() {
                            emit_overlay_ocr_context_unavailable(
                                app,
                                request_log.request_id(),
                                Some(join_err.to_string()),
                            );
                        }
                    }
                }
                let cancelled = join_err.is_cancelled();
                let reason = join_err.to_string();
                request_log.with_request(|log| {
                    log.ocr_status =
                        Some(if cancelled { "cancelled" } else { "failed" }.to_string());
                    log.ocr_failed_reason = Some(reason);
                    log.warn("OCR: task aborted".to_string());
                });
            }
        }
    }

    pub(crate) fn get_ocr_status(&self) -> String {
        let inner = match self.inner.lock() {
            Ok(g) => g,
            Err(_) => return "failed".to_string(),
        };

        if inner.ocr_cancelled {
            return "cancelled".to_string();
        }

        if inner.ocr_result.is_some() {
            return "done".to_string();
        }

        if inner.ocr_task.is_some() || inner.ocr_awaiting {
            return "running".to_string();
        }

        if inner.ocr_failed_reason.is_some() {
            return "failed".to_string();
        }

        "not_started".to_string()
    }

    pub(crate) async fn get_ocr_result_with_timeout(
        &self,
        timeout: Duration,
    ) -> Option<crate::ocr::OcrResult> {
        // IMPORTANT: Do not permanently take/drop the OCR task handle when timing out.
        // If we drop the JoinHandle on timeout, the OCR task will keep running in the background
        // (and may even log "response received"), but the pipeline can no longer consume/store
        // the result.
        let mut task = {
            let mut inner = self.inner.lock().ok()?;
            if let Some(result) = inner.ocr_result.as_ref() {
                return Some(result.clone());
            }
            let Some(task) = inner.ocr_task.take() else {
                inner.ocr_awaiting = false;
                return None;
            };
            // Mark that we're awaiting the OCR result so get_ocr_status() still returns "running".
            inner.ocr_awaiting = true;
            task
        };
        let task_session_id = task.session_id.clone();
        let request_log_id = task.request_log_id.clone();
        let request_log = self
            .app_handle
            .lock()
            .ok()
            .and_then(|g| g.clone())
            .map(|app| OcrRequestLog::for_request_id(&app, request_log_id.clone()))
            .unwrap_or_default();

        let res = tokio::select! {
            r = &mut task.handle => r,
            _ = tokio::time::sleep(timeout) => {
                let mut restore_task = Some(task);
                // Put the handle back so future callers (or overlay polling) can still consume it.
                if let Ok(mut inner) = self.inner.lock() {
                    if session_matches(inner.ocr_session_id.as_deref(), task_session_id.as_deref()) {
                        // Only restore if we didn't end up with a result while waiting and no new
                        // task was installed for the same session.
                        if inner.ocr_result.is_none() && inner.ocr_task.is_none() {
                            inner.ocr_task = restore_task.take();
                        }
                        inner.ocr_awaiting = false;
                    } else {
                        log::debug!(
                            "get_ocr_result_with_timeout: dropping stale OCR task for session {:?}; current={:?}",
                            task_session_id,
                            inner.ocr_session_id
                        );
                    }
                }

                if let Some(stale_task) = restore_task {
                    stale_task.handle.abort();
                }

                request_log.with_request(|log| {
                    // Keep status as running; this is "not ready in time", not a failure.
                    if log.ocr_status.is_none() {
                        log.ocr_status = Some("running".to_string());
                    }
                    log.info(format!(
                        "OCR: still running (not ready before timeout {}ms)",
                        timeout.as_millis()
                    ));
                });

                return None;
            }
        };

        match res {
            Ok(Ok(result)) => {
                if let Ok(mut inner) = self.inner.lock() {
                    if !session_matches(inner.ocr_session_id.as_deref(), task_session_id.as_deref())
                    {
                        log::debug!(
                            "get_ocr_result_with_timeout: ignoring stale OCR result for session {:?}; current={:?}",
                            task_session_id,
                            inner.ocr_session_id
                        );
                        return None;
                    }
                    inner.ocr_result = Some(result.clone());
                    inner.ocr_failed_reason = None;
                    inner.ocr_cancelled = false;
                    inner.ocr_awaiting = false;
                    inner.ocr_abort_handle = None;
                }

                request_log.with_request(|log| {
                    log.ocr_status = Some("done".to_string());
                    log.info("OCR: done".to_string());
                });
                Some(result)
            }
            Ok(Err(err)) => {
                if let Ok(mut inner) = self.inner.lock() {
                    if !session_matches(inner.ocr_session_id.as_deref(), task_session_id.as_deref())
                    {
                        log::debug!(
                            "get_ocr_result_with_timeout: ignoring stale OCR failure for session {:?}; current={:?}",
                            task_session_id,
                            inner.ocr_session_id
                        );
                        return None;
                    }
                    inner.ocr_failed_reason = Some(err.clone());
                    inner.ocr_cancelled = false;
                    inner.ocr_awaiting = false;
                    inner.ocr_abort_handle = None;
                }

                if let Ok(app_guard) = self.app_handle.lock() {
                    if let Some(app) = app_guard.as_ref() {
                        emit_overlay_ocr_context_unavailable(
                            app,
                            request_log.request_id(),
                            Some(err.clone()),
                        );
                    }
                }
                request_log.with_request(|log| {
                    log.ocr_status = Some("failed".to_string());
                    log.ocr_failed_reason = Some(err.clone());
                    log.warn("OCR: failed".to_string());
                });
                None
            }
            Err(err) => {
                if let Ok(mut inner) = self.inner.lock() {
                    if !session_matches(inner.ocr_session_id.as_deref(), task_session_id.as_deref())
                    {
                        log::debug!(
                            "get_ocr_result_with_timeout: ignoring stale OCR join error for session {:?}; current={:?}",
                            task_session_id,
                            inner.ocr_session_id
                        );
                        return None;
                    }
                    inner.ocr_failed_reason = Some(err.to_string());
                    inner.ocr_cancelled = err.is_cancelled();
                    inner.ocr_awaiting = false;
                    inner.ocr_abort_handle = None;
                }

                if let Ok(app_guard) = self.app_handle.lock() {
                    if let Some(app) = app_guard.as_ref() {
                        if !err.is_cancelled() {
                            emit_overlay_ocr_context_unavailable(
                                app,
                                request_log.request_id(),
                                Some(err.to_string()),
                            );
                        }
                    }
                }
                request_log.with_request(|log| {
                    log.ocr_status = Some(
                        if err.is_cancelled() {
                            "cancelled"
                        } else {
                            "failed"
                        }
                        .to_string(),
                    );
                    log.ocr_failed_reason = Some(err.to_string());
                    log.warn("OCR: task aborted".to_string());
                });
                return None;
            }
        }
    }

    pub(crate) fn get_ocr_failed_reason(&self) -> Option<String> {
        self.inner
            .lock()
            .ok()
            .and_then(|inner| inner.ocr_failed_reason.clone())
    }

    /// Begin (or switch) the OCR session associated with the current user request.
    ///
    /// We use the current Request Log id as a stable session identifier so OCR can remain
    /// consumable even if the pipeline returns to Idle while post-processing continues
    /// (e.g., Quick Ask / Quick Replace).
    pub fn begin_ocr_session(&self, session_id: String) {
        let mut inner = match self.inner.lock() {
            Ok(g) => g,
            Err(_) => return,
        };

        // If we're already on this session id, do nothing.
        if inner.ocr_session_id.as_deref() == Some(session_id.as_str()) {
            log::debug!(
                "begin_ocr_session: already on session {}, skipping",
                session_id
            );
            return;
        }

        // Supersede any previous session.
        if inner.ocr_task.is_some()
            || inner.ocr_abort_handle.is_some()
            || inner.ocr_result.is_some()
            || inner.ocr_awaiting
        {
            log::debug!(
                "begin_ocr_session: superseding previous session {:?} with {}",
                inner.ocr_session_id,
                session_id
            );
            inner.cancel_ocr_task(true);
        } else {
            log::debug!(
                "begin_ocr_session: starting new session {} (no previous)",
                session_id
            );
        }

        inner.ocr_session_id = Some(session_id);
        inner.ocr_cancelled = false;
        inner.ocr_failed_reason = None;
        inner.ocr_result = None;
        inner.ocr_task = None;
        inner.ocr_awaiting = false;
    }

    /// End the OCR session if it matches the provided session id.
    ///
    /// This should be called once all flows that might consume OCR (Quick Ask answer,
    /// Quick Replace extra LLM step, etc.) have completed.
    pub fn end_ocr_session_if_matches(&self, session_id: &str) {
        let mut inner = match self.inner.lock() {
            Ok(g) => g,
            Err(_) => return,
        };

        if inner.ocr_session_id.as_deref() != Some(session_id) {
            log::debug!(
                "end_ocr_session_if_matches: session_id={} does not match current {:?}",
                session_id,
                inner.ocr_session_id
            );
            return;
        }

        log::debug!(
            "end_ocr_session_if_matches: session_id={} matches, clearing OCR",
            session_id
        );
        inner.cancel_ocr_task(false);
        inner.ocr_session_id = None;
    }

    /// Read the current OCR session id (if any).
    pub fn ocr_session_id(&self) -> Option<String> {
        self.inner
            .lock()
            .ok()
            .and_then(|g| g.ocr_session_id.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::truncate_overlay_reason;

    #[test]
    fn truncate_overlay_reason_trims_empty_reason() {
        assert_eq!(truncate_overlay_reason("   "), "");
    }

    #[test]
    fn truncate_overlay_reason_caps_long_reason_with_ellipsis() {
        let input = "x".repeat(300);
        let out = truncate_overlay_reason(&input);

        assert_eq!(out.chars().count(), 220);
        assert!(out.ends_with('…'));
    }
}
