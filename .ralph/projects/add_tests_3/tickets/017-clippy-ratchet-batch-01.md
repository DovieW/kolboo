# Ticket: Clippy ratchet batches (rolling)

## Goal (what we want)

Reduce Clippy warnings by fixing 1–3 low-risk warnings per small batch.

- We want: fewer warnings than before.
- So that: we can eventually reach 0 warnings without risky mega-PRs.

## Context (what exists today)

- Run: `pnpm -C app cargo:clippy:ci`
- There are many warnings (see tracker doc from the “Clippy zero warning tracker” ticket).
- This ticket is meant to be repeated as needed (keep batches tiny).

## Acceptance criteria (how we know it’s done)

- [ ] Pick 1–3 warnings that are mechanical/behavior-preserving (examples: `needless_return`, `unwrap_or_default`, `manual_clamp`, `redundant_closure`).
- [ ] Fix them with the smallest code change.
- [ ] Record before/after warning counts in the tracker doc.
- [ ] Ensure `pnpm -C app check:ci` is green.
- [ ] If more warnings remain, repeat this ticket in another small batch (same rules).

## Edge cases / gotchas

- Avoid refactors that change runtime behavior.
