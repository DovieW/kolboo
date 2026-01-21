# High-impact refactors

## Core architecture / design improvements

- **Continue extraction of remaining "god file" areas from `app/src-tauri/src/pipeline.rs` (~2020 lines, reduced from ~3400):**
  - `pipeline/audio_loop.rs` (CPAL/VAD orchestration) - audio capture is already behind `AudioCaptureBackend` trait, but orchestration code remains in main file
  - **Next good extraction candidates (keep diffs small + cohesive):**
    - `pipeline/recording.rs`: the start/stop recording session logic + copying `last_wav_bytes` / diagnostics.
      - Goal: `pipeline.rs` keeps the public methods, but delegates to a small helper that returns `(wav_bytes, diagnostics)` and handles the size limit + state transitions.
    - STT provider resolution still duplicated 3x (profile override -> global fallback logic) - but tightly coupled with `PipelineInner` and `stt_provider_cache`; may not be worth extracting.
