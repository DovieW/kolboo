# Feature Specification: Active Window OCR Context

**Feature Branch**: `003-active-window-ocr`  
**Created**: 2026-01-25  
**Status**: Draft  
**Input**: User description: "I want to add the option to grab OCR context from the currently active window. So the way it would work is that if the context is enabled for whichever feature the user happens to be using in that moment, whether it's rewrite or quick replace or quick ask, it will essentially take a screenshot of the currently active window and send it off to the OCR service and return with a whatever it got and dump it to the LLM specifying that it is OCR context from the currently active window."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Use OCR context in a supported tool (Priority: P1)

As a user using a text tool (Rewrite, Quick Replace, or Quick Ask), I want the app to include text recognized from my currently active window as extra context, so the assistant can better understand what I’m looking at without me manually copying everything.

**Why this priority**: This is the core value: better answers/rewrites with less manual effort.

**Independent Test**: Enable OCR context for exactly one tool (e.g., Quick Ask), run that tool while a window containing readable text is active, and verify the resulting assistant request includes a clearly labeled “OCR context from the active window” section.

**Acceptance Scenarios**:

1. **Given** OCR context is enabled for Quick Ask and the user has an active window with visible text, **When** the user triggers Quick Ask, **Then** the assistant request includes an additional context section labeled as OCR from the active window.
2. **Given** OCR context is disabled for Rewrite, **When** the user triggers Rewrite, **Then** no OCR-derived context is captured or included.

---

### User Story 2 - Control OCR context per tool (Priority: P2)

As a user, I want to enable/disable “active window OCR context” separately for Rewrite, Quick Replace, and Quick Ask, so I can decide where this extra context is helpful versus intrusive.

**Why this priority**: Different tools have different sensitivity and workflows; per-tool control makes the feature usable.

**Independent Test**: Toggle OCR context on for one tool and off for another, then run each tool and verify OCR context is only present where enabled.

**Acceptance Scenarios**:

1. **Given** OCR context is enabled for Quick Replace and disabled for Quick Ask, **When** the user runs Quick Replace and then runs Quick Ask, **Then** only the Quick Replace request includes OCR context.

---

### User Story 3 - Safe failure behavior (Priority: P3)

As a user, if text recognition fails (permissions, unsupported window, or temporary service issues), I want the tool to still run without OCR context and to receive a clear, non-alarming explanation.

**Why this priority**: Reliability and trust—OCR context should improve the experience, not block it.

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
- “Currently active window” means the OS-focused window at the moment the user triggers the tool.
- OCR context is opt-in and defaults to off (to avoid surprise capture/processing).

### Dependencies

- Operating system support/permissions for identifying and capturing the active window.
- Availability of the OCR capability (local or remote) at runtime.

### Functional Requirements

- **FR-001**: Users MUST be able to enable or disable “Active Window OCR Context” independently for Rewrite, Quick Replace, and Quick Ask.
- **FR-002**: When a user triggers a tool where OCR context is enabled, the system MUST attempt to extract text from the currently active window and include it as additional context for the assistant.
- **FR-003**: The system MUST label the added context clearly as “OCR context from the currently active window” so it is distinguishable from other context sources.
- **FR-004**: When OCR context is disabled for a tool, the system MUST NOT capture/process OCR for that tool.
- **FR-005**: If OCR context cannot be obtained (for any reason), the system MUST continue running the tool without OCR context.
- **FR-006**: If OCR context cannot be obtained, the system MUST provide a user-understandable explanation (e.g., “OCR context unavailable”) without exposing sensitive technical details.
- **FR-007**: The system MUST limit OCR context size (e.g., truncation and/or summarization) to keep assistant prompts manageable while retaining the most relevant text.
- **FR-008**: The system MUST minimize sensitive data exposure by default (e.g., avoid retaining captured window imagery and OCR-derived text beyond the immediate request unless the user explicitly opts in to persistence).

### Key Entities *(include if feature involves data)*

- **OCR Context Setting**: Per-tool preference indicating whether OCR context should be captured and included.
- **OCR Context Payload**: The extracted text plus lightweight metadata needed for clarity (e.g., capture time and optional window label), excluding captured imagery by default.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: When OCR context is enabled for a tool, users can successfully include OCR-derived active-window text in at least 90% of attempts where (a) the active window contains readable, high-contrast text and (b) required OS permissions are granted.
- **SC-002**: When OCR context is disabled for a tool, OCR-derived context is included in 0% of those tool requests (verified via automated tests and/or request inspection logs).
- **SC-003**: In failure cases, the tool completes successfully (no user-blocking errors) in 100% of tested OCR failure scenarios.
- **SC-004**: Users can enable/disable the option per tool in under 30 seconds without needing external documentation.
