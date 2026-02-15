# Tasks: Phase 0 Enterprise Posture

**Input**: Design documents from `/specs/001-phase0-enterprise-posture/`
**Prerequisites**: `plan.md`, `spec.md`, `research.md`, `data-model.md`, `contracts/policy-local-api.yaml`

**Tests**: Include deterministic tests for policy normalization/enforcement, UI transparency, and diagnostics redaction.

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Establish feature scaffolding and shared type surface for Phase 0 policy posture.

- [X] T001 Add Phase 0 policy constants and shared TS types in `app/src/lib/tauri/types.ts`
- [X] T002 Add policy API wrapper skeleton in `app/src/lib/tauri/policy.ts`
- [X] T003 [P] Add policy query hook scaffold in `app/src/lib/queries.ts`
- [X] T004 [P] Add backend policy module skeleton in `app/src-tauri/src/policy.rs`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Implement core policy-state plumbing that all user stories depend on.

**⚠️ CRITICAL**: No user story implementation should start until this phase is complete.

- [X] T005 Implement `PolicyState` model and validation helpers in `app/src-tauri/src/policy.rs`
- [X] T006 Register policy commands/events and shared app state wiring in `app/src-tauri/src/lib.rs`
- [X] T007 Implement policy-state load/cache + normalization entry points in `app/src/lib/tauri/settings.ts`
- [X] T008 Wire typed policy command wrappers in `app/src/lib/tauri/commands.ts` and `app/src/lib/tauri/policy.ts`
- [X] T009 [P] Add/adjust event typings for policy-driven refresh in `app/src/lib/tauri/events.ts` and `app/src/lib/tauri/types.generated.ts`
- [X] T010 [P] Add foundational deterministic tests for policy-state validity transitions in `app/src/lib/tauri/settings.legacy.test.ts`

**Checkpoint**: Foundation ready; user stories can be implemented independently.

---

## Phase 3: User Story 1 - Admin-enforced settings baseline (Priority: P1) 🎯 MVP

**Goal**: Apply policy constraints to effective settings and block non-compliant user edits.

**Independent Test**: Apply a policy with required/blocked values and verify constrained fields stay compliant through edits and restarts.

### Tests for User Story 1

- [X] T011 [P] [US1] Add policy-enforcement normalization tests in `app/src/lib/tauri/settings.actions.test.ts`
- [X] T012 [P] [US1] Add command-wrapper behavior tests for policy state fetch/apply flow in `app/src/lib/tauri/commands.test.ts`

### Implementation for User Story 1

- [X] T013 [US1] Implement policy precedence merge into effective settings in `app/src/lib/tauri/settings.ts`
- [X] T014 [US1] Enforce blocked/fixed field mutation prevention in `app/src/lib/tauri/settings.ts`
- [X] T015 [US1] Ensure policy-driven effective changes trigger runtime sync via `configAPI.syncPipelineConfig` in `app/src/lib/tauri/settings.ts`
- [X] T016 [US1] Emit `settings-changed` after policy-driven normalization updates in `app/src/lib/tauri/settings.ts`
- [X] T017 [US1] Add backend-side policy validity checks used by commands in `app/src-tauri/src/policy.rs`

**Checkpoint**: US1 independently functional and testable.

---

## Phase 4: User Story 2 - Policy transparency for end users (Priority: P2)

**Goal**: Show users the active policy state, what is enforced, and why.

**Independent Test**: Open Policy UI and verify source/status/timestamps and enforced reasons are visible and accurate.

### Tests for User Story 2

- [X] T018 [P] [US2] Add UI tests for policy visibility and enforced reason rendering in `app/src/components/settings/PolicySettings.test.tsx`
- [X] T019 [P] [US2] Add query/wrapper tests for policy state retrieval in `app/src/lib/tauri/events.test.ts`

### Implementation for User Story 2

- [X] T020 [US2] Implement Policy screen component in `app/src/components/settings/PolicySettings.tsx`
- [X] T021 [US2] Integrate Policy screen into settings navigation/layout in `app/src/components/settings/Settings.tsx`
- [X] T022 [US2] Implement policy state query and UI binding in `app/src/lib/queries.ts` and `app/src/lib/tauri/policy.ts`
- [X] T023 [US2] Add policy-enforced metadata display fields in `app/src/lib/tauri/types.ts`

**Checkpoint**: US1 and US2 both independently functional.

---

## Phase 5: User Story 3 - Support-ready policy diagnostics (Priority: P3)

**Goal**: Export redacted policy diagnostics for support workflows without exposing secrets.

**Independent Test**: Export diagnostics and confirm policy metadata is present while sensitive fields are redacted/absent.

### Tests for User Story 3

- [X] T024 [P] [US3] Add redaction tests for diagnostics payload in `app/src-tauri/src/policy.rs`
- [X] T025 [P] [US3] Add UI flow test for diagnostics export action in `app/src/components/settings/PolicySettings.test.tsx`

### Implementation for User Story 3

- [X] T026 [US3] Implement diagnostics export command and redaction logic in `app/src-tauri/src/policy.rs`
- [X] T027 [US3] Wire diagnostics export wrapper and response typing in `app/src/lib/tauri/policy.ts` and `app/src/lib/tauri/types.ts`
- [X] T028 [US3] Add diagnostics export action and status handling in `app/src/components/settings/PolicySettings.tsx`

**Checkpoint**: All user stories independently functional.

---

## Phase 6: Polish & Cross-Cutting

**Purpose**: Final hardening, docs alignment, and CI validation.

- [X] T029 [P] Update policy behavior and support guidance docs in `docs/How Tos/` and `docs/User Docs/`
- [X] T030 Run formatting and lint gate, then apply fixes in `app/src/lib/tauri/settings.ts`, `app/src/components/settings/PolicySettings.tsx`, and `app/src-tauri/src/policy.rs`
- [X] T031 Run deterministic test suites and fix regressions in `app/src/lib/tauri/settings.actions.test.ts`, `app/src/components/settings/PolicySettings.test.tsx`, and `app/src-tauri/src/policy.rs`
- [X] T032 Run final CI gate and resolve remaining issues in `app/src/lib/tauri/commands.ts`, `app/src/lib/queries.ts`, and `app/src-tauri/src/lib.rs`

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 1 (Setup)**: starts immediately.
- **Phase 2 (Foundational)**: depends on Phase 1 and blocks all user stories.
- **Phase 3+ (User Stories)**: depend on Phase 2 completion.
- **Phase 6 (Polish)**: depends on completion of desired user stories.

### User Story Dependencies

- **US1 (P1)**: no dependency on other user stories.
- **US2 (P2)**: depends on foundational policy state plumbing; can ship after US1.
- **US3 (P3)**: depends on policy state + policy UI entry points; can ship after US2.

### Dependency Graph

- **Execution order**: `US1 -> US2 -> US3`

### Parallel Opportunities

- T003 and T004 can run in parallel.
- T009 and T010 can run in parallel.
- Within US1: T011 and T012 can run in parallel.
- Within US2: T018 and T019 can run in parallel.
- Within US3: T024 and T025 can run in parallel.
- T029 can run while validation commands are being prepared.

---

## Parallel Example: User Story 1

- Run T011 and T012 together (different test files).
- Run T013/T014 after tests are in place, then T015/T016 as integration wiring.

## Parallel Example: User Story 2

- Run T018 and T019 together (different files).
- Run T020 and T022 in parallel, then complete T021 integration.

## Parallel Example: User Story 3

- Run T024 and T025 together (backend and UI tests in different files).
- Run T026 and T027 in parallel, then complete T028 UI action wiring.

---

## Implementation Strategy

### MVP First (US1)

1. Complete Setup + Foundational.
2. Deliver US1 enforcement baseline.
3. Validate independently with US1 tests.

### Incremental Delivery

1. Add US2 policy transparency UI.
2. Add US3 diagnostics export.
3. Finish with polish and full CI gate.
