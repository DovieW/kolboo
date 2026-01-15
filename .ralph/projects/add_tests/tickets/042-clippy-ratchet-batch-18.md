# Ticket: Clippy ratchet batch 18 (fix 1–3 warnings)

<!-- ralph:skip -->

> Deprecated (consolidation): keep the clippy tracker + batches 01–06 for now.

## Goal (what we want)

Reduce Clippy warnings by fixing 1–3 low-risk warnings.

## Context (what exists today)

- Run: `pnpm -C app cargo:clippy:ci`
- Update: `docs/Plans/CLIPPY_ZERO_WARNINGS.md`

## Acceptance criteria (how we know it’s done)

- [ ] Fix 1–3 warnings (mechanical).
- [ ] Update tracker with before/after counts.
- [ ] `pnpm -C app check:ci` green.

## Backpressure (must be green)

- `pnpm -C app check:ci`
