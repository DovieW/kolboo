# Contracts — Tauri commands/events used by overlay

**Feature**: `005-fix-overlay-visibility`  
**Date**: 2026-01-26

Kolboo’s overlay behavior is driven by a mix of backend commands and backend→frontend events.

This feature aims to improve reliability without changing the public surface area if possible. If we add/adjust any event payloads, we must update both Rust and TypeScript (constitution: UI↔backend contract stays in sync).

## Existing backend commands (Rust → invoked by frontend)

These already exist in `app/src-tauri/src/commands/overlay.rs`:

- `show_overlay()`
- `hide_overlay()`
- `resize_overlay(width, height)`
- `set_overlay_mode(mode)`
- `set_widget_position(position)`

## Existing events (Rust → frontend)

- `overlay-hide-requested`
  - Payload: none (unit)
  - Purpose: let frontend animate out before backend forces a hide

Other overlay-related events exist (e.g., audio level), but are not central to this bug.

## Proposed changes (if needed)

### Option 1 (preferred): no new commands/events

- Improve backend recovery behavior and frontend gating without changing command/event shapes.

### Option 2: add a small “overlay-show-requested” event

If we find we need an explicit synchronization signal to prevent missed transitions:

- Event name: `overlay-show-requested`
- Payload:
  - `epoch: number` (matches backend visibility epoch)
  - `reason: "recording-start" | "recovery" | "settings"` (optional)

If implemented:

- Add event constant in `app/src-tauri/src/events.rs`
- Emit in backend in the overlay show path
- Listen in overlay UI and use it to cancel any stale hide timers
- Add/update schema tests for events

