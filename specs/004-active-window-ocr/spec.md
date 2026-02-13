# Feature Specification: Active Window OCR Context

**Feature Branch**: `004-active-window-ocr`
**Created**: 2026-01-25
**Status**: Draft
**Input**: User description: "I want to add the option to grab OCR context from the currently active window. So the way it would work is that if the context is enabled for whichever feature the user happens to be using in that moment, whether it's rewrite or quick replace or quick ask, it will essentially take a screenshot of the currently active window and send it off to the OCR service and return with a whatever it got and dump it to the LLM specifying that it is OCR context from the currently active window."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Use OCR context in a supported tool (Priority: P1)

As a user using a text tool (Rewrite, Quick Replace, or Quick Ask), I want the app to include text recognized from my currently active window as extra context, so the assistant can better understand what I'm looking at without me manually copying everything.

**Why this priority**: This is the core value: better answers/rewrites with less manual effort.

**Independent Test**: Enable OCR context for exactly one tool (e.g., Quick Ask), run that tool while a window containing readable text is active, and verify the resulting assistant request includes a clearly labeled "OCR context from the active window" section.

**Acceptance Scenarios**:

1. **Given** OCR context is enabled for Quick Ask and the user has an active window with visible text, **When** the user triggers Quick Ask, **Then** the assistant request includes an additional context section labeled as OCR from the active window.
2. **Given** OCR context is disabled for Rewrite, **When** the user triggers Rewrite, **Then** no OCR-derived context is captured or included.

---

### User Story 2 - Control OCR context per tool (Priority: P2)

As a user, I want to control "active window OCR context" separately for Rewrite, Quick Replace, and Quick Ask (including an optional manual trigger mode), so I can decide where this extra context is helpful versus intrusive.

**Why this priority**: Different tools have different sensitivity and workflows; per-tool control makes the feature usable.

**Independent Test**: Toggle OCR context on for one tool and off for another, then run each tool and verify OCR context is only present where enabled.

**Acceptance Scenarios**:

1. **Given** OCR context is enabled for Quick Replace and disabled for Quick Ask, **When** the user runs Quick Replace and then runs Quick Ask, **Then** only the Quick Replace request includes OCR context.

2. **Given** OCR context mode is set to Manual for a tool, **When** the user records/transcribes without pressing the OCR button, **Then** no OCR capture/request is performed.

3. **Given** OCR context mode is set to Manual, **When** the user presses the OCR button in the recording overlay, **Then** OCR capture/request begins immediately and (if it completes in time) the labeled OCR context is included.

4. **Given** OCR is in progress from a manual trigger, **When** the user changes the tool's OCR mode to Disabled, **Then** the OCR work is cancelled and no OCR context is included.

---

### User Story 3 - Safe failure behavior (Priority: P3)

As a user, if text recognition fails (permissions, unsupported window, or temporary service issues), I want the tool to still run without OCR context and to receive a clear, non-alarming explanation.

**Why this priority**: Reliability and trust - OCR context should improve the experience, not block it.

**Independent Test**: Simulate an OCR failure condition (e.g., feature disabled at OS level or no active window detected) and confirm the tool continues with a user-visible message and no OCR context included.

**Acceptance Scenarios**:

1. **Given** OCR context is enabled, **When** OCR cannot be obtained for the active window, **Then** the tool proceeds without OCR context and provides a clear message that OCR context was unavailable.

### Edge Cases

- Active window cannot be identified (no focused window, transient focus changes, or OS restriction).
- Active window is a protected surface where capture/recognition is blocked.
- Active window contains no readable text (images only, very small font, or heavy blur).
- Multiple monitors / DPI scaling / window partially off-screen.
- OCR returns very large text; context must be truncated/summarized to avoid overwhelming the assistant.
- User triggers tools repeatedly; OCR capture/recognition should not cause runaway delays.

## Requirements *(mandatory)*

### Assumptions

- The app already has a way to perform OCR (text recognition) and can request it during a user action.
- "Currently active window" means the OS-focused window at the moment the user triggers the tool.
- OCR context is opt-in and defaults to off (to avoid surprise capture/processing).

### Dependencies

- Operating system support/permissions for identifying and capturing the active window.
- Availability of an OCR capability (local or remote) at runtime.
- OCR provider configuration (URL/model and optionally auth) must be present for OCR to be considered "available".

### Functional Requirements

- **FR-001**: Users MUST be able to control "Active Window OCR Context" independently for Rewrite, Quick Replace, and Quick Ask.
- **FR-002**: Each tool's OCR setting MUST support three modes:
  - Disabled (never run OCR)
  - Auto (run OCR automatically when the tool is triggered)
  - Manual (show an OCR button in the recording overlay; OCR runs only after the user clicks it)
- **FR-003a**: When a user triggers a tool where OCR mode is Auto, the system MUST attempt to extract text from the currently active window and include it as additional context for the assistant.
- **FR-004**: When a tool's OCR mode is Manual, the system MUST NOT capture/process OCR unless the user clicks the OCR button in the recording overlay.
- **FR-003b**: The system MUST label the added context clearly as "OCR context from the currently active window" so it is distinguishable from other context sources.
- **FR-005a**: When OCR mode is Disabled for a tool, the system MUST NOT capture/process OCR for that tool.
- **FR-006a**: If OCR is in-progress (Auto or Manual) and the user disables OCR mode, the system MUST cancel the in-flight OCR work.
- **FR-005b**: If OCR context cannot be obtained (for any reason), the system MUST continue running the tool without OCR context.
- **FR-006b**: If OCR context cannot be obtained, the system MUST provide a user-understandable explanation (e.g., "OCR context unavailable") without exposing sensitive technical details.
- **FR-007**: The system MUST limit OCR context size (e.g., truncation and/or summarization) to keep assistant prompts manageable while retaining the most relevant text.
- **FR-008**: The system MUST minimize sensitive data exposure by default (e.g., avoid retaining captured window imagery and OCR-derived text beyond the immediate request unless the user explicitly opts in to persistence).

#### Provider configuration requirements

- **FR-009**: The app MUST support configuring an OCR provider in Settings (Providers tab) with at least:
  - a base URL (e.g., `http://localhost:8000` or `https://api.openai.com`)
  - a model identifier (string)
  - optional authentication (API key) stored securely

- **FR-010**: The OCR integration MUST be provider-agnostic within the OpenAI-style ecosystem:
  - It MUST work with locally hosted OpenAI-compatible servers (e.g. vLLM).
  - It MUST be possible to point it at other OCR-capable OpenAI-style providers by changing settings (URL/model and auth), without code changes.

- **FR-011**: If the configured OCR provider requires an API key, the system MUST NOT attempt OCR unless the key is present.

- **FR-012**: The system MUST NOT log OCR API keys or authorization headers.

### Key Entities *(include if feature involves data)*

- **OCR Provider Settings**: Base URL, model, and optional API key (stored securely).
- **OCR Context Setting**: Per-tool mode (`off` / `auto` / `manual`) indicating when OCR context is captured.
- **OCR Context Payload**: The extracted text plus lightweight metadata needed for clarity (e.g., capture time and optional window label), excluding captured imagery by default.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: When OCR context is enabled for a tool, users can successfully include OCR-derived active-window text in at least 90% of attempts where (a) the active window contains readable, high-contrast text and (b) required OS permissions are granted.
- **SC-002**: When OCR context is disabled for a tool, OCR-derived context is included in 0% of those tool requests (verified via automated tests and/or request inspection logs).
- **SC-003**: In failure cases, the tool completes successfully (no user-blocking errors) in 100% of tested OCR failure scenarios.
- **SC-004**: Users can enable/disable the option per tool in under 30 seconds without needing external documentation.


