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

### Settings View

A source-aware, normalized read model for persisted settings.

Settings View preserves explicit-null semantics, falls back safely for missing or malformed values, and identifies whether an effective value came from stored/global/profile/preset/default inputs.

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
