# High-impact refactors

## Core architecture / design improvements

- **Expand deterministic, headless pipeline integration tests (no network, no hardware).**
  - Add embeddings routing injection (or a deterministic offline substitute) for full integration-level routing tests
  - Add routing/preset selection invariants

- **Continue extraction of remaining "god file" areas from `app/src-tauri/src/pipeline.rs` (~3000 lines):**
  - `pipeline/audio_loop.rs` (CPAL/VAD orchestration) — audio capture is already behind `AudioCaptureBackend` trait, but orchestration code remains in main file
  - **Wire up `pipeline/transcription_flow.rs`** — module was created with shared routing + LLM rewrite logic, but `stop_and_transcribe_detailed` and `transcribe_wav_bytes_detailed_for_profile` still use inline implementations. The next step is to replace the inline code with calls to the new `complete_transcription_flow()` function.
