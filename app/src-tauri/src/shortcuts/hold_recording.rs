//! Hold-to-record shortcut flow.
//!
//! This Module keeps the hold-specific start-on-press / stop-on-release rules together for both
//! regular global shortcuts and Windows modifier-only hook events. The main dispatcher still owns
//! action matching, while registration stays in `shortcuts/lifecycle.rs` and the low-level hook
//! stays in `windows_modifier_hotkeys.rs`.

use std::borrow::Cow;
use std::sync::atomic::Ordering;

use tauri::{AppHandle, Manager};

use crate::audio;
use crate::audio_mute::AudioMuteManager;
use crate::core::recording::PlayingAudioHandling;
use crate::emit_system_event;
use crate::pipeline::{self, PipelineState};
use crate::state::AppState;
use crate::{start_recording, stop_recording};

#[derive(Debug, Clone, Copy)]
pub(crate) enum HoldShortcutSource<'a> {
    Global,
    ModifierOnly { key: &'a str },
}

impl<'a> HoldShortcutSource<'a> {
    fn label(self) -> Cow<'a, str> {
        match self {
            Self::Global => Cow::Borrowed("Hold"),
            Self::ModifierOnly { key } => Cow::Owned(format!("Hold({key})")),
        }
    }

    fn emit_press_diagnostics(self, app: &AppHandle, pipeline_state: Option<PipelineState>) {
        if !matches!(self, Self::Global) {
            return;
        }

        log::info!("Hold pressed: pipeline state = {:?}", pipeline_state);
        emit_system_event(
            app,
            "shortcut",
            "Hold key pressed",
            Some(&format!("Pipeline state: {:?}", pipeline_state)),
        );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HoldEventDecision {
    IgnorePressRepeat,
    StartRecording,
    IgnorePressState,
    IgnoreReleaseNotHeld,
    StopRecording,
    IgnoreReleaseState,
}

fn classify_hold_event(
    is_down: bool,
    was_held: bool,
    pipeline_state: Option<PipelineState>,
) -> HoldEventDecision {
    if is_down {
        if was_held {
            return HoldEventDecision::IgnorePressRepeat;
        }

        if pipeline_state
            .map(|state| state.can_start_recording())
            .unwrap_or(false)
        {
            return HoldEventDecision::StartRecording;
        }

        return HoldEventDecision::IgnorePressState;
    }

    if !was_held {
        return HoldEventDecision::IgnoreReleaseNotHeld;
    }

    if matches!(pipeline_state, Some(PipelineState::Recording)) {
        return HoldEventDecision::StopRecording;
    }

    HoldEventDecision::IgnoreReleaseState
}

pub(crate) fn handle_hold_shortcut_event(
    app: &AppHandle,
    state: &AppState,
    is_down: bool,
    source: HoldShortcutSource<'_>,
    sound_enabled: bool,
    audio_cue: audio::AudioCue,
    playing_audio_handling: PlayingAudioHandling,
) {
    let audio_mute_manager = app.try_state::<AudioMuteManager>();

    if is_down {
        let was_held = state.ptt_key_held.swap(true, Ordering::SeqCst);
        let pipeline_state = app
            .try_state::<pipeline::SharedPipeline>()
            .map(|pipeline| pipeline.state());
        source.emit_press_diagnostics(app, pipeline_state);

        if matches!(
            classify_hold_event(is_down, was_held, pipeline_state),
            HoldEventDecision::StartRecording
        ) {
            let label = source.label();
            start_recording(
                app,
                state,
                sound_enabled,
                audio_cue,
                &audio_mute_manager,
                playing_audio_handling,
                label.as_ref(),
            );
        }
        return;
    }

    let was_held = state.ptt_key_held.swap(false, Ordering::SeqCst);
    let pipeline_state = if was_held {
        app.try_state::<pipeline::SharedPipeline>()
            .map(|pipeline| pipeline.state())
    } else {
        None
    };

    if matches!(
        classify_hold_event(is_down, was_held, pipeline_state),
        HoldEventDecision::StopRecording
    ) {
        let label = source.label();
        stop_recording(
            app,
            state,
            sound_enabled,
            audio_cue,
            &audio_mute_manager,
            playing_audio_handling,
            label.as_ref(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn global_hold_press_starts_from_idle() {
        assert_eq!(
            classify_hold_event(true, false, Some(PipelineState::Idle)),
            HoldEventDecision::StartRecording
        );
    }

    #[test]
    fn global_hold_repeat_press_is_ignored() {
        assert_eq!(
            classify_hold_event(true, true, Some(PipelineState::Idle)),
            HoldEventDecision::IgnorePressRepeat
        );
    }

    #[test]
    fn global_hold_press_ignores_busy_pipeline_state() {
        assert_eq!(
            classify_hold_event(true, false, Some(PipelineState::Transcribing)),
            HoldEventDecision::IgnorePressState
        );
    }

    #[test]
    fn modifier_hold_release_stops_only_while_recording() {
        assert_eq!(
            classify_hold_event(false, true, Some(PipelineState::Recording)),
            HoldEventDecision::StopRecording
        );
    }

    #[test]
    fn modifier_hold_release_ignores_non_recording_state() {
        assert_eq!(
            classify_hold_event(false, true, Some(PipelineState::Idle)),
            HoldEventDecision::IgnoreReleaseState
        );
    }

    #[test]
    fn modifier_hold_release_without_latch_is_ignored() {
        assert_eq!(
            classify_hold_event(false, false, Some(PipelineState::Recording)),
            HoldEventDecision::IgnoreReleaseNotHeld
        );
    }

    #[test]
    fn modifier_hold_source_label_includes_key_name() {
        let label = HoldShortcutSource::ModifierOnly { key: "AltRight" }.label();
        assert_eq!(label.as_ref(), "Hold(AltRight)");
    }
}
