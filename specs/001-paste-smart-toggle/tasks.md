# Tasks: Paste Safety Toggle

**Input**: Design documents from `/specs/001-paste-smart-toggle/`
**Prerequisites**: plan.md (required), spec.md, research.md, data-model.md, contracts/

**Tests**: Include tests when they are the fastest, most reliable way to lock in behavior.

- Tests MUST be deterministic.
- Tests MUST NOT make real network calls.
- Tests MUST NOT require real API keys by default.

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- Include exact file paths in descriptions

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Project initialization and basic structure

- [x] T001 [P] Review plan scope and artifacts in `specs/001-paste-smart-toggle/plan.md`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core infrastructure that MUST be complete before ANY user story can be implemented

- [x] T002 Add default setting seed for `output_smart_paste_protection` in `app/src-tauri/src/settings/defaults.rs`
- [x] T003 [P] Add `output_smart_paste_protection` to `AppSettings` in `app/src/lib/tauri/types.ts`
- [x] T004 [P] Normalize/load `output_smart_paste_protection` in `app/src/lib/tauri/settings.ts`
- [x] T005 Add a settings update helper for `output_smart_paste_protection` in `app/src/lib/tauri/settings.ts`

**Checkpoint**: Foundation ready - user story implementation can now begin

---

## Phase 3: User Story 1 - Control smart paste protection (Priority: P1) 🎯 MVP

**Goal**: Users can toggle smart paste protection in the UI tab and the behavior applies immediately.

**Independent Test**: Toggle the setting in the UI tab and confirm paste behavior changes for subsequent output attempts.

### Implementation for User Story 1

- [x] T006 [P] [US1] Add UI toggle for smart paste protection in `app/src/components/settings/UiSettings.tsx`
- [x] T007 [P] [US1] Wire UI toggle to settings mutation in `app/src/lib/queries.ts` and `app/src/lib/tauri/settings.ts`
- [x] T008 [US1] Apply setting at runtime by gating safety checks in `app/src-tauri/src/lib.rs` and `app/src-tauri/src/windows_uia/insert.rs`

**Checkpoint**: User Story 1 is fully functional and testable independently

---

## Phase 4: User Story 2 - Understand what the setting does (Priority: P2)

**Goal**: Users see a clear explanation of what the smart paste protection setting does.

**Independent Test**: Open UI tab and verify the description text is present and accurate.

### Implementation for User Story 2

- [x] T009 [US2] Add short explanatory description in `app/src/components/settings/UiSettings.tsx`

**Checkpoint**: User Story 2 is complete and independently verifiable

---

## Phase 5: User Story 3 - Safe fallback when save fails (Priority: P3)

**Goal**: Users are informed if the setting fails to save and the previous value remains.

**Independent Test**: Simulate a settings save failure and confirm a clear error message is shown.

### Implementation for User Story 3

- [x] T010 [US3] Add save-failure notification for this toggle in `app/src/lib/queries.ts` (and any local UI handling in `app/src/components/settings/UiSettings.tsx` if needed)

**Checkpoint**: User Story 3 is complete and independently verifiable

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Improvements that affect multiple user stories

- [x] T011 [P] Validate and update `specs/001-paste-smart-toggle/quickstart.md` if implementation details changed

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies - can start immediately
- **Foundational (Phase 2)**: Depends on Setup completion - BLOCKS all user stories
- **User Stories (Phase 3+)**: All depend on Foundational completion
- **Polish (Final Phase)**: Depends on all desired user stories being complete

### User Story Dependencies

- **User Story 1 (P1)**: Can start after Foundational (Phase 2)
- **User Story 2 (P2)**: Can start after Foundational (Phase 2)
- **User Story 3 (P3)**: Can start after Foundational (Phase 2)

### Parallel Opportunities

- T003 and T004 can run in parallel (different files)
- T006 and T007 can run in parallel (different files)

---

## Parallel Example: User Story 1

- T006 [US1] Add UI toggle in `app/src/components/settings/UiSettings.tsx`
- T007 [US1] Wire mutations in `app/src/lib/queries.ts` and `app/src/lib/tauri/settings.ts`

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup
2. Complete Phase 2: Foundational
3. Complete Phase 3: User Story 1
4. Validate behavior manually (toggle + paste output)

### Incremental Delivery

1. Add User Story 2 (description)
2. Add User Story 3 (save failure messaging)
3. Run full check (`pnpm -C app check:ci`) before merge
