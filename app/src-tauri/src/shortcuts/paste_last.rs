//! Paste-last shortcut flow.
//!
//! This Module keeps the last-transcription output behavior together for both regular global
//! shortcuts and Windows modifier-only hook events. The main dispatcher still decides whether the
//! Paste Last action matched; this Module just owns the debounced release behavior and output work.

use std::borrow::Cow;
use std::sync::atomic::Ordering;

use tauri::{AppHandle, Manager};

use crate::history::HistoryStorage;
use crate::state::AppState;

#[derive(Debug, Clone, Copy)]
pub(crate) enum PasteLastShortcutSource<'a> {
    Global,
    ModifierOnly {
        key: &'a str,
        suppress_release_actions: bool,
    },
}

impl<'a> PasteLastShortcutSource<'a> {
    fn label(self) -> Cow<'a, str> {
        match self {
            Self::Global => Cow::Borrowed("OutputLast"),
            Self::ModifierOnly { key, .. } => Cow::Owned(format!("OutputLast({key})")),
        }
    }

    fn suppress_release_actions(self) -> bool {
        matches!(
            self,
            Self::ModifierOnly {
                suppress_release_actions: true,
                ..
            }
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PasteLastEventDecision {
    LatchPress,
    IgnoreReleaseNotHeld,
    IgnoreReleaseSuppressed,
    OutputLast,
}

fn classify_paste_last_event(
    is_down: bool,
    was_held: bool,
    suppress_release_actions: bool,
) -> PasteLastEventDecision {
    if is_down {
        return PasteLastEventDecision::LatchPress;
    }

    if !was_held {
        return PasteLastEventDecision::IgnoreReleaseNotHeld;
    }

    if suppress_release_actions {
        return PasteLastEventDecision::IgnoreReleaseSuppressed;
    }

    PasteLastEventDecision::OutputLast
}

fn output_last_transcription(app: &AppHandle, label: &str) {
    log::info!("{}: outputting last transcription", label);

    // Keep output intent resolution here so both global shortcuts and modifier-only hook events
    // honor the same persisted output-mode/privacy settings.
    let output_intent =
        crate::core::output_settings::resolve_output_intent_from_store(app, None, None);
    let history_storage = app.state::<HistoryStorage>();

    if let Ok(entries) = history_storage.get_all(Some(1)) {
        if let Some(entry) = entries.first() {
            if let Err(e) = crate::text::inject::output_text_with_app(
                app,
                &entry.text,
                output_intent.mode(),
                output_intent.hit_enter(),
                !output_intent.clipboard_privacy_mode(),
            ) {
                log::error!("Failed to output last transcription: {}", e);
            }
        } else {
            log::info!("{}: no history entries available", label);
        }
    }
}

pub(crate) fn handle_paste_last_shortcut_event(
    app: &AppHandle,
    state: &AppState,
    is_down: bool,
    source: PasteLastShortcutSource<'_>,
) {
    if is_down {
        state.paste_key_held.swap(true, Ordering::SeqCst);
        return;
    }

    let was_held = state.paste_key_held.swap(false, Ordering::SeqCst);
    if matches!(
        classify_paste_last_event(is_down, was_held, source.suppress_release_actions()),
        PasteLastEventDecision::OutputLast
    ) {
        let label = source.label();
        output_last_transcription(app, label.as_ref());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn global_paste_last_press_only_latches() {
        assert_eq!(
            classify_paste_last_event(true, false, false),
            PasteLastEventDecision::LatchPress
        );
    }

    #[test]
    fn global_paste_last_release_outputs_when_held() {
        assert_eq!(
            classify_paste_last_event(false, true, false),
            PasteLastEventDecision::OutputLast
        );
    }

    #[test]
    fn modifier_paste_last_release_can_be_suppressed() {
        assert_eq!(
            classify_paste_last_event(false, true, true),
            PasteLastEventDecision::IgnoreReleaseSuppressed
        );
    }

    #[test]
    fn modifier_paste_last_release_without_latch_is_ignored() {
        assert_eq!(
            classify_paste_last_event(false, false, false),
            PasteLastEventDecision::IgnoreReleaseNotHeld
        );
    }

    #[test]
    fn modifier_paste_last_source_label_includes_key_name() {
        let label = PasteLastShortcutSource::ModifierOnly {
            key: "AltRight",
            suppress_release_actions: false,
        }
        .label();

        assert_eq!(label.as_ref(), "OutputLast(AltRight)");
    }
}
