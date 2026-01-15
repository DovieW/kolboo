# Ticket: Define coverage threshold scope (high-risk only)

## Goal (what we want)

Decide (and document) which parts of the frontend codebase are “high risk” enough that we want coverage thresholds.

- We want: a small, reasonable threshold scope.
- So that: coverage becomes a guardrail without turning into a tax.

## Context (what exists today)

- Coverage is already configured in: `app/vite.config.ts` (`test.coverage`).
- Coverage policy today is report-only (no thresholds).

## Consolidation note

This ticket also covers implementing/enforcing the chosen threshold policy (previously a separate “wire into CI/tooling” ticket).

- We already have several high-value tests in place (settings normalization, provider contracts, utility helpers).

## Acceptance criteria (how we know it’s done)

- [ ] Pick a scope that is small and high-risk (examples):
  - `app/src/lib/**` (or a narrower subset like `app/src/lib/tauri.ts`, `app/src/lib/queries.ts`)
  - provider request shaping helpers
  - settings normalization logic
- [ ] Write the decision into `docs/Plans/TESTING_AND_QUALITY_PLAN.md` under “Coverage policy”.
- [ ] Choose threshold _types_ (statements/branches/functions/lines) and initial numbers.
- [ ] Thresholds must be realistic for the current baseline (no “set to 80% and explode the repo” surprises).

- [ ] Implement the threshold policy in the tooling:
  - Add coverage thresholds in `app/vite.config.ts` (or referenced config) for the chosen scope.
  - Decide enforcement point:
    - Option A: thresholds apply only when running `pnpm -C app coverage` (recommended at first).
    - Option B: add a separate CI job/command (e.g. `check:ci:coverage`) so PRs can opt-in.
  - Document how to run/enforce in `docs/Plans/TESTING_AND_QUALITY_PLAN.md`.
  - Ensure `pnpm -C app check:ci` remains reasonably fast (don’t silently add coverage to it unless explicitly decided).

## Edge cases / gotchas

- Exclude low-signal files (Vite entrypoints, generated/types-only) so thresholds don’t punish the wrong things.
- Branch coverage is usually the first one that makes you sad—set it deliberately.

## Backpressure (must be green)

- `pnpm -C app check:ci`

## Non-goals (explicitly out of scope)

- Enforcing coverage repo-wide.
- Adding E2E tests.

## Notes / hints

- Prefer starting with 1–2 folders/files and expanding later.
