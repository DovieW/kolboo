//! Toggle-recording shortcut flow.
//!
//! This Module owns the toggle-specific debounce + pipeline-state decision matrix for both
//! regular global shortcuts and Windows modifier-only hook events. We keep the Tauri side effects
//! visible here, but the main dispatcher still decides *which* action was matched, registration
//! stays in `shortcuts/lifecycle.rs`, and Windows hook mechanics stay in
//! `windows_modifier_hotkeys.rs`.

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
pub(crate) enum ToggleShortcutSource<'a> {
    Global,
    ModifierOnly {
        key: &'a str,
        suppress_release_actions: bool,
        hotkey_debug: bool,
    },
}

impl<'a> ToggleShortcutSource<'a> {
    fn label(self) -> Cow<'a, str> {
        match self {
            Self::Global => Cow::Borrowed("Toggle"),
            Self::ModifierOnly { key, .. } => Cow::Owned(format!("Toggle({key})")),
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

    fn hotkey_debug(self) -> bool {
        matches!(
            self,
            Self::ModifierOnly {
                hotkey_debug: true,
                ..
            }
        )
    }

    fn emit_release_diagnostics(self, app: &AppHandle, pipeline_state: Option<PipelineState>) {
        let details = format!("Pipeline state: {:?}", pipeline_state);

        match self {
            Self::Global => {
                log::info!("Toggle released: pipeline state = {:?}", pipeline_state);
                emit_system_event(app, "shortcut", "Toggle key released", Some(&details));
            }
            Self::ModifierOnly { .. } if self.hotkey_debug() => {
                let label = self.label();
                emit_system_event(
                    app,
                    "debug",
                    &format!("{}: key released", label),
                    Some(&details),
                );
            }
            Self::ModifierOnly { .. } => {}
        }
    }

    fn emit_suppressed_release_diagnostics(self, app: &AppHandle) {
        if !self.hotkey_debug() {
            return;
        }

        let label = self.label();
        emit_system_event(
            app,
            "debug",
            &format!("{}: release suppressed", label),
            Some("AltGr/typing suppression triggered"),
        );
    }

    fn emit_unlatched_release_diagnostics(self, app: &AppHandle) {
        if !self.hotkey_debug() {
            return;
        }

        let label = self.label();
        emit_system_event(
            app,
            "debug",
            &format!("{}: key released but was_held=false", label),
            Some("Down event was not observed/latched"),
        );
    }

    fn emit_busy_diagnostics(self, app: &AppHandle, pipeline_state: Option<PipelineState>) {
        let label = self.label();
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

    fn emit_cannot_start_stop_diagnostics(
        self,
        app: &AppHandle,
        pipeline_state: Option<PipelineState>,
    ) {
        let label = self.label();
        log::info!("{} ignored (pipeline state: {:?})", label, pipeline_state);

        if self.hotkey_debug() {
            emit_system_event(
                app,
                "debug",
                &format!("{} ignored (cannot start/stop)", label),
                Some(&format!("Pipeline state: {:?}", pipeline_state)),
            );
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ToggleReleaseDecision {
    StartRecording,
    StopRecording,
    IgnoreBusy,
    IgnoreState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ToggleEventDecision {
    LatchPress,
    IgnoreReleaseNotHeld,
    IgnoreReleaseSuppressed,
    Release(ToggleReleaseDecision),
}

fn classify_toggle_release(pipeline_state: Option<PipelineState>) -> ToggleReleaseDecision {
    if matches!(
        pipeline_state,
        Some(PipelineState::Transcribing | PipelineState::Rewriting)
    ) {
        return ToggleReleaseDecision::IgnoreBusy;
    }

    if pipeline_state
        .map(|state| state.can_stop_recording())
        .unwrap_or(false)
    {
        return ToggleReleaseDecision::StopRecording;
    }

    if pipeline_state
        .map(|state| state.can_start_recording())
        .unwrap_or(false)
    {
        return ToggleReleaseDecision::StartRecording;
    }

    ToggleReleaseDecision::IgnoreState
}

fn classify_toggle_event(
    is_down: bool,
    was_held: bool,
    suppress_release_actions: bool,
    pipeline_state: Option<PipelineState>,
) -> ToggleEventDecision {
    if is_down {
        return ToggleEventDecision::LatchPress;
    }

    if !was_held {
        return ToggleEventDecision::IgnoreReleaseNotHeld;
    }

    if suppress_release_actions {
        return ToggleEventDecision::IgnoreReleaseSuppressed;
    }

    ToggleEventDecision::Release(classify_toggle_release(pipeline_state))
}

pub(crate) fn handle_toggle_shortcut_event(
    app: &AppHandle,
    state: &AppState,
    is_down: bool,
    source: ToggleShortcutSource<'_>,
    sound_enabled: bool,
    audio_cue: audio::AudioCue,
    playing_audio_handling: PlayingAudioHandling,
) {
    let audio_mute_manager = app.try_state::<AudioMuteManager>();

    if is_down {
        state.toggle_key_held.swap(true, Ordering::SeqCst);
        return;
    }

    let was_held = state.toggle_key_held.swap(false, Ordering::SeqCst);
    let pipeline_state = if was_held && !source.suppress_release_actions() {
        app.try_state::<pipeline::SharedPipeline>()
            .map(|pipeline| pipeline.state())
    } else {
        None
    };

    match classify_toggle_event(
        is_down,
        was_held,
        source.suppress_release_actions(),
        pipeline_state,
    ) {
        ToggleEventDecision::LatchPress => {}
        ToggleEventDecision::IgnoreReleaseNotHeld => {
            source.emit_unlatched_release_diagnostics(app);
        }
        ToggleEventDecision::IgnoreReleaseSuppressed => {
            source.emit_suppressed_release_diagnostics(app);
        }
        ToggleEventDecision::Release(release) => {
            source.emit_release_diagnostics(app, pipeline_state);

            match release {
                ToggleReleaseDecision::StartRecording => {
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
                ToggleReleaseDecision::StopRecording => {
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
                ToggleReleaseDecision::IgnoreBusy => {
                    source.emit_busy_diagnostics(app, pipeline_state);
                }
                ToggleReleaseDecision::IgnoreState => {
                    source.emit_cannot_start_stop_diagnostics(app, pipeline_state);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn global_toggle_press_only_latches() {
        assert_eq!(
            classify_toggle_event(true, false, false, Some(PipelineState::Idle)),
            ToggleEventDecision::LatchPress
        );
    }

    #[test]
    fn global_toggle_release_starts_from_idle() {
        assert_eq!(
            classify_toggle_event(false, true, false, Some(PipelineState::Idle)),
            ToggleEventDecision::Release(ToggleReleaseDecision::StartRecording)
        );
    }

    #[test]
    fn global_toggle_release_stops_from_recording() {
        assert_eq!(
            classify_toggle_event(false, true, false, Some(PipelineState::Recording)),
            ToggleEventDecision::Release(ToggleReleaseDecision::StopRecording)
        );
    }

    #[test]
    fn global_toggle_release_treats_transcribing_as_busy() {
        assert_eq!(
            classify_toggle_event(false, true, false, Some(PipelineState::Transcribing)),
            ToggleEventDecision::Release(ToggleReleaseDecision::IgnoreBusy)
        );
    }

    #[test]
    fn global_toggle_release_treats_routing_as_non_startable_state() {
        assert_eq!(
            classify_toggle_event(false, true, false, Some(PipelineState::Routing)),
            ToggleEventDecision::Release(ToggleReleaseDecision::IgnoreState)
        );
    }

    #[test]
    fn modifier_toggle_release_can_be_suppressed() {
        assert_eq!(
            classify_toggle_event(false, true, true, Some(PipelineState::Idle)),
            ToggleEventDecision::IgnoreReleaseSuppressed
        );
    }

    #[test]
    fn modifier_toggle_release_without_latch_is_ignored() {
        assert_eq!(
            classify_toggle_event(false, false, false, Some(PipelineState::Idle)),
            ToggleEventDecision::IgnoreReleaseNotHeld
        );
    }

    #[test]
    fn modifier_toggle_source_label_includes_key_name() {
        let label = ToggleShortcutSource::ModifierOnly {
            key: "AltRight",
            suppress_release_actions: false,
            hotkey_debug: true,
        }
        .label();

        assert_eq!(label.as_ref(), "Toggle(AltRight)");
    }
}
