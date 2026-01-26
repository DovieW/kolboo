# Phase 0 Research — Fix overlay visibility after wake

**Feature**: `001-fix-overlay-visibility`  
**Date**: 2026-01-26  
**Goal**: Explain *why* the overlay can be “logically visible” but not actually seen, and choose recovery strategies that fit Kolboo’s current architecture.

## Findings (current behavior)

From `app/src-tauri/src/commands/overlay.rs`, the overlay “show” path already performs several best-effort recovery actions:

- `window.show()`
- (recording-only) `window.set_size(224x56 logical)` to avoid a tiny window that “looks missing”
- `window.unminimize()`
- `window.center()`
- `window.set_always_on_top(true)`
- (non-"always") snap to saved widget position with clamping on a selected monitor
- logs final geometry + monitor + `is_visible()`

This explains your logs: “show complete (visible_after=Some(true))” is reporting the native window state, not “human-perceivable”.

From the overlay UI (`app/src/overlay/RecordingControl.tsx` + `app/src/overlay/useOverlayHideRequested.ts`):

- The backend emits `overlay-hide-requested` and the overlay listens via `useOverlayHideRequested`, which calls `requestAnimatedHide()`.
- `requestAnimatedHide()` uses `applyAnimatedHideGate()` (cooldown default 350ms) to prevent repeated exit requests, then:
  - sets `animState` → `"exit"`
  - starts an exit timer (210ms) that calls `invoke("hide_overlay")`
  - resets `animState` to `"enter"` and clears any held phase text on completion.
- The show animation is driven by a pipeline-state transition effect:
  - It only calls `requestAnimatedShow()` on *idle → active* transitions (arming/recording/etc.) to avoid flicker from polling.
  - `requestAnimatedShow()` cancels any pending exit timer, sets `animState` to `"enter"`, then flips to `"visible"` on the next frame.
- In `recording_only` mode, the UI keeps the widget expanded whenever visible and relies on the backend to show/hide the native window.
- Pipeline state comes from a mix of polling (`useOverlayPipelineStatePolling`) and events (`useOverlayPipelineEvents`), so missed transitions can leave stale hide timers armed.

## Most likely failure modes

### 1) Z-order/topmost quirks on Windows (topmost ≠ raised)

**Observation**: A window can be topmost and visible but still appear behind another topmost window (or not be raised within the topmost stack). Re-applying “always-on-top” does not always bump z-order.

**Practical implication**: We need an explicit “raise without focus” strategy on Windows.

### 2) Overlay UI not mounted / webview not rendering yet

**Observation**: The overlay window is transparent. If the webview content is not rendered (slow startup, crash, JS error, stuck state), the window can be “visible” while effectively invisible.

**Practical implication**: Ensure the overlay frontend receives a “show now” signal reliably (even if it missed the first pipeline-state change), and log when the frontend is ready.

### 3) Race: stale hide timers hiding immediately after show

**Observation**: The overlay frontend schedules a delayed hide after receiving `overlay-hide-requested`. If the overlay misses the subsequent “recording started” state transition, it may not cancel that timer and can hide itself shortly after a new show.

**Practical implication**: Make showing resilient to missed transitions (backend re-emit, or frontend logic that forces “enter visible” when pipeline is active).

### 4) Geometry/monitor mismatch after sleep/wake

**Observation**: Kolboo already clamps and logs geometry, which reduces off-screen issues, but resume/hotplug can still temporarily produce inconsistent monitor/window information.

**Practical implication**: Add a short delayed “verify + retry recovery” pass guarded by an epoch so it’s safe.

## Decisions

### Decision A — Add a Windows-specific “raise without focus” step (recommended)

**Chosen**: On Windows, after calling `show()`, do a best-effort z-order bump that does **not** steal focus.

**Rationale**: Matches the requirement that overlay must not steal focus (FR-004) while addressing a common Windows behavior: topmost status does not guarantee the window is on top of the topmost band.

**Implementation direction (non-binding)**:

- Prefer Tauri APIs first (`set_always_on_top` toggle).
- If needed for robustness, use Win32 `SetWindowPos(..., SWP_NOACTIVATE | SWP_SHOWWINDOW | ...)` via the existing `windows` crate usage in this file.

**Alternatives considered**:

- Calling `set_focus()` — rejected (violates FR-004 and is annoying UX).

### Decision B — Add a “verify + retry recovery” pass after show (recommended)

**Chosen**: After a show request, schedule a short delayed check (e.g., ~50–150ms) to re-run recovery if the window appears to be in a suspicious state (e.g., minimized, zero-ish geometry, no monitor).

**Rationale**: Sleep/wake and display transitions can cause transient window/monitor state inconsistencies.

**Alternatives considered**:

- Doing nothing (status quo) — rejected (bug persists).

### Decision C — Make frontend show/hide logic resilient to missed transitions (recommended)

**Chosen**: Ensure the overlay frontend will enter the “visible” animation state whenever the pipeline is active, even if it missed an idle→active transition.

**Rationale**: Prevents the “stale hide timer” race.

**Alternatives considered**:

- Relying purely on backend epoch cancellation — rejected (frontend timers are independent).

### Decision D — Prefer diagnostics that prove what happened

**Chosen**: Expand diagnostic info so we can answer: did we try to raise z-order? did we retry? did we think the frontend was mounted?

**Rationale**: Your current logs already capture geometry and `is_visible()`; we’ll extend them to cover the new recovery steps.

## Test strategy (deterministic)

- **Unit tests (frontend)**: test any pure “hide/show gating” logic (no timers; no real windows).
- **Unit tests (backend)**: test pure helper functions only (monitor target parsing, point-in-monitor). Avoid GUI/monitor-dependent tests.
- **Manual test checklist (Windows)**:
  1. Start recording: overlay appears immediately.
  2. Stop recording: overlay hides.
  3. Sleep → wake → start recording: overlay appears.
  4. Put another always-on-top window in front, start recording: overlay still appears.
  5. Multi-monitor / DPI change: overlay appears on-screen.
