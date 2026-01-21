# High-impact refactors (schedule / plan)

These are the refactors that most directly reduce risk (settings breakage, Rust/TS drift), improve correctness, or unlock reliable automated testing.

If you're choosing what to do "on purpose" (not as a drive-by), start here.

## Core architecture / design improvements

These are "bigger than a ticket" changes that would make the core easier to evolve safely.

- **Expand deterministic, headless pipeline integration tests (no network, no hardware).**
  - ✅ ~~Add a rewrite-disabled test case (ensure `final_text` falls back to `stt_text`)~~ — Added `rewrite_disabled_falls_back_to_stt_text` test in `pipeline/tests.rs`
  - ✅ ~~Add cosine similarity unit tests~~ — Added 8 tests in `embeddings/mod.rs` covering identical, opposite, orthogonal, empty, zero, and realistic embedding vectors
  - Add embeddings routing injection (or a deterministic offline substitute) for full integration-level routing tests
  - Add routing/preset selection invariants
