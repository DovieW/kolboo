# High-impact refactors

## Core architecture / design improvements

- **Expand deterministic, headless pipeline integration tests (no network, no hardware).**
  - Add embeddings routing injection (or a deterministic offline substitute) for full integration-level routing tests
  - Add routing/preset selection invariants

- **Continue extraction of remaining "god file" areas from `app/src-tauri/src/pipeline.rs` (~3000 lines):**
  - `pipeline/audio_loop.rs` (CPAL/VAD orchestration) — audio capture is already behind `AudioCaptureBackend` trait, but orchestration code remains in main file
  - **Complete `pipeline/transcription_flow.rs` integration** — module exists with shared routing + LLM rewrite logic (`TranscriptionContext`, `TranscriptionCallbacks` trait, `complete_transcription_flow()`). `PipelineCallbacks` adapter is implemented. Remaining work:
    1. Replace inline routing/rewrite code in `stop_and_transcribe_detailed` with calls to `complete_transcription_flow()`
    2. Replace inline routing/rewrite code in `transcribe_wav_bytes_detailed_for_profile` with calls to `complete_transcription_flow()`
    3. Remove the duplicate inline implementations (~600+ lines of duplicated code)
