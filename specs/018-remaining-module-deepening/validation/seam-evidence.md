# Seam Evidence

## Telemetry Mapping

- Interface: `telemetry::cost_inputs_from_request_log(...)`.
- Implementation: maps rich `RequestLog` variants into compact `CostTelemetryInputs`.
- Depth: Stats no longer knows Quick Ask / Quick Replace field priority rules.
- Deletion test: removing the Module pushes request-kind branching back into `stats.rs` and any future telemetry consumers.
- Characterization: unit tests in `telemetry.rs` cover normal transcription, Quick Ask precedence, and Quick Replace fallback.

## Cost Reporting

- Interface: `cost::reporting::{report_stt_cost, report_llm_cost}`.
- Implementations/Adapters: OpenAI and Groq are covered by shared report tests; additional provider branches remain provider-specific and table/formula-owned.
- Depth: parsing provider response telemetry and choosing event-level estimates moved out of `stats.rs`.
- Deletion test: removing the Module reintroduces a long provider-specific match in `stats.rs` for every stats event emission path.
- Characterization: unit tests in `cost/reporting.rs` cover OpenAI STT duration fallback, Groq STT duration pricing, OpenAI/Groq LLM usage mapping with distinct rates, and Anthropic cache token mapping.

## Embeddings Module

- Interface: `EmbeddingsProvider` plus `embeddings::build_provider(...)` and input-role helpers.
- Implementations/Adapters: OpenAI, Cohere, and Fireworks concrete providers.
- Depth: production routing now calls through the Interface rather than direct provider modules.
- Deletion test: removing the Module helpers pushes provider default/model/input-type logic and direct HTTP calls back into `pipeline/routing.rs`.
- Characterization: routing tests cover shared cache-key use and Cohere document input-role diagnostics.

## STT provider construction locality

- Interface: `stt_cloud_adapters::adapter_for(...).build(...)`.
- Implementations/Adapters: OpenAI and Groq construction are characterized without network calls; registry includes all cloud STT providers.
- Depth: `stt_provider.rs` validates shared preconditions and delegates constructor quirks.
- Deletion test: removing the adapter registry restores the large constructor match and provider-specific wiring in the factory.
- Characterization: adapter registry tests verify expected IDs and two concrete provider constructions.

## WebSocket transport policy

- Interface: `stt::websocket_transport::connect_ws_with_transport_policy(...)` plus the compatibility wrapper `stt::streaming::connect_ws_split_with_timeout(...)`.
- Implementations/Adapters: OpenAI Realtime, Deepgram, ElevenLabs, Speechmatics, AssemblyAI, and Fireworks now pass provider-owned `ProxySettings` into the shared transport path while keeping provider-specific headers, URLs, and protocol messages in their adapters.
- Depth: manual proxy CONNECT tunnelling, manual `no_proxy` bypass, and trusted-CA / invalid-cert TLS overrides moved out of provider adapters; `stt/streaming.rs` stays focused on provider-independent session lifecycle helpers.
- Deletion test: removing the Module pushes manual proxy/tunnel/TLS policy back into every realtime STT adapter or reintroduces the old silent gap where WS ignored configured transport settings.
- Characterization: unit tests in `stt/websocket_transport.rs` cover exact/suffix/port and wildcard `no_proxy` host matching, the remaining unsupported HTTPS-proxy diagnostic, and local loopback manual HTTP proxy CONNECT request shape with Basic auth.

## OpenAI realtime STT provider-local module

- Interface: `stt::openai::realtime::{supports_realtime_streaming, realtime_transcription_model, realtime_ws_url, start_streaming_session}` called from `stt/openai.rs`.
- Implementation: owns OpenAI-specific realtime WebSocket URL/auth/session-update payloads, server-event parsing, partial transcript accumulation, and post-commit finalization heuristics.
- Depth: `stt/openai.rs` stays focused on batch endpoint selection, prompt/language normalization, and Responses/transcriptions HTTP flows while provider-independent WS/session helpers remain in `stt/streaming.rs`.
- Deletion test: removing the Module restores the long OpenAI realtime protocol state machine to `stt/openai.rs`, mixing batch-path selection with provider-specific WebSocket event handling again.
- Characterization: unit tests in `stt/openai/realtime.rs` cover delta/completed/error event parsing, transcript accumulation/finalization, and realtime model/URL conventions with synthetic JSON payloads only.

## Deepgram realtime STT provider-local module

- Interface: `stt::deepgram::realtime::{streaming_ws_url, start_streaming_session}` called from `stt/deepgram.rs`.
- Implementation: owns Deepgram-specific realtime WebSocket URL/auth setup, `Results` / `Metadata` / `Error` event parsing, transcript accumulation, and finalize/close behavior.
- Depth: `stt/deepgram.rs` stays focused on batch `/v1/listen` request construction, model/language normalization, and `SttProvider` trait wiring while provider-independent WS/session helpers remain in `stt/streaming.rs`.
- Deletion test: removing the Module restores the long Deepgram realtime protocol state machine to `stt/deepgram.rs`, mixing batch-path HTTP behavior with provider-specific WebSocket event handling again.
- Characterization: unit tests in `stt/deepgram/realtime.rs` cover URL shaping, realtime event parsing, transcript accumulation/finalization, and empty/whitespace segment handling with synthetic JSON payloads only.

## ElevenLabs realtime STT provider-local module

- Interface: `stt::elevenlabs::realtime::{speech_to_text_realtime_ws_url, transcribe_realtime_ws, start_streaming_session}` called from `stt/elevenlabs.rs`.
- Implementation: owns ElevenLabs-specific realtime WS URL/query construction, buffered realtime transcription flow, concurrent streaming session flow, VAD/manual commit semantics, PCM/WAV decode for the realtime batch path, and `partial_transcript` / `committed_transcript` event parsing.
- Depth: `stt/elevenlabs.rs` stays focused on provider construction, model/language normalization, HTTP multipart fallback, and `SttProvider` trait wiring while provider-independent WS/session lifecycle helpers remain in `stt/streaming.rs`.
- Deletion test: removing the Module restores both ElevenLabs realtime protocol loops to `stt/elevenlabs.rs`, mixing legacy HTTP fallback selection with provider-specific WebSocket URL/auth logic, VAD/manual commit behavior, and transcript accumulation again.
- Characterization: unit tests in `stt/elevenlabs/realtime.rs` cover WS URL shaping, event parsing, partial/committed transcript accumulation, and empty/whitespace segment joining with synthetic payloads only.

## AssemblyAI realtime STT provider-local module

- Interface: `stt::assemblyai::realtime::{streaming_ws_url, start_streaming_session}` called from `stt/assemblyai.rs`.
- Implementation: owns AssemblyAI-specific realtime WS URL/query construction, `Begin` / `Turn` / `Termination` event parsing, turn accumulation/finalization, and `Terminate` shutdown behavior.
- Depth: `stt/assemblyai.rs` stays focused on batch upload/submit/poll request construction, language normalization, and `SttProvider` trait wiring while provider-independent WS/session lifecycle helpers remain in `stt/streaming.rs`.
- Deletion test: removing the Module restores the long AssemblyAI realtime protocol loop to `stt/assemblyai.rs`, mixing batch workflow ownership with provider-specific WebSocket URL/auth logic, `Turn` semantics, and transcript accumulation again.
- Characterization: unit tests in `stt/assemblyai/realtime.rs` cover WS URL shaping, event parsing, transcript accumulation/finalization, and empty/whitespace turn joining with synthetic payloads only.

## Speechmatics realtime STT provider-local module

- Interface: `stt::speechmatics::realtime::{transcribe_ws, start_streaming_session}` called from `stt/speechmatics.rs`.
- Implementation: owns Speechmatics-specific batch-via-WS and concurrent streaming startup/task flows, `RecognitionStarted` / `AudioAdded` / transcript event parsing, and sentence-level transcript accumulation based on `is_eos`.
- Depth: `stt/speechmatics.rs` stays focused on provider construction, WAV/PCM decode helpers, language normalization, and `SttProvider` trait wiring while provider-independent WS/session lifecycle helpers remain in `stt/streaming.rs`.
- Deletion test: removing the Module restores both Speechmatics WS protocol loops to `stt/speechmatics.rs`, mixing provider construction and decode helpers with provider-specific startup, transcript parsing, and sentence-boundary accumulation again.
- Characterization: unit tests in `stt/speechmatics/realtime.rs` cover event parsing, sentence-commit accumulation/finalization, and empty/whitespace segment joining with synthetic payloads only.

## Fireworks realtime STT provider-local module

- Interface: `stt::fireworks::realtime::{streaming_ws_url, start_streaming_session}` called from `stt/fireworks.rs`.
- Implementation: owns Fireworks-specific streaming WS URL/auth setup, 16 kHz resampling send loop, segment/checkpoint parsing, segment-id parsing, and stability/age commit heuristics.
- Depth: `stt/fireworks.rs` stays focused on provider construction, batch OpenAI-compatible multipart transcription, model/language normalization, and `SttProvider` trait wiring while provider-independent WS/session lifecycle helpers remain in `stt/streaming.rs`.
- Deletion test: removing the Module restores the Fireworks streaming protocol loop to `stt/fireworks.rs`, mixing batch multipart behavior with provider-specific checkpoint handling and stability heuristics again.
- Characterization: unit tests in `stt/fireworks/realtime.rs` cover WS URL shaping, checkpoint/error parsing, segment-id parsing, stability-based commit behavior, and empty/whitespace segment joining with synthetic payloads only.

## Recording Orchestration

- Interface: `recording_orchestration::{spawn_routing_started_watcher, spawn_rewriting_started_watcher}`.
- Implementation: command-facing phase watchers emit UI events when the pipeline reaches a target state.
- Depth: duplicate watcher loops moved out of `commands/recording.rs`; the state machine still owns transitions.
- Deletion test: removing the Module duplicates polling intervals, terminal-state handling, hard-stop timeout, and event emission in every recording command flow.
- Characterization: pure classification tests cover target, waiting, and terminal states.

## Audio Capture Internals

- Interface: `audio_capture::{list_input_devices_v2, get_default_input_device_info, AudioCaptureBackend}` plus internal `audio_capture::{device_selection, preprocessing, meters}` Modules.
- Implementation: device-selection tokens/read models, stop-time preprocessing helpers, and realtime meter snapshots are split from CPAL stream/runtime orchestration.
- Depth: `audio_capture.rs` no longer owns mic ID encoding/ordinal matching, capture cleanup algorithms, RMS/peak/waveform meter storage, or speech-presence helpers while the external capture Interface stays unchanged.
- Deletion test: removing these Modules pushes duplicate-name mic selection, noise-gate/high-pass/AGC/noise-suppression logic, and atomic meter state back into the large CPAL runtime file.
- Characterization: unit tests in `audio_capture/device_selection.rs`, `audio_capture/preprocessing.rs`, and `audio_capture/meters.rs` cover encoded mic IDs, ordinal/legacy selection, gate-strength mapping, preprocessing invariants, meter clamping, waveform bucket shape, and empty speech detection. Additional runtime characterization in `audio_capture.rs` covers buffer format/reset transitions, rolling pre-roll resizing, hot-mic start/stop cleanup, VAD event polling, drop cleanup, and watchdog backoff without real devices.

## Retry Last Shortcut

- Interface: `shortcuts::retry_last::spawn_retry_last_recording_and_output(...)` re-exported through `shortcuts::spawn_retry_last_recording_and_output(...)` for existing dispatch call sites.
- Implementation: resolves the most recent history entry with a persisted recording, starts retry transcription, force-shows overlay loading UX, sanitizes transcript output, and emits no-recording feedback.
- Depth: retry-specific history/recording lookup and output orchestration moved out of generic shortcut dispatch while registration decisions and Windows hook mechanics remain in their existing Modules.
- Deletion test: removing the Module restores retryable-history selection and retry output side effects to both global-shortcut and modifier-only dispatch branches.
- Characterization: unit tests in `shortcuts/retry_last.rs` cover explicit recording-source preference, legacy entry-id fallback, and trimmed recording ids.

## Shortcut Recording Actions

- Interface: `shortcuts::{toggle_recording::handle_toggle_shortcut_event, hold_recording::handle_hold_shortcut_event, paste_last::handle_paste_last_shortcut_event}` called from `shortcuts/mod.rs` after action matching.
- Implementation: owns Toggle/Hold/Paste Last debounce, pipeline busy/start/stop decisions, and last-transcription output across both global-shortcut and modifier-only hook paths.
- Depth: duplicated recording/output action branches moved out of generic shortcut dispatch while registration decisions, Windows hook mechanics, Retry Last, and Escape cancel remain separate.
- Deletion test: removing these Modules restores duplicated toggle/hold/paste-last release handling, pipeline-state guards, and source-label/output logic in both global-shortcut and modifier-only branches.
- Characterization: unit tests in `shortcuts/toggle_recording.rs`, `shortcuts/hold_recording.rs`, and `shortcuts/paste_last.rs` cover global and modifier-only release gating, start/stop decisions, suppressed modifier releases, and source-label shaping.

## Quick Ask Shortcut Actions

- Interface: `shortcuts::{quick_ask_hold::handle_quick_ask_hold_shortcut_event, quick_ask_toggle::handle_quick_ask_toggle_shortcut_event}` called from `shortcuts/mod.rs` after action matching.
- Implementation: owns Quick Ask hold/toggle press/release debounce, pipeline busy/start/stop decisions, Quick Ask session-intent guarding, and source-label diagnostics across both global-shortcut and modifier-only hook paths.
- Depth: duplicated Quick Ask recording branches moved out of generic shortcut dispatch while registration decisions, Windows hook mechanics, Retry Last, and Escape cancel remain separate.
- Deletion test: removing these Modules restores duplicated Quick Ask hold/toggle release handling, busy-state guards, Quick Ask session checks, and source-label logic in both global-shortcut and modifier-only branches.
- Characterization: unit tests in `shortcuts/quick_ask_hold.rs` and `shortcuts/quick_ask_toggle.rs` cover busy states, unlatched releases, suppressed modifier releases, non-Quick-Ask stop suppression, and source-label shaping.

## History Feed Orchestration

- Interface: `useHistoryFeedOrchestration(...)` in `app/src/lib/history/orchestration.ts`, called from `app/src/components/HistoryFeed.tsx` after query hooks/mutations are created.
- Implementation: owns recording-availability probing, hidden-entry optimistic UI state, copied-entry timing, retry-last-failed coordination, and shared-recording delete-mode coordination for the History tab.
- Depth: non-rendering History tab coordination moved out of `HistoryFeed.tsx` while query hooks, notifications, playback wiring, analysis modal wiring, and presentational sections remain visible in the Adapter.
- Deletion test: removing the Module pushes recording probe polling, hidden-entry rollback, retry-last-failed branching, and shared-recording delete coordination back into `HistoryFeed.tsx` rather than into the deterministic read model or filter-state Modules.
- Characterization: unit tests in `app/src/lib/history/orchestration.test.ts` cover recording probe prioritization, hidden-entry rollback helpers, retry action state, and delete/shared-recording helper decisions without real recordings or network.

## Data Backup / Cloud Sync Orchestration

- Interface: `useDataBackupCloudSyncOrchestration(...)` in `app/src/lib/settings/dataBackupCloudSync.ts`, called from `app/src/components/settings/DataSettings.tsx` with adapter-owned invalidation and shortcut re-registration callbacks.
- Implementation: owns settings export/import orchestration, GitHub token mutation plumbing, Gist push/pull, cloud-sync push/pull, analytics opt-in mutation wiring, and cloud-sync UI-state refresh for Data Settings.
- Depth: backup/cloud-sync mutation coordination moved out of `DataSettings.tsx` while file dialogs, notifications, destructive confirmations, and presentational sections remain visible in the Adapter.
- Deletion test: removing the Module pushes GitHub token/gist draft state, backup/cloud-sync mutation wiring, import re-register flow, and cloud-sync refresh/tracking behavior back into `DataSettings.tsx` rather than into the read-model Module.
- Characterization: unit tests in `app/src/lib/settings/dataBackupCloudSync.test.ts` cover import/re-register invalidation flow, gist-id normalization, gist push/pull orchestration, cloud-sync success/failure refresh behavior, and analytics opt-in tracking without real GitHub, network, or cloud sync.

## Data Retention Orchestration

- Interface: `useDataRetentionOrchestration(...)` in `app/src/lib/settings/dataRetention.ts`, called from `app/src/components/settings/DataSettings.tsx` with adapter-owned query invalidation callbacks.
- Implementation: owns request-log retention, recordings retention, transcription retention, stats retention, draft reset rules, unit-preserving retention transitions, and targeted settings/query invalidation for Data Settings.
- Depth: retention draft/mutation coordination moved out of `DataSettings.tsx`, while folder-opening actions, backup/cloud-sync actions, notifications, destructive confirmations, and presentational sections remain visible in the Adapter.
- Deletion test: removing the Module pushes retention source defaults, draft reset effects, logs/recordings/stats invalidation branching, legacy `max_saved_recordings` syncing, and unit-preserving transition logic back into `DataSettings.tsx` or `DataRetentionSection.tsx` instead of keeping `DataRetentionSection.tsx` presentational.
- Characterization: unit tests in `app/src/lib/settings/dataRetention.test.ts` cover source defaults, draft reset detection, unit-preserving unit changes, global-only disable behavior, and logs/recordings/stats invalidation intent without real Tauri writes.

## Logs View Orchestration

- Interface: `useLogsViewOrchestration(...)` in `app/src/lib/logs/orchestration.ts`, called from `app/src/components/LogsView.tsx` after request-log/settings queries are read.
- Implementation: owns export option selection, export popover state, clear-log mutation coordination, export notification intent shaping, and hotkey-debug cleanup for the Logs page.
- Depth: non-rendering toolbar/export coordination moved out of `LogsView.tsx`, while request-log queries, system-event listening, playback wiring, notifications, and page-level filter state remain visible in the Adapter.
- Deletion test: removing the Module pushes export mode selection, save-dialog branching, request-log invalidation intent, and hotkey-debug cleanup back into `LogsView.tsx` rather than into the deterministic read-model or presentational toolbar Module.
- Characterization: unit tests in `app/src/lib/logs/orchestration.test.ts` cover privacy-safe/full export selection, cancelled export handling, export notification intent, clear-log invalidation intent, and cleanup-only hotkey-debug shutdown without real dialogs or Tauri writes.

## Batch STT Orchestration

- Interface: `SharedPipeline::run_batch_stt_request(...)` implemented by `pipeline/batch_stt_orchestration.rs`.
- Implementation: wraps already-resolved STT providers for normal batch, streaming fallback, retry, and CLI transcription paths.
- Depth: managed-auth refresh retry, `stt_complete` bookkeeping, and shared failed-attempt state handling moved out of the main pipeline state-machine file while provider resolution and STT execution transport remain separate.
- Deletion test: removing the Module reintroduces managed-auth retry and failed-attempt handling into every batch STT caller in `pipeline.rs`.
- Characterization: unit tests in `pipeline/batch_stt_orchestration.rs` cover managed-auth token error classification.

## Recording Command Error Mapping

- Interface: `impl From<PipelineError> for CommandError` in `commands/recording_errors.rs`.
- Implementation: maps rich pipeline/STT/LLM/audio/state errors into stable command error categories, codes, retryability flags, and details.
- Depth: command return-shape classification is separated from recording request orchestration, history updates, retention, cost, OCR cleanup, and Tauri event emission.
- Deletion test: removing the Module pushes auth/rate-limit/state/size classification back into the large recording command flow.
- Characterization: unit tests in `commands/recording_errors.rs` cover STT auth, STT rate-limit, and stable state-error code mappings.

## Profile Query

- Interface: `pipeline::profile_query::{resolve_profile_for_foreground_app, resolve_profile_by_id, program_basename_for_log}` re-exported through `pipeline.rs` for command-facing callers.
- Implementation: lightweight profile identity lookup for UI chips, retry/test-transcription metadata preservation, and safe foreground-program logging.
- Depth: command modules no longer repeat profile-list scans or basename fallback rules, while request-time effective behavior remains in Profile Resolution.
- Deletion test: removing the Module pushes foreground/default-profile chip semantics, retry profile lookup, and basename fallback behavior back into `commands/recording.rs`.
- Characterization: unit tests in `pipeline/profile_query.rs` cover unknown foreground apps, unmatched default chips, matched profile identity, default marker preservation, and empty basename fallback.

## Transcription Retention

- Interface: `sessions::retention::apply_transcription_retention(...)`.
- Implementation: parses new and legacy retention settings, computes cutoff durations, prunes History rows, optionally deletes matching WAV files, and emits `history-changed` best-effort.
- Depth: raw settings-store reads and retention pruning are separated from command recording flows; commands only choose terminal boundaries where retention should run.
- Deletion test: removing the Module pushes retention unit/value parsing, legacy days fallback, prune/error handling, optional recording deletion, and History invalidation back into every transcription command path.
- Characterization: unit tests in `sessions/retention.rs` cover numeric/string retention values, fractional-hour retention, legacy-day fallback, and keep-forever zero/negative semantics.
