//! Profile-query helpers used by command-facing recording flows.
//!
//! The pipeline still owns real request-time resolution through `resolve_request_profile_context`,
//! but command/orchestration code also needs a few lightweight lookups for UI chips, retry
//! bookkeeping, and logging. Keeping them here prevents those read-model helpers from drifting.

use super::profile_matcher::select_profile_for_program_path;
use super::PipelineConfig;

fn resolve_profile_for_foreground_path(
    cfg: &PipelineConfig,
    foreground_path: Option<&str>,
) -> (Option<String>, Option<String>) {
    // Distinguish between:
    // - Unknown foreground app (can't determine): return no profile -> no chip in UI.
    // - Known foreground app but no profile match: return explicit "default".
    let Some(foreground_path) = foreground_path else {
        return (None, None);
    };

    let profile = select_profile_for_program_path(&cfg.llm_config, foreground_path);
    if let Some(profile) = profile {
        return (Some(profile.id), Some(profile.name));
    }

    (Some("default".to_string()), Some("Default".to_string()))
}

pub(crate) fn resolve_profile_for_foreground_app(
    cfg: &PipelineConfig,
) -> (Option<String>, Option<String>) {
    #[cfg(desktop)]
    {
        let foreground = crate::windows_apps::get_foreground_process_path();
        resolve_profile_for_foreground_path(cfg, foreground.as_deref())
    }

    #[cfg(not(desktop))]
    {
        let _ = cfg;
        (None, None)
    }
}

pub(crate) fn resolve_profile_by_id(
    cfg: &PipelineConfig,
    profile_id: Option<&str>,
) -> (Option<String>, Option<String>) {
    let Some(profile_id) = profile_id else {
        return (Some("default".to_string()), Some("Default".to_string()));
    };

    if profile_id == "default" {
        return (Some("default".to_string()), Some("Default".to_string()));
    }

    let name = cfg
        .llm_config
        .program_prompt_profiles
        .iter()
        .find(|profile| profile.id == profile_id)
        .map(|profile| profile.name.clone());

    (Some(profile_id.to_string()), name)
}

pub(crate) fn program_basename_for_log(path: &str) -> String {
    let base = crate::app_shared::basename_for_log(path).trim();
    if base.is_empty() {
        "<unknown>".to_string()
    } else {
        base.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::{LlmConfig, ProgramPromptProfile, PromptSections};

    fn profile(id: &str, name: &str, program_path: &str) -> ProgramPromptProfile {
        ProgramPromptProfile {
            id: id.to_string(),
            name: name.to_string(),
            program_paths: vec![program_path.to_string()],
            prompts: PromptSections::default(),
            presets: Vec::new(),
            default_preset_id: None,
            default_preset_description: None,
            default_target_rewrite_llm_enabled: true,
            active_preset_id: None,
            router: None,
            rewrite_llm_enabled: None,
            stt_provider: None,
            stt_model: None,
            stt_language: None,
            stt_timeout_seconds: None,
            llm_provider: None,
            llm_model: None,
            openai_reasoning_effort: None,
            gemini_thinking_budget: None,
            gemini_thinking_level: None,
            anthropic_thinking_budget: None,
            quick_ask_provider: None,
            quick_ask_model: None,
            quick_ask_system_prompt: None,
            context_grab_method: None,
            rewrite_include_clipboard_context: None,
            quick_replace_include_clipboard_context: None,
            quick_ask_include_clipboard_context: None,
            rewrite_active_window_ocr_mode: None,
            quick_replace_active_window_ocr_mode: None,
            quick_ask_active_window_ocr_mode: None,
            quick_replace_enabled: None,
            quick_replace_provider: None,
            quick_replace_model: None,
            quick_replace_system_prompt: None,
            quick_ask_openai_reasoning_effort: None,
            quick_ask_gemini_thinking_budget: None,
            quick_ask_gemini_thinking_level: None,
            quick_ask_anthropic_thinking_budget: None,
        }
    }

    fn config_with_profiles(profiles: Vec<ProgramPromptProfile>) -> PipelineConfig {
        PipelineConfig {
            llm_config: LlmConfig {
                program_prompt_profiles: profiles,
                ..LlmConfig::default()
            },
            ..PipelineConfig::default()
        }
    }

    #[test]
    fn foreground_unknown_keeps_profile_chip_empty() {
        let config = config_with_profiles(Vec::new());

        assert_eq!(
            resolve_profile_for_foreground_path(&config, None),
            (None, None)
        );
    }

    #[test]
    fn unmatched_foreground_app_uses_explicit_default_profile() {
        let config = config_with_profiles(vec![profile(
            "vscode",
            "VS Code",
            "C:/Program Files/Microsoft VS Code/Code.exe",
        )]);

        assert_eq!(
            resolve_profile_for_foreground_path(&config, Some("C:/Windows/System32/notepad.exe")),
            (Some("default".to_string()), Some("Default".to_string()))
        );
    }

    #[test]
    fn matching_foreground_app_returns_profile_identity() {
        let config = config_with_profiles(vec![profile(
            "vscode",
            "VS Code",
            "C:/Program Files/Microsoft VS Code/Code.exe",
        )]);

        assert_eq!(
            resolve_profile_for_foreground_path(
                &config,
                Some("C:/Program Files/Microsoft VS Code/Code.exe"),
            ),
            (Some("vscode".to_string()), Some("VS Code".to_string()))
        );
    }

    #[test]
    fn resolve_profile_by_id_preserves_default_marker() {
        let config = config_with_profiles(vec![profile(
            "terminal",
            "Terminal",
            "C:/Windows/System32/WindowsTerminal.exe",
        )]);

        assert_eq!(
            resolve_profile_by_id(&config, None),
            (Some("default".to_string()), Some("Default".to_string()))
        );
        assert_eq!(
            resolve_profile_by_id(&config, Some("default")),
            (Some("default".to_string()), Some("Default".to_string()))
        );
        assert_eq!(
            resolve_profile_by_id(&config, Some("terminal")),
            (Some("terminal".to_string()), Some("Terminal".to_string()))
        );
    }

    #[test]
    fn basename_for_log_falls_back_when_empty() {
        assert_eq!(program_basename_for_log(""), "<unknown>");
    }
}
