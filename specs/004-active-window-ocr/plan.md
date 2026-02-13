# Implementation Plan: Active Window OCR Context

**Branch**: `004-active-window-ocr` | **Date**: 2026-01-28 | **Spec**: `specs/004-active-window-ocr/spec.md`
**Input**: Feature specification from `specs/004-active-window-ocr/spec.md`

**Note**: This file is generated/maintained via the Speckit workflow.

## Summary

Add an **opt-in, per-tool** “Active Window OCR Context” feature for **Rewrite**, **Quick Replace**, and **Quick Ask**.

When enabled for a tool:

- Kolboo captures the currently active window (Windows-first) and sends the image to an **OpenAI-compatible** OCR service.
- The returned text is injected into the downstream LLM prompt labeled as:
	- `OCR context from the currently active window:`

Key UX requirements:

- Per-tool tri-state mode: `off | auto | manual`
- Manual mode triggers OCR only when the overlay OCR button is clicked.
- Failures are non-blocking and show a calm “OCR context unavailable” message.

**Robustness correction (important):** OCR must be owned by an explicit per-request session context (Option A). OCR work must not silently disappear due to pipeline state transitions like “reset to idle”. This change should apply across **all flows** (Rewrite / Quick Replace / Quick Ask) and be extensible to future OCR uses.

## Technical Context

**Language/Version**: TypeScript (React/Vite, strict) + Rust (Tauri v2)

**Primary Dependencies**:

- UI: React, TanStack Query, Mantine, Vitest, Biome
- Backend: Tauri v2, Tokio async runtime, Reqwest (HTTP), Serde (JSON)

**Storage**:

- Settings: Tauri store (`settings.json`) + migrations/normalization
- Secrets: OS secure storage (e.g., keyring) for OCR API key
- OCR imagery: **ephemeral only** (never persisted)

**Testing**:

- UI tests: `pnpm -C app test`
- Rust tests: `pnpm -C app cargo:test`
- CI gate: `pnpm -C app check:ci`

**Target Platform**: Windows desktop (primary)

**Project Type**: Desktop app (Tauri) with React UI

**Performance Goals**:

- OCR should be best-effort and overlap with recording/STT where possible.
- OCR should never make the tool feel “stuck” without clear logging.
- OCR waiting must be bounded by `ocr_request_timeout_ms` (per settings), but only after ensuring the OCR job actually exists and is owned by the active session.

**Constraints**:

- Privacy: do not retain screenshots; do not log base64 image bytes; avoid logging raw OCR text.
- Secrets hygiene: never log OCR keys/headers.
- Deterministic tests: no real OCR server calls.

**Scale/Scope**:

- Single-user desktop app; small concurrency, but multiple overlapping flows over time.

### Robustness design: session-owned OCR jobs (Option A)

**Problem observed:** OCR state was tied to the pipeline’s state machine and could be cleared during `reset_to_idle()`, leading to “OCR started” followed by “status=not_started” and immediate continuation without waiting.

**Design decision:** introduce a per-request `SessionContext` with a `session_id`, and move OCR task/result ownership into that session context.

High-level rules:

- A session owns the OCR job(s) for that user action.
- Pipeline state transitions (Idle/Recording/Transcribing/…) must not implicitly destroy OCR jobs for the active session.
- Only explicit session end/cancel (Escape, superseded by a new session, force reset) can cancel OCR.
- Flows (Rewrite/Quick Replace/Quick Ask) should consume OCR from the current session. They may start OCR automatically (`auto`) or only after the overlay trigger (`manual`).

Extensibility:

- Model OCR work as jobs keyed by purpose (e.g., `active_window_context`) so future OCR features can add jobs without stepping on existing ones.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- [x] Deterministic tests: no real network calls in tests; no real API keys required by default
- [x] UI↔backend contract: any command/event/type changes are updated in BOTH Rust and TypeScript
- [x] Settings discipline: any settings additions/changes include migrations/normalization and apply immediately at runtime
- [x] Secrets hygiene: no logging of secrets; redact sensitive data in logs
- [x] Tooling gate: keep `pnpm -C app check:ci` green (format → lint → tests)

## Project Structure

### Documentation (this feature)

```text
specs/[###-feature]/
├── plan.md              # This file (/speckit.plan command output)
├── research.md          # Phase 0 output (/speckit.plan command)
├── data-model.md        # Phase 1 output (/speckit.plan command)
├── quickstart.md        # Phase 1 output (/speckit.plan command)
├── contracts/           # Phase 1 output (/speckit.plan command)
└── tasks.md             # Phase 2 output (/speckit.tasks command - NOT created by /speckit.plan)
```

### Source Code (repository root)
<!--
  ACTION REQUIRED: Replace the placeholder tree below with the concrete layout
  for this feature. Delete unused options and expand the chosen structure with
  real paths (e.g., apps/admin, packages/something). The delivered plan must
  not include Option labels.
-->

```text
app/
├── src/                # React/TypeScript UI
├── src-tauri/src/      # Rust/Tauri backend
└── tests/              # (if present) test helpers, fixtures, etc.

docs/
scripts/
```

**Structure Decision**: Use the existing split:

- Rust pipeline + commands/events in `app/src-tauri/src/**`
- UI settings + overlay UX in `app/src/**`

Session-owned OCR will primarily touch `app/src-tauri/src/pipeline.rs` (session context + OCR jobs) and the flow entrypoints that consume OCR.

## Phase outputs

- Phase 0: `research.md` updated to include the session-owned OCR design decision.
- Phase 1: `data-model.md` updated with `SessionContext` + `OcrJob` entities.
- Phase 1: `contracts/` expanded to include a Rust↔TS OCR contract note (session id + job status).
- Phase 1: `quickstart.md` updated and encoding glitches fixed.

## Risks & mitigations

- **Risk: session/OCR lifetime confusion** (e.g., new recording supersedes old OCR)
  - Mitigation: explicit `session_id` in logs and overlay state; explicit cancellation reasons.

- **Risk: regressions across flows** (Rewrite/Quick Replace/Quick Ask)
  - Mitigation: cross-flow unit tests using mocked OCR results + deterministic failure/cancel scenarios.

- **Risk: leaking sensitive text/images**
  - Mitigation: keep OCR text ephemeral by default; log only presence/char count; never log image bytes.

## Complexity Tracking

> **Fill ONLY if Constitution Check has violations that must be justified**

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| [e.g., 4th project] | [current need] | [why 3 projects insufficient] |
| [e.g., Repository pattern] | [specific problem] | [why direct DB access insufficient] |
