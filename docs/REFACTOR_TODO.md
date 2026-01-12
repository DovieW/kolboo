# Refactor ideas (out of scope)

This file is a parking lot for larger refactors that came up while working on smaller changes.

## Overlay UI (React)

- **Split `app/src/OverlayApp.tsx` into smaller modules.**

  - Suggested extraction targets:
    - `RecordingControl` (top-level controller)
    - `BackendAudioWave` + rendering helpers
    - `AudioWave` (browser analyser fallback)
    - Hover gating logic (mouse tracking + suppress-on-show)

- **Extract the overlay UI reducer into a dedicated hook.**

  - Move the reducer + action types into something like `app/src/lib/useOverlayUiReducer.ts`.
  - Add a short transition table comment that explains how the UI should behave when:
    - hotkey fires before `pipeline-state-changed`
    - polling returns a stale state
    - recording-only mode hides right after going idle

- **Consider a single “overlay controller” state object.**
  - Right now some state lives in refs, some in `useState`, some in the reducer.
  - A follow-up could consolidate more of this into one predictable state machine, but that’s a larger change.
