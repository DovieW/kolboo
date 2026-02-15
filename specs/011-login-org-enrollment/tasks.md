# Tasks: Phase 1 Login and Org Enrollment

**Input**: Design documents from `/specs/011-login-org-enrollment/`
**Prerequisites**: `plan.md`, `spec.md`, `research.md`, `data-model.md`, `contracts/`

**Tests**: Tests are included because the specification has explicit independent test criteria and success metrics for each user story.

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Prepare baseline files and contracts for implementation.

- [X] T001 Create account/licensing module stubs in `app/src-tauri/src/licensing.rs` and `app/src-tauri/src/commands/licensing.rs`
- [X] T002 Create frontend licensing wrapper module in `app/src/lib/tauri/license.ts`
- [X] T003 [P] Add initial account settings component scaffold in `app/src/components/settings/AccountSettings.tsx`
- [X] T004 [P] Add feature task doc links to `specs/011-login-org-enrollment/quickstart.md` and `specs/011-login-org-enrollment/plan.md`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core cross-story infrastructure required before user stories.

- [X] T005 Add shared `LicenseState`/`TierLimits`/`UsageStats` TS types in `app/src/lib/tauri/types.ts`
- [X] T006 [P] Define Rust licensing domain structs/enums (`LicenseStatus`, `LicenseState`, `OrgContext`) in `app/src-tauri/src/licensing.rs`
- [X] T007 Implement secure session material interface (store/load/clear) in `app/src-tauri/src/licensing.rs`
- [X] T008 [P] Register licensing command module and command handlers in `app/src-tauri/src/commands/mod.rs` and `app/src-tauri/src/lib.rs`
- [X] T009 Add TS command wrappers for `licenseGetState`, `licenseStartLogin`, `licenseLogout`, `licenseRefreshEntitlement`, `licenseGetManagementUrl` in `app/src/lib/tauri/commands.ts`
- [X] T010 Implement shared user-facing licensing error mapping in `app/src/lib/tauri/license.ts` and `app/src-tauri/src/licensing.rs`

**Checkpoint**: Foundation complete; user stories can proceed.

---

## Phase 3: User Story 1 - Optional account sign-in for managed features (Priority: P1) 🎯 MVP

**Goal**: Deliver optional sign-in/out flow with visible account+tier state while preserving signed-out usability.

**Independent Test**: User can sign in, see account/tier state, sign out, and still use baseline non-managed mode.

### Tests for User Story 1

- [X] T011 [P] [US1] Add TS command wrapper tests for login/logout/state in `app/src/lib/tauri/commands.test.ts`
- [X] T012 [P] [US1] Add Rust unit tests for login/sign-out state transitions in `app/src-tauri/src/licensing.rs`
- [X] T013 [P] [US1] Add account settings UI tests for signed-out/signed-in rendering in `app/src/components/settings/AccountSettings.test.tsx`

### Implementation for User Story 1

- [X] T014 [US1] Implement login flow orchestration and state normalization in `app/src-tauri/src/licensing.rs`
- [X] T015 [US1] Implement logout flow clearing secure session material in `app/src-tauri/src/licensing.rs`
- [X] T016 [P] [US1] Add query hook for account/license state in `app/src/lib/queries.ts`
- [X] T017 [P] [US1] Build `AccountSettings` sign-in/sign-out UI and tier display in `app/src/components/settings/AccountSettings.tsx`
- [X] T018 [US1] Wire account settings into navigation/panels in `app/src/App.tsx` and `app/src/components/settings/index.ts`
- [X] T019 [US1] Emit and consume account-state update events for UI refresh in `app/src-tauri/src/commands/licensing.rs` and `app/src/lib/tauri/license.ts`
- [X] T020 [US1] Add non-technical auth failure messages and retry actions in `app/src/components/settings/AccountSettings.tsx`

**Checkpoint**: US1 is independently functional and testable.

---

## Phase 4: User Story 2 - Enterprise org enrollment visibility (Priority: P2)

**Goal**: Show organization membership context and entitlement scope for signed-in enterprise users.

**Independent Test**: Signed-in enterprise user sees org name/identifier and tier context; personal/community users do not see enterprise context badges.

### Tests for User Story 2

- [X] T021 [P] [US2] Add TS UI tests for org context rendering rules in `app/src/components/settings/AccountSettings.test.tsx`
- [X] T022 [P] [US2] Add Rust unit tests for org context mapping in entitlement snapshots in `app/src-tauri/src/licensing.rs`

### Implementation for User Story 2

- [X] T023 [US2] Extend backend entitlement snapshot to include org context in `app/src-tauri/src/licensing.rs`
- [X] T024 [P] [US2] Extend TS types/contracts for org context in `app/src/lib/tauri/types.ts` and `app/src/lib/tauri/commands.ts`
- [X] T025 [US2] Render org identity/tier scope in `app/src/components/settings/AccountSettings.tsx`
- [X] T026 [US2] Handle org membership refresh updates in `app/src/lib/queries.ts` and `app/src/lib/tauri/license.ts`

**Checkpoint**: US2 is independently functional and testable with signed-in state.

---

## Phase 5: User Story 3 - Resilient entitlement behavior with offline grace (Priority: P3)

**Goal**: Maintain account-based capabilities during bounded outage windows, then degrade predictably when grace expires.

**Independent Test**: Simulated refresh failures keep grace-active state within window, then transition to expired after window elapses.

### Tests for User Story 3

- [X] T027 [P] [US3] Add Rust tests for `active -> grace -> expired` transitions and boundary timestamps in `app/src-tauri/src/licensing.rs`
- [X] T028 [P] [US3] Add TS tests for grace/expired UI messaging and status badges in `app/src/components/settings/AccountSettings.test.tsx`
- [X] T029 [P] [US3] Add TS tests for refresh failure handling in query layer in `app/src/lib/queries.test.ts`

### Implementation for User Story 3

- [X] T030 [US3] Implement grace-window evaluator and expiry transitions in `app/src-tauri/src/licensing.rs`
- [X] T031 [US3] Implement entitlement refresh command behavior and fallback state returns in `app/src-tauri/src/commands/licensing.rs`
- [X] T032 [P] [US3] Persist non-secret entitlement cache timestamps/state in `app/src/lib/tauri/settings.ts` and `app/src-tauri/src/lib.rs`
- [X] T033 [US3] Implement UI grace/expired indicators and downgrade explanation text in `app/src/components/settings/AccountSettings.tsx`
- [X] T034 [US3] Emit diagnostics-safe entitlement transition events in `app/src-tauri/src/licensing.rs` and consume in `app/src/lib/tauri/license.ts`

**Checkpoint**: US3 is independently functional and testable.

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Finish quality gate, docs, and cross-story validation.

- [X] T035 [P] Update user-facing account/licensing documentation in `docs/User Docs/` (new or existing account guidance doc)
- [X] T036 [P] Update implementation notes in `docs/How Tos/` for support troubleshooting and diagnostics usage
- [X] T037 Run formatting/checks pipeline (`pnpm -C app lint`, `pnpm -C app test`, `pnpm -C app cargo:test`, `pnpm -C app check:ci`) and record outcomes in `specs/011-login-org-enrollment/quickstart.md`
- [X] T038 Verify contract parity between `specs/011-login-org-enrollment/contracts/license-local-api.yaml`, `app/src/lib/tauri/commands.ts`, and `app/src-tauri/src/commands/licensing.rs`
- [X] T039 [P] Add Phase 1 Sentry initialization for account/licensing surfaces in `app/src/main.tsx` and relevant Tauri startup wiring
- [X] T040 [P] Add redaction-safe Sentry helpers for account/licensing errors in `app/src/lib/tauri/license.ts` and `app/src-tauri/src/licensing.rs`
- [X] T041 Add deterministic tests verifying Sentry payload redaction behavior in `app/src/lib/tauri/license.test.ts` and/or Rust unit tests
- [X] T042 Update `specs/011-login-org-enrollment/quickstart.md` with Sentry validation steps and add result notes

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 1 (Setup)**: No dependencies.
- **Phase 2 (Foundational)**: Depends on Phase 1 completion and blocks all user stories.
- **Phase 3 (US1)**: Depends on Phase 2.
- **Phase 4 (US2)**: Depends on Phase 2 and US1 authenticated-state baseline.
- **Phase 5 (US3)**: Depends on Phase 2 and US1 entitlement baseline.
- **Phase 6 (Polish)**: Depends on completion of target user stories.

### User Story Dependencies

- **US1 (P1)**: No user-story dependency; MVP entry point.
- **US2 (P2)**: Requires signed-in account state from US1 but remains independently testable once that baseline exists.
- **US3 (P3)**: Requires entitlement baseline from US1; grace behavior remains independently testable with mocked refresh outcomes.

### Within Each User Story

- Tests first (write and verify failing), then implementation.
- Backend state model/logic before UI rendering.
- Contract updates in Rust and TS before integration validation.
- Story-level checks complete before moving to polish.

---

## Parallel Execution Examples

### User Story 1

- Run in parallel:
  - T011 (`app/src/lib/tauri/commands.test.ts`)
  - T012 (`app/src-tauri/src/licensing.rs` tests)
  - T013 (`app/src/components/settings/AccountSettings.test.tsx`)
- Then run in parallel:
  - T016 (`app/src/lib/queries.ts`)
  - T017 (`app/src/components/settings/AccountSettings.tsx`)

### User Story 2

- Run in parallel:
  - T021 (`app/src/components/settings/AccountSettings.test.tsx`)
  - T022 (`app/src-tauri/src/licensing.rs` tests)
- Then run in parallel:
  - T024 (`app/src/lib/tauri/types.ts`, `app/src/lib/tauri/commands.ts`)
  - T025 (`app/src/components/settings/AccountSettings.tsx`)

### User Story 3

- Run in parallel:
  - T027 (`app/src-tauri/src/licensing.rs` tests)
  - T028 (`app/src/components/settings/AccountSettings.test.tsx`)
  - T029 (`app/src/lib/queries.test.ts`)
- Then run in parallel:
  - T032 (`app/src/lib/tauri/settings.ts`, `app/src-tauri/src/lib.rs`)
  - T033 (`app/src/components/settings/AccountSettings.tsx`)

---

## Implementation Strategy

### MVP First (US1)

1. Complete Phase 1 and Phase 2.
2. Deliver Phase 3 (US1) end-to-end.
3. Validate US1 independently before proceeding.

### Incremental Delivery

1. MVP: US1 optional sign-in + account status.
2. Add US2 org enrollment visibility.
3. Add US3 offline grace resilience.
4. Run full polish and CI gate.

### Team Parallelization

1. One developer completes foundational backend contract setup (T005–T010).
2. After foundation:
   - Dev A: US1 UI/query tasks
   - Dev B: US2 org context
   - Dev C: US3 grace logic
3. Integrate with shared contract parity check (T038).

---

## Notes

- `[P]` tasks are parallel-safe by file separation and dependency order.
- Every story includes deterministic test coverage and an independent test checkpoint.
- No tasks require real network calls or real API keys in automated tests.
