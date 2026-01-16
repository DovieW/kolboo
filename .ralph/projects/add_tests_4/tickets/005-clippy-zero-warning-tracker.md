# Ticket: Create a Clippy “zero warnings” tracker (and baseline count)

## Goal (what we want)

Turn “get clippy to 0 warnings” into a manageable checklist with a baseline count, so we can chip away safely.

- We want: one place that lists current warnings and how many remain.
- So that: the follow-up “ratchet batches” can be tiny and deterministic.

## Context (what exists today)

- `pnpm -C app check:ci` runs `cargo clippy` and currently prints warnings (but does not fail CI).
- We want to reach a state where clippy output contains **0 warnings**.

## Acceptance criteria (how we know it’s done)

- [ ] Add a small tracker doc (suggested location): `docs/Plans/CLIPPY_ZERO_WARNINGS.md`.
- [ ] Record a baseline warning count from `pnpm -C app cargo:clippy:ci`.
- [ ] Include a simple “How to run” section and a rule: fix 1–3 warnings per PR.
- [ ] Link to the tracker from `docs/REFACTOR_TODO.md` under the clippy section.

## Edge cases / gotchas

- Don’t paste secrets or API keys into logs or examples.
- Clippy output can change between Rust versions; record the Rust toolchain version if practical.

## Non-goals (explicitly out of scope)

- Fixing any warnings in this ticket (that’s what the batch tickets are for).
