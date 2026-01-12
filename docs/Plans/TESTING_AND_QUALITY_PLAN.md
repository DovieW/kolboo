# Testing & Quality Plan (Implementation Guide)

This document is a practical, step-by-step plan for getting Kolboo (this repo) to a **really good place** testing-wise.

It is written for real day-to-day development: fast local feedback, reliable CI, and tests that catch the bugs we actually ship.

**Status**: Active plan

**Date**: 2026-01-12

---

## What exists today (current state)

### Tooling in place

**Frontend (TypeScript/React)**

- Type checking: `tsc --noEmit` (`pnpm -C app typecheck`)
- Unit tests: Vitest v4 (`pnpm -C app test`)
- Coverage: supported via Vitest v4 + `@vitest/coverage-v8` (`pnpm -C app coverage`)
- Lint/format: Biome (`pnpm -C app lint` / `pnpm -C app lint:ci`)
- Code smell / dead-code detection: Knip (`pnpm -C app knip`)

**Backend (Rust/Tauri)**

- Unit/integration tests: `cargo test` via `pnpm -C app cargo:test`
- Lints: `cargo clippy` via `pnpm -C app cargo:clippy`
- Formatting: `cargo fmt` via `pnpm -C app cargo:fmt`
- Non-mutating format check: `pnpm -C app cargo:fmt:check`

### One “CI-style” command

- `pnpm -C app check:ci`
  - Runs lint/typecheck/tests without rewriting files.
  - This is the best “single command” to answer: “Would CI be happy?”

### Hooks / automation

- Pre-commit hook runs `pnpm -C app check:ci` (so commits don’t sneak in type errors / failing tests).

### CI

- Windows workflow exists and runs checks.
- Recent work aligned Windows checks to call `pnpm check:ci`.

### Real bugs we’ve already seen

- Windows-only Rust test startup crash (status `0xc0000139`) due to a Common Controls v6 manifest dependency.
  - This is an example of a bug that “unit tests exist” won’t catch unless CI actually runs them on Windows.

---

## What “really good” looks like (target state)

This is the end goal. It’s okay if it takes multiple phases.

1. **Fast local loop**
   - Developers can run a small set of tests quickly (seconds–minutes), many times a day.
2. **Stable CI**
   - CI is deterministic and doesn’t randomly hang or depend on UI interaction.
3. **Tiered test strategy**
   - Fast unit tests always run.
   - Slower integration tests run in CI (or nightly).
   - E2E tests exist for the highest value user flows.
4. **Meaningful coverage**
   - Coverage is measured and used as a guardrail, not a vanity metric.
   - We at least cover the high-risk logic (settings normalization, pipeline transitions, provider request shaping).
5. **Testing is pleasant**
   - Running tests from VS Code is one click.
   - Failing tests give actionable output.

---

## Testing layers (what we should have)

### Layer 1: Pure unit tests (fast, no IO)

**Frontend**

- Pure function tests (e.g. diffing, formatting helpers, settings normalization)
- UI component tests only when there’s meaningful behavior (not snapshot spam)

**Rust**

- Pure logic tests (parsers, request shaping, state machine transitions)

**Goal:** these should run in seconds.

### Layer 2: Integration tests (some IO, still deterministic)

- Provider request/response handling with mocked HTTP
- Settings read/write flows with a temp store path
- Pipeline edge cases with mocked components

**Goal:** these should run in CI reliably.

### Layer 3: E2E tests (highest value user flows)

For a Tauri app, E2E can be tricky, so we should focus on flows that genuinely prevent regressions:

- “Start recording → stop → transcription appears”
- “Cancel transcription”
- “Hotkey registration doesn’t break”
- “Overlay shows correct state transitions”

**Goal:** run these less frequently (nightly or on demand) until stable.

---

## Step-by-step plan

### Step 1 — Lock in the foundation (already mostly done)

- Keep `check:ci` as the canonical command.
- Keep hooks using `check:ci` (no file rewriting in pre-commit).
- Ensure Windows CI uses the same command.

**Done when:** contributors can run one command and get the same answer locally and in CI.

### Step 2 — Make coverage usable (not annoying)

Coverage exists now, but we need to make it useful.

1. Decide which coverage reports we care about:
   - Terminal summary (quick)
   - HTML report (investigation)
2. Decide what files should be excluded:
   - generated files
   - types-only files
   - Vite entrypoints (usually not valuable)
3. Add a baseline “coverage expectation” policy:
   - Start with **report-only** (no thresholds) for a short period.
   - Later add thresholds only for high-risk folders.

**Done when:** anyone can run `pnpm -C app coverage` and understand what to improve.

### Step 3 — Increase the number of meaningful frontend tests

Today the frontend test surface is small. The fastest improvement is to add tests around code that:

- transforms settings
- formats request payloads
- handles error display/formatting

Suggested targets:

- `app/src/lib/formatError.ts`
- `app/src/lib/textDiff.ts`
- `app/src/lib/tauri.ts` (settings normalization / migrations)

**Rule of thumb:** test code that can break silently.

### Step 4 — Make Rust tests more “isolated”

Rust tests are plentiful, but we should prioritize tests that don’t require:

- network access
- real audio devices
- UI interaction

Suggested approach:

- Identify modules that currently talk to the network and wrap them behind small interfaces.
- For tests, swap in fake implementations.

**Done when:** `cargo test` is stable and fast on Windows/Linux/macOS.

### Step 5 — Add HTTP mocking for provider logic

A lot of bugs come from request/response shape drift.

Plan:

- For Rust provider modules, add tests that spin up a local mock server (so we test real HTTP handling without hitting real APIs).
- For frontend, use MSW (Mock Service Worker) if/when we add tests that involve fetch-like behavior.

**Done when:** provider bugs are caught before shipping.

### Step 6 — Add a small E2E smoke test suite (later)

This is optional until unit/integration testing is solid.

- Start with 1–2 high value smoke flows.
- Run them nightly or manually.

**Done when:** we catch “app boots but core flow broke” type regressions.

---

## CI recommendations (how we keep it reliable)

1. Prefer `check:ci` in CI.
2. Keep Windows in the test matrix (we’ve already seen Windows-specific runtime/linker issues).
3. Avoid tests that require human interaction.
   - If a test needs UI interaction, it must be clearly marked and run only in a special workflow.
4. Keep a “fast PR check” path if CI becomes too slow.
   - Option A: run everything on PRs
   - Option B: run unit tests on PRs; run full suite on merge/nightly

---

## Practical workflow for contributors (super simple)

- While working: run `pnpm -C app test:watch` (fast feedback)
- Before committing: let the hook run `pnpm -C app check:ci`
- When you want auto-fixes: run `pnpm -C app lint`
- When you’re improving coverage: run `pnpm -C app coverage`

---

## Risks & gotchas

- **Coverage isn’t free**: it adds overhead; don’t run it on every commit by default.
- **Coverage % can be misleading**: 80% coverage can still miss the important bugs.
- **Tauri E2E is harder than web E2E**: keep the scope small.

---

## Success checklist (how we know we’re winning)

- [ ] `check:ci` is the canonical local + CI command
- [ ] Frontend has a growing set of unit tests in the highest-risk modules
- [ ] Rust tests are stable across platforms
- [ ] Coverage reports exist and are acted on
- [ ] At least 1–2 E2E smoke flows exist (optional, later)
