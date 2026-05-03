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
