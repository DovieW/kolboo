# Testing “Ideal State” Plan (No E2E)

**Audience:** Dovie + future contributors

## Why this exists

Dovie wants “all the important stuff covered by tests” **without** end-to-end (E2E) UI automation.

This doc lays out a practical path to an “ideal state” for testing in Kolboo:

- High confidence in core behavior
- Deterministic tests (no real network, no real audio hardware required)
- Clear quality gates in CI
- Fast feedback loops for day-to-day work

## Scope / non-goals

### In scope

- Unit tests (TS + Rust)
- Contract tests (Rust↔TS event names, command names, schema alignment)
- “Integration-lite” tests using mocks (Wiremock / stub clients) that validate HTTP requests and error handling
- Component/Hook-level tests **only when they are deterministic**

### Explicitly out of scope

- Browser automation E2E (Playwright/Cypress) for full UI flows
- Tests that require real API keys by default
- Tests that require actual microphones/audio devices by default

> Note: We can still keep a _small_ set of manual `#[ignore]` Rust tests for “real API smoke checks”, but they must not run in CI.

## What counts as “important” (the app’s critical surfaces)

If these are correct, the app is usually “safe”:

1. **UI ↔ Rust contract**
   - command names and payload shapes
   - event names and payload shapes

2. **Settings**
   - persisted keys exist / defaults seeded
   - migrations + normalization are correct
   - `null` vs “missing” semantics are preserved

3. **Pipeline safety**
   - state transitions are valid
   - cancel/reset behavior never deadlocks
   - timeouts + limits behave sensibly

4. **Provider IO (network)**
   - correct request bodies/headers
   - non-2xx error bodies surface as useful errors
   - provider options (model, prompt, base URL) behave

5. **Data IO (disk)**
   - history / recordings / logs: read/write/delete correctness
   - schema compatibility for stored data

6. **OS integration seams (best-effort, deterministic-only)**
   - hotkey normalization and duplicate detection
   - output mode logic (paste/keystrokes) as pure functions where possible

## Current state snapshot (what we already have)

### TypeScript (Vitest)

Strengths:

- **Settings normalization is tested** (`app/src/lib/tauri.getSettings.test.ts`).
- **Contract tests exist**:
  - Rust emits vs TS `EVENT_NAMES` (`app/src/lib/contracts/eventsNameContract.test.ts`).
  - TS types match Rust JSON schemas (`app/src/lib/contracts/schemas/*`).
- Some pure logic tests exist (e.g. hotkey utilities).

Gap:

- Minimal React UI coverage (components/screens/hooks mostly untested).

### Rust (cargo test)

Strengths:

- Pipeline edge-case tests exist (`app/src-tauri/src/tests/pipeline_edge_case_tests.rs`).
- Provider HTTP logic is tested using `wiremock` (good, deterministic).

Gap:

- Not much direct test coverage for command handlers / orchestration in `app/src-tauri/src/lib.rs` and `app/src-tauri/src/commands/**`.

### Coverage enforcement

- Only one per-file TS threshold is enforced today: `src/lib/tauri.ts` at ~50%.

## Target “ideal state” (definition of done)

This is the bar we’re aiming for.

### ✅ Contract safety

- Every Rust-emitted event name is represented in TS (already done; keep it).
- Every Tauri command name invoked from TS is defined in Rust.
- Event payload shapes and command response shapes remain validated against generated JSON schemas.

### ✅ Settings safety

- Every settings key seeded by Rust defaults has a TS normalization rule or an explicit “backend-only” allow-list entry.
- Every migration path is tested with at least one real “legacy settings” fixture.

### ✅ Pipeline safety

- Pipeline state transitions are tested for:
  - valid paths
  - invalid paths (must return typed errors, not panic)
  - cancellation during each active phase
  - concurrency safety (no deadlocks)

### ✅ Provider safety

- Each provider has deterministic tests that validate:
  - request shape
  - base URL overrides
  - error surfacing

### ✅ Data IO safety

- History and recording storage logic has deterministic tests using temp dirs.

### ✅ UI correctness (without E2E)

- We don’t test “click flows” end-to-end, but we do test:
  - hook/query logic
  - reducers/state machines
  - event handling glue
  - settings update side effects (emit + `syncPipelineConfig()`)

### ✅ Coverage gates (pragmatic)

We enforce higher coverage only for “critical files”, not the entire UI.

Example targets (adjust as we learn):

- `app/src/lib/tauri.ts`: **80% lines**, **70% branches**
- `app/src/lib/queries.ts`: **70% lines**
- `app/src/lib/tauri/events.ts` (if present): **80% lines**

## Workstreams (high-payoff changes)

### Workstream A — Strengthen TS↔Rust contracts (no UI libs needed)

**Goal:** prevent “renamed command / renamed event / payload drift” bugs.

1. **Command name contract test (NEW)**

- Add a TS contract test that:
  - scans Rust `#[tauri::command]` registrations (or command function names in `app/src-tauri/src/commands/**`)
  - compares against the set of command names used by `app/src/lib/tauri.ts` wrappers.

Acceptance:

- If TS calls a command that Rust doesn’t provide, tests fail.

2. **Event payload schema contract (tighten)**

- Continue validating payload schemas in both Rust and TS.
- Add “smoke” examples for the trickiest event payloads (pipeline + overlay) so drift is obvious.

### Workstream B — Make `tauri.ts` testable by design

**Goal:** every wrapper has a test that verifies `invoke()` arguments.

1. Split “invoke wrappers” from “settings normalization” (optional but recommended)

- Current hotspot file: `app/src/lib/tauri.ts`.
- The ideal shape is:
  - `lib/tauri/commands.ts` (thin wrappers)
  - `lib/tauri/settings.ts` (normalize + migration + persistence)
  - `lib/tauri/events.ts` (emit/listen helpers)

2. Add wrapper tests

- For each exported API group (`configAPI`, etc.) add tests that assert:
  - correct command name
  - correct payload keys
  - correct post-actions (when applicable), e.g.:
    - “persist setting AND call `configAPI.syncPipelineConfig()`”
    - “persist setting AND emit `settings-changed`”

Acceptance:

- At least one test per command wrapper.
- Coverage threshold increases for the file(s) that own wrappers.

### Workstream C — Test UI logic without React E2E

**Goal:** cover the _behavior_ without needing DOM click automation.

Strategy: shift “important behavior” into testable pure modules.

1. Overlay behavior as a reducer/hook

- Extract overlay state machine/reducer logic out of `app/src/OverlayApp.tsx` into a module like:
  - `app/src/lib/overlay/overlayUiReducer.ts`
  - `app/src/lib/overlay/overlayEvents.ts` (maps backend events → reducer actions)

2. Test it like a state machine

- Provide tests that feed in sequences of events and assert the resulting state.
- This catches the classic bugs:
  - event arrives before polling update
  - stale state after rapid cancel
  - hide/show gating

Acceptance:

- Overlay logic has deterministic tests with event sequences.

3. Query/hook logic

- For `app/src/lib/queries.ts`:
  - extract queryFns into pure functions (accepting `tauriAPI` as parameter)
  - test queryFns directly (no need to mount React)

Acceptance:

- At least the critical queries are covered (pipeline state, settings, history).

### Workstream D — Rust command & orchestration test seams

**Goal:** avoid “Tauri runtime required” by moving logic into testable functions.

1. Move logic out of `lib.rs`

- Keep `lib.rs` as wiring.
- Put real behavior into modules with small dependency interfaces.

2. Add unit tests for command logic

- For the most important commands (start/stop/cancel, settings sync, overlay show/hide):
  - test “given input, what state changes and what events are emitted?”
  - use an event sink abstraction for tests (collect emitted events into a vec)

Acceptance:

- Core orchestration can be tested without a full Tauri app.

### Workstream E — Pipeline + IO reliability (Rust)

**Goal:** pipeline is safe under stress; disk IO is correct.

1. Pipeline transitions

- Add tests for:
  - cancelling during recording/transcribing/routing/rewriting
  - repeated cancel
  - error recovery paths

2. Disk IO with temp dirs

- Add tests for history/recordings/logs using a temp directory.

Acceptance:

- Disk behavior is deterministic and doesn’t touch real user directories.

## Testing tools policy (keeping things simple)

Dovie doesn’t want E2E.

### Preferred approach

- **No new frontend test libraries** until we need them.
- Instead, refactor “important logic” into pure modules and test those.

### Optional upgrade (only if we truly need component tests)

If we later decide to test a couple of React components directly, add:

- `@testing-library/react`
- `@testing-library/jest-dom`
- `jsdom` (or `happy-dom`)

But treat this as a deliberate decision. If we do it, keep tests small and focus on critical components only.

## CI quality gates (how we know we’re safe)

### Required commands

- `pnpm -C app test`
- `pnpm -C app coverage` (for enforced thresholds)
- `pnpm -C app check:ci`

### Coverage ratchet (recommended)

- Increase per-file thresholds gradually:
  1. raise thresholds for `src/lib/tauri.ts` (or its split modules)
  2. add thresholds for `src/lib/queries.ts`
  3. add thresholds for overlay glue modules once extracted

This avoids the “big bang 80% everywhere” trap.

## Rollout plan (phased)

### Phase 1 — Contract and wrapper safety (fast win)

- Add command-name contract test
- Add wrapper tests for the highest-traffic commands
- Raise coverage threshold for wrapper module(s)

### Phase 2 — Overlay + settings behavior

- Extract overlay reducer/events glue
- Add state-machine tests
- Add tests for settings update side effects (emit + sync)

### Phase 3 — Backend orchestration

- Add event-sink seams
- Move logic from `lib.rs` into testable modules
- Add unit tests around core commands

### Phase 4 — IO hardening

- Temp-dir tests for history/recordings/logs

## Dumbed-down explanation (so future-Dovie doesn’t hate present-Dovie)

We’re not going to automate clicking around the app.

Instead, we’ll:

- **test the important “glue”** (Rust↔TS contracts)
- **test the important “brains”** (settings normalization, pipeline rules)
- **test the scary networking bits** (providers) using fake servers
- **test UI behavior as plain functions** (reducers/query functions), not as a full browser

That gets most of the confidence of E2E, without the flakiness.
