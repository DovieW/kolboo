# Low-urgency refactors

<!-- Add low-urgency refactor ideas here. Keep each item specific and code-grounded. -->

## Revisit the OCR Session file split only if OCR work reopens it

**Status:** Identified as low urgency by the post-deepening review (2026-05-08)

Files:

- `app/src-tauri/src/pipeline/ocr_session.rs`
- `app/src-tauri/src/pipeline/ocr_session_state.rs`

Current state:

- OCR Session ownership is in the right place overall, and the current Interface has real leverage for request-owned OCR lifecycle behavior.
- The split between orchestration and state may now be slightly too fine-grained: callers/maintainers often need both files open to understand one OCR Session change.

Follow-up idea:

- If the next OCR feature touches both files heavily, re-run the deletion test and consider collapsing them into one Module.
- Do **not** reopen this as a speculative refactor now; the current split is working and does not block feature work.

## Revisit recording command lifecycle packaging only when feature work exposes real friction

**Status:** Identified as low urgency by the post-deepening review (2026-05-08)

Files:

- `app/src-tauri/src/commands/recording_lifecycle.rs`
- `app/src-tauri/src/recording_request_initialization.rs`
- `app/src-tauri/src/recording_orchestration.rs`
- `app/src-tauri/src/recording_completion.rs`

Current state:

- The recording command flow has much better locality than the old monolith, and each Module has a visible ownership line.
- The remaining pain point is cognitive: understanding one end-to-end recording command still requires bouncing across several Modules.

Follow-up idea:

- Only revisit this if a future recording feature repeatedly changes sequencing across all four files.
- If that happens, look for a deeper command-facing Module that improves leverage without hiding pipeline state-machine ownership or reintroducing a generic “recording manager.”

