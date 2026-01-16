# Ticket: Clippy ratchet batch 04 (fix 1–3 warnings)

<!-- ralph:skip -->

> Deprecated (consolidated): use `025-clippy-ratchet-batch-01.md` (rolling).

## Goal (what we want)

Reduce Clippy warnings by fixing 1–3 low-risk warnings.

## Context (what exists today)

- Run: `pnpm -C app cargo:clippy:ci`
- Track warning count deltas in: `docs/Plans/CLIPPY_ZERO_WARNINGS.md`.

## Acceptance criteria (how we know it’s done)

- [ ] Fix 1–3 warnings (mechanical changes only).
- [ ] Update the tracker with before/after counts.
- [ ] `pnpm -C app check:ci` stays green.

## Backpressure (must be green)

- `pnpm -C app check:ci`
