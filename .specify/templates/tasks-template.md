---

description: "Task list template for feature implementation"
---

# Tasks: [FEATURE NAME]

**Input**: Design documents from `/specs/[###-feature-name]/`
**Prerequisites**: plan.md (required), spec.md (required for user stories), research.md, data-model.md, contracts/

**Tests**: Behavior changes MUST include focused deterministic validation unless the plan documents why the change is docs-only, wiring-only, or already covered. Tests MUST NOT require real network calls, API keys, paid accounts, audio devices, or timing sleeps by default.

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- Include exact file paths in descriptions

## Path Conventions

- **UI**: `app/src/**` with Vitest tests next to relevant code where practical
- **Tauri/backend**: `app/src-tauri/src/**` with Rust unit tests inline or under `app/src-tauri/src/tests/**`
- **Generated contracts**: `app/src-tauri/gen/**`, `app/src/lib/tauri/*.generated.ts`
- **Scripts/tooling**: `app/scripts/**`
- **Docs/refactors/user docs**: `docs/**`
- **Specs**: `specs/[###-feature-name]/**`

<!--
  ============================================================================
  IMPORTANT: The tasks below are SAMPLE TASKS for illustration purposes only.

  The /speckit.tasks command MUST replace these with actual tasks based on:
  - User stories from spec.md (with their priorities P1, P2, P3...)
  - Feature requirements from plan.md
  - Entities from data-model.md
  - Endpoints from contracts/

  Tasks MUST be organized by user story so each story can be:
  - Implemented independently
  - Tested independently
  - Delivered as an MVP increment

  DO NOT keep these sample tasks in the generated tasks.md file.
  ============================================================================
-->

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Project initialization and basic structure

- [ ] T001 Create project structure per implementation plan
- [ ] T002 Initialize [language] project with [framework] dependencies
- [ ] T003 [P] Configure linting and formatting tools

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core infrastructure that MUST be complete before ANY user story can be implemented

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

Examples of foundational tasks (adjust based on your project):

- [ ] T004 Identify sensitive data/storage/network impacts from spec.md and plan.md
- [ ] T005 [P] Update shared Tauri command/event/type contracts needed by all stories
- [ ] T006 [P] Add or update settings defaults, migrations, and normalization shared by all stories
- [ ] T007 Create shared pipeline/state-machine guard or background-task plumbing
- [ ] T008 Configure deterministic fake inputs for tests without real providers, API keys, audio devices, or sleeps
- [ ] T009 Update docs/refactor notes for cross-cutting behavior or known deferred work

**Checkpoint**: Foundation ready - user story implementation can now begin in parallel

---

## Phase 3: User Story 1 - [Title] (Priority: P1) 🎯 MVP

**Goal**: [Brief description of what this story delivers]

**Independent Test**: [How to verify this story works on its own]

### Tests for User Story 1 (REQUIRED for behavior changes) ⚠️

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**

- [ ] T010 [P] [US1] Deterministic UI/unit test for [behavior] in app/src/[path]/[name].test.ts
- [ ] T011 [P] [US1] Rust unit test for [backend behavior] in app/src-tauri/src/[path].rs

### Implementation for User Story 1

- [ ] T012 [P] [US1] Implement UI state or component changes in app/src/[path]/[file].tsx
- [ ] T013 [P] [US1] Implement TypeScript wrapper/type changes in app/src/lib/tauri/[file].ts
- [ ] T014 [US1] Implement Rust command/event/pipeline behavior in app/src-tauri/src/[path].rs
- [ ] T015 [US1] Regenerate/check schemas or generated Tauri event/type files if contracts changed
- [ ] T016 [US1] Add validation, error handling, and redacted logging
- [ ] T017 [US1] Update relevant user/dev docs for visible behavior or setting changes

**Checkpoint**: At this point, User Story 1 should be fully functional and testable independently

---

## Phase 4: User Story 2 - [Title] (Priority: P2)

**Goal**: [Brief description of what this story delivers]

**Independent Test**: [How to verify this story works on its own]

### Tests for User Story 2 (REQUIRED for behavior changes) ⚠️

- [ ] T018 [P] [US2] Deterministic UI/unit test for [behavior] in app/src/[path]/[name].test.ts
- [ ] T019 [P] [US2] Rust unit test for [backend behavior] in app/src-tauri/src/[path].rs

### Implementation for User Story 2

- [ ] T020 [P] [US2] Implement UI/settings changes in app/src/[path]/[file].tsx
- [ ] T021 [US2] Implement TypeScript orchestration or wrapper changes in app/src/lib/[path].ts
- [ ] T022 [US2] Implement Rust backend behavior in app/src-tauri/src/[path].rs
- [ ] T023 [US2] Integrate with User Story 1 components (if needed)

**Checkpoint**: At this point, User Stories 1 AND 2 should both work independently

---

## Phase 5: User Story 3 - [Title] (Priority: P3)

**Goal**: [Brief description of what this story delivers]

**Independent Test**: [How to verify this story works on its own]

### Tests for User Story 3 (REQUIRED for behavior changes) ⚠️

- [ ] T024 [P] [US3] Deterministic UI/unit test for [behavior] in app/src/[path]/[name].test.ts
- [ ] T025 [P] [US3] Rust unit test for [backend behavior] in app/src-tauri/src/[path].rs

### Implementation for User Story 3

- [ ] T026 [P] [US3] Implement UI changes in app/src/[path]/[file].tsx
- [ ] T027 [US3] Implement TypeScript orchestration or wrapper changes in app/src/lib/[path].ts
- [ ] T028 [US3] Implement Rust backend behavior in app/src-tauri/src/[path].rs

**Checkpoint**: All user stories should now be independently functional

---

[Add more user story phases as needed, following the same pattern]

---

## Phase N: Polish & Cross-Cutting Concerns

**Purpose**: Improvements that affect multiple user stories

- [ ] TXXX [P] Documentation updates in docs/
- [ ] TXXX Record out-of-scope refactors in docs/Refactors/ instead of drive-by refactoring
- [ ] TXXX Performance optimization across all stories
- [ ] TXXX [P] Additional deterministic tests for edge cases in app/src/** or app/src-tauri/src/**
- [ ] TXXX Security/privacy hardening and log redaction review
- [ ] TXXX Run formatting before the smallest relevant validation command set
- [ ] TXXX Run quickstart.md validation

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies - can start immediately
- **Foundational (Phase 2)**: Depends on Setup completion - BLOCKS all user stories
- **User Stories (Phase 3+)**: All depend on Foundational phase completion
  - User stories can then proceed in parallel (if staffed)
  - Or sequentially in priority order (P1 → P2 → P3)
- **Polish (Final Phase)**: Depends on all desired user stories being complete

### User Story Dependencies

- **User Story 1 (P1)**: Can start after Foundational (Phase 2) - No dependencies on other stories
- **User Story 2 (P2)**: Can start after Foundational (Phase 2) - May integrate with US1 but should be independently testable
- **User Story 3 (P3)**: Can start after Foundational (Phase 2) - May integrate with US1/US2 but should be independently testable

### Within Each User Story

- Tests for behavior changes MUST be written and FAIL before implementation
- Shared types/contracts before wrappers and call sites
- Settings migrations/defaults before UI that relies on them
- Rust state transitions before UI assumes new backend state/event behavior
- Core implementation before integration and documentation updates
- Story complete before moving to next priority

### Parallel Opportunities

- All Setup tasks marked [P] can run in parallel
- All Foundational tasks marked [P] can run in parallel (within Phase 2)
- Once Foundational phase completes, all user stories can start in parallel (if team capacity allows)
- All tests for a user story marked [P] can run in parallel
- Independent UI, Rust, docs, and test tasks within a story marked [P] can run in parallel
- Different user stories can be worked on in parallel by different team members

---

## Parallel Example: User Story 1

```bash
# Launch all deterministic tests for User Story 1 together:
Task: "UI/unit test for [behavior] in app/src/[path]/[name].test.ts"
Task: "Rust unit test for [backend behavior] in app/src-tauri/src/[path].rs"

# Launch independent implementation work for User Story 1 together:
Task: "Implement UI state/component changes in app/src/[path]/[file].tsx"
Task: "Implement Rust command/event behavior in app/src-tauri/src/[path].rs"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup
2. Complete Phase 2: Foundational (CRITICAL - blocks all stories)
3. Complete Phase 3: User Story 1
4. **STOP and VALIDATE**: Test User Story 1 independently
5. Deploy/demo if ready

### Incremental Delivery

1. Complete Setup + Foundational → Foundation ready
2. Add User Story 1 → Test independently → Deploy/Demo (MVP!)
3. Add User Story 2 → Test independently → Deploy/Demo
4. Add User Story 3 → Test independently → Deploy/Demo
5. Each story adds value without breaking previous stories

### Parallel Team Strategy

With multiple developers:

1. Team completes Setup + Foundational together
2. Once Foundational is done:
   - Developer A: User Story 1
   - Developer B: User Story 2
   - Developer C: User Story 3
3. Stories complete and integrate independently

---

## Notes

- [P] tasks = different files, no dependencies
- [Story] label maps task to specific user story for traceability
- Each user story should be independently completable and testable
- Verify tests fail before implementing
- Commit after each task or logical group
- Stop at any checkpoint to validate story independently
- Avoid: vague tasks, same file conflicts, cross-story dependencies that break independence
