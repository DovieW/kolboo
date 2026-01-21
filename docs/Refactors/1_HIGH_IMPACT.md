# High-impact refactors (schedule / plan)

These are the refactors that most directly reduce risk (settings breakage, Rust/TS drift), improve correctness, or unlock reliable automated testing.

If you’re choosing what to do “on purpose” (not as a drive-by), start here.

## Core architecture / design improvements

These are “bigger than a ticket” changes that would make the core easier to evolve safely.


- **Decompose `pipeline.rs` into modules while preserving the state machine.**
  - Motivation: reduce blast radius and make “routing”, “transcription”, and “audio loop” testable in isolation.
  - Target shape (example):
    - ✅ `pipeline/state_machine.rs` (pure transitions + guards)
    - ✅ `pipeline/routing.rs` (intent routing logic)
    - ✅ `pipeline/types.rs` (pipeline errors + transcription result structs)
    - ✅ `pipeline/utils.rs` (small pure helpers)
    - ✅ `pipeline/llm_provider.rs` (LLM provider construction)
    - ⏳ `pipeline/audio_loop.rs` (cpal + buffering + VAD)
    - ⏳ `pipeline/stt.rs` (provider loop + retry/timeout)
  - Ideal outcome: `pipeline.rs` becomes a small facade and orchestration entrypoint.

- **Expand deterministic, headless pipeline integration tests (no network, no hardware).**
  - Baseline exists (fake audio capture + injected STT provider).
  - Remaining seams to add (if needed for coverage):
    - LLM provider injection (rewrite step)
    - embeddings routing injection (or a deterministic offline substitute)
  - Additional flows worth covering:
    - routing/preset selection invariants
    - rewrite-enabled vs rewrite-disabled behavior

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
