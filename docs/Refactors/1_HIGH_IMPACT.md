# High-impact refactors (schedule / plan)

These are the refactors that most directly reduce risk (settings breakage, Rust/TS drift), improve correctness, or unlock reliable automated testing.

If you’re choosing what to do “on purpose” (not as a drive-by), start here.

## Core architecture / design improvements

These are “bigger than a ticket” changes that would make the core easier to evolve safely.

- **Make settings a versioned, schema-driven contract (single source of truth).**
  - Today: backend seeds defaults in `ensure_default_settings(...)`, and frontend normalizes/migrates in `app/src/lib/tauri.ts`.
  - Upgrade the model:
    - Make migrations explicit and ordered (vN -> vN+1).
    - Run migrations at startup *once* (before any UI depends on settings).
    - Keep a `settings_version` in persisted state and bump it for every shape change.
  - Strong recommended variant (multi-window safety):
    - **Backend is the only writer.** Frontend windows send “patches”; Rust validates, writes, and emits `settings-changed`.
    - This avoids last-write-wins clobber bugs when multiple windows write `settings.json` from stale snapshots.
  - End goal:
    - TS reads a trusted, migrated shape (so TS normalization can shrink from “repair everything” to “validate + default”).

- **Dependency injection seams for hard IO (testability).**
  - You already have good “seams” in places (e.g. `with_client(...)` patterns).
  - Next step: formalize a few minimal traits/interfaces so the pipeline can be tested without:
    - CPAL devices
    - real filesystem
    - real network
  - Keep it small: only extract interfaces where unit tests would meaningfully increase confidence.

- **Create a clean layering boundary: “Tauri shell” vs “Core services”.**
  - Today: a lot of orchestration lives in `app/src-tauri/src/lib.rs` and reaches into many subsystems.
  - Goal: keep `lib.rs` focused on wiring (commands/events/windows/tray), and move business logic into a small set of services/modules.
  - Suggested shape:
    - `core/*` (pipeline orchestration, quick ask/replace orchestration)
    - `adapters/*` (tauri emit/invoke, filesystem, audio capture, clipboard)
    - `commands/*` becomes thin wrappers around core services
  - Why this helps: it’s much easier to test “core logic” without needing a Tauri runtime.

- **Make UI↔backend contract drift hard/impossible.**
  - Current risk: “string glue” (command names, event names, payload shapes) can drift without compile-time errors.
  - Direction:
    - Prefer generation for the contract surface where practical (commands/events/payload types), so renames become build errors.
    - Keep contract tests, but aim for a world where they mostly confirm the generator is working.

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

## Suggested sequencing (so refactors don’t turn into a “forever project”)

- Phase 1 (risk killers): backend-only settings writer + explicit versioned migrations.
- Phase 2 (safety net): deterministic pipeline tests with fakes.
- Phase 3 (structure): split `lib.rs` into core/adapters/commands; decompose `pipeline.rs` into modules.
