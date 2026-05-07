//! Retry-last-recording shortcut flow.
//!
//! Keep this focused on the Retry hotkey. Shortcut registration decisions stay in
//! `shortcuts/lifecycle.rs`, Windows modifier-hook mechanics stay in
//! `windows_modifier_hotkeys.rs`, and the main dispatch file only decides when a
//! Retry action was requested.

use tauri::{AppHandle, Manager};

use crate::commands;
use crate::emit_system_event;
use crate::history::{HistoryEntry, HistoryStorage};
use crate::pipeline;
use crate::recordings::RecordingStore;

#[derive(Debug, Clone, Copy)]
struct RetryCandidate<'a> {
    entry_id: &'a str,
    recording_request_id: Option<&'a str>,
}

fn resolve_retryable_history_entry_id<'a>(
    entries: impl IntoIterator<Item = RetryCandidate<'a>>,
    has_recording: impl Fn(&str) -> bool,
) -> Option<String> {
    for entry in entries {
        // Prefer an explicit recording pointer (covers reruns), but fall back to
        // legacy storage where the WAV is stored under the entry id.
        let candidate_ids = [entry.recording_request_id, Some(entry.entry_id)];

        if candidate_ids
            .iter()
            .flatten()
            .map(|rid| rid.trim())
            .any(&has_recording)
        {
            return Some(entry.entry_id.to_string());
        }
    }

    None
}

fn history_entry_to_retry_candidate(entry: &HistoryEntry) -> RetryCandidate<'_> {
    RetryCandidate {
        entry_id: entry.id.as_str(),
        recording_request_id: entry.recording_request_id.as_deref(),
    }
}

/// Resolve the most recent history entry id that has a persisted recording available.
///
/// This is used by the Retry hotkey to pick "the last recording".
fn resolve_last_recording_history_entry_id(app: &AppHandle) -> Option<String> {
    let history = app.try_state::<HistoryStorage>()?;
    let store = app.try_state::<RecordingStore>()?;

    // Be conservative on work done inside shortcut-triggered paths.
    let entries = history.get_all(Some(50)).ok()?;
    resolve_retryable_history_entry_id(
        entries.iter().map(history_entry_to_retry_candidate),
        |rid| store.has(rid),
    )
}

/// Retry the last available recording and output the result.
///
/// Intended for use by the global Retry hotkey (so it shows the overlay loading state
/// even when the overlay is normally hidden).
pub(crate) fn spawn_retry_last_recording_and_output(app: &AppHandle, source: &str) {
    let app = app.clone();
    let source = source.to_string();

    tauri::async_runtime::spawn(async move {
        let Some(pipeline) = app.try_state::<pipeline::SharedPipeline>() else {
            log::warn!("{source}: pipeline not available; cannot retry");
            return;
        };
        let pipeline = (*pipeline).clone();

        let pipeline_state = pipeline.state();
        if !matches!(
            pipeline_state,
            pipeline::PipelineState::Idle | pipeline::PipelineState::Error
        ) {
            log::info!(
                "{source}: retry ignored (pipeline busy: {:?})",
                pipeline_state
            );
            return;
        }

        let Some(history_entry_id) = resolve_last_recording_history_entry_id(&app) else {
            log::info!("{source}: no recording available to retry");
            emit_system_event(&app, "shortcut", "Retry: no recording available", None);
            return;
        };

        // Force-show overlay so the user gets the loading state UX.
        if let Err(e) = commands::overlay::show_overlay_with_reset_if_not_always(&app) {
            log::warn!("{source}: failed to show overlay for retry: {}", e);
        }

        let transcript = match commands::recording::pipeline_retry_transcription_impl(
            app.clone(),
            pipeline.clone(),
            history_entry_id,
        )
        .await
        {
            Ok(t) => t,
            Err(e) => {
                log::warn!("{source}: retry failed: {}", e.message);
                return;
            }
        };

        let Some(text) = crate::sanitize_transcript(&transcript) else {
            log::info!("{source}: retry returned empty transcript; nothing to output");
            return;
        };

        let output_intent =
            crate::core::output_settings::resolve_output_intent_from_store(&app, None, None);

        if let Err(e) = commands::text::output_text_with_mode_options(
            &text,
            output_intent.mode(),
            output_intent.hit_enter(),
            !output_intent.clipboard_privacy_mode(),
        ) {
            log::error!("{source}: failed to output retry transcript: {}", e);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn candidate<'a>(
        entry_id: &'a str,
        recording_request_id: Option<&'a str>,
    ) -> RetryCandidate<'a> {
        RetryCandidate {
            entry_id,
            recording_request_id,
        }
    }

    #[test]
    fn retry_resolution_prefers_first_entry_with_explicit_recording_pointer() {
        let available = HashSet::from(["source-2"]);
        let resolved = resolve_retryable_history_entry_id(
            [
                candidate("entry-1", Some("missing-source")),
                candidate("entry-2", Some("source-2")),
                candidate("entry-3", Some("source-3")),
            ],
            |rid| available.contains(rid),
        );

        assert_eq!(resolved.as_deref(), Some("entry-2"));
    }

    #[test]
    fn retry_resolution_falls_back_to_legacy_entry_id_recording() {
        let available = HashSet::from(["entry-2"]);
        let resolved = resolve_retryable_history_entry_id(
            [
                candidate("entry-1", Some("missing-source")),
                candidate("entry-2", None),
            ],
            |rid| available.contains(rid),
        );

        assert_eq!(resolved.as_deref(), Some("entry-2"));
    }

    #[test]
    fn retry_resolution_trims_recording_ids_before_lookup() {
        let available = HashSet::from(["source-1"]);
        let resolved =
            resolve_retryable_history_entry_id([candidate("entry-1", Some(" source-1 "))], |rid| {
                available.contains(rid)
            });

        assert_eq!(resolved.as_deref(), Some("entry-1"));
    }
}
