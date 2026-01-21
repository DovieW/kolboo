# High-impact refactors (schedule / plan)

These are the refactors that most directly reduce risk (settings breakage, Rust/TS drift), improve correctness, or unlock reliable automated testing.

If you're choosing what to do "on purpose" (not as a drive-by), start here.

## Core architecture / design improvements

These are "bigger than a ticket" changes that would make the core easier to evolve safely.

- **Expand deterministic, headless pipeline integration tests (no network, no hardware).**
  - Add embeddings routing injection (or a deterministic offline substitute)
  - Add routing/preset selection invariants
  - Add a rewrite-disabled test case (ensure `final_text` falls back to `stt_text`)
