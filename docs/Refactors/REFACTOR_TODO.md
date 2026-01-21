# Refactor TODO (opportunistic follow-ups)

These are small/medium follow-ups discovered while doing other work.

## Pipeline module cleanup

- Continue extraction of remaining "god file" areas from `app/src-tauri/src/pipeline.rs` (~3000 lines):
  - `pipeline/audio_loop.rs` (CPAL/VAD orchestration) — audio capture is already behind `AudioCaptureBackend` trait, but orchestration code remains in main file
  - Consider extracting transcription flow logic (currently spread across multiple large `pub async fn` methods)

## Supply chain follow-ups

- Investigate reducing/removing GTK3 transitive dependencies on Linux (Tauri pulls GTK3 crates that are now unmaintained).
  - Evaluate GTK4-compatible stack or alternative windowing path for Linux.
