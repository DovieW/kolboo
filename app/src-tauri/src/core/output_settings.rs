use crate::commands::text::OutputMode;

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
}
