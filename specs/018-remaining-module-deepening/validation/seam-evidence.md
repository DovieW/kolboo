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
- Characterization: unit tests in `audio_capture/device_selection.rs`, `audio_capture/preprocessing.rs`, and `audio_capture/meters.rs` cover encoded mic IDs, ordinal/legacy selection, gate-strength mapping, preprocessing invariants, meter clamping, waveform bucket shape, and empty speech detection.

## Retry Last Shortcut

- Interface: `shortcuts::retry_last::spawn_retry_last_recording_and_output(...)` re-exported through `shortcuts::spawn_retry_last_recording_and_output(...)` for existing dispatch call sites.
- Implementation: resolves the most recent history entry with a persisted recording, starts retry transcription, force-shows overlay loading UX, sanitizes transcript output, and emits no-recording feedback.
- Depth: retry-specific history/recording lookup and output orchestration moved out of generic shortcut dispatch while registration decisions and Windows hook mechanics remain in their existing Modules.
- Deletion test: removing the Module restores retryable-history selection and retry output side effects to both global-shortcut and modifier-only dispatch branches.
- Characterization: unit tests in `shortcuts/retry_last.rs` cover explicit recording-source preference, legacy entry-id fallback, and trimmed recording ids.

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
