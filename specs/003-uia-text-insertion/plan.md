# Implementation Plan: Windows Context + Insertion Reliability (UIA-first)

**Branch**: `003-uia-text-insertion` | **Date**: 2026-01-25 | **Spec**: `specs/003-uia-text-insertion/spec.md`
**Input**: Feature specification from `specs/003-uia-text-insertion/spec.md`

## Summary

Improve Windows text context capture and insertion reliability across _all_ insertion/context flows (dictation insertion, Quick Replace, Quick Ask, rewrite actions) by:

- Using Windows UI Automation (UIA) to detect the focused element, decide if it’s safe/editable, and (when possible) retrieve selection/surrounding text.
- Performing insertion with a “reliability ladder”: UIA direct set (ValuePattern) → paste (clipboard restore) → simulated typing.
- Persisting per-app “capability memory” locally to learn which insertion path works best for each app.
- Preserving safety/UX guarantees: never read/insert into password fields; if insertion is blocked/unsafe, auto-copy transcript to clipboard and show a clear toast.

The Phase 0 research output is in `specs/003-uia-text-insertion/research.md`.

## Technical Context

**Language/Version**: TypeScript (strict) + React 19 + Vite 7 (UI); Rust 2021 + Tauri 2 (backend)

**Primary Dependencies**: UI: `@tanstack/react-query`, Mantine; Backend: `windows` crate (Win32 bindings), `tokio`, `enigo`, `arboard`, `tauri-plugin-store`

**Storage**: Tauri store (`settings.json`) for settings and locally persisted “App Capability Memory”.

**Testing**: Vitest (`pnpm -C app test`); Rust tests (`pnpm -C app cargo:test`); CI gate `pnpm -C app check:ci`

**Target Platform**: Windows 10/11 only (feature is Windows-specific)

**Project Type**: Desktop app (Tauri). UI lives in `app/src/**`. Backend lives in `app/src-tauri/src/**`.

**Performance Goals**:

- UIA calls MUST run on a non-UI thread to avoid hangs/slowness when interacting with our own UI (and to follow UIA guidance).
- Context retrieval MUST be bounded (max chars) to avoid expensive cross-process calls.
- If UIA calls are slow/unresponsive, the user experience MUST still complete via fallback (paste/typing + safe fallback).

**Constraints**:

- No readback verification of field contents after insertion (per spec).
- Clipboard-based context capture is behind a default-off setting.
- Never read/insert into password/secure fields.
- Diagnostics stay local-only; do not upload captured context.

**Scale/Scope**:

- Scope is limited to Windows text context + insertion reliability behaviors and the settings/state needed to support them.

## Constitution Check

_GATE: Must pass before Phase 0 research. Re-check after Phase 1 design._

- [x] Deterministic tests: no real network calls in tests; no real API keys required by default
- [x] UI↔backend contract: any command/event/type changes are updated in BOTH Rust and TypeScript
- [x] Settings discipline: any settings additions/changes include migrations/normalization and apply immediately at runtime
- [x] Secrets hygiene: no logging of secrets; redact sensitive data in logs
- [x] Tooling gate: plan includes how we’ll keep `pnpm -C app check:ci` green

## Project Structure

### Documentation (this feature)

```text
specs/003-uia-text-insertion/
├── spec.md              # Feature spec
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output
│   └── tauri-openapi.yaml
└── tasks.md             # Phase 2 output (/speckit.tasks command - NOT created by /speckit.plan)
```

### Source Code (repository root)

```text
app/
├── src/                       # React/TypeScript UI
│   └── lib/tauri/**            # invoke wrappers + types + settings normalization
├── src-tauri/src/             # Rust/Tauri backend
│   ├── text/selection_probe.rs # current clipboard-based selection probe
│   ├── clipboard_context.rs    # current clipboard context capture + prompt building
│   └── lib.rs                  # command registration + orchestration
└── src-tauri/Cargo.toml        # Rust deps include `windows`, `enigo`, `arboard`

docs/
specs/
```

**Structure Decision**: Implement Windows UIA logic in the Rust backend (new module under `app/src-tauri/src/windows/**` or `app/src-tauri/src/text/**`), expose minimal Tauri commands/events, and keep UI changes limited to settings + wrapper calls.

## Phase 0: Outline & Research (completed)

Output: `specs/003-uia-text-insertion/research.md`

Key research questions answered:

- How to safely call UIA: COM + MTA threading requirements, and why UIA calls should not be on the UI thread.
- Which properties/patterns matter for safety/editability (IsPassword/IsEnabled, ValuePattern/TextPattern).
- UIA limitations in browsers/Electron and implications for fallback strategy.

## Phase 1: Design & Contracts (completed)

Outputs:

- `specs/003-uia-text-insertion/data-model.md`
- `specs/003-uia-text-insertion/contracts/tauri-openapi.yaml`
- `specs/003-uia-text-insertion/quickstart.md`

## Phase 2: Implementation Planning (ready for /speckit.tasks)

### Backend (Rust / Tauri)

- Add a Windows-only UIA module to:
  - Capture a “Context Snapshot” (focused element + process/window metadata + safety/editability flags).
  - Retrieve selection/surrounding context using TextPattern/ValuePattern when available, bounded by max chars.
  - Produce an “Insertion Plan” for the transcript text, selecting the insertion method ladder.
- Integrate with existing insertion flows so they:
  - Take a snapshot near recording stop, then re-check before insert (avoid wrong-target insert).
  - Abort to safe fallback when unsafe/unknown.
- Persist “App Capability Memory” locally and consult it during insertion planning.

### Frontend (TypeScript / React)

- Clarify UI context settings:
  - Highlighted-text context uses UIA only (no clipboard fallback).
  - Clipboard context is a separate explicit opt-in.
- Ensure settings changes follow existing conventions:
  - Persist to store + normalize/migrate
  - If runtime-behavior-affecting, call pipeline config sync + emit `settings-changed` as needed.

### Tests

- Add deterministic Rust unit tests for:
  - Safety policy decisions (password/read-only/disabled).
  - Insertion method selection logic (ValuePattern vs paste vs typing).
- Add/extend UI unit tests only if UI settings logic changes in thresholded files.

### Validation / CI

- During implementation, keep `pnpm -C app check:ci` green.

## Complexity Tracking

No constitution violations are required for this plan.
