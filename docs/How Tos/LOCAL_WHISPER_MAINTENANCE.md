# Local Whisper maintenance notes

This document is an **internal maintenance guide** for Kolboo’s Local Whisper subsystem (offline STT), including the load lifecycle, settings migrations, UI↔backend contract, and CUDA troubleshooting.

## Quick mental model

Local Whisper is a Whisper.cpp-based STT provider implemented via the Rust crate `whisper-rs` (which builds/links whisper.cpp + ggml).

- **Frontend** (React/TS) manages model downloads, selection, and load/unload UX.
- **Backend** (Tauri/Rust) maintains a provider cache and a pipeline state machine.
- **Settings** are persisted in `settings.json` (tauri store), normalized in `app/src/lib/tauri.ts`, and default-seeded/migrated in `app/src-tauri/src/lib.rs`.

## Where things live

### Backend (Rust)

- Provider + CUDA preflight: `app/src-tauri/src/stt/whisper.rs`
- Pipeline integration / caching / load modes: `app/src-tauri/src/pipeline.rs`
- Commands and events (download/load): `app/src-tauri/src/commands/whisper.rs`
- Command registration + startup preload behavior: `app/src-tauri/src/lib.rs`
- Request logs: `app/src-tauri/src/request_log.rs` (effective provider/model should reflect what ran)

### Frontend (TS/React)

- Settings UI: `app/src/components/settings/ApiKeysSettings.tsx` (Local Whisper card)
- Tauri API wrapper + types: `app/src/lib/tauri.ts`
- Query hooks: `app/src/lib/queries.ts`

## Settings (canonical store)

Persisted in `settings.json` via `@tauri-apps/plugin-store`.

Key settings used by Local Whisper:

- `stt_provider`: must be `local-whisper` or `whisper` for Local Whisper to be used
- `local_whisper_model_id`: which model file to use (e.g. `base`, `small`, `large-v3`)
- `local_whisper_load_mode`: when to load the model into memory
  - `manual` (default)
  - `on_transcribe`
  - `on_launch`

If you add/rename settings:

1. Update default seeding/migrations in `ensure_default_settings(...)` (`app/src-tauri/src/settings/defaults.rs`)
2. Update TS normalization in `tauriAPI.getSettings()` (`app/src/lib/tauri.ts`)
3. If overlays depend on it, emit `settings-changed` so secondary windows refresh.

## Load lifecycle (important for UX + avoiding deadlocks)

Local Whisper model initialization can be expensive.

**Rule:** do not do heavy work while holding the pipeline mutex.

The pipeline uses a 3-phase pattern:

1. Capture config under lock
2. Perform heavy work outside the lock (model/provider creation)
3. Re-acquire lock to insert provider/cache

### Load modes

- `manual`: user must click “Load model” in Settings before transcribing
- `on_transcribe`: auto-load on first transcription attempt
- `on_launch`: best-effort preload when the app starts

### Commands/events

- `load_local_whisper_model` / `unload_local_whisper_model`
- Load status event: `local-whisper-model-load` with statuses `started|completed|error`
- Download progress event: `whisper-model-download-progress`

Frontend listens to these events and shows notifications + invalidates React Query keys.

## Model downloads and verification

Models are downloaded to a models directory reported by `get_whisper_models_dir` and listed by `get_whisper_models`.

Downloads are verified using SHA-256.

UI supports:

- download
- cancel
- validate
- delete
- select active model

Changing the model ID unloads the currently loaded model, but does **not** auto-load the new one.

## Provider caching and request logs

### Caching

Local Whisper providers are cached by **model path** (not just a generic model name) so switching models doesn’t mistakenly reuse a provider.

Config sync should not aggressively evict the Local Whisper cache unless the **model path** or **transcription prompt** changes.

### Request logs

Request logs should persist the **effective provider/model** used at transcription time (including local model filename), across main + retry paths.

## CUDA: what we actually mean

There are two different concepts users conflate:

- **Driver capability** (reported by `nvidia-smi`, e.g. "CUDA Version: 12.9")
- **Runtime libraries** your app loads (`cudart64_12.dll`, `cublas64_12.dll`, etc.)

To use CUDA compute:

- the NVIDIA driver must be installed (`nvcuda.dll`)
- the CUDA runtime DLLs must be discoverable (PATH or next to the executable)
- the driver must be new enough for the runtime major version

### Diagnostics surfaced in UI

Settings → Local Whisper shows:

- **Compute**: CPU vs GPU (CUDA) based on build feature + DLL preflight
- **Observed**: whether `nvidia-smi` sees this PID in the compute apps list

Important: “Compute: GPU (CUDA)” means "Kolboo will request GPU"; “Observed” is a runtime sanity check.

### Common Windows failure modes

1. **CUDA toolkit installed but DLLs not on PATH**

   - Fix: add toolkit `bin\\x64` to PATH for that session, or bundle the DLLs next to the app.

2. **Driver/runtime mismatch**
   - Example: app loads CUDA 13 runtime DLLs but driver reports CUDA 12.x capability.
   - Fix: upgrade driver, or build/bundle against CUDA 12 for broad compatibility.

### Compatibility strategy (recommended)

- Ship a CPU build that works everywhere.
- Target **CUDA 12 runtime** for the CUDA build variant for maximum Windows compatibility.
- Consider an optional/experimental CUDA 13 build later.

## CI notes (GitHub Actions)

- GitHub Actions **Windows** runners do not reliably have the CUDA Toolkit installed.
- `container:` jobs only apply to **Linux** runners.

Kolboo’s current policy is to **avoid CUDA builds in GitHub Actions**.

If you need CUDA artifacts, build them locally on a machine with CUDA Toolkit installed and upload them to Releases.

## Packaging/bundling guidance (Windows)

To avoid requiring end users to install the CUDA Toolkit:

- Bundle the required CUDA runtime DLLs next to the app executable for the CUDA build.
- Users still need the NVIDIA driver.

Keep versions consistent:

- CUDA 12 build should ship `cudart64_12.dll`, `cublas64_12.dll`, `cublasLt64_12.dll` (and any additional dependencies required by the runtime).

## When changing Local Whisper in the future

When you touch Local Whisper, sanity-check:

- UI settings shape + migrations (`settings.json` defaults + TS normalization)
- Pipeline lock usage (no heavy work under mutex)
- Cache keys (model path) and eviction rules
- Request logs show effective provider/model
- Events and payloads remain consistent between Rust and TS
- CUDA diagnostics still match reality (driver/runtime mismatch handling)
