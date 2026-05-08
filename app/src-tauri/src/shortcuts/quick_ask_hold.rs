//! Quick Ask hold-to-record shortcut flow.
//!
//! This Module keeps the Quick Ask hold-specific start-on-press / stop-on-release rules together
//! for both regular global shortcuts and Windows modifier-only hook events. The main dispatcher
//! still owns action matching, registration stays in `shortcuts/lifecycle.rs`, and low-level hook
//! mechanics stay in `windows_modifier_hotkeys.rs`.

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
pub(crate) enum QuickAskHoldShortcutSource<'a> {
    Global,
    ModifierOnly { key: &'a str, hotkey_debug: bool },
}

impl<'a> QuickAskHoldShortcutSource<'a> {
    fn recording_label(self) -> Cow<'a, str> {
        match self {
            Self::Global => Cow::Borrowed("QuickAskHold"),
            Self::ModifierOnly { key, .. } => Cow::Owned(format!("QuickAskHold({key})")),
        }
    }

    fn hotkey_debug(self) -> bool {
        matches!(
            self,
            Self::ModifierOnly {
                hotkey_debug: true,
                ..
            }
        )
    }

    fn emit_press_diagnostics(self, app: &AppHandle, pipeline_state: Option<PipelineState>) {
        match self {
            Self::Global => {
                log::info!(
                    "QuickAskHold pressed: pipeline state = {:?}",
                    pipeline_state
                );
                emit_system_event(
                    app,
                    "shortcut",
                    "Quick Ask Hold pressed",
                    Some(&format!("Pipeline state: {:?}", pipeline_state)),
                );
            }
            Self::ModifierOnly { .. } if self.hotkey_debug() => {
                let label = self.recording_label();
                emit_system_event(
                    app,
                    "debug",
                    &format!("{} pressed", label),
                    Some(&format!("Pipeline state: {:?}", pipeline_state)),
                );
            }
            Self::ModifierOnly { .. } => {}
        }
    }

    fn emit_busy_diagnostics(self, app: &AppHandle, pipeline_state: Option<PipelineState>) {
        let label = self.recording_label();
        log::info!("{} ignored (pipeline busy: {:?})", label, pipeline_state);

        if self.hotkey_debug() {
            emit_system_event(
                app,
                "debug",
                &format!("{} ignored (pipeline busy)", label),
                Some(&format!("Pipeline state: {:?}", pipeline_state)),
            );
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QuickAskHoldEventDecision {
    IgnorePressRepeat,
    IgnorePressBusy,
    StartRecording,
    IgnorePressState,
    IgnoreReleaseNotHeld,
    StopRecording,
    IgnoreReleaseState,
}

fn classify_quick_ask_hold_event(
    is_down: bool,
    was_held: bool,
    pipeline_state: Option<PipelineState>,
) -> QuickAskHoldEventDecision {
    if is_down {
        if was_held {
            return QuickAskHoldEventDecision::IgnorePressRepeat;
        }

        if matches!(
            pipeline_state,
            Some(PipelineState::Transcribing | PipelineState::Rewriting)
        ) {
            return QuickAskHoldEventDecision::IgnorePressBusy;
        }

        if pipeline_state
            .map(|state| state.can_start_recording())
            .unwrap_or(false)
        {
            return QuickAskHoldEventDecision::StartRecording;
        }

        return QuickAskHoldEventDecision::IgnorePressState;
    }

    if !was_held {
        return QuickAskHoldEventDecision::IgnoreReleaseNotHeld;
    }

    if matches!(pipeline_state, Some(PipelineState::Recording)) {
        return QuickAskHoldEventDecision::StopRecording;
    }

    QuickAskHoldEventDecision::IgnoreReleaseState
}

pub(crate) fn handle_quick_ask_hold_shortcut_event(
    app: &AppHandle,
    state: &AppState,
    is_down: bool,
    source: QuickAskHoldShortcutSource<'_>,
    sound_enabled: bool,
    audio_cue: audio::AudioCue,
    playing_audio_handling: PlayingAudioHandling,
) {
    let audio_mute_manager = app.try_state::<AudioMuteManager>();

    if is_down {
        let was_held = state.quick_ask_key_held.swap(true, Ordering::SeqCst);
        let pipeline_state = app
            .try_state::<pipeline::SharedPipeline>()
            .map(|pipeline| pipeline.state());
        source.emit_press_diagnostics(app, pipeline_state);

        match classify_quick_ask_hold_event(is_down, was_held, pipeline_state) {
            QuickAskHoldEventDecision::StartRecording => {
                state.quick_ask_session_active.store(true, Ordering::SeqCst);
                let label = source.recording_label();
                start_recording(
                    app,
                    state,
                    sound_enabled,
                    audio_cue,
                    &audio_mute_manager,
                    playing_audio_handling,
                    label.as_ref(),
                );

                // Keep the Quick Ask session flag aligned with actual pipeline entry.
                let is_recording = app
                    .try_state::<pipeline::SharedPipeline>()
                    .map(|pipeline| pipeline.state() == PipelineState::Recording)
                    .unwrap_or(false);
                if !is_recording {
                    state
                        .quick_ask_session_active
                        .store(false, Ordering::SeqCst);
                }
            }
            QuickAskHoldEventDecision::IgnorePressBusy => {
                source.emit_busy_diagnostics(app, pipeline_state);
            }
            QuickAskHoldEventDecision::IgnorePressRepeat
            | QuickAskHoldEventDecision::IgnorePressState
            | QuickAskHoldEventDecision::IgnoreReleaseNotHeld
            | QuickAskHoldEventDecision::StopRecording
            | QuickAskHoldEventDecision::IgnoreReleaseState => {}
        }

        return;
    }

    let was_held = state.quick_ask_key_held.swap(false, Ordering::SeqCst);
    let pipeline_state = if was_held {
        app.try_state::<pipeline::SharedPipeline>()
            .map(|pipeline| pipeline.state())
    } else {
        None
    };

    match classify_quick_ask_hold_event(is_down, was_held, pipeline_state) {
        QuickAskHoldEventDecision::StopRecording => {
            let label = source.recording_label();
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
        QuickAskHoldEventDecision::IgnoreReleaseState => {
            // If the pipeline never entered Recording (or already left it), clear the
            // Quick Ask intent so later stop paths don't treat the next session as Quick Ask.
            state
                .quick_ask_session_active
                .store(false, Ordering::SeqCst);
        }
        QuickAskHoldEventDecision::IgnorePressRepeat
        | QuickAskHoldEventDecision::IgnorePressBusy
        | QuickAskHoldEventDecision::StartRecording
        | QuickAskHoldEventDecision::IgnorePressState
        | QuickAskHoldEventDecision::IgnoreReleaseNotHeld => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn global_quick_ask_hold_press_starts_from_idle() {
        assert_eq!(
            classify_quick_ask_hold_event(true, false, Some(PipelineState::Idle)),
            QuickAskHoldEventDecision::StartRecording
        );
    }

    #[test]
    fn global_quick_ask_hold_repeat_press_is_ignored() {
        assert_eq!(
            classify_quick_ask_hold_event(true, true, Some(PipelineState::Idle)),
            QuickAskHoldEventDecision::IgnorePressRepeat
        );
    }

    #[test]
    fn global_quick_ask_hold_press_ignores_busy_pipeline_state() {
        assert_eq!(
            classify_quick_ask_hold_event(true, false, Some(PipelineState::Transcribing)),
            QuickAskHoldEventDecision::IgnorePressBusy
        );
    }

    #[test]
    fn global_quick_ask_hold_press_ignores_non_startable_state() {
        assert_eq!(
            classify_quick_ask_hold_event(true, false, Some(PipelineState::Routing)),
            QuickAskHoldEventDecision::IgnorePressState
        );
    }

    #[test]
    fn quick_ask_hold_release_stops_only_while_recording() {
        assert_eq!(
            classify_quick_ask_hold_event(false, true, Some(PipelineState::Recording)),
            QuickAskHoldEventDecision::StopRecording
        );
    }

    #[test]
    fn quick_ask_hold_release_without_recording_clears_intent() {
        assert_eq!(
            classify_quick_ask_hold_event(false, true, Some(PipelineState::Idle)),
            QuickAskHoldEventDecision::IgnoreReleaseState
        );
    }

    #[test]
    fn quick_ask_hold_release_without_latch_is_ignored() {
        assert_eq!(
            classify_quick_ask_hold_event(false, false, Some(PipelineState::Recording)),
            QuickAskHoldEventDecision::IgnoreReleaseNotHeld
        );
    }

    #[test]
    fn modifier_quick_ask_hold_source_label_includes_key_name() {
        let label = QuickAskHoldShortcutSource::ModifierOnly {
            key: "AltRight",
            hotkey_debug: true,
        }
        .recording_label();

        assert_eq!(label.as_ref(), "QuickAskHold(AltRight)");
    }
}
