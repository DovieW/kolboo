# Phase 1 Design — Data model

**Feature**: `001-fix-overlay-visibility`  
**Date**: 2026-01-26

This feature is primarily behavioral. The “data model” is minimal and mostly reuses existing state.

## Entities

### Overlay visibility state

Represents what the system *intends* the overlay to be doing.

- `intended_visibility`: `shown | hidden`
- `mode`: `always | never | recording_only` (already exists as a setting)
- `epoch`: monotonically increasing counter used to invalidate stale show/hide actions (already exists as `overlay_visibility_epoch`)

### Overlay placement state

Represents where the overlay should appear.

- `widget_position`: one of `top-left | top-center | top-right | center | bottom-left | bottom-center | bottom-right` (already exists as a setting)
- `overlay_monitor_target`: one of `main | cursor | active_window` (already exists as a setting)

### Display environment (observed)

Used only for best-effort recovery and diagnostics.

- `available_monitors`: list of monitors with bounds + scale
- `current_monitor`: the monitor Tauri reports the window belongs to

## Validation rules

- If an overlay placement is outside the bounds of the chosen monitor, clamp it into view.
- If window size is reported near-zero while hidden, use a conservative fallback size for placement math.
- Recovery operations must be idempotent and safe to repeat.

## State transitions

- On recording start:
  - intended_visibility → `shown`
  - epoch increments
  - request show + recovery steps

- On recording stop/cancel:
  - intended_visibility → `hidden`
  - (frontend may animate out; backend may perform fallback hide)

