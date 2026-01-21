# High-impact refactors

## Core architecture / design improvements

- **Expand deterministic, headless pipeline integration tests (no network, no hardware).**
  - Add embeddings routing injection (or a deterministic offline substitute) for full integration-level routing tests
  - Add routing/preset selection invariants

- **Continue extraction of remaining "god file" areas from `app/src-tauri/src/pipeline.rs` (~3000 lines):**
  - `pipeline/audio_loop.rs` (CPAL/VAD orchestration) — audio capture is already behind `AudioCaptureBackend` trait, but orchestration code remains in main file
  - Consider extracting transcription flow logic (currently spread across multiple large `pub async fn` methods)
