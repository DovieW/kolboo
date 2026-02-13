---

description: "Tasks for Disable Profile Toggle"
---

# Tasks: Disable Profile Toggle

**Input**: Design documents from `specs/002-profile-disable-toggle/`

- `specs/002-profile-disable-toggle/spec.md`
- `specs/002-profile-disable-toggle/plan.md`
- `specs/002-profile-disable-toggle/research.md`
- `specs/002-profile-disable-toggle/data-model.md`
- `specs/002-profile-disable-toggle/contracts/`
- `specs/002-profile-disable-toggle/quickstart.md`

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- Every task includes explicit file path(s)

## Phase 1: Setup (Shared Infrastructure)

- [ ] T001 Confirm feature docs are present under `specs/002-profile-disable-toggle/` (spec.md, plan.md, research.md, data-model.md, contracts/, quickstart.md)
- [ ] T002 [P] Identify all UI strings to rename by searching `app/src/components/settings/ProgramsModal.tsx` for "Disable all overrides" and related dialog copy
- [x] T001 Confirm feature docs are present under `specs/002-profile-disable-toggle/` (spec.md, plan.md, research.md, data-model.md, contracts/, quickstart.md)
- [x] T002 [P] Identify all UI strings to rename by searching `app/src/components/settings/ProgramsModal.tsx` for "Disable all overrides" and related dialog copy

## Phase 2: Foundational (Blocking Prerequisites)

> These tasks create the shared data plumbing for the new `disabled` profile flag.

- [x] T003 [P] Add `disabled?: boolean` to `RewriteProgramPromptProfile` in `app/src/lib/tauri/types.ts`
- [x] T004 [P] Normalize persisted `disabled` (missing/invalid → false) in `app/src/lib/tauri/settings.ts` (`normalizeRewriteProfile(...)`)
- [x] T005 [P] Update TypeScript contract/fixtures to include/allow `disabled` in `app/src/lib/settingsContract.test.ts` and `app/src/lib/tauri.getSettings.test.ts`
- [x] T006 Add `disabled` field to Rust `RewriteProgramPromptProfile` in `app/src-tauri/src/settings.rs` (serde default so missing → false)
- [x] T007 Filter disabled profiles out when building runtime candidates in `app/src-tauri/src/bootstrap/mod.rs` (stored profiles → `ProgramPromptProfile` list)
- [x] T008 Filter disabled profiles out in any config snapshot path that builds program profiles in `app/src-tauri/src/commands/config.rs`
- [x] T009 Update schema lockfile for `RewriteProgramPromptProfile` in `app/src-tauri/gen/schemas/rewrite-program-profile.schema.json` and keep schema test passing in `app/src-tauri/src/tests/rewrite_program_profile_schema_tests.rs`

**Checkpoint**: At this point, profiles can persist `disabled`, and the backend will not consider disabled profiles for activation.

---

## Phase 3: User Story 1 - Temporarily disable a profile (Priority: P1) 🎯 MVP

**Goal**: A user can disable a profile so it never activates, and if it was currently active it deactivates immediately.

**Independent Test**: Using `specs/002-profile-disable-toggle/quickstart.md`, disable one profile and confirm it never becomes active and persists across restart.

### Tests (recommended)

- [x] T010 [P] [US1] Add TS tests for profile `disabled` normalization in `app/src/lib/tauri.getSettings.test.ts` (missing → false, true preserved, false preserved)
- [x] T011 [P] [US1] Add Rust unit test ensuring disabled profiles are not selected for program matching in `app/src-tauri/src/pipeline/program_profiles.rs` (or a focused test helper module under `app/src-tauri/src/tests/`)

### Implementation

- [x] T012 [P] [US1] Add a "Disable profile" toggle to the profile config modal UI in `app/src/components/settings/ProgramsModal.tsx`
- [x] T013 [P] [US1] Persist toggle changes by updating the selected profile (set `disabled: true/false`) via `app/src/lib/queries.ts` (`useUpdateRewriteProgramPromptProfiles`) from `app/src/components/settings/ProgramsModal.tsx`
- [x] T014 [P] [US1] Add clear disabled-state UI affordance in `app/src/components/settings/ProgramsModal.tsx` (e.g., text/badge/disabled styling)
- [x] T015 [US1] Ensure disabling a currently-active profile immediately deactivates it by validating/clearing active profile during config sync in `app/src-tauri/src/pipeline.rs` (and any related state in `app/src-tauri/src/pipeline/transcription_flow.rs`)
- [x] T023 [P] [US1] Grey out and strike through disabled profiles in the profile selector dropdown UI in `app/src/App.tsx`

**Checkpoint**: A disabled profile never activates and disabling an active profile deactivates immediately.

---

## Phase 4: User Story 2 - Re-enable a disabled profile (Priority: P2)

**Goal**: A user can re-enable a profile using the same toggle, and it becomes eligible for activation again.

**Independent Test**: Disable then re-enable a profile and confirm it can become active again (per `specs/002-profile-disable-toggle/quickstart.md`).

- [x] T016 [P] [US2] Add/extend TS tests to cover toggling disabled true → false in `app/src/lib/tauri.getSettings.test.ts` (disabled persists and can be set back)
- [x] T017 [US2] Verify UI allows toggling back off and that re-enabled profiles can be selected again by activation logic (touchpoints: `app/src/components/settings/ProgramsModal.tsx`, `app/src-tauri/src/pipeline/program_profiles.rs`)

---

## Phase 5: User Story 3 - Reset a profile’s overrides (Priority: P3)

**Goal**: Rename "Disable all overrides" to "Reset profile" (behavior unchanged).

**Independent Test**: Click "Reset profile" and confirm override fields clear but program matching and disabled/enabled state remain unchanged.

- [x] T018 [P] [US3] Rename button text and confirmation dialog copy in `app/src/components/settings/ProgramsModal.tsx` from "Disable all overrides" to "Reset profile" (keep reset logic identical)
- [x] T019 [US3] Ensure the reset implementation does not modify `program_paths` or `disabled` in `app/src/components/settings/ProgramsModal.tsx`

---

## Phase 6: Polish & Cross-Cutting Concerns

- [x] T020 [P] Reconcile docs with final behavior in `specs/002-profile-disable-toggle/quickstart.md` and `specs/002-profile-disable-toggle/contracts/*.md`
- [x] T021 Run UI tests with `app/` via `pnpm -C app test` and fix failures in `app/src/**`
- [x] T022 Run full CI gate with `app/` via `pnpm -C app check:ci` and fix failures across `app/src/**` and `app/src-tauri/src/**`

---

## Dependencies & Execution Order

### User Story completion order

- **US1 (P1)** must be completed first (it introduces the disable behavior).
- **US2 (P2)** depends on US1 (re-enable is the inverse of disable).
- **US3 (P3)** is independent of US1/US2 (rename/reset behavior), but touches the same UI file.

### Dependency graph

- Foundational (T003–T009) → US1 (T010–T015)
- US1 (T010–T015) → US2 (T016–T017)
- US3 (T018–T019) can run after Foundational (or even in parallel with US1 if carefully coordinated, since it touches the same file)

## Parallel execution examples

### Example: parallel work for US1

- TS side in parallel:
	- [P] T003 in `app/src/lib/tauri/types.ts`
	- [P] T004 in `app/src/lib/tauri/settings.ts`
	- [P] T010 in `app/src/lib/tauri.getSettings.test.ts`

- Rust side in parallel:
	- T006 in `app/src-tauri/src/settings.rs`
	- T007 in `app/src-tauri/src/bootstrap/mod.rs`
	- [P] T011 in `app/src-tauri/src/pipeline/program_profiles.rs`

## Implementation Strategy (MVP first)

- MVP = **Phase 2 + US1**: after T003–T015, the disable toggle works end-to-end and is testable.
