# Ticket: Wire coverage thresholds into tooling (without slowing dev)

<!-- ralph:skip -->

> Deprecated (consolidated): this work is now covered by `004-coverage-thresholds-define-scope.md`.

## Goal (what we want)

Implement the threshold policy decided in the “define scope” ticket, so it’s actually enforced in a predictable way.

- We want: thresholds enforced in the right place.
- So that: we don’t regress coverage in the high-risk areas.

## Context (what exists today)

- Coverage config lives in: `app/vite.config.ts` under `test.coverage`.
- Command exists: `pnpm -C app coverage`.
- Canonical CI-style command exists: `pnpm -C app check:ci`.

## Acceptance criteria (how we know it’s done)

- [ ] Add coverage thresholds in `app/vite.config.ts` (or a referenced config) for the chosen scope.
- [ ] Decide enforcement point:
  - Option A: thresholds apply only when running `pnpm -C app coverage` (recommended at first).
  - Option B: add a separate CI job/command (e.g. `check:ci:coverage`) so PRs can opt-in.
- [ ] Document how to run it in `docs/Plans/TESTING_AND_QUALITY_PLAN.md`.
- [ ] Make sure `pnpm -C app check:ci` remains reasonably fast (don’t silently add coverage to it unless explicitly decided).

## Edge cases / gotchas

- Coverage on Windows can be slightly different; don’t set thresholds so tight that OS drift breaks CI.

## Backpressure (must be green)

- `pnpm -C app check:ci`

## Non-goals (explicitly out of scope)

- Requiring coverage on every commit.
