# Tasks: Architecture Deepening Plan

**Input**: Design documents from `specs/017-architecture-deepening-plan/`
**Prerequisites**: `plan.md`, `spec.md`, `research.md`, `data-model.md`, `contracts/`, `quickstart.md`
**Tests**: Required. This feature explicitly requires deterministic tests and 100% in-scope statement, branch, and function coverage for changed or newly introduced modules.
**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each slice.

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Prepare planning/evidence files and confirm the active Spec Kit feature state.

- [x] T001 Verify `.specify/feature.json` points to `specs/017-architecture-deepening-plan` and record the result in `specs/017-architecture-deepening-plan/quickstart.md`
- [x] T002 Create the validation evidence directory in `specs/017-architecture-deepening-plan/validation/README.md`
- [x] T003 [P] Create the coverage evidence log template in `specs/017-architecture-deepening-plan/validation/coverage-evidence.md`
- [x] T004 [P] Create the edge-case matrix template in `specs/017-architecture-deepening-plan/validation/edge-case-matrix.md`
- [x] T005 [P] Create the per-slice safe-stop checklist in `specs/017-architecture-deepening-plan/validation/slice-checklist.md`
- [x] T006 [P] Add a deferred-decision log for provider-family seams in `specs/017-architecture-deepening-plan/validation/provider-family-decisions.md`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Establish deterministic validation and coverage foundations required before any user story can claim completion.

**⚠️ CRITICAL**: No user story may be marked complete until this phase and the User Story 8 coverage gate are complete.

- [x] T007 Record Rust in-scope coverage package-script requirements in `specs/017-architecture-deepening-plan/validation/coverage-evidence.md`
- [x] T008 [P] Create Rust coverage helper interface notes in `specs/017-architecture-deepening-plan/validation/rust-coverage-interface.md`
- [x] T009 Add Rust coverage tool usage notes and Windows PowerShell cache setup to `specs/017-architecture-deepening-plan/quickstart.md`
- [x] T010 [P] Add backend deterministic fixture helpers for pipeline tests in `app/src-tauri/src/tests/architecture_fixtures.rs`
- [x] T011 Update backend test module registration for architecture fixtures in `app/src-tauri/src/tests/mod.rs`
- [x] T012 [P] Add frontend deterministic settings fixture helpers in `app/src/lib/testing/settingsFixtures.ts`
- [x] T013 Add coverage evidence instructions for TypeScript and Rust modules in `specs/017-architecture-deepening-plan/validation/coverage-evidence.md`
- [x] T014 Record the initial in-scope module list and coverage baseline expectations in `specs/017-architecture-deepening-plan/validation/coverage-evidence.md`

**Checkpoint**: Deterministic validation foundation exists and user story slices can now start.

---

## Phase 3: User Story 8 - Prove the initiative is bug-free with 100% in-scope coverage (Priority: P1)

**Goal**: Provide the quality gate that every later slice must satisfy.

**Independent Test**: Run the coverage/evidence workflow against a small touched-module example and verify incomplete coverage blocks completion documentation.

### Tests for User Story 8

- [x] T015 [P] [US8] Add tests for TypeScript coverage evidence parsing in `app/scripts/coverage-evidence.test.ts`
- [x] T016 [P] [US8] Add tests for Rust coverage helper argument validation in `app/scripts/rust-coverage.test.ts`
- [x] T017 [US8] Add edge-case matrix validation examples in `specs/017-architecture-deepening-plan/validation/edge-case-matrix.md`
- [x] T018 [US8] Add a coverage-gate checklist and regression-defect log fixture in `specs/017-architecture-deepening-plan/validation/coverage-evidence.md`

### Implementation for User Story 8

- [x] T019 [US8] Implement the TypeScript coverage evidence parser in `app/scripts/coverage-evidence.mjs`
- [x] T020 [US8] Implement the Rust coverage helper script in `app/scripts/rust-coverage.mjs`
- [x] T021 [US8] Wire coverage helper scripts into `app/package.json`
- [x] T022 [US8] Document how changed/new modules declare in-scope coverage in `specs/017-architecture-deepening-plan/validation/coverage-evidence.md`
- [x] T023 [US8] Document the edge-case-to-test mapping workflow in `specs/017-architecture-deepening-plan/validation/edge-case-matrix.md`
- [x] T024 [US8] Validate the coverage and regression-test gate workflow and record results in `specs/017-architecture-deepening-plan/validation/coverage-evidence.md`

**Checkpoint**: Coverage workflow is ready and every later story can prove 100% in-scope coverage.

---

## Phase 4: User Story 1 - Preserve OCR Session correctness while simplifying ownership (Priority: P1) 🎯 MVP

**Goal**: Move OCR Session lifecycle state and invariants behind one authoritative module interface while preserving request ownership, cancellation, timeout reuse, failure reporting, overlay status, and request-log behavior.

**Independent Test**: Exercise OCR Session lifecycle scenarios with deterministic fake tasks and request-log fixtures without real screenshots or network calls.

### Tests for User Story 1

- [x] T025 [US1] Add OCR Session stale-success and stale-failure tests in `app/src-tauri/src/pipeline/ocr_session_state.rs`
- [x] T026 [US1] Add OCR Session await-timeout-restore tests in `app/src-tauri/src/pipeline/ocr_session_state.rs`
- [x] T027 [US1] Add OCR Session cancel/idempotency tests in `app/src-tauri/src/pipeline/ocr_session_state.rs`
- [x] T028 [US1] Add OCR Session sanitized failure/status tests in `app/src-tauri/src/pipeline/ocr_session_state.rs`
- [x] T029 [US1] Add OCR Session integration characterization tests in `app/src-tauri/src/pipeline/ocr_session.rs`

### Implementation for User Story 1

- [x] T030 [US1] Create the OCR Session state module in `app/src-tauri/src/pipeline/ocr_session_state.rs`
- [x] T031 [US1] Register the OCR Session state module in `app/src-tauri/src/pipeline.rs`
- [x] T032 [US1] Replace loose OCR Session fields with the OCR Session state module in `app/src-tauri/src/pipeline.rs`
- [x] T033 [US1] Route OCR task start, cancel, finalize, await, status, and failure reads through the OCR Session state interface in `app/src-tauri/src/pipeline/ocr_session.rs`
- [x] T034 [US1] Preserve request-log correlation behavior while using the OCR Session state interface in `app/src-tauri/src/pipeline/ocr_session.rs`
- [x] T035 [US1] Record OCR Session coverage evidence and edge-case results in `specs/017-architecture-deepening-plan/validation/coverage-evidence.md`

**Checkpoint**: OCR Session behavior is independently testable and preserves request ownership invariants.

---

## Phase 5: User Story 2 - Make settings behavior drift-proof (Priority: P1)

**Goal**: Centralize canonical defaults, normalization, explicit-null semantics, profile inheritance, and effective settings views behind testable module interfaces.

**Independent Test**: Load representative settings snapshots and verify normalized/default/effective values and explicit-null behavior without backend runtime sync.

### Tests for User Story 2

- [x] T036 [P] [US2] Add TypeScript default/malformed/null Settings View tests in `app/src/lib/tauri/settingsViews.test.ts`
- [x] T037 [US2] Add TypeScript profile inheritance Settings View tests in `app/src/lib/tauri/settingsViews.test.ts`
- [x] T038 [P] [US2] Add Rust settings default drift tests in `app/src-tauri/src/settings/default_values.rs`
- [x] T039 [P] [US2] Add legacy settings snapshot normalization tests in `app/src/lib/tauri/settings.legacy.test.ts`
- [x] T040 [US2] Add cross-layer settings default contract tests in `app/src/lib/contracts/settingsDefaultsContract.test.ts`

### Implementation for User Story 2

- [x] T041 [US2] Add canonical TypeScript settings defaults in `app/src/lib/tauri/settingsDefaults.ts`
- [x] T042 [US2] Add canonical Rust settings defaults in `app/src-tauri/src/settings/default_values.rs`
- [x] T043 [US2] Export Rust default values through settings module wiring in `app/src-tauri/src/settings.rs`
- [x] T044 [US2] Update Rust default seeding to use canonical defaults in `app/src-tauri/src/settings/defaults.rs`
- [x] T045 [US2] Update pipeline defaults that overlap settings defaults in `app/src-tauri/src/pipeline/config.rs`
- [x] T046 [US2] Add source-aware Settings View helpers in `app/src/lib/tauri/settingsViews.ts`
- [x] T047 [US2] Refactor `getSettings` normalization to use settings defaults and views in `app/src/lib/tauri/settings.ts`
- [x] T048 [US2] Update settings-related contract docs in `specs/017-architecture-deepening-plan/contracts/module-interface-contracts.md`
- [x] T049 [US2] Record Settings View coverage evidence and edge-case results in `specs/017-architecture-deepening-plan/validation/coverage-evidence.md`

**Checkpoint**: Settings defaults and effective values are drift-proof for touched settings.

---

## Phase 6: User Story 3 - Make settings runtime sync explicit and complete (Priority: P1)

**Goal**: Centralize settings runtime side effects so pipeline sync and secondary-window notifications happen exactly once when required.

**Independent Test**: Apply each setting-change category and verify pipeline sync, settings-change event, both, or neither with fake invoke/event adapters.

### Tests for User Story 3

- [x] T050 [P] [US3] Add runtime sync policy category tests in `app/src/lib/tauri/settingsSync.test.ts`
- [x] T051 [US3] Add runtime sync batch dedupe tests in `app/src/lib/tauri/settingsSync.test.ts`
- [x] T052 [US3] Add policy/license/API-key runtime sync preservation tests in `app/src/lib/tauri/settingsSync.test.ts`
- [x] T053 [P] [US3] Add query mutation integration tests for settings sync in `app/src/lib/queries.test.ts`
- [x] T054 [US3] Add command wrapper and overlay consumer settings-change tests in `app/src/lib/tauri/commands.test.ts` and `app/src/lib/overlay/overlaySettings.test.ts`

### Implementation for User Story 3

- [x] T055 [US3] Implement the Runtime Sync Policy module in `app/src/lib/tauri/settingsSync.ts`
- [x] T056 [US3] Route settings patch side effects through Runtime Sync Policy in `app/src/lib/tauri/settings.ts`
- [x] T057 [US3] Replace scattered pipeline sync calls with Runtime Sync Policy in `app/src/lib/queries.ts`
- [x] T058 [US3] Replace direct sync/event calls for API key settings in `app/src/components/settings/ApiKeysSettings.tsx`
- [x] T059 [US3] Replace direct sync/event calls for data retention settings in `app/src/components/settings/DataSettings.tsx`
- [x] T060 [US3] Replace direct sync/event calls for settings guide changes in `app/src/components/settings/SettingsGuideOverlay.tsx`
- [x] T061 [US3] Update command wrapper helpers to delegate settings-change emission consistently in `app/src/lib/tauri/commands.ts`
- [x] T062 [US3] Record Runtime Sync Policy coverage evidence and edge-case results in `specs/017-architecture-deepening-plan/validation/coverage-evidence.md`

**Checkpoint**: Settings runtime effects are explicit, deduplicated, and independently testable.

---

## Phase 7: User Story 4 - Make Transcription Flow routing strategy-independent (Priority: P2)

**Goal**: Make Transcription Flow consume one Routing Decision interface instead of strategy-specific tuples and diagnostics shapes.

**Independent Test**: Use deterministic router outcomes to verify selected preset, default target, no decision, ambiguity, failure, unknown id, and cancellation behavior.

### Tests for User Story 4

- [x] T063 [P] [US4] Add Routing Decision outcome tests for embeddings routing in `app/src-tauri/src/pipeline/routing.rs`
- [x] T064 [US4] Add Routing Decision outcome tests for LLM routing in `app/src-tauri/src/pipeline/routing.rs`
- [x] T065 [P] [US4] Add Transcription Flow routing-decision tests in `app/src-tauri/src/pipeline/transcription_flow.rs`
- [x] T066 [P] [US4] Add request-log diagnostics tests for routing decisions in `app/src-tauri/src/tests/request_log_schema_tests.rs`

### Implementation for User Story 4

- [x] T067 [US4] Add `RoutingDecision` and routing outcome types in `app/src-tauri/src/pipeline/routing.rs`
- [x] T068 [US4] Refactor embeddings routing to return `RoutingDecision` in `app/src-tauri/src/pipeline/routing.rs`
- [x] T069 [US4] Refactor LLM routing to return `RoutingDecision` in `app/src-tauri/src/pipeline/routing.rs`
- [x] T070 [US4] Update Transcription Flow to consume `RoutingDecision` in `app/src-tauri/src/pipeline/transcription_flow.rs`
- [x] T071 [US4] Preserve and centralize routing request-log updates in `app/src-tauri/src/pipeline/transcription_flow.rs`
- [x] T072 [US4] Record Routing Decision coverage evidence and edge-case results in `specs/017-architecture-deepening-plan/validation/coverage-evidence.md`

**Checkpoint**: Transcription Flow no longer knows strategy-specific routing return shapes.

---

## Phase 8: User Story 5 - Separate profile matching from effective profile behavior (Priority: P2)

**Goal**: Split program profile matching from effective preset and Active Window OCR behavior resolution.

**Independent Test**: Validate path matching, default fallback, effective preset selection, disabled/invalid profile handling, and OCR precedence independently.

### Tests for User Story 5

- [x] T073 [P] [US5] Add profile matcher path normalization tests in `app/src-tauri/src/pipeline/profile_matcher.rs`
- [x] T074 [US5] Add profile effective preset tests in `app/src-tauri/src/pipeline/profile_resolution.rs`
- [x] T075 [US5] Add Active Window OCR mode precedence matrix tests in `app/src-tauri/src/pipeline/profile_resolution.rs`
- [x] T076 [US5] Add disabled/invalid profile, duplicate-match, and missing-foreground handling tests in `app/src-tauri/src/pipeline/profile_resolution.rs`

### Implementation for User Story 5

- [x] T077 [US5] Create program profile matcher module in `app/src-tauri/src/pipeline/profile_matcher.rs`
- [x] T078 [US5] Create effective profile behavior module in `app/src-tauri/src/pipeline/profile_resolution.rs`
- [x] T079 [US5] Register profile matcher and profile resolution modules in `app/src-tauri/src/pipeline.rs`
- [x] T080 [US5] Move path matching behavior out of `app/src-tauri/src/pipeline/program_profiles.rs`
- [x] T081 [US5] Move effective preset and Active Window OCR mode behavior out of `app/src-tauri/src/pipeline/program_profiles.rs`
- [x] T082 [US5] Update pipeline call sites to use profile matcher and profile resolution modules in `app/src-tauri/src/pipeline.rs`
- [x] T083 [US5] Record Profile Resolution coverage evidence and edge-case results in `specs/017-architecture-deepening-plan/validation/coverage-evidence.md`

**Checkpoint**: Matching and effective behavior are separately testable and preserve current profile semantics.

---

## Phase 9: User Story 6 - Localize local-provider lifecycle rules (Priority: P2)

**Goal**: Centralize local-provider load, cache, readiness, manual-mode, and managed-bypass behavior behind a focused module interface.

**Independent Test**: Use fake local-provider adapters and deterministic configuration inputs without loading real models or calling providers.

### Tests for User Story 6

- [x] T084 [US6] Add local-provider cache identity tests in `app/src-tauri/src/pipeline/local_provider_lifecycle.rs`
- [x] T085 [US6] Add local-provider manual-mode/readiness tests in `app/src-tauri/src/pipeline/local_provider_lifecycle.rs`
- [x] T086 [US6] Add local-provider managed-bypass tests in `app/src-tauri/src/pipeline/local_provider_lifecycle.rs`
- [x] T087 [US6] Add local-provider unload/config-change tests in `app/src-tauri/src/pipeline/local_provider_lifecycle.rs`

### Implementation for User Story 6

- [x] T088 [US6] Create local-provider lifecycle module in `app/src-tauri/src/pipeline/local_provider_lifecycle.rs`
- [x] T089 [US6] Register local-provider lifecycle module in `app/src-tauri/src/pipeline.rs`
- [x] T090 [US6] Move Local Whisper cache-key and load/unload behavior into local-provider lifecycle in `app/src-tauri/src/pipeline.rs`
- [x] T091 [US6] Delegate local-provider cache identity from STT Provider Resolution in `app/src-tauri/src/pipeline/stt_provider_resolver.rs`
- [x] T092 [US6] Delegate local-provider construction/readiness decisions from STT provider construction in `app/src-tauri/src/pipeline/stt_provider.rs`
- [x] T093 [US6] Preserve request-log and safe failure messages for local providers in `app/src-tauri/src/pipeline/stt_provider_resolver.rs`
- [x] T094 [US6] Record Local Provider Lifecycle coverage evidence and edge-case results in `specs/017-architecture-deepening-plan/validation/coverage-evidence.md`

**Checkpoint**: Local-provider lifecycle rules are localized and independently testable.

---

## Phase 10: User Story 7 - Provide a safe path for provider-family seams (Priority: P3)

**Goal**: Record two-adapter proof before introducing provider-family seams and implement only seams that pass the deletion test.

**Independent Test**: Select one provider-family concern at a time, prove existing behavior is characterized, and show at least two adapters satisfy the seam before implementation.

### Tests for User Story 7

- [x] T095 [P] [US7] Add managed-mode provider behavior characterization tests in `app/src-tauri/src/tests/managed_personal_tests.rs`
- [x] T096 [P] [US7] Add provider error classification characterization tests in `app/src-tauri/src/tests/stt_integration_tests.rs`
- [x] T097 [US7] Add provider metadata/redaction characterization tests in `app/src-tauri/src/tests/request_log_schema_tests.rs`
- [x] T098 [P] [US7] Add provider cost behavior characterization tests in `app/src-tauri/src/tests/pricing_llm_schema_tests.rs`

### Implementation for User Story 7

- [x] T099 [US7] Document managed-mode adapter two-adapter proof or deferral in `specs/017-architecture-deepening-plan/validation/provider-family-decisions.md`
- [x] T100 [US7] Document provider error-classification two-adapter proof or deferral in `specs/017-architecture-deepening-plan/validation/provider-family-decisions.md`
- [x] T101 [US7] Document provider metadata/redaction two-adapter proof or deferral in `specs/017-architecture-deepening-plan/validation/provider-family-decisions.md`
- [x] T102 [US7] Document provider cost two-adapter proof or deferral in `specs/017-architecture-deepening-plan/validation/provider-family-decisions.md`
- [x] T103 [US7] If the managed-mode concern passes two-adapter proof, implement the named managed-mode seam in `app/src-tauri/src/managed_inference/mod.rs`; otherwise record deferral in `specs/017-architecture-deepening-plan/validation/provider-family-decisions.md`
- [x] T104 [US7] If the provider error/metadata concern passes two-adapter proof, update the named STT provider call sites in `app/src-tauri/src/pipeline/stt_provider.rs`; otherwise record deferral in `specs/017-architecture-deepening-plan/validation/provider-family-decisions.md`
- [x] T105 [US7] If the provider error/cost concern passes two-adapter proof, update the named LLM provider call sites in `app/src-tauri/src/pipeline/llm_provider.rs`; otherwise record deferral in `specs/017-architecture-deepening-plan/validation/provider-family-decisions.md`
- [x] T106 [US7] Record Provider-Family Seam coverage evidence and deferral decisions in `specs/017-architecture-deepening-plan/validation/coverage-evidence.md`

**Checkpoint**: Provider-family seams are implemented only when real, or explicitly deferred with load-bearing reasons.

---

## Final Phase: Polish & Cross-Cutting Concerns

**Purpose**: Validate the full initiative, synchronize generated contracts if needed, and document completion evidence.

- [x] T107 [P] Update architecture domain terms added by completed slices in `CONTEXT.md`
- [x] T108 [P] Update out-of-scope or deferred refactors in `docs/Refactors/2_MEDIUM.md`
- [x] T109 [P] Update active Spec Kit implementation notes in `.github/copilot-instructions.md`
- [x] T110 Update generated schema/type check notes if contract files changed in `specs/017-architecture-deepening-plan/validation/coverage-evidence.md`
- [x] T111 Run Rust formatting and record results in `specs/017-architecture-deepening-plan/validation/coverage-evidence.md`
- [x] T112 Run TypeScript formatting/linting and record results in `specs/017-architecture-deepening-plan/validation/coverage-evidence.md`
- [x] T113 Run targeted Rust tests and coverage for completed Rust slices and record results in `specs/017-architecture-deepening-plan/validation/coverage-evidence.md`
- [x] T114 Run targeted TypeScript tests and coverage for completed TypeScript slices and record results in `specs/017-architecture-deepening-plan/validation/coverage-evidence.md`
- [x] T115 Run generated schema/type/event checks if touched and record results in `specs/017-architecture-deepening-plan/validation/coverage-evidence.md`
- [x] T116 Run final `pnpm -C app check:ci` and record results in `specs/017-architecture-deepening-plan/validation/coverage-evidence.md`
- [x] T117 Verify every edge-case matrix row has deterministic automated coverage in `specs/017-architecture-deepening-plan/validation/edge-case-matrix.md`
- [x] T118 Verify no slice remains incomplete against the safe-stop checklist and regression-defect log in `specs/017-architecture-deepening-plan/validation/slice-checklist.md` and `specs/017-architecture-deepening-plan/validation/coverage-evidence.md`

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies.
- **Foundational (Phase 2)**: Depends on Setup and blocks all story completion.
- **US8 Coverage Gate (Phase 3)**: Depends on Foundational and blocks completion claims for all implementation stories.
- **US1 OCR Session (Phase 4)**: Depends on Foundational and US8 coverage workflow.
- **US2 Settings View (Phase 5)**: Depends on Foundational and US8; independent of US1 implementation.
- **US3 Runtime Sync Policy (Phase 6)**: Depends on US2 because runtime sync classifications rely on setting keys and effective semantics.
- **US4 Routing Decision (Phase 7)**: Depends on Foundational and US8; independent of settings slices.
- **US5 Profile Resolution (Phase 8)**: Depends on Foundational and US8; may run after or beside US4 if call-site conflicts are coordinated.
- **US6 Local Provider Lifecycle (Phase 9)**: Depends on Foundational and US8; should run after US5 only if profile resolution touches the same STT call sites.
- **US7 Provider-Family Seam (Phase 10)**: Depends on US6 and characterization coverage discipline.
- **Polish**: Depends on all desired story slices.

### User Story Dependencies

- **US8 (P1)**: Quality gate; complete before claiming any other story done.
- **US1 (P1)**: MVP correctness slice after US8.
- **US2 (P1)**: Can proceed after US8; no dependency on US1.
- **US3 (P1)**: Depends on US2.
- **US4 (P2)**: Can proceed after US8.
- **US5 (P2)**: Can proceed after US8; coordinate with US4 due shared Transcription Flow call sites.
- **US6 (P2)**: Can proceed after US8; coordinate with US5 due STT Provider Resolution call sites.
- **US7 (P3)**: Starts after US6 and only implements seams with two-adapter proof.

### Within Each User Story

- Tests marked for the story must be written first and fail before implementation.
- Module-interface types/contracts should be introduced before call-site migration.
- Characterization tests must pass before moving behavior behind a deeper seam.
- Coverage evidence must be recorded before a story checkpoint is considered complete.
- Formatting must run before test/check commands for touched areas.

## Parallel Opportunities

- T003, T004, T005, and T006 can run in parallel after T002 creates the validation directory.
- T008, T010, and T012 can run in parallel after T007 starts coverage-tooling setup; T013 updates the same evidence file as T007.
- US1 test design can be split by scenario, but T025-T028 all edit `app/src-tauri/src/pipeline/ocr_session_state.rs` and should be applied sequentially.
- US2 tests T036, T038, T039, and T040 can run in parallel before US2 implementation; T037 shares `app/src/lib/tauri/settingsViews.test.ts` with T036.
- US3 tests T050, T053, and T054 can run in parallel before US3 implementation; T051-T052 share `app/src/lib/tauri/settingsSync.test.ts` with T050.
- US4 tests T063, T065, and T066 can run in parallel before US4 implementation; T064 shares `app/src-tauri/src/pipeline/routing.rs` with T063.
- US5 test design can be split by concern, but T074-T076 all edit `app/src-tauri/src/pipeline/profile_resolution.rs` and should be applied sequentially after or beside T073.
- US6 test design can be split by concern, but T084-T087 all edit `app/src-tauri/src/pipeline/local_provider_lifecycle.rs` and should be applied sequentially.
- US7 characterization tests T095-T098 can run in parallel before US7 decisions.
- Final documentation tasks T107-T109 can run in parallel after story slices settle; T110 updates the shared coverage evidence file.

## Parallel Example: User Story 1

```text
Task: "T025 [US1] Add OCR Session stale-success and stale-failure tests in app/src-tauri/src/pipeline/ocr_session_state.rs"
Task: "T026 [US1] Add OCR Session await-timeout-restore tests in app/src-tauri/src/pipeline/ocr_session_state.rs"
Task: "T027 [US1] Add OCR Session cancel/idempotency tests in app/src-tauri/src/pipeline/ocr_session_state.rs"
Task: "T028 [US1] Add OCR Session sanitized failure/status tests in app/src-tauri/src/pipeline/ocr_session_state.rs"
```

## Parallel Example: User Story 2

```text
Task: "T036 [P] [US2] Add TypeScript default/malformed/null Settings View tests in app/src/lib/tauri/settingsViews.test.ts"
Task: "T038 [P] [US2] Add Rust settings default drift tests in app/src-tauri/src/settings/default_values.rs"
Task: "T039 [P] [US2] Add legacy settings snapshot normalization tests in app/src/lib/tauri/settings.legacy.test.ts"
```

## Implementation Strategy

### MVP First

1. Complete Phase 1 Setup.
2. Complete Phase 2 Foundational prerequisites.
3. Complete Phase 3 US8 coverage gate.
4. Complete Phase 4 US1 OCR Session.
5. Stop and validate OCR Session independently with Rust tests and in-scope coverage evidence.

### Incremental Delivery

1. Foundation + US8 quality gate.
2. US1 OCR Session correctness MVP.
3. US2 Settings View drift prevention.
4. US3 Runtime Sync Policy.
5. US4 Routing Decision.
6. US5 Profile Resolution.
7. US6 Local Provider Lifecycle.
8. US7 Provider-Family Seam decisions and selected real seams.
9. Final polish and `check:ci`.

### Team Parallel Strategy

- One person can own US8 coverage tooling while another writes US1 characterization tests.
- US2 and US4 can proceed in parallel after US8 because they touch mostly different files.
- US3 should wait for US2 to avoid reworking setting keys/effective semantics.
- US5 and US6 should coordinate around `app/src-tauri/src/pipeline.rs` and STT Provider Resolution call sites.
