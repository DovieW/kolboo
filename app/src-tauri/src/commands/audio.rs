use crate::audio::{self, AudioCue, SoundType};
use crate::audio_capture;
use crate::events;
use crate::state::{MicTestMeterState, MicTestPipelineRestore};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::thread;
use std::time::Duration;

use tauri::{Emitter, Manager};

#[cfg(desktop)]
use crate::pipeline::{PipelineState, SharedPipeline};

#[cfg(desktop)]
use tokio_util::sync::CancellationToken;

/// Play the selected cue once as a short preview.
///
/// Frontend passes the cue string (e.g. "kolboo"). Unknown values fall back to the default cue.
#[tauri::command]
pub async fn play_audio_cue_preview(cue: String) -> Result<(), String> {
    let cue = AudioCue::from_str(&cue);

    // Preview both sounds so it's obvious which pair will be used during real recording.
    log::info!("Previewing audio cue: {:?} (start then stop)", cue);

    // Run the preview sequence off-thread so we don't block the command handler.
    thread::spawn(move || {
        if let Err(e) = audio::play_sound_blocking(SoundType::RecordingStart, cue) {
            log::warn!("Failed to play preview start sound: {}", e);
            return;
        }

        // A small deliberate gap so users can clearly distinguish start vs stop.
        thread::sleep(Duration::from_millis(140));

        if let Err(e) = audio::play_sound_blocking(SoundType::RecordingStop, cue) {
            log::warn!("Failed to play preview stop sound: {}", e);
        }
    });

    Ok(())
}

/// List available audio input devices as seen by the backend (CPAL).
///
/// This is the authoritative device list for recording and the backend-driven overlay waveform.
#[tauri::command]
pub fn list_audio_input_devices() -> Vec<String> {
    audio_capture::list_input_devices()
}

/// List available audio input devices as seen by the backend (CPAL), with unique IDs.
///
/// Use this from the frontend when building Select/Combobox options.
#[tauri::command]
pub fn list_audio_input_devices_v2() -> Vec<audio_capture::AudioInputDeviceInfo> {
    audio_capture::list_input_devices_v2()
}

/// Get the backend default audio input device name (CPAL default), if available.
#[tauri::command]
pub fn get_default_audio_input_device_name() -> Option<String> {
    audio_capture::get_default_input_device_info().map(|(name, _sr, _ch)| name)
}

#[derive(Debug, Deserialize)]
pub struct MicTestStartArgs {
    /// Selected mic identifier from settings.
    ///
    /// Accept both snake_case and common camelCase keys.
    #[serde(default, alias = "inputDeviceId", alias = "micId")]
    pub input_device_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct MicTestAudioLevelPayload {
    pub active: bool,
    pub session_id: u64,
    pub seq: u64,
    pub rms: f32,
    pub peak: f32,
}

/// Start a realtime microphone level meter for the Settings UI.
///
/// This opens a backend CPAL input stream for the selected device and publishes
/// peak/RMS levels as `mic-test-audio-level` events.
#[tauri::command]
pub async fn mic_test_start_meter(
    app: tauri::AppHandle,
    state: tauri::State<'_, MicTestMeterState>,
    args: MicTestStartArgs,
) -> Result<(), String> {
    #[cfg(not(desktop))]
    {
        let _ = app;
        let _ = state;
        let _ = args;
        return Err("Mic test is not supported on this platform.".to_string());
    }

    #[cfg(desktop)]
    {
        // Avoid competing capture streams while actively recording.
        let Some(pipeline) = app.try_state::<SharedPipeline>() else {
            return Err("Pipeline is not initialized yet.".to_string());
        };
        if (*pipeline).try_state() == Some(PipelineState::Recording) {
            return Err("Cannot test microphone level while recording.".to_string());
        }

        let mut guard = state
            .inner
            .lock()
            .map_err(|e| format!("Mic test state lock error: {e}"))?;

        // Stop any existing publisher loop.
        // IMPORTANT: do NOT restore capture behavior here; mic switching uses repeated start calls.
        if let Some(token) = guard.cancel.take() {
            token.cancel();
        }

        // Bump session id so the frontend can ignore stale stop events from the previous loop.
        guard.session_id = guard.session_id.saturating_add(1);
        let session_id = guard.session_id;

        // Snapshot current pipeline capture behavior so we can restore on stop.
        // Only capture once (first start). During restarts we keep the original restore values.
        let cfg = (*pipeline).config();
        if guard.restore.is_none() {
            guard.restore = Some(MicTestPipelineRestore {
                hot_mic_enabled: cfg.hot_mic_enabled,
                hot_mic_pre_roll_ms: cfg.hot_mic_pre_roll_ms,
                mic_auto_recover_enabled: cfg.mic_auto_recover_enabled,
                input_device_name: cfg.input_device_name.clone(),
            });
        }

        // Ensure there is an active CPAL stream (meter updates require callbacks).
        // We enable Hot Mic temporarily so the stream remains open while the user speaks.
        (*pipeline)
            .set_capture_behavior_override(
                true,
                cfg.hot_mic_pre_roll_ms,
                cfg.mic_auto_recover_enabled,
                args.input_device_id.as_deref(),
            )
            .map_err(|e| format!("Failed to start mic test: {e}"))?;
        let cancel = CancellationToken::new();
        let cancel_for_task = cancel.clone();
        let app_handle = app.clone();
        let pipeline_for_task = (*pipeline).clone();

        // Prime UI immediately so it can un-grey the meter.
        let _ = app_handle.emit(
            events::EVENT_MIC_TEST_AUDIO_LEVEL,
            MicTestAudioLevelPayload {
                active: true,
                session_id,
                seq: 0,
                rms: 0.0,
                peak: 0.0,
            },
        );

        tauri::async_runtime::spawn(async move {
            use std::time::Instant;
            use tokio::time;

            let mut last_seq: u64 = 0;
            let mut last_emit = Instant::now();

            loop {
                if cancel_for_task.is_cancelled() {
                    break;
                }

                // 30Hz is plenty for a simple level bar.
                time::sleep(Duration::from_millis(33)).await;

                let s = pipeline_for_task.audio_level_snapshot_fast();
                if s.seq == last_seq {
                    continue;
                }
                last_seq = s.seq;

                // Defensive throttle (avoid accidental busy loops).
                if last_emit.elapsed() < Duration::from_millis(10) {
                    continue;
                }
                last_emit = Instant::now();

                let _ = app_handle.emit(
                    events::EVENT_MIC_TEST_AUDIO_LEVEL,
                    MicTestAudioLevelPayload {
                        active: true,
                        session_id,
                        seq: s.seq,
                        rms: s.rms,
                        peak: s.peak,
                    },
                );
            }

            // Tell the UI to grey out.
            let _ = app_handle.emit(
                events::EVENT_MIC_TEST_AUDIO_LEVEL,
                MicTestAudioLevelPayload {
                    active: false,
                    session_id,
                    seq: last_seq,
                    rms: 0.0,
                    peak: 0.0,
                },
            );
        });

        guard.cancel = Some(cancel);
        Ok(())
    }
}

/// Stop the realtime microphone test meter (if running).
#[tauri::command]
pub async fn mic_test_stop_meter(
    app: tauri::AppHandle,
    state: tauri::State<'_, MicTestMeterState>,
) -> Result<(), String> {
    #[cfg(not(desktop))]
    {
        let _ = app;
        let _ = state;
        Ok(())
    }

    #[cfg(desktop)]
    {
        let pipeline = app.try_state::<SharedPipeline>();
        let mut guard = state
            .inner
            .lock()
            .map_err(|e| format!("Mic test state lock error: {e}"))?;

        let session_id = guard.session_id;

        if let Some(token) = guard.cancel.take() {
            token.cancel();
        }

        // Restore any previous capture behavior we temporarily changed.
        if let (Some(p), Some(prev)) = (pipeline, guard.restore.take()) {
            let _ = (*p).set_capture_behavior_override(
                prev.hot_mic_enabled,
                prev.hot_mic_pre_roll_ms,
                prev.mic_auto_recover_enabled,
                prev.input_device_name.as_deref(),
            );
        }

        // Best-effort: ensure UI returns to greyed state immediately.
        let _ = app.emit(
            events::EVENT_MIC_TEST_AUDIO_LEVEL,
            MicTestAudioLevelPayload {
                active: false,
                session_id,
                seq: 0,
                rms: 0.0,
                peak: 0.0,
            },
        );

        Ok(())
    }
}
