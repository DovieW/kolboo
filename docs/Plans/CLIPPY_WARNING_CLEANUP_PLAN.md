# Clippy warning cleanup plan (batched + reviewable)

This plan turns the current clippy output in `docs/Plans/CLIPPY_OUTPUT.md` into a handful of small, reviewable batches.

## Goal

- Reduce clippy warnings toward **0** (or at least “low enough that new warnings stand out”).
- Keep each PR small enough to review without needing to understand the entire Tauri pipeline.
- Prefer **mechanical**, low-risk fixes first.

## Non-goals

- No behavior changes unless a warning forces it.
- No big refactors “just to make clippy happy” in the early batches.

## How to run/verify

In each batch, use the same workflow:

1. Make the code changes for that batch.
2. Verify just the Rust-side quality gate:

   - `pnpm -C app cargo:clippy:ci`
   - `pnpm -C app cargo:fmt:check`
   - `pnpm -C app cargo:test:ci`

3. If the batch touched UI↔backend contracts or command surfaces, run the full gate:

   - `pnpm -C app check:ci`

### Optional “clippy fix” usage

Clippy reports: `run cargo clippy --fix --lib -p kolboo to apply 44 suggestions`.

That can be helpful, but it can also create a large diff. The safer approach is:

- Use `cargo clippy --fix` **only for a single batch** (limited set of files), and review the diff carefully.
- If the diff gets big, undo and apply the suggestions manually.

## Batch 1: Purely mechanical cleanups in `src/commands/**`

**Why first:** these warnings are usually trivial (remove `return`, collapse `if`s) and very unlikely to change behavior.

**Target files (from output):**

- `app/src-tauri/src/commands/network.rs` (`manual_div_ceil`)
- `app/src-tauri/src/commands/overlay.rs` (`needless_return`)
- `app/src-tauri/src/commands/recording.rs` (`needless_return`, `neg_cmp_op_on_partial_ord`, plus a few small style warnings)
- `app/src-tauri/src/commands/settings.rs` (`redundant_closure`)
- `app/src-tauri/src/commands/llm.rs` (`too_many_arguments` — defer the refactor to Batch 5; keep Batch 1 limited to mechanical warnings only)

**Warnings to fix here:**

- `clippy::needless_return`
- `clippy::collapsible_if` / `clippy::collapsible_else_if` / `clippy::collapsible_match`
- `clippy::manual_div_ceil`
- `clippy::redundant_closure`
- `clippy::neg_cmp_op_on_partial_ord` (be careful; use a clear equivalent like `value <= 0.0` or `partial_cmp` if NaN handling matters)

**Done when:** `cargo:clippy:ci` no longer reports warnings for these files (or warning count drops and remaining ones are intentionally deferred).

## Batch 2: Provider + integration modules (safe string/vec tweaks)

**Why:** these warnings are typically “make Rust idiomatic” changes that don’t touch the core state machine.

**Target files (from output):**

- `app/src-tauri/src/cost/fireworks.rs` (`if_same_then_else`, `manual_pattern_char_comparison`)
- `app/src-tauri/src/embeddings/cohere.rs` (`vec_init_then_push`, `single_char_add_str`)
- `app/src-tauri/src/embeddings/fireworks.rs` (`single_char_add_str`)
- `app/src-tauri/src/embeddings/openai.rs` (`single_char_add_str`)
- `app/src-tauri/src/llm/cerebras.rs` (`single_char_add_str`)
- `app/src-tauri/src/llm/prompts.rs` (`derivable_impls`)

**Warnings to fix here:**

- `clippy::single_char_add_str` (use `push('x')` instead of `push_str("x")`)
- `clippy::vec_init_then_push` / `clippy::reserve_after_initialization`
- `clippy::if_same_then_else`
- `clippy::manual_pattern_char_comparison`
- `clippy::derivable_impls`

## Batch 3: Core-ish helpers (history/request_log/settings/state/vad)

**Why:** medium-scope modules that are still relatively self-contained.

**Target files (from output):**

- `app/src-tauri/src/history.rs` (`reserve_after_initialization`, `manual_div_ceil`, etc.)
- `app/src-tauri/src/request_log.rs` (`manual_clamp`)
- `app/src-tauri/src/settings.rs` (`derivable_impls`)
- `app/src-tauri/src/state.rs` (`manual_clamp`)
- `app/src-tauri/src/vad.rs` (`derivable_impls`, `manual_clamp`, `collapsible_if`)

**Warnings to fix here:**

- `clippy::manual_clamp` (replace with `.clamp(min, max)` where types allow)
- `clippy::derivable_impls`
- `clippy::reserve_after_initialization`
- `clippy::collapsible_if`

## Batch 4: Big-but-mechanical files (`stats.rs`, `pipeline.rs`, `lib.rs`)

**Why:** these files have a lot of warnings, but many are still mechanical.

**Target files (from output):**

- `app/src-tauri/src/stats.rs` (`needless_borrow`, `collapsible_if`, etc.)
- `app/src-tauri/src/pipeline.rs` (`needless_lifetimes`, `too_many_arguments`, `collapsible_if`, plus some `push_str` single-char style)
- `app/src-tauri/src/lib.rs` (mix of `question_mark`, `nonminimal_bool`, `let_and_return`, `collapsible_else_if`, `unwrap_or_default`, `redundant_closure`, etc.)

**Rules for this batch:**

- Only do **mechanical** rewrites. If a change feels like it could alter behavior, move it to Batch 5.
- Keep PR size in check by splitting this batch into 4a/4b if needed:
  - 4a: `lib.rs`
  - 4b: `stats.rs` + `pipeline.rs` (mechanical only)

## Batch 5: Refactor-needed warnings (`too_many_arguments`)

**Why last:** these often require reorganizing code and can accidentally change behavior.

**Target warnings/files (from output):**

- `app/src-tauri/src/audio_capture.rs`: `run_capture_thread(...)` has 14 args
- `app/src-tauri/src/commands/llm.rs`: `iterate_rewrite_prompt(...)` has 14 args
- `app/src-tauri/src/pipeline.rs`: a few functions are over the argument limit (8/7, 9/7)

**Suggested approach:**

- Introduce a small “args struct” per function (e.g., `RunCaptureThreadArgs { ... }`).
- Keep fields named so call-sites are clearer.
- Add 1–2 unit tests for any extracted pure helpers (if feasible) so we’re confident nothing changed.

## Batch 6: Windows-only hotkeys module

**Target file (from output):**

- `app/src-tauri/src/windows_modifier_hotkeys.rs` (`collapsible_if`, etc.)

This can be its own PR since it’s Windows-specific and should be easy to review.

## Suggested PR ordering (smallest-first)

1. Batch 1 (commands)
2. Batch 2 (providers)
3. Batch 3 (history/request_log/settings/state/vad)
4. Batch 6 (windows hotkeys)
5. Batch 4 (big-but-mechanical)
6. Batch 5 (refactors)
