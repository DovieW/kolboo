# Kolboo Context

## Domain terms

### STT Provider Resolution

The transcription-time decision that turns profile, preset, global settings, and optional per-run CLI overrides into the concrete STT provider used for a request.

This includes provider/model/language selection, request-log provider metadata, local provider special cases, managed inference routing, provider cache lookup/creation, and global-provider fallback when a profile or override provider is unavailable.

### STT Execution

The act of sending WAV audio to an already-resolved STT provider and returning normalized transcript text plus timing/retry telemetry.

STT execution includes retry policy, timeout behavior, and cancellation priority, but does not choose which provider to use.

### Transcription Flow

The post-STT flow that turns raw transcript text into final output text.

This includes preset routing and optional LLM rewrite. It starts after STT execution has produced transcript text.

### Routing Decision

A strategy-independent result from preset routing that Transcription Flow can consume without knowing whether embeddings or an LLM made the choice.

Routing Decision distinguishes selected preset, default target, no decision, ambiguity, provider failure, and cancellation, and carries bounded request-log diagnostics.

### Profile Resolution

The deterministic behavior that matches a foreground program to a prompt profile and resolves effective profile/preset behavior for a transcription or quick action.

Profile Resolution is split into program matching (path normalization and first-match ordering) and effective behavior (default profile fallback, active preset selection, and Active Window OCR mode precedence).

### Profile Query

The command-facing read model for lightweight profile identity lookups used outside request-time Profile Resolution.

Profile Query owns foreground-profile chips, retry/test-transcription profile identity preservation, and safe program-basename logging. It does not decide effective runtime behavior for a request; that remains in Profile Resolution and Transcription Flow.

### Settings View

A source-aware, normalized read model for persisted settings.

Settings View preserves explicit-null semantics, falls back safely for missing or malformed values, and identifies whether an effective value came from stored/global/profile/preset/default inputs.

### History Request Lifecycle

The app-facing sequence of request-row transitions for History entries.

History Request Lifecycle turns command/session events into deterministic History updates such as create-in-progress, profile/preset/model mirroring, recording-source attachment, terminal success/error, and cancellation cleanup. `history.rs` still owns persistence and querying; the lifecycle layer exists so callers stop hand-stitching low-level History mutations.

### Runtime Sync Policy

The frontend policy that classifies settings mutations by runtime side effect and deduplicates pipeline config syncs and secondary-window `settings-changed` events for one logical settings batch.

### OCR Session

The best-effort active-window OCR lifecycle tied to a user-visible request id.

OCR Session includes runtime readiness validation, screenshot task ownership, result reuse, cancellation, overlay status, and request-log correlation across internal pipeline transitions. Its core invariant is that OCR task results and telemetry belong only to the session that started them, even if the pipeline returns to idle or a newer request supersedes the old one.

### Local Provider Lifecycle

The local STT provider rules for cache identity, manual/on-transcribe/on-launch readiness, managed-inference bypass, explicit unload, and config-change eviction.

Local Provider Lifecycle is deterministic and separate from cloud provider construction; actual local model loading remains feature-gated and user-initiated according to settings.

### Provider-Family Seam

A shared provider abstraction that is only introduced when at least two concrete adapters use it and the deletion test proves caller complexity would otherwise reappear.

Provider-family concerns without a real two-adapter proof are documented as deferred rather than implemented as pass-through abstractions.

### WebSocket Transport Policy

The provider-family connection policy for realtime STT WebSocket transports.

WebSocket Transport Policy owns manual HTTP proxy CONNECT tunnelling, manual `no_proxy` bypass, trusted CA certificates, and invalid-cert override handling. It does not own provider URLs, headers, protocol messages, partial transcript semantics, or streaming state machines; those remain in concrete STT adapters and provider-independent session helpers.

### Data Lifecycle

The frontend read model for user-visible app data retention, storage, and sync state.

Data Lifecycle owns UI-safe normalization for data/storage settings such as retention unit conversions, recordings storage summaries, cloud-sync display state, and byte formatting. Data settings components consume this read model and remain UI adapters over Mantine controls and mutation hooks.

### History Feed Read Model

The frontend read model for History tab filtering, grouping, and transcript-analysis prompt preparation.

History Feed Read Model owns persisted filter normalization, date grouping, token-count estimates, and deterministic analysis prompt construction. `HistoryFeed.tsx` remains the UI adapter over query hooks, playback controls, and destructive-action modals.

### Telemetry Mapping

The conversion from rich, request-scoped diagnostics into narrow read models for downstream systems.

Telemetry Mapping does not own request-log storage, redaction, or export stripping. It selects the minimum fields needed by a consumer (for example, Cost Reporting inputs) so consumers do not need to know every `RequestLog` variant and fallback rule.

### Cost Reporting

The event-level assembly of provider telemetry, usage counters, duration fallbacks, and estimated list-price cost for persisted stats events.

Cost Reporting owns provider response parsing and choosing the cost estimate for a single STT or LLM event. Provider pricing tables and formulas remain in provider-specific `cost/**` modules, while `stats.rs` owns persistence, aggregation, retention, and UI invalidation.

### Embeddings Module

The provider interface and concrete adapters used to turn text into embeddings for intent routing.

The Embeddings Module owns provider defaults, query-vs-document input roles, and construction of HTTP-backed embeddings adapters. Routing Decision logic consumes the `EmbeddingsProvider` interface and does not call provider-specific embedding modules directly.

### Recording Orchestration

The command-facing coordination that translates pipeline state transitions into user-visible recording phase events and related side effects.

Recording Orchestration does not own the pipeline state machine, STT execution, Transcription Flow, or Quick Ask / Quick Replace execution. It keeps repeated command-side watchers and phase-notification policies local while preserving the pipeline as the source of truth.

### Recording Completion

The narrow command-facing tail work that happens after a recording request reaches a terminal outcome.

Recording Completion covers saved-WAV persistence for retry/playback and the shared final transcript / cancelled / error event shapes. It does not own request-log success stamping, cost emission, OCR cleanup, or platform text output.

### Transcription Retention

The backend policy that interprets persisted transcription-retention settings and prunes old History rows plus optional recording WAV files.

Transcription Retention owns raw settings parsing, legacy fallback, cutoff calculation, best-effort pruning, and History invalidation. Command flows decide when a terminal transcription attempt should apply the policy; they should not repeat retention setting reads or prune logic inline.

### Audio Capture Internals

The backend Modules inside Audio Capture that keep microphone selection, stop-time preprocessing, and realtime meters local without changing the external `AudioCaptureBackend` Interface.

Audio Capture Internals owns session-stable mic selection tokens, capture cleanup controls such as high-pass/AGC/noise suppression/noise gate, and atomic level/waveform meter snapshots. It does not own provider-independent audio format conversion; that remains in Audio Normalization.

### Batch STT Orchestration

The cross-flow wrapper around already-resolved batch STT attempts.

Batch STT Orchestration owns managed-auth refresh retry, `stt_complete` bookkeeping, and shared failed-attempt state handling for normal batch, streaming fallback, retry, and CLI transcription paths. STT Provider Resolution still chooses providers, and STT Execution still performs provider transport/retry/timeout work.

### Retry Last Shortcut

The Shortcut Dispatch slice that resolves the most recent saved recording and starts a retry transcription/output action.

Retry Last Shortcut owns retryable history-entry selection, recording-source fallback rules, overlay loading UX, and retry output. It does not own hotkey registration decisions or Windows modifier-only hook mechanics.

### Recording Command Error Mapping

The command-facing translation from rich pipeline errors into stable UI error codes, retryability flags, and error categories.

Recording Command Error Mapping keeps Tauri command return shapes consistent without adding that classification logic back into the large recording command flow.
