//! Local Whisper STT provider using whisper.cpp bindings.
//!
//! This module provides offline speech-to-text using the whisper-rs library
//! (Rust bindings for whisper.cpp). It's an optional feature that requires
//! the `local-whisper` feature flag to be enabled.
//!
//! ## Requirements
//! - whisper.cpp model files (downloaded separately)
//! - Feature flag: `--features local-whisper`
//!
//! ## Model Sizes
//! - tiny: ~75MB, fastest, lower accuracy
//! - base: ~142MB, good balance
//! - small: ~466MB, better accuracy
//! - medium: ~1.5GB, high accuracy
//! - large: ~2.9GB, highest accuracy

use super::{AudioFormat, SttError, SttProvider};
use async_trait::async_trait;
use schemars::JsonSchema;
use std::path::PathBuf;
use std::sync::Arc;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

#[cfg(target_os = "windows")]
fn get_nvidia_smi_reported_cuda_version_windows() -> Option<f32> {
    use std::process::Command;

    // `nvidia-smi -q` includes a line like:
    // "CUDA Version                              : 12.9"
    let out = Command::new("nvidia-smi").args(["-q"]).output().ok()?;
    if !out.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&out.stdout);
    for line in stdout.lines() {
        let line = line.trim();
        if !line.to_ascii_lowercase().starts_with("cuda version") {
            continue;
        }

        let (_k, v) = line.split_once(':')?;
        let v = v.trim();
        // Parse as float (e.g., 12.9)
        if let Ok(parsed) = v.parse::<f32>() {
            return Some(parsed);
        }
    }

    None
}

#[cfg(not(target_os = "windows"))]
fn get_nvidia_smi_reported_cuda_version_windows() -> Option<f32> {
    None
}

#[derive(Debug, Clone, serde::Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LocalWhisperComputeBackend {
    Cpu,
    Cuda,
}

/// Diagnostic information about which compute backend Local Whisper will use.
///
/// This is primarily for UI/debugging so users can see whether Kolboo is using
/// GPU acceleration (CUDA) or falling back to CPU, and why.
#[derive(Debug, Clone, serde::Serialize, JsonSchema)]
pub struct LocalWhisperBackendStatus {
    pub build_has_local_whisper: bool,
    pub build_has_cuda: bool,
    pub compute: LocalWhisperComputeBackend,
    pub reason: Option<String>,
    pub missing_dlls: Vec<String>,
}

fn find_dll_windows(name: &str) -> Option<std::path::PathBuf> {
    use std::path::Path;

    if !cfg!(target_os = "windows") {
        return None;
    }

    let candidates: Vec<std::path::PathBuf> = (|| {
        let mut dirs: Vec<std::path::PathBuf> = Vec::new();

        // 1) exe directory
        if let Ok(exe) = std::env::current_exe() {
            if let Some(parent) = exe.parent() {
                dirs.push(parent.to_path_buf());
            }
        }

        // 2) System32 / SysWOW64
        if let Ok(windir) = std::env::var("WINDIR") {
            let w = std::path::PathBuf::from(windir);
            dirs.push(w.join("System32"));
            dirs.push(w.join("SysWOW64"));
        }

        // 3) PATH entries
        if let Some(path) = std::env::var_os("PATH") {
            for p in std::env::split_paths(&path) {
                dirs.push(p);
            }
        }

        dirs
    })();

    for dir in candidates {
        if dir.as_os_str().is_empty() {
            continue;
        }
        let candidate = Path::new(&dir).join(name);
        if candidate.exists() {
            return Some(candidate);
        }
    }

    None
}

#[cfg(target_os = "windows")]
fn find_cuda_toolkit_bin_candidates_windows() -> Vec<std::path::PathBuf> {
    // Common CUDA Toolkit install root on Windows.
    // Example: C:\Program Files\NVIDIA GPU Computing Toolkit\CUDA\v13.1\bin\x64
    let root = std::path::PathBuf::from(r"C:\Program Files\NVIDIA GPU Computing Toolkit\CUDA");
    let mut out: Vec<std::path::PathBuf> = Vec::new();

    if let Ok(entries) = std::fs::read_dir(&root) {
        let mut version_dirs: Vec<std::path::PathBuf> = entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .filter(|p| {
                p.file_name()
                    .and_then(|s| s.to_str())
                    .map(|s| s.starts_with('v'))
                    .unwrap_or(false)
            })
            .collect();

        // Prefer highest version directory (rough lexicographic works for vX.Y in practice).
        version_dirs.sort_by(|a, b| b.cmp(a));

        for vdir in version_dirs.into_iter().take(5) {
            let bin = vdir.join("bin").join("x64");
            if bin.exists() {
                out.push(bin);
            }
        }
    }

    out
}

/// Best-effort check for CUDA runtime availability.
///
/// whisper.cpp's CUDA backend on Windows typically requires both:
/// - NVIDIA driver (nvcuda.dll)
/// - CUDA runtime DLLs (cudart + cublas)
///
/// If these DLLs are missing, whisper.cpp may print messages like:
/// "ggml_cuda_init: failed to initialize CUDA" and fall back to CPU.
pub fn get_local_whisper_backend_status() -> LocalWhisperBackendStatus {
    let build_has_local_whisper = cfg!(feature = "local-whisper");
    let build_has_cuda = cfg!(feature = "local-whisper-cuda");

    if !build_has_local_whisper {
        return LocalWhisperBackendStatus {
            build_has_local_whisper,
            build_has_cuda,
            compute: LocalWhisperComputeBackend::Cpu,
            reason: Some("Local Whisper feature is not enabled in this build.".to_string()),
            missing_dlls: Vec::new(),
        };
    }

    if !build_has_cuda {
        return LocalWhisperBackendStatus {
            build_has_local_whisper,
            build_has_cuda,
            compute: LocalWhisperComputeBackend::Cpu,
            reason: Some(
                "This build does not include CUDA support; Local Whisper will run on CPU."
                    .to_string(),
            ),
            missing_dlls: Vec::new(),
        };
    }

    // CUDA build: preflight DLL presence. This is cheap and gives actionable UI feedback.
    // If any required DLL is missing, we *intentionally* do not request GPU so we avoid
    // confusing stderr spam and can provide a clear reason.
    let mut missing: Vec<String> = Vec::new();
    let nvcuda_ok = find_dll_windows("nvcuda.dll").is_some();
    if !nvcuda_ok {
        missing.push("nvcuda.dll".to_string());
    }

    // Common CUDA runtime / BLAS DLLs used by ggml/whisper.cpp on Windows.
    // Note: the CUDA build is typically tied to a specific major version (11/12/13)
    // for these DLL names. We still check multiple versions to provide a better
    // error message, but later we also guard against driver/runtime incompatibility.
    let cudart_13 = find_dll_windows("cudart64_13.dll").is_some();
    let cudart_12 = find_dll_windows("cudart64_12.dll").is_some();
    let cudart_11 = find_dll_windows("cudart64_11.dll").is_some();
    let cudart_ok = cudart_13 || cudart_12 || cudart_11;
    if !cudart_ok {
        missing.push("cudart64_13.dll (or cudart64_12.dll / cudart64_11.dll)".to_string());
    }

    let cublas_13 = find_dll_windows("cublas64_13.dll").is_some();
    let cublas_12 = find_dll_windows("cublas64_12.dll").is_some();
    let cublas_11 = find_dll_windows("cublas64_11.dll").is_some();
    let cublas_ok = cublas_13 || cublas_12 || cublas_11;
    if !cublas_ok {
        missing.push("cublas64_13.dll (or cublas64_12.dll / cublas64_11.dll)".to_string());
    }

    let cublaslt_13 = find_dll_windows("cublasLt64_13.dll").is_some();
    let cublaslt_12 = find_dll_windows("cublasLt64_12.dll").is_some();
    let cublaslt_11 = find_dll_windows("cublasLt64_11.dll").is_some();
    let cublaslt_ok = cublaslt_13 || cublaslt_12 || cublaslt_11;
    if !cublaslt_ok {
        missing.push("cublasLt64_13.dll (or cublasLt64_12.dll / cublasLt64_11.dll)".to_string());
    }

    if !missing.is_empty() {
        #[cfg(target_os = "windows")]
        {
            // If CUDA Toolkit is installed but its bin folder isn't on PATH, whisper.cpp won't
            // be able to LoadLibrary the required DLLs. Provide a more actionable hint.
            let bins = find_cuda_toolkit_bin_candidates_windows();
            for bin in bins {
                let has_any = bin.join("cudart64_13.dll").exists()
                    || bin.join("cudart64_12.dll").exists()
                    || bin.join("cudart64_11.dll").exists();
                if has_any {
                    return LocalWhisperBackendStatus {
                        build_has_local_whisper,
                        build_has_cuda,
                        compute: LocalWhisperComputeBackend::Cpu,
                        reason: Some(format!(
                            "CUDA Toolkit detected, but its DLL directory isn't on the DLL search path. Add this folder to PATH (or copy the DLLs next to the app): {}",
                            bin.display()
                        )),
                        missing_dlls: missing,
                    };
                }
            }
        }

        return LocalWhisperBackendStatus {
            build_has_local_whisper,
            build_has_cuda,
            compute: LocalWhisperComputeBackend::Cpu,
            reason: Some(
                "CUDA build detected, but the CUDA runtime libraries were not found. Local Whisper will run on CPU.".to_string(),
            ),
            missing_dlls: missing,
        };
    }

    // Extra guard: CUDA runtime DLLs can be present but still unusable if the NVIDIA driver
    // is too old for the CUDA runtime major version.
    // Example: driver reports CUDA 12.9, but PATH points to cudart64_13.dll.
    #[cfg(target_os = "windows")]
    {
        let runtime_major = if cudart_13 || cublas_13 || cublaslt_13 {
            Some(13u32)
        } else if cudart_12 || cublas_12 || cublaslt_12 {
            Some(12u32)
        } else if cudart_11 || cublas_11 || cublaslt_11 {
            Some(11u32)
        } else {
            None
        };

        if let (Some(runtime_major), Some(driver_cuda)) = (
            runtime_major,
            get_nvidia_smi_reported_cuda_version_windows(),
        ) {
            let driver_major = driver_cuda.floor() as u32;
            if driver_major < runtime_major {
                return LocalWhisperBackendStatus {
                    build_has_local_whisper,
                    build_has_cuda,
                    compute: LocalWhisperComputeBackend::Cpu,
                    reason: Some(format!(
                        "CUDA runtime {} detected, but NVIDIA driver reports CUDA {} support. This usually means the driver is too old for the runtime, so whisper.cpp will fall back to CPU. Upgrade your NVIDIA driver or use a CUDA {} build.",
                        runtime_major,
                        driver_cuda,
                        driver_major,
                    )),
                    missing_dlls: Vec::new(),
                };
            }
        }
    }

    LocalWhisperBackendStatus {
        build_has_local_whisper,
        build_has_cuda,
        compute: LocalWhisperComputeBackend::Cuda,
        reason: None,
        missing_dlls: Vec::new(),
    }
}

/// Available Whisper model sizes
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum WhisperModel {
    Tiny,
    TinyEn,
    Base,
    BaseEn,
    Small,
    SmallEn,
    Medium,
    MediumEn,
    LargeV1,
    LargeV2,
    LargeV3,
    LargeV3Turbo,
}

impl WhisperModel {
    /// Get the model filename
    pub fn filename(&self) -> &'static str {
        match self {
            Self::Tiny => "ggml-tiny.bin",
            Self::TinyEn => "ggml-tiny.en.bin",
            Self::Base => "ggml-base.bin",
            Self::BaseEn => "ggml-base.en.bin",
            Self::Small => "ggml-small.bin",
            Self::SmallEn => "ggml-small.en.bin",
            Self::Medium => "ggml-medium.bin",
            Self::MediumEn => "ggml-medium.en.bin",
            Self::LargeV1 => "ggml-large-v1.bin",
            Self::LargeV2 => "ggml-large-v2.bin",
            Self::LargeV3 => "ggml-large-v3.bin",
            Self::LargeV3Turbo => "ggml-large-v3-turbo.bin",
        }
    }

    /// Get the Hugging Face download URL
    pub fn download_url(&self) -> String {
        format!(
            "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/{}",
            self.filename()
        )
    }

    /// Expected SHA-256 for the model file (hex-encoded, lowercase).
    ///
    /// These are pinned to the Hugging Face LFS pointer metadata for
    /// `ggerganov/whisper.cpp` and are used to validate downloads.
    pub fn expected_sha256(&self) -> &'static str {
        match self {
            Self::Tiny => "be07e048e1e599ad46341c8d2a135645097a538221678b7acdd1b1919c6e1b21",
            Self::TinyEn => "921e4cf8686fdd993dcd081a5da5b6c365bfde1162e72b08d75ac75289920b1f",
            Self::Base => "60ed5bc3dd14eea856493d334349b405782ddcaf0028d4b5df4088345fba2efe",
            Self::BaseEn => "a03779c86df3323075f5e796cb2ce5029f00ec8869eee3fdfb897afe36c6d002",
            Self::Small => "1be3a9b2063867b937e64e2ec7483364a79917e157fa98c5d94b5c1fffea987b",
            Self::SmallEn => "c6138d6d58ecc8322097e0f987c32f1be8bb0a18532a3f88f734d1bbf9c41e5d",
            Self::Medium => "6c14d5adee5f86394037b4e4e8b59f1673b6cee10e3cf0b11bbdbee79c156208",
            Self::MediumEn => "cc37e93478338ec7700281a7ac30a10128929eb8f427dda2e865faa8f6da4356",
            Self::LargeV1 => "7d99f41a10525d0206bddadd86760181fa920438b6b33237e3118ff6c83bb53d",
            Self::LargeV2 => "9a423fe4d40c82774b6af34115b8b935f34152246eb19e80e376071d3f999487",
            Self::LargeV3 => "64d182b440b98d5203c4f9bd541544d84c605196c4f7b845dfa11fb23594d1e2",
            Self::LargeV3Turbo => {
                "1fc70f774d38eb169993ac391eea357ef47c88757ef72ee5943879b7e8e2bc69"
            }
        }
    }

    /// Get approximate model size in bytes
    pub fn size_bytes(&self) -> u64 {
        match self {
            Self::Tiny | Self::TinyEn => 75_000_000,
            Self::Base | Self::BaseEn => 142_000_000,
            Self::Small | Self::SmallEn => 466_000_000,
            Self::Medium | Self::MediumEn => 1_500_000_000,
            Self::LargeV1 | Self::LargeV2 | Self::LargeV3 => 2_900_000_000,
            Self::LargeV3Turbo => 1_600_000_000,
        }
    }

    /// Get human-readable model name
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Tiny => "Tiny",
            Self::TinyEn => "Tiny English",
            Self::Base => "Base",
            Self::BaseEn => "Base English",
            Self::Small => "Small",
            Self::SmallEn => "Small English",
            Self::Medium => "Medium",
            Self::MediumEn => "Medium English",
            Self::LargeV1 => "Large v1",
            Self::LargeV2 => "Large v2",
            Self::LargeV3 => "Large v3",
            Self::LargeV3Turbo => "Large v3 Turbo",
        }
    }

    /// Check if this is an English-only model
    pub fn is_english_only(&self) -> bool {
        matches!(
            self,
            Self::TinyEn | Self::BaseEn | Self::SmallEn | Self::MediumEn
        )
    }

    /// List all available models
    pub fn all() -> Vec<Self> {
        vec![
            Self::Tiny,
            Self::TinyEn,
            Self::Base,
            Self::BaseEn,
            Self::Small,
            Self::SmallEn,
            Self::Medium,
            Self::MediumEn,
            Self::LargeV1,
            Self::LargeV2,
            Self::LargeV3,
            Self::LargeV3Turbo,
        ]
    }
}

impl Default for WhisperModel {
    fn default() -> Self {
        Self::Base // Good balance of speed and accuracy
    }
}

/// Configuration for the local Whisper provider
#[derive(Debug, Clone)]
pub struct LocalWhisperConfig {
    /// Path to the model file
    pub model_path: PathBuf,
    /// Language to use (None for auto-detect)
    pub language: Option<String>,
    /// Whether to translate to English
    pub translate: bool,
    /// Number of threads to use (0 = auto)
    pub n_threads: u32,

    /// Optional transcription prompt/context (biases decoding).
    ///
    /// This maps to Whisper's "initial prompt" capability and is useful for
    /// domain-specific vocabulary (project names, acronyms, etc.).
    pub transcription_prompt: Option<String>,
}

impl Default for LocalWhisperConfig {
    fn default() -> Self {
        Self {
            model_path: PathBuf::new(),
            language: Some("en".to_string()),
            translate: false,
            n_threads: 0, // Auto-detect
            transcription_prompt: None,
        }
    }
}

/// Local Whisper STT provider
pub struct LocalWhisperProvider {
    ctx: Arc<WhisperContext>,
    config: LocalWhisperConfig,
}

impl LocalWhisperProvider {
    /// Create a new provider with the given model path
    pub fn new(model_path: PathBuf) -> Result<Self, SttError> {
        Self::with_config(LocalWhisperConfig {
            model_path,
            ..Default::default()
        })
    }

    /// Create a new provider with custom configuration
    pub fn with_config(config: LocalWhisperConfig) -> Result<Self, SttError> {
        if !config.model_path.exists() {
            return Err(SttError::Audio(format!(
                "Model file not found: {}",
                config.model_path.display()
            )));
        }

        let mut ctx_params = WhisperContextParameters::default();

        // Decide whether to request GPU based on runtime availability.
        // This prevents confusing "no GPU found" logs when the CUDA runtime DLLs
        // aren't installed/bundled (common on Windows).
        #[cfg(feature = "local-whisper-cuda")]
        {
            let status = get_local_whisper_backend_status();
            ctx_params.use_gpu(matches!(status.compute, LocalWhisperComputeBackend::Cuda));
        }

        let ctx = WhisperContext::new_with_params(
            config
                .model_path
                .to_str()
                .ok_or_else(|| SttError::Audio("Invalid model path encoding".to_string()))?,
            ctx_params,
        )
        .map_err(|e| SttError::Audio(format!("Failed to load Whisper model: {}", e)))?;

        Ok(Self {
            ctx: Arc::new(ctx),
            config,
        })
    }

    /// Check if a model file exists at the given path
    pub fn model_exists(model_path: &PathBuf) -> bool {
        model_path.exists() && model_path.is_file()
    }

    /// Get the default models directory
    pub fn default_models_dir() -> Option<PathBuf> {
        dirs::data_local_dir().map(|d| d.join("kolboo").join("models"))
    }
}

#[async_trait]
impl SttProvider for LocalWhisperProvider {
    async fn transcribe(&self, audio: &[u8], _format: &AudioFormat) -> Result<String, SttError> {
        // Decode WAV to f32 samples
        let samples = decode_wav_to_f32_mono_16khz(audio)?;

        if samples.is_empty() {
            return Ok(String::new());
        }

        // Clone what we need for the blocking task
        let ctx = self.ctx.clone();
        let language = self.config.language.clone();
        let translate = self.config.translate;
        let n_threads = self.config.n_threads;
        let transcription_prompt = self.config.transcription_prompt.clone().and_then(|p| {
            let trimmed = p.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        });

        // whisper-rs is synchronous, so we use spawn_blocking
        let result = tokio::task::spawn_blocking(move || {
            let mut state = ctx
                .create_state()
                .map_err(|e| SttError::Audio(format!("Failed to create Whisper state: {}", e)))?;

            let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });

            // Set language
            if let Some(lang) = &language {
                params.set_language(Some(lang));
            }

            // Set translate mode
            params.set_translate(translate);

            // Optional initial prompt / context
            if let Some(prompt) = transcription_prompt.as_deref() {
                // whisper-rs exposes this as "initial prompt".
                // If the upstream API changes, this will fail to compile and we can adjust.
                params.set_initial_prompt(prompt);
            }

            // Set thread count
            if n_threads > 0 {
                params.set_n_threads(n_threads as i32);
            }

            // Disable printing to reduce noise
            params.set_print_special(false);
            params.set_print_progress(false);
            params.set_print_realtime(false);
            params.set_print_timestamps(false);

            // Run inference
            state
                .full(params, &samples)
                .map_err(|e| SttError::Audio(format!("Whisper inference failed: {}", e)))?;

            // Collect results
            let num_segments = state.full_n_segments();

            if num_segments < 0 {
                return Err(SttError::Audio(format!(
                    "Whisper returned negative segment count: {}",
                    num_segments
                )));
            }

            let mut text = String::new();
            for i in 0..num_segments {
                let Some(segment) = state.get_segment(i) else {
                    continue;
                };

                if let Ok(segment_text) = segment.to_str() {
                    text.push_str(segment_text);
                }
            }

            Ok::<String, SttError>(text.trim().to_string())
        })
        .await
        .map_err(|e| SttError::Audio(format!("Task join error: {}", e)))??;

        Ok(result)
    }

    fn name(&self) -> &'static str {
        "local-whisper"
    }
}

/// Decode WAV audio to f32 samples, converting to mono 16kHz if needed
fn decode_wav_to_f32_mono_16khz(wav_bytes: &[u8]) -> Result<Vec<f32>, SttError> {
    use std::io::Cursor;

    let cursor = Cursor::new(wav_bytes);
    let mut reader = hound::WavReader::new(cursor)
        .map_err(|e| SttError::Audio(format!("Failed to read WAV: {}", e)))?;

    let spec = reader.spec();
    let sample_rate = spec.sample_rate;
    let channels = spec.channels as usize;

    // Read samples based on format
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader.samples::<f32>().map(|s| s.unwrap_or(0.0)).collect(),
        hound::SampleFormat::Int => {
            let bits = spec.bits_per_sample;
            let max_val = (1 << (bits - 1)) as f32;
            reader
                .samples::<i32>()
                .map(|s| s.unwrap_or(0) as f32 / max_val)
                .collect()
        }
    };

    // Convert to mono if stereo
    let mono_samples: Vec<f32> = if channels > 1 {
        samples
            .chunks(channels)
            .map(|chunk| chunk.iter().sum::<f32>() / channels as f32)
            .collect()
    } else {
        samples
    };

    // Resample to 16kHz if needed
    if sample_rate != 16000 {
        Ok(crate::vad::resample_to_16khz(&mono_samples, sample_rate))
    } else {
        Ok(mono_samples)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_filenames() {
        assert_eq!(WhisperModel::Tiny.filename(), "ggml-tiny.bin");
        assert_eq!(WhisperModel::BaseEn.filename(), "ggml-base.en.bin");
        assert_eq!(WhisperModel::LargeV3.filename(), "ggml-large-v3.bin");
    }

    #[test]
    fn test_model_urls() {
        let url = WhisperModel::Base.download_url();
        assert!(url.contains("huggingface.co"));
        assert!(url.contains("ggml-base.bin"));
    }

    #[test]
    fn test_english_only_models() {
        assert!(WhisperModel::TinyEn.is_english_only());
        assert!(WhisperModel::BaseEn.is_english_only());
        assert!(!WhisperModel::Tiny.is_english_only());
        assert!(!WhisperModel::LargeV3.is_english_only());
    }

    #[test]
    fn test_all_models() {
        let models = WhisperModel::all();
        assert!(models.len() >= 10);
    }
}
