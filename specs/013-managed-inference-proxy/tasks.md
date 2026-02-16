# Tasks: Phase 3 Managed Inference Proxy

**Input**: Design documents from `/specs/013-managed-inference-proxy/`
**Prerequisites**: `plan.md`, `spec.md`, `research.md`, `data-model.md`, `contracts/`

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Create shared scaffolding and configuration surfaces used by all stories.

- [x] T001 Add managed-inference desktop wrapper scaffold in `app/src/lib/tauri/managedInference.ts`
- [x] T002 Add managed-inference backend module scaffold in `app/src-tauri/src/managed_inference/mod.rs`
- [x] T003 [P] Add API edge managed-inference route scaffold in `C:/Users/dovie/repos/kolboo-private/apps/api-edge/src/routes/managed-inference.ts`
- [x] T004 [P] Add managed inference env var placeholders in `C:/Users/dovie/repos/kolboo-private/.env.example`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Implement core cross-story contracts and routing foundations.

**⚠️ CRITICAL**: No user-story implementation should start before this phase is complete.

- [x] T005 Implement shared managed error taxonomy in `app/src/lib/tauri/types.ts`
- [x] T006 [P] Implement managed error mapping + codes in `app/src-tauri/src/managed_inference/errors.rs`
- [x] T007 [P] Implement idempotency key generation/propagation utility in `app/src/lib/tauri/managedInference.ts`
- [x] T008 Implement provider mode resolver (`managed` vs `byok`) in `app/src-tauri/src/pipeline/config.rs`
- [x] T009 [P] Implement managed entitlement/mode preflight middleware in `C:/Users/dovie/repos/kolboo-private/apps/api-edge/src/middleware/managed-preflight.ts`
- [x] T010 [P] Implement usage-state contract adapters from OpenAPI in `app/src/lib/tauri/commands.ts`
- [x] T011 Implement metadata-only request record model in `C:/Users/dovie/repos/kolboo-private/packages/contracts/src/managed-request-record.ts`
- [x] T012 Implement metering ledger idempotency store interface in `C:/Users/dovie/repos/kolboo-private/apps/api-edge/src/services/metering-ledger.ts`

**Checkpoint**: Foundation complete; user stories can proceed independently.

---

## Phase 3: User Story 1 - Personal user runs without API keys (Priority: P1) 🎯 MVP

**Goal**: Signed-in personal users complete STT/LLM flows through managed mode without local provider keys.

**Independent Test**: On clean install, personal user signs in and completes voice/rewrite flows; over-quota requests fail with deterministic actionable messaging.

### Tests for User Story 1

- [x] T013 [P] [US1] Add managed mode resolver unit tests in `app/src/lib/tauri/settings.managed-mode.test.ts`
- [x] T014 [P] [US1] Add pipeline routing unit tests for personal managed mode in `app/src-tauri/src/pipeline/tests/managed_personal_tests.rs`
- [x] T015 [P] [US1] Add managed gateway wrapper tests (idempotency header + error mapping) in `app/src/lib/tauri/managedInference.test.ts`

### Implementation for User Story 1

- [x] T016 [US1] Route personal-tier STT/LLM calls through managed wrapper in `app/src-tauri/src/pipeline.rs`
- [x] T017 [P] [US1] Implement managed STT/LLM HTTP calls (`/v1/stt/transcribe`, `/v1/llm/complete`) in `app/src/lib/tauri/managedInference.ts`
- [x] T018 [P] [US1] Implement managed usage state query (`/v1/usage/state`) in `app/src/lib/tauri/commands.ts`
- [x] T019 [US1] Add personal usage meter + threshold warning UI in `app/src/components/settings/AccountSettings.tsx`
- [x] T020 [US1] Implement deterministic user-facing quota/ineligible/unauthorized messages in `app/src/lib/queries.ts`
- [x] T021 [US1] Mark managed requests distinctly in request logs using existing fields in `app/src-tauri/src/stats.rs`

**Checkpoint**: US1 independently functional and testable.

---

## Phase 4: User Story 2 - Enterprise admin chooses inference mode (Priority: P1)

**Goal**: Enterprise org admins can switch `managed`/`byok`, and members receive consistent behavior after sync.

**Independent Test**: Toggle org mode and verify member routing/UX behavior updates within sync window and respects org-BYOK semantics.

### Tests for User Story 2

- [x] T022 [P] [US2] Add enterprise mode sync normalization tests in `app/src/lib/tauri/settings.enterprise-mode.test.ts`
- [x] T023 [P] [US2] Add admin API contract tests for org inference mode endpoints in `C:/Users/dovie/repos/kolboo-private/apps/admin-dashboard/tests/inference-mode.contract.test.ts`
- [x] T024 [P] [US2] Add backend mode enforcement tests for org members in `app/src-tauri/src/pipeline/tests/enterprise_mode_tests.rs`

### Implementation for User Story 2

- [x] T025 [US2] Implement org inference mode GET/PUT handlers in `C:/Users/dovie/repos/kolboo-private/apps/api-edge/src/routes/org-inference-mode.ts`
- [x] T026 [P] [US2] Implement org key-rotation continuity endpoint in `C:/Users/dovie/repos/kolboo-private/apps/api-edge/src/routes/org-byok-rotation.ts`
- [x] T027 [P] [US2] Implement admin dashboard inference-mode controls in `C:/Users/dovie/repos/kolboo-private/apps/admin-dashboard/src/features/inference-mode/InferenceModePanel.tsx`
- [x] T028 [US2] Apply org mode from policy/entitlement sync into runtime config in `app/src-tauri/src/commands/config.rs`
- [x] T029 [US2] Preserve org-BYOK routing path and non-managed credit semantics in `app/src-tauri/src/pipeline.rs`
- [X] T030 [US2] Emit settings-changed signal after mode updates for UI coherence in `app/src/lib/tauri/settings.ts`

**Checkpoint**: US2 independently functional and testable.

---

## Phase 5: User Story 3 - Operations team controls abuse/cost risk (Priority: P2)

**Goal**: Enforce abuse protections and provide content-safe diagnostics for managed inference operations.

**Independent Test**: Simulate invalid auth, quota overage, and burst traffic; verify deterministic denials and metadata-only telemetry.

### Tests for User Story 3

- [x] T031 [P] [US3] Add rate-limit + abuse guard tests in `C:/Users/dovie/repos/kolboo-private/apps/api-edge/tests/managed-abuse-controls.test.ts`
- [x] T032 [P] [US3] Add idempotent metering ledger tests in `C:/Users/dovie/repos/kolboo-private/apps/api-edge/tests/metering-idempotency.test.ts`
- [x] T033 [P] [US3] Add telemetry redaction tests for managed inference traces in `C:/Users/dovie/repos/kolboo-private/packages/observability/tests/managed-redaction.test.ts`

### Implementation for User Story 3

- [x] T034 [US3] Implement tenant/subject keyed rate limiting in `C:/Users/dovie/repos/kolboo-private/apps/api-edge/src/middleware/rate-limit.ts`
- [x] T035 [P] [US3] Implement anomaly + hard quota prechecks before provider dispatch in `C:/Users/dovie/repos/kolboo-private/apps/api-edge/src/services/quota-precheck.ts`
- [x] T036 [P] [US3] Implement exactly-once metering commit by idempotency key in `C:/Users/dovie/repos/kolboo-private/apps/api-edge/src/services/metering-ledger.ts`
- [x] T037 [US3] Implement metadata-only managed request telemetry sink in `C:/Users/dovie/repos/kolboo-private/packages/observability/src/managedTelemetry.ts`
- [x] T038 [US3] Implement operator diagnostics query surface (tenant/failure/time window) in `C:/Users/dovie/repos/kolboo-private/apps/api-edge/src/routes/managed-diagnostics.ts`

**Checkpoint**: US3 independently functional and testable.

---

## Phase 6: User Story 4 - User continuity during outages (Priority: P2)

**Goal**: Provide graceful fallback behavior and clear recovery guidance during managed-path incidents.

**Independent Test**: Simulate managed gateway/upstream degradation; verify fallback to valid non-managed path when available, otherwise clear recovery message.

### Tests for User Story 4

- [X] T039 [P] [US4] Add fallback behavior tests for temporary managed outages in `app/src/lib/queries.managed-fallback.test.ts`
- [X] T040 [P] [US4] Add backend degraded-status mapping tests in `app/src-tauri/src/pipeline/tests/managed_outage_tests.rs`

### Implementation for User Story 4

- [X] T041 [US4] Implement managed temporary-unavailable fallback routing in `app/src-tauri/src/pipeline.rs`
- [X] T042 [P] [US4] Implement user recovery/fallback messaging in `app/src/components/settings/AccountSettings.tsx`
- [X] T043 [P] [US4] Implement non-blocking observability failure handling in `C:/Users/dovie/repos/kolboo-private/packages/observability/src/managedTelemetry.ts`
- [X] T044 [US4] Implement gateway degraded-response normalization (`temporarily_unavailable`) in `C:/Users/dovie/repos/kolboo-private/apps/api-edge/src/routes/managed-inference.ts`

**Checkpoint**: US4 independently functional and testable.

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: Final hardening, docs, and end-to-end validation.

- [X] T045 [P] Update managed inference operational runbook in `C:/Users/dovie/repos/kolboo-private/docs/RUNBOOK_MANAGED_INFERENCE.md`
- [X] T046 [P] Update desktop user/admin docs for managed vs BYOK modes in `docs/User Docs/MANAGED_VS_BYOK_MODES.md`
- [X] T047 Align readiness evidence links for Phase 3 in `C:/Users/dovie/repos/kol-software/plans/KOLBOO_ENTERPRISE_AND_SUBSCRIPTION_READINESS.md`
- [X] T048 Run quickstart validation and record outcomes in `specs/013-managed-inference-proxy/quickstart.md`

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 1 (Setup)**: starts immediately.
- **Phase 2 (Foundational)**: depends on Phase 1 and blocks all user stories.
- **Phases 3-6 (User Stories)**: each depends on Phase 2 completion; then proceed by priority or in parallel.
- **Phase 7 (Polish)**: depends on completion of all intended user stories.

### User Story Dependencies

- **US1 (P1)**: no dependency on other stories after foundational work.
- **US2 (P1)**: no hard dependency on US1; integrates same foundational routing/contracts.
- **US3 (P2)**: depends on foundational/gateway paths; can run in parallel with US2.
- **US4 (P2)**: depends on managed path availability/error taxonomy; can run after US1 baseline.

### Suggested Story Completion Order (Dependency Graph)

1. **US1** (MVP managed personal path)
2. **US2** (enterprise mode control)
3. **US3** (abuse/cost controls + diagnostics)
4. **US4** (outage continuity)

---

## Parallel Execution Examples

### US1 Parallel Example

- Run `T013`, `T014`, and `T015` together (distinct test files).
- Run `T017` and `T018` together (separate wrapper/commands files) while `T016` proceeds in backend.

### US2 Parallel Example

- Run `T023` and `T024` together (private dashboard tests vs Rust tests).
- Run `T026` and `T027` together (API route vs dashboard UI).

### US3 Parallel Example

- Run `T031`, `T032`, `T033` together (separate test targets).
- Run `T035` and `T037` together (quota service vs observability package).

### US4 Parallel Example

- Run `T039` and `T040` together.
- Run `T042` and `T043` together while backend handles `T041`/`T044`.

---

## Implementation Strategy

### MVP First (US1)

1. Finish Setup + Foundational (T001-T012).
2. Complete US1 (T013-T021).
3. Validate clean-install personal managed flow before expanding scope.

### Incremental Delivery

1. Deliver US1 (personal managed value).
2. Deliver US2 (enterprise mode governance).
3. Deliver US3 (operational safety + diagnostics).
4. Deliver US4 (reliability/fallback UX).
5. Finish polish and readiness evidence.

### Validation Cadence

- During iteration: run smallest relevant checks first.
- Before final handoff: run formatting, targeted tests, then one final `pnpm -C app check:ci`.
