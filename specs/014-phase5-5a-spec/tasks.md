# Tasks: Phase 5 Dashboard + Phase 5A Admin Testing Path

**Input**: Design documents from `/specs/014-phase5-5a-spec/`
**Prerequisites**: `plan.md` (required), `spec.md` (required), `research.md`, `data-model.md`, `contracts/admin-dashboard.openapi.yaml`, `quickstart.md`

**Tests**: Include tests where they provide the fastest, most reliable lock on behavior.

- Tests MUST be deterministic.
- Tests MUST NOT make real network calls.
- Tests MUST NOT require real API keys by default.

**Organization**: Tasks are grouped by user story to enable independent implementation and testing.

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Initialize dashboard app surface and shared tooling for Phase 5/5A work.

- [x] T001 Create dashboard app scaffold in `kolboo-private/apps/admin-dashboard/package.json`, `kolboo-private/apps/admin-dashboard/tsconfig.json`, and `kolboo-private/apps/admin-dashboard/vite.config.ts`
- [x] T002 Create dashboard entry and shell in `kolboo-private/apps/admin-dashboard/src/main.tsx` and `kolboo-private/apps/admin-dashboard/src/App.tsx`
- [x] T003 [P] Configure Tailwind + shadcn base setup in `kolboo-private/apps/admin-dashboard/tailwind.config.ts`, `kolboo-private/apps/admin-dashboard/postcss.config.js`, and `kolboo-private/apps/admin-dashboard/src/styles.css`
- [x] T004 [P] Add dashboard scripts and workspace wiring in `kolboo-private/package.json` and `kolboo-private/pnpm-workspace.yaml`
- [x] T005 [P] Add dashboard lint/test config in `kolboo-private/apps/admin-dashboard/biome.json` and `kolboo-private/apps/admin-dashboard/vitest.config.ts`
- [x] T006 Add dashboard env template and runtime env parsing in `kolboo-private/apps/admin-dashboard/.env.example` and `kolboo-private/apps/admin-dashboard/src/lib/env.ts`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Establish shared auth/RBAC, data contracts, and API-client foundations that all stories depend on.

**⚠️ CRITICAL**: No user story work starts before this phase is complete.

- [x] T007 Create shared domain types from contract/data model in `kolboo-private/apps/admin-dashboard/src/lib/types.ts` and `kolboo-private/apps/api-edge/src/types/admin.ts`
- [x] T008 Implement Supabase JWT auth + allowlist middleware in `kolboo-private/apps/api-edge/src/middleware/auth.ts`
- [x] T009 [P] Implement role guard middleware (`owner`/`admin`/`viewer`) in `kolboo-private/apps/api-edge/src/middleware/rbac.ts`
- [x] T010 [P] Register base admin routes and middleware chain in `kolboo-private/apps/api-edge/src/index.ts` and `kolboo-private/apps/api-edge/src/routes/admin-router.ts`
- [x] T011 Implement dashboard API client and auth token handling in `kolboo-private/apps/admin-dashboard/src/lib/api/client.ts`
- [x] T012 [P] Implement React Router route guards in `kolboo-private/apps/admin-dashboard/src/router.tsx` and `kolboo-private/apps/admin-dashboard/src/components/auth/ProtectedRoute.tsx`
- [x] T013 [P] Implement TanStack Query provider/bootstrap in `kolboo-private/apps/admin-dashboard/src/lib/query-client.ts` and `kolboo-private/apps/admin-dashboard/src/main.tsx`
- [x] T014 Add deterministic shared test fixtures for org/user/role primitives in `kolboo-private/apps/api-edge/tests/fixtures/admin-fixtures.ts` and `kolboo-private/apps/admin-dashboard/src/test/fixtures.ts`

**Checkpoint**: Foundation complete; user stories can proceed.

---

## Phase 3: User Story 1 - Admin validates dashboard features end-to-end (Priority: P1) 🎯 MVP

**Goal**: Deterministic seed/reset and preview-usable baseline so SaaS admin can validate dashboard behavior quickly.

**Independent Test**: Run seed for `pr-<id>`, execute baseline member/policy/usage reads in dashboard, run reset, confirm only context-tagged records are removed.

### Tests for User Story 1

- [x] T015 [P] [US1] Add seed/reset route tests in `kolboo-private/apps/api-edge/tests/routes/test-fixtures.routes.test.ts`
- [x] T016 [P] [US1] Add seed/reset service unit tests in `kolboo-private/apps/api-edge/tests/services/preview-fixtures.service.test.ts`
- [x] T017 [P] [US1] Add dashboard fixture-control UI tests in `kolboo-private/apps/admin-dashboard/src/features/test-fixtures/TestFixturesPanel.test.tsx`

### Implementation for User Story 1

- [x] T018 [US1] Implement preview seed/reset service with context tagging in `kolboo-private/apps/api-edge/src/services/preview-fixtures.service.ts`
- [x] T019 [US1] Implement `/v1/test-fixtures/{contextKey}/seed` and `/v1/test-fixtures/{contextKey}/reset` handlers in `kolboo-private/apps/api-edge/src/routes/test-fixtures.ts`
- [x] T020 [P] [US1] Implement preview context persistence model in `kolboo-private/apps/api-edge/src/repositories/preview-context.repository.ts`
- [x] T021 [P] [US1] Implement dashboard test-fixture controls UI in `kolboo-private/apps/admin-dashboard/src/features/test-fixtures/TestFixturesPanel.tsx`
- [x] T022 [US1] Add dashboard query hooks for seed/reset actions in `kolboo-private/apps/admin-dashboard/src/features/test-fixtures/queries.ts`
- [x] T023 [US1] Add non-prod gating and production-block guard for fixture controls in `kolboo-private/apps/admin-dashboard/src/features/test-fixtures/guards.ts`
- [x] T024 [US1] Document local/preview seed-reset execution in `kolboo-private/docs/PHASE5A_TESTING_PATH.md`

**Checkpoint**: US1 independently functional and testable.

---

## Phase 4: User Story 2 - Org admins manage enterprise controls in dashboard (Priority: P2)

**Goal**: Deliver dashboard frontend + API support for members, policy publish, and usage/audit views with enforced RBAC.

**Independent Test**: In seeded org, complete member lifecycle, update+publish policy, and run usage/audit filters; verify admin succeeds and viewer is denied admin-only actions.

### Tests for User Story 2

- [x] T025 [P] [US2] Add members route tests in `kolboo-private/apps/api-edge/tests/routes/members.routes.test.ts`
- [x] T026 [P] [US2] Add policy route tests in `kolboo-private/apps/api-edge/tests/routes/policy.routes.test.ts`
- [x] T027 [P] [US2] Add usage/audit route tests in `kolboo-private/apps/api-edge/tests/routes/usage-audit.routes.test.ts`
- [x] T028 [P] [US2] Add RBAC permission matrix tests in `kolboo-private/apps/api-edge/tests/middleware/rbac.test.ts`
- [x] T029 [P] [US2] Add members page UI tests in `kolboo-private/apps/admin-dashboard/src/features/members/MembersPage.test.tsx`
- [x] T030 [P] [US2] Add policy editor UI tests in `kolboo-private/apps/admin-dashboard/src/features/policy/PolicyEditorPage.test.tsx`
- [x] T031 [P] [US2] Add usage/audit filter UI tests in `kolboo-private/apps/admin-dashboard/src/features/usage-audit/UsageAuditPage.test.tsx`

### Implementation for User Story 2

- [x] T032 [US2] Implement members repository/service in `kolboo-private/apps/api-edge/src/services/members.service.ts` and `kolboo-private/apps/api-edge/src/repositories/members.repository.ts`
- [x] T033 [US2] Implement members endpoints (`GET/POST/PATCH`) in `kolboo-private/apps/api-edge/src/routes/members.ts`
- [x] T034 [US2] Implement policy draft/publish service with versioning in `kolboo-private/apps/api-edge/src/services/policy.service.ts`
- [x] T035 [US2] Implement policy endpoints (`PUT /draft`, `POST /publish`) in `kolboo-private/apps/api-edge/src/routes/policy.ts`
- [x] T036 [US2] Implement usage/audit query service + endpoint in `kolboo-private/apps/api-edge/src/services/usage-audit.service.ts` and `kolboo-private/apps/api-edge/src/routes/usage-audit.ts`
- [x] T037 [P] [US2] Implement members frontend page/components in `kolboo-private/apps/admin-dashboard/src/features/members/MembersPage.tsx`, `kolboo-private/apps/admin-dashboard/src/features/members/components/MemberTable.tsx`, and `kolboo-private/apps/admin-dashboard/src/features/members/components/MemberEditorDialog.tsx`
- [x] T038 [P] [US2] Implement policy editor frontend page/components in `kolboo-private/apps/admin-dashboard/src/features/policy/PolicyEditorPage.tsx`, `kolboo-private/apps/admin-dashboard/src/features/policy/components/PolicyEditorForm.tsx`, and `kolboo-private/apps/admin-dashboard/src/features/policy/components/PublishPolicyButton.tsx`
- [x] T039 [P] [US2] Implement usage/audit frontend page/components in `kolboo-private/apps/admin-dashboard/src/features/usage-audit/UsageAuditPage.tsx`, `kolboo-private/apps/admin-dashboard/src/features/usage-audit/components/UsageAuditFilters.tsx`, and `kolboo-private/apps/admin-dashboard/src/features/usage-audit/components/UsageAuditTable.tsx`
- [x] T040 [US2] Implement frontend query hooks and API adapters for members/policy/usage in `kolboo-private/apps/admin-dashboard/src/features/members/queries.ts`, `kolboo-private/apps/admin-dashboard/src/features/policy/queries.ts`, and `kolboo-private/apps/admin-dashboard/src/features/usage-audit/queries.ts`
- [x] T041 [US2] Wire protected routes/navigation and role-aware UI affordances in `kolboo-private/apps/admin-dashboard/src/router.tsx` and `kolboo-private/apps/admin-dashboard/src/components/layout/AppNav.tsx`

**Checkpoint**: US2 independently functional and testable.

---

## Phase 5: User Story 3 - Operators release safely from preview to production (Priority: P3)

**Goal**: Enforce local→preview→production release path with manual prod approval and smoke evidence recording.

**Independent Test**: PR runs preview deploy + smoke + evidence; production deploy requires manual approval and records smoke outcome/rollback notes.

### Tests for User Story 3

- [x] T042 [P] [US3] Add release evidence route tests in `kolboo-private/apps/api-edge/tests/routes/release-evidence.routes.test.ts`
- [x] T043 [P] [US3] Add production smoke script tests in `kolboo-private/tests/phase5/prod-smoke-check.test.ts`

### Implementation for User Story 3

- [x] T044 [US3] Implement release evidence endpoint in `kolboo-private/apps/api-edge/src/routes/release-evidence.ts` and `kolboo-private/apps/api-edge/src/services/release-evidence.service.ts`
- [x] T045 [US3] Add preview workflow gates for dashboard/API smoke in `kolboo-private/.github/workflows/preview.yml`
- [x] T046 [US3] Add manual-approval + smoke + evidence gate in `kolboo-private/.github/workflows/deploy.yml`
- [x] T047 [US3] Implement production smoke runner with auth-guard + policy draft→publish roundtrip checks in `kolboo-private/scripts/phase0b/prod-smoke-check.mjs`
- [x] T048 [US3] Update operator runbook with gate evidence + rollback execution in `kolboo-private/docs/PHASE0B_RELEASE_WORKFLOW.md` and `kolboo-private/docs/PHASE5A_TESTING_PATH.md`

**Checkpoint**: US3 independently functional and testable.

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Final hardening across stories and readiness verification.

- [x] T049 [P] Validate OpenAPI contract remains aligned with implementation in `specs/014-phase5-5a-spec/contracts/admin-dashboard.openapi.yaml` and `kolboo-private/apps/api-edge/contracts/admin-dashboard.openapi.yaml`
- [x] T050 [P] Add/refresh quickstart verification notes in `specs/014-phase5-5a-spec/quickstart.md`
- [x] T051 Run final quality gates referenced by `kolboo-private/apps/admin-dashboard/package.json` and `kolboo-private/package.json` (dashboard lint/test and preview/prod smoke entrypoints)
- [x] T052 Perform security/logging review for secret redaction in `kolboo-private/apps/api-edge/src/middleware/*.ts` and `kolboo-private/scripts/phase0b/*.mjs`
- [x] T053 Add admin API performance validation (p95 target) in `kolboo-private/scripts/phase5/benchmark-admin-api.mjs` and `kolboo-private/tests/phase5/benchmark-admin-api.test.ts`
- [x] T054 Add backend production hard-stop tests for non-prod fixture/test-access routes in `kolboo-private/apps/api-edge/tests/routes/test-access-prod-guard.test.ts`
- [x] T055 Implement backend environment hard-stop for fixture/test-access routes in `kolboo-private/apps/api-edge/src/middleware/nonprod-only.ts` and `kolboo-private/apps/api-edge/src/routes/test-fixtures.ts`

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 1 (Setup)**: Can start immediately.
- **Phase 2 (Foundational)**: Depends on Phase 1; blocks all user stories.
- **Phase 3 (US1)**: Depends on Phase 2; MVP slice.
- **Phase 4 (US2)**: Depends on Phase 2; can run in parallel with US3 implementation once capacity allows.
- **Phase 5 (US3)**: Depends on Phase 2; final release-gate validation depends on US1/US2 checks being available.
- **Phase 6 (Polish)**: Depends on desired user stories being complete.

### User Story Dependencies

- **US1 (P1)**: No dependency on other stories; first deliverable MVP.
- **US2 (P2)**: Independent of US1 for implementation, but practically validated faster with US1 fixture path.
- **US3 (P3)**: Implementation can begin after Phase 2; final validation depends on US1/US2 dashboard/API checks.

### Within Each User Story

- Tests first where feasible.
- Repository/service before route handler.
- Backend contract before frontend integration hooks.
- Route/page wiring after core logic passes tests.

---

## Parallel Execution Examples

### User Story 1

- Run in parallel:
  - T015, T016, T017 (test authoring in separate files)
  - T020 and T021 (backend context persistence + frontend control panel)

### User Story 2

- Run in parallel:
  - T025–T031 (tests by route/page area)
  - T032, T034, T036 (independent backend services)
  - T037, T038, T039 (frontend feature pages)

### User Story 3

- Run in parallel:
  - T042 and T043 (API route + script tests)
  - T045 and T047 (workflow gate + smoke command improvements)

---

## Implementation Strategy

### MVP First (US1 only)

1. Complete Phase 1 and Phase 2.
2. Deliver Phase 3 (US1) seed/reset + non-prod controls.
3. Validate independent test criteria for US1.
4. Demo preview-ready admin validation path.

### Incremental Delivery

1. Setup + foundational baseline.
2. Add US1 (deterministic testing path).
3. Add US2 (core dashboard admin workflows).
4. Add US3 (release gate safety + evidence).
5. Finish polish and readiness checks.

### Suggested MVP Scope

- **MVP scope**: User Story 1 only (T015–T024) after setup/foundation.
