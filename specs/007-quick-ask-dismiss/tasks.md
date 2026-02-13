---
description: "Task list for Quick Ask dismiss options"
---

# Tasks: Quick Ask dismiss options

**Input**: Design documents from `/specs/007-quick-ask-dismiss/`
**Prerequisites**: plan.md (required), spec.md (required for user stories), research.md, data-model.md, contracts/

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

- [x] T001 [P] Add shared dismiss mode labels/options in `app/src/lib/quickAskDismissMode.ts`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core infrastructure that MUST be complete before ANY user story can be implemented

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

- [x] T002 Add `QuickAskDismissMode` type export in `app/src/lib/tauri/types.ts`
- [x] T003 Add settings normalization + defaults for `quick_ask_dismiss_mode` in `app/src/lib/tauri/settings.ts`
- [x] T004 Add Rust default seeding/migration for `quick_ask_dismiss_mode` in `app/src-tauri/src/lib.rs`
- [x] T005 [P] Update settings key contract list for `quick_ask_dismiss_mode` in `app/src/lib/contracts/settingsKeysContract.test.ts`

**Checkpoint**: Foundation ready - user story implementation can now begin in parallel

---

## Phase 3: User Story 1 - Choose dismiss behavior per profile (Priority: P1) 🎯 MVP

**Goal**: Let users set a per-profile Quick Ask dismiss mode (Manual default, Auto option) and apply it in overlay behavior.

**Independent Test**: Change the profile’s dismiss mode and verify click-away behavior on the Quick Ask overlay.

### Implementation for User Story 1

- [x] T006 [US1] Add dismiss mode dropdown (default override + per-profile) in `app/src/components/settings/prompt/QuickAskPanel.tsx`
- [x] T007 [US1] Persist profile overrides and defaults in `app/src/components/settings/prompt/usePromptSettingsProfileState.ts`
- [x] T008 [US1] Apply dismiss mode to click-away behavior in `app/src/QuickAskApp.tsx`
- [x] T009 [US1] Ensure settings changes refresh overlays (emit settings-changed) in `app/src/lib/tauri/commands.ts`

**Checkpoint**: User Story 1 fully functional and testable independently

---

## Phase 4: User Story 2 - Close from the overlay itself (Priority: P2)

**Goal**: Add an inline X close button that dismisses the Quick Ask overlay without changing its height.

**Independent Test**: Open Quick Ask, click the X button, confirm overlay closes and height remains unchanged.

### Implementation for User Story 2

- [x] T010 [US2] Add inline X close button aligned with the question row in `app/src/QuickAskApp.tsx`
- [x] T011 [US2] Add styles to keep height stable and right-align the close control in `app/src/app.css`

**Checkpoint**: User Stories 1 and 2 both work independently

---

## Phase 5: Polish & Cross-Cutting Concerns

**Purpose**: Improvements that affect multiple user stories

- [x] T012 [P] Validate `specs/007-quick-ask-dismiss/quickstart.md` steps and update if needed

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies - can start immediately
- **Foundational (Phase 2)**: Depends on Setup completion - BLOCKS all user stories
- **User Stories (Phase 3+)**: All depend on Foundational phase completion
  - User stories can then proceed in parallel (if staffed)
  - Or sequentially in priority order (P1 → P2)
- **Polish (Final Phase)**: Depends on all desired user stories being complete

### User Story Dependencies

- **User Story 1 (P1)**: Can start after Foundational (Phase 2) - No dependencies on other stories
- **User Story 2 (P2)**: Can start after Foundational (Phase 2) - Depends on shared dismiss mode types/settings but otherwise independent

### Within Each User Story

- Settings/types before UI wiring
- Overlay behavior before styling polish
- Story complete before moving to next priority

### Parallel Opportunities

- T001 and T005 can run in parallel
- T002–T004 are sequential (type → normalization → Rust defaults)
- T006 and T007 can run in parallel (different files) once T002–T004 are complete
- T010 and T011 can run in parallel once T008 is complete

---

## Parallel Example: User Story 1

```bash
Task: "Add dismiss mode dropdown in app/src/components/settings/prompt/QuickAskPanel.tsx"
Task: "Persist profile overrides in app/src/components/settings/prompt/usePromptSettingsProfileState.ts"
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

### Parallel Team Strategy

With multiple developers:

1. Team completes Setup + Foundational together
2. Once Foundational is done:
   - Developer A: User Story 1
   - Developer B: User Story 2
3. Stories complete and integrate independently

---

## Notes

- [P] tasks = different files, no dependencies
- [Story] label maps task to specific user story for traceability
- Each user story should be independently completable and testable
- Avoid vague tasks or tasks that touch the same file in parallel
