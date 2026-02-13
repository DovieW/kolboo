# Implementation Plan: Disable Profile Toggle

**Branch**: `002-profile-disable-toggle` | **Date**: 2026-01-25 | **Spec**: `specs/002-profile-disable-toggle/spec.md`
**Input**: Feature specification from `/specs/002-profile-disable-toggle/spec.md`

## Summary

Add a per-profile "Disable profile" toggle in the Profile config modal so a profile can be temporarily excluded from activation (and immediately deactivated if currently active), while still being visible/editable and persisted across restarts. Disabled profiles should appear greyed out and crossed out in the profile selector dropdown.

Also rename the existing "Disable all overrides" button to "Reset profile" (behavior unchanged: clears per-profile overrides back to inherit/baseline).

## Technical Context

**Language/Version**: TypeScript (strict) + Rust (Tauri)
**Primary Dependencies**: React/Vite UI, Mantine UI components, TanStack Query, Tauri commands/events, `@tauri-apps/plugin-store`
**Storage**: Tauri store `settings.json` (canonical persisted settings)
**Testing**: Vitest (`pnpm -C app test`), Rust tests (`pnpm -C app cargo:test`), CI gate (`pnpm -C app check:ci`)
**Target Platform**: Windows desktop (primary)
**Project Type**: Desktop app (Tauri)
**Performance Goals**: N/A (small UI + settings change)
**Constraints**: Must be deterministic and backward compatible with existing `settings.json` shapes
**Scale/Scope**: Small feature touching profile settings model + activation filtering

## Constitution Check

_GATE: Must pass before Phase 0 research. Re-check after Phase 1 design._

- [x] Deterministic tests: no real network calls in tests; no real API keys required by default
- [x] UI↔backend contract: any command/event/type changes are updated in BOTH Rust and TypeScript
- [x] Settings discipline: any settings additions/changes include migrations/normalization and apply immediately at runtime
- [x] Secrets hygiene: no logging of secrets; redact sensitive data in logs
- [x] Tooling gate: plan includes how you’ll keep `pnpm -C app check:ci` green

## Project Structure

### Documentation (this feature)

```text
specs/002-profile-disable-toggle/
├── plan.md              # This file (/speckit.plan command output)
├── research.md          # Phase 0 output (/speckit.plan command)
├── data-model.md        # Phase 1 output (/speckit.plan command)
├── quickstart.md        # Phase 1 output (/speckit.plan command)
├── contracts/           # Phase 1 output (/speckit.plan command)
└── tasks.md             # Phase 2 output (/speckit.tasks command - NOT created by /speckit.plan)
```

### Source Code (touchpoints)

```text
app/
├── src/
│   ├── components/settings/ProgramsModal.tsx    # "Profile config" modal UI + reset button
│   ├── lib/tauri/types.ts                       # RewriteProgramPromptProfile TS type
│   ├── lib/tauri/settings.ts                    # settings.json normalization
│   └── lib/queries.ts                           # mutation triggers pipeline config sync
└── src-tauri/
    ├── src/settings.rs                          # RewriteProgramPromptProfile Rust type
    ├── src/bootstrap/mod.rs                     # stored profiles → runtime candidates
    ├── src/commands/config.rs                   # config snapshot path
    ├── src/pipeline/program_profiles.rs         # selection logic (consumes candidates)
    ├── src/tests/rewrite_program_profile_schema_tests.rs
    └── gen/schemas/rewrite-program-profile.schema.json
```

**Structure Decision**:

- UI updates profiles through `tauriAPI.updateRewriteProgramPromptProfiles(...)` (via `useUpdateRewriteProgramPromptProfiles()`), which persists to `settings.json` and then calls `configAPI.syncPipelineConfig()`.
- Backend builds runtime `ProgramPromptProfile` candidates from stored `RewriteProgramPromptProfile` settings during bootstrap and pipeline config sync.

## Phase 0: Research

Research output: `specs/002-profile-disable-toggle/research.md`

## Phase 1: Design & Contracts

Design outputs:

- Data model: `specs/002-profile-disable-toggle/data-model.md`
- Contracts: `specs/002-profile-disable-toggle/contracts/*`
- Quickstart: `specs/002-profile-disable-toggle/quickstart.md`

## Phase 2: Implementation Outline (for /speckit.tasks)

### 1) Types + normalization (UI)

- Add `disabled?: boolean` to `RewriteProgramPromptProfile` in `app/src/lib/tauri/types.ts`.
- Update `normalizeRewriteProfile(...)` in `app/src/lib/tauri/settings.ts` to read `disabled` and default missing/invalid to `false`.
- Update TS fixtures/tests that construct sample profiles.

### 2) UI controls

- Add a "Disable profile" toggle in `ProgramsModal.tsx` for non-default profiles.
- Persist toggle by updating the selected profile via `useUpdateRewriteProgramPromptProfiles()`.
- Visually indicate disabled state.
- In the profile selector dropdown, render disabled profiles greyed out and crossed out.

### 3) Reset profile rename

- Rename "Disable all overrides" → "Reset profile" in UI copy and confirmation dialog.
- Keep reset logic unchanged (clear override fields to `null`, keep program matching intact).

### 4) Backend filtering + schema

- Add `disabled` to Rust `RewriteProgramPromptProfile` in `app/src-tauri/src/settings.rs`.
- Filter disabled profiles out when building runtime `ProgramPromptProfile` candidates (e.g. `bootstrap/mod.rs`, and any config snapshot code path).
- Regenerate/update schema lockfile and keep schema tests passing.

### 5) Immediate deactivation

- Ensure pipeline clears/reselects active profile if it becomes disabled after `syncPipelineConfig()`.

### 6) Tests + CI

- Add deterministic unit tests:
  - TS: missing `disabled` normalizes to `false`
  - Rust: disabled profiles are never selected as candidates
- Run `pnpm -C app test`, then `pnpm -C app check:ci`.
