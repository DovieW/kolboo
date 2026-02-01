# Feature Specification: Quick Ask dismiss options

**Feature Branch**: `001-quick-ask-dismiss`
**Created**: 2026-01-30
**Status**: Draft
**Input**: User description: "Quick Ask should have a drop down for each profile in the settings. So it's a per profile setting with default override, just like all the other settings that has multiple options for how the Quick Ask overlay gets dismissed. By default, the option should be manual dismiss or just call it manual. And there will be a X button in the top right corner of the overlay. The X button should not add more height to the overlay. It should just be right aligned on the same question where the actual question, the transcribed question shows. And then there'll be another option in the dropdown for auto dismiss or just auto call it, which is when the user clicks away, it will dismiss."

## User Scenarios & Testing _(mandatory)_

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

### User Story 1 - Choose dismiss behavior per profile (Priority: P1)

As a user, I can set a Quick Ask dismiss mode per profile (with a default override) so each profile behaves the way I expect without extra steps.

**Why this priority**: It directly controls how Quick Ask closes, which affects every use of the overlay.

**Independent Test**: Can be fully tested by changing the per-profile setting and observing the overlay’s dismiss behavior for that profile.

**Acceptance Scenarios**:

1. **Given** a profile with dismiss mode set to Manual, **When** Quick Ask is opened, **Then** it does not dismiss on click-away and only closes via the explicit close control.
2. **Given** a profile with dismiss mode set to Auto, **When** Quick Ask is open and the user clicks away, **Then** the overlay dismisses.

---

### User Story 2 - Close from the overlay itself (Priority: P2)

As a user, I can close the Quick Ask overlay using a visible X button aligned with the question text so I can dismiss it without changing the layout.

**Why this priority**: Provides a clear, accessible close control that works regardless of click-away behavior.

**Independent Test**: Can be fully tested by opening Quick Ask and clicking the X button, confirming layout stays the same height.

**Acceptance Scenarios**:

1. **Given** Quick Ask is open, **When** the user clicks the X button, **Then** the overlay closes.
2. **Given** Quick Ask is open, **When** the X button is visible, **Then** the overlay height does not increase compared to the same view without the button.

---

### Edge Cases

<!--
  ACTION REQUIRED: The content in this section represents placeholders.
  Fill them out with the right edge cases.
-->

- What happens when a profile has no explicit dismiss mode set (uses the default override)?
- How does the system behave if the active profile is switched while Quick Ask is open?
- What happens when click-away occurs while the overlay is already closing?

## Scope

**In scope**:

- A per-profile dismiss mode setting with a default override.
- Manual and Auto dismiss behaviors for Quick Ask.
- An inline X button aligned with the question row without increasing overlay height.

**Out of scope**:

- Changes to other overlays or dismiss behaviors outside Quick Ask.
- New dismiss modes beyond Manual and Auto.

## Requirements _(mandatory)_

<!--
  ACTION REQUIRED: The content in this section represents placeholders.
  Fill them out with the right functional requirements.
-->

### Functional Requirements

- **FR-001**: System MUST allow a per-profile Quick Ask dismiss mode with a default override.
- **FR-002**: System MUST provide at least two dismiss mode options: Manual and Auto.
- **FR-003**: The default dismiss mode MUST be Manual for profiles that do not override it.
- **FR-004**: When dismiss mode is Manual, the overlay MUST remain open on click-away and close only via the explicit close control.
- **FR-005**: When dismiss mode is Auto, the overlay MUST dismiss on click-away.
- **FR-006**: The overlay MUST display an X close button aligned with the question text row, right-aligned on the same line.
- **FR-007**: Showing the X button MUST NOT increase the overlay’s height compared to the same content without the button.
- **FR-008**: The selected dismiss mode MUST persist per profile and be applied whenever Quick Ask opens for that profile.

### Key Entities _(include if feature involves data)_

- **Profile**: A user-selectable context that can override Quick Ask behavior settings.
- **Quick Ask Dismiss Mode**: A per-profile preference indicating Manual or Auto dismiss behavior.

## Assumptions

- Profiles already exist and can store per-profile settings with a default override.
- Click-away is a supported user action for dismissing overlays in the product experience.

## Dependencies

- The settings area can display per-profile dropdowns consistent with other multi-option settings.
- The Quick Ask overlay can display an inline close control without changing its overall height.

## Success Criteria _(mandatory)_

<!--
  ACTION REQUIRED: Define measurable success criteria.
  These must be technology-agnostic and measurable.
-->

### Measurable Outcomes

- **SC-001**: Users can change the Quick Ask dismiss mode for a profile in under 30 seconds.
- **SC-002**: In Manual mode, 100% of observed click-away actions keep the overlay open until the X button is used.
- **SC-003**: In Auto mode, 100% of observed click-away actions close the overlay.
- **SC-004**: The overlay’s height remains unchanged (no increase) when the X button is shown.
