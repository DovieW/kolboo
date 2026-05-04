use crate::llm::{LlmConfig, ProgramPreset, ProgramPromptProfile};
use crate::settings::IntentRouterStrategy;

fn normalize_active_window_ocr_mode(mode: Option<&str>) -> Option<&str> {
    match mode {
        Some("off") | Some("auto") | Some("manual") => mode,
        _ => None,
    }
}

pub(crate) fn resolve_rewrite_active_window_ocr_mode(
    active_profile: Option<&ProgramPromptProfile>,
    default_profile: Option<&ProgramPromptProfile>,
    global_fallback: &str,
) -> String {
    let global = normalize_active_window_ocr_mode(Some(global_fallback)).unwrap_or("off");
    normalize_active_window_ocr_mode(
        active_profile.and_then(|p| p.rewrite_active_window_ocr_mode.as_deref()),
    )
    .or_else(|| {
        normalize_active_window_ocr_mode(
            default_profile.and_then(|p| p.rewrite_active_window_ocr_mode.as_deref()),
        )
    })
    .unwrap_or(global)
    .to_string()
}

pub(crate) fn resolve_quick_replace_active_window_ocr_mode(
    active_profile: Option<&ProgramPromptProfile>,
    default_profile: Option<&ProgramPromptProfile>,
    global_fallback: &str,
) -> String {
    let global = normalize_active_window_ocr_mode(Some(global_fallback)).unwrap_or("off");
    normalize_active_window_ocr_mode(
        active_profile.and_then(|p| p.quick_replace_active_window_ocr_mode.as_deref()),
    )
    .or_else(|| {
        normalize_active_window_ocr_mode(
            default_profile.and_then(|p| p.quick_replace_active_window_ocr_mode.as_deref()),
        )
    })
    .unwrap_or(global)
    .to_string()
}

pub(crate) fn resolve_quick_ask_active_window_ocr_mode(
    active_profile: Option<&ProgramPromptProfile>,
    default_profile: Option<&ProgramPromptProfile>,
    global_fallback: &str,
) -> String {
    let global = normalize_active_window_ocr_mode(Some(global_fallback)).unwrap_or("off");
    normalize_active_window_ocr_mode(
        active_profile.and_then(|p| p.quick_ask_active_window_ocr_mode.as_deref()),
    )
    .or_else(|| {
        normalize_active_window_ocr_mode(
            default_profile.and_then(|p| p.quick_ask_active_window_ocr_mode.as_deref()),
        )
    })
    .unwrap_or(global)
    .to_string()
}

/// Decide whether to auto-start Active Window OCR for the current flow.
///
/// - Quick Ask sessions should only consider the Quick Ask OCR mode.
/// - Non-Quick-Ask sessions may use OCR for rewrite and/or quick-replace flows.
pub(crate) fn should_auto_start_active_window_ocr(
    is_quick_ask_session: bool,
    rewrite_ocr_mode: &str,
    quick_ask_ocr_mode: &str,
    quick_replace_ocr_mode: &str,
) -> bool {
    if is_quick_ask_session {
        quick_ask_ocr_mode == "auto"
    } else {
        rewrite_ocr_mode == "auto" || quick_replace_ocr_mode == "auto"
    }
}

pub(crate) fn select_default_profile(llm_config: &LlmConfig) -> Option<ProgramPromptProfile> {
    llm_config
        .program_prompt_profiles
        .iter()
        .find(|p| p.id == "default")
        .cloned()
}

pub(crate) fn select_effective_preset(profile: &ProgramPromptProfile) -> Option<&ProgramPreset> {
    let find_by_id = |id: &str| profile.presets.iter().find(|p| p.id == id);

    if let Some(id) = profile.active_preset_id.as_deref() {
        return find_by_id(id);
    }
    if let Some(id) = profile.default_preset_id.as_deref() {
        return find_by_id(id);
    }
    None
}

pub(crate) fn find_preset_by_id<'a>(
    profile: &'a ProgramPromptProfile,
    id: &'a str,
) -> Option<&'a ProgramPreset> {
    profile.presets.iter().find(|p| p.id == id)
}

pub(crate) fn router_enabled(profile: &ProgramPromptProfile) -> bool {
    profile
        .router
        .as_ref()
        .map(|r| r.enabled && r.strategy != IntentRouterStrategy::Off)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::PromptSections;
    use crate::settings::IntentRouterSettings;

    fn preset(id: &str) -> ProgramPreset {
        ProgramPreset {
            id: id.to_string(),
            name: id.to_string(),
            routing_hints: vec![],
            prompts: PromptSections::default(),
            rewrite_llm_enabled: true,
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
        }
    }

    fn profile(id: &str, presets: Vec<ProgramPreset>) -> ProgramPromptProfile {
        ProgramPromptProfile {
            id: id.to_string(),
            name: id.to_string(),
            program_paths: vec![],
            prompts: PromptSections::default(),
            presets,
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
            quick_replace_enabled: None,
            quick_replace_provider: None,
            quick_replace_model: None,
            quick_replace_system_prompt: None,
            quick_ask_openai_reasoning_effort: None,
            quick_ask_gemini_thinking_budget: None,
            quick_ask_gemini_thinking_level: None,
            quick_ask_anthropic_thinking_budget: None,
            rewrite_active_window_ocr_mode: None,
            quick_replace_active_window_ocr_mode: None,
            quick_ask_active_window_ocr_mode: None,
        }
    }

    #[test]
    fn resolve_rewrite_ocr_mode_prefers_active_profile_then_default_then_global() {
        let mut default_p = profile("default", vec![]);
        default_p.rewrite_active_window_ocr_mode = Some("manual".to_string());

        let mut active_p = profile("app", vec![]);
        active_p.rewrite_active_window_ocr_mode = Some("auto".to_string());

        let mode = resolve_rewrite_active_window_ocr_mode(Some(&active_p), Some(&default_p), "off");
        assert_eq!(mode, "auto");

        let mode = resolve_rewrite_active_window_ocr_mode(None, Some(&default_p), "off");
        assert_eq!(mode, "manual");

        let mode = resolve_rewrite_active_window_ocr_mode(None, None, "auto");
        assert_eq!(mode, "auto");
    }

    #[test]
    fn resolve_quick_ask_ocr_mode_ignores_invalid_values() {
        let mut default_p = profile("default", vec![]);
        default_p.quick_ask_active_window_ocr_mode = Some("not-a-mode".to_string());

        let mut active_p = profile("app", vec![]);
        active_p.quick_ask_active_window_ocr_mode = Some("also-bad".to_string());

        let mode =
            resolve_quick_ask_active_window_ocr_mode(Some(&active_p), Some(&default_p), "manual");
        assert_eq!(mode, "manual");
    }

    #[test]
    fn resolve_quick_replace_ocr_mode_prefers_active_profile_then_default_then_global() {
        let mut default_p = profile("default", vec![]);
        default_p.quick_replace_active_window_ocr_mode = Some("manual".to_string());

        let mut active_p = profile("app", vec![]);
        active_p.quick_replace_active_window_ocr_mode = Some("auto".to_string());

        assert_eq!(
            resolve_quick_replace_active_window_ocr_mode(Some(&active_p), Some(&default_p), "off"),
            "auto"
        );
        assert_eq!(
            resolve_quick_replace_active_window_ocr_mode(None, Some(&default_p), "off"),
            "manual"
        );
        assert_eq!(
            resolve_quick_replace_active_window_ocr_mode(None, None, "not-a-mode"),
            "off"
        );
    }

    #[test]
    fn active_window_ocr_auto_start_uses_session_specific_precedence() {
        assert!(!should_auto_start_active_window_ocr(
            true, "auto", "off", "auto"
        ));
        assert!(should_auto_start_active_window_ocr(
            true, "off", "auto", "off"
        ));
        assert!(should_auto_start_active_window_ocr(
            false, "auto", "off", "off"
        ));
        assert!(should_auto_start_active_window_ocr(
            false, "off", "off", "auto"
        ));
        assert!(!should_auto_start_active_window_ocr(
            false, "off", "auto", "off"
        ));
    }

    #[test]
    fn select_effective_preset_prefers_active_then_default() {
        let mut p = profile("app", vec![preset("active"), preset("default")]);
        p.active_preset_id = Some("active".to_string());
        p.default_preset_id = Some("default".to_string());
        assert_eq!(
            select_effective_preset(&p).map(|p| p.id.as_str()),
            Some("active")
        );

        p.active_preset_id = Some("missing".to_string());
        assert!(select_effective_preset(&p).is_none());

        p.active_preset_id = None;
        assert_eq!(
            select_effective_preset(&p).map(|p| p.id.as_str()),
            Some("default")
        );

        p.default_preset_id = Some("missing".to_string());
        assert!(select_effective_preset(&p).is_none());
    }

    #[test]
    fn find_preset_by_id_returns_matching_preset_only() {
        let p = profile("app", vec![preset("one"), preset("two")]);

        assert_eq!(
            find_preset_by_id(&p, "two").map(|p| p.id.as_str()),
            Some("two")
        );
        assert!(find_preset_by_id(&p, "missing").is_none());
    }

    #[test]
    fn select_default_profile_and_router_enabled_handle_missing_and_disabled_values() {
        let default_p = profile("default", vec![]);
        let cfg = LlmConfig {
            program_prompt_profiles: vec![profile("app", vec![]), default_p],
            ..Default::default()
        };
        assert_eq!(
            select_default_profile(&cfg).map(|p| p.id),
            Some("default".to_string())
        );

        let mut p = profile("app", vec![]);
        assert!(!router_enabled(&p));

        p.router = Some(IntentRouterSettings {
            enabled: true,
            strategy: IntentRouterStrategy::Off,
            embedding_provider: None,
            embedding_model: None,
            pick_highest_score: false,
            similarity_threshold: None,
            similarity_margin: None,
            llm_provider: None,
            llm_model: None,
            llm_system_prompt: None,
            openai_reasoning_effort: None,
            gemini_thinking_budget: None,
            gemini_thinking_level: None,
            anthropic_thinking_budget: None,
        });
        assert!(!router_enabled(&p));

        p.router.as_mut().expect("router").strategy = IntentRouterStrategy::Embeddings;
        assert!(router_enabled(&p));
    }
}
