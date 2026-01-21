# High-impact refactors

## Core architecture / design improvements

- **Expand deterministic, headless pipeline integration tests (no network, no hardware).**
  - Add embeddings routing injection (or a deterministic offline substitute) for full integration-level routing tests
  - Add routing/preset selection invariants

- **Continue extraction of remaining "god file" areas from `app/src-tauri/src/pipeline.rs` (~2100 lines, reduced from ~3400):**
  - `pipeline/audio_loop.rs` (CPAL/VAD orchestration) — audio capture is already behind `AudioCaptureBackend` trait, but orchestration code remains in main file
  - ✅ **`pipeline/transcription_flow.rs` integration COMPLETE** — shared routing + LLM rewrite logic extracted (~895 lines). Both `stop_and_transcribe_detailed` and `transcribe_wav_bytes_detailed_for_profile` now delegate to `complete_transcription_flow()`. Net reduction: ~1300 lines from `pipeline.rs`.
  - **Next good extraction candidates (keep diffs small + cohesive):**
    - `pipeline/recording.rs`: the start/stop recording session logic + building `AudioEncodeConfig` + copying `last_wav_bytes` / diagnostics.
      - Goal: `pipeline.rs` keeps the public methods, but delegates to a small helper that returns `(wav_bytes, diagnostics)` and handles the size limit + state transitions.
    - `pipeline/stt_flow.rs`: the STT provider/model selection + `with_retry(...)` transcription chunk used by both live and retry flows.
      - Goal: eliminate another chunk of duplicated “resolve effective STT settings + call provider + log provider/model used”.
