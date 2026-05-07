use serde_json::json;
use tauri::{AppHandle, Manager};
use tauri_plugin_cli::Matches;

use crate::cli::{
    arg_string, arg_u64, output_format_from, CliError, CliOutputFormat, CommandResult,
};
use crate::pipeline::SharedPipeline;
use crate::request_log::{RequestLog, RequestLogStore, RequestStatus};

fn arg_f64(matches: &Matches, name: &str) -> Option<f64> {
    let value = matches.args.get(name).map(|arg| &arg.value)?;
    match value {
        serde_json::Value::Number(n) => n.as_f64(),
        serde_json::Value::String(s) => s.trim().parse::<f64>().ok(),
        serde_json::Value::Null => None,
        _ => None,
    }
}

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
            let forced_stt_provider = arg_string(&subcommand.matches, "stt_provider");
            let forced_stt_model = arg_string(&subcommand.matches, "stt_model");
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
            let mut last_request_log: Option<serde_json::Value> = None;
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

                // When invoking the pipeline via CLI, there is no recording-start hook to
                // automatically create a request log. Create one so providers can attach
                // useful debug metadata (e.g. realtime WS endpoint, chunks sent).
                if let Some(store) = &request_log_store {
                    let provider_for_log = forced_stt_provider
                        .clone()
                        .unwrap_or_else(|| "auto".to_string());
                    let model_for_log = forced_stt_model.clone();
                    store.start_request(provider_for_log, model_for_log);
                    let _ = store.with_current(|log| {
                        log.mark_processing_started();
                    });
                }

                let result = tauri::async_runtime::block_on(
                    pipeline.transcribe_wav_bytes_detailed_for_profile_with_llm_overrides(
                        bytes_for_run,
                        profile_id.as_deref(),
                        forced_stt_provider.as_deref(),
                        forced_stt_model.as_deref(),
                        forced_llm_provider.as_deref(),
                        forced_llm_model.as_deref(),
                    ),
                );
                let run_wall_ms = run_start.elapsed().as_millis() as u64;
                let after_count = request_log_store.as_ref().map(|s| s.count());

                match result {
                    Ok(r) => {
                        if let Some(store) = &request_log_store {
                            let _ = store.with_current(|log| {
                                log.complete_success();
                            });
                            store.complete_current();
                        }

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
                            if request_log.is_some() {
                                last_request_log = request_log.clone();
                            }
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
                        if let Some(store) = &request_log_store {
                            let _ = store.with_current(|log| {
                                log.complete_error(err.to_string());
                            });
                            store.complete_current();
                        }

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
                "request_log": if repeat == 1 { last_request_log } else { None },
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
        "stream" => handle_stream(app, &subcommand.matches),
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

    // Request logs can include large payloads; keep CLI output small and focused.
    // This is particularly useful for verifying whether STT used realtime WS vs batch HTTP.
    let stt_request_endpoint = log
        .stt_request_json
        .as_ref()
        .and_then(|v| v.get("endpoint"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let stt_request_content_type = log
        .stt_request_json
        .as_ref()
        .and_then(|v| v.get("content_type"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let stt_request_model_id = log
        .stt_request_json
        .as_ref()
        .and_then(|v| v.get("fields"))
        .and_then(|v| v.get("model_id"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let stt_response_chunks_sent = log
        .stt_response_json
        .as_ref()
        .and_then(|v| v.get("chunks_sent"))
        .and_then(|v| v.as_u64());

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
        "stt_request": {
            "endpoint": stt_request_endpoint,
            "content_type": stt_request_content_type,
            "model_id": stt_request_model_id,
        },
        "stt_response": {
            "chunks_sent": stt_response_chunks_sent,
        },
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

/// Handle the `pipeline stream` subcommand.
///
/// This feeds a WAV file through a streaming STT provider session (the same WS
/// path used during live recording) and prints every `PartialTranscript` as it
/// arrives. This lets you verify committed-text chunks, timing, and ordering
/// without needing the full GUI recording flow.
fn handle_stream(
    app: &AppHandle,
    matches: &Matches,
) -> Result<(CliOutputFormat, CommandResult<serde_json::Value>), CliError> {
    let output_format = output_format_from(matches);

    let file_path = arg_string(matches, "file")
        .ok_or_else(|| CliError::Validation("Missing --file (WAV file path)".to_string()))?;
    let provider_id = arg_string(matches, "stt_provider")
        .ok_or_else(|| CliError::Validation("Missing --stt_provider".to_string()))?;
    let model = arg_string(matches, "stt_model");
    let language = arg_string(matches, "language");
    let speed: f64 = arg_f64(matches, "speed").unwrap_or(1.0);

    // Read and parse WAV file.
    let wav_bytes = std::fs::read(&file_path)
        .map_err(|e| CliError::Runtime(format!("Failed to read file: {e}")))?;
    if wav_bytes.is_empty() {
        return Err(CliError::Validation("Audio file is empty".to_string()));
    }
    let wav_info = crate::cli::wav_info::parse_wav_info(&wav_bytes)
        .ok_or_else(|| CliError::Validation("Could not parse WAV header".to_string()))?;

    if wav_info.bits_per_sample != 16 {
        return Err(CliError::Validation(format!(
            "Only 16-bit WAV files supported (got {} bits)",
            wav_info.bits_per_sample
        )));
    }

    let sample_rate = wav_info.sample_rate;
    let channels = wav_info.channels as usize;

    // Locate the data chunk.
    let data_start = find_wav_data_offset(&wav_bytes)
        .ok_or_else(|| CliError::Validation("Could not find WAV data chunk".to_string()))?;
    let data_end = data_start + wav_info.data_bytes as usize;
    if data_end > wav_bytes.len() {
        return Err(CliError::Validation(
            "WAV data chunk extends beyond file".to_string(),
        ));
    }
    let pcm_data = &wav_bytes[data_start..data_end];

    // Convert interleaved i16 PCM → mono f32 in [-1, 1].
    let samples_i16: Vec<i16> = pcm_data
        .chunks_exact(2)
        .map(|b| i16::from_le_bytes([b[0], b[1]]))
        .collect();

    let mono_f32: Vec<f32> = if channels == 1 {
        samples_i16
            .iter()
            .map(|&s| s as f32 / i16::MAX as f32)
            .collect()
    } else {
        // Down-mix to mono by averaging channels.
        samples_i16
            .chunks(channels)
            .map(|frame| {
                let sum: f32 = frame.iter().map(|&s| s as f32 / i16::MAX as f32).sum();
                sum / channels as f32
            })
            .collect()
    };

    let total_samples = mono_f32.len();
    let duration_secs = total_samples as f64 / sample_rate as f64;

    eprintln!(
        "Streaming: {} ({:.1}s, {}Hz, {} ch) → provider={} model={} lang={} speed={}",
        file_path,
        duration_secs,
        sample_rate,
        wav_info.channels,
        provider_id,
        model.as_deref().unwrap_or("<default>"),
        language.as_deref().unwrap_or("<auto>"),
        if speed <= 0.0 {
            "max".to_string()
        } else {
            format!("{speed:.1}x")
        },
    );

    // Resolve API key.
    let api_key_name = format!("{}_api_key", provider_id);
    let api_key = crate::secrets::get_api_key(app, &api_key_name).unwrap_or_default();
    if api_key.is_empty() {
        return Err(CliError::Runtime(format!(
            "No API key configured for provider '{provider_id}' (expected setting: {api_key_name})"
        )));
    }

    // Request log store for provider diagnostics.
    let request_log_store: Option<RequestLogStore> =
        app.try_state::<RequestLogStore>().map(|s| (*s).clone());

    if let Some(store) = &request_log_store {
        store.start_request(provider_id.clone(), model.clone());
        let _ = store.with_current(|log| {
            log.mark_processing_started();
        });
    }

    // Create the STT provider.
    let proxy_settings: crate::settings::ProxySettings = crate::get_setting_from_store(
        app,
        "proxy_settings",
        crate::settings::ProxySettings::default(),
    );
    let client = crate::pipeline::stt_provider::build_stt_client(&proxy_settings)
        .map_err(|e| CliError::Runtime(format!("Failed to create HTTP client: {e}")))?;

    let provider = crate::pipeline::stt_provider::create_cloud_stt_provider(
        client,
        crate::pipeline::stt_provider::SttProviderParams {
            provider_id: provider_id.clone(),
            model: model.clone(),
            language: language.clone(),
            api_key,
            managed_gateway_url: None,
            transcription_prompt: None,
            request_log_store: request_log_store.clone(),
            stt_live_output: true,
        },
    )
    .map_err(|e| CliError::Runtime(format!("Failed to create STT provider: {e}")))?;

    if !provider.supports_streaming() {
        return Err(CliError::Validation(format!(
            "Provider '{provider_id}' does not support streaming"
        )));
    }

    // Run the streaming session on the async runtime.
    let result = tauri::async_runtime::block_on(async {
        run_stream_session(
            provider,
            &mono_f32,
            sample_rate,
            speed,
            request_log_store,
            proxy_settings,
        )
        .await
    })
    .map_err(|e| CliError::Runtime(e.to_string()))?;

    let tail_latency_ms = result.session_ms.saturating_sub(result.feed_done_ms);
    let payload = json!({
        "final_text": result.final_text,
        "duration_secs": duration_secs,
        "sample_rate": sample_rate,
        "total_partials": result.total_partials,
        "total_commits": result.total_commits,
        "session_ms": result.session_ms,
        "feed_done_ms": result.feed_done_ms,
        "tail_latency_ms": tail_latency_ms,
        "partials": result.partials,
    });

    Ok((
        output_format,
        CommandResult::success(Some(payload), Some("Streaming complete".to_string())),
    ))
}

struct StreamResult {
    final_text: String,
    total_partials: usize,
    total_commits: usize,
    session_ms: u64,
    /// Elapsed ms when the audio feed finished (all chunks sent, `audio_tx` dropped).
    feed_done_ms: u64,
    partials: Vec<serde_json::Value>,
}

async fn run_stream_session(
    provider: std::sync::Arc<dyn crate::stt::SttProvider>,
    mono_f32: &[f32],
    sample_rate: u32,
    speed: f64,
    request_log_store: Option<RequestLogStore>,
    proxy_settings: crate::settings::ProxySettings,
) -> Result<StreamResult, crate::stt::SttError> {
    if let Some(message) =
        crate::stt::streaming::describe_websocket_transport_policy_gap(&proxy_settings)
    {
        eprintln!("{}", message);
        if let Some(store) = &request_log_store {
            let warning = message.clone();
            let _ = store.with_current(|log| {
                log.warn(warning);
            });
        }
    }

    // Start the streaming session.
    let mut session = provider.start_streaming(sample_rate).await?;
    let audio_tx = session.audio_tx.clone();
    let partial_rx = session
        .take_partial_rx()
        .ok_or_else(|| crate::stt::SttError::Config("No partial_rx available".to_string()))?;

    let session_start = std::time::Instant::now();

    // Spawn a task to feed audio chunks.
    let chunk_duration_ms: u64 = 100; // 100ms chunks
    let chunk_samples = (sample_rate as u64 * chunk_duration_ms / 1000) as usize;
    let samples = mono_f32.to_vec();
    let feed_task = tokio::spawn(async move {
        let mut offset = 0usize;
        let mut chunk_index = 0u64;
        while offset < samples.len() {
            let end = (offset + chunk_samples).min(samples.len());
            let chunk = samples[offset..end].to_vec();
            if audio_tx.send(chunk).await.is_err() {
                break;
            }
            offset = end;
            chunk_index += 1;

            // Simulate real-time pacing (unless speed is 0 = max speed).
            if speed > 0.0 {
                let delay_ms = (chunk_duration_ms as f64 / speed) as u64;
                tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
            } else {
                // Yield to let the runtime breathe.
                tokio::task::yield_now().await;
            }
        }
        eprintln!(
            "Feed: sent {} chunks ({} samples)",
            chunk_index,
            samples.len()
        );
        // Drop audio_tx to signal end-of-audio.
    });

    // Spawn a task to collect partial transcripts.
    let collect_start = session_start;
    let collect_task =
        tokio::spawn(async move { collect_partials(partial_rx, collect_start).await });

    // Wait for feeding to finish (this drops audio_tx).
    let _ = feed_task.await;
    let feed_done_ms = session_start.elapsed().as_millis() as u64;

    // Finalize the session (waits for provider to finish processing).
    let final_text = session.finalize().await?;

    // Collect results from the partial consumer.
    let (partials, total_partials, total_commits) = collect_task
        .await
        .map_err(|e| crate::stt::SttError::NetworkMessage(format!("Collect task panicked: {e}")))?;

    let session_ms = session_start.elapsed().as_millis() as u64;

    let tail_latency_ms = session_ms.saturating_sub(feed_done_ms);
    eprintln!(
        "\nStream complete: {} chars, {} partials, {} commits, {}ms (feed={}ms, tail={}ms)",
        final_text.len(),
        total_partials,
        total_commits,
        session_ms,
        feed_done_ms,
        tail_latency_ms,
    );

    if let Some(store) = &request_log_store {
        let _ = store.with_current(|log| {
            log.complete_success();
        });
        store.complete_current();
    }

    Ok(StreamResult {
        final_text,
        total_partials,
        total_commits,
        session_ms,
        feed_done_ms,
        partials,
    })
}

async fn collect_partials(
    mut partial_rx: tokio::sync::mpsc::Receiver<crate::stt::streaming::PartialTranscript>,
    session_start: std::time::Instant,
) -> (Vec<serde_json::Value>, usize, usize) {
    let mut partials = Vec::new();
    let mut total_partials = 0usize;
    let mut total_commits = 0usize;

    while let Some(partial) = partial_rx.recv().await {
        total_partials += 1;
        let elapsed_ms = session_start.elapsed().as_millis() as u64;
        let has_commit = partial.committed_text.is_some();

        if has_commit {
            total_commits += 1;
        }

        // Print to stderr for real-time visibility.
        if let Some(ref committed) = partial.committed_text {
            eprintln!(
                "[{:>6}ms] COMMIT #{}: {:?}",
                elapsed_ms, total_commits, committed
            );
        } else {
            eprintln!(
                "[{:>6}ms] partial: {:?}",
                elapsed_ms,
                truncate_for_display(&partial.text, 80)
            );
        }

        partials.push(json!({
            "elapsed_ms": elapsed_ms,
            "text": partial.text,
            "committed_text": partial.committed_text,
        }));
    }

    (partials, total_partials, total_commits)
}

fn truncate_for_display(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_len).collect();
        format!("{truncated}…")
    }
}

/// Find the byte offset of the start of the WAV "data" chunk payload.
fn find_wav_data_offset(wav_bytes: &[u8]) -> Option<usize> {
    if wav_bytes.len() < 44 {
        return None;
    }
    let mut offset = 12usize; // Skip RIFF header + WAVE tag
    while offset + 8 <= wav_bytes.len() {
        let chunk_id = &wav_bytes[offset..offset + 4];
        let chunk_size = u32::from_le_bytes([
            wav_bytes[offset + 4],
            wav_bytes[offset + 5],
            wav_bytes[offset + 6],
            wav_bytes[offset + 7],
        ]) as usize;
        if chunk_id == b"data" {
            return Some(offset + 8);
        }
        offset += 8 + chunk_size + (chunk_size % 2);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::request_log_summary;
    use crate::request_log::RequestLog;
    use serde_json::json;

    #[test]
    fn request_log_summary_extracts_stt_request_endpoint() {
        let mut log = RequestLog::new("elevenlabs".to_string(), Some("scribe_v2".to_string()));
        log.stt_request_json = Some(json!({
            "endpoint": "wss://api.elevenlabs.io/v1/speech-to-text/realtime?model_id=scribe_v2_realtime",
            "content_type": "websocket-json",
            "fields": { "model_id": "scribe_v2_realtime" }
        }));
        log.stt_response_json = Some(json!({ "chunks_sent": 12 }));

        let summary = request_log_summary(log);
        assert_eq!(
            summary
                .get("stt_request")
                .and_then(|v| v.get("endpoint"))
                .and_then(|v| v.as_str()),
            Some("wss://api.elevenlabs.io/v1/speech-to-text/realtime?model_id=scribe_v2_realtime")
        );
        assert_eq!(
            summary
                .get("stt_request")
                .and_then(|v| v.get("content_type"))
                .and_then(|v| v.as_str()),
            Some("websocket-json")
        );
        assert_eq!(
            summary
                .get("stt_request")
                .and_then(|v| v.get("model_id"))
                .and_then(|v| v.as_str()),
            Some("scribe_v2_realtime")
        );
        assert_eq!(
            summary
                .get("stt_response")
                .and_then(|v| v.get("chunks_sent"))
                .and_then(|v| v.as_u64()),
            Some(12)
        );
    }
}
