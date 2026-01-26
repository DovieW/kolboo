# Tasks: Fix overlay visibility after wake

**Input**: Design documents from `specs/001-fix-overlay-visibility/`  
**Prerequisites**: `plan.md` (required), `spec.md` (required), plus `research.md`, `data-model.md`, `contracts/`, `quickstart.md`

**Tests**: Include tests when they are the fastest, most reliable way to lock in behavior.

- Tests MUST be deterministic.
- Tests MUST NOT make real network calls.
- Tests MUST NOT require real API keys by default.

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- Include exact file paths in descriptions

## Path Conventions

- **Kolboo UI (TypeScript)**: `app/src/**`
- **Kolboo backend (Rust/Tauri)**: `app/src-tauri/src/**`
- If a task touches both UI and backend, call out both paths explicitly

## Phase 1: Setup (Shared Infrastructure)

- [x] T001 Confirm overlay-related settings defaults already exist (no new settings needed) in `app/src-tauri/src/settings/defaults.rs` and are normalized/typesafe in `app/src/lib/tauri/settings.ts`
- [x] T002 [P] Confirm overlay event contract is in sync (Rust event constant + JSON schema + TS typing + contract test) in `app/src-tauri/src/events.rs`, `app/src-tauri/gen/schemas/overlay-hide-requested.schema.json`, `app/src/lib/tauri/events.ts`, `app/src/lib/settingsContract.test.ts`
- [x] T003 [P] Inventory overlay hide/show paths and stale-timer risks in `app/src/overlay/RecordingControl.tsx` and `app/src/overlay/useOverlayHideRequested.ts`, then update notes in `specs/001-fix-overlay-visibility/research.md`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Checkpoint**: Foundation ready — overlay show path has recovery primitives we can build on.

- [x] T004 Add a Windows-only "raise without focus" recovery helper (best-effort z-order bump using `SetWindowPos(... SWP_NOACTIVATE ...)`) in `app/src-tauri/src/commands/overlay.rs`
- [x] T005 Add a delayed "verify + retry recovery" pass after show (guarded by `overlay_visibility_epoch`) in `app/src-tauri/src/commands/overlay.rs`
- [x] T006 Expand backend overlay diagnostics to include recovery steps taken + retry fired + available monitors summary in `app/src-tauri/src/commands/overlay.rs`

---

## Phase 3: User Story 1 — Overlay always appears during recording (Priority: P1) 🎯 MVP

**Goal**: When recording starts, the overlay reliably becomes visible and stays visible until recording stops/cancels.

**Independent Test**: Start a recording and verify overlay appears within 1 second and does not steal focus; then stop/cancel and verify it hides.

- [ ] T007 [P] [US1] Make overlay frontend ignore/cancel any pending animated-hide when the pipeline is active (so a stale hide can’t hide a fresh show) in `app/src/overlay/RecordingControl.tsx`
- [ ] T008 [P] [US1] Extend the pure hide-gating helper to support the new "don’t hide while active" rule in `app/src/overlay/overlayHideGate.ts`
- [ ] T009 [P] [US1] Update/add deterministic unit tests for the gating behavior (including "active pipeline blocks hide") in `app/src/overlay/overlayHideGate.test.ts`
- [x] T007 [P] [US1] Make overlay frontend ignore/cancel any pending animated-hide when the pipeline is active (so a stale hide can’t hide a fresh show) in `app/src/overlay/RecordingControl.tsx`
- [x] T008 [P] [US1] Extend the pure hide-gating helper to support the new "don’t hide while active" rule in `app/src/overlay/overlayHideGate.ts`
- [x] T009 [P] [US1] Update/add deterministic unit tests for the gating behavior (including "active pipeline blocks hide") in `app/src/overlay/overlayHideGate.test.ts`
- [ ] T010 [US1] Ensure recording start path triggers overlay show (recording-only mode) and bumps visibility epoch as needed in `app/src-tauri/src/core/recording.rs` and `app/src-tauri/src/commands/overlay.rs`
- [x] T010 [US1] Ensure recording start path triggers overlay show (recording-only mode) and bumps visibility epoch as needed in `app/src-tauri/src/core/recording.rs` and `app/src-tauri/src/commands/overlay.rs`
- [x] T011 [US1] Decide if an explicit "overlay-show-requested" event is needed to prevent missed transitions (not needed after frontend gating + backend retry)
- [ ] T012 [US1] Validate MVP manually using `specs/001-fix-overlay-visibility/quickstart.md` (sections “Basic show/hide” + focus check) and update `specs/001-fix-overlay-visibility/quickstart.md` if any steps/logs changed
- [x] T012 [US1] Validate MVP manually using `specs/001-fix-overlay-visibility/quickstart.md` (sections “Basic show/hide” + focus check) and update `specs/001-fix-overlay-visibility/quickstart.md` if any steps/logs changed

---

## Phase 4: User Story 2 — Overlay recovers after sleep/wake (Priority: P2)

**Goal**: After sleep/wake or display reconnect, the next recording still produces a visible overlay without requiring an app restart.

**Independent Test**: Put the computer to sleep, wake it, then start a recording and verify the overlay appears.

- [x] T013 [US2] Tighten the backend’s “suspicious state” detection used by the post-show verify/retry (e.g., monitor=<none>, near-zero geometry) in `app/src-tauri/src/commands/overlay.rs`
- [ ] T014 [US2] Validate sleep/wake scenario using `specs/001-fix-overlay-visibility/quickstart.md` (section “Sleep/wake recovery”) and record any tweaks back into `specs/001-fix-overlay-visibility/quickstart.md`

---

## Phase 5: User Story 3 — Overlay stays on-screen across monitor/DPI changes (Priority: P3)

**Goal**: Monitor layout / DPI changes do not cause the overlay to appear off-screen.

**Independent Test**: Change monitor layout or DPI scaling, then start a recording and verify the overlay appears within visible monitor bounds.

- [x] T015 [US3] Add a "window is off-screen" sanity check and recovery (re-center + snap) for show/retry paths (avoid changing normal resize behavior) in `app/src-tauri/src/commands/overlay.rs`
- [ ] T016 [US3] Validate monitor/DPI change scenarios using `specs/001-fix-overlay-visibility/quickstart.md` (section “Monitor/DPI change resilience”) and update `specs/001-fix-overlay-visibility/quickstart.md` if needed

---

## Phase 6: Polish & Cross-Cutting Concerns

- [x] T017 Run unit tests and fix any failures in touched files (`app/src/**`) — `pnpm -C app test` (see scripts in `app/package.json`)
- [x] T018 Run the Rust tests (if backend changed) and fix any failures in touched files (`app/src-tauri/src/**`) — `pnpm -C app cargo:test` (see scripts in `app/package.json`)
- [x] T019 Run the CI gate and fix any issues in touched files — `pnpm -C app check:ci` (see scripts in `app/package.json`)
- [ ] T020 Tighten final logging levels (keep logs helpful but not noisy) in `app/src-tauri/src/commands/overlay.rs`
- [ ] T021 Re-run the full manual verification checklist and finalize expected log snippets in `specs/001-fix-overlay-visibility/quickstart.md`

---

## Dependencies & Execution Order

### Phase dependencies

- Phase 1 (Setup) → Phase 2 (Foundational) → Phase 3 (US1) → Phase 4 (US2) → Phase 5 (US3) → Phase 6 (Polish)

### User story dependency graph

- **US1** is the MVP and should be done first.
- **US2** builds on the same recovery mechanisms and manual QA.
- **US3** extends the same placement/recovery logic for monitor/DPI scenarios.

So the intended completion order is:

$$
US1 \;\rightarrow\; US2 \;\rightarrow\; US3
$$

## Parallel execution examples

### US1 parallelizable tasks

- T007, T008, and T009 can run in parallel (they touch different files under `app/src/overlay/**`).
- Backend work (T010) can run in parallel with frontend work (T007–T009).

### US2 parallelizable tasks

- T013 can run in parallel with US1 frontend-only work (it touches `app/src-tauri/src/commands/overlay.rs` while US1 frontend tasks touch `app/src/overlay/**`).

### US3 parallelizable tasks

- T015 should be done sequentially with any other edits to `app/src-tauri/src/commands/overlay.rs` to avoid merge conflicts.

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1 (Setup)
2. Complete Phase 2 (Foundational)
3. Complete Phase 3 (US1)
4. **STOP and VALIDATE** using `specs/001-fix-overlay-visibility/quickstart.md`

### Incremental Delivery

1. Foundation ready → implement US1 → validate
2. Add US2 recovery tuning → validate
3. Add US3 off-screen recovery tuning → validate
