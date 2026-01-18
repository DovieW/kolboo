//! Tauri commands for local Whisper model management.
//!
//! These commands are only available when the `local-whisper` feature is enabled.

#[cfg(feature = "local-whisper")]
use crate::stt::WhisperModel;
use schemars::JsonSchema;
#[cfg(feature = "local-whisper")]
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{Emitter, Manager};
use tokio::sync::{Mutex, Semaphore};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, serde::Serialize, JsonSchema)]
pub struct LocalWhisperBackendObserved {
    pub nvidia_smi_available: bool,
    pub pid: u32,
    pub cuda_process_present: Option<bool>,
    pub used_gpu_memory_mb: Option<u64>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LocalWhisperComputeBackend {
    Cpu,
    Cuda,
}

#[derive(Debug, Clone, serde::Serialize, JsonSchema)]
pub struct LocalWhisperBackendStatusResponse {
    pub build_has_local_whisper: bool,
    pub build_has_cuda: bool,
    pub compute: LocalWhisperComputeBackend,
    pub reason: Option<String>,
    pub missing_dlls: Vec<String>,
    pub observed: LocalWhisperBackendObserved,
}

#[cfg(target_os = "windows")]
fn observe_cuda_usage_via_nvidia_smi() -> LocalWhisperBackendObserved {
    use std::process::Command;

    let pid = std::process::id();

    let output = Command::new("nvidia-smi")
        .args([
            "--query-compute-apps=pid,used_gpu_memory",
            "--format=csv,noheader,nounits",
        ])
        .output();

    let Ok(output) = output else {
        return LocalWhisperBackendObserved {
            nvidia_smi_available: false,
            pid,
            cuda_process_present: None,
            used_gpu_memory_mb: None,
            error: Some("Failed to launch nvidia-smi".to_string()),
        };
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return LocalWhisperBackendObserved {
            nvidia_smi_available: false,
            pid,
            cuda_process_present: None,
            used_gpu_memory_mb: None,
            error: Some(if stderr.is_empty() {
                "nvidia-smi failed".to_string()
            } else {
                stderr
            }),
        };
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut present = false;
    let mut used_mb: Option<u64> = None;

    for line in stdout.lines() {
        // Expected: "1234, 256" (csv, no header, no units)
        let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
        if parts.len() < 2 {
            continue;
        }
        let Ok(row_pid) = parts[0].parse::<u32>() else {
            continue;
        };
        if row_pid != pid {
            continue;
        }

        present = true;
        if let Ok(mb) = parts[1].parse::<u64>() {
            used_mb = Some(mb);
        }
        break;
    }

    LocalWhisperBackendObserved {
        nvidia_smi_available: true,
        pid,
        cuda_process_present: Some(present),
        used_gpu_memory_mb: used_mb,
        error: None,
    }
}

#[cfg(not(target_os = "windows"))]
fn observe_cuda_usage_via_nvidia_smi() -> LocalWhisperBackendObserved {
    LocalWhisperBackendObserved {
        nvidia_smi_available: false,
        pid: std::process::id(),
        cuda_process_present: None,
        used_gpu_memory_mb: None,
        error: Some("nvidia-smi observation is only implemented on Windows".to_string()),
    }
}

#[cfg(feature = "local-whisper")]
pub const WHISPER_MODEL_DOWNLOAD_PROGRESS_EVENT: &str = "whisper-model-download-progress";

pub const LOCAL_WHISPER_MODEL_LOAD_EVENT: &str = "local-whisper-model-load";

#[derive(Debug, Clone, serde::Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LocalWhisperModelLoadStatus {
    Started,
    Completed,
    Error,
}

#[derive(Debug, Clone, serde::Serialize, JsonSchema)]
pub struct LocalWhisperModelLoadEvent {
    pub status: LocalWhisperModelLoadStatus,
    pub message: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WhisperModelDownloadStatus {
    Queued,
    Downloading,
    Verifying,
    Completed,
    Cancelled,
    Error,
}

#[derive(Debug, Clone, serde::Serialize, JsonSchema)]
pub struct WhisperModelDownloadProgress {
    pub model_id: String,
    pub status: WhisperModelDownloadStatus,
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
    pub percent: Option<f64>,
    pub message: Option<String>,
}

#[derive(Debug)]
struct DownloadJob {
    cancel: CancellationToken,
}

/// Manages concurrent Whisper model downloads (shared across windows).
///
/// This is always constructed (even when the feature flag is off) so the frontend
/// can call commands and get a consistent error message.
#[cfg_attr(not(feature = "local-whisper"), allow(dead_code))]
#[derive(Debug)]
pub struct WhisperDownloadManager {
    client: reqwest::Client,
    semaphore: Arc<Semaphore>,
    jobs: Arc<Mutex<HashMap<String, DownloadJob>>>,
}

impl Default for WhisperDownloadManager {
    fn default() -> Self {
        Self::new(3)
    }
}

impl WhisperDownloadManager {
    pub fn new(max_concurrent: usize) -> Self {
        Self {
            client: reqwest::Client::new(),
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
            jobs: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

/// Error type for Whisper commands
#[derive(Debug, serde::Serialize)]
pub struct WhisperCommandError {
    pub message: String,
}

impl From<String> for WhisperCommandError {
    fn from(message: String) -> Self {
        Self { message }
    }
}

/// Information about a Whisper model
#[derive(Debug, serde::Serialize, JsonSchema)]
pub struct WhisperModelInfo {
    pub id: String,
    pub name: String,
    pub filename: String,
    pub size_bytes: u64,
    pub size_display: String,
    pub download_url: String,
    pub expected_sha256: String,
    pub is_english_only: bool,
    pub is_downloaded: bool,
}

/// Check if local Whisper feature is enabled
#[tauri::command]
pub fn is_local_whisper_available() -> bool {
    cfg!(feature = "local-whisper")
}

/// Returns diagnostic information about whether Local Whisper will use CPU or CUDA.
///
/// This is used by the UI to show "GPU vs CPU" and why GPU might be unavailable.
#[tauri::command]
pub fn get_local_whisper_backend_status() -> LocalWhisperBackendStatusResponse {
    #[cfg(feature = "local-whisper")]
    {
        let s = crate::stt::get_local_whisper_backend_status();
        let observed = observe_cuda_usage_via_nvidia_smi();
        let compute = match s.compute {
            crate::stt::LocalWhisperComputeBackend::Cpu => LocalWhisperComputeBackend::Cpu,
            crate::stt::LocalWhisperComputeBackend::Cuda => LocalWhisperComputeBackend::Cuda,
        };

        return LocalWhisperBackendStatusResponse {
            build_has_local_whisper: s.build_has_local_whisper,
            build_has_cuda: s.build_has_cuda,
            compute,
            reason: s.reason,
            missing_dlls: s.missing_dlls,
            observed,
        };
    }

    #[cfg(not(feature = "local-whisper"))]
    {
        LocalWhisperBackendStatusResponse {
            build_has_local_whisper: false,
            build_has_cuda: false,
            compute: LocalWhisperComputeBackend::Cpu,
            reason: Some("Local Whisper feature is not enabled in this build.".to_string()),
            missing_dlls: Vec::new(),
            observed: observe_cuda_usage_via_nvidia_smi(),
        }
    }
}

fn get_pipeline(
    app: &tauri::AppHandle,
) -> Result<crate::pipeline::SharedPipeline, WhisperCommandError> {
    app.try_state::<crate::pipeline::SharedPipeline>()
        .map(|s| s.inner().clone())
        .ok_or_else(|| WhisperCommandError::from("Pipeline not initialized".to_string()))
}

/// Returns true if the current local-whisper model is already loaded in-memory.
#[tauri::command]
pub fn is_local_whisper_model_loaded(app: tauri::AppHandle) -> Result<bool, WhisperCommandError> {
    let pipeline = get_pipeline(&app)?;
    Ok(pipeline.is_local_whisper_loaded())
}

/// Force-load the local-whisper model into memory.
///
/// This is intended for the UI "Load model" button when load mode is "manual".
#[tauri::command]
pub fn load_local_whisper_model(app: tauri::AppHandle) -> Result<(), WhisperCommandError> {
    let pipeline = get_pipeline(&app)?;

    // Emit a "started" event immediately so the UI can show a toast / spinner.
    let _ = app.emit(
        LOCAL_WHISPER_MODEL_LOAD_EVENT,
        LocalWhisperModelLoadEvent {
            status: LocalWhisperModelLoadStatus::Started,
            message: None,
        },
    );

    let app_for_emit = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let result = pipeline.force_load_local_whisper();

        let payload = match result {
            Ok(()) => LocalWhisperModelLoadEvent {
                status: LocalWhisperModelLoadStatus::Completed,
                message: None,
            },
            Err(e) => LocalWhisperModelLoadEvent {
                status: LocalWhisperModelLoadStatus::Error,
                message: Some(e.to_string()),
            },
        };

        let _ = app_for_emit.emit(LOCAL_WHISPER_MODEL_LOAD_EVENT, payload);
    });

    Ok(())
}

/// Unload (evict) any cached local-whisper models.
#[tauri::command]
pub fn unload_local_whisper_model(app: tauri::AppHandle) -> Result<(), WhisperCommandError> {
    let pipeline = get_pipeline(&app)?;
    pipeline
        .unload_local_whisper()
        .map_err(|e| WhisperCommandError::from(e.to_string()))
}

/// Get list of available Whisper models with download status
#[tauri::command]
pub fn get_whisper_models(
    app: tauri::AppHandle,
) -> Result<Vec<WhisperModelInfo>, WhisperCommandError> {
    #[cfg(feature = "local-whisper")]
    {
        let models_dir = get_models_dir(&app)?;

        let models: Vec<WhisperModelInfo> = WhisperModel::all()
            .into_iter()
            .map(|model| {
                let model_path = models_dir.join(model.filename());
                let is_downloaded = model_path.exists();

                WhisperModelInfo {
                    id: format!("{:?}", model).to_lowercase(),
                    name: model.display_name().to_string(),
                    filename: model.filename().to_string(),
                    size_bytes: model.size_bytes(),
                    size_display: format_size(model.size_bytes()),
                    download_url: model.download_url(),
                    expected_sha256: model.expected_sha256().to_string(),
                    is_english_only: model.is_english_only(),
                    is_downloaded,
                }
            })
            .collect();

        Ok(models)
    }

    #[cfg(not(feature = "local-whisper"))]
    {
        let _ = app;
        Err(WhisperCommandError::from(
            "Local Whisper feature is not enabled".to_string(),
        ))
    }
}

/// Get the path to the models directory
#[tauri::command]
pub fn get_whisper_models_dir(app: tauri::AppHandle) -> Result<String, WhisperCommandError> {
    let models_dir = get_models_dir(&app)?;
    Ok(models_dir.to_string_lossy().to_string())
}

/// Check if a specific model is downloaded
#[tauri::command]
pub fn is_whisper_model_downloaded(
    app: tauri::AppHandle,
    model_id: String,
) -> Result<bool, WhisperCommandError> {
    #[cfg(feature = "local-whisper")]
    {
        let model = parse_model_id(&model_id)?;
        let models_dir = get_models_dir(&app)?;
        let model_path = models_dir.join(model.filename());
        Ok(model_path.exists())
    }

    #[cfg(not(feature = "local-whisper"))]
    {
        let _ = (app, model_id);
        Err(WhisperCommandError::from(
            "Local Whisper feature is not enabled".to_string(),
        ))
    }
}

/// Get the download URL for a model
#[tauri::command]
pub fn get_whisper_model_url(model_id: String) -> Result<String, WhisperCommandError> {
    #[cfg(feature = "local-whisper")]
    {
        let model = parse_model_id(&model_id)?;
        Ok(model.download_url())
    }

    #[cfg(not(feature = "local-whisper"))]
    {
        let _ = model_id;
        Err(WhisperCommandError::from(
            "Local Whisper feature is not enabled".to_string(),
        ))
    }
}

/// Delete a downloaded model
#[tauri::command]
pub fn delete_whisper_model(
    app: tauri::AppHandle,
    model_id: String,
) -> Result<(), WhisperCommandError> {
    #[cfg(feature = "local-whisper")]
    {
        let model = parse_model_id(&model_id)?;
        let models_dir = get_models_dir(&app)?;
        let model_path = models_dir.join(model.filename());

        if model_path.exists() {
            std::fs::remove_file(&model_path)
                .map_err(|e| WhisperCommandError::from(format!("Failed to delete model: {}", e)))?;
            log::info!("Deleted Whisper model: {}", model.filename());
        }

        Ok(())
    }

    #[cfg(not(feature = "local-whisper"))]
    {
        let _ = (app, model_id);
        Err(WhisperCommandError::from(
            "Local Whisper feature is not enabled".to_string(),
        ))
    }
}

/// Validate a downloaded model file
#[tauri::command]
pub fn validate_whisper_model(
    app: tauri::AppHandle,
    model_id: String,
) -> Result<bool, WhisperCommandError> {
    #[cfg(feature = "local-whisper")]
    {
        let model = parse_model_id(&model_id)?;
        let models_dir = get_models_dir(&app)?;
        let model_path = models_dir.join(model.filename());

        if !model_path.exists() {
            return Ok(false);
        }

        // Quick size sanity check (at least 50% of expected)
        let metadata = std::fs::metadata(&model_path).map_err(|e| {
            WhisperCommandError::from(format!("Failed to read model metadata: {}", e))
        })?;

        let expected_size = model.size_bytes();
        let actual_size = metadata.len();

        // Model should be at least 50% of expected size
        if actual_size < expected_size / 2 {
            log::warn!(
                "Model {} appears incomplete: {} bytes (expected ~{} bytes)",
                model_id,
                actual_size,
                expected_size
            );
            return Ok(false);
        }

        // SHA-256 validation (streamed; does not load the whole file into memory)
        let expected = model.expected_sha256();
        let mut hasher = Sha256::new();

        let mut file = std::fs::File::open(&model_path)
            .map_err(|e| WhisperCommandError::from(format!("Failed to open model file: {}", e)))?;

        use std::io::Read;
        let mut buf = vec![0u8; 8 * 1024 * 1024];
        loop {
            let n = file.read(&mut buf).map_err(|e| {
                WhisperCommandError::from(format!("Failed reading model file: {}", e))
            })?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
        }

        let actual = format!("{:x}", hasher.finalize());
        if actual != expected {
            log::warn!(
                "Model {} SHA mismatch. Expected {}, got {}",
                model_id,
                expected,
                actual
            );
            return Ok(false);
        }

        Ok(true)
    }

    #[cfg(not(feature = "local-whisper"))]
    {
        let _ = (app, model_id);
        Err(WhisperCommandError::from(
            "Local Whisper feature is not enabled".to_string(),
        ))
    }
}

/// Download a Whisper model to the app data directory.
///
/// Emits progress events to `WHISPER_MODEL_DOWNLOAD_PROGRESS_EVENT`.
#[tauri::command]
pub async fn download_whisper_model(
    app: tauri::AppHandle,
    state: tauri::State<'_, WhisperDownloadManager>,
    model_id: String,
) -> Result<(), WhisperCommandError> {
    #[cfg(feature = "local-whisper")]
    {
        use futures_util::StreamExt;
        use tokio::io::AsyncWriteExt;

        let model = parse_model_id(&model_id)?;
        let models_dir = get_models_dir(&app)?;
        let model_path = models_dir.join(model.filename());

        // Basic validation: don't re-download if already present.
        if model_path.exists() {
            return Err(WhisperCommandError::from(format!(
                "Model already downloaded: {}",
                model_id
            )));
        }

        // Prevent duplicate in-flight downloads.
        let jobs = state.jobs.clone();
        let cancel = CancellationToken::new();

        {
            let mut jobs_guard = jobs.lock().await;
            if jobs_guard.contains_key(&model_id) {
                return Err(WhisperCommandError::from(format!(
                    "Model is already downloading: {}",
                    model_id
                )));
            }

            jobs_guard.insert(
                model_id.clone(),
                DownloadJob {
                    cancel: cancel.clone(),
                },
            );
        }

        app.emit(
            WHISPER_MODEL_DOWNLOAD_PROGRESS_EVENT,
            WhisperModelDownloadProgress {
                model_id: model_id.clone(),
                status: WhisperModelDownloadStatus::Queued,
                downloaded_bytes: 0,
                total_bytes: None,
                percent: None,
                message: None,
            },
        )
        .ok();

        let client = state.client.clone();
        let semaphore = state.semaphore.clone();
        let app_handle = app.clone();
        let jobs_for_task = jobs.clone();

        tauri::async_runtime::spawn(async move {
            let _permit = match semaphore.acquire_owned().await {
                Ok(p) => p,
                Err(_) => {
                    app_handle
                        .emit(
                            WHISPER_MODEL_DOWNLOAD_PROGRESS_EVENT,
                            WhisperModelDownloadProgress {
                                model_id: model_id.clone(),
                                status: WhisperModelDownloadStatus::Error,
                                downloaded_bytes: 0,
                                total_bytes: None,
                                percent: None,
                                message: Some("Download semaphore closed".to_string()),
                            },
                        )
                        .ok();
                    let mut jobs = jobs_for_task.lock().await;
                    jobs.remove(&model_id);
                    return;
                }
            };

            let filename = model.filename().to_string();
            let tmp_path = models_dir.join(format!("{}.part", filename));
            let url = model.download_url();
            let expected_sha = model.expected_sha256().to_string();

            let send_progress = |status: WhisperModelDownloadStatus,
                                 downloaded_bytes: u64,
                                 total_bytes: Option<u64>,
                                 message: Option<String>| {
                let percent = total_bytes.and_then(|t| {
                    if t == 0 {
                        None
                    } else {
                        Some((downloaded_bytes as f64 / t as f64) * 100.0)
                    }
                });

                app_handle
                    .emit(
                        WHISPER_MODEL_DOWNLOAD_PROGRESS_EVENT,
                        WhisperModelDownloadProgress {
                            model_id: model_id.clone(),
                            status,
                            downloaded_bytes,
                            total_bytes,
                            percent,
                            message,
                        },
                    )
                    .ok();
            };

            let result: Result<(), String> = async {
                send_progress(
                    WhisperModelDownloadStatus::Downloading,
                    0,
                    None,
                    Some("Starting download".to_string()),
                );

                let resp = client
                    .get(url)
                    .send()
                    .await
                    .map_err(|e| format!("Request failed: {}", e))?;

                if !resp.status().is_success() {
                    return Err(format!("Download failed (HTTP {})", resp.status()));
                }

                let total = resp.content_length();

                if let Some(parent) = tmp_path.parent() {
                    tokio::fs::create_dir_all(parent)
                        .await
                        .map_err(|e| format!("Failed to create models dir: {}", e))?;
                }

                let mut file = tokio::fs::File::create(&tmp_path)
                    .await
                    .map_err(|e| format!("Failed to create temp file: {}", e))?;

                let mut hasher = Sha256::new();
                let mut downloaded: u64 = 0;
                let mut stream = resp.bytes_stream();
                let mut last_emit = std::time::Instant::now();

                while let Some(next) = stream.next().await {
                    if cancel.is_cancelled() {
                        return Err("cancelled".to_string());
                    }

                    let chunk = next.map_err(|e| format!("Stream error: {}", e))?;
                    hasher.update(&chunk);
                    file.write_all(&chunk)
                        .await
                        .map_err(|e| format!("Write failed: {}", e))?;
                    downloaded = downloaded.saturating_add(chunk.len() as u64);

                    let should_emit = last_emit.elapsed() >= std::time::Duration::from_millis(250);
                    if should_emit {
                        last_emit = std::time::Instant::now();
                        send_progress(
                            WhisperModelDownloadStatus::Downloading,
                            downloaded,
                            total,
                            None,
                        );
                    }
                }

                file.flush()
                    .await
                    .map_err(|e| format!("Flush failed: {}", e))?;
                drop(file);

                send_progress(
                    WhisperModelDownloadStatus::Verifying,
                    downloaded,
                    total,
                    Some("Verifying SHA-256".to_string()),
                );

                let actual_sha = format!("{:x}", hasher.finalize());
                if actual_sha != expected_sha {
                    return Err(format!(
                        "SHA-256 mismatch (expected {}, got {})",
                        expected_sha, actual_sha
                    ));
                }

                // Move into place.
                if model_path.exists() {
                    let _ = tokio::fs::remove_file(&model_path).await;
                }

                tokio::fs::rename(&tmp_path, &model_path)
                    .await
                    .map_err(|e| format!("Failed to finalize model file: {}", e))?;

                send_progress(
                    WhisperModelDownloadStatus::Completed,
                    downloaded,
                    total,
                    Some("Download complete".to_string()),
                );

                Ok(())
            }
            .await;

            match result {
                Ok(()) => {}
                Err(e) if e == "cancelled" => {
                    let _ = tokio::fs::remove_file(&tmp_path).await;
                    send_progress(
                        WhisperModelDownloadStatus::Cancelled,
                        0,
                        None,
                        Some("Cancelled".to_string()),
                    );
                }
                Err(e) => {
                    let _ = tokio::fs::remove_file(&tmp_path).await;
                    send_progress(WhisperModelDownloadStatus::Error, 0, None, Some(e));
                }
            }

            let mut jobs = jobs_for_task.lock().await;
            jobs.remove(&model_id);
        });

        Ok(())
    }

    #[cfg(not(feature = "local-whisper"))]
    {
        let _ = (app, state, model_id);
        Err(WhisperCommandError::from(
            "Local Whisper feature is not enabled".to_string(),
        ))
    }
}

/// Cancel an in-flight model download.
#[tauri::command]
pub async fn cancel_whisper_model_download(
    state: tauri::State<'_, WhisperDownloadManager>,
    model_id: String,
) -> Result<(), WhisperCommandError> {
    let jobs = state.jobs.lock().await;
    if let Some(job) = jobs.get(&model_id) {
        job.cancel.cancel();
    }
    Ok(())
}

// Helper functions

fn get_models_dir(app: &tauri::AppHandle) -> Result<PathBuf, WhisperCommandError> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| WhisperCommandError::from(format!("Failed to get app data dir: {}", e)))?;

    let models_dir = app_data_dir.join("whisper-models");

    // Create directory if it doesn't exist
    if !models_dir.exists() {
        std::fs::create_dir_all(&models_dir).map_err(|e| {
            WhisperCommandError::from(format!("Failed to create models directory: {}", e))
        })?;
    }

    Ok(models_dir)
}

#[cfg(feature = "local-whisper")]
fn parse_model_id(model_id: &str) -> Result<WhisperModel, WhisperCommandError> {
    let model = match model_id.to_lowercase().as_str() {
        "tiny" => WhisperModel::Tiny,
        "tinyen" | "tiny_en" | "tiny-en" => WhisperModel::TinyEn,
        "base" => WhisperModel::Base,
        "baseen" | "base_en" | "base-en" => WhisperModel::BaseEn,
        "small" => WhisperModel::Small,
        "smallen" | "small_en" | "small-en" => WhisperModel::SmallEn,
        "medium" => WhisperModel::Medium,
        "mediumen" | "medium_en" | "medium-en" => WhisperModel::MediumEn,
        "largev1" | "large_v1" | "large-v1" => WhisperModel::LargeV1,
        "largev2" | "large_v2" | "large-v2" => WhisperModel::LargeV2,
        "largev3" | "large_v3" | "large-v3" => WhisperModel::LargeV3,
        "largev3turbo" | "large_v3_turbo" | "large-v3-turbo" => WhisperModel::LargeV3Turbo,
        _ => {
            return Err(WhisperCommandError::from(format!(
                "Unknown model: {}",
                model_id
            )));
        }
    };
    Ok(model)
}

#[cfg_attr(not(test), allow(dead_code))]
fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.0} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.0} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} bytes", bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(500), "500 bytes");
        assert_eq!(format_size(1024), "1 KB");
        assert_eq!(format_size(75_000_000), "72 MB"); // 75M / 1024 / 1024 ≈ 71.5 → rounds to 72
        assert_eq!(format_size(1_500_000_000), "1.4 GB");
    }

    #[test]
    fn test_is_local_whisper_available() {
        // This will be true or false depending on feature flag
        let _ = is_local_whisper_available();
    }
}
