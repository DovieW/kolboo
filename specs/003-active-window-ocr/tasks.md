# Tasks: Active Window OCR Context

**Input**: Design documents from `specs/003-active-window-ocr/`

- `spec.md`
- `plan.md`
- `research.md`
- `data-model.md`
- `contracts/ocr-service.openapi.yaml`
- `contracts/ocr-internal.contract.md`
- `quickstart.md`

**Tests**: Keep tests deterministic.

- Tests MUST be deterministic.
- Tests MUST NOT make real network calls.
- Tests MUST NOT require real API keys by default.

**Note about this tasks file**: This branch has already implemented substantial portions of US1–US3. The checklist below is written as an executable, end-to-end task plan (including the robustness upgrade). Use the request logs + `pnpm -C app dev` logs to validate each checkpoint.

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Add the minimal code + dependency scaffolding needed by all stories.

- [x] T001 Add OCR backend deps in `app/src-tauri/Cargo.toml` (Win capture + image resize + PNG encode + cancellation + base64 + `wiremock`)
- [x] T002 [P] Create OCR module skeleton in `app/src-tauri/src/ocr/mod.rs` and `app/src-tauri/src/ocr/openai_compatible.rs`
- [x] T003 [P] Create active-window capture module skeleton in `app/src-tauri/src/active_window_capture.rs`
- [x] T004 Register new Rust modules in `app/src-tauri/src/lib.rs` (mod declarations / exports)

---

## Phase 2: Foundational (Blocking prerequisites + robustness)

**Purpose**: Settings + provider configuration + session-owned OCR job ownership (Option A). This phase blocks all user stories.

- [x] T005 Add OCR settings defaults + legacy migration in `app/src-tauri/src/settings/defaults.rs` (`ensure_default_settings(...)`)
- [x] T006 [P] Add OCR settings fields + types in `app/src/lib/tauri/types.ts`
- [x] T007 [P] Normalize OCR settings + migrate legacy bools in `app/src/lib/tauri/settings.ts`
- [x] T008 [P] Add deterministic settings migration tests in `app/src/lib/tauri/settings.legacy.test.ts`
- [x] T009 Add backend OCR provider availability gating in `app/src-tauri/src/commands/ocr.rs` (invalid URL / missing key => unavailable)
- [x] T010 [P] Add OCR provider UI section in `app/src/components/settings/ApiKeysSettings.tsx` (base URL, model, auth mode, API key)
- [x] T011 [P] Wire OCR provider settings mutations in `app/src/lib/queries.ts` (persist + store secret key)
- [x] T012 Ensure OCR API key is stored via secrets mechanism (use secret key `ocr_api_key`)

### Robustness upgrade (Option A): session-owned OCR jobs

- [x] T013 Introduce `SessionContext` + `session_id` in `app/src-tauri/src/pipeline.rs` (created/ended per tool invocation)
- [x] T014 Move OCR state into session-owned `OcrJob` keyed by purpose (`active_window_context`) in `app/src-tauri/src/pipeline.rs`
- [x] T015 Ensure pipeline state transitions (e.g., `reset_to_idle()`) do not implicitly destroy OCR jobs for the active session in `app/src-tauri/src/pipeline.rs`
- [x] T016 Add explicit cancel reasons (user_cancel, superseded_by_new_session, provider_unavailable, etc.) in `app/src-tauri/src/pipeline.rs`
- [x] T017 [P] Update overlay polling payload to include `session_id` + stable OCR job fields in `app/src-tauri/src/commands/ocr.rs` and `app/src/lib/tauri/commands.ts` (follow `specs/003-active-window-ocr/contracts/ocr-internal.contract.md`)
- [x] T018 [P] Update manual trigger command to optionally include `session_id` for safety in `app/src-tauri/src/commands/ocr.rs` and `app/src/lib/tauri/commands.ts` (ignore mismatches)
- [x] T019 [P] Add request-log breadcrumbs that include `session_id` in `app/src-tauri/src/request_log.rs` + call sites

**Checkpoint**: Foundation ready — OCR provider settings are editable end-to-end and OCR job lifetime is session-owned (no silent disappearance).

---

## Phase 3: User Story 1 — Use OCR context in a supported tool (Priority: P1) 🎯 MVP

**Goal**: When OCR mode is `auto`, capture active window text via OCR and inject it into the tool prompt labeled as OCR context.

**Independent Test**: Enable OCR only for Quick Ask (`quick_ask_active_window_ocr_mode="auto"`, others `off`), focus a window with readable text, run Quick Ask, verify the assistant request includes the OCR labeled section.

- [x] T020 [P] [US1] Implement Windows active-window capture to PNG in `app/src-tauri/src/active_window_capture.rs` (`GetForegroundWindow` + `PrintWindow` + `BitBlt` fallback; downscale)
- [x] T021 [P] [US1] Implement OCR HTTP client in `app/src-tauri/src/ocr/openai_compatible.rs` (OpenAI-compatible chat completions; optional bearer auth; no base64/auth logging)
- [x] T022 [P] [US1] Add OCR text truncation + prompt label helper in `app/src-tauri/src/ocr/mod.rs` (cap via `ocr_context_max_chars`)
- [x] T023 [P] [US1] Add deterministic `wiremock` tests for OCR request/response parsing in `app/src-tauri/src/ocr/openai_compatible.rs`
- [x] T024 [US1] Start OCR early (auto mode) at tool start for the active session in `app/src-tauri/src/pipeline.rs`
- [x] T025 [US1] Consume OCR (bounded wait) and inject labeled OCR context into Rewrite prompt in `app/src-tauri/src/pipeline/transcription_flow.rs`
- [x] T026 [US1] Consume OCR (bounded wait) and inject labeled OCR context into Quick Ask prompt in `app/src-tauri/src/lib.rs`
- [x] T027 [US1] Consume OCR (bounded wait) and inject labeled OCR context into Quick Replace prompt in `app/src-tauri/src/lib.rs`
- [x] T028 [US1] Record OCR presence/char-count metadata (not raw text) in request logs in `app/src-tauri/src/request_log.rs` + call sites

**Checkpoint**: Auto OCR works for Quick Ask / Rewrite / Quick Replace when provider is configured; failures do not block.

---

## Phase 4: User Story 2 — Control OCR context per tool (Priority: P2)

**Goal**: Per-tool tri-state (`off|auto|manual`) and manual trigger via overlay button.

**Independent Test**: Set Quick Ask OCR mode to Manual; record without pressing OCR (no OCR). Press OCR (starts). Turn mode to Off while running (cancels).

- [x] T029 [P] [US2] Add tri-state OCR mode control for Rewrite in `app/src/components/settings/prompt/RewriteSettingsSection.tsx`
- [x] T030 [P] [US2] Add tri-state OCR mode control for Quick Ask in `app/src/components/settings/prompt/QuickAskPanel.tsx`
- [x] T031 [P] [US2] Add tri-state OCR mode control for Quick Replace in `app/src/components/settings/QuickReplaceSettings.tsx`
- [x] T032 [P] [US2] Persist per-tool OCR modes + emit settings-changed + sync pipeline config in `app/src/lib/queries.ts` + `app/src/lib/tauri/settings.ts`
- [x] T033 [US2] Ensure per-tool OCR mode supports per-profile override/inheritance in `app/src/lib/tauri/settings.ts` + Rust profile mapping (if applicable)
- [x] T034 [P] [US2] Add backend command to manually trigger OCR for the active session in `app/src-tauri/src/commands/ocr.rs`
- [x] T035 [P] [US2] Add TS invoke wrapper for manual OCR trigger in `app/src/lib/tauri/commands.ts`
- [x] T036 [US2] Render OCR button next to waveform in overlay in `app/src/overlay/RecordingControl.tsx` (manual mode only; show running/disabled state)
- [x] T037 [US2] Cancel in-flight OCR when mode flips to `off` in `app/src-tauri/src/pipeline.rs`

**Checkpoint**: Manual mode works end-to-end; OCR never runs unless clicked in Manual.

---

## Phase 5: User Story 3 — Safe failure behavior (Priority: P3)

**Goal**: OCR failures never block tool execution; user sees a calm "OCR context unavailable" message.

**Independent Test**: Configure invalid OCR URL, enable Quick Ask OCR Auto, run Quick Ask, verify tool completes and overlay shows the unavailable message.

- [x] T038 [P] [US3] Emit user-friendly OCR failure event payload in `app/src-tauri/src/events.rs` + `app/src-tauri/src/event_payloads.rs` (no technical stack traces)
- [x] T039 [P] [US3] Add UI event typing/handling for OCR failure in `app/src/lib/tauri/events.ts` + `app/src/lib/tauri/types.ts`
- [x] T040 [US3] Display "OCR context unavailable" in overlay UI in `app/src/overlay/RecordingControl.tsx` (non-blocking)
- [x] T041 [P] [US3] Add deterministic Rust tests for failure sanitization + non-blocking behavior in `app/src-tauri/src/ocr/mod.rs`

**Checkpoint**: Failure UX is clear and non-blocking.

---

## Phase 6: Tests & hardening (cross-flow)

**Purpose**: Lock in the robustness guarantees across all flows.

- [x] T042 [P] Add cross-flow Rust tests ensuring OCR remains consumable after internal state transitions within a session in `app/src-tauri/src/pipeline/tests.rs`
- [x] T043 [P] Add tests ensuring session cancellation cancels OCR and produces a stable cancellation reason in `app/src-tauri/src/pipeline/tests.rs`
- [x] T044 [P] Add tests ensuring starting a new session supersedes/cancels old OCR without cross-contamination in `app/src-tauri/src/pipeline/tests.rs`

---

## Phase 7: Polish & cross-cutting concerns

- [x] T045 [P] Update quickstart steps + debug tips in `specs/003-active-window-ocr/quickstart.md`
- [x] T046 [P] Validate OCR provider HTTP contract assumptions in `specs/003-active-window-ocr/contracts/ocr-service.openapi.yaml`
- [x] T047 [P] Confirm internal contract matches implementations in `specs/003-active-window-ocr/contracts/ocr-internal.contract.md`
- [x] T048 Run formatter/lint (`pnpm -C app lint`)
- [x] T049 Run CI gate (`pnpm -C app check:ci`)

---

## Dependencies & execution order

### User story completion order

- US1 → US2 → US3 (per spec priority)

### Practical build order (recommended)

1. Phase 1–2 (foundation + **session-owned OCR jobs**) — this prevents silent OCR disappearance.
2. US1 injection paths
3. US2 manual trigger + per-tool control
4. US3 failure UX
5. Cross-flow tests + CI gate

### Parallel opportunities

- `app/src-tauri/src/active_window_capture.rs` (capture) and `app/src-tauri/src/ocr/openai_compatible.rs` (HTTP) can be implemented in parallel.
- UI per-tool setting panels can be done in parallel:
  - `app/src/components/settings/prompt/RewriteSettingsSection.tsx`
  - `app/src/components/settings/prompt/QuickAskPanel.tsx`
  - `app/src/components/settings/QuickReplaceSettings.tsx`
- Capture module vs HTTP client can be built in parallel:
  - `app/src-tauri/src/active_window_capture.rs`

  - `app/src-tauri/src/ocr/openai_compatible.rs`
