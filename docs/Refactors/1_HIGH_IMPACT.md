# High-impact refactors (schedule / plan)

These are the refactors that most directly reduce risk (settings breakage, Rust/TS drift), improve correctness, or unlock reliable automated testing.

If you're choosing what to do "on purpose" (not as a drive-by), start here.

## Core architecture / design improvements

These are "bigger than a ticket" changes that would make the core easier to evolve safely.

- **Decompose `pipeline.rs` into modules while preserving the state machine.** DONE
  - Motivation: reduce blast radius and make "routing", "transcription", and "audio loop" testable in isolation.
  - Target shape (example):
    - `pipeline/state_machine.rs` (pure transitions + guards)
    - `pipeline/routing.rs` (intent routing logic)
    - `pipeline/types.rs` (pipeline errors + transcription result structs)
    - `pipeline/utils.rs` (small pure helpers)
    - `pipeline/llm_provider.rs` (LLM provider construction)
    - `audio_capture.rs` (cpal + buffering + VAD) - already a separate module
    - `pipeline/stt_provider.rs` (STT provider factory)
  - Ideal outcome: `pipeline.rs` becomes a small facade and orchestration entrypoint.

- **Expand deterministic, headless pipeline integration tests (no network, no hardware).** DONE
  - Fake audio capture (`FakeAudioCapture`)
  - STT provider injection (`inject_stt_provider_for_tests`)
  - LLM provider injection (`inject_llm_provider_for_tests`)
  - Test for transcription without network/hardware
  - Test for transcription + rewrite without network/hardware
  - Remaining if needed:
    - embeddings routing injection (or a deterministic offline substitute)
    - routing/preset selection invariants

## Rust deterministic testing seams (hard IO audit)

- **Network IO (reqwest providers + proxy config):** DONE
  - Hot spots:
    - `app/src-tauri/src/network.rs` + `app/src-tauri/src/commands/network.rs` (proxy + custom cert loading)
    - Providers under `app/src-tauri/src/{llm,stt,embeddings}/**` (reqwest calls)
  - Small test seams:
    - `with_client(...)` constructors are present in all providers for test injection

- **Audio capture IO (CPAL device dependency):** DONE
  - `AudioCaptureBackend` trait allows injection
  - `FakeAudioCapture` used in pipeline tests
  - Tests run in CI without microphone/device
