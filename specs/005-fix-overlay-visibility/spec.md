# Feature Specification: Fix overlay visibility after wake

**Feature Branch**: `005-fix-overlay-visibility`  
**Created**: 2026-01-26  
**Status**: Draft  
**Input**: User description: "Actively recording overlay doesn't seem to show (noticed after waking computer); app logs indicate the overlay was asked to show and reported as visible, but the user couldn’t see it."

## User Scenarios & Testing *(mandatory)*

<!--
  IMPORTANT: User stories should be PRIORITIZED as user journeys ordered by importance.
  Each user story/journey must be INDEPENDENTLY TESTABLE - meaning if you implement just ONE of them,
  you should still have a viable MVP (Minimum Viable Product) that delivers value.
  
  Assign priorities (P1, P2, P3, etc.) to each story, where P1 is the most critical.
  Think of each story as a standalone slice of functionality that can be:
  - Developed independently
  - Tested independently
  - Deployed independently
  - Demonstrated to users independently
-->

### User Story 1 - Overlay always appears during recording (Priority: P1)

As a user, when I start recording, I want to see the “actively recording” overlay so I can quickly confirm recording is active and I’m not accidentally recording (or not recording).

**Why this priority**: This is immediate, core feedback during recording. If it fails, the app feels broken and users lose confidence.

**Independent Test**: Can be fully tested by starting a recording and visually verifying the overlay appears and stays visible until recording stops.

**Acceptance Scenarios**:

1. **Given** the app is running and overlay is enabled, **When** the user starts recording, **Then** the actively recording overlay becomes visible without requiring any manual refresh.
2. **Given** the overlay is currently visible during recording, **When** the user stops/cancels recording, **Then** the overlay hides and no “stuck overlay” remains.

---

### User Story 2 - Overlay recovers after sleep/wake (Priority: P2)

As a user, after my computer sleeps and wakes (or the display powers off/on), the overlay should still show up the next time I record.

**Why this priority**: This is a high-frequency real-world workflow (laptop usage). The overlay being “logically visible” but not actually seen is confusing.

**Independent Test**: Can be fully tested by putting the computer to sleep, waking it, then starting a recording and verifying the overlay is visible.

**Acceptance Scenarios**:

1. **Given** the computer has resumed from sleep (or display re-attached), **When** the user starts recording, **Then** the overlay becomes visible on-screen.
2. **Given** the system believes the overlay is visible, **When** the overlay is not actually visible to the user, **Then** the system performs a recovery attempt that results in a visible overlay.

---

### User Story 3 - Overlay stays on-screen across monitor/DPI changes (Priority: P3)

As a user, if my monitor configuration changes (dock/undock, DPI scale changes, external monitor removed), the overlay should remain visible and not end up off-screen.

**Why this priority**: Sleep/wake often coincides with monitor changes; “window is visible=true but off-screen” is a common failure mode.

**Independent Test**: Can be fully tested by changing monitor configuration and starting a recording, then verifying the overlay appears within the visible bounds of an active monitor.

**Acceptance Scenarios**:

1. **Given** the user changes monitor layout or DPI scale, **When** the user starts recording, **Then** the overlay appears within the visible bounds of a connected monitor.

---

[Add more user stories as needed, each with an assigned priority]

### Edge Cases

- Overlay window exists but is behind other windows despite being “visible”.
- Overlay window is moved to a position that is no longer valid after monitor changes (off-screen).
- Monitor count changes between “show requested” and actual window display.
- Display scaling changes cause the overlay to render at an unexpected location/size.
- Recording starts while the system is still resuming and the operating system temporarily reports inconsistent window state.

## Requirements *(mandatory)*

<!--
  ACTION REQUIRED: The content in this section represents placeholders.
  Fill them out with the right functional requirements.
-->

### Functional Requirements

- **FR-001**: When recording starts, the system MUST show the actively recording overlay.
- **FR-002**: The overlay MUST be visible on-screen even after sleep/wake or display reconnect events.
- **FR-003**: If a show request completes but the overlay is not actually visible to the user, the system MUST attempt recovery to make it visible again without requiring an app restart.
- **FR-004**: Showing or recovering the overlay MUST NOT steal keyboard focus from the user’s current app.
- **FR-005**: The system MUST record diagnostic information that support/developers can use to confirm: the overlay was requested, where it was expected to appear, whether recovery was attempted, and the final user-visible outcome.

### Assumptions & Dependencies

- Overlay is a user-facing confirmation and should be reliable even if other window state becomes inconsistent.
- The system can detect “not actually visible” states via operating system state (or via a conservative fallback heuristic).
- Recovery actions must be safe to run multiple times and should not create duplicate overlay windows.
- **Out of scope**: Changing the overlay’s visual design/content, adding new overlay modes, or changing recording shortcut behavior.

### Key Entities *(include if feature involves data)*

- **Overlay visibility state**: Whether the overlay is intended to be shown/hidden, plus its last-known position and size.
- **Current display setup**: The set of connected screens and their usable bounds (as observed when showing the overlay).

## Success Criteria *(mandatory)*

<!--
  ACTION REQUIRED: Define measurable success criteria.
  These must be technology-agnostic and measurable.
-->

### Measurable Outcomes

- **SC-001**: In a manual test run of 20 consecutive recording starts (including at least 5 after sleep/wake), the actively recording overlay is visible every time.
- **SC-002**: The overlay becomes visible within 1 second of starting recording in normal conditions.
- **SC-003**: Starting/stopping recording does not interrupt typing: in 10 trials, the user’s keyboard focus remains in the previously active app.
- **SC-004**: Reduce reports of “recording overlay not showing” to near-zero for users who frequently sleep/wake their device.
