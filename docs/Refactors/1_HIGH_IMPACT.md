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

## Rust deterministic testing seams (hard IO audit)

- **Network IO (reqwest providers + proxy config):**
  - Hot spots:
    - `app/src-tauri/src/network.rs` + `app/src-tauri/src/commands/network.rs` (proxy + custom cert loading)
    - Providers under `app/src-tauri/src/{llm,stt,embeddings}/**` (reqwest calls)
  - Small test seams:
    - Continue the existing pattern of `with_client(...)` constructors (already present in several providers) so tests can inject a preconfigured client.
