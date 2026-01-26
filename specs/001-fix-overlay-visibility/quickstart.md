# Quickstart (QA) — Fix overlay visibility after wake

**Feature**: `001-fix-overlay-visibility`

This is a hands-on checklist you (or future-you) can run to prove the bug is fixed.

## Prerequisites

- Windows machine (this bug appears to be Windows-focused)
- Kolboo running in dev mode (`pnpm dev`)
- Overlay mode set to **recording only**

## Repro checklist (current bug)

1. Start Kolboo dev.
2. Start a recording (your usual hotkey).
3. Confirm the backend logs show something like:
   - `[overlay] show requested ...`
   - `[overlay] final geom ...`
   - `[overlay] show complete (visible_after=Some(true))`
4. If the overlay is not visible even though those logs appear, you reproduced the bug.

## Verification checklist (after fix)

Run these in order; each is a pass/fail.

### 1) Basic show/hide

- Start recording → overlay appears within 1 second.
- Stop/cancel recording → overlay hides.
- Start recording again → overlay appears again.

### 2) Sleep/wake recovery

- Put the computer to sleep.
- Wake it.
- Start recording → overlay appears.

### 3) Always-on-top / z-order resilience

- Put another always-on-top window in front (Task Manager has an option for this).
- Start recording → overlay still appears (and is not hidden behind the other window).

### 4) Monitor/DPI change resilience

- Dock/undock or change display scaling.
- Start recording → overlay appears on-screen (not off to the side).

## Expected logs

- You should see logs indicating any recovery steps taken (raise without focus / retry verification), so we can confirm which path fired if it fails again.

