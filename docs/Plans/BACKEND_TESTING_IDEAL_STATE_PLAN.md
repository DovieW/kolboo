# Backend Testing “Ideal State” Plan (Rust/Tauri, No Hardware Required by Default)

**Audience:** Dovie + future contributors

This doc is specifically about the **Rust/Tauri backend** in `app/src-tauri/src/**`.

It complements:

- `docs/Plans/TESTING_IDEAL_STATE_PLAN.md` (repo-wide “ideal state”)
- `docs/Plans/TESTING_AND_QUALITY_PLAN.md` (implementation guide)

## What “ideal backend tests” means (definition of done)

We’re in a great place when:

1. **Pipeline safety is boring**
   - Valid transitions work.
   - Invalid transitions return typed errors.
   - Cancellation is correct and cannot deadlock.

2. **Provider/network behavior is deterministic**
   - Each important provider has contract tests (request shape + error parsing) using a local mock server.
   - No real API keys required by default.

3. **Disk IO is correct**
   - History, recordings, stats, backups behave correctly using temp dirs.
   - No tests write to real user directories.

4. **Tauri command orchestration is covered without a real Tauri runtime**
   - Command handlers delegate to testable functions.
   - Event emission and side effects are verified with a fake event sink.

5. **Hardware/OS integration is handled responsibly**
   - Audio capture (CPAL) and OS integrations are not required for CI.
   - A small set of manual tests exist and are `#[ignore]` with clear run instructions.

## What matters most (backend critical surfaces)

These are the backend areas where a bug is expensive:

1. **Pipeline state machine** (start/stop/cancel/reset; concurrency)
2. **Audio capture seam** (CPAL streams, callbacks, meters)
3. **Provider IO** (LLM/STT/embeddings request/response contracts)
4. **Data IO** (history, recordings, stats, backups)
5. **Settings seeding/migrations** (defaults, normalization, versioning)
6. **Event emission contract** (names + payloads; overlay + quick ask + pipeline events)

## Testing layers (backend)

### Layer 1: Pure unit tests (fast, no IO)

Target:

- pipeline guards / transition table logic
- config defaults + normalization
- small utilities (path normalization, scoring, parsing)

Expectations:

- no filesystem
- no network
- no audio

### Layer 2: Deterministic integration tests (controlled IO)

Target:

- provider contract tests via Wiremock (local server)
- filesystem logic using temp dirs

Expectations:

- no real network
- no real user directories

### Layer 3: Orchestration tests (fake runtime)

Target:

- command handlers and “glue” behavior
- event emission correctness

Approach:

- define a small event sink interface used by orchestration logic
- tests inject a fake sink and assert emitted events + payloads

### Layer 4 (manual): Hardware/real-world tests

These are explicitly **not CI**.

- audio capture smoke tests
- optional “real API provider” smoke tests (keys)

All must be:

- `#[ignore]`
- documented with “run with …”

## Coverage philosophy

We do **not** need a single backend coverage percentage to be “ideal”.

Instead, we need:

- **strong tests** around high-risk modules
- deterministic tests that run in CI

If we later want Rust coverage numbers, use that as a _trend_, not as a hard gate.

## Gaps vs current state (the punch list)

Based on existing tests, we’re strong on:

- provider request/response contracts (Wiremock)
- some pipeline guard/default behavior
- history/recordings storage correctness
- schema/contract validation

The remaining high-value gaps are mostly:

1. **Audio capture behavior** without hardware (needs a seam)
2. **Command orchestration correctness** without full Tauri runtime (needs seams)
3. **Cancellation and ordering** tests across more pipeline phases

## Phased rollout (recommended)

### Phase 0 — Inventory + map tests to surfaces

Outcome:

- A living checklist that answers: “is this critical surface covered?”

Work:

- Add a short “coverage map” section to this doc:
  - surface → which test file(s) prove it
  - surface → what’s still missing

Acceptance criteria:

- Every critical surface listed in this doc has at least one pointer (or is explicitly called out as missing).

### Phase 1 — Pipeline correctness beyond guards

Goal: pipeline is safe under cancellation and concurrency.

Work:

- Add tests for:
  - cancel during each active phase (recording/transcribing/routing/rewriting)
  - repeated cancel
  - error recovery to a safe state
  - event ordering assumptions (when applicable)

Acceptance criteria:

- The pipeline returns typed errors and does not deadlock.
- Tests do not require audio hardware.

### Phase 2 — Audio capture seam for deterministic tests

Goal: test “audio-driven behavior” without CPAL.

Work (minimal seam, high payoff):

- Introduce a tiny interface (trait) that the pipeline uses instead of calling CPAL directly.
  - Example conceptually: an `AudioCaptureBackend` that can produce “audio level” samples and/or a sample stream.
- Provide:
  - a real CPAL implementation (production)
  - a fake implementation (tests)

Acceptance criteria:

- At least one pipeline test can simulate audio level / stream behavior deterministically.
- No CI tests require real audio devices.

### Phase 3 — Command orchestration tests via event sink seams

Goal: prove the “wiring layer” is correct.

Work:

- Move logic out of Tauri command fns into testable functions.
- Add an `EventSink`/`AppEvents` abstraction so tests can assert emitted events.
- Add tests for critical commands:
  - start/stop/cancel
  - settings sync / pipeline config sync
  - overlay show/hide requests

Acceptance criteria:

- Command orchestration is covered without a running Tauri app.
- Tests assert side effects: state changes + emitted events.

### Phase 4 — Data IO expansion + retention safety

Goal: data deletion/retention cannot surprise users.

Work:

- Add temp-dir tests for:
  - request logs retention/deletion
  - stats append/list/delete behavior
  - backup export/import

Acceptance criteria:

- All tests are deterministic and don’t touch real user data.

### Phase 5 — (Optional) Add Rust coverage tooling

Only if we decide it’s helpful.

- Add a `cargo llvm-cov` script (not in default CI gate)
- Use it as a trend report and to help identify dead zones

Acceptance criteria:

- Can generate an HTML report locally.
- No new “coverage threshold” gate unless we deliberately choose one.

## Commands (what to run)

- `pnpm -C app cargo:test` (backend tests)
- `pnpm -C app check:ci` (what CI cares about)

Optional/manual:

- `cargo test --manifest-path app/src-tauri/Cargo.toml -- --ignored`

## Dumbed-down summary (for future-Dovie)

Don’t try to test real microphones in CI.

Instead:

- test the pipeline rules (especially cancel)
- test provider HTTP behavior with fake servers
- test disk IO with temp folders
- test command “wiring” by injecting fake event sinks

That covers the backend’s scary parts without flakiness.
