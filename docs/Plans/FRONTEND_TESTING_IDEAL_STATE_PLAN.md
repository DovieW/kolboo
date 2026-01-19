# Frontend Testing “Ideal State” Plan (No E2E)

**Audience:** Dovie + future contributors

This doc is specifically about the **TypeScript/React frontend**.

It complements:

- `docs/Plans/TESTING_IDEAL_STATE_PLAN.md` (repo-wide “ideal state”)
- `docs/Plans/TESTING_AND_QUALITY_PLAN.md` (how we actually implement + run checks)

## The real question: “80% coverage, or just high-value code?”

**Answer:** for this repo, “ideal” is **high confidence in high-value code**, not “80% everywhere”.

Why:

- The UI has lots of **rendering glue** where chasing coverage can become busywork.
- The repo’s biggest real risks are **contract drift**, **settings/migrations**, and **event/command wiring**.
- We can get most of the safety of E2E _without_ E2E by testing:
  - reducers/state machines
  - query functions
  - tauri invoke wrappers
  - settings normalization/migrations

So: we use **coverage thresholds as a ratchet** on a curated set of critical modules, and we measure overall app coverage only as a rough trend.

## What “ideal frontend tests” means (definition of done)

We’re “ideal” when all of the following are true:

1. **Contract safety is boring**
   - If Rust adds/renames an event/command/schema, tests fail fast with a clear message.

2. **Settings safety is locked down**
   - Legacy settings shapes are migrated/normalized deterministically.
   - “missing vs null” behavior is tested and preserved.
   - When normalization fixes data, it writes back (so installs don’t stay rotten).

3. **UI behavior is tested as logic, not clicks**
   - Overlay logic is tested as a reducer/state machine.
   - Query functions that call `tauriAPI` are tested without mounting React.
   - We don’t rely on E2E click automation.

4. **Coverage gates exist only where they help**
   - Per-file thresholds exist for Tier-0/Tier-1 modules.
   - Thresholds are enforced only when running `pnpm -C app coverage` (keep the normal loop fast).

## Testing layers (frontend)

### Layer 1: Pure unit tests (highest ROI)

Test pure functions/modules with no DOM, no timers, no network.

Examples:

- settings normalization and migrations
- payload shaping for `invoke()`
- reducers/state machines
- parsing/formatting utilities

### Layer 2: “Integration-lite” tests (mocked Tauri)

Test that wrappers call the right Tauri commands/events.

- Mock `@tauri-apps/api/core` `invoke`
- Mock `@tauri-apps/api/event` `emit/listen`
- Mock `@tauri-apps/plugin-store` `Store.load()`

This catches:

- wrong command names
- wrong payload keys
- forgetting to emit `settings-changed`
- forgetting to call `configAPI.syncPipelineConfig()` after settings changes

### Layer 3 (optional): Minimal React component tests

Only when there’s meaningful behavior that _cannot_ be expressed as pure logic.

Default posture: **avoid adding React testing libraries** unless we have a concrete need.

If we do add them later, keep the scope tiny and focus on a couple of critical components.

## Coverage philosophy (pragmatic)

### Do not target “80% of the entire app” yet

Right now, overall app coverage will stay low because most components aren’t exercised by unit tests.
That’s fine.

### Do target “high confidence in high-value modules”

We classify frontend code into tiers and enforce coverage thresholds per tier.

#### Tier 0 (must be protected)

These files are the _contract glue_ and _data correctness_.

- `app/src/lib/tauri/settings.ts` (settings normalization/migrations)
- `app/src/lib/tauri/commands.ts` (invoke wrappers)
- `app/src/lib/tauri/events.ts` (typed event map + helpers)
- `app/src/lib/contracts/**` (contract tests)

Suggested thresholds (start where we are; ratchet upward):

- lines/statements/functions: **80%+**
- branches: **70%+**

#### Tier 1 (high-value logic)

- `app/src/lib/queries/**` (queryFns, etc.)
- `app/src/lib/overlay/**` (overlay reducer/event mapping)
- any “business rules” modules extracted from UI components

Suggested thresholds:

- lines/statements/functions: **70%+**
- branches: **60%+**

#### Tier 2 (UI rendering)

- `app/src/components/**`
- screen components

No coverage thresholds by default.
If a component has tricky behavior, extract logic into Tier-1 modules and test that.

## Concrete workstreams (how we get there)

### Workstream A — Make the Rust↔TS contract unbreakable

Goal: shipping “Rust changed but UI didn’t” becomes hard.

Actions:

- Keep/expand:
  - command-name contracts
  - event-name contracts
  - schema parity checks

Success looks like:

- Renaming an event/command breaks tests immediately.
- The failing output tells Dovie exactly what changed.

### Workstream B — Settings: treat migrations as a product feature

Actions:

- Add/maintain a small set of **legacy fixtures** in tests.
- Every time we add a new migration rule, we add one test.

Success looks like:

- Upgrading from old `settings.json` shapes doesn’t silently change behavior.

### Workstream C — Overlay behavior as a state machine

Actions:

- Keep overlay logic in `app/src/lib/overlay/**` (reducer + event mapping)
- Test sequences like:
  - “poll says idle” but event says recording
  - “cancel” mid-transition
  - rapid start/stop events

Success looks like:

- Overlay doesn’t flicker or get stuck due to ordering/races.

### Workstream D — Queries: test queryFns directly

Actions:

- Structure query logic so the core is a pure function that accepts a `tauriAPI` shape.
- Unit test:
  - correct tauri calls
  - response mapping
  - error behavior (especially formatting)

Success looks like:

- Data-fetch behavior is stable without having to render React.

## Phased rollout (recommended)

This plan should be implemented in phases.

Even a very capable AI agent can do a lot quickly, but doing everything at once creates a giant diff, makes review painful, and increases the chance of “tests pass locally but something subtle broke” problems.

### Phase 0 — Baseline + boundaries

Outcome:

- This doc stays aligned with reality.
- We agree on tiers and which files are thresholded.

Acceptance criteria:

- Tier-0/Tier-1 module list is up to date.
- No new test tooling added.

### Phase 1 — Tier 0: tauri glue is locked down (1–3 PRs)

Targets:

- `app/src/lib/tauri/commands.ts`
- `app/src/lib/tauri/settings.ts`
- `app/src/lib/tauri/events.ts`
- `app/src/lib/contracts/**`

Work:

- Add/expand deterministic tests that validate:
  - `invoke()` command names + payload shapes
  - event emit/listen names + payload shapes
  - settings write side effects (`settings-changed`, `syncPipelineConfig()` where required)
- Add per-file coverage thresholds _only after_ tests exist.

Acceptance criteria:

- Tier-0 modules meet their thresholds when running `pnpm -C app coverage`.
- `pnpm -C app test` stays fast and stable.

#### Phase 1 progress tracker (started 2026-01-19)

- [x] Add basic invoke wrapper tests for `app/src/lib/tauri/commands.ts` (Dovie: started with high-signal wrappers).
- [x] Add typed event helper tests for `app/src/lib/tauri/events.ts`.
- [x] Add settings side-effect tests for `app/src/lib/tauri/settings.ts` (emit + invoke + store writes).
- [x] Expand command wrapper coverage (more invoke wrappers + payload shapes).
- [x] Add per-file coverage thresholds for `commands.ts`, `settings.ts`, `events.ts` (baseline thresholds; ratchet up later).
- [x] Verify `pnpm -C app test` and `pnpm -C app coverage` are green for Dovie.

**Phase 1 status:** ✅ Complete (baseline thresholds set; next step is ratcheting up coverage).

### Phase 2 — Tier 1: overlay behavior as a state machine

Targets:

- `app/src/lib/overlay/**`

Work:

- Add state-machine style tests for event ordering/race-y sequences.
- Only add thresholds once the reducer/event-mapping logic is truly extracted from React components.

Acceptance criteria:

- Overlay reducer + event mapping has deterministic tests for the known tricky sequences.

#### Phase 2 progress tracker (started 2026-01-19)

- [x] Add reducer sequence tests for poll vs event ordering and rapid transitions.
- [ ] Add event-mapping tests if/when we extract more overlay event glue.

**Phase 2 status:** ✅ Complete for reducer sequences (event glue unchanged).

### Phase 3 — Tier 1: queries tested without React

Targets:

- `app/src/lib/queries/**`

Work:

- Test queryFns by injecting/mocking `tauriAPI`.
- Avoid mounting components.

Acceptance criteria:

- Critical queryFns (settings, pipeline state, history) have stable unit tests.

#### Phase 3 progress tracker (started 2026-01-19)

- [x] Expand queryFn unit tests (cost summaries, history, settings, request logs).
- [ ] Add tests for additional critical queryFns as they are extracted.

**Phase 3 status:** ✅ Complete for baseline queryFns.

### Phase 4 — Optional: minimal component tests (only if necessary)

Default posture: avoid this.

Only do it if we hit important UI behavior that cannot be expressed as pure logic.

Acceptance criteria:

- Any added React component tests are small, deterministic, and focused.
- We don’t end up writing “click flow” tests that act like pseudo-E2E.

**Phase 4 status:** ⏸️ Deferred (no component tests needed yet, Dovie).

## How we measure progress (simple metrics)

We track the stuff that matters:

- % of Tier-0 modules meeting their thresholds
- Contract test suite stability (no flakes)
- “Bugs prevented” anecdotes (settings drift, wrong command names, etc.)

We do **not** treat overall app coverage % as the primary KPI.

## Commands (what to run)

- `pnpm -C app test` (fast, normal loop)
- `pnpm -C app coverage` (only when touching thresholded modules)
- `pnpm -C app check:ci` (what CI cares about)

## Dumbed-down summary (for future-Dovie)

Don’t chase a big coverage number across the whole UI.

Instead:

- Put tests around the **glue** (tauri wrappers + events + settings).
- Put tests around the **brains** (reducers, queryFns, normalization).
- Keep UI rendering mostly untested unless it’s truly tricky.

That gives you high confidence without brittle click-based testing.
