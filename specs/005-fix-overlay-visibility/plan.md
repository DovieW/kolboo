# Implementation Plan: Fix overlay visibility after wake

**Branch**: `005-fix-overlay-visibility` | **Date**: 2026-01-26 | **Spec**: [`spec.md`](./spec.md)  
**Input**: Feature specification from `specs/005-fix-overlay-visibility/spec.md`

## Summary

The actively-recording overlay sometimes fails to appear after sleep/wake even though backend logs indicate the overlay window is visible. We will make overlay showing more reliable by:

- adding a Windows-focused raise without focus recovery step,
- adding a short delayed verify + retry recovery pass guarded by the existing epoch, and
- making the overlay frontends show/hide logic resilient to missed state transitions so stale hide timers cannot immediately hide a freshly shown overlay.

Diagnostics will be expanded to make it obvious which recovery path executed.

Phase 0/1 artifacts:

- Phase 0: [`research.md`](./research.md)
- Phase 1: [`data-model.md`](./data-model.md), [`contracts/tauri-surface.md`](./contracts/tauri-surface.md), [`quickstart.md`](./quickstart.md)

## Technical Context

**Language/Version**: TypeScript (strict) + Rust (Tauri desktop app)  
**Primary Dependencies**: React/Vite (UI), Tauri (backend), `tauri-plugin-store` (settings), `tokio` (async), Windows APIs via `windows` crate (already used)  
**Storage**: Tauri store (`settings.json`) for overlay settings (mode, widget position, monitor target)  
**Testing**: Vitest (`pnpm -C app test`), Rust tests (`pnpm -C app cargo:test`), CI gate (`pnpm -C app check:ci`)  
**Target Platform**: Windows desktop (primary). Fix must not regress other desktop platforms.  
**Project Type**: Desktop app (Tauri) with separate overlay window(s)  
**Performance Goals**: Overlay becomes visible within 1 second of recording start (SC-002)  
**Constraints**: MUST NOT steal focus while showing/recovering overlay (FR-004)  
**Scale/Scope**: Small, localized change across overlay backend + overlay UI logic

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- [x] Deterministic tests: no real network calls in tests; no real API keys required by default
- [x] UIbackend contract: if we add/rename any command/event/type, update BOTH Rust and TypeScript
- [x] Settings discipline: no new settings planned; any setting change would include migrations/normalization + immediate runtime apply
- [x] Secrets hygiene: no logging of secrets; diagnostics are window/geometry only
- [x] Tooling gate: changes will keep `pnpm -C app check:ci` green

## Project Structure

### Documentation (this feature)

```text
specs/005-fix-overlay-visibility/
 plan.md
 research.md
 data-model.md
 quickstart.md
 contracts/
     tauri-surface.md
```

### Source Code (repository root)

```text
app/
 src/                       # React/TypeScript UI
    overlay/               # Overlay UI (webview)
 src-tauri/src/             # Rust/Tauri backend
     commands/overlay.rs    # Overlay show/hide/positioning commands
     core/recording.rs      # Recording start path (triggers overlay show)
```

**Structure Decision**: This feature is implemented in-place in the existing overlay backend (`app/src-tauri/src/commands/overlay.rs`) and overlay UI (`app/src/overlay/**`). No new modules are required.

## Phase 0  Research (complete)

See [`research.md`](./research.md).

Key outputs used for design:

- The backend already does `show + unminimize + center + always_on_top + snap + log geometry`.
- Likely remaining failure modes: Windows z-order edge cases, transparent webview not rendering yet, and frontend stale hide timer races.
- Win32 provides a raise without activating pattern (`SetWindowPos` with `SWP_NOACTIVATE`).

## Phase 1  Design (complete)

### Data model

See [`data-model.md`](./data-model.md). No new persisted entities required.

### Contracts

See [`contracts/tauri-surface.md`](./contracts/tauri-surface.md).

Default intent: **no new commands/events**. If we need a small sync signal (`overlay-show-requested`), it will be added with schema + TS/Rust updates.

### Quickstart / verification

See [`quickstart.md`](./quickstart.md) for a reproducible Windows QA checklist.

## Phase 2  Implementation planning (what we will build)

### Backend changes (Rust)

1. In `show_overlay_with_reset_if_not_always`:
   - Add a Windows-only raise without focus step.
     - First attempt: toggle `always_on_top` offon.
     - If needed: call Win32 `SetWindowPos` with `SWP_NOACTIVATE` (no keyboard focus stealing).
   - Add a delayed verify + retry recovery pass guarded by `overlay_visibility_epoch`.
   - Expand diagnostics so logs clearly show:
     - which recovery steps ran,
     - whether a retry fired,
     - and final geometry/monitor.

2. Ensure recovery is safe to repeat (idempotent) and does not create duplicate windows.

### Frontend changes (TypeScript overlay)

1. Adjust overlay hide/show logic so that:
   - if the pipeline is active, the overlay enters the visible state even if it missed the idleactive transition,
   - any previously scheduled hide timer is cancelled when the pipeline is active.

2. Keep changes small and testable (prefer pure gating logic that can be unit tested).

### Tests

- UI: add/update Vitest unit tests for any pure gating logic.
- Rust: add unit tests only for pure helpers if we add any new pure helper functions.
- Run CI gate before finishing:
  - `pnpm -C app test`
  - `pnpm -C app cargo:test` (if Rust changes are non-trivial)
  - preferred final gate: `pnpm -C app check:ci`

### Rollout / manual verification

Use [`quickstart.md`](./quickstart.md). In particular, validate sleep/wake and another always-on-top window exists scenarios.

## Complexity Tracking

No constitution violations are required for this plan.