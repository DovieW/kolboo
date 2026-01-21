# Refactor TODO (opportunistic follow-ups)

These are small/medium follow-ups discovered while doing other work.

## Pipeline module cleanup

- Remove the now-dead legacy routing implementation that is still present in `app/src-tauri/src/pipeline.rs` behind `#[cfg(any())]`.
  - It was left temporarily to keep the diff manageable while extracting routing into `app/src-tauri/src/pipeline/routing.rs`.
  - After removal, `pipeline.rs` should shrink significantly.

- Continue extraction of remaining "god file" areas from `app/src-tauri/src/pipeline.rs`:
  - `pipeline/audio_loop.rs` (CPAL/VAD orchestration)
  - `pipeline/stt.rs` (STT provider selection + retry/timeout + request logging)

- Consider moving the large `pipeline.rs` test module into `app/src-tauri/src/pipeline/tests.rs` (or `app/src-tauri/src/pipeline/tests/mod.rs`) once the module layout is stable.
  - Goal: make `pipeline.rs` a clearer orchestration entrypoint.
