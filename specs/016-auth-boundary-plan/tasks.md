# Tasks: Identity-Policy Boundary for Desktop Auth

**Input**: Design documents from `/specs/016-auth-boundary-plan/`
**Prerequisites**: `plan.md`, `spec.md`, `research.md`, `data-model.md`, `contracts/auth-boundary.openapi.yaml`, `quickstart.md`

**Tests**: Include deterministic unit/contract tests where they are the fastest way to lock in behavior.

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Prepare implementation scaffolding and artifact alignment.

- [x] T001 Align feature docs header/links in `specs/016-auth-boundary-plan/spec.md`, `specs/016-auth-boundary-plan/plan.md`, and `specs/016-auth-boundary-plan/quickstart.md`
- [x] T002 Normalize auth boundary contract metadata/version notes in `specs/016-auth-boundary-plan/contracts/auth-boundary.openapi.yaml`
- [x] T003 [P] Add implementation decision log stub in `specs/016-auth-boundary-plan/research.md` for token-exchange trigger reviews

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core baseline required before any user story work.

**⚠️ CRITICAL**: No user story implementation starts before this phase is complete.

- [x] T004 Define shared auth domain types in `app/src/lib/tauri/types.ts` and `app/src-tauri/src/commands/licensing.rs`
- [x] T005 [P] Add shared auth error/reason-code mapping in `app/src/lib/tauri/license.ts` and `app/src/lib/tauri/commands.ts`
- [x] T006 [P] Add secure-storage lifecycle helpers for auth session material in `app/src-tauri/src/secrets.rs`
- [x] T007 Add auth contract wrapper entrypoints in `app/src/lib/tauri/commands.ts` for `auth/context` and managed auth failures
- [x] T008 Add secrets-redaction guardrails for auth paths in `app/src/lib/telemetry/sentry.ts` and `app/src/lib/telemetry/posthog.ts`

**Checkpoint**: Foundation complete — user stories can proceed independently.

---

## Phase 3: User Story 1 - Secure Sign-In and Access (Priority: P1) 🎯 MVP

**Goal**: Deliver PKCE desktop sign-in, secure session handling, and authenticated managed-access flow.

**Independent Test**: Sign in via browser flow, persist session securely, restart app, and successfully execute one managed operation requiring auth.

### Tests for User Story 1

- [x] T009 [P] [US1] Add frontend auth lifecycle tests in `app/src/lib/tauri/license.test.ts`
- [x] T010 [P] [US1] Add backend session lifecycle tests in `app/src-tauri/src/commands/licensing.rs`

### Implementation for User Story 1

- [x] T011 [US1] Implement Authorization Code + PKCE loopback flow orchestration in `app/src-tauri/src/commands/licensing.rs`
- [x] T012 [P] [US1] Implement sign-in UX and callback wiring in `app/src/components/settings/AccountSettings.tsx` and `app/src/lib/tauri/license.ts`
- [x] T013 [US1] Persist refresh/session secrets in OS secure storage via `app/src-tauri/src/secrets.rs` and auth command integration in `app/src-tauri/src/commands/licensing.rs`
- [x] T014 [US1] Implement startup async refresh and privileged-call gating in `app/src/main.tsx` and `app/src/lib/queries.ts`
- [x] T015 [US1] Implement logout wipe (secure storage + cached auth snapshot) in `app/src-tauri/src/commands/licensing.rs` and `app/src/lib/tauri/license.ts`
- [x] T016 [US1] Add re-auth-required UX state handling in `app/src/components/settings/AccountSettings.tsx` and `app/src/lib/queries.ts`

**Checkpoint**: US1 is independently functional and demoable (MVP).

---

## Phase 4: User Story 2 - Centralized Policy Enforcement (Priority: P2)

**Goal**: Ensure managed operations are authorized by edge policy with clear denial reasons and metering awareness, with explicit desktop-client and edge-runtime ownership.

**Independent Test**: Call managed operation with valid vs invalid auth/policy contexts and verify correct allow/deny handling with reason codes.

### Tests for User Story 2

 [x] T017 [P] [US2] Add managed auth contract tests in `app/src/lib/tauri/managedInference.test.ts`
 [x] T021 [US2] Enforce **desktop client** bearer-auth attachment and error handling for managed inference path in `app/src/lib/tauri/managedInference.ts`
 [x] T023 [US2] Preserve BYOK/non-managed behavior when managed auth path is unavailable in `app/src/lib/tauri/managedInference.ts` and `app/src/lib/tauri/managedInference.test.ts`

### Implementation for User Story 2

- [x] T019 [US2] Implement **desktop client** auth-context retrieval command and response mapping in `app/src-tauri/src/commands/licensing.rs` and command registration in `app/src-tauri/src/lib.rs`
- [x] T020 [P] [US2] Implement frontend auth-context wrapper and normalization in `app/src/lib/tauri/commands.ts` and `app/src/lib/tauri/license.ts`
- [x] T021 [US2] Enforce **desktop client** bearer-auth attachment and error handling for managed inference path in `app/src/lib/tauri/managedInference.ts`
- [x] T022 [US2] Map deny reason codes to user-actionable UI outcomes in `app/src/lib/queries.ts` and `app/src/components/settings/AccountSettings.tsx`
- [x] T023 [US2] Preserve BYOK/non-managed behavior when managed auth path is unavailable in `app/src/lib/tauri/managedInference.ts` and `app/src/lib/tauri/managedInference.test.ts`
- [x] T024 [US2] Extend contract examples for 401/402/403 + reason-code coverage in `specs/016-auth-boundary-plan/contracts/auth-boundary.openapi.yaml`
- [x] T037 [US2] Implement edge JWT verification (`iss`/`aud`/`exp`/`nbf`) and JWKS rotation handling in `c:/Users/dovie/repos/kolboo-private/apps/api-edge/src/middleware/auth.ts`
- [x] T038 [US2] Implement edge org membership + entitlement + policy evaluation in `c:/Users/dovie/repos/kolboo-private/apps/api-edge/src/middleware/rbac.ts` and `c:/Users/dovie/repos/kolboo-private/apps/api-edge/src/services/policy.service.ts`
- [x] T039 [US2] Implement edge metering enforcement/write path for managed requests in `c:/Users/dovie/repos/kolboo-private/apps/api-edge/src/services/metering-ledger.ts` and `c:/Users/dovie/repos/kolboo-private/apps/api-edge/src/routes/managed-inference.ts`
- [x] T040 [US2] Categorize and emit authz denial reason codes for observability in `c:/Users/dovie/repos/kolboo-private/apps/api-edge/src/routes/managed-inference.ts` and `c:/Users/dovie/repos/kolboo-private/apps/api-edge/src/index.ts`

**Checkpoint**: US1 and US2 both work independently and together.

---

## Phase 5: User Story 3 - Enterprise-Ready Evolution Path (Priority: P3)

**Goal**: Implement objective token-exchange trigger logic and optional exchange readiness path.

**Independent Test**: Evaluate trigger inputs and verify architecture mode output (`direct_idp_token` vs `adopt_token_exchange`) without affecting existing managed path.

### Tests for User Story 3

- [x] T025 [P] [US3] Add token-exchange trigger evaluation tests in `app/src/lib/auth/tokenExchangeGate.test.ts`
- [x] T026 [P] [US3] Add backend trigger-state normalization tests in `app/src-tauri/src/commands/licensing.rs`

### Implementation for User Story 3

- [x] T027 [US3] Implement token-exchange trigger evaluator in `app/src/lib/auth/tokenExchangeGate.ts`
- [x] T028 [US3] Add trigger-state persistence/normalization in `app/src/lib/tauri/settings.ts` and migration/default seeding in `app/src-tauri/src/lib.rs`
- [x] T029 [US3] Add optional session-exchange command placeholder and TS wrapper in `app/src-tauri/src/commands/licensing.rs` and `app/src/lib/tauri/license.ts`
- [x] T030 [US3] Extend OpenAPI contract for session-exchange readiness and decision fields in `specs/016-auth-boundary-plan/contracts/auth-boundary.openapi.yaml`
- [x] T031 [US3] Document operational trigger review checklist in `specs/016-auth-boundary-plan/quickstart.md`

**Checkpoint**: All stories are independently testable and implementation-ready.

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Final hardening, documentation, and validation.

- [x] T032 [P] Update architecture notes and rationale roll-up in `specs/016-auth-boundary-plan/research.md` and `specs/016-auth-boundary-plan/plan.md`
- [x] T033 Run targeted validation for touched areas via `pnpm -C app test` and `pnpm -C app cargo:test` and record outcomes in `specs/016-auth-boundary-plan/quickstart.md`
- [x] T034 Run final CI gate `pnpm -C app check:ci` and resolve remaining issues in touched files under `app/src/**` and `app/src-tauri/src/**`

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 1 (Setup)**: no dependencies
- **Phase 2 (Foundational)**: depends on Phase 1; blocks all stories
- **Phase 3 (US1)**: depends on Phase 2
- **Phase 4 (US2)**: depends on Phase 2; can run parallel to US1 after foundation, but integrates cleanly after US1 checkpoint
- **Phase 5 (US3)**: depends on Phase 2; safest after US1+US2 contract baselines
- **Phase 6 (Polish)**: depends on completion of selected stories

### User Story Dependencies

- **US1 (P1)**: independent after foundational phase
- **US2 (P2)**: independent after foundational phase, but has two coordinated tracks: desktop client integration (`app/**`) and edge runtime enforcement (`kolboo-private/apps/api-edge/**`)
- **US3 (P3)**: independent trigger logic after foundational phase; optional exchange wrapper reuses US1/US2 auth surfaces

### Within Each User Story

- Write tests before implementation and ensure they fail first.
- Implement backend/frontend pieces in parallel where files do not overlap.
- Complete story wiring and error handling before declaring checkpoint done.

## Parallel Opportunities

- Foundational: T005, T006, T007 can run in parallel after T004.
- US1: T009 and T010 in parallel; T012 can run parallel with T011/T013.
- US2: T017/T018 (desktop tests) and T035/T036 (edge tests) can run in parallel; T020/T024 can run parallel with T019; T037/T038 can run parallel with desktop integration tasks.
- US3: T025 and T026 in parallel; T028 and T030 can run parallel after T027 starts.

## Parallel Example: User Story 1

- Run together:
  - T009 [US1] `app/src/lib/tauri/license.test.ts`
  - T010 [US1] `app/src-tauri/src/commands/licensing.rs` tests
- Build together after tests exist:
  - T011 [US1] backend PKCE orchestration
  - T012 [US1] frontend sign-in UX wiring

## Implementation Strategy

### MVP First (US1)

1. Complete Phase 1 + Phase 2.
2. Deliver US1 (Phase 3) end-to-end.
3. Validate secure sign-in + managed call success.
4. Demo/review before expanding scope.

### Incremental Delivery

1. US1: secure sign-in + session lifecycle.
2. US2: edge policy enforcement and deny reason handling.
3. US3: token-exchange trigger governance and optional exchange readiness.
4. Polish with final CI gate.

### Notes

- `[P]` means tasks touch separate files and can execute concurrently.
- Keep all tests deterministic and network-free by default.
- Do not log secrets, tokens, or raw auth headers in any layer.
