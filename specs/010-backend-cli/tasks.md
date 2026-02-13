---

description: "Task list for backend CLI subcommand"
---

# Tasks: Backend CLI Subcommand

**Input**: Design documents from `/specs/010-backend-cli/`
**Prerequisites**: plan.md (required), spec.md (required for user stories), research.md, data-model.md, contracts/

**Tests**: Not requested for this spec. Add only if implementation uncovers high-risk logic that needs coverage.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- Include exact file paths in descriptions

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Prepare CLI plumbing and config scaffolding

- [x] T001 Add CLI plugin dependency in `app/src-tauri/Cargo.toml`
- [x] T002 Define CLI plugin scaffold in `app/src-tauri/tauri.conf.json`
- [x] T003 [P] Create CLI module skeleton in `app/src-tauri/src/cli/mod.rs` and `app/src-tauri/src/cli/types.rs`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core CLI routing and shared utilities required by all stories

- [x] T004 Implement CLI match routing in `app/src-tauri/src/lib.rs`
- [x] T005 [P] Implement standard CLI output + exit code helpers in `app/src-tauri/src/cli/output.rs`
- [x] T006 [P] Define full CLI subcommands/args in `app/src-tauri/tauri.conf.json`
- [x] T007 [P] Add shared CLI error mapping in `app/src-tauri/src/cli/errors.rs`

**Checkpoint**: CLI routing and shared helpers ready

---

## Phase 3: User Story 1 - Run the pipeline headlessly (Priority: P1) 🎯 MVP

**Goal**: Start a headless pipeline run and fetch status from the CLI

**Independent Test**: Run `kolboo pipeline run` and `kolboo pipeline status` and verify structured output + exit codes

### Implementation for User Story 1

- [x] T008 [P] [US1] Implement pipeline run handler in `app/src-tauri/src/cli/pipeline.rs`
- [x] T009 [P] [US1] Implement pipeline status handler in `app/src-tauri/src/cli/pipeline.rs`
- [x] T010 [US1] Wire pipeline handlers into CLI router in `app/src-tauri/src/lib.rs`
- [x] T011 [US1] Validate pipeline CLI output format in `app/src-tauri/src/cli/output.rs`

**Checkpoint**: Pipeline run/status work headlessly with JSON output and proper exit codes

---

## Phase 4: User Story 2 - Manage settings and profiles (Priority: P2)

**Goal**: Read/update settings and list/select profiles from the CLI

**Independent Test**: Run `kolboo settings get/set` and `kolboo profiles list/use` and verify persistence and output

### Implementation for User Story 2

- [x] T012 [P] [US2] Implement settings get/set handlers in `app/src-tauri/src/cli/settings.rs`
- [x] T013 [P] [US2] Implement profile list/use handlers in `app/src-tauri/src/cli/profiles.rs`
- [x] T014 [US2] Wire settings/profile handlers into CLI router in `app/src-tauri/src/lib.rs`
- [x] T015 [US2] Ensure settings changes trigger sync + events in `app/src-tauri/src/lib.rs` (use existing settings helpers)

**Checkpoint**: Settings/profile commands persist and reflect current state

---

## Phase 5: User Story 3 - Inspect diagnostics and export configuration (Priority: P3)

**Goal**: Provide diagnostics and config export from the CLI

**Independent Test**: Run `kolboo diagnostics` and `kolboo config export` and verify structured output

### Implementation for User Story 3

- [x] T016 [P] [US3] Implement diagnostics handler in `app/src-tauri/src/cli/diagnostics.rs`
- [x] T017 [P] [US3] Implement config export handler in `app/src-tauri/src/cli/config_export.rs`
- [x] T018 [US3] Wire diagnostics/export handlers into CLI router in `app/src-tauri/src/lib.rs`

**Checkpoint**: Diagnostics and config export return structured, useful data

---

## Phase 6: Polish & Cross-Cutting Concerns

- [ ] T019 [P] Update CLI documentation in `README.md` or `docs/` (add CLI usage/flags)
- [ ] T020 Run `quickstart.md` validation steps and update `specs/010-backend-cli/quickstart.md` if needed

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies - can start immediately
- **Foundational (Phase 2)**: Depends on Setup completion - BLOCKS all user stories
- **User Stories (Phase 3+)**: All depend on Foundational phase completion
- **Polish (Phase 6)**: Depends on desired user stories being complete

### User Story Dependencies

- **User Story 1 (P1)**: Can start after Foundational - no dependencies on other stories
- **User Story 2 (P2)**: Can start after Foundational - can be done independently
- **User Story 3 (P3)**: Can start after Foundational - can be done independently

### Parallel Opportunities

- Phase 1 tasks marked [P] can run in parallel
- Phase 2 tasks marked [P] can run in parallel
- Each story’s [P] tasks can run in parallel if staffed

---

## Parallel Example: User Story 1

```bash
Task: "Implement pipeline run handler in app/src-tauri/src/cli/pipeline.rs"
Task: "Implement pipeline status handler in app/src-tauri/src/cli/pipeline.rs"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup
2. Complete Phase 2: Foundational
3. Complete Phase 3: User Story 1
4. Validate CLI run/status outputs

### Incremental Delivery

1. Setup + Foundational
2. User Story 1 → validate
3. User Story 2 → validate
4. User Story 3 → validate
5. Polish
