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

#[cfg(test)]
mod active_window_ocr_mode_tests {
    use super::should_auto_start_active_window_ocr;

    #[test]
    fn quick_ask_session_ignores_rewrite_auto() {
        // Regression test: rewrite auto should NOT force OCR to run during Quick Ask.
        assert!(!should_auto_start_active_window_ocr(
            true, "auto", "off", "auto"
        ));
    }

    #[test]
    fn quick_ask_session_respects_quick_ask_auto() {
        assert!(should_auto_start_active_window_ocr(
            true, "off", "auto", "off"
        ));
    }

    #[test]
    fn non_quick_ask_session_uses_rewrite_or_quick_replace_auto() {
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
}

fn normalize_program_path(path: &str) -> String {
    // Windows comparisons are case-insensitive, and we want to treat / and \\ equivalently.
    // Also strip common Windows path prefixes that may appear depending on how the OS reports
    // process image names.
    let mut s = path.trim().trim_matches('"').replace('/', "\\");
    if let Some(rest) = s.strip_prefix("\\\\?\\") {
        s = rest.to_string();
    } else if let Some(rest) = s.strip_prefix("\\\\??\\") {
        s = rest.to_string();
    }
    s.to_lowercase()
}

fn program_basename_lower(path_norm: &str) -> &str {
    path_norm.rsplit('\\').next().unwrap_or(path_norm).trim()
}

fn strip_exe_suffix(name: &str) -> &str {
    name.strip_suffix(".exe").unwrap_or(name)
}

fn program_basename_for_log(path: &str) -> String {
    // Only log the executable basename to avoid leaking sensitive filesystem paths
    // (usernames, install locations) into logs.
    let norm = normalize_program_path(path);
    let base = program_basename_lower(&norm);
    if base.is_empty() {
        "<unknown>".to_string()
    } else {
        base.to_string()
    }
}

fn matches_program_path_norm(
    foreground_norm: &str,
    foreground_base: &str,
    foreground_base_noexe: &str,
    configured: &str,
) -> bool {
    let p_norm = normalize_program_path(configured);
    if p_norm == foreground_norm {
        return true;
    }

    // Allow configuring just an executable name (e.g. "obsidian.exe") instead of
    // the full path. Also handle cases where the stored path differs (portable installs,
    // drive letter changes, etc.) but the basename is stable.
    let p_base = program_basename_lower(&p_norm);
    if !p_base.is_empty() {
        if p_base == foreground_base {
            return true;
        }
        if strip_exe_suffix(p_base) == foreground_base_noexe {
            return true;
        }
    }

    // If the stored path is a full path, basename check above already covers most
    // mismatches. As a last resort, allow "ends_with" on the normalized path.
    // This helps when the OS returns slightly different prefixes.
    if p_norm.contains('\\') {
        if foreground_norm.ends_with(&p_norm) {
            return true;
        }
        if !p_norm.ends_with(".exe") {
            let p_norm_exe = format!("{}.exe", p_norm);
            if foreground_norm.ends_with(&p_norm_exe) {
                return true;
            }
        }
    }

    false
}

/// Match a configured program path (either full path or basename) to a foreground executable path.
#[cfg(test)]
pub(crate) fn matches_program_path(foreground: &str, configured: &str) -> bool {
    let foreground_norm = normalize_program_path(foreground);
    let foreground_base = program_basename_lower(&foreground_norm);
    let foreground_base_noexe = strip_exe_suffix(foreground_base);
    matches_program_path_norm(
        &foreground_norm,
        foreground_base,
        foreground_base_noexe,
        configured,
    )
}

pub(crate) fn select_profile_for_program_path(
    llm_config: &LlmConfig,
    foreground: &str,
) -> Option<ProgramPromptProfile> {
    let foreground_norm = normalize_program_path(foreground);
    let foreground_base = program_basename_lower(&foreground_norm);
    let foreground_base_noexe = strip_exe_suffix(foreground_base);

    for profile in &llm_config.program_prompt_profiles {
        if profile.program_paths.iter().any(|p| {
            matches_program_path_norm(&foreground_norm, foreground_base, foreground_base_noexe, p)
        }) {
            log::debug!(
                "Pipeline: Using profile '{}' for foreground app {}",
                profile.name,
                program_basename_for_log(foreground)
            );
            return Some(profile.clone());
        }
    }

    // Helpful when users report everything is always "Default".
    // Keep this at trace to avoid noisy logs, but include key derived values.
    log::trace!(
        "Pipeline: No program profile match for foreground_base='{}' (profiles={})",
        program_basename_for_log(foreground),
        llm_config.program_prompt_profiles.len()
    );

    None
}

pub(crate) fn select_profile_for_foreground_app(
    llm_config: &LlmConfig,
) -> Option<ProgramPromptProfile> {
    let foreground = crate::windows_apps::get_foreground_process_path();
    let Some(foreground) = foreground else {
        log::debug!(
			"Pipeline: Foreground process path unavailable; cannot select per-program profile (profiles={})",
			llm_config.program_prompt_profiles.len()
		);
        return None;
    };

    select_profile_for_program_path(llm_config, &foreground)
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
    use crate::llm::{ProgramPromptProfile, PromptSections};

    fn profile(id: &str, program_paths: Vec<&str>) -> ProgramPromptProfile {
        ProgramPromptProfile {
            id: id.to_string(),
            name: id.to_string(),
            program_paths: program_paths.into_iter().map(ToString::to_string).collect(),
            prompts: PromptSections::default(),
            presets: vec![],
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

        let mut active_p = profile("app", vec!["app.exe"]);
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

        let mut active_p = profile("app", vec!["app.exe"]);
        active_p.quick_ask_active_window_ocr_mode = Some("also-bad".to_string());

        let mode =
            resolve_quick_ask_active_window_ocr_mode(Some(&active_p), Some(&default_p), "manual");
        assert_eq!(mode, "manual");
    }

    #[test]
    fn matches_full_path_case_insensitive() {
        assert!(matches_program_path(
            "C:/Program Files/Obsidian/Obsidian.EXE",
            "c:\\program files\\obsidian\\obsidian.exe"
        ));
    }

    #[test]
    fn matches_by_basename() {
        assert!(matches_program_path(
            "C:\\Apps\\Obsidian\\obsidian.exe",
            "obsidian.exe"
        ));
        assert!(matches_program_path("C:\\Apps\\ObSIDIAN.EXE", "obsidian"));
    }

    #[test]
    fn matches_windows_prefix_paths() {
        assert!(matches_program_path(
            "\\\\?\\C:\\Apps\\Foo\\bar.exe",
            "C:\\Apps\\Foo\\bar.exe"
        ));
        assert!(matches_program_path(
            "\\\\??\\C:\\Apps\\Foo\\bar.exe",
            "C:\\Apps\\Foo\\bar.exe"
        ));
    }

    #[test]
    fn select_profile_for_program_path_picks_first_match() {
        let cfg = LlmConfig {
            program_prompt_profiles: vec![
                profile("a", vec!["notepad.exe"]),
                profile("b", vec!["obsidian.exe"]),
                profile("c", vec!["obsidian.exe"]),
            ],
            ..Default::default()
        };

        let selected = select_profile_for_program_path(&cfg, "C:\\Apps\\Obsidian\\Obsidian.exe")
            .expect("expected profile");
        assert_eq!(selected.id, "b");
    }
}
