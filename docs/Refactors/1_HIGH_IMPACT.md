# High-impact refactors (schedule / plan)

These are the refactors that most directly reduce risk (settings breakage, Rust/TS drift), improve correctness, or unlock reliable automated testing.

If you’re choosing what to do “on purpose” (not as a drive-by), start here.

## Core architecture / design improvements

These are “bigger than a ticket” changes that would make the core easier to evolve safely.

- **Make settings a versioned, schema-driven contract (single source of truth).**
  - Today: backend seeds defaults in `ensure_default_settings(...)`, and frontend normalizes/migrates in `app/src/lib/tauri.ts`.
  - Add:
    - explicit migrations (vN -> vN+1) that run at startup (not when visiting a UI screen)
  - Bonus: this also reduces Rust/TS drift because the migration logic lives in one place.

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

- **Standardize error handling across commands (one error shape to the UI).**
  - Today: errors bubble up in different formats depending on where they come from.
  - Suggested: one `AppError` type with stable fields (code, message, details, retryable, request_id?) and a single conversion path to Tauri command errors.
  - Why: frontend error UI becomes simpler and more consistent; logging/telemetry can attach stable codes.

## Rust deterministic testing seams (hard IO audit)

- **Audio device IO (CPAL):**
  - Hot spots:
    - `app/src-tauri/src/audio_capture.rs` (CPAL host/device/stream + callback threading)
    - `app/src-tauri/src/commands/audio.rs` (device listing, “ensure active stream” for meters)
    - `app/src-tauri/src/pipeline.rs` (references to CPAL device selection + meter updates)
  - Small test seams:
    - Keep CPAL behind a tiny trait (e.g. “AudioCaptureBackend”) so pipeline state transitions can be tested with a fake capture backend that emits deterministic “audio level” events.

- **Filesystem IO (history/recordings/stats/backups/models):**
  - Hot spots:
    - `app/src-tauri/src/history.rs` (read/write history file, metadata checks)
    - `app/src-tauri/src/recordings.rs` (create/read/delete/list recordings)
    - `app/src-tauri/src/stats.rs` and `app/src-tauri/src/commands/stats.rs` (append logs, list/delete)
    - `app/src-tauri/src/commands/backup.rs` (read/write settings backup)
    - `app/src-tauri/src/commands/whisper.rs` + `app/src-tauri/src/commands/config.rs` (model dir creation/download temp files)
  - Small test seams:
    - Consider a minimal “Fs” interface only where needed (read/write/list/delete) for the most critical code paths (history + recordings), but avoid big refactors.

- **Network IO (reqwest providers + proxy config):**
  - Hot spots:
    - `app/src-tauri/src/network.rs` + `app/src-tauri/src/commands/network.rs` (proxy + custom cert loading)
    - Providers under `app/src-tauri/src/{llm,stt,embeddings}/**` (reqwest calls)
  - Small test seams:
    - Continue the existing pattern of `with_client(...)` constructors (already present in several providers) so tests can inject a preconfigured client.
