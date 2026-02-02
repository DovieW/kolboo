# Feature Specification: Paste Safety Toggle

**Feature Branch**: `001-paste-smart-toggle`
**Created**: 2026-02-01
**Status**: Draft
**Input**: User description: "currently we have the \"paste\" try to be smart and try not to paste into sensitive places. i want this to be made a setting in the UI tab and it should be off by default"

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

### User Story 1 - Control smart paste protection (Priority: P1)

As a user, I can turn smart paste protection on or off from the UI tab so I decide whether the app tries to avoid pasting into sensitive places.

**Why this priority**: Users need control over the behavior because it can be helpful or annoying depending on context.

**Independent Test**: Can be fully tested by toggling the setting in the UI tab and verifying the behavior changes for subsequent paste attempts.

**Acceptance Scenarios**:

1. **Given** a new or reset setup, **When** I open the UI tab, **Then** the smart paste protection setting is visible and off by default.
2. **Given** the setting is off, **When** I turn it on, **Then** smart paste protection is enabled for future paste attempts and the change is saved.
3. **Given** the setting is on, **When** I turn it off, **Then** smart paste protection is disabled for future paste attempts and the change is saved.

---

### User Story 2 - Understand what the setting does (Priority: P2)

As a user, I can see a short explanation so I understand what “smart paste protection” changes.

**Why this priority**: Clear wording reduces confusion and prevents accidental behavior changes.

**Independent Test**: Can be tested by checking that the UI tab shows a helpful description next to the setting.

**Acceptance Scenarios**:

1. **Given** I am viewing the UI tab, **When** I look at the setting, **Then** I see a short description that explains what the protection does.

---

### User Story 3 - Safe fallback when save fails (Priority: P3)

As a user, if the setting cannot be saved, I am told that it did not stick so I am not surprised later.

**Why this priority**: It prevents silent failures that could lead to confusing paste behavior.

**Independent Test**: Can be tested by simulating a save failure and confirming the user is informed and the prior value remains.

**Acceptance Scenarios**:

1. **Given** the setting cannot be saved, **When** I try to change it, **Then** I see a clear message and the previous value remains active.

### Edge Cases

- User toggles the setting while a paste action is already in progress.
- The saved setting is missing or invalid and needs a safe default on next launch.
- The UI is opened on a device/layout where smart paste protection is not available.

## Requirements *(mandatory)*

<!--
  ACTION REQUIRED: The content in this section represents placeholders.
  Fill them out with the right functional requirements.
-->

### Functional Requirements

- **FR-001**: The UI tab MUST include a setting that controls smart paste protection.
- **FR-002**: The setting MUST be off by default for new users and when no valid value is stored.
- **FR-003**: Users MUST be able to toggle the setting on and off.
- **FR-004**: Changes to the setting MUST take effect for future paste attempts without requiring a restart.
- **FR-005**: The setting MUST persist so the chosen value remains after restarting the app.
- **FR-006**: When the setting is off, paste behavior MUST not use smart protection.
- **FR-007**: When the setting is on, paste behavior MUST use the existing smart protection logic.
- **FR-008**: If the setting cannot be saved, the user MUST be informed and the previous value MUST remain active.

### Key Entities *(include if feature involves data)*

- **Paste Protection Setting**: A user preference that is either on or off and determines whether smart paste protection is applied.

## Assumptions

- Smart paste protection already exists and remains unchanged; this feature only adds user control in the UI tab.
- “Sensitive places” are defined by the current smart paste behavior and are not redefined here.
- If the stored value is missing or invalid, the app uses the default (off).

## Success Criteria *(mandatory)*

<!--
  ACTION REQUIRED: Define measurable success criteria.
  These must be technology-agnostic and measurable.
-->

### Measurable Outcomes

- **SC-001**: 100% of new installs show smart paste protection set to off by default.
- **SC-002**: At least 95% of setting changes persist after a restart in manual QA checks.
- **SC-003**: 90% of users can find and change the setting in the UI tab within 10 seconds in usability testing.
- **SC-004**: Support requests about unexpected paste blocking decrease by 30% within one release cycle.
