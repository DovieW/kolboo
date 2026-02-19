# Tasks: Finish Remaining Phase 5/5A/5B Readiness

**Input**: Design documents from `/specs/015-finish-phase5-readiness/`
**Prerequisites**: `plan.md`, `spec.md`, `research.md`, `data-model.md`, `contracts/`, `quickstart.md`

**Tests**: Include deterministic tests to lock behavior for each user story.

- Tests MUST be deterministic.
- Tests MUST NOT make real network calls.
- Tests MUST NOT require real API keys by default.

**Organization**: Tasks are grouped by user story to keep each story independently implementable and testable.

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Prepare cross-repo workspace wiring for Phase 5/5A/5B completion work.

- [x] T001 Create feature tracking docs index in `specs/015-finish-phase5-readiness/README.md`
- [x] T002 Add shared env placeholders for remaining admin/billing/test-access surfaces in `kolboo-private/.env.example`
- [x] T003 [P] Add workspace scripts for Phase 5 readiness checks in `kolboo-private/package.json`
- [x] T004 [P] Add desktop-side feature flags/placeholders for enterprise test persona indicators in `kolboo/app/src/lib/tauri/settings.ts`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Define shared contracts, authz rules, and audit primitives needed by all stories.

**⚠️ CRITICAL**: No user story implementation starts before this phase is complete.

- [x] T005 Create/align shared admin domain types for new entities in `kolboo-private/apps/api-edge/src/types/admin.ts`
- [x] T006 [P] Implement shared audit action writer for admin + `kolops` operations in `kolboo-private/apps/api-edge/src/services/audit-actions.service.ts`
- [x] T007 [P] Add RBAC guards for shared profile and billing access surfaces in `kolboo-private/apps/api-edge/src/middleware/rbac.ts`
- [x] T008 [P] Wire shared-profile, billing-access, and test-access routes into router in `kolboo-private/apps/api-edge/src/routes/admin-router.ts` and `kolboo-private/apps/api-edge/src/index.ts`
- [x] T009 Add contract parity check notes for API + CLI in `specs/015-finish-phase5-readiness/contracts/README.md`
- [x] T010 Add deterministic fixture context schema helpers in `kolboo-private/apps/api-edge/src/repositories/preview-context.repository.ts`

**Checkpoint**: Foundation complete; stories can proceed independently.

---

## Phase 3: User Story 1 - Complete enterprise admin workflows (Priority: P1) 🎯 MVP

**Goal**: Deliver full org-admin account-control workflows (shared profiles, billing access, plus desktop enterprise touchpoints).

**Independent Test**: Admin can complete member/policy/usage/shared-profile/billing flows in seeded org; viewer is denied restricted actions.

### Tests for User Story 1

- [x] T011 [P] [US1] Add shared profiles route tests in `kolboo-private/apps/api-edge/tests/routes/shared-profiles.routes.test.ts`
- [x] T012 [P] [US1] Add billing access route tests in `kolboo-private/apps/api-edge/tests/routes/billing-access.routes.test.ts`
- [x] T013 [P] [US1] Add shared profiles UI tests in `kolboo-private/apps/admin-dashboard/src/features/shared-profiles/SharedProfilesPage.test.tsx`
- [x] T014 [P] [US1] Add billing management UI tests in `kolboo-private/apps/admin-dashboard/src/features/billing/BillingPage.test.tsx`
- [x] T015 [P] [US1] Add desktop enterprise-surface integration tests in `kolboo/app/src/components/settings/EnterpriseAccountControls.test.tsx`

### Implementation for User Story 1

- [x] T016 [US1] Implement shared profiles repository/service in `kolboo-private/apps/api-edge/src/repositories/shared-profiles.repository.ts` and `kolboo-private/apps/api-edge/src/services/shared-profiles.service.ts`
- [x] T017 [US1] Implement shared profiles endpoints (`GET/POST/PATCH`) in `kolboo-private/apps/api-edge/src/routes/shared-profiles.ts`
- [x] T018 [US1] Implement billing access service and endpoint in `kolboo-private/apps/api-edge/src/services/billing-access.service.ts` and `kolboo-private/apps/api-edge/src/routes/billing-access.ts`
- [x] T019 [P] [US1] Implement shared profiles dashboard page/components in `kolboo-private/apps/admin-dashboard/src/features/shared-profiles/SharedProfilesPage.tsx` and `kolboo-private/apps/admin-dashboard/src/features/shared-profiles/components/SharedProfileEditorDialog.tsx`
- [x] T020 [P] [US1] Implement billing management dashboard page/components in `kolboo-private/apps/admin-dashboard/src/features/billing/BillingPage.tsx` and `kolboo-private/apps/admin-dashboard/src/features/billing/components/BillingAccessPanel.tsx`
- [x] T021 [US1] Wire dashboard query hooks for shared profiles and billing access in `kolboo-private/apps/admin-dashboard/src/features/shared-profiles/queries.ts` and `kolboo-private/apps/admin-dashboard/src/features/billing/queries.ts`
- [x] T022 [US1] Add nav and role-aware route guards for new admin pages in `kolboo-private/apps/admin-dashboard/src/components/layout/AppNav.tsx` and `kolboo-private/apps/admin-dashboard/src/router.tsx`
- [x] T023 [US1] Add desktop enterprise account-control surface integration in `kolboo/app/src/components/settings/Settings.tsx` and `kolboo/app/src/components/settings/EnterpriseAccountControls.tsx`
- [x] T024 [US1] Update desktop tauri types/wrappers for enterprise account surfaces in `kolboo/app/src/lib/tauri/types.ts` and `kolboo/app/src/lib/tauri/enterpriseAdmin.ts`

**Checkpoint**: US1 independently functional and testable.

---

## Phase 4: User Story 2 - Deterministic non-production admin testing path (Priority: P2)

**Goal**: Deliver repeatable local/preview/staging validation with seeded contexts, stable personas, reversible fallback.

**Independent Test**: Seed/validate/reset works per context; staging persona scripts produce deterministic pass/fail evidence; fallback runbook is reversible.

### Tests for User Story 2

- [x] T025 [P] [US2] Add seed/reset idempotency and isolation tests in `kolboo-private/apps/api-edge/tests/services/preview-fixtures.personas.test.ts`
- [x] T026 [P] [US2] Add staging persona smoke script tests in `kolboo-private/tests/phase5/staging-personas-smoke.test.ts`
- [x] T027 [P] [US2] Add dashboard persona-indicator UI tests in `kolboo/app/src/components/settings/TestPersonaIndicator.test.tsx`

### Implementation for User Story 2

- [x] T028 [US2] Extend fixture seeding for BYOK/managed/mixed-policy staging personas in `kolboo-private/apps/api-edge/src/services/preview-fixtures.service.ts`
- [x] T029 [US2] Implement staging persona bootstrap/reset script in `kolboo-private/scripts/phase5/staging-personas.mjs`
- [x] T030 [US2] Add preview workflow hook for persona smoke execution in `kolboo-private/.github/workflows/preview.yml`
- [x] T031 [US2] Document reversible SQL/CLI fallback playbook in `kolboo-private/docs/PHASE5A_TESTING_PATH.md` and `kolboo-private/docs/PHASE5A_SQL_CLI_PLAYBOOK.md`
- [x] T032 [US2] Implement desktop non-production persona context indicator in `kolboo/app/src/components/settings/TestPersonaIndicator.tsx`
- [x] T033 [US2] Wire desktop persona context state and events in `kolboo/app/src/lib/queries/enterprisePersona.ts` and `kolboo/app/src/lib/tauri/types.ts`

**Checkpoint**: US2 independently functional and testable.

---

## Phase 5: User Story 3 - Production-safe test access and release evidence (Priority: P3)

**Goal**: Enforce non-prod-only test access with audit trail and maintain release evidence safety gates.

**Independent Test**: Non-prod can create short-lived test-access sessions with audits; production remains hard-blocked with deterministic response; release evidence captures required fields.

### Tests for User Story 3

- [x] T034 [P] [US3] Add test-access session route tests (env guard + TTL + audit) in `kolboo-private/apps/api-edge/tests/routes/test-access-session.routes.test.ts`
- [x] T035 [P] [US3] Add production hard-stop regression tests for test-access endpoint in `kolboo-private/apps/api-edge/tests/routes/test-access-prod-guard.test.ts`
- [x] T036 [P] [US3] Add release evidence schema validation tests in `kolboo-private/apps/api-edge/tests/routes/release-evidence.routes.test.ts`

### Implementation for User Story 3

- [x] T037 [US3] Implement test-access session service and persistence in `kolboo-private/apps/api-edge/src/services/test-access-session.service.ts` and `kolboo-private/apps/api-edge/src/repositories/test-access-session.repository.ts`
- [x] T038 [US3] Implement `POST /v1/test-access/session` endpoint with non-prod hard-stop in `kolboo-private/apps/api-edge/src/routes/test-access-session.ts`
- [x] T039 [US3] Emit audit events for session start/expiry/revoke in `kolboo-private/apps/api-edge/src/services/audit-actions.service.ts`
- [x] T040 [US3] Update deploy workflow evidence requirements for smoke/rollback linkage in `kolboo-private/.github/workflows/deploy.yml`
- [x] T041 [US3] Update release workflow runbook for test-access and evidence policy in `kolboo-private/docs/PHASE0B_RELEASE_WORKFLOW.md`
- [x] T042 [US3] Add desktop badge for active non-prod test-access session in `kolboo/app/src/components/settings/TestPersonaIndicator.tsx`

**Checkpoint**: US3 independently functional and testable.

---

## Phase 6: User Story 4 - Platform admin operations through kolops (Priority: P3)

**Goal**: Deliver audited `kolops` command groups for org/policy/keys/entitlement/usage/audit operations.

**Independent Test**: Each command group runs with deterministic output, enforced authorization, and auditable records.

### Tests for User Story 4

- [x] T043 [P] [US4] Add `kolops org` command tests in `kolboo-private/apps/kolops-cli/tests/org.commands.test.ts`
- [x] T044 [P] [US4] Add `kolops policy` + `kolops keys` command tests in `kolboo-private/apps/kolops-cli/tests/policy-keys.commands.test.ts`
- [x] T045 [P] [US4] Add entitlement/usage/audit command tests in `kolboo-private/apps/kolops-cli/tests/entitlement-usage-audit.commands.test.ts`
- [x] T046 [P] [US4] Add audit emission integration tests for CLI actions in `kolboo-private/apps/api-edge/tests/services/platform-admin-actions.audit.test.ts`

### Implementation for User Story 4

- [x] T047 [US4] Implement `kolops org` command handlers in `kolboo-private/apps/kolops-cli/src/commands/org.ts`
- [x] T048 [US4] Implement `kolops policy` command handlers in `kolboo-private/apps/kolops-cli/src/commands/policy.ts`
- [x] T049 [US4] Implement `kolops keys` command handlers with secret redaction in `kolboo-private/apps/kolops-cli/src/commands/keys.ts`
- [x] T050 [US4] Implement `kolops entitlement` command handlers in `kolboo-private/apps/kolops-cli/src/commands/entitlement.ts`
- [x] T051 [US4] Implement `kolops usage` and `kolops audit` command handlers in `kolboo-private/apps/kolops-cli/src/commands/usage.ts` and `kolboo-private/apps/kolops-cli/src/commands/audit.ts`
- [x] T052 [US4] Implement shared CLI output/exit-code contract helpers in `kolboo-private/apps/kolops-cli/src/lib/output.ts`
- [x] T053 [US4] Wire CLI action audit sink into api-edge services in `kolboo-private/apps/api-edge/src/services/platform-admin-actions.service.ts`
- [x] T054 [US4] Document operator CLI usage and safety guardrails in `kolboo-private/docs/PHASE5B_KOLOPS_RUNBOOK.md`

**Checkpoint**: US4 independently functional and testable.

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: Final parity, security, performance, and validation across all stories.

- [x] T055 [P] Align contracts with implementation for API + CLI in `specs/015-finish-phase5-readiness/contracts/admin-phase5-readiness.openapi.yaml`, `specs/015-finish-phase5-readiness/contracts/kolops-cli.contract.md`, and `kolboo-private/apps/api-edge/contracts/admin-dashboard.openapi.yaml`
- [x] T056 [P] Refresh quickstart validation steps and evidence capture in `specs/015-finish-phase5-readiness/quickstart.md`
- [x] T057 Run targeted format/lint/test checks for touched areas in `kolboo/app/package.json` and `kolboo-private/package.json`
- [x] T058 Run final CI validation gate in `kolboo/app` via `pnpm -C app check:ci` and in `kolboo-private` via repository CI-equivalent command scripts
- [x] T059 [P] Perform security/logging redaction review for dashboard/api-edge/kolops surfaces in `kolboo-private/apps/api-edge/src/**`, `kolboo-private/apps/kolops-cli/src/**`, and `kolboo/app/src/**`

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 1 (Setup)**: Starts immediately.
- **Phase 2 (Foundational)**: Depends on Setup; blocks all stories.
- **Phase 3 (US1)**: Depends on Foundational.
- **Phase 4 (US2)**: Depends on Foundational; best validated after US1 surfaces exist.
- **Phase 5 (US3)**: Depends on Foundational; uses US2 validation path.
- **Phase 6 (US4)**: Depends on Foundational; mostly independent of US1/US2 UI delivery.
- **Phase 7 (Polish)**: Depends on all desired stories being complete.

### User Story Dependencies

- **US1 (P1)**: First MVP story; no dependency on other stories after foundation.
- **US2 (P2)**: Uses admin workflow surfaces from US1 for richer validation.
- **US3 (P3)**: Depends on US2 non-production flow and existing release evidence path.
- **US4 (P3)**: Independent of UI stories after foundational auth/audit primitives.

### Parallel Opportunities

- Setup tasks marked [P] can run together.
- Foundational tasks T006–T008 can run in parallel.
- US1 backend and frontend tasks T019/T020 can run in parallel after T016–T018.
- US2 seed/persona scripting and desktop indicator work (T029, T032, T033) can run in parallel.
- US3 route/service and workflow/runbook tasks can run in parallel after T037.
- US4 command-group implementations T047–T051 can run in parallel by command group.

## Parallel Execution Examples

### User Story 1

- Run in parallel:
  - T011, T012, T013, T014, T015
  - T019 and T020

### User Story 2

- Run in parallel:
  - T025, T026, T027
  - T029 with T032/T033

### User Story 3

- Run in parallel:
  - T034, T035, T036
  - T040 and T041

### User Story 4

- Run in parallel:
  - T043, T044, T045, T046
  - T047, T048, T049, T050, T051

## Implementation Strategy

### MVP First

1. Complete Setup + Foundational (Phase 1–2).
2. Deliver US1 (Phase 3) as the first business-ready increment.
3. Validate US1 independently before starting broader readiness hardening.

### Incremental Delivery

1. US1 admin workflows
2. US2 deterministic validation path
3. US3 production-safe test-access and release evidence hardening
4. US4 `kolops` operator CLI completion
5. Polish and final validation gates

### Suggested MVP Scope

- **MVP**: US1 only (Phase 3) after Setup/Foundation.
- **Operational readiness extension**: US2 + US3.
- **Platform operations completion**: US4.
