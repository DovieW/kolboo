# Refactor TODO (opportunistic follow-ups)

These are small/medium follow-ups discovered while doing other work.

## Pipeline module cleanup

- Continue extraction of remaining "god file" areas from `app/src-tauri/src/pipeline.rs`:
  - `pipeline/audio_loop.rs` (CPAL/VAD orchestration)
  - `pipeline/stt.rs` (STT provider selection + retry/timeout + request logging)

- ✅ ~~Consider moving the large `pipeline.rs` test module into `app/src-tauri/src/pipeline/tests.rs`.~~
  - Done: Tests now live in `pipeline/tests.rs`.
