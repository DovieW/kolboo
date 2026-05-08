//! Quick Ask toggle shortcut flow.
//!
//! This Module owns the Quick Ask toggle-specific debounce and pipeline-state decision matrix
//! for both global shortcuts and Windows modifier-only hook events. The main dispatcher still
//! owns action matching, registration stays in `shortcuts/lifecycle.rs`, and low-level hook
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
pub(crate) enum QuickAskToggleShortcutSource<'a> {
    Global,
    ModifierOnly {
        key: &'a str,
        suppress_release_actions: bool,
        hotkey_debug: bool,
    },
}

impl<'a> QuickAskToggleShortcutSource<'a> {
    fn recording_label(self) -> Cow<'a, str> {
        match self {
            Self::Global => Cow::Borrowed("QuickAskToggle"),
            Self::ModifierOnly { key, .. } => Cow::Owned(format!("QuickAskToggle({key})")),
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
        match self {
            Self::Global => {
                log::info!(
                    "QuickAskToggle released: pipeline state = {:?}",
                    pipeline_state
                );
                emit_system_event(
                    app,
                    "shortcut",
                    "Quick Ask Toggle released",
                    Some(&format!("Pipeline state: {:?}", pipeline_state)),
                );
            }
            Self::ModifierOnly { .. } if self.hotkey_debug() => {
                let label = self.recording_label();
                emit_system_event(
                    app,
                    "debug",
                    &format!("{} released", label),
                    Some(&format!("Pipeline state: {:?}", pipeline_state)),
                );
            }
            Self::ModifierOnly { .. } => {}
        }
    }

    fn emit_suppressed_release_diagnostics(self, app: &AppHandle) {
        if !self.hotkey_debug() {
            return;
        }

        let label = self.recording_label();
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

        let label = self.recording_label();
        emit_system_event(
            app,
            "debug",
            &format!("{}: key released but was_held=false", label),
            Some("Down event was not observed/latched"),
        );
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

    fn emit_cannot_start_stop_diagnostics(
        self,
        app: &AppHandle,
        pipeline_state: Option<PipelineState>,
    ) {
        let label = self.recording_label();
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

    fn emit_non_quick_ask_session_diagnostics(self, app: &AppHandle) {
        let label = self.recording_label();
        log::info!("{} stop ignored (active session is not Quick Ask)", label);

        if self.hotkey_debug() {
            emit_system_event(
                app,
                "debug",
                &format!("{} stop ignored (active session is not Quick Ask)", label),
                None,
            );
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QuickAskToggleReleaseDecision {
    StartRecording,
    StopRecording,
    IgnoreBusy,
    IgnoreState,
    IgnoreNonQuickAskSession,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QuickAskToggleEventDecision {
    LatchPress,
    IgnoreReleaseNotHeld,
    IgnoreReleaseSuppressed,
    Release(QuickAskToggleReleaseDecision),
}

fn classify_quick_ask_toggle_release(
    pipeline_state: Option<PipelineState>,
    quick_ask_session_active: bool,
) -> QuickAskToggleReleaseDecision {
    if matches!(
        pipeline_state,
        Some(PipelineState::Transcribing | PipelineState::Rewriting)
    ) {
        return QuickAskToggleReleaseDecision::IgnoreBusy;
    }

    if pipeline_state
        .map(|state| state.can_stop_recording())
        .unwrap_or(false)
    {
        return if quick_ask_session_active {
            QuickAskToggleReleaseDecision::StopRecording
        } else {
            QuickAskToggleReleaseDecision::IgnoreNonQuickAskSession
        };
    }

    if pipeline_state
        .map(|state| state.can_start_recording())
        .unwrap_or(false)
    {
        return QuickAskToggleReleaseDecision::StartRecording;
    }

    QuickAskToggleReleaseDecision::IgnoreState
}

fn classify_quick_ask_toggle_event(
    is_down: bool,
    was_held: bool,
    suppress_release_actions: bool,
    pipeline_state: Option<PipelineState>,
    quick_ask_session_active: bool,
) -> QuickAskToggleEventDecision {
    if is_down {
        return QuickAskToggleEventDecision::LatchPress;
    }

    if !was_held {
        return QuickAskToggleEventDecision::IgnoreReleaseNotHeld;
    }

    if suppress_release_actions {
        return QuickAskToggleEventDecision::IgnoreReleaseSuppressed;
    }

    QuickAskToggleEventDecision::Release(classify_quick_ask_toggle_release(
        pipeline_state,
        quick_ask_session_active,
    ))
}

pub(crate) fn handle_quick_ask_toggle_shortcut_event(
    app: &AppHandle,
    state: &AppState,
    is_down: bool,
    source: QuickAskToggleShortcutSource<'_>,
    sound_enabled: bool,
    audio_cue: audio::AudioCue,
    playing_audio_handling: PlayingAudioHandling,
) {
    let audio_mute_manager = app.try_state::<AudioMuteManager>();

    if is_down {
        state.quick_ask_toggle_key_held.swap(true, Ordering::SeqCst);
        return;
    }

    let was_held = state
        .quick_ask_toggle_key_held
        .swap(false, Ordering::SeqCst);
    let pipeline_state = if was_held && !source.suppress_release_actions() {
        app.try_state::<pipeline::SharedPipeline>()
            .map(|pipeline| pipeline.state())
    } else {
        None
    };
    let quick_ask_session_active = state.quick_ask_session_active.load(Ordering::SeqCst);

    match classify_quick_ask_toggle_event(
        is_down,
        was_held,
        source.suppress_release_actions(),
        pipeline_state,
        quick_ask_session_active,
    ) {
        QuickAskToggleEventDecision::LatchPress => {}
        QuickAskToggleEventDecision::IgnoreReleaseNotHeld => {
            source.emit_unlatched_release_diagnostics(app);
        }
        QuickAskToggleEventDecision::IgnoreReleaseSuppressed => {
            source.emit_suppressed_release_diagnostics(app);
        }
        QuickAskToggleEventDecision::Release(release) => {
            source.emit_release_diagnostics(app, pipeline_state);

            match release {
                QuickAskToggleReleaseDecision::StartRecording => {
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
                QuickAskToggleReleaseDecision::StopRecording => {
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
                QuickAskToggleReleaseDecision::IgnoreBusy => {
                    source.emit_busy_diagnostics(app, pipeline_state);
                }
                QuickAskToggleReleaseDecision::IgnoreState => {
                    source.emit_cannot_start_stop_diagnostics(app, pipeline_state);
                }
                QuickAskToggleReleaseDecision::IgnoreNonQuickAskSession => {
                    source.emit_non_quick_ask_session_diagnostics(app);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quick_ask_toggle_press_only_latches() {
        assert_eq!(
            classify_quick_ask_toggle_event(true, false, false, Some(PipelineState::Idle), false),
            QuickAskToggleEventDecision::LatchPress
        );
    }

    #[test]
    fn quick_ask_toggle_release_starts_from_idle() {
        assert_eq!(
            classify_quick_ask_toggle_event(false, true, false, Some(PipelineState::Idle), false),
            QuickAskToggleEventDecision::Release(QuickAskToggleReleaseDecision::StartRecording)
        );
    }

    #[test]
    fn quick_ask_toggle_release_stops_only_for_quick_ask_sessions() {
        assert_eq!(
            classify_quick_ask_toggle_event(
                false,
                true,
                false,
                Some(PipelineState::Recording),
                true,
            ),
            QuickAskToggleEventDecision::Release(QuickAskToggleReleaseDecision::StopRecording)
        );
    }

    #[test]
    fn quick_ask_toggle_release_ignores_non_quick_ask_recording_session() {
        assert_eq!(
            classify_quick_ask_toggle_event(
                false,
                true,
                false,
                Some(PipelineState::Recording),
                false,
            ),
            QuickAskToggleEventDecision::Release(
                QuickAskToggleReleaseDecision::IgnoreNonQuickAskSession,
            )
        );
    }

    #[test]
    fn quick_ask_toggle_release_treats_transcribing_as_busy() {
        assert_eq!(
            classify_quick_ask_toggle_event(
                false,
                true,
                false,
                Some(PipelineState::Transcribing),
                false,
            ),
            QuickAskToggleEventDecision::Release(QuickAskToggleReleaseDecision::IgnoreBusy)
        );
    }

    #[test]
    fn quick_ask_toggle_release_can_be_suppressed() {
        assert_eq!(
            classify_quick_ask_toggle_event(false, true, true, Some(PipelineState::Idle), false),
            QuickAskToggleEventDecision::IgnoreReleaseSuppressed
        );
    }

    #[test]
    fn quick_ask_toggle_release_without_latch_is_ignored() {
        assert_eq!(
            classify_quick_ask_toggle_event(false, false, false, Some(PipelineState::Idle), false),
            QuickAskToggleEventDecision::IgnoreReleaseNotHeld
        );
    }

    #[test]
    fn modifier_quick_ask_toggle_source_label_includes_key_name() {
        let label = QuickAskToggleShortcutSource::ModifierOnly {
            key: "AltRight",
            suppress_release_actions: false,
            hotkey_debug: true,
        }
        .recording_label();

        assert_eq!(label.as_ref(), "QuickAskToggle(AltRight)");
    }
}
