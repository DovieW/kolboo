# Rust dead-code suppression audit

_Audit date: 2026-05-12_

This note captures a follow-up audit of the Rust `#[allow(dead_code)]` suppressions in `app/src-tauri/src/**` after wiring in stricter dead-code and dead-dependency checks.

## Why this audit exists

We added repo-level Rust dead-code checks:

- `pnpm -C app cargo:deadcode`
- `pnpm -C app cargo:deadcode:ci`
- `pnpm -C app cargo:deaddeps`

As of 2026-09-03, strict dead-code denial is Windows-owned. Running the same reachability lint on Ubuntu produced false failures for live Windows UIA and modifier-only shortcut paths. Linux and macOS continue to run ordinary Clippy and tests until platform modules are isolated finely enough for strict target-local reachability checks.

Those checks were green for the default feature set, which was surprising enough to warrant a closer look at the current suppression footprint.

The key distinction is:

- the new commands tell us whether there is **unsuppressed** dead code or dead dependencies
- they do **not** tell us whether existing `allow(dead_code)` suppressions are still justified

## Summary

The audit found **159** `allow(dead_code)` occurrences across `app/src-tauri/src/**/*.rs` and `app/src-tauri/src/*.rs`.

Breakdown by attribute shape:

- **81** plain `#[allow(dead_code)]`
- **69** `#[cfg_attr(not(test), allow(dead_code))]`
- **4** `#[cfg_attr(not(feature = "..."), allow(dead_code))]`
- **3** module-level `#![allow(dead_code)]`
- **2** `#[cfg_attr(not(desktop), allow(dead_code))]`

High-level conclusion:

- **Some suppressions are clearly justified**: test-only seams, feature-gated helpers, and non-desktop helpers.
- **Some suppressions are stale**: the symbol is now used, but the old suppression remains.
- **Some suppressions are broader than they need to be**: blanket module-level allows in actively used modules.

So the dead-code gate is still valuable, but today it is greener than the real maintenance picture because part of the surface area is hidden behind old suppressions.

## Scope and method

This audit focused on:

- suppression counts and shape grouping
- hotspot files with the highest concentration of suppressions
- representative usages to determine whether suppressions are justified, stale, or over-broad

The review concentrated on these hotspot files first:

- `app/src-tauri/src/audio_capture.rs`
- `app/src-tauri/src/pipeline.rs`
- `app/src-tauri/src/stt/mod.rs`
- `app/src-tauri/src/vad.rs`
- `app/src-tauri/src/llm/ollama.rs`

It also reviewed the edge-case buckets that tend to hide the most signal:

- module-level `#![allow(dead_code)]`
- `not(feature = "local-whisper")`
- `not(desktop)`

## What looks justified

### Test-only seams

The `#[cfg_attr(not(test), allow(dead_code))]` bucket is often legitimate.

Representative examples:

- test-only constructors/helpers in `app/src-tauri/src/pipeline.rs`
- helper accessors in `app/src-tauri/src/audio_capture.rs` that are exercised by unit tests
- provider helper methods in `app/src-tauri/src/stt/mod.rs` and several provider modules that exist primarily for tests

These should not all be assumed perfect, but this bucket is the least suspicious overall.

### Feature-gated Local Whisper helpers

The `not(feature = "local-whisper")` suppressions look justified.

Representative examples:

- `app/src-tauri/src/pipeline/local_provider_lifecycle.rs`
- `app/src-tauri/src/commands/whisper.rs`

These helpers are real parts of the Local Whisper path, but default builds can legitimately leave them unused.

### Non-desktop helpers

The `not(desktop)` suppressions in `app/src-tauri/src/app_shared.rs` look justified.

These helpers are used by desktop call paths and would otherwise warn in non-desktop builds.

### Intentional API compatibility holds

A few suppressions exist to preserve an outward-facing seam even when the symbol is lightly used.

Representative example:

- `app/src-tauri/src/audio_capture.rs` re-exports `WAVEFORM_BINS` from `audio_capture/meter.rs` to preserve the existing `audio_capture` API surface.

These should stay narrow and well-commented, but they are not inherently suspicious.

## What looks stale or too broad

### `app/src-tauri/src/pipeline.rs`

Several suppressions in this file look stale because the methods are actively used.

Representative stale examples:

- `audio_level_snapshot_fast()`
  - used by `app/src-tauri/src/overlay/mod.rs`
  - used by `app/src-tauri/src/commands/audio.rs`
- `audio_waveform_snapshot_fast()`
  - used by `app/src-tauri/src/overlay/mod.rs`
- `stop_recording_before_after()`
  - used by `app/src-tauri/src/commands/recording.rs`

Other public compatibility helpers like `stop_and_transcribe()`, `poll_vad_event()`, `audio_level_snapshot()`, `current_provider_name()`, and `get_cancel_token()` are also still referenced by tests or external call sites. They should be reviewed individually before keeping any suppression.

### `app/src-tauri/src/audio_capture.rs`

This file has the largest concentration of suppressions and includes several that now look stale.

Representative stale examples:

- `AudioBuffer::sample_rate()`
- `AudioBuffer::channels()`
- `AudioBuffer::to_wav_bytes()`
- `AudioCapture::level_snapshot()`
- `AudioCapture::poll_vad_event()`
- `AudioCapture::is_vad_auto_stop_enabled()`

These are referenced internally, through the trait implementation, or by tests. The suppressions appear to have survived earlier warning-cleanup work even though later refactors started using the symbols again.

### `app/src-tauri/src/llm/ollama.rs`

Several suppressions here also look stale.

Representative stale examples:

- `with_model()`
  - used by `app/src-tauri/src/pipeline/llm_provider.rs`
  - used by tests
- `with_client()`
  - used by `app/src-tauri/src/pipeline/llm_provider.rs`
- `list_models()`
  - used by `app/src-tauri/src/commands/ollama.rs`
- `TagsResponse` / `ModelInfo`
  - used by `list_models()`

The same general pattern likely exists across several other provider modules: a constructor or helper was once only test-facing, then became part of a real factory path, but the suppression remained.

### Module-level allows in active modules

These are the broadest and most suspicious suppressions in the current set.

#### `app/src-tauri/src/active_window_capture.rs`

This file has `#![allow(dead_code)]`, but the module is actively used by:

- `app/src-tauri/src/pipeline/ocr_session.rs`

The blanket module-level allow is too coarse for an actively used module. It may still contain some legitimately-unused fallback helpers, but the file-level suppression hides more than necessary.

#### `app/src-tauri/src/clipboard_context.rs`

This file also has `#![allow(dead_code)]`, but the module is actively used by:

- `app/src-tauri/src/sessions/context_collection.rs`
- `app/src-tauri/src/sessions/quick_action_execution.rs`
- `app/src-tauri/src/pipeline/transcription_flow.rs`

Again, the blanket suppression looks broader than necessary.

## Hotspots worth cleaning first

If we want the strict dead-code signal to mean what it appears to mean, the highest-leverage cleanup order is:

1. `app/src-tauri/src/pipeline.rs`
2. `app/src-tauri/src/audio_capture.rs`
3. blanket module-level allows in:
   - `app/src-tauri/src/active_window_capture.rs`
   - `app/src-tauri/src/clipboard_context.rs`
4. provider constructors/helpers in:
   - `app/src-tauri/src/llm/*.rs`
   - `app/src-tauri/src/stt/*.rs`

That order should remove the biggest amount of stale suppression with the lowest risk of changing behavior.

## Recommended cleanup strategy

Keep this incremental rather than trying to clean all 159 at once.

### Phase 1: remove obviously stale suppressions

Start with symbols that are definitely referenced today.

Representative candidates:

- stale `allow(dead_code)` attributes in `pipeline.rs`
- stale `allow(dead_code)` attributes in `audio_capture.rs`
- stale `allow(dead_code)` attributes in `llm/ollama.rs`

Then rerun:

- `pnpm -C app cargo:deadcode`
- `pnpm -C app test:all` when Rust + TS surface is touched together

### Phase 2: replace blanket file-level suppressions with narrow ones

For:

- `active_window_capture.rs`
- `clipboard_context.rs`

remove the file-level `#![allow(dead_code)]`, then add narrow symbol-level suppressions only where they are still truly needed.

### Phase 3: audit provider modules in batches

The provider files appear to have accumulated a pattern of constructor/helper suppressions that may have become stale after factory extraction work.

Audit in small batches by family:

- `llm/*.rs`
- `stt/*.rs`

This keeps fallout understandable and avoids one giant warning explosion.

## Limits / follow-up caveats

This audit was strongest for the default backend feature set.

A local run of the stricter `local-whisper` dead-code command was blocked by toolchain prerequisites:

- `pnpm -C app cargo:deadcode:local-whisper:ci`
- locally failed before linting because `whisper-rs-sys` could not find `clang.dll` / `libclang.dll`

So this report should be read as:

- **high confidence** for the default build shape
- **partial confidence** for the `local-whisper` feature shape until that toolchain prerequisite is satisfied locally or confirmed in CI

## Bottom line

The repo is in a better place than “we have no Rust dead-code story”, but it is **not** accurate to treat the current green dead-code result as proof that all Rust dead-code suppressions are justified.

The real state is:

- the new dead-code and dead-dependency commands are working
- there is no obvious unsuppressed Rust dead code in the default feature set
- there is still meaningful suppression debt, including stale and over-broad `allow(dead_code)` usage

That means the next quality win is not inventing another tool — it is shrinking the suppression footprint so the current tools can see more of the truth.
