//! Shared lifecycle vocabulary for Quick Ask and Quick Replace requests.
//!
//! This module intentionally starts as pure data + small decision helpers. The goal is to
//! make the future flow extraction safer: `lib.rs` can gradually hand off configuration,
//! context, logging, and cleanup decisions without first moving OS-specific selection probing,
//! provider construction, or output injection internals.
//!
//! Keep Quick Ask and Quick Replace behavior explicit. They share request lifecycle ownership,
//! but Quick Ask answers in an overlay while Quick Replace rewrites selected text and may fall
//! back to normal dictation output.

use crate::commands::text::ContextGrabMethod;
use crate::llm::{self, LlmConfig, ProgramPromptProfile};
use crate::request_log::RequestKind;
use crate::sessions::selection_probe::{ProbeKind, SelectionProbeContext};
use crate::windows_uia::types::WindowsTextContextSource;

pub(crate) const DEFAULT_QUICK_ASK_SYSTEM_PROMPT: &str =
    "You are a helpful assistant. Answer the user's question based on the transcript.";

pub(crate) const DEFAULT_QUICK_REPLACE_SYSTEM_PROMPT: &str =
    "You are an expert editor. Apply the user's instructions to the provided text.\n\nRules:\n- Return ONLY the updated text (no commentary, no code fences).\n- Preserve the original language and formatting unless instructed otherwise.";

/// Default best-effort wait for highlighted-selection probes.
///
/// Keep this in lifecycle vocabulary so Quick Ask and Quick Replace do not quietly drift to
/// different probe timing when callers construct their plans.
pub(crate) const DEFAULT_QUICK_ACTION_PROBE_TIMEOUT_MS: u64 = 700;

/// The LLM-backed quick action that can claim a completed recording.
// Keep this enum tiny and explicit: these are feature-level actions, not provider-family seams.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum QuickActionKind {
    QuickAsk,
    QuickReplace,
}

impl QuickActionKind {
    /// Request-log kind to stamp when this quick action starts owning a request.
    pub(crate) fn request_kind(self) -> RequestKind {
        match self {
            Self::QuickAsk => RequestKind::QuickAsk,
            Self::QuickReplace => RequestKind::QuickReplace,
        }
    }

    /// Selection probe slot used by this quick action.
    pub(crate) fn probe_kind(self) -> ProbeKind {
        match self {
            Self::QuickAsk => ProbeKind::QuickAsk,
            Self::QuickReplace => ProbeKind::QuickReplace,
        }
    }
}

/// Recording-level intent resolved before the stop-recording flow decides who owns output.
// Quick Ask wins over Quick Replace so the final output path stays deterministic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RecordingIntent {
    primary: PrimaryRecordingFlow,
    quick_replace_enabled: bool,
}

impl RecordingIntent {
    /// Build the intent from today's AppState/settings booleans.
    ///
    /// Quick Ask is mutually exclusive with normal final output. Quick Replace is an optional
    /// post-transcription attempt for non-Quick-Ask recordings, so keep it as eligibility rather
    /// than as the primary flow.
    pub(crate) fn from_flags(is_quick_ask_session: bool, quick_replace_enabled: bool) -> Self {
        Self {
            primary: if is_quick_ask_session {
                PrimaryRecordingFlow::QuickAsk
            } else {
                PrimaryRecordingFlow::Dictation
            },
            quick_replace_enabled,
        }
    }

    pub(crate) fn is_quick_ask(self) -> bool {
        self.primary == PrimaryRecordingFlow::QuickAsk
    }

    /// Whether Quick Replace is allowed to attempt to claim this request.
    ///
    /// Quick Ask deliberately wins over Quick Replace. That preserves the current user-visible
    /// contract: Quick Ask answers in its overlay and never performs final paste/output.
    pub(crate) fn may_attempt_quick_replace(self) -> bool {
        self.primary == PrimaryRecordingFlow::Dictation && self.quick_replace_enabled
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PrimaryRecordingFlow {
    Dictation,
    QuickAsk,
}

/// Start-time Quick Ask configuration resolved from active/default profiles.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct QuickAskProfileConfig {
    pub(crate) provider: Option<String>,
    pub(crate) model: Option<String>,
    pub(crate) system_prompt: Option<String>,
    pub(crate) openai_reasoning_effort: Option<String>,
    pub(crate) gemini_thinking_budget: Option<i64>,
    pub(crate) gemini_thinking_level: Option<String>,
    pub(crate) anthropic_thinking_budget: Option<i64>,
    pub(crate) include_clipboard_context: bool,
}

impl QuickAskProfileConfig {
    pub(crate) fn from_profiles(
        active_profile: Option<&ProgramPromptProfile>,
        default_profile: Option<&ProgramPromptProfile>,
    ) -> Self {
        let include_clipboard_context = active_profile
            .and_then(|p| p.quick_ask_include_clipboard_context)
            .or_else(|| default_profile.and_then(|p| p.quick_ask_include_clipboard_context))
            .unwrap_or(false);

        active_profile
            .map(|p| Self {
                provider: p.quick_ask_provider.clone(),
                model: p.quick_ask_model.clone(),
                system_prompt: p.quick_ask_system_prompt.clone(),
                openai_reasoning_effort: p.quick_ask_openai_reasoning_effort.clone(),
                gemini_thinking_budget: p.quick_ask_gemini_thinking_budget,
                gemini_thinking_level: p.quick_ask_gemini_thinking_level.clone(),
                anthropic_thinking_budget: p.quick_ask_anthropic_thinking_budget,
                include_clipboard_context,
            })
            .unwrap_or(Self {
                include_clipboard_context,
                ..Default::default()
            })
    }
}

/// Global Quick Ask settings read by the caller from the settings store.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct QuickAskGlobalConfig {
    pub(crate) provider: Option<String>,
    pub(crate) model: Option<String>,
    pub(crate) system_prompt: Option<String>,
    pub(crate) openai_reasoning_effort: Option<String>,
    pub(crate) gemini_thinking_budget: Option<i64>,
    pub(crate) gemini_thinking_level: Option<String>,
    pub(crate) anthropic_thinking_budget: Option<i64>,
    pub(crate) fallback_provider: Option<String>,
    pub(crate) conversation_history_enabled: bool,
    pub(crate) conversation_history_count_raw: u64,
    pub(crate) request_logs_privacy_mode: bool,
}

/// Effective Quick Ask config after profile -> global Quick Ask -> global rewrite fallback.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct QuickAskEffectiveConfig {
    pub(crate) provider: String,
    pub(crate) model: Option<String>,
    pub(crate) system_prompt: String,
    pub(crate) openai_reasoning_effort: Option<String>,
    pub(crate) gemini_thinking_budget: Option<i64>,
    pub(crate) gemini_thinking_level: Option<String>,
    pub(crate) anthropic_thinking_budget: Option<i64>,
    pub(crate) include_clipboard_context: bool,
    pub(crate) conversation_history_enabled: bool,
    pub(crate) conversation_history_count: usize,
    pub(crate) request_logs_privacy_mode: bool,
}

impl QuickAskEffectiveConfig {
    pub(crate) fn resolve(profile: &QuickAskProfileConfig, global: QuickAskGlobalConfig) -> Self {
        Self {
            provider: profile
                .provider
                .clone()
                .or(global.provider)
                .or(global.fallback_provider)
                .unwrap_or_else(|| "openai".to_string()),
            model: profile.model.clone().or(global.model),
            system_prompt: profile
                .system_prompt
                .clone()
                .or(global.system_prompt)
                .unwrap_or_else(|| DEFAULT_QUICK_ASK_SYSTEM_PROMPT.to_string()),
            openai_reasoning_effort: profile
                .openai_reasoning_effort
                .clone()
                .or(global.openai_reasoning_effort),
            gemini_thinking_budget: profile
                .gemini_thinking_budget
                .or(global.gemini_thinking_budget),
            gemini_thinking_level: profile
                .gemini_thinking_level
                .clone()
                .or(global.gemini_thinking_level),
            anthropic_thinking_budget: profile
                .anthropic_thinking_budget
                .or(global.anthropic_thinking_budget),
            include_clipboard_context: profile.include_clipboard_context,
            conversation_history_enabled: global.conversation_history_enabled,
            conversation_history_count: global.conversation_history_count_raw.clamp(1, 20) as usize,
            request_logs_privacy_mode: global.request_logs_privacy_mode,
        }
    }
}

/// Effective Quick Replace config resolved at recording stop.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct QuickReplaceConfig {
    pub(crate) enabled: bool,
    pub(crate) provider: Option<String>,
    pub(crate) model: Option<String>,
    pub(crate) system_prompt: String,
    pub(crate) include_clipboard_context: bool,
}

impl QuickReplaceConfig {
    pub(crate) fn resolve(
        active_profile: Option<&ProgramPromptProfile>,
        default_profile: Option<&ProgramPromptProfile>,
        global_llm_config: &LlmConfig,
        is_quick_ask_session: bool,
        legacy_enabled: bool,
    ) -> Self {
        let enabled_opt = active_profile
            .and_then(|p| p.quick_replace_enabled)
            .or_else(|| default_profile.and_then(|p| p.quick_replace_enabled));

        let enabled = !is_quick_ask_session && enabled_opt.unwrap_or(legacy_enabled);

        let provider = active_profile
            .and_then(|p| p.quick_replace_provider.clone())
            .or_else(|| default_profile.and_then(|p| p.quick_replace_provider.clone()))
            .or_else(|| active_profile.and_then(|p| p.llm_provider.clone()))
            .or_else(|| default_profile.and_then(|p| p.llm_provider.clone()))
            .or(Some(global_llm_config.provider.clone()));

        let provider_for_default_model = provider.as_deref().unwrap_or("openai");

        let model = active_profile
            .and_then(|p| p.quick_replace_model.clone())
            .or_else(|| default_profile.and_then(|p| p.quick_replace_model.clone()))
            .or_else(|| active_profile.and_then(|p| p.llm_model.clone()))
            .or_else(|| default_profile.and_then(|p| p.llm_model.clone()))
            .or_else(|| global_llm_config.model.clone())
            .or_else(|| {
                llm::default_llm_model_for_provider(provider_for_default_model)
                    .map(|m| m.to_string())
            });

        let system_prompt = active_profile
            .and_then(|p| p.quick_replace_system_prompt.clone())
            .or_else(|| default_profile.and_then(|p| p.quick_replace_system_prompt.clone()))
            .unwrap_or_else(|| DEFAULT_QUICK_REPLACE_SYSTEM_PROMPT.to_string());

        let include_clipboard_context = active_profile
            .and_then(|p| p.quick_replace_include_clipboard_context)
            .or_else(|| default_profile.and_then(|p| p.quick_replace_include_clipboard_context))
            .unwrap_or(false);

        Self {
            enabled,
            provider,
            model,
            system_prompt,
            include_clipboard_context,
        }
    }
}

/// Resolve the highlighted-selection capture method from active/default profiles.
pub(crate) fn resolve_context_grab_method(
    active_profile: Option<&ProgramPromptProfile>,
    default_profile: Option<&ProgramPromptProfile>,
) -> ContextGrabMethod {
    let method_str = active_profile
        .and_then(|p| p.context_grab_method.as_deref())
        .or_else(|| default_profile.and_then(|p| p.context_grab_method.as_deref()));

    match method_str {
        Some("none") => ContextGrabMethod::None,
        Some("ctrl_shift_c") => ContextGrabMethod::CtrlShiftC,
        Some("ctrl_insert") => ContextGrabMethod::CtrlInsert,
        _ => ContextGrabMethod::CtrlC,
    }
}

/// Plan for a selection probe that belongs to a quick-action attempt.
// `lib.rs` may still pass raw epochs to lower-level helpers, but this type owns the decision
// about whether awaiting a probe can produce useful context.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct QuickActionProbePlan {
    kind: QuickActionKind,
    epoch: u64,
    context_grab_method: ContextGrabMethod,
    timeout_ms: u64,
}

impl QuickActionProbePlan {
    pub(crate) fn new(
        kind: QuickActionKind,
        epoch: u64,
        context_grab_method: ContextGrabMethod,
        timeout_ms: u64,
    ) -> Self {
        Self {
            kind,
            epoch,
            context_grab_method,
            timeout_ms,
        }
    }

    pub(crate) fn probe_kind(self) -> ProbeKind {
        self.kind.probe_kind()
    }

    pub(crate) fn epoch(self) -> u64 {
        self.epoch
    }

    pub(crate) fn timeout_ms(self) -> u64 {
        self.timeout_ms
    }

    /// Whether waiting for this probe can ever produce context.
    ///
    /// An epoch of zero is the existing sentinel for "probe was not started". `None` context grab
    /// disables probing even if a caller accidentally passes a non-zero epoch.
    pub(crate) fn should_await(self) -> bool {
        self.epoch != 0 && self.context_grab_method != ContextGrabMethod::None
    }
}

/// Structured context gathered for one quick-action request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct QuickActionContext {
    pub(crate) selection_text: Option<String>,
    pub(crate) surrounding_text: Option<String>,
    pub(crate) clipboard_context: Option<String>,
    pub(crate) selection_source: WindowsTextContextSource,
}

impl Default for QuickActionContext {
    fn default() -> Self {
        Self {
            selection_text: None,
            surrounding_text: None,
            clipboard_context: None,
            selection_source: WindowsTextContextSource::None,
        }
    }
}

impl QuickActionContext {
    pub(crate) fn from_probe_result(probe: Option<SelectionProbeContext>) -> Self {
        let Some(probe) = probe else {
            return Self::default();
        };

        Self {
            selection_text: probe.selection_text,
            surrounding_text: probe.surrounding_text,
            clipboard_context: None,
            selection_source: probe.source,
        }
    }

    pub(crate) fn with_clipboard_context(mut self, clipboard_context: Option<String>) -> Self {
        self.clipboard_context = clipboard_context;
        self
    }

    pub(crate) fn selected_text_for_prompt(&self) -> Option<&str> {
        normalized_prompt_text(self.selection_text.as_deref())
    }

    pub(crate) fn surrounding_text_for_prompt(&self) -> Option<&str> {
        normalized_prompt_text(self.surrounding_text.as_deref())
    }

    pub(crate) fn clipboard_context_for_prompt(&self) -> Option<&str> {
        normalized_prompt_text(self.clipboard_context.as_deref())
    }
}

fn normalized_prompt_text(text: Option<&str>) -> Option<&str> {
    text.map(str::trim).filter(|text| !text.is_empty())
}

/// Sentinel written by selection probing while waiting for the target app to copy text.
pub(crate) fn is_selection_probe_sentinel(text: &str) -> bool {
    text.trim_start().starts_with("__kolboo_selection_probe__")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn probe(
        selection_text: Option<&str>,
        surrounding_text: Option<&str>,
    ) -> SelectionProbeContext {
        SelectionProbeContext {
            selection_text: selection_text.map(str::to_string),
            surrounding_text: surrounding_text.map(str::to_string),
            source: WindowsTextContextSource::Uia,
        }
    }

    fn profile(id: &str) -> ProgramPromptProfile {
        ProgramPromptProfile {
            id: id.to_string(),
            name: id.to_string(),
            program_paths: Vec::new(),
            prompts: crate::llm::PromptSections::default(),
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

    #[test]
    fn quick_action_kind_maps_to_request_log_and_probe_kinds() {
        assert_eq!(
            QuickActionKind::QuickAsk.request_kind(),
            RequestKind::QuickAsk
        );
        assert_eq!(
            QuickActionKind::QuickReplace.request_kind(),
            RequestKind::QuickReplace
        );
        assert_eq!(QuickActionKind::QuickAsk.probe_kind(), ProbeKind::QuickAsk);
        assert_eq!(
            QuickActionKind::QuickReplace.probe_kind(),
            ProbeKind::QuickReplace
        );
    }

    #[test]
    fn recording_intent_keeps_quick_ask_primary_over_quick_replace() {
        let intent = RecordingIntent::from_flags(true, true);
        assert!(intent.is_quick_ask());
        assert!(!intent.may_attempt_quick_replace());

        let intent = RecordingIntent::from_flags(false, true);
        assert!(!intent.is_quick_ask());
        assert!(intent.may_attempt_quick_replace());

        let intent = RecordingIntent::from_flags(false, false);
        assert!(!intent.may_attempt_quick_replace());
    }

    #[test]
    fn probe_plan_only_waits_for_started_enabled_probes() {
        let active = QuickActionProbePlan::new(
            QuickActionKind::QuickReplace,
            42,
            ContextGrabMethod::CtrlC,
            DEFAULT_QUICK_ACTION_PROBE_TIMEOUT_MS,
        );
        assert!(active.should_await());
        assert_eq!(active.probe_kind(), ProbeKind::QuickReplace);
        assert_eq!(active.epoch(), 42);
        assert_eq!(active.timeout_ms(), DEFAULT_QUICK_ACTION_PROBE_TIMEOUT_MS);

        let missing_epoch = QuickActionProbePlan::new(
            QuickActionKind::QuickAsk,
            0,
            ContextGrabMethod::CtrlC,
            DEFAULT_QUICK_ACTION_PROBE_TIMEOUT_MS,
        );
        assert!(!missing_epoch.should_await());

        let disabled_method = QuickActionProbePlan::new(
            QuickActionKind::QuickAsk,
            42,
            ContextGrabMethod::None,
            DEFAULT_QUICK_ACTION_PROBE_TIMEOUT_MS,
        );
        assert!(!disabled_method.should_await());
    }

    #[test]
    fn quick_ask_profile_config_uses_active_values_and_default_clipboard() {
        let mut active = profile("active");
        active.quick_ask_provider = Some("anthropic".into());
        active.quick_ask_model = Some("claude-test".into());
        active.quick_ask_system_prompt = Some("answer briefly".into());
        active.quick_ask_openai_reasoning_effort = Some("medium".into());
        active.quick_ask_gemini_thinking_budget = Some(123);
        active.quick_ask_gemini_thinking_level = Some("high".into());
        active.quick_ask_anthropic_thinking_budget = Some(456);

        let mut default = profile("default");
        default.quick_ask_include_clipboard_context = Some(true);

        let cfg = QuickAskProfileConfig::from_profiles(Some(&active), Some(&default));

        assert_eq!(cfg.provider.as_deref(), Some("anthropic"));
        assert_eq!(cfg.model.as_deref(), Some("claude-test"));
        assert_eq!(cfg.system_prompt.as_deref(), Some("answer briefly"));
        assert_eq!(cfg.openai_reasoning_effort.as_deref(), Some("medium"));
        assert_eq!(cfg.gemini_thinking_budget, Some(123));
        assert_eq!(cfg.gemini_thinking_level.as_deref(), Some("high"));
        assert_eq!(cfg.anthropic_thinking_budget, Some(456));
        assert!(cfg.include_clipboard_context);
    }

    #[test]
    fn quick_ask_effective_config_preserves_fallback_order_and_clamps_history() {
        let profile_cfg = QuickAskProfileConfig {
            provider: Some("profile-provider".into()),
            model: Some("profile-model".into()),
            system_prompt: None,
            gemini_thinking_budget: Some(99),
            include_clipboard_context: true,
            ..Default::default()
        };

        let cfg = QuickAskEffectiveConfig::resolve(
            &profile_cfg,
            QuickAskGlobalConfig {
                provider: Some("quick-ask-provider".into()),
                model: Some("quick-ask-model".into()),
                system_prompt: Some("global prompt".into()),
                openai_reasoning_effort: Some("low".into()),
                gemini_thinking_budget: Some(7),
                gemini_thinking_level: Some("auto".into()),
                anthropic_thinking_budget: Some(8),
                fallback_provider: Some("rewrite-provider".into()),
                conversation_history_enabled: true,
                conversation_history_count_raw: 200,
                request_logs_privacy_mode: true,
            },
        );

        assert_eq!(cfg.provider, "profile-provider");
        assert_eq!(cfg.model.as_deref(), Some("profile-model"));
        assert_eq!(cfg.system_prompt, "global prompt");
        assert_eq!(cfg.openai_reasoning_effort.as_deref(), Some("low"));
        assert_eq!(cfg.gemini_thinking_budget, Some(99));
        assert_eq!(cfg.gemini_thinking_level.as_deref(), Some("auto"));
        assert_eq!(cfg.anthropic_thinking_budget, Some(8));
        assert!(cfg.include_clipboard_context);
        assert!(cfg.conversation_history_enabled);
        assert_eq!(cfg.conversation_history_count, 20);
        assert!(cfg.request_logs_privacy_mode);

        let fallback_cfg = QuickAskEffectiveConfig::resolve(
            &QuickAskProfileConfig::default(),
            QuickAskGlobalConfig {
                fallback_provider: Some("rewrite-provider".into()),
                conversation_history_count_raw: 0,
                ..Default::default()
            },
        );
        assert_eq!(fallback_cfg.provider, "rewrite-provider");
        assert_eq!(fallback_cfg.system_prompt, DEFAULT_QUICK_ASK_SYSTEM_PROMPT);
        assert_eq!(fallback_cfg.conversation_history_count, 1);
    }

    #[test]
    fn quick_replace_config_resolves_profile_defaults_and_disables_during_quick_ask() {
        let mut active = profile("active");
        active.quick_replace_enabled = Some(true);
        active.quick_replace_provider = Some("active-provider".into());
        active.quick_replace_system_prompt = Some("rewrite exactly".into());

        let mut default = profile("default");
        default.quick_replace_model = Some("default-model".into());
        default.quick_replace_include_clipboard_context = Some(true);

        let cfg = QuickReplaceConfig::resolve(
            Some(&active),
            Some(&default),
            &LlmConfig::default(),
            false,
            false,
        );

        assert!(cfg.enabled);
        assert_eq!(cfg.provider.as_deref(), Some("active-provider"));
        assert_eq!(cfg.model.as_deref(), Some("default-model"));
        assert_eq!(cfg.system_prompt, "rewrite exactly");
        assert!(cfg.include_clipboard_context);

        let quick_ask_cfg = QuickReplaceConfig::resolve(
            Some(&active),
            Some(&default),
            &LlmConfig::default(),
            true,
            true,
        );
        assert!(!quick_ask_cfg.enabled);
    }

    #[test]
    fn context_grab_method_uses_active_then_default() {
        let mut active = profile("active");
        active.context_grab_method = Some("ctrl_shift_c".into());

        let mut default = profile("default");
        default.context_grab_method = Some("ctrl_insert".into());

        assert_eq!(
            resolve_context_grab_method(Some(&active), Some(&default)),
            ContextGrabMethod::CtrlShiftC
        );

        assert_eq!(
            resolve_context_grab_method(None, Some(&default)),
            ContextGrabMethod::CtrlInsert
        );

        // Preserve current behavior: unknown/newer persisted values fall back to Ctrl+C until
        // the settings UI/runtime contract deliberately opts into them.
        default.context_grab_method = Some("clipboard_only".into());
        assert_eq!(
            resolve_context_grab_method(None, Some(&default)),
            ContextGrabMethod::CtrlC
        );
    }

    #[test]
    fn context_from_probe_preserves_selection_surrounding_and_source() {
        let context = QuickActionContext::from_probe_result(Some(probe(
            Some("  selected  "),
            Some("  surrounding  "),
        )));

        assert_eq!(context.selected_text_for_prompt(), Some("selected"));
        assert_eq!(context.surrounding_text_for_prompt(), Some("surrounding"));
        assert_eq!(context.selection_source, WindowsTextContextSource::Uia);
    }

    #[test]
    fn context_distinguishes_empty_from_missing_context_for_attempts() {
        let empty_selection = QuickActionContext::from_probe_result(Some(probe(Some("  "), None)));
        assert_eq!(empty_selection.selected_text_for_prompt(), None);
        assert_eq!(empty_selection.surrounding_text_for_prompt(), None);

        let no_probe = QuickActionContext::from_probe_result(None);
        assert_eq!(no_probe.selection_source, WindowsTextContextSource::None);
        assert_eq!(no_probe.selected_text_for_prompt(), None);
        assert_eq!(no_probe.surrounding_text_for_prompt(), None);
    }

    #[test]
    fn context_counts_clipboard_text_as_prompt_context() {
        let clipboard = QuickActionContext::default().with_clipboard_context(Some(" clip ".into()));
        assert_eq!(clipboard.clipboard_context_for_prompt(), Some("clip"));
    }

    #[test]
    fn sentinel_detection_matches_selection_probe_clipboard_marker() {
        assert!(is_selection_probe_sentinel(
            "   __kolboo_selection_probe__abc"
        ));
        assert!(!is_selection_probe_sentinel("actual clipboard text"));
    }
}
