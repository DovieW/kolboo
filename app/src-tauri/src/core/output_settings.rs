use crate::commands::text::OutputMode;
use tauri::AppHandle;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ResolvedOutputIntent {
    mode: OutputMode,
    hit_enter: bool,
    clipboard_privacy_mode: bool,
    smart_paste_protection: bool,
}

impl ResolvedOutputIntent {
    pub(crate) fn mode(&self) -> OutputMode {
        self.mode
    }

    pub(crate) fn hit_enter(&self) -> bool {
        self.hit_enter
    }

    pub(crate) fn clipboard_privacy_mode(&self) -> bool {
        self.clipboard_privacy_mode
    }

    pub(crate) fn smart_paste_protection(&self) -> bool {
        self.smart_paste_protection
    }
}

/// Resolve the effective output settings (mode + hit-enter) given global settings
/// and optional per-program profile overrides.
///
/// Why this exists (plain English):
/// - The UI lets you set output mode per program profile (e.g. terminals use clipboard-only).
/// - The backend still needs to apply those per-profile overrides at output time.
/// - We also defensively force `hit_enter = false` when output mode is "clipboard".
pub(crate) fn resolve_effective_output_settings(
    global_mode: OutputMode,
    global_hit_enter: bool,
    profile_output_mode: Option<&str>,
    profile_hit_enter: Option<bool>,
) -> (OutputMode, bool) {
    let mode_override = profile_output_mode.and_then(parse_output_mode_override);
    let mode = mode_override.unwrap_or(global_mode);
    let mut hit_enter = profile_hit_enter.unwrap_or(global_hit_enter);

    // "clipboard" means we never paste, so "hit enter" doesn't make sense.
    if matches!(mode, OutputMode::Clipboard) {
        hit_enter = false;
    }

    (mode, hit_enter)
}

pub(crate) fn resolve_output_intent(
    global_mode: OutputMode,
    global_hit_enter: bool,
    profile_output_mode: Option<&str>,
    profile_hit_enter: Option<bool>,
    clipboard_privacy_mode: bool,
    smart_paste_protection: bool,
) -> ResolvedOutputIntent {
    let (mode, hit_enter) = resolve_effective_output_settings(
        global_mode,
        global_hit_enter,
        profile_output_mode,
        profile_hit_enter,
    );

    ResolvedOutputIntent {
        mode,
        hit_enter,
        clipboard_privacy_mode,
        smart_paste_protection,
    }
}

pub(crate) fn resolve_output_intent_from_store(
    app: &AppHandle,
    profile_output_mode: Option<&str>,
    profile_hit_enter: Option<bool>,
) -> ResolvedOutputIntent {
    let global_mode_str: String =
        crate::get_setting_from_store(app, "output_mode", "paste".to_string());
    let global_mode = OutputMode::from_str(&global_mode_str);
    let global_hit_enter: bool = crate::get_setting_from_store(app, "output_hit_enter", false);
    let clipboard_privacy_mode: bool =
        crate::get_setting_from_store(app, "output_clipboard_privacy_mode", false);
    let smart_paste_protection: bool =
        crate::get_setting_from_store(app, "output_smart_paste_protection", false);

    resolve_output_intent(
        global_mode,
        global_hit_enter,
        profile_output_mode,
        profile_hit_enter,
        clipboard_privacy_mode,
        smart_paste_protection,
    )
}

fn parse_output_mode_override(raw: &str) -> Option<OutputMode> {
    match raw.trim() {
        "paste" => Some(OutputMode::Paste),
        "paste_and_clipboard" => Some(OutputMode::PasteAndClipboard),
        "clipboard" => Some(OutputMode::Clipboard),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_overrides_uses_global() {
        let (mode, hit_enter) =
            resolve_effective_output_settings(OutputMode::Paste, true, None, None);
        assert_eq!(mode, OutputMode::Paste);
        assert!(hit_enter);
    }

    #[test]
    fn profile_can_override_mode_and_hit_enter() {
        let (mode, hit_enter) = resolve_effective_output_settings(
            OutputMode::Paste,
            false,
            Some("paste_and_clipboard"),
            Some(true),
        );
        assert_eq!(mode, OutputMode::PasteAndClipboard);
        assert!(hit_enter);
    }

    #[test]
    fn clipboard_forces_hit_enter_false_even_if_overridden_true() {
        let (mode, hit_enter) = resolve_effective_output_settings(
            OutputMode::Paste,
            true,
            Some("clipboard"),
            Some(true),
        );
        assert_eq!(mode, OutputMode::Clipboard);
        assert!(!hit_enter);
    }

    #[test]
    fn invalid_profile_mode_does_not_clobber_global() {
        let (mode, hit_enter) = resolve_effective_output_settings(
            OutputMode::PasteAndClipboard,
            false,
            Some("not-a-real-mode"),
            Some(true),
        );
        assert_eq!(mode, OutputMode::PasteAndClipboard);
        assert!(hit_enter);
    }

    #[test]
    fn resolved_output_intent_carries_output_flags_after_mode_resolution() {
        let intent = resolve_output_intent(
            OutputMode::Paste,
            true,
            Some("clipboard"),
            Some(true),
            true,
            true,
        );

        assert_eq!(intent.mode(), OutputMode::Clipboard);
        assert!(!intent.hit_enter());
        assert!(intent.clipboard_privacy_mode());
        assert!(intent.smart_paste_protection());
    }
}
