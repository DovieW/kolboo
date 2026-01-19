# Medium-priority refactors (opportunistic)

These are worthwhile, but I’d usually do them **when you’re already working nearby**, or when you have a dedicated cleanup window.

They tend to improve maintainability and reduce future friction, but they’re not as directly “risk reducing” as the high-impact list.

## Overlay UI (React)

- **Split `app/src/OverlayApp.tsx` into smaller modules.**
  - Suggested extraction targets:
    - `RecordingControl` (top-level controller)
    - `BackendAudioWave` + rendering helpers
    - `AudioWave` (browser analyser fallback)
    - Hover gating logic (mouse tracking + suppress-on-show)

- **Consider a single “overlay controller” state object.**
  - Right now some state lives in refs, some in `useState`, some in the reducer.
  - A follow-up could consolidate more of this into one predictable state machine, but that’s a larger change.

## Biggest “hot spot” files by size (worth refactoring)

These are the files that are _currently_ the largest / most responsibility-dense. They aren’t “bad”, but they’re the most likely to become painful to change.

### Rust backend

- **Split `app/src-tauri/src/pipeline.rs` (~188KB).**
  - Why: it contains unrelated concerns (provider construction/caching, routing logic, state machine + config, embedding cache/persistence, and helper utilities).
  - Suggested splits:
    - `pipeline/state.rs` (state machine + transitions/guards)
    - `pipeline/config.rs` (PipelineConfig defaults + normalization)
    - `pipeline/providers.rs` (STT/LLM provider creation + caching)
    - `pipeline/router/*` (embeddings router + LLM router + diagnostics payload building)
  - Bonus: lots of helper functions here are pure (e.g. path normalization / routing scoring) and can get fast unit tests once extracted.

### Frontend (React/TS)

- **Continue splitting `app/src/OverlayApp.tsx` (~81KB).**
  - This is already tracked above, but size-wise it’s still one of the top hotspots.

## Testing “ideal state” follow-ups (optional)

- **Raise TS per-file coverage thresholds gradually.**
  - Current: we’ve started ratcheting the Tauri client modules upward (e.g. `app/src/lib/tauri/commands.ts`, `app/src/lib/tauri/settings.ts`).
  - Next suggested targets from the plan: **80% lines**, **70% branches** (only if the churn feels manageable).

## Lint rule ratchet (Biome)

- **Re-enable stricter Biome rules gradually (ratchet).**
  - To get CI stable, we temporarily downgraded several high-churn rules to warnings in `app/biome.json`.
    - Hook dependency checks: `lint/correctness/useExhaustiveDependencies`
    - “Unknown data” typing noise: `lint/suspicious/noExplicitAny`
    - A11y rules that require larger UI refactors: `lint/a11y/*` (semantic buttons, ARIA checks, media captions)
    - Security/XSS rule that needs a more deliberate audit: `lint/security/noDangerouslySetInnerHtml`
    - Non-null assertions: `lint/style/noNonNullAssertion`
    - Some style/complexity preferences: `lint/style/useTemplate`, `lint/style/useExponentiationOperator`, `lint/complexity/useOptionalChain`
  - Follow-up approach:
    - Pick one rule at a time (e.g. `lint/correctness/useExhaustiveDependencies`) and fix the existing findings.
    - Flip it back to `error` once the repo is clean.
  - Goal: keep CI green while steadily improving quality instead of “big bang” lint migrations.
