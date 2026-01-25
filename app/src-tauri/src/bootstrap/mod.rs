use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager,
};

use crate::commands;
use crate::events;
use crate::history::HistoryStorage;
use crate::llm;
use crate::pipeline;
use crate::request_log::{RequestLogStore, RequestLogsRetentionConfig, RequestLogsRetentionMode};
use crate::settings;
use crate::state::TrayKeepAlive;
use crate::stt;
use crate::{get_setting_from_store, stats};

/// Setup system tray
pub(crate) fn setup_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    fn show_main_window(app: &AppHandle, source: &str) {
        let Some(window) = app.get_webview_window("main") else {
            log::error!("{source}: main window not found (was it closed?) - recreating");

            // Recreate main window if it was previously closed/destroyed.
            // NOTE: Creating windows from synchronous event handlers can deadlock on Windows,
            // so we do it on a separate thread.
            let app_handle = app.clone();
            let source = source.to_string();
            std::thread::spawn(move || {
                log::info!("{source}: creating main window");
                match tauri::WebviewWindowBuilder::new(
                    &app_handle,
                    "main",
                    tauri::WebviewUrl::App("index.html".into()),
                )
                .title("Kolboo")
                .inner_size(1280.0, 720.0)
                .resizable(true)
                .center()
                .build()
                {
                    Ok(w) => {
                        let _ = w.unminimize();
                        let _ = w.show();
                        let _ = w.set_focus();
                        log::info!("{source}: main window created and shown");
                    }
                    Err(e) => {
                        log::error!("{source}: failed to create main window: {e}");
                    }
                }
            });
            return;
        };

        let visible_before = window.is_visible().ok();
        log::info!("{source}: attempting to show main window (visible_before={visible_before:?})");

        if let Err(e) = window.unminimize() {
            log::warn!("{source}: window.unminimize() failed: {e}");
        }
        if let Err(e) = window.show() {
            log::warn!("{source}: window.show() failed: {e}");
        }
        // If the window was previously on a disconnected monitor, showing/focusing may succeed
        // but the window can still be effectively invisible. Centering is a good recovery.
        if let Err(e) = window.center() {
            log::warn!("{source}: window.center() failed: {e}");
        }
        // A brief always-on-top toggle can help bring the window above other windows on Windows.
        let _ = window.set_always_on_top(true);
        if let Err(e) = window.set_focus() {
            log::warn!("{source}: window.set_focus() failed: {e}");
        }
        let _ = window.set_always_on_top(false);

        let visible_after = window.is_visible().ok();
        log::info!("{source}: done (visible_after={visible_after:?})");
    }

    let show_item = MenuItem::with_id(app, "show", "Show Window", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show_item, &quit_item])?;

    // Use the same tray icon everywhere (full-color, brand-consistent).
    // NOTE: Some platforms (notably macOS) have UI conventions around template icons,
    // but we intentionally keep it consistent with the rest of the app branding.
    let icon_bytes = include_bytes!("../../icons/32x32.png");
    let icon = tauri::image::Image::from_bytes(icon_bytes)?;

    let tray = TrayIconBuilder::new()
        .icon(icon)
        .icon_as_template(false)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => {
                log::info!("Tray menu: show");
                show_main_window(app, "tray-menu-show");
            }
            "quit" => {
                log::info!("Tray menu: quit");
                // Emit disconnect request to frontend before exiting
                if let Some(window) = app.get_webview_window("overlay") {
                    let _ = window.emit(events::EVENT_REQUEST_DISCONNECT, ());
                }
                // Give frontend time to disconnect gracefully
                std::thread::sleep(std::time::Duration::from_millis(500));
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            match event {
                // On Windows, a double click triggers two `Click` events plus one `DoubleClick`.
                // If we "toggle" visibility on click, the two clicks cancel each other out and it
                // looks like double click does nothing.
                TrayIconEvent::DoubleClick {
                    button: MouseButton::Left,
                    ..
                }
                | TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } => {
                    log::info!("Tray icon: activate (left click/double-click)");
                    let app = tray.app_handle();
                    show_main_window(app, "tray-icon-activate");
                }
                _ => {}
            }
        })
        .build(app)?;

    // Keep the tray handle alive for the lifetime of the app.
    // This helps ensure click + menu callbacks keep firing reliably.
    #[cfg(desktop)]
    {
        let keepalive = app.state::<TrayKeepAlive>();
        keepalive.set(tray);
    }

    Ok(())
}

/// Initialize the recording pipeline from settings in the store
#[cfg(desktop)]
pub(crate) fn initialize_pipeline_from_settings(app: &AppHandle) -> pipeline::SharedPipeline {
    use std::collections::HashMap;
    use std::time::Duration;

    // Read STT settings from store
    let stt_provider: String = get_setting_from_store(app, "stt_provider", "groq".to_string());

    // Read STT model from store
    let stt_model: Option<String> = get_setting_from_store(app, "stt_model", None);

    // Read global STT transcription prompt from store
    let stt_transcription_prompt: Option<String> =
        get_setting_from_store(app, "stt_transcription_prompt", None);

    // Read STT timeout from store (seconds)
    let stt_timeout_seconds_raw: f64 = get_setting_from_store(app, "stt_timeout_seconds", 10.0);
    let stt_timeout_seconds: f64 =
        if stt_timeout_seconds_raw.is_finite() && stt_timeout_seconds_raw > 0.0 {
            stt_timeout_seconds_raw
        } else {
            log::warn!(
                "Invalid stt_timeout_seconds value in store ({}); falling back to 10s",
                stt_timeout_seconds_raw
            );
            10.0
        };

    // Read all available STT API keys (for per-profile provider overrides at runtime)
    let mut stt_api_keys: HashMap<String, String> = HashMap::new();
    for provider in [
        "openai",
        "fireworks",
        "aquavoice",
        "groq",
        "elevenlabs",
        "assemblyai",
        "speechmatics",
        "deepgram",
    ] {
        let key_name = format!("{}_api_key", provider);
        let key: String = crate::secrets::get_api_key(app, &key_name).unwrap_or_default();
        if !key.is_empty() {
            stt_api_keys.insert(provider.to_string(), key);
        }
    }

    // Get the appropriate API key based on provider
    let stt_api_key: String = match stt_provider.as_str() {
        "openai" => crate::secrets::get_api_key(app, "openai_api_key").unwrap_or_default(),
        "fireworks" => crate::secrets::get_api_key(app, "fireworks_api_key").unwrap_or_default(),
        "aquavoice" => crate::secrets::get_api_key(app, "aquavoice_api_key").unwrap_or_default(),
        "groq" => crate::secrets::get_api_key(app, "groq_api_key").unwrap_or_default(),
        "elevenlabs" => crate::secrets::get_api_key(app, "elevenlabs_api_key").unwrap_or_default(),
        "assemblyai" => crate::secrets::get_api_key(app, "assemblyai_api_key").unwrap_or_default(),
        "speechmatics" => {
            crate::secrets::get_api_key(app, "speechmatics_api_key").unwrap_or_default()
        }
        "deepgram" => crate::secrets::get_api_key(app, "deepgram_api_key").unwrap_or_default(),
        _ => String::new(),
    };

    // Read VAD settings from store
    let vad_settings: settings::VadSettings =
        get_setting_from_store(app, "vad_settings", settings::VadSettings::default());

    // Read quiet-audio gate settings from store
    let default_pipeline_config = pipeline::PipelineConfig::default();

    let sanitize_quiet_duration_secs = |v: f32, fallback: f32| -> f32 {
        if !v.is_finite() {
            return fallback;
        }
        // UI clamps to a small range; keep this defensive in case the store was edited.
        if v < 0.0 {
            return fallback;
        }
        v.min(30.0)
    };

    let sanitize_quiet_dbfs_threshold = |v: f32, fallback: f32| -> f32 {
        if !v.is_finite() {
            return fallback;
        }
        // dBFS thresholds should be negative. If the store contains 0 or a positive number,
        // the quiet gate would (almost) always trigger.
        if v > -1.0 {
            return fallback;
        }
        v.clamp(-120.0, -1.0)
    };
    let quiet_audio_gate_enabled: bool = get_setting_from_store(
        app,
        "quiet_audio_gate_enabled",
        default_pipeline_config.quiet_audio_gate_enabled,
    );
    let quiet_audio_min_duration_secs: f32 = get_setting_from_store(
        app,
        "quiet_audio_min_duration_secs",
        default_pipeline_config.quiet_audio_min_duration_secs,
    );
    let quiet_audio_rms_dbfs_threshold: f32 = get_setting_from_store(
        app,
        "quiet_audio_rms_dbfs_threshold",
        default_pipeline_config.quiet_audio_rms_dbfs_threshold,
    );
    let quiet_audio_peak_dbfs_threshold: f32 = get_setting_from_store(
        app,
        "quiet_audio_peak_dbfs_threshold",
        default_pipeline_config.quiet_audio_peak_dbfs_threshold,
    );

    let quiet_audio_min_duration_secs = sanitize_quiet_duration_secs(
        quiet_audio_min_duration_secs,
        default_pipeline_config.quiet_audio_min_duration_secs,
    );
    let quiet_audio_rms_dbfs_threshold = sanitize_quiet_dbfs_threshold(
        quiet_audio_rms_dbfs_threshold,
        default_pipeline_config.quiet_audio_rms_dbfs_threshold,
    );
    let quiet_audio_peak_dbfs_threshold = sanitize_quiet_dbfs_threshold(
        quiet_audio_peak_dbfs_threshold,
        default_pipeline_config.quiet_audio_peak_dbfs_threshold,
    );

    // Read experimental noise gate settings from store.
    // New key is `noise_gate_threshold_dbfs` (Option<f32>), with legacy fallback to
    // `noise_gate_strength` (0..=100 mapped to -75..-30 dBFS).
    let sanitize_noise_gate_threshold_dbfs = |v: f32| -> Option<f32> {
        if !v.is_finite() {
            return None;
        }
        Some(v.clamp(-75.0, -30.0))
    };

    let noise_gate_threshold_dbfs: Option<f32> = {
        let raw: Option<f32> = get_setting_from_store(app, "noise_gate_threshold_dbfs", None);
        if let Some(v) = raw.and_then(sanitize_noise_gate_threshold_dbfs) {
            Some(v)
        } else {
            // Legacy fallback
            let strength_raw: u64 = get_setting_from_store(app, "noise_gate_strength", 0u64);
            let strength = (strength_raw.min(100) as u8) as f32;
            if strength <= 0.0 {
                None
            } else {
                let t = strength / 100.0;
                Some((-75.0 + (-30.0 + 75.0) * t).clamp(-75.0, -30.0))
            }
        }
    };

    // Read voice-pickup preprocessing toggles from store.
    let audio_downmix_to_mono: bool = get_setting_from_store(
        app,
        "audio_downmix_to_mono",
        default_pipeline_config.audio_downmix_to_mono,
    );
    let audio_resample_to_16khz: bool = get_setting_from_store(
        app,
        "audio_resample_to_16khz",
        default_pipeline_config.audio_resample_to_16khz,
    );
    let audio_highpass_enabled: bool = get_setting_from_store(
        app,
        "audio_highpass_enabled",
        default_pipeline_config.audio_highpass_enabled,
    );
    let audio_agc_enabled: bool = get_setting_from_store(
        app,
        "audio_agc_enabled",
        default_pipeline_config.audio_agc_enabled,
    );
    let audio_noise_suppression_enabled: bool = get_setting_from_store(
        app,
        "audio_noise_suppression_enabled",
        default_pipeline_config.audio_noise_suppression_enabled,
    );

    let quiet_audio_require_speech: bool = get_setting_from_store(
        app,
        "quiet_audio_require_speech",
        default_pipeline_config.quiet_audio_require_speech,
    );

    // Read LLM settings from store
    let rewrite_llm_enabled: bool = get_setting_from_store(app, "rewrite_llm_enabled", false);
    let llm_provider_setting: Option<String> = get_setting_from_store(app, "llm_provider", None);
    let llm_model_setting: Option<String> = get_setting_from_store(app, "llm_model", None);

    // Optional provider-specific reasoning/thinking knobs.
    // These are ignored unless the selected provider/model supports them.
    let openai_reasoning_effort: Option<String> =
        get_setting_from_store(app, "openai_reasoning_effort", None);
    let gemini_thinking_budget: Option<i64> =
        get_setting_from_store(app, "gemini_thinking_budget", None);
    let gemini_thinking_level: Option<String> =
        get_setting_from_store(app, "gemini_thinking_level", None);
    let anthropic_thinking_budget: Option<i64> =
        get_setting_from_store(app, "anthropic_thinking_budget", None);

    // If the user never explicitly selected a model, treat "default" as the provider's
    // concrete default model so request logs can display the exact model used.
    let llm_provider_effective = llm_provider_setting
        .clone()
        .unwrap_or_else(|| "openai".to_string());
    let llm_model_effective: Option<String> = llm_model_setting.or_else(|| {
        if rewrite_llm_enabled {
            llm::default_llm_model_for_provider(llm_provider_effective.as_str())
                .map(|m| m.to_string())
        } else {
            None
        }
    });

    let llm_api_key: String = llm_provider_setting
        .as_deref()
        .map(|provider| {
            let key_name = format!("{}_api_key", provider);
            crate::secrets::get_api_key(app, &key_name).unwrap_or_default()
        })
        .unwrap_or_default();

    // IMPORTANT: `enabled` is only the global toggle. The effective provider/key is resolved
    // per transcription based on the active profile.
    let llm_enabled = rewrite_llm_enabled;

    // Read all available LLM API keys (for per-profile provider overrides at runtime)
    let mut llm_api_keys: HashMap<String, String> = HashMap::new();
    for provider in [
        "openai",
        "fireworks",
        "anthropic",
        "groq",
        "gemini",
        "cohere",
        "cerebras",
    ] {
        let key_name = format!("{}_api_key", provider);
        let key: String = crate::secrets::get_api_key(app, &key_name).unwrap_or_default();
        if !key.is_empty() {
            llm_api_keys.insert(provider.to_string(), key);
        }
    }

    // Read rewrite prompt sections + per-program profiles from store
    //
    // `cleanup_prompt_sections` is treated as overrides on top of the built-in defaults.
    // Each program profile can further override individual sections.
    let cleanup_prompt_sections: Option<settings::CleanupPromptSectionsSetting> =
        get_setting_from_store(app, "cleanup_prompt_sections", None);
    let base_prompts: llm::PromptSections = cleanup_prompt_sections
        .as_ref()
        .map(|o| o.apply_to(&llm::PromptSections::default()))
        .unwrap_or_default();

    let rewrite_program_prompt_profiles: Vec<settings::RewriteProgramPromptProfile> =
        get_setting_from_store(app, "rewrite_program_prompt_profiles", Vec::new());
    let rewrite_program_prompt_profiles =
        settings::filter_enabled_rewrite_profiles(rewrite_program_prompt_profiles);

    let program_prompt_profiles: Vec<llm::ProgramPromptProfile> = rewrite_program_prompt_profiles
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

                    llm::ProgramPreset {
                        id: preset.id,
                        name: preset.name,
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

            llm::ProgramPromptProfile {
                id: p.id,
                name: p.name,
                program_paths: p.program_paths,
                prompts: profile_prompts,

                presets,
                default_preset_id: p.default_preset_id,
                default_preset_description: p.default_preset_description,
                default_target_rewrite_llm_enabled: p.default_target_rewrite_llm_enabled,
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

                quick_ask_provider: p.quick_ask_provider,
                quick_ask_model: p.quick_ask_model,
                quick_ask_system_prompt: p.quick_ask_system_prompt,
                context_grab_method: p.context_grab_method,

                quick_replace_enabled: p.quick_replace_enabled,
                quick_replace_provider: p.quick_replace_provider,
                quick_replace_model: p.quick_replace_model,
                quick_replace_system_prompt: p.quick_replace_system_prompt,
                quick_ask_openai_reasoning_effort: p.quick_ask_openai_reasoning_effort,
                quick_ask_gemini_thinking_budget: p.quick_ask_gemini_thinking_budget,
                quick_ask_gemini_thinking_level: p.quick_ask_gemini_thinking_level,
                quick_ask_anthropic_thinking_budget: p.quick_ask_anthropic_thinking_budget,

                rewrite_include_clipboard_context: p.rewrite_include_clipboard_context,
                quick_replace_include_clipboard_context: p.quick_replace_include_clipboard_context,
                quick_ask_include_clipboard_context: p.quick_ask_include_clipboard_context,
            }
        })
        .collect();

    // Microphone selection (backend / CPAL).
    // Historical key name is `selected_mic_id` (originally from browser deviceId).
    // Newer builds store a backend-generated selection token (unique per enumerated device).
    // Older builds stored a CPAL device *name*.
    // The audio capture layer accepts both.
    let input_device_name: Option<String> = {
        let raw: Option<String> = get_setting_from_store(app, "selected_mic_id", None);
        raw.and_then(|s| {
            let t = s.trim().to_string();
            if t.is_empty() || t == "default" {
                None
            } else {
                Some(t)
            }
        })
    };

    // Microphone capture behavior.
    // - hot_mic_enabled: keep the input stream open while idle and maintain a rolling pre-roll
    // - hot_mic_pre_roll_ms: pre-roll duration (ms) to prepend at record start
    // - mic_auto_recover_enabled: watchdog the stream and attempt restart on hangs/disconnects
    let hot_mic_enabled: bool = get_setting_from_store(app, "hot_mic_enabled", false);
    let hot_mic_pre_roll_ms: u32 =
        get_setting_from_store(app, "hot_mic_pre_roll_ms", 1500u32).min(5000);
    let mic_auto_recover_enabled: bool =
        get_setting_from_store(app, "mic_auto_recover_enabled", false);

    let proxy_settings: settings::ProxySettings =
        get_setting_from_store(app, "proxy_settings", settings::ProxySettings::default());

    let whisper_server_base_url: Option<String> = {
        let raw: Option<String> = get_setting_from_store(app, "whisper_server_base_url", None);
        crate::app_shared::normalize_optional_string(raw)
    };

    let ollama_url: Option<String> = {
        let raw: Option<String> = get_setting_from_store(app, "ollama_url", None);
        crate::app_shared::normalize_optional_base_url(raw)
    };

    #[cfg(feature = "local-whisper")]
    let whisper_model_path: Option<std::path::PathBuf> = {
        use crate::stt::WhisperModel;

        let model_id: String =
            get_setting_from_store(app, "local_whisper_model_id", "base".to_string());
        let model = match model_id.trim().to_lowercase().as_str() {
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
            _ => WhisperModel::Base,
        };

        app.path().app_data_dir().ok().map(|app_data_dir| {
            let models_dir = app_data_dir.join("whisper-models");
            // Best-effort; if it fails we'll still return a path and LocalWhisperProvider
            // will error clearly if the model doesn't exist.
            let _ = std::fs::create_dir_all(&models_dir);
            models_dir.join(model.filename())
        })
    };

    let local_whisper_load_mode: String =
        get_setting_from_store(app, "local_whisper_load_mode", "manual".to_string());

    let config = pipeline::PipelineConfig {
        input_device_name,

        hot_mic_enabled,
        hot_mic_pre_roll_ms,
        mic_auto_recover_enabled,

        stt_provider,
        stt_api_key,
        stt_api_keys,
        stt_model,
        stt_transcription_prompt,
        whisper_server_base_url,
        max_duration_secs: 300.0,
        retry_config: stt::RetryConfig::default(),
        vad_config: vad_settings.to_vad_auto_stop_config(),
        transcription_timeout: Duration::from_secs_f64(stt_timeout_seconds),
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

        llm_config: llm::LlmConfig {
            enabled: llm_enabled,
            provider: llm_provider_effective,
            api_key: llm_api_key,
            model: llm_model_effective,
            ollama_url,
            openai_reasoning_effort,
            gemini_thinking_budget,
            gemini_thinking_level,
            anthropic_thinking_budget,
            prompts: base_prompts,
            program_prompt_profiles,
            ..Default::default()
        },
        llm_api_keys,

        // Allow providers to enrich the active RequestLog with request/response payloads.
        request_log_store: app
            .try_state::<RequestLogStore>()
            .map(|s| s.inner().clone()),

        #[cfg(feature = "local-whisper")]
        whisper_model_path,

        local_whisper_load_mode,
    };

    log::info!(
        "Initializing pipeline with STT provider: {}, VAD enabled: {}",
        config.stt_provider,
        config.vad_config.enabled
    );

    let pipeline = pipeline::SharedPipeline::new(config);
    pipeline.set_app_handle(app.clone());
    pipeline
}

#[cfg(desktop)]
pub(crate) fn initialize_request_log_store(app: &AppHandle) {
    use chrono::Duration as ChronoDuration;

    let mode: String =
        get_setting_from_store(app, "request_logs_retention_mode", "amount".to_string());
    let amount: u64 = get_setting_from_store(app, "request_logs_retention_amount", 50u64);
    let days: u64 = get_setting_from_store(app, "request_logs_retention_days", 7u64);

    let mode = if mode == "time" {
        RequestLogsRetentionMode::Time
    } else {
        RequestLogsRetentionMode::Amount
    };

    let retention = RequestLogsRetentionConfig {
        mode,
        amount: amount.clamp(1, 200) as usize,
        time_retention: if days == 0 {
            None
        } else {
            Some(ChronoDuration::days(days as i64))
        },
    };

    let request_log_store = RequestLogStore::new_with_retention(retention);
    app.manage(request_log_store);
}

#[cfg(not(desktop))]
pub(crate) fn initialize_request_log_store(app: &AppHandle) {
    let request_log_store = RequestLogStore::new();
    app.manage(request_log_store);
}

#[cfg(desktop)]
pub(crate) fn apply_startup_retention(app: &AppHandle) {
    // Apply stats retention immediately on startup.
    // This keeps disk usage bounded even if the app is updated after long gaps.
    {
        let cfg = stats::read_stats_retention_config(app);
        if let Some(store) = app.try_state::<stats::StatsStore>() {
            let _ = store.prune(cfg);
        }
    }

    // Apply the configured history retention policy immediately so existing installs
    // don't keep more entries than the UI/backend intend.
    if let Some(history) = app.try_state::<HistoryStorage>() {
        // Safety net: if the app was closed/crashed mid-transcription, the
        // placeholder history rows would otherwise remain stuck as "in_progress".
        let _ =
            history.finalize_all_in_progress_as_error("Interrupted (app restarted)".to_string());

        let max_entries = commands::history::get_history_max_entries(app);
        let _ = history.trim_to_configured(max_entries);
    }
}
