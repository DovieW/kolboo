use crate::llm::{LlmConfig, ProgramPreset, ProgramPromptProfile};
use crate::settings::IntentRouterStrategy;

/// Global Active Window OCR fallbacks for each flow.
///
/// Callers pass these together so Profile Resolution owns the full rewrite / Quick Ask /
/// Quick Replace precedence calculation instead of making every caller remember to invoke
/// three sibling helpers in the same order.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ActiveWindowOcrModeFallbacks<'a> {
    pub(crate) rewrite: &'a str,
    pub(crate) quick_ask: &'a str,
    pub(crate) quick_replace: &'a str,
}

/// Effective Active Window OCR modes for the current profile context.
///
/// This value is intentionally the caller-facing Interface for OCR-mode decisions: once a
/// caller has it, auto/manual/effective-flow questions stay local to Profile Resolution rules
/// instead of spreading string comparisons across the recording and overlay code paths.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResolvedActiveWindowOcrModes {
    rewrite: String,
    quick_ask: String,
    quick_replace: String,
}

impl ResolvedActiveWindowOcrModes {
    pub(crate) fn rewrite(&self) -> &str {
        self.rewrite.as_str()
    }

    pub(crate) fn quick_ask(&self) -> &str {
        self.quick_ask.as_str()
    }

    pub(crate) fn quick_replace(&self) -> &str {
        self.quick_replace.as_str()
    }

    /// The effective OCR mode to record for the user-visible flow.
    pub(crate) fn effective_mode_for_session(&self, is_quick_ask_session: bool) -> &str {
        if is_quick_ask_session {
            self.quick_ask()
        } else {
            // Normal dictation starts from the rewrite flow. Quick Replace can still attach
            // OCR later, so keep all modes logged separately for diagnostics.
            self.rewrite()
        }
    }

    /// Whether any flow currently supports manual OCR from the overlay button.
    pub(crate) fn has_manual_mode(&self) -> bool {
        self.rewrite() == "manual"
            || self.quick_ask() == "manual"
            || self.quick_replace() == "manual"
    }

    /// Decide whether OCR should auto-start for the current user-visible flow.
    ///
    /// Quick Ask intentionally considers only the Quick Ask mode; normal dictation can auto-start
    /// OCR for rewrite or Quick Replace because both can consume the same request-owned OCR Session.
    pub(crate) fn should_auto_start(&self, is_quick_ask_session: bool) -> bool {
        if is_quick_ask_session {
            self.quick_ask() == "auto"
        } else {
            self.rewrite() == "auto" || self.quick_replace() == "auto"
        }
    }

    /// Whether normal dictation should wait for an OCR Session result before
    /// continuing Transcription Flow.
    ///
    /// Rewrite and Quick Replace can both consume active-window OCR in the normal
    /// dictation path. Keeping this question here prevents callers from knowing
    /// the flow-specific string fields and repeating the same precedence rules.
    pub(crate) fn should_wait_for_normal_dictation_ocr(&self) -> bool {
        self.rewrite() != "off" || self.quick_replace() != "off"
    }
}

fn normalize_active_window_ocr_mode(mode: Option<&str>) -> Option<&str> {
    match mode {
        Some("off") | Some("auto") | Some("manual") => mode,
        _ => None,
    }
}

fn resolve_active_window_ocr_mode(
    active_profile_mode: Option<&str>,
    default_profile_mode: Option<&str>,
    global_fallback: &str,
) -> String {
    let global = normalize_active_window_ocr_mode(Some(global_fallback)).unwrap_or("off");
    normalize_active_window_ocr_mode(active_profile_mode)
        .or_else(|| normalize_active_window_ocr_mode(default_profile_mode))
        .unwrap_or(global)
        .to_string()
}

pub(crate) fn resolve_active_window_ocr_modes(
    active_profile: Option<&ProgramPromptProfile>,
    default_profile: Option<&ProgramPromptProfile>,
    global_fallbacks: ActiveWindowOcrModeFallbacks<'_>,
) -> ResolvedActiveWindowOcrModes {
    ResolvedActiveWindowOcrModes {
        rewrite: resolve_active_window_ocr_mode(
            active_profile.and_then(|p| p.rewrite_active_window_ocr_mode.as_deref()),
            default_profile.and_then(|p| p.rewrite_active_window_ocr_mode.as_deref()),
            global_fallbacks.rewrite,
        ),
        quick_ask: resolve_active_window_ocr_mode(
            active_profile.and_then(|p| p.quick_ask_active_window_ocr_mode.as_deref()),
            default_profile.and_then(|p| p.quick_ask_active_window_ocr_mode.as_deref()),
            global_fallbacks.quick_ask,
        ),
        quick_replace: resolve_active_window_ocr_mode(
            active_profile.and_then(|p| p.quick_replace_active_window_ocr_mode.as_deref()),
            default_profile.and_then(|p| p.quick_replace_active_window_ocr_mode.as_deref()),
            global_fallbacks.quick_replace,
        ),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DefaultProfileSelectionPolicy {
    UseDefaultAsActiveFallback,
    KeepDefaultAsFallbackOnly,
}

/// Coherent request-time profile context for pipeline/command callers.
///
/// Matching stays outside this Module (`profile_matcher.rs` still owns foreground-app matching),
/// but once a caller has a candidate foreground profile id and/or a session override, this
/// interface keeps profile/default/preset/OCR/request-log decisions together.
#[derive(Debug, Clone)]
pub(crate) struct RequestProfileContext {
    active_profile: Option<ProgramPromptProfile>,
    default_profile: Option<ProgramPromptProfile>,
    effective_preset: Option<ProgramPreset>,
    ocr_modes: ResolvedActiveWindowOcrModes,
    request_log_profile_id: Option<String>,
    request_log_profile_name: Option<String>,
}

impl RequestProfileContext {
    pub(crate) fn active_profile(&self) -> Option<&ProgramPromptProfile> {
        self.active_profile.as_ref()
    }

    pub(crate) fn default_profile(&self) -> Option<&ProgramPromptProfile> {
        self.default_profile.as_ref()
    }

    pub(crate) fn effective_preset(&self) -> Option<&ProgramPreset> {
        self.effective_preset.as_ref()
    }

    pub(crate) fn ocr_modes(&self) -> &ResolvedActiveWindowOcrModes {
        &self.ocr_modes
    }

    pub(crate) fn request_log_profile_id(&self) -> Option<&str> {
        self.request_log_profile_id.as_deref()
    }

    pub(crate) fn request_log_profile_name(&self) -> Option<&str> {
        self.request_log_profile_name.as_deref()
    }
}

pub(crate) fn resolve_request_profile_context(
    llm_config: &LlmConfig,
    session_profile_id: Option<&str>,
    foreground_profile: Option<ProgramPromptProfile>,
    global_ocr_fallbacks: ActiveWindowOcrModeFallbacks<'_>,
    default_policy: DefaultProfileSelectionPolicy,
) -> RequestProfileContext {
    let default_profile = select_default_profile(llm_config);

    let session_profile = session_profile_id.and_then(|id| {
        llm_config
            .program_prompt_profiles
            .iter()
            .find(|p| p.id == id)
            .cloned()
    });

    let active_profile = session_profile
        .or(foreground_profile)
        .or_else(|| match default_policy {
            DefaultProfileSelectionPolicy::UseDefaultAsActiveFallback => default_profile.clone(),
            DefaultProfileSelectionPolicy::KeepDefaultAsFallbackOnly => None,
        });

    let effective_preset = active_profile
        .as_ref()
        .and_then(select_effective_preset)
        .cloned();
    let ocr_modes = resolve_active_window_ocr_modes(
        active_profile.as_ref(),
        default_profile.as_ref(),
        global_ocr_fallbacks,
    );

    let (request_log_profile_id, request_log_profile_name) =
        if let Some(profile) = active_profile.as_ref() {
            (Some(profile.id.clone()), Some(profile.name.clone()))
        } else if session_profile_id == Some("default") {
            (Some("default".to_string()), Some("Default".to_string()))
        } else {
            (None, None)
        };

    RequestProfileContext {
        active_profile,
        default_profile,
        effective_preset,
        ocr_modes,
        request_log_profile_id,
        request_log_profile_name,
    }
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

    #[test]
    fn resolve_rewrite_ocr_mode_prefers_active_profile_then_default_then_global() {
        let mut default_p = profile("default", vec![]);
        default_p.rewrite_active_window_ocr_mode = Some("manual".to_string());

        let mut active_p = profile("app", vec![]);
        active_p.rewrite_active_window_ocr_mode = Some("auto".to_string());

        let modes = resolve_active_window_ocr_modes(
            Some(&active_p),
            Some(&default_p),
            ActiveWindowOcrModeFallbacks {
                rewrite: "off",
                quick_ask: "off",
                quick_replace: "off",
            },
        );
        assert_eq!(modes.rewrite(), "auto");

        let modes = resolve_active_window_ocr_modes(
            None,
            Some(&default_p),
            ActiveWindowOcrModeFallbacks {
                rewrite: "off",
                quick_ask: "off",
                quick_replace: "off",
            },
        );
        assert_eq!(modes.rewrite(), "manual");

        let modes = resolve_active_window_ocr_modes(
            None,
            None,
            ActiveWindowOcrModeFallbacks {
                rewrite: "auto",
                quick_ask: "off",
                quick_replace: "off",
            },
        );
        assert_eq!(modes.rewrite(), "auto");
    }

    #[test]
    fn resolve_quick_ask_ocr_mode_ignores_invalid_values() {
        let mut default_p = profile("default", vec![]);
        default_p.quick_ask_active_window_ocr_mode = Some("not-a-mode".to_string());

        let mut active_p = profile("app", vec![]);
        active_p.quick_ask_active_window_ocr_mode = Some("also-bad".to_string());

        let modes = resolve_active_window_ocr_modes(
            Some(&active_p),
            Some(&default_p),
            ActiveWindowOcrModeFallbacks {
                rewrite: "off",
                quick_ask: "manual",
                quick_replace: "off",
            },
        );
        assert_eq!(modes.quick_ask(), "manual");
    }

    #[test]
    fn resolve_quick_replace_ocr_mode_prefers_active_profile_then_default_then_global() {
        let mut default_p = profile("default", vec![]);
        default_p.quick_replace_active_window_ocr_mode = Some("manual".to_string());

        let mut active_p = profile("app", vec![]);
        active_p.quick_replace_active_window_ocr_mode = Some("auto".to_string());

        assert_eq!(
            resolve_active_window_ocr_modes(
                Some(&active_p),
                Some(&default_p),
                ActiveWindowOcrModeFallbacks {
                    rewrite: "off",
                    quick_ask: "off",
                    quick_replace: "off",
                },
            )
            .quick_replace(),
            "auto"
        );
        assert_eq!(
            resolve_active_window_ocr_modes(
                None,
                Some(&default_p),
                ActiveWindowOcrModeFallbacks {
                    rewrite: "off",
                    quick_ask: "off",
                    quick_replace: "off",
                },
            )
            .quick_replace(),
            "manual"
        );
        assert_eq!(
            resolve_active_window_ocr_modes(
                None,
                None,
                ActiveWindowOcrModeFallbacks {
                    rewrite: "off",
                    quick_ask: "off",
                    quick_replace: "not-a-mode",
                },
            )
            .quick_replace(),
            "off"
        );
    }

    #[test]
    fn resolve_active_window_ocr_modes_resolves_all_flows_together() {
        let mut default_p = profile("default", vec![]);
        default_p.rewrite_active_window_ocr_mode = Some("manual".to_string());
        default_p.quick_ask_active_window_ocr_mode = Some("auto".to_string());
        default_p.quick_replace_active_window_ocr_mode = Some("not-a-mode".to_string());

        let mut active_p = profile("app", vec![]);
        active_p.rewrite_active_window_ocr_mode = Some("also-bad".to_string());
        active_p.quick_replace_active_window_ocr_mode = Some("manual".to_string());

        let modes = resolve_active_window_ocr_modes(
            Some(&active_p),
            Some(&default_p),
            ActiveWindowOcrModeFallbacks {
                rewrite: "off",
                quick_ask: "manual",
                quick_replace: "auto",
            },
        );

        assert_eq!(modes.rewrite(), "manual");
        assert_eq!(modes.quick_ask(), "auto");
        assert_eq!(modes.quick_replace(), "manual");
        assert!(modes.has_manual_mode());
    }

    #[test]
    fn resolved_active_window_ocr_modes_own_flow_specific_decisions() {
        let modes = ResolvedActiveWindowOcrModes {
            rewrite: "auto".to_string(),
            quick_ask: "off".to_string(),
            quick_replace: "manual".to_string(),
        };

        assert_eq!(modes.effective_mode_for_session(true), "off");
        assert_eq!(modes.effective_mode_for_session(false), "auto");
        assert!(!modes.should_auto_start(true));
        assert!(modes.should_auto_start(false));
        assert!(modes.has_manual_mode());
        assert!(modes.should_wait_for_normal_dictation_ocr());
    }

    #[test]
    fn active_window_ocr_auto_start_uses_session_specific_precedence() {
        let modes =
            |rewrite: &str, quick_ask: &str, quick_replace: &str| ResolvedActiveWindowOcrModes {
                rewrite: rewrite.to_string(),
                quick_ask: quick_ask.to_string(),
                quick_replace: quick_replace.to_string(),
            };

        assert!(!modes("auto", "off", "auto").should_auto_start(true));
        assert!(modes("off", "auto", "off").should_auto_start(true));
        assert!(modes("auto", "off", "off").should_auto_start(false));
        assert!(modes("off", "off", "auto").should_auto_start(false));
        assert!(!modes("off", "auto", "off").should_auto_start(false));
        assert!(modes("off", "auto", "manual").should_wait_for_normal_dictation_ocr());
        assert!(!modes("off", "auto", "off").should_wait_for_normal_dictation_ocr());
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

    #[test]
    fn request_profile_context_prefers_session_override_and_returns_request_identity() {
        let mut app = profile("app", vec![preset("active")]);
        app.active_preset_id = Some("active".to_string());
        app.rewrite_active_window_ocr_mode = Some("manual".to_string());

        let mut default_p = profile("default", vec![preset("fallback")]);
        default_p.quick_ask_active_window_ocr_mode = Some("auto".to_string());

        let cfg = LlmConfig {
            program_prompt_profiles: vec![default_p.clone(), app.clone()],
            ..Default::default()
        };

        let context = resolve_request_profile_context(
            &cfg,
            Some("app"),
            Some(default_p.clone()),
            ActiveWindowOcrModeFallbacks {
                rewrite: "off",
                quick_ask: "off",
                quick_replace: "off",
            },
            DefaultProfileSelectionPolicy::UseDefaultAsActiveFallback,
        );

        assert_eq!(context.active_profile().map(|p| p.id.as_str()), Some("app"));
        assert_eq!(
            context.effective_preset().map(|preset| preset.id.as_str()),
            Some("active")
        );
        assert_eq!(context.request_log_profile_id(), Some("app"));
        assert_eq!(context.request_log_profile_name(), Some("app"));
        assert_eq!(context.ocr_modes().rewrite(), "manual");
        assert_eq!(context.ocr_modes().quick_ask(), "auto");
    }

    #[test]
    fn request_profile_context_can_keep_default_as_fallback_only() {
        let mut default_p = profile("default", vec![]);
        default_p.quick_replace_active_window_ocr_mode = Some("manual".to_string());

        let cfg = LlmConfig {
            program_prompt_profiles: vec![default_p],
            ..Default::default()
        };

        let context = resolve_request_profile_context(
            &cfg,
            None,
            None,
            ActiveWindowOcrModeFallbacks {
                rewrite: "off",
                quick_ask: "off",
                quick_replace: "off",
            },
            DefaultProfileSelectionPolicy::KeepDefaultAsFallbackOnly,
        );

        assert!(context.active_profile().is_none());
        assert_eq!(
            context.default_profile().map(|p| p.id.as_str()),
            Some("default")
        );
        assert_eq!(context.ocr_modes().quick_replace(), "manual");
        assert_eq!(context.request_log_profile_id(), None);
    }

    #[test]
    fn request_profile_context_can_promote_default_to_active_fallback() {
        let mut default_p = profile("default", vec![preset("fallback")]);
        default_p.default_preset_id = Some("fallback".to_string());
        let cfg = LlmConfig {
            program_prompt_profiles: vec![default_p],
            ..Default::default()
        };

        let context = resolve_request_profile_context(
            &cfg,
            None,
            None,
            ActiveWindowOcrModeFallbacks {
                rewrite: "auto",
                quick_ask: "off",
                quick_replace: "off",
            },
            DefaultProfileSelectionPolicy::UseDefaultAsActiveFallback,
        );

        assert_eq!(
            context.active_profile().map(|p| p.id.as_str()),
            Some("default")
        );
        assert_eq!(
            context.effective_preset().map(|preset| preset.id.as_str()),
            Some("fallback")
        );
        assert_eq!(context.request_log_profile_id(), Some("default"));
        assert_eq!(context.ocr_modes().rewrite(), "auto");
    }
}
