# Testing & Quality Plan (Implementation Guide)

This document is a practical, step-by-step plan for getting Kolboo (this repo) to a **really good place** testing-wise.

It is written for real day-to-day development: fast local feedback, reliable CI, and tests that catch the bugs we actually ship.

**Status**: Active plan

**Date**: 2026-01-12

---

## Scope (what we will and won’t do)

This plan explicitly targets **unit + integration + contract testing**, plus CI reliability.

**Out of scope for this plan:** end-to-end (E2E) UI automation.

- Rationale: Tauri E2E tends to be higher-effort and higher-flake than the value we need right now.
- If we ever add E2E later, it should be a separate doc/workstream so it doesn’t block shipping.

## Progress log (what we’ve already done)

### 2026-01-12 — Testing foundation + CI alignment

- Added a single canonical, non-mutating “CI style” command: `pnpm -C app check:ci` (includes: Biome check (no write), TypeScript typecheck, Knip, Vitest, Rust clippy, Rust fmt check, Rust tests)
- Hooked pre-commit to run `check:ci` (prevents committing broken code without rewriting files).
- Aligned Windows CI to run `pnpm check:ci` (instead of separate ad-hoc steps).
- Added Vitest coverage command (`pnpm -C app coverage`) and coverage provider dependency.
- Ignored generated coverage artifacts in `.gitignore` (e.g. `app/coverage/`).
- Adjusted `check:ci` to avoid `cargo clippy --all-features` by default (prevents slow/fragile builds of optional native deps like `whisper-rs-sys`).

### 2026-01-12 — Deterministic provider contract tests

- Added deterministic provider tests using a local mock HTTP server (Wiremock) so we can verify request/response shape without real API keys.
- STT: Whisper Server provider (`multipart/form-data`, prompt clamping, non-2xx error surface)
- LLM: Ollama provider (`/api/chat` request JSON contract, JSON error parsing, `/api/tags` parsing)

### 2026-01-13 — Settings normalization guardrails

- Expanded `tauriAPI.getSettings()` regression tests to lock down:
- Legacy Quick Ask hotkey fallback rules (missing vs explicit null)
- Normalization/clamping for invalid stored values
- Write-back behavior when `cleanup_prompt_sections` is malformed but salvageable

### 2026-01-13 — More getSettings legacy coverage

- Added tests for additional legacy fallbacks (so old `settings.json` installs keep working):
- `auto_mute_audio` → `playing_audio_handling`
- `noise_gate_strength` → `noise_gate_threshold_dbfs`
- `transcription_retention_days` → `{ unit: "days", value: ... }`
- legacy/typo enum values like `overlay_monitor_target: "activeWindow"`

### 2026-01-13 — Cloud provider contract tests (Wiremock)

- Added deterministic Wiremock tests for OpenAI providers without real API keys:
- LLM: `/v1/responses` request JSON contract + JSON error parsing
- STT: `/v1/audio/transcriptions` multipart contract + prompt clamping

### 2026-01-13 — Coverage policy decision

- Coverage remains **report-only** for now (no thresholds), to avoid slowing the default dev loop or blocking contributors on a number.

### 2026-01-15 — Coverage thresholds implemented (high-risk scope only)

- Added coverage thresholds for `app/src/lib/tauri.ts` (settings normalization logic)
- Thresholds: 50% statements/functions/lines, 40% branches
- Enforcement: thresholds only apply when running `pnpm -C app coverage` (not in default `check:ci`)

---

## What exists today (current state)

### Tooling in place

#### Frontend (TypeScript/React)

- Type checking: `tsc --noEmit` (`pnpm -C app typecheck`)
- Unit tests: Vitest v4 (`pnpm -C app test`)
- Coverage: supported via Vitest v4 + `@vitest/coverage-v8` (`pnpm -C app coverage`)
- Lint/format: Biome (`pnpm -C app lint` / `pnpm -C app lint:ci`)
- Code smell / dead-code detection: Knip (`pnpm -C app knip`)

#### Backend (Rust/Tauri)

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

## Coverage policy

### Current approach

Coverage is configured with **targeted thresholds** for high-risk code only.

**Scope:** `app/src/lib/tauri.ts` (settings normalization and migration logic)

**Thresholds:**
- Statements: 50%
- Branches: 40%
- Functions: 50%
- Lines: 50%

**Rationale:**
- Settings normalization is high-risk (silent bugs persist across upgrades)
- Thresholds are realistic for current baseline (not punitive)
- Limited scope keeps the dev loop fast

### How to run coverage

**Local development:**
```bash
pnpm -C app coverage
```

This generates:
- Terminal summary (shows threshold pass/fail)
- HTML report in `app/coverage/` (for investigation)

**Important:**
- Coverage thresholds are **not** enforced in `pnpm -C app check:ci` (to keep commits fast)
- Thresholds only apply when explicitly running the `coverage` command
- Use coverage as a guardrail when touching settings logic, not as a gate for all changes

### Excluded files

The following are excluded from coverage (low signal):
- Type declaration files (`*.d.ts`)
- Vite entrypoints (`main.tsx`, `overlay-main.tsx`, etc.)
- Generated files

### Expanding coverage

When adding thresholds for new files:
1. Pick high-risk code (provider contracts, state machines, critical business logic)
2. Check current baseline with `pnpm -C app coverage`
3. Set thresholds at or slightly below current coverage
4. Document the decision here

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
   - UI automation/E2E tests are intentionally excluded from this plan.
4. **Meaningful coverage**
   - Coverage is measured and used as a guardrail, not a vanity metric.
   - We at least cover the high-risk logic (settings normalization, pipeline transitions, provider request shaping).
5. **Testing is pleasant**
   - Running tests from VS Code is one click.
   - Failing tests give actionable output.

---

## Testing layers (what we should have)

### Layer 1: Pure unit tests (fast, no IO)

#### Frontend

- Pure function tests (e.g. diffing, formatting helpers, settings normalization)
- UI component tests only when there’s meaningful behavior (not snapshot spam)

#### Rust

- Pure logic tests (parsers, request shaping, state machine transitions)

**Goal:** these should run in seconds.

### Layer 2: Integration tests (some IO, still deterministic)

- Provider request/response handling with mocked HTTP
- Settings read/write flows with a temp store path
- Pipeline edge cases with mocked components

**Goal:** these should run in CI reliably.

---

## Step-by-step plan

### Step 1 — Lock in the foundation (already mostly done)

- Keep `check:ci` as the canonical command.
- Keep hooks using `check:ci` (no file rewriting in pre-commit).
- Ensure Windows CI uses the same command.

**Done when:** contributors can run one command and get the same answer locally and in CI.

**Status:** Done (foundation is in place).

**Note:** keep `check:ci` fast and deterministic. Run optional heavy checks (like clippy with `local-whisper`) separately.

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

**Status:** Done (coverage command exists; include/exclude + report output are configured; thresholds intentionally deferred).

### Step 3 — Increase the number of meaningful frontend tests

Today the frontend test surface is small. The fastest improvement is to add tests around code that:

- transforms settings
- formats request payloads
- handles error display/formatting

Suggested targets:

- `app/src/lib/formatError.ts`
- `app/src/lib/textDiff.ts`
- `app/src/lib/tauri.ts` (settings normalization / migrations)

Concrete “tickets” (handoff-friendly):

1. **`formatErrorMessage(...)` behavior lock-down** (`app/src/lib/formatError.test.ts`)
   - Input: `null` / `undefined` → `"Unknown error"`
   - Input: strings/numbers/booleans/bigints → `String(value)`
   - Input: `new Error("x")` → `"x"`
   - Input: `new Error("")` → should fall back to something non-empty (`error.toString()`)
   - Input: `{ message: "x" }` or `{ error: "x" }` → `"x"` (trimmed)
   - Input: circular object → returns JSON containing `"[Circular]"` (and does not throw)
   - Input: `{}` → falls back to `String(error)` (current behavior)

2. **`diffTextInline(...)` / `isDiffTrivial(...)` edge cases** (extend `app/src/lib/textDiff.test.ts`)
   - Unicode + apostrophes: `"don’t"` vs `"don't"` should be a minimal diff (tokenization should not explode)
   - Whitespace preservation: ensure added/removed chunks include whitespace where appropriate
   - Multi-line edits: inserting a newline should show as an insertion of `"\n"` (or be trivial when only CRLF vs LF differs)
   - Non-English: a short sample with accents (e.g. `"café"`) should not get split into weird tokens

3. **New tests should follow the repo style**
   - Use Vitest; avoid snapshots unless they truly add signal.
   - Keep tests deterministic and fast (< 50ms each).

**Rule of thumb:** test code that can break silently.

**Status:** In progress (utility-module unit tests added; keep expanding into settings normalization / migrations).

### Step 4 — Make Rust tests more “isolated”

Rust tests are plentiful, but we should prioritize tests that don’t require:

- network access
- real audio devices
- UI interaction

Suggested approach:

- Identify modules that currently talk to the network and wrap them behind small interfaces.
- For tests, swap in fake implementations.

**Done when:** `cargo test` is stable and fast on Windows/Linux/macOS.

Concrete “tickets” (handoff-friendly):

1. **Inventory “hard IO” paths** (one-time audit)
   - Identify modules that touch:
     - audio devices (cpal)
     - filesystem persistence (history, recordings, request log)
     - network (providers)
   - For each, write down the smallest seam we can introduce to fake it in tests.

2. **Prefer test seams that don’t change runtime behavior**
   - Example pattern we already used successfully:
     - “base URL override” (for Wiremock) on providers, defaulting to production URLs.
   - Avoid large refactors unless the payoff is clear.

3. **Prioritize provider code + parsing code**
   - These are high-risk and easiest to make deterministic.
   - Keep adding Wiremock contract tests as providers are touched.

4. **Only add new Rust tests if they are deterministic on Windows**
   - No real network.
   - No real audio devices.
   - No UI interaction.

### Step 5 — Add HTTP mocking for provider logic

A lot of bugs come from request/response shape drift.

Plan:

- For Rust provider modules, add tests that spin up a local mock server (so we test real HTTP handling without hitting real APIs).
- For frontend, use MSW (Mock Service Worker) if/when we add tests that involve fetch-like behavior.

**Done when:** provider bugs are caught before shipping.

**Status:** In progress (Wiremock is set up; Whisper Server STT + Ollama LLM are covered; more providers next).

---

### Step 5.5 — (Optional) Clippy warning “ratchet” (make lints useful again)

Goal: clippy is currently noisy; we want it to be a **signal**, not a wall of text.

Important constraints:

- Do **not** attempt “fix all warnings” in one PR.
- Only do **mechanical** / behavior-preserving changes (e.g., `contains`, `clamp`, `unwrap_or_default`, removing redundant cfgs).
- Always keep `pnpm -C app check:ci` green.

Workflow for each PR:

1. Pick **1–3 small warnings** in **small files**.
2. Fix them with the simplest change.
3. Run:
   - `pnpm -C app cargo:clippy:ci`
   - `pnpm -C app cargo:test:ci` (or full `pnpm -C app check:ci` if unsure)
4. Note the rough “warning count delta” in the PR description.

Suggested low-risk warning types to prioritize:

- `manual_contains`
- `manual_clamp`
- `needless_return`
- `unwrap_or_default`
- `redundant_closure`
- trivial casts (`unnecessary_cast`)

Out of scope for this plan: making clippy warnings a CI failure.

---

### Step 6 — Expand settings normalization/migration tests (high ROI)

Goal: settings bugs are silent and persistent, so we want a strong safety net around `tauriAPI.getSettings()`.

Target file(s):

- `app/src/lib/tauri.getSettings.test.ts`

Todos:

1. Add tests that lock down **null vs missing** semantics (important: `null` often means “explicitly disabled”).
2. Add tests for **type coercion / invalid values** (e.g. numbers coming back as strings, `NaN`, out-of-range).
3. Add tests for additional **legacy shape migrations** (only if they exist in `tauri.ts` today), e.g. legacy prompt sections / profile / hotkey shapes.
4. Add tests that assert **write-back** happens when normalization fixes something (this is how we keep the store from rotting).

**Done when:** we can refactor `getSettings()` with confidence and not break upgrades for existing users.

**Status:** Done (covered null vs missing, invalid values + write-back, and current known legacy shapes; expand when new migrations are added).

---

### Step 7 — Add contract tests for 1–2 cloud providers (without real API keys)

Goal: catch “request shape drift” for real providers (OpenAI/Groq/Anthropic/etc) without hitting the network.

Approach:

- Prefer providers that already allow injecting a `reqwest::Client` and/or base URL.
- If a provider hardcodes its base URL, consider a **minimal, test-only base URL override** (or a small refactor to pass an endpoint builder) so contract tests can point at a local mock server.

Suggested targets (pick 1–2):

- STT: OpenAI-compatible STT (multipart request + response parsing)
- LLM: OpenAI-compatible LLM (JSON body contract + error parsing)

**Done when:** we have at least one “real cloud provider” covered by deterministic contract tests.

**Status:** Done (OpenAI LLM + STT contract tests via Wiremock).

---

### Step 8 — Decide and implement a coverage enforcement policy (only where it helps)

Goal: keep coverage as a guardrail, not a vanity metric.

Policy options:

1. Report-only for a while (no thresholds).
2. Add thresholds only for high-risk folders (example: settings normalization + provider request shaping).

Implementation todo:

- Choose a small set of folders to enforce (or explicitly decide “none for now”).
- If enforcing, wire thresholds into `pnpm -C app coverage` (and/or `check:ci` only after the suite is stable).

**Done when:** coverage is measured consistently and nudges improvements without being annoying.

**Decision:** High-risk scope only - `app/src/lib/tauri.ts` (settings normalization).

**Thresholds:** 50% statements/functions/lines, 40% branches (realistic for current baseline).

**Enforcement:** Thresholds apply only when running `pnpm -C app coverage` (not in `check:ci`).

**Status:** Done (thresholds implemented in `vite.config.ts` for settings normalization logic).

---

## Rolling TODO list (single source of truth)

If we stop work and come back later, this section should contain *everything that remains*.

### Provider contract tests (Wiremock)

- [x] Add Wiremock as Rust dev dependency
- [x] STT: Whisper Server provider contract tests
- [x] LLM: Ollama provider contract tests
- [x] Add contract tests for 1–2 cloud providers (see Step 7)

### Frontend settings normalization tests

- [x] Expand `tauriAPI.getSettings()` tests for null vs missing
- [x] Expand `tauriAPI.getSettings()` tests for invalid values + write-back
- [x] Expand `tauriAPI.getSettings()` tests for any remaining legacy shapes

### Coverage policy

- [x] Decide report-only vs thresholds (high-risk scope: tauri.ts)
- [x] Define scope (app/src/lib/tauri.ts - settings normalization)
- [x] Wire into vite.config.ts (thresholds: 50/40/50/50)
- [x] Document policy in TESTING_AND_QUALITY_PLAN.md

### Frontend unit tests (next batch)

- [x] Add `formatErrorMessage(...)` tests (`app/src/lib/formatError.test.ts`)
- [x] Expand `textDiff` tests for Unicode + whitespace edge cases (`app/src/lib/textDiff.test.ts`)

### Rust isolation / deterministic tests (next batch)

- [ ] Do a quick audit list of “hard IO” hot spots and note the smallest test seams (base URL overrides, small traits)
- [ ] Add 1–2 more deterministic provider contract tests when a provider is touched (Wiremock)

### Clippy warning ratchet (optional but recommended)

- [x] Remove redundant Windows cfg attribute (duplicated cfg)
- [x] Fix a few low-risk mechanical warnings (`contains`, `clamp`, etc.)
- [ ] Keep reducing warnings in small batches (1–3 per PR)

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
- **Tauri E2E is harder than web E2E**: this plan intentionally avoids it.

---

## Success checklist (how we know we’re winning)

- [x] `check:ci` is the canonical local + CI command
- [ ] Frontend has a growing set of unit tests in the highest-risk modules
- [ ] Rust tests are stable across platforms
- [ ] Coverage reports exist and are acted on

### Explicit non-goal

- We are **not** requiring E2E smoke flows as part of reaching “ideal tests” for this plan.
