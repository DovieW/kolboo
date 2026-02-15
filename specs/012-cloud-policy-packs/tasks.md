# Tasks: Phase 2 Cloud Policy Packs

**Input**: Design documents from `/specs/012-cloud-policy-packs/`
**Prerequisites**: `plan.md` (required), `spec.md` (required for user stories), `research.md`, `data-model.md`, `contracts/`, `quickstart.md`

**Tests**: Include deterministic tests to lock in policy validation, enforcement, and outage behavior.

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Create the policy implementation skeleton and shared scaffolding.

- [x] T001 Create policy module shell in `app/src-tauri/src/policy.rs`
- [x] T002 Create policy command shell in `app/src-tauri/src/commands/policy.rs`
- [x] T003 Create TS policy wrapper module in `app/src/lib/tauri/policy.ts`
- [x] T004 [P] Add policy domain type placeholders in `app/src/lib/tauri/types.ts`
- [x] T005 [P] Wire policy module exports in `app/src-tauri/src/commands/mod.rs` and `app/src/lib/tauri.ts`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Build shared core behavior that all stories depend on.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete.

- [x] T006 Implement `PolicyPack` and `PolicyState` Rust structs + serde mapping in `app/src-tauri/src/policy.rs`
- [x] T007 [P] Implement policy validation helpers (schema/integrity/version checks) in `app/src-tauri/src/policy.rs`
- [x] T008 [P] Implement policy cache read/write helpers in `app/src-tauri/src/policy.rs` and `app/src-tauri/src/lib.rs`
- [x] T009 Implement TS policy state/type mirrors in `app/src/lib/tauri/types.ts`
- [x] T010 [P] Add settings normalization hooks for policy metadata in `app/src/lib/tauri/settings.ts`
- [x] T011 Register policy commands in `app/src-tauri/src/lib.rs`
- [x] T012 [P] Add TS invoke wrappers for policy commands in `app/src/lib/tauri/commands.ts` and `app/src/lib/tauri/policy.ts`
- [x] T013 Add policy-state event emit/listen contract in `app/src-tauri/src/lib.rs` and `app/src/lib/tauri/events.ts`

**Checkpoint**: Foundation ready — user story work can proceed.

---

## Phase 3: User Story 1 - Org admin publishes enforceable policy pack (Priority: P1) 🎯 MVP

**Goal**: Enrolled clients can sync, verify, and apply a valid org policy pack consistently.

**Independent Test**: Publish/update policy for a test org, sync from multiple enrolled clients, and verify matching effective restrictions.

### Tests for User Story 1

- [x] T014 [P] [US1] Add Rust unit tests for valid/invalid policy verification in `app/src-tauri/src/policy.rs`
- [x] T015 [P] [US1] Add Rust unit tests for monotonic version supersede logic in `app/src-tauri/src/policy.rs`
- [x] T016 [P] [US1] Add TS command wrapper tests for `policy_sync` and `policy_get_state` in `app/src/lib/tauri/commands.test.ts`

### Implementation for User Story 1

- [x] T017 [US1] Implement `policy_sync` command flow (eligibility, fetch result mapping, apply on success) in `app/src-tauri/src/commands/policy.rs`
- [x] T018 [US1] Implement effective settings merge from validated policy constraints in `app/src-tauri/src/policy.rs`
- [x] T019 [US1] Apply policy-driven settings to runtime sync path in `app/src-tauri/src/lib.rs` and `app/src/lib/tauri/settings.ts`
- [x] T020 [US1] Emit `settings-changed` and `policy-state-changed` after successful apply in `app/src-tauri/src/commands/policy.rs`
- [x] T021 [US1] Add query hook for sync/state refresh in `app/src/lib/queries.ts`
- [x] T022 [US1] Handle no-org/none-source path without blocking baseline usage in `app/src-tauri/src/commands/policy.rs` and `app/src/lib/queries.ts`

**Checkpoint**: User Story 1 is independently functional and testable.

---

## Phase 4: User Story 2 - End users understand enforced settings (Priority: P2)

**Goal**: Users can see enforced fields, understand why they are locked, and cannot override them.

**Independent Test**: Load policy-enforced fields and confirm lock indicators, reason text, and blocked edits in settings UI.

### Tests for User Story 2

- [x] T023 [P] [US2] Add TS unit tests for policy lock derivation helpers in `app/src/lib/tauri/settings.test.ts`
- [x] T024 [P] [US2] Add component tests for enforced control rendering in `app/src/components/settings/SettingsPanel.test.tsx`
- [x] T025 [P] [US2] Add TS tests for unlock behavior after policy removal in `app/src/lib/queries.test.ts`

### Implementation for User Story 2

- [x] T026 [US2] Implement per-setting enforcement metadata mapping in `app/src/lib/tauri/settings.ts`
- [x] T027 [US2] Render policy indicators + reason labels in `app/src/components/settings/SettingsPanel.tsx`
- [x] T028 [US2] Disable/guard edits for enforced controls in `app/src/components/settings/SettingsPanel.tsx`
- [x] T029 [US2] Add policy summary section (source/version/last update/expiry) in `app/src/components/settings/PolicyDiagnosticsCard.tsx`
- [x] T030 [US2] Rehydrate UI state on `policy-state-changed` across windows in `app/src/lib/tauri/events.ts` and `app/src/lib/queries.ts`

**Checkpoint**: User Story 2 is independently functional and testable.

---

## Phase 5: User Story 3 - Clients stay reliable during policy service outages (Priority: P3)

**Goal**: Clients keep enforcing last valid policy during temporary outages and degrade predictably after expiry.

**Independent Test**: Simulate fetch failures before and after expiry; verify cached behavior, degraded transition, and recovery after successful sync.

### Tests for User Story 3

- [x] T031 [P] [US3] Add Rust tests for cache-valid outage fallback and expiry transition in `app/src-tauri/src/policy.rs`
- [x] T032 [P] [US3] Add Rust tests for stale version rejection and recovery path in `app/src-tauri/src/policy.rs`
- [x] T033 [P] [US3] Add TS tests for degraded-state diagnostics rendering in `app/src/components/settings/PolicyDiagnosticsCard.test.tsx`

### Implementation for User Story 3

- [x] T034 [US3] Implement stale-while-valid fallback state transitions in `app/src-tauri/src/policy.rs`
- [x] T035 [US3] Implement `degraded_expired` behavior and failure reason persistence in `app/src-tauri/src/policy.rs`
- [x] T036 [US3] Implement auto-recovery to `cloud` on successful re-sync in `app/src-tauri/src/commands/policy.rs`
- [x] T037 [US3] Implement diagnostics export command with redaction guarantees in `app/src-tauri/src/commands/policy.rs`
- [x] T038 [US3] Add TS wrapper + query action for diagnostics export in `app/src/lib/tauri/commands.ts` and `app/src/lib/queries.ts`

**Checkpoint**: User Story 3 is independently functional and testable.

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Final hardening, docs, and verification across stories.

- [x] T039 [P] Add policy flow documentation in `docs/How Tos/POLICY_PACKS.md`
- [x] T040 Add redaction/security review notes in `docs/Dev Docs/SECURITY_NOTES.md`
- [x] T041 Run quickstart validation checklist updates in `specs/012-cloud-policy-packs/quickstart.md`
- [x] T042 Run final CI gate and record result in `specs/012-cloud-policy-packs/quickstart.md`

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 1 (Setup)**: No dependencies.
- **Phase 2 (Foundational)**: Depends on Phase 1; blocks all user stories.
- **Phase 3 (US1)**: Depends on Phase 2.
- **Phase 4 (US2)**: Depends on Phase 2 and consumes enforcement metadata from US1 outputs.
- **Phase 5 (US3)**: Depends on Phase 2 and extends sync/cache behavior from US1.
- **Phase 6 (Polish)**: Depends on all selected user stories being complete.

### User Story Dependencies

- **US1 (P1)**: Independent after foundational phase; defines baseline sync/apply behavior.
- **US2 (P2)**: Independent testability preserved; uses policy state/enforcement outputs from US1.
- **US3 (P3)**: Independent testability preserved; extends US1 with outage fallback and diagnostics export.

### Within Each User Story

- Write tests first and confirm they fail before implementation.
- Implement backend/domain logic before UI integration where contracts are affected.
- Update TS/Rust contracts in tandem for any command/event/type changes.

---

## Parallel Opportunities

- **Setup**: T004 and T005 can run in parallel after T001–T003.
- **Foundational**: T007, T008, T010, and T012 can run in parallel once T006/T009 skeletons exist.
- **US1**: T014–T016 parallel test authoring; T017 and T018 can proceed in parallel before wiring tasks T019–T022.
- **US2**: T023–T025 parallel tests; T027 and T029 can run in parallel after T026.
- **US3**: T031–T033 parallel tests; T034 and T037 can run in parallel before T038 integration.
- **Polish**: T039 and T040 can run in parallel.

---

## Parallel Example: User Story 1

- Run backend sync flow and policy validation in parallel:
   - `app/src-tauri/src/commands/policy.rs`
   - `app/src-tauri/src/policy.rs`
- In parallel, run UI query integration and command-wrapper tests:
   - `app/src/lib/queries.ts`
   - `app/src/lib/tauri/commands.test.ts`

---

## Implementation Strategy

### MVP First (User Story 1)

1. Complete Phase 1 and Phase 2.
2. Deliver Phase 3 (US1) and validate independent test criteria.
3. Demo/release MVP with cloud policy sync + apply behavior.

### Incremental Delivery

1. Add US2 for enforcement visibility and edit blocking.
2. Add US3 for outage resilience + diagnostics export.
3. Finish with Phase 6 polish and full CI validation.

### Parallel Team Strategy

1. Shared effort on Setup + Foundational.
2. Split by story after foundational checkpoint:
   - Engineer A: US1 sync/apply consistency
   - Engineer B: US2 settings UX indicators/locks
   - Engineer C: US3 outage fallback + diagnostics
