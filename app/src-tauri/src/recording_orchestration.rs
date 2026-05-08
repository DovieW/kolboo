//! Recording Orchestration helpers for command-facing phase notifications.
//!
//! The pipeline state machine owns real transitions. This Module owns the small
//! watcher loops that translate those transitions into UI events for command
//! flows, keeping the polling policy in one place instead of repeating it in
//! every recording command.

use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter};
use tracing::Instrument;

use crate::events;
use crate::pipeline::{PipelineState, SharedPipeline};
use crate::PipelineStateEvent;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RecordingPhaseWatcherBundle {
    StopAndTranscribe,
    RetryTranscription,
    Dictate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RecordingPhaseWatcherPlan {
    transcription_started: bool,
    routing_started: bool,
    rewriting_started: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PhaseWatchDecision {
    Emit,
    Stop,
    Continue,
}

fn classify_phase_watch_state(state: PipelineState, target: PipelineState) -> PhaseWatchDecision {
    if state == target {
        return PhaseWatchDecision::Emit;
    }

    match state {
        PipelineState::Idle | PipelineState::Error => PhaseWatchDecision::Stop,
        PipelineState::Recording
        | PipelineState::Transcribing
        | PipelineState::Routing
        | PipelineState::Rewriting => PhaseWatchDecision::Continue,
    }
}

fn recording_phase_watcher_plan(bundle: RecordingPhaseWatcherBundle) -> RecordingPhaseWatcherPlan {
    match bundle {
        RecordingPhaseWatcherBundle::StopAndTranscribe => RecordingPhaseWatcherPlan {
            transcription_started: true,
            routing_started: true,
            rewriting_started: true,
        },
        RecordingPhaseWatcherBundle::RetryTranscription => RecordingPhaseWatcherPlan {
            transcription_started: false,
            routing_started: true,
            rewriting_started: true,
        },
        RecordingPhaseWatcherBundle::Dictate => RecordingPhaseWatcherPlan {
            transcription_started: true,
            routing_started: true,
            rewriting_started: false,
        },
    }
}

pub(crate) fn spawn_recording_phase_watchers(
    app: AppHandle,
    pipeline: SharedPipeline,
    bundle: RecordingPhaseWatcherBundle,
) {
    let plan = recording_phase_watcher_plan(bundle);

    if plan.transcription_started {
        spawn_transcription_started_watcher(app.clone(), pipeline.clone());
    }

    if plan.routing_started {
        spawn_routing_started_watcher(app.clone(), pipeline.clone());
    }

    if plan.rewriting_started {
        spawn_rewriting_started_watcher(app, pipeline);
    }
}

pub(crate) fn spawn_transcription_started_watcher(app: AppHandle, pipeline: SharedPipeline) {
    // Keep this watcher intentionally short-lived. The command flows use it specifically to avoid
    // flashing "TRANSCRIBING" when the quiet-audio gate skips STT and returns directly to Idle.
    spawn_phase_started_watcher(
        app,
        pipeline,
        PipelineState::Transcribing,
        events::EVENT_PIPELINE_TRANSCRIPTION_STARTED,
        PipelineStateEvent::Transcribing,
        Duration::from_millis(15),
        Duration::from_secs(2),
    );
}

pub(crate) fn spawn_routing_started_watcher(app: AppHandle, pipeline: SharedPipeline) {
    spawn_phase_started_watcher(
        app,
        pipeline,
        PipelineState::Routing,
        events::EVENT_PIPELINE_ROUTING_STARTED,
        PipelineStateEvent::Routing,
        Duration::from_millis(25),
        Duration::from_secs(15 * 60),
    );
}

pub(crate) fn spawn_rewriting_started_watcher(app: AppHandle, pipeline: SharedPipeline) {
    spawn_phase_started_watcher(
        app,
        pipeline,
        PipelineState::Rewriting,
        events::EVENT_PIPELINE_REWRITING_STARTED,
        PipelineStateEvent::Rewriting,
        Duration::from_millis(50),
        Duration::from_secs(15 * 60),
    );
}

fn spawn_phase_started_watcher(
    app: AppHandle,
    pipeline: SharedPipeline,
    target: PipelineState,
    started_event: &'static str,
    state_event: PipelineStateEvent,
    poll_interval: Duration,
    hard_stop_after: Duration,
) {
    tauri::async_runtime::spawn(
        async move {
            let start = Instant::now();
            loop {
                match classify_phase_watch_state(pipeline.state(), target) {
                    PhaseWatchDecision::Emit => {
                        let _ = app.emit(started_event, ());
                        let _ = app.emit(events::EVENT_PIPELINE_STATE_CHANGED, state_event);
                        break;
                    }
                    PhaseWatchDecision::Stop => break,
                    PhaseWatchDecision::Continue => {}
                }

                // Hard stop to avoid a runaway task in pathological cases. Keep this
                // timeout generous because provider calls can legitimately run for a while.
                if start.elapsed() > hard_stop_after {
                    break;
                }

                tokio::time::sleep(poll_interval).await;
            }
        }
        .in_current_span(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phase_watcher_emits_only_for_the_target_state() {
        assert_eq!(
            classify_phase_watch_state(PipelineState::Routing, PipelineState::Routing),
            PhaseWatchDecision::Emit
        );
        assert_eq!(
            classify_phase_watch_state(PipelineState::Rewriting, PipelineState::Routing),
            PhaseWatchDecision::Continue
        );
    }

    #[test]
    fn phase_watcher_stops_on_terminal_command_states() {
        assert_eq!(
            classify_phase_watch_state(PipelineState::Idle, PipelineState::Routing),
            PhaseWatchDecision::Stop
        );
        assert_eq!(
            classify_phase_watch_state(PipelineState::Error, PipelineState::Rewriting),
            PhaseWatchDecision::Stop
        );
    }

    #[test]
    fn transcription_watcher_waits_while_recording_and_routing() {
        assert_eq!(
            classify_phase_watch_state(PipelineState::Recording, PipelineState::Transcribing),
            PhaseWatchDecision::Continue
        );
        assert_eq!(
            classify_phase_watch_state(PipelineState::Routing, PipelineState::Transcribing),
            PhaseWatchDecision::Continue
        );
    }

    #[test]
    fn watcher_bundle_plans_match_command_flow_expectations() {
        assert_eq!(
            recording_phase_watcher_plan(RecordingPhaseWatcherBundle::StopAndTranscribe),
            RecordingPhaseWatcherPlan {
                transcription_started: true,
                routing_started: true,
                rewriting_started: true,
            }
        );
        assert_eq!(
            recording_phase_watcher_plan(RecordingPhaseWatcherBundle::RetryTranscription),
            RecordingPhaseWatcherPlan {
                transcription_started: false,
                routing_started: true,
                rewriting_started: true,
            }
        );
        assert_eq!(
            recording_phase_watcher_plan(RecordingPhaseWatcherBundle::Dictate),
            RecordingPhaseWatcherPlan {
                transcription_started: true,
                routing_started: true,
                rewriting_started: false,
            }
        );
    }
}
