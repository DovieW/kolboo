---
description: "Task list for Windows UIA-first context + insertion reliability"
---

# Tasks: Windows Context + Insertion Reliability (UIA-first)

**Input**: Design documents from `specs/001-uia-text-insertion/`

- Required: `specs/001-uia-text-insertion/spec.md`, `specs/001-uia-text-insertion/plan.md`
- Supporting: `specs/001-uia-text-insertion/research.md`, `specs/001-uia-text-insertion/data-model.md`, `specs/001-uia-text-insertion/contracts/tauri-openapi.yaml`, `specs/001-uia-text-insertion/quickstart.md`

**Tests** (recommended where they lock behavior fast):

- Must be deterministic.
- Must not make real network calls.
- Must not require API keys.

**Path conventions**:

- UI (TypeScript/React): `app/src/**`
- Backend (Rust/Tauri): `app/src-tauri/src/**`

---

## Phase 1: Setup (Shared Infrastructure)

- [x] T001 Add `specs/001-uia-text-insertion/tasks.md` from template structure (this file)
- [x] T002 [P] Confirm Windows UIA dependencies are available and documented in `app/src-tauri/Cargo.toml`
- [x] T003 [P] Add a dedicated module namespace for UIA work in `app/src-tauri/src/windows_uia/mod.rs`

---

## Phase 2: Foundational (Blocking Prerequisites)

> These tasks are required before any story work, because they define the core primitives (snapshot/context/plan) and settings/event plumbing used by all stories.

- [x] T004 Define core data types (`WindowsTextTargetSnapshot`, `WindowsTextContext`, `WindowsInsertPlan`) in `app/src-tauri/src/windows_uia/types.rs`

- [x] T005 Implement COM+MTA initialization helper for UIA worker calls in `app/src-tauri/src/windows_uia/com.rs`
- [x] T006 Implement UIA client bootstrap (`IUIAutomation` creation + focus acquisition + retry) in `app/src-tauri/src/windows_uia/client.rs`
- [x] T007 Implement snapshot capture (isPassword/isEnabled/isReadOnly + supported patterns) in `app/src-tauri/src/windows_uia/snapshot.rs`
- [x] T008 Implement bounded context capture helpers (selection + surrounding excerpt) in `app/src-tauri/src/windows_uia/context.rs`

- [x] T009 Implement persisted per-app capability memory storage + load/save helpers in `app/src-tauri/src/windows_uia/capability_memory.rs`
- [x] T010 Add a stable “app identity key” helper (exe path normalization) in `app/src-tauri/src/windows_uia/app_identity.rs`

- [x] T011 Remove the clipboard-fallback setting (clipboard context is separate, explicit opt-in) in `app/src-tauri/src/settings.rs`
- [x] T012 Remove settings seeding/migration for the clipboard-fallback setting in `app/src-tauri/src/settings/defaults.rs`
- [x] T013 [P] Remove TS types + settings wiring for clipboard-fallback in `app/src/lib/tauri/types.ts` and `app/src/lib/tauri/settings.ts`

- [x] T014 Add a backend event constant for safe fallback toasts (e.g. transcript copied) in `app/src-tauri/src/events.rs`
- [x] T015 [P] Add TS event typing/wiring for the new backend event in `app/src/lib/tauri/events.ts`

**Checkpoint**: Foundation ready (snapshot/context/plan primitives + settings + event wiring exist)

---

## Phase 3: User Story 1 - Insert transcript into the right field (Priority: P1) 🎯 MVP

**Goal**: When dictation completes, insert the transcript into the correct focused editable field reliably; if focus changes or no safe target exists, abort to safe fallback.

**Independent Test** (manual): Focus Notepad, dictate a short phrase, verify it inserts. Change focus mid-flow, verify no wrong-target insertion (safe fallback is used).

### Tests (recommended)

- [x] T016 [P] [US1] Add Rust unit tests for insertion-plan selection (ValuePattern → paste → typing → none) in `app/src-tauri/src/windows_uia/insert_plan.rs`
- [x] T017 [P] [US1] Add Rust unit tests for focus mismatch abort logic in `app/src-tauri/src/windows_uia/target_match.rs`

### Implementation

- [x] T018 [US1] Implement “target match” (snapshot-to-current comparison) in `app/src-tauri/src/windows_uia/target_match.rs`
- [x] T019 [US1] Implement insertion ladder executor (ValuePattern set → paste → typing) in `app/src-tauri/src/windows_uia/insert.rs`
- [x] T020 [US1] Update `type_text` to use UIA-first insertion on Windows (and keep macOS main-thread rule) in `app/src-tauri/src/commands/text.rs`
- [x] T021 [US1] Integrate snapshot capture + re-check near insertion time in the main dictation stop path in `app/src-tauri/src/lib.rs`
- [x] T022 [US1] Record insertion outcomes into capability memory (per app) in `app/src-tauri/src/windows_uia/capability_memory.rs`

- [x] T023 [US1] Implement safe fallback behavior: copy transcript to clipboard + emit toast event in `app/src-tauri/src/text/inject.rs` and `app/src-tauri/src/lib.rs`
- [x] T024 [US1] Show a clear toast/notification when safe fallback triggers in `app/src/App.tsx` (or the existing top-level notifications wiring under `app/src/**`)

- [x] T041 [P] [US1] Add Rust unit tests for “non-content verification” rules (error/timeout/target mismatch/clipboard restore) in `app/src-tauri/src/windows_uia/verify.rs`
- [x] T042 [US1] Implement non-content verification helpers and a single `verify_or_fallback(...)` decision point in `app/src-tauri/src/windows_uia/verify.rs`
- [x] T043 [US1] Wire verification into the insertion ladder so failed verification triggers the next fallback method (and ultimately safe fallback) in `app/src-tauri/src/windows_uia/insert.rs`

**Checkpoint**: US1 works end-to-end in Notepad without random failure; focus-change aborts to safe fallback.

---

## Phase 4: User Story 2 - Use on-screen context without disrupting clipboard (Priority: P2)

**Goal**: Capture selection/surrounding context from the focused field using UIA when available; do not touch clipboard unless the user enabled the explicit setting.

**Independent Test** (manual): Select text in Notepad, run a context-using flow (Quick Ask / Quick Replace), confirm selected text is captured and clipboard is unchanged.

### Tests (recommended)

- [x] T025 [P] [US2] Add Rust unit tests for bounded context truncation and “no clipboard touched” guarantee in `app/src-tauri/src/windows_uia/context.rs`
- [x] T026 [P] [US2] Remove clipboard fallback selection probe (UIA-only highlighted text capture) in `app/src-tauri/src/sessions/selection_probe.rs`

### Implementation

- [x] T027 [US2] Add a UIA-first selection probe path that tries TextPattern first in `app/src-tauri/src/sessions/selection_probe.rs`
- [x] T028 [US2] Remove clipboard-based fallback probing for highlighted text on Windows (keep UIA-only) in `app/src-tauri/src/sessions/selection_probe.rs`
- [x] T029 [US2] Wire captured context into Quick Ask prompt building (selected text / surrounding excerpt) in `app/src-tauri/src/clipboard_context.rs` and `app/src-tauri/src/lib.rs`
- [x] T030 [US2] Wire captured context into Quick Replace prompt building (selected text / surrounding excerpt) in `app/src-tauri/src/lib.rs`

- [x] T031 [US2] Add UI setting toggle (default off) to Settings UI in `app/src/components/**` (where other settings live)
- [x] T032 [US2] Ensure settings changes persist + normalize + propagate correctly in `app/src/lib/tauri/settings.ts`

**Checkpoint**: US2 context capture works in Notepad without clipboard disruption; clipboard probing only occurs when explicitly enabled.

---

## Phase 5: User Story 3 - Stay safe around sensitive or non-editable fields (Priority: P3)

**Goal**: Never read or insert into password fields / secure inputs, and avoid disabled/read-only controls; always fall back safely.

**Independent Test** (manual): Focus a password field, dictate, verify: no insertion, transcript copied to clipboard, toast shown.

### Tests (recommended)

- [x] T033 [P] [US3] Add Rust unit tests for password/read-only/disabled safety policy decisions in `app/src-tauri/src/windows_uia/safety.rs`

### Implementation

- [x] T034 [US3] Implement a centralized safety policy (block capture+insert for password; block insert for disabled/read-only) in `app/src-tauri/src/windows_uia/safety.rs`
- [x] T035 [US3] Enforce safety policy in snapshot capture + context capture + insert plan execution in `app/src-tauri/src/windows_uia/snapshot.rs`, `app/src-tauri/src/windows_uia/context.rs`, and `app/src-tauri/src/windows_uia/insert.rs`
- [x] T036 [US3] Ensure all relevant flows use safe fallback consistently (dictation, Quick Replace, Quick Ask) in `app/src-tauri/src/lib.rs`

**Checkpoint**: US3 guarantees hold (no capture/insert in password fields; safe fallback always fires).

---

## Phase 6: Polish & Cross-Cutting Concerns

- [x] T037 [P] Improve structured logging around UIA attempts/fallbacks (no sensitive text) in `app/src-tauri/src/windows_uia/*.rs`
- [x] T038 Document the new setting and manual test steps in `specs/001-uia-text-insertion/quickstart.md`
- [x] T039 Add a brief developer note on known UIA limitations (Chromium/Electron) in `specs/001-uia-text-insertion/research.md`
- [ ] T040 Run the repo CI gate and fix regressions as needed (`pnpm -C app check:ci` from `app/package.json`)

---

## Manual testing (recommended)

- [x] T044 [US1] Verify insertion succeeds in Notepad and focus-change aborts to safe fallback (clipboard + toast) in `specs/001-uia-text-insertion/quickstart.md`
- [ ] T045 [US2] Verify selection/surrounding context capture works without clipboard changes when fallback is OFF in `specs/001-uia-text-insertion/quickstart.md`
- [ ] T046 [US2] Verify clipboard context is only included when explicitly enabled (not as a highlighted-text fallback) in `specs/001-uia-text-insertion/quickstart.md`
- [ ] T047 [US3] Verify password/disabled/read-only fields block capture/insert and trigger safe fallback in `specs/001-uia-text-insertion/quickstart.md`

---

## Dependencies & Execution Order

### Phase Dependencies

- Phase 1 (Setup) → Phase 2 (Foundational) → User stories (US1 → US2 → US3 by priority) → Polish

### User Story Dependencies

- **US1 (P1)** depends on Foundational
- **US2 (P2)** depends on Foundational; benefits from US1’s snapshot/plan work but must be independently testable
- **US3 (P3)** depends on Foundational; can be developed after US1, and should not require US2

### Dependency Graph (story-level)

```text
Setup → Foundational → US1 → US3
                      └────→ US2
```

---

## Parallel execution examples (per story)

### US1

- [P] T016 and T017 can be written in parallel (different files under `app/src-tauri/src/windows_uia/**`).
- [P] T023 (backend safe fallback emit) and T024 (UI toast handling) can be done in parallel.

### US2

- [P] T025 (context truncation tests) can be done in parallel with T028 (settings gate for clipboard probe).
- [P] T031 (UI setting toggle) can be done in parallel with T027 (UIA-first selection probe path).

### US3

- [P] T033 (safety unit tests) can be written in parallel with T034 (safety implementation) if the tests target exported pure helpers.

---

## Implementation strategy

- **MVP scope**: US1 only (Phase 1 + 2 + US1). Validate in Notepad before moving on.
- Then add US2 (context capture) and US3 (hard safety guarantees) as separate, independently testable increments.
