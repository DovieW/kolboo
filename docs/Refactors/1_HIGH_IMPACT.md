# High-impact refactors (schedule / plan)

These are the refactors that most directly reduce risk (settings breakage, Rust/TS drift), improve correctness, or unlock reliable automated testing.

If you’re choosing what to do “on purpose” (not as a drive-by), start here.

## Core architecture / design improvements

These are “bigger than a ticket” changes that would make the core easier to evolve safely.


- **Decompose `pipeline.rs` into modules while preserving the state machine.**
  - Motivation: reduce blast radius and make “routing”, “transcription”, and “audio loop” testable in isolation.
  - Target shape (example):
    - `pipeline/state_machine.rs` (pure transitions + guards)
    - `pipeline/audio_loop.rs` (cpal + buffering + VAD)
    - `pipeline/stt.rs` (provider loop + retry/timeout)
    - `pipeline/routing/*` (intent routing logic)
  - Ideal outcome: `pipeline.rs` becomes a small facade and orchestration entrypoint.

- **Add deterministic, headless pipeline integration tests (no network, no hardware).**
  - Inject fakes for:
    - audio capture
    - STT provider
    - LLM provider
    - embeddings provider (if applicable)
  - Cover critical flows:
    - happy path (record → transcribe → output)
    - cancellation (during recording and during provider requests)
    - timeout/retry and recovery back to `Idle`
    - routing/preset selection invariants

## Rust deterministic testing seams (hard IO audit)

- **Network IO (reqwest providers + proxy config):**
  - Hot spots:
    - `app/src-tauri/src/network.rs` + `app/src-tauri/src/commands/network.rs` (proxy + custom cert loading)
    - Providers under `app/src-tauri/src/{llm,stt,embeddings}/**` (reqwest calls)
  - Small test seams:
    - Continue the existing pattern of `with_client(...)` constructors (already present in several providers) so tests can inject a preconfigured client.

- **Audio capture IO (CPAL device dependency):**
  - If you can’t run pipeline tests in CI without a microphone/device, you don’t really have pipeline tests.
  - Add (or finish) an `AudioCapture` abstraction so tests can inject sample buffers deterministically.
