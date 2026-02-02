# Tasks: Hotkey Shortcut Cards

**Input**: Design documents from `/specs/001-hotkey-shortcut-cards/`
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

- [x] T001 Review existing hotkey flows and settings docs in `docs/How Tos/HOTKEYS_AND_SHORTCUTS.md`
- [x] T002 Create UI scaffolding for shortcut cards in `app/src/components/settings/HotkeyShortcutCard.tsx`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core infrastructure that MUST be complete before ANY user story can be implemented

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

- [x] T003 Define shortcut card settings shape + normalization in `app/src/lib/tauri/settings.ts`
- [x] T004 [P] Add shared types/helpers for shortcut cards in `app/src/lib/hotkeys.ts`
- [x] T005 Update settings migrations/defaults for new shortcut card storage in `app/src-tauri/src/settings/defaults.rs`
- [x] T006 Update backend settings structs for shortcut cards in `app/src-tauri/src/settings.rs`
- [x] T007 Ensure shortcut registration reads multiple cards in `app/src-tauri/src/shortcuts/mod.rs`
- [x] T008 Add store update helpers for shortcut cards in `app/src/lib/tauri.ts`

**Checkpoint**: Foundation ready - user story implementation can now begin in parallel

---

## Phase 3: User Story 1 - Scan and manage existing shortcuts (Priority: P1) 🎯 MVP

**Goal**: Show only configured shortcuts as cards with a clean, readable layout and empty state.

**Independent Test**: Open Settings → Hotkeys and verify only configured shortcuts appear as cards (or empty state is shown).

### Implementation for User Story 1

- [x] T009 [P] [US1] Render shortcut cards list in `app/src/components/settings/HotkeySettings.tsx`
- [x] T010 [P] [US1] Add empty state UI in `app/src/components/settings/HotkeySettings.tsx`
- [x] T011 [P] [US1] Style shortcut cards in `app/src/app.css`

**Checkpoint**: User Story 1 is fully functional and independently testable

---

## Phase 4: User Story 2 - Add and edit a shortcut card (Priority: P2)

**Goal**: Let users add shortcut cards from a dropdown, then set, unset, or delete them.

**Independent Test**: Add a shortcut card, set a key binding, unset it, and delete the card.

### Implementation for User Story 2

- [x] T012 [P] [US2] Add shortcut type dropdown + Add button in `app/src/components/settings/HotkeySettings.tsx`
- [x] T013 [P] [US2] Wire card-level set/unset/delete actions in `app/src/components/settings/HotkeyShortcutCard.tsx`
- [x] T014 [P] [US2] Implement shortcut card CRUD mutations in `app/src/lib/queries.ts`
- [x] T015 [P] [US2] Implement shortcut card update helpers in `app/src/lib/hotkeyMutations.ts`
- [x] T016 [P] [US2] Add backend commands for shortcut cards in `app/src-tauri/src/commands/settings.rs`
- [x] T017 [US2] Sync runtime shortcuts after card updates in `app/src-tauri/src/commands/settings.rs`

**Checkpoint**: User Story 2 is fully functional and independently testable

---

## Phase 5: User Story 3 - Multiple shortcuts for one action (Priority: P3)

**Goal**: Allow multiple cards for the same shortcut type and prevent key binding conflicts.

**Independent Test**: Add the same shortcut type twice, set different bindings, and see both saved.

### Implementation for User Story 3

- [x] T018 [P] [US3] Allow duplicate types in dropdown logic in `app/src/components/settings/HotkeySettings.tsx`
- [x] T019 [P] [US3] Enforce unique key bindings across cards in `app/src/lib/hotkeyMutations.ts`
- [x] T020 [P] [US3] Enforce unique key bindings in backend validation in `app/src-tauri/src/commands/settings.rs`
- [x] T021 [US3] Update shortcut registration to handle duplicate types in `app/src-tauri/src/shortcuts/mod.rs`

**Checkpoint**: All user stories are independently functional

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Improvements that affect multiple user stories

- [x] T022 [P] Update hotkey documentation for the new UI in `docs/How Tos/HOTKEYS_AND_SHORTCUTS.md`
- [x] T023 [P] Run quickstart validation steps in `specs/001-hotkey-shortcut-cards/quickstart.md`

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

- **User Story 1 (P1)**: Can start after Foundational (Phase 2) - no dependencies on other stories
- **User Story 2 (P2)**: Can start after Foundational (Phase 2) - may integrate with US1 but should be independently testable
- **User Story 3 (P3)**: Can start after Foundational (Phase 2) - may integrate with US1/US2 but should be independently testable

### Within Each User Story

- Models before services
- Services before endpoints
- Core implementation before integration
- Story complete before moving to next priority

### Parallel Opportunities

- All Setup tasks marked [P] can run in parallel
- All Foundational tasks marked [P] can run in parallel (within Phase 2)
- Once Foundational phase completes, all user stories can start in parallel (if team capacity allows)
- Different user stories can be worked on in parallel by different team members

---

## Parallel Example: User Story 2

```bash
# Launch UI + backend work in parallel when it touches different files:
Task: "Add shortcut type dropdown + Add button in app/src/components/settings/HotkeySettings.tsx"
Task: "Add backend commands for shortcut cards in app/src-tauri/src/commands/settings.rs"
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
- Commit after each task or logical group
- Stop at any checkpoint to validate story independently
- Avoid: vague tasks, same file conflicts, cross-story dependencies that break independence
