use serde_json::json;
use tauri::{AppHandle, Manager};
use tauri_plugin_cli::Matches;

use crate::cli::{
    arg_string, arg_u64, output_format_from, CliError, CliOutputFormat, CommandResult,
};
use crate::pipeline::SharedPipeline;
use crate::request_log::{RequestLog, RequestLogStore, RequestStatus};

pub(crate) fn handle_pipeline(
    app: &AppHandle,
    matches: &Matches,
) -> Result<(CliOutputFormat, CommandResult<serde_json::Value>), CliError> {
    let Some(subcommand) = matches.subcommand.as_ref() else {
        return Err(CliError::Validation(
            "Missing pipeline subcommand".to_string(),
        ));
    };

    match subcommand.name.as_str() {
        "run" => {
            let output_format = output_format_from(&subcommand.matches);
            let profile_id = arg_string(&subcommand.matches, "profile");
            let pipeline = app.state::<SharedPipeline>();

            if let Some(profile_id) = profile_id.as_deref() {
                pipeline
                    .set_session_profile_override(Some(profile_id.to_string()))
                    .map_err(|err| CliError::Runtime(err.to_string()))?;
            }

            crate::commands::recording::pipeline_start_recording(app.clone(), pipeline)
                .map_err(|err| CliError::Runtime(err.to_string()))?;

            let state =
                crate::commands::recording::pipeline_get_state(app.state::<SharedPipeline>())
                    .map_err(|err| CliError::Runtime(err.to_string()))?;

            Ok((
                output_format,
                CommandResult::success(
                    Some(json!({ "state": state })),
                    Some("Recording started".to_string()),
                ),
            ))
        }
        "stop" => {
            let output_format = output_format_from(&subcommand.matches);
            let pipeline = app.state::<SharedPipeline>();

            let transcript = tauri::async_runtime::block_on(
                crate::commands::recording::pipeline_stop_and_transcribe(app.clone(), pipeline),
            )
            .map_err(|err| CliError::Runtime(err.to_string()))?;

            Ok((
                output_format,
                CommandResult::success(
                    Some(json!({ "transcript": transcript })),
                    Some("Recording stopped".to_string()),
                ),
            ))
        }
        "transcribe" => {
            let output_format = output_format_from(&subcommand.matches);
            let file_path = arg_string(&subcommand.matches, "file")
                .ok_or_else(|| CliError::Validation("Missing --file".to_string()))?;
            let profile_id = arg_string(&subcommand.matches, "profile");
            let forced_llm_provider = arg_string(&subcommand.matches, "llm_provider");
            let forced_llm_model = arg_string(&subcommand.matches, "llm_model");

            let repeat: usize = arg_u64(&subcommand.matches, "repeat")
                .unwrap_or(1)
                .clamp(1, 50) as usize;
            let warmup: usize = arg_u64(&subcommand.matches, "warmup")
                .unwrap_or(0)
                .clamp(0, 50) as usize;

            let total_start = std::time::Instant::now();
            let read_start = std::time::Instant::now();
            let wav_bytes = std::fs::read(&file_path)
                .map_err(|err| CliError::Runtime(format!("Failed to read file: {err}")))?;
            let read_ms = read_start.elapsed().as_millis() as u64;
            if wav_bytes.is_empty() {
                return Err(CliError::Validation("Audio file is empty".to_string()));
            }

            let wav_info = crate::cli::wav_info::parse_wav_info(&wav_bytes);
            let wav_duration_secs_est = wav_info.and_then(|w| w.duration_secs_f64());

            let pipeline = app.state::<SharedPipeline>();
            let request_log_store: Option<RequestLogStore> =
                app.try_state::<RequestLogStore>().map(|s| (*s).clone());

            let mut runs: Vec<serde_json::Value> = Vec::new();
            let mut last_result: Option<crate::pipeline::TranscriptionResult> = None;
            let mut warmup_errors: Vec<String> = Vec::new();
            let mut run_wall_ms_values: Vec<u64> = Vec::new();
            let mut stt_ms_values: Vec<u64> = Vec::new();
            let mut llm_ms_values: Vec<u64> = Vec::new();

            for i in 0..(warmup + repeat) {
                let is_warmup = i < warmup;
                let run_start = std::time::Instant::now();
                let clone_start = std::time::Instant::now();
                let bytes_for_run = wav_bytes.clone();
                let clone_ms = clone_start.elapsed().as_millis() as u64;

                let before_count = request_log_store.as_ref().map(|s| s.count());
                let result = tauri::async_runtime::block_on(
                    pipeline.transcribe_wav_bytes_detailed_for_profile_with_llm_overrides(
                        bytes_for_run,
                        profile_id.as_deref(),
                        forced_llm_provider.as_deref(),
                        forced_llm_model.as_deref(),
                    ),
                );
                let run_wall_ms = run_start.elapsed().as_millis() as u64;
                let after_count = request_log_store.as_ref().map(|s| s.count());

                match result {
                    Ok(r) => {
                        last_result = Some(r.clone());

                        let created_new_log = match (before_count, after_count) {
                            (Some(b), Some(a)) => a > b,
                            _ => false,
                        };
                        let request_log = if created_new_log {
                            request_log_store
                                .as_ref()
                                .and_then(|s| s.get_logs(Some(1)).into_iter().next())
                                .map(request_log_summary)
                        } else {
                            None
                        };

                        if !is_warmup {
                            run_wall_ms_values.push(run_wall_ms);
                            stt_ms_values.push(r.stt_duration_ms);
                            llm_ms_values.push(r.llm_duration_ms.unwrap_or(0));
                            runs.push(json!({
                                "index": (i - warmup) + 1,
                                "run_wall_ms": run_wall_ms,
                                "wav_bytes_clone_ms": clone_ms,
                                "stt_duration_ms": r.stt_duration_ms,
                                "stt_retry": r.stt_retry,
                                "llm_duration_ms": r.llm_duration_ms,
                                "llm_outcome": r.llm_outcome.code(),
                                "llm_provider_used": r.llm_provider_used,
                                "llm_model_used": r.llm_model_used,
                                "request_log": request_log,
                            }));
                        }
                    }
                    Err(err) => {
                        let msg = err.to_string();
                        if is_warmup {
                            warmup_errors.push(msg);
                        } else {
                            return Err(CliError::Runtime(msg));
                        }
                    }
                }
            }

            let total_wall_ms = total_start.elapsed().as_millis() as u64;
            let Some(result) = last_result else {
                return Err(CliError::Runtime(
                    "Transcription failed during warmup runs (no successful run)".to_string(),
                ));
            };

            let summary = if repeat > 1 {
                Some(json!({
                    "run_wall_ms": summarize_u64_samples(&run_wall_ms_values),
                    "stt_duration_ms": summarize_u64_samples(&stt_ms_values),
                    "llm_duration_ms": summarize_u64_samples(&llm_ms_values),
                }))
            } else {
                None
            };

            let diagnostics = json!({
                "file": {
                    "path": file_path,
                    "size_bytes": wav_bytes.len(),
                    "read_ms": read_ms,
                },
                "llm_override": {
                    "provider": forced_llm_provider,
                    "model": forced_llm_model,
                },
                "wav": wav_info.map(|w| json!({
                    "sample_rate": w.sample_rate,
                    "channels": w.channels,
                    "bits_per_sample": w.bits_per_sample,
                    "data_bytes": w.data_bytes,
                    "duration_secs_est": wav_duration_secs_est,
                })),
                "timings": {
                    "total_wall_ms": total_wall_ms,
                    "warmup_runs": warmup,
                    "measured_runs": repeat,
                },
                "warmup_errors": warmup_errors,
            });

            let payload = json!({
                "final_text": result.final_text,
                "stt_text": result.stt_text,
                "stt_duration_ms": result.stt_duration_ms,
                "stt_retry": result.stt_retry,
                "llm_duration_ms": result.llm_duration_ms,
                "llm_provider_used": result.llm_provider_used,
                "llm_model_used": result.llm_model_used,
                "llm_outcome": result.llm_outcome.code(),
                "diagnostics": diagnostics,
                "summary": summary,
                "runs": if repeat > 1 { Some(runs) } else { None },
            });

            Ok((
                output_format,
                CommandResult::success(Some(payload), Some("Transcription complete".to_string())),
            ))
        }
        "status" => {
            let output_format = output_format_from(&subcommand.matches);
            let state =
                crate::commands::recording::pipeline_get_state(app.state::<SharedPipeline>())
                    .map_err(|err| CliError::Runtime(err.to_string()))?;

            Ok((
                output_format,
                CommandResult::success(
                    Some(json!({ "state": state })),
                    Some(format!("Pipeline state: {}", state)),
                ),
            ))
        }
        _ => Err(CliError::Validation(format!(
            "Unknown pipeline subcommand: {}",
            subcommand.name
        ))),
    }
}

fn request_log_summary(log: RequestLog) -> serde_json::Value {
    let (debug_count, info_count, warn_count, error_count) = log.entries.iter().fold(
        (0u64, 0u64, 0u64, 0u64),
        |(d, i, w, e), entry| match entry.level {
            crate::request_log::LogLevel::Debug => (d + 1, i, w, e),
            crate::request_log::LogLevel::Info => (d, i + 1, w, e),
            crate::request_log::LogLevel::Warn => (d, i, w + 1, e),
            crate::request_log::LogLevel::Error => (d, i, w, e + 1),
        },
    );

    let status_str = match log.status {
        RequestStatus::InProgress => "in_progress",
        RequestStatus::Success => "success",
        RequestStatus::Error => "error",
        RequestStatus::Cancelled => "cancelled",
    };

    json!({
        "id": log.id,
        "kind": format!("{:?}", log.kind).to_lowercase(),
        "status": status_str,
        "error_message": log.error_message,
        "total_duration_ms": log.total_duration_ms,
        "stt_duration_ms": log.stt_duration_ms,
        "llm_duration_ms": log.llm_duration_ms,
        "router_duration_ms": log.router_duration_ms,
        "router_strategy": log.router_strategy,
        "ocr_status": log.ocr_status,
        "ocr_duration_ms": log.ocr_duration_ms,
        "ocr_not_attempted_reason": log.ocr_not_attempted_reason,
        "ocr_failed_reason": log.ocr_failed_reason,
        "llm_outcome": log.llm_outcome,
        "llm_not_attempted_reason": log.llm_not_attempted_reason,
        "llm_error_message": log.llm_error_message,
        "stt_provider": log.stt_provider,
        "stt_model": log.stt_model,
        "llm_provider": log.llm_provider,
        "llm_model": log.llm_model,
        "audio_duration_secs": log.audio_duration_secs,
        "audio_size_bytes": log.audio_size_bytes,
        "sample_rate": log.sample_rate,
        "entries": {
            "debug": debug_count,
            "info": info_count,
            "warn": warn_count,
            "error": error_count,
        }
    })
}

fn summarize_u64_samples(samples: &[u64]) -> serde_json::Value {
    if samples.is_empty() {
        return json!({
            "count": 0,
        });
    }
    let mut min = u64::MAX;
    let mut max = 0u64;
    let mut sum: u128 = 0;
    for &v in samples {
        min = min.min(v);
        max = max.max(v);
        sum += v as u128;
    }
    let avg = (sum / samples.len() as u128) as u64;
    json!({
        "count": samples.len(),
        "min": min,
        "max": max,
        "avg": avg,
    })
}
