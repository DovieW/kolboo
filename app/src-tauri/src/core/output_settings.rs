use crate::commands::text::OutputMode;
use crate::settings::store::SettingsReadMode;
use crate::settings_view;
use crate::text::key_inject::PasteShortcut;
use tauri::AppHandle;
use tauri::Manager;

/// Resolve a captured profile for recording output, or the foreground app for
/// one-shot actions. Never re-detect focus after a recording has started.
pub(crate) fn paste_shortcut_for_profile(
    app: &AppHandle,
    profile_id: Option<&str>,
) -> PasteShortcut {
    let global =
        settings_view::read_output_settings_view(app, SettingsReadMode::Cached).paste_shortcut;
    let profiles: Vec<crate::settings::RewriteProgramPromptProfile> =
        crate::get_setting_from_store(app, "rewrite_program_prompt_profiles", Vec::new());
    resolve_profile_paste_shortcut(global, profile_id, &profiles)
}

fn resolve_profile_paste_shortcut(
    global: PasteShortcut,
    profile_id: Option<&str>,
    profiles: &[crate::settings::RewriteProgramPromptProfile],
) -> PasteShortcut {
    let profile = profiles
        .iter()
        .find(|p| Some(p.id.as_str()) == profile_id && p.id != "default");
    profile
        .and_then(|p| p.output_paste_shortcut.as_deref())
        .and_then(PasteShortcut::parse)
        .unwrap_or(global)
}

pub(crate) fn foreground_paste_shortcut(app: &AppHandle) -> PasteShortcut {
    let profile = app
        .try_state::<crate::pipeline::SharedPipeline>()
        .and_then(|pipeline| {
            crate::pipeline::select_profile_for_foreground_app(&pipeline.config().llm_config)
        });
    paste_shortcut_for_profile(app, profile.as_ref().map(|p| p.id.as_str()))
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ResolvedOutputIntent {
    mode: OutputMode,
    hit_enter: bool,
    clipboard_privacy_mode: bool,
    smart_paste_protection: bool,
    paste_shortcut: PasteShortcut,
}

impl ResolvedOutputIntent {
    pub(crate) fn paste_shortcut(&self) -> PasteShortcut {
        self.paste_shortcut
    }

    pub(crate) fn with_paste_shortcut(
        mut self,
        global: PasteShortcut,
        profile: Option<&str>,
    ) -> Self {
        self.paste_shortcut = profile.and_then(PasteShortcut::parse).unwrap_or(global);
        self
    }

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
        paste_shortcut: PasteShortcut::System,
    }
}

pub(crate) fn resolve_output_intent_from_store(
    app: &AppHandle,
    profile_output_mode: Option<&str>,
    profile_hit_enter: Option<bool>,
    profile_paste_shortcut: Option<&str>,
) -> ResolvedOutputIntent {
    let view = settings_view::read_output_settings_view(app, SettingsReadMode::Cached);

    resolve_output_intent(
        view.mode,
        view.hit_enter,
        profile_output_mode,
        profile_hit_enter,
        view.clipboard_privacy_mode,
        view.smart_paste_protection,
    )
    .with_paste_shortcut(view.paste_shortcut, profile_paste_shortcut)
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
    fn captured_profile_shortcut_survives_serialization_and_other_profiles_stay_unchanged() {
        let mut profiles: Vec<crate::settings::RewriteProgramPromptProfile> =
            serde_json::from_value(serde_json::json!([
                { "id": "terminal", "name": "Terminal", "output_paste_shortcut": "ctrl_shift_v" },
                { "id": "browser", "name": "Browser" },
                { "id": "default", "name": "Default", "output_paste_shortcut": "shift_insert" }
            ]))
            .unwrap();
        profiles = serde_json::from_str(&serde_json::to_string(&profiles).unwrap()).unwrap();
        let global = PasteShortcut::CtrlV;
        assert_eq!(
            resolve_profile_paste_shortcut(global, Some("terminal"), &profiles),
            PasteShortcut::CtrlShiftV
        );
        for id in [Some("browser"), Some("default"), Some("deleted"), None] {
            assert_eq!(
                resolve_profile_paste_shortcut(global, id, &profiles),
                global
            );
        }
        profiles[0].output_paste_shortcut = Some("invalid".into());
        assert_eq!(
            resolve_profile_paste_shortcut(global, Some("terminal"), &profiles),
            global
        );
        profiles[0].output_paste_shortcut = None;
        assert_eq!(
            resolve_profile_paste_shortcut(global, Some("terminal"), &profiles),
            global
        );
    }

    #[test]
    fn paste_shortcut_overrides_are_explicit_and_invalid_values_inherit() {
        let base = resolve_output_intent(OutputMode::Paste, false, None, None, false, false);
        assert_eq!(base.paste_shortcut(), PasteShortcut::System);
        for missing in [None, Some("bad")] {
            assert_eq!(
                base.with_paste_shortcut(PasteShortcut::CtrlShiftV, missing)
                    .paste_shortcut(),
                PasteShortcut::CtrlShiftV
            );
        }
        assert_eq!(
            base.with_paste_shortcut(PasteShortcut::CtrlV, Some("ctrl_shift_v"))
                .paste_shortcut(),
            PasteShortcut::CtrlShiftV
        );
        assert_eq!(
            base.with_paste_shortcut(PasteShortcut::CtrlShiftV, Some("system"))
                .paste_shortcut(),
            PasteShortcut::System
        );
        // Clipboard mode still suppresses Enter; setting a chord never changes output mode.
        let copy = resolve_output_intent(OutputMode::Clipboard, true, None, None, true, true)
            .with_paste_shortcut(PasteShortcut::ShiftInsert, None);
        assert_eq!(copy.mode(), OutputMode::Clipboard);
        assert!(!copy.hit_enter());
        assert!(copy.clipboard_privacy_mode());
    }

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
