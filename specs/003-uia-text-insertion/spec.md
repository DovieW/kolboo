# Feature Specification: Windows Context + Insertion Reliability

**Feature Branch**: `003-uia-text-insertion`
**Created**: 2026-01-25
**Status**: Draft
**Input**: User description: "Improve Windows text context and insertion reliability (automation-first)."

## Assumptions

- This feature applies to Windows 10/11 only.
- This work applies to all Windows text-context and insertion pathways (dictation insertion, Quick Replace, Quick Ask, rewrite actions).
- The app already produces transcript text and can paste/type text today.
- This feature does not include OCR.
- "Context" means text from the currently focused editable field (selection and/or nearby text) when it can be retrieved safely.

## Dependencies

- The operating system and target app must allow programmatic interaction with focused text fields (some apps may block it).
- Users may need to grant any required accessibility permissions for the app to read focus/selection and insert text.
- A focused editable field is required for automatic insertion; otherwise the safe fallback is used.

## Clarifications

### Session 2026-01-25

- Q: When automatic insertion is blocked/unsafe (no focused editable field, password field, focus changed, etc.), what should the safe fallback user experience be? → A: Auto-copy transcript to clipboard and show a clear notification/toast.
- Q: If UI Automation can’t retrieve on-screen context (selected text / surrounding excerpt), should we fall back to a clipboard-based “copy selection” probe to get context? → A: No. Clipboard is a separate, explicit context source (“Include Clipboard Context”), not a fallback for highlighted text.
- Q: Should the app verify insertion by reading back text from the focused field after inserting (when possible)? → A: No readback verification; rely on method success/failure only.
- Q: Which user-facing flows should be covered by this Windows UIA-first context + insertion reliability work? → A: All text-context and insertion pathways on Windows.
- Q: Should the per-application “App Capability Memory” (preferred insertion fallback behavior) persist across app restarts? → A: Yes, persist locally across restarts.

## User Scenarios & Testing _(mandatory)_

### User Story 1 - Insert transcript into the right field (Priority: P1)

When I finish dictating, I want the transcript to go into the text field I'm actively using, without random failures or ending up in the wrong app.

**Why this priority**: This is the core promise of the product; if insertion is unreliable, everything else feels broken.

**Independent Test**: Can be fully tested by dictating a short phrase with focus in a simple text editor and verifying the field content changes correctly.

**Acceptance Scenarios**:

1. **Given** an editable text field is focused, **When** dictation completes, **Then** the transcript is inserted or replaces the current selection in that field.
2. **Given** no editable text field is focused, **When** dictation completes, **Then** the app does not attempt to insert into an unknown target and instead uses the safe fallback (auto-copy transcript to clipboard + show a clear notification/toast).
3. **Given** focus changes between recording start and insertion time, **When** dictation completes, **Then** insertion is aborted and the transcript is provided via the safe fallback.

---

### User Story 2 - Use on-screen context without disrupting clipboard (Priority: P2)

When I have text selected (or a caret in a larger document), I want the app to use that on-screen context to improve edit-style actions and reduce mistakes, without needing to copy manually.

**Why this priority**: Better context improves output quality and enables reliable "replace selection / edit this" flows.

**Independent Test**: Can be tested by selecting text in a basic editor, triggering an action that uses context, and confirming the captured selection and/or surrounding excerpt is correct.

**Acceptance Scenarios**:

1. **Given** a focused editable field with a text selection, **When** context is captured, **Then** the selected text is retrieved when available and stored in a short-lived snapshot for the current request.
2. **Given** context capture succeeds without using the clipboard, **When** the user checks their clipboard afterwards, **Then** clipboard contents are unchanged.
3. **Given** context is not available from the focused control, **When** context capture runs, **Then** the app continues without context (or uses an explicit fallback only where allowed) rather than failing the entire request.
4. **Given** on-screen context is not available from the focused control, **When** context capture runs, **Then** the app continues without highlighted-text context (clipboard context is separate and only used when explicitly enabled).

---

### User Story 3 - Stay safe around sensitive or non-editable fields (Priority: P3)

As a user, I want the app to avoid reading from or inserting into password fields, disabled controls, or read-only areas.

**Why this priority**: Prevents accidental data leaks and typing into the wrong place incidents.

**Independent Test**: Can be tested by focusing a password field / read-only field and verifying the app refuses to capture/insert while still providing a safe fallback.

**Acceptance Scenarios**:

1. **Given** a password/secure input is focused, **When** dictation completes, **Then** the app does not read on-screen text and does not insert text; it only offers the safe fallback.
2. **Given** a control is disabled or read-only, **When** dictation completes, **Then** the app does not attempt insertion and uses the safe fallback.
3. **Given** insertion fails after trying preferred methods, **When** all fallback methods are exhausted, **Then** the transcript is still preserved for the user (safe fallback) and the app reports a clear error state.

### Edge Cases

- Focus moves to a different window between recording stop and insertion.
- Target app uses a custom editor where selection offsets are unavailable.
- Very large text fields (avoid capturing full contents; only capture a bounded excerpt).
- Clipboard is temporarily busy/locked by another app.
- Multiple selections (if supported by the focused app).
- Editable-looking UI that is actually non-editable (e.g., chat transcript view).
- Secure fields where the OS does not reliably label them (avoid overly aggressive guessing, but bias toward safety).

## Requirements _(mandatory)_

### Functional Requirements

- **FR-001**: The system MUST detect the currently focused UI element on Windows at the time an insertion or context capture is attempted.
- **FR-002**: The system MUST determine whether the focused element is editable, enabled, and safe to interact with.
- **FR-003**: The system MUST block reading and insertion when the focused element is a password/secure field.
- **FR-004**: When the focused element supports retrieving selected text, the system MUST be able to capture the selected text (bounded to a maximum length).
- **FR-005**: When the focused element supports retrieving surrounding text, the system SHOULD capture a bounded excerpt around the caret/selection for use as context.
- **FR-006**: The system MUST choose an insertion method using this priority order: (1) direct insertion through the focused control when available, (2) paste-based fallback, (3) simulated typing fallback.
- **FR-007**: The system MUST restore the user's clipboard after any paste-based fallback.
- **FR-008**: The system MUST avoid inserting into an unintended target by re-checking that the insertion target matches the captured snapshot (or otherwise aborting to the safe fallback).
- **FR-009**: If insertion cannot be performed safely, the system MUST use the safe fallback: auto-copy the transcript to the clipboard and show a clear notification/toast.
- **FR-010**: The system SHOULD verify insertion success using **non-content signals only** (e.g., API call result/error codes, timeouts, target still matches the captured snapshot, and clipboard-restore success for paste) and MUST fall back if verification fails.
- **FR-011**: The system SHOULD maintain a per-application preference/cache to improve reliability (e.g., remembering that certain apps require paste-based insertion) and persist it locally across restarts.
- **FR-012**: The system MUST keep diagnostics local-only and MUST NOT upload captured context or field contents without explicit user configuration.
- **FR-013**: Clipboard context is a separate, explicit opt-in. It MUST NOT be used as a fallback for highlighted-text capture.

### Acceptance Criteria (by requirement)

- **AC-FR-001**: When insertion/context capture is attempted, the system identifies the current focused element and its owning application.
- **AC-FR-002**: Disabled or read-only fields do not receive insertion; the system uses the safe fallback.
- **AC-FR-003**: Password/secure fields never receive insertion and never have on-screen text captured.
- **AC-FR-004**: When a selection is present and retrievable, the captured selected text matches what the user selected (up to the configured maximum length).
- **AC-FR-005**: When surrounding text is retrievable, the captured excerpt is bounded and includes nearby text around the caret/selection.
- **AC-FR-006**: The system attempts insertion methods in the documented priority order.
- **AC-FR-007**: After any clipboard-based insertion attempt, the user's clipboard content is restored.
- **AC-FR-008**: If focus/target changes before insertion, the system aborts insertion and uses the safe fallback.
- **AC-FR-009**: If insertion cannot be performed safely, the system auto-copies the transcript to the clipboard and shows a clear notification/toast.
- **AC-FR-010**: When non-content verification is possible (e.g., method returned an error, timed out, target mismatch was detected, or clipboard-restore validation failed) and it fails, the system falls back automatically and reports a clear error state.
- **AC-FR-011**: The system remembers per-application insertion preferences locally across restarts and improves success rates over repeated attempts.
- **AC-FR-012**: No captured context or field contents leave the device without explicit user configuration.
- **AC-FR-013**: When UIA cannot retrieve highlighted text, the system does not fall back to clipboard selection capture; clipboard context is only included when explicitly enabled.

### Key Entities _(include if feature involves data)_

- **Context Snapshot**: A short-lived record of the active window/process and focused element capabilities, plus optional bounded context text.
- **Insert Plan**: The user-intended insertion behavior (insert vs replace selection vs replace all) plus the text to insert.
- **Safety Policy**: Rules for when context capture/insertion is allowed vs blocked (e.g., password fields, disabled/read-only).
- **App Capability Memory**: A local-only mapping of app/process -> preferred fallback behavior, persisted across restarts.

## Success Criteria _(mandatory)_

### Measurable Outcomes

- **SC-001**: In Notepad and WordPad, users can complete a dictation-and-insert flow successfully at least 80% of the time without manual retry.
- **SC-002**: In at least 3 common non-standard editors (e.g., browser text areas or Electron apps), insertion succeeds via automatic fallback at least 90% of the time, and the user's clipboard is restored.
- **SC-003**: The system performs zero insertions into detected password/secure fields in manual test runs.
- **SC-004**: When focus changes mid-flow, the system never inserts into the wrong window during manual test runs; instead it aborts to the safe fallback.
- **SC-005**: Context captured from fields (selection/surrounding excerpt) is bounded and does not exceed configured limits in 100% of test cases.
