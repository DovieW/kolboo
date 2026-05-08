# Provider adapter audit (Phase 6)

_Date: 2026-05-07_

_Update (2026-05-07): Speechmatics and Fireworks provider-local realtime extractions are now complete in `app/src-tauri/src/stt/speechmatics/realtime.rs` and `app/src-tauri/src/stt/fireworks/realtime.rs`. This audit no longer has a remaining STT realtime extraction candidate._

This audit is intentionally **read-only**. Its job is to measure the current STT provider adapters, record where each one mixes batch construction with realtime protocol state machines, and pick the next provider-local Module without inventing a provider-family parser Seam.

## Ground rules

- This audit does **not** justify a cross-provider realtime parser Interface.
- Provider-independent WebSocket/session lifecycle already lives in `app/src-tauri/src/stt/streaming.rs`.
- Transport policy already has a real provider-family Seam in `app/src-tauri/src/stt/websocket_transport.rs`.
- The question here is narrower: which concrete provider adapters are deep enough to benefit from a provider-local `realtime.rs` style Module like `app/src-tauri/src/stt/openai/realtime.rs`.

## Baseline proof

| Provider        | Current provider-local Module              | Lines | Why it matters                                                                                                                                                                                              |
| --------------- | ------------------------------------------ | ----: | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| OpenAI realtime | `app/src-tauri/src/stt/openai/realtime.rs` |   704 | Proven template: provider-specific session-update payloads, event parsing, transcript accumulation, and finalize heuristics can move out of the main Adapter without creating a cross-provider parser seam. |

## Current adapters

| Provider     | File                                    | Lines | Current responsibilities                                                                                                                                                                                                                                               | Test count | Provider-local module deletion test                                                                                                                                                                                          | Priority |
| ------------ | --------------------------------------- | ----: | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------: | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------- |
| Deepgram     | `app/src-tauri/src/stt/deepgram.rs`     |   239 | Batch HTTP `/v1/listen`, model/language normalization, request-log shaping, and provider wiring now that realtime WS URL/auth setup, `Results` parsing, finalize/close choreography, and transcript accumulation live in `deepgram/realtime.rs`                        |         10 | **Passes** — deleting `deepgram/realtime.rs` would push Deepgram-specific WS URL construction, `Results` parsing, finalize/close choreography, and transcript accumulation back into the main Adapter                        | Complete |
| ElevenLabs   | `app/src-tauri/src/stt/elevenlabs.rs`   |   243 | HTTP multipart fallback, provider construction, model/language normalization, and trait wiring now that buffered realtime WS transcription, concurrent streaming, VAD/manual commit behavior, and realtime transcript parsing live in `elevenlabs/realtime.rs`         |          8 | **Passes strongly** — deleting `elevenlabs/realtime.rs` would re-mix both realtime protocol loops with HTTP fallback/model selection in the main Adapter                                                                     | Complete |
| Speechmatics | `app/src-tauri/src/stt/speechmatics.rs` |   222 | Provider construction, WAV/PCM decode helpers, language normalization, and trait wiring now that concurrent streaming WS startup, sentence-level transcript accumulation, and one-shot batch-via-WS protocol handling live in `speechmatics/realtime.rs`               |          7 | **Passes** — deleting `speechmatics/realtime.rs` would push both Speechmatics WS protocol loops, `RecognitionStarted` / `AudioAdded` / transcript event handling, and sentence-level accumulation back into the main Adapter | Complete |
| AssemblyAI   | `app/src-tauri/src/stt/assemblyai.rs`   |   423 | Batch upload/submit/poll workflow, language mapping, request-log shaping, and provider wiring now that realtime WS URL/auth setup, `Turn` / `Termination` parsing, and turn accumulation moved into `assemblyai/realtime.rs`                                           |         12 | **Passes** — deleting `assemblyai/realtime.rs` would push AssemblyAI-specific WS URL construction, `Turn` / `Termination` parsing, terminate choreography, and transcript accumulation back into the main Adapter            | Complete |
| Fireworks    | `app/src-tauri/src/stt/fireworks.rs`    |   229 | OpenAI-compatible batch multipart call, provider construction, model/language normalization, and trait wiring now that streaming WS URL/auth setup, 16 kHz resampling, segment/checkpoint parsing, and stability/age commit heuristics live in `fireworks/realtime.rs` |         10 | **Passes** — deleting `fireworks/realtime.rs` would push Fireworks-specific streaming URL/auth setup, segment-id parsing, checkpoint handling, and stability/age commit heuristics back into the main Adapter                | Complete |

## What the audit found

### Deepgram

Why it is the cleanest next extraction:

- The batch path is already a compact HTTP Adapter.
- The realtime path is a self-contained Deepgram protocol state machine:
  - WS query construction
  - `Results` / `Metadata` / `UtteranceEnd` handling
  - `is_final` commit rules
  - `Finalize` + `CloseStream` shutdown
- Existing tests mostly cover URL and transcript-shaping helpers, so a provider-local Module would add meaningful synthetic frame coverage instead of just shuffling code.

Recommended target split:

- Keep in `deepgram.rs`:
  - provider construction
  - batch `transcribe(...)`
  - model/language normalization
  - `SttProvider` trait implementation
- Move to `stt/deepgram/realtime.rs`:
  - streaming WS URL/auth/session start
  - Deepgram `Results` parsing
  - partial/final transcript accumulation
  - finalize/close behavior
  - synthetic frame tests

### ElevenLabs

Why it is next after Deepgram:

- The Adapter is doing more than one realtime job today:
  - buffered realtime batch transcription (`transcribe_realtime_ws(...)`)
  - concurrent session streaming (`start_streaming_session(...)` + `run_streaming_task(...)`)
- It also carries commit-strategy behavior (`manual` vs `vad`) and provider-specific message parsing (`partial_transcript`, `committed_transcript`, `committed_transcript_with_timestamps`).
- Test coverage is currently thin relative to the amount of protocol logic.

Recommended target split:

- Keep in `elevenlabs.rs`:
  - provider construction
  - model selection / `should_use_realtime_api()`
  - batch HTTP fallback
  - audio decode helper only if it remains shared by both paths
  - `SttProvider` trait implementation
- Move to `stt/elevenlabs/realtime.rs`:
  - WS URL building for realtime
  - buffered realtime WS path
  - concurrent streaming session path
  - VAD/manual commit semantics
  - event parsing and transcript accumulation
  - synthetic event tests

### Speechmatics

Why it worked once the earlier proof existed:

- It really did contain **two** provider-specific WS modes:
  - concurrent streaming
  - one-shot batch-via-WS
- The live-output behavior is provider-local rather than family-local because sentence commits depend on Speechmatics `is_eos`, not a cross-provider finality rule.
- Once Deepgram / ElevenLabs / AssemblyAI had proven the pattern, moving both WS paths together was more about careful ownership than uncertainty.

Implemented split:

- Keep in `speechmatics.rs`:
  - provider construction
  - WAV/PCM decode helpers
  - language normalization
  - `SttProvider` trait implementation
- Move to `stt/speechmatics/realtime.rs`:
  - batch-via-WS startup/send/drain flow
  - concurrent streaming WS startup/task flow
  - `RecognitionStarted` / `AudioAdded` / transcript event parsing
  - sentence-level transcript accumulation based on `is_eos`
  - synthetic frame tests

### AssemblyAI

Why it worked as the next extraction:

- The `Turn`/`Termination` streaming protocol was clean enough to isolate without inventing a provider-family parser seam.
- The batch upload/submit/poll workflow was already a coherent HTTP Adapter, so the realtime loop could move out without dragging endpoint selection or provider construction with it.
- Existing URL and transcript-shaping tests made it straightforward to add synthetic frame coverage for the provider-local module.

Implemented split:

- Keep in `assemblyai.rs`:
  - provider construction
  - batch upload/submit/poll workflow
  - language normalization
  - `SttProvider` trait implementation
- Move to `stt/assemblyai/realtime.rs`:
  - streaming WS URL/auth/session start
  - `Begin` / `Turn` / `Termination` parsing
  - partial/final turn accumulation
  - `Terminate` shutdown handling
  - synthetic frame tests

### Fireworks

Why it worked as the final follow-up extraction:

- The file was smaller than Speechmatics, but the provider-specific logic was still deep enough to justify locality once the testing pattern had hardened.
- The segment stability heuristic and checkpoint flow are specific enough that keeping them in the main adapter would keep a false “simple streaming provider” illusion alive.
- After the other extractions, the remaining work was cleanly provider-local rather than a reason to invent a family parser seam.

Implemented split:

- Keep in `fireworks.rs`:
  - provider construction
  - batch OpenAI-compatible multipart transcription
  - model/language normalization
  - `SttProvider` trait implementation
- Move to `stt/fireworks/realtime.rs`:
  - streaming WS URL/auth/session start
  - 16 kHz resampling and chunk send loop
  - segment/checkpoint protocol parsing
  - segment-id parsing
  - stability/age commit heuristics
  - synthetic frame tests

## Decision

### Provider-family seam decision

Now **reject** the broad cross-provider realtime parser Interface.

Why:

- The shared parts are already extracted at the right layer:
  - WebSocket transport policy
  - provider-independent session lifecycle helpers
  - audio normalization
- The remaining complexity is provider-local:
  - Deepgram `Results` semantics
  - ElevenLabs VAD/manual commit messages
  - Speechmatics `AddTranscript` / `AddPartialTranscript` with sentence-level `is_eos`
  - AssemblyAI `Turn` / `Termination`
  - Fireworks segment stability and checkpoint behavior

Deleting a hypothetical shared parser seam would not simplify callers; it would mostly recreate a pass-through wrapper around six different protocol state machines whose current callers already consume `StreamingSttSession` / `PartialTranscript` rather than provider-specific parse state.

Reopen only if a **narrower** two-adapter subproblem emerges with its own deletion test (for example a proven shared accumulator or shared post-audio session driver). Do not reopen the broad parser idea just because the loop shapes look similar.

## Current status

- No remaining STT provider-local realtime extraction candidates remain from this audit.
- The provider-family parser seam stays deferred because the shared leverage is still in transport/session helpers, not protocol parsing.

## Notes for future provider additions

- Do not move batch endpoint selection or provider construction into the new realtime Modules.
- Do not add a provider-family `RealtimeParser` trait.
- Prefer synthetic JSON frame tests and session-state helpers over network tests.
- Treat `app/src-tauri/src/stt/openai/realtime.rs` as the template for locality, not as a mandate to force identical shapes where a provider's protocol differs.
