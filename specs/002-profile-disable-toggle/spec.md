# Feature Specification: Disable Profile Toggle

**Feature Branch**: `002-profile-disable-toggle`
**Created**: 2026-01-25
**Status**: Draft
**Input**: User description: "in the profile config modal there should be a button to 'Disable profile'. which is a toggle which temporarily disables the entire profile so it never gets activated. also, rename the 'Disable all overrides' button to just 'Reset profile' since I think it is more descriptive."

## Clarifications

### Session 2026-01-25

- Q: What happens if the user disables a profile that is currently active? → A: Disabling immediately deactivates it (falls back to “no profile applies” or next eligible profile).

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

### User Story 1 - Temporarily disable a profile (Priority: P1)

As a user editing a profile, I want a simple toggle to disable the profile so it is never activated, without having to delete it or manually undo lots of settings.

**Why this priority**: This directly prevents unwanted behavior (the wrong profile activating) and gives users a fast “off switch” for troubleshooting.

**Independent Test**: Can be fully tested by disabling one profile and verifying that any activation flow never picks it, while the profile remains visible and editable.

**Acceptance Scenarios**:

1. **Given** a profile is enabled, **When** I toggle "Disable profile" on, **Then** the profile becomes disabled and is clearly indicated as disabled.
2. **Given** a profile is disabled, **When** the app would normally activate that profile, **Then** it does not activate and instead behaves as if the profile does not exist for activation.
3. **Given** a profile is disabled, **When** I close and reopen the app, **Then** the profile is still disabled.
4. **Given** a profile is currently active, **When** I toggle "Disable profile" on, **Then** it is immediately deactivated and no longer applies.
5. **Given** a profile is disabled, **When** I view the profile selector dropdown, **Then** that profile appears greyed out and crossed out so it is visibly disabled.

---

### User Story 2 - Re-enable a disabled profile (Priority: P2)

As a user, I want to re-enable a disabled profile so it becomes eligible for activation again.

**Why this priority**: Disabling is only useful if it’s easy to undo once the user is done troubleshooting or wants the profile back.

**Independent Test**: Can be fully tested by disabling, re-enabling, and confirming the profile can activate again.

**Acceptance Scenarios**:

1. **Given** a profile is disabled, **When** I toggle "Disable profile" off, **Then** the profile becomes enabled and eligible for activation.
2. **Given** a profile is disabled, **When** I edit its settings, **Then** my edits are preserved even while the profile stays disabled.

---

### User Story 3 - Reset a profile’s overrides (Priority: P3)

As a user, I want a clearly named action ("Reset profile") that resets the profile’s overrides back to their default/unset state, so I can quickly get back to a clean profile without deleting it.

**Why this priority**: The current label ("Disable all overrides") is easy to misinterpret; “Reset profile” better communicates “put it back to a baseline”.

**Independent Test**: Can be fully tested by setting overrides, clicking "Reset profile", and verifying the profile’s override values return to the baseline/unset state.

**Acceptance Scenarios**:

1. **Given** a profile has one or more overrides set, **When** I click "Reset profile", **Then** all override values in that profile return to the baseline/unset state.
2. **Given** a profile has no overrides set, **When** I click "Reset profile", **Then** nothing breaks and the profile remains unchanged.

---

### Edge Cases

- Disabling a profile that is currently active (expected: it immediately stops applying, and the system falls back to “no profile applies” or the next eligible profile).
- A disabled profile should not be silently re-enabled by other actions (editing, resetting, importing, etc.).
- Resetting a disabled profile should only reset its overrides; it should not automatically enable it.
- If there is only one profile and it is disabled, activation should fall back to “no profile applies” behavior rather than forcing activation.

## Requirements _(mandatory)_

### Functional Requirements

- **FR-001**: The profile configuration modal MUST provide a "Disable profile" toggle for each profile.
- **FR-002**: When a profile is disabled, the system MUST treat it as ineligible for activation in all activation flows (automatic and manual).
- **FR-003**: Users MUST be able to re-enable a disabled profile using the same toggle.
- **FR-004**: The disabled/enabled state MUST be saved so it persists across app restarts.
- **FR-005**: Disabled profiles MUST remain visible and editable (disabling is not deletion).
- **FR-006**: The existing "Disable all overrides" action MUST be renamed to "Reset profile" without changing its core behavior (it resets the profile’s overrides back to baseline/unset).
- **FR-007**: "Reset profile" MUST NOT delete the profile and MUST NOT automatically change whether the profile is disabled/enabled.
- **FR-008**: The UI MUST clearly communicate when a profile is disabled (so users can understand why it is not activating).
- **FR-009**: If a user disables a profile that is currently active, the system MUST immediately stop applying that profile and fall back to “no profile applies” or the next eligible profile.
- **FR-010**: Disabled profiles in the profile selector dropdown MUST appear greyed out and crossed out.

### Assumptions

- Users may want to disable a profile temporarily for troubleshooting or to prevent accidental activation.
- A disabled profile should behave like it is “not available” for activation, but it should remain available for editing.
- "Reset profile" means "reset the profile’s override values back to baseline/unset", not "delete profile".

### Out of Scope

- Deleting profiles, exporting/importing profiles, or changing how profiles are created.
- Any new user roles/permissions; this is a user-facing quality-of-life change.

### Key Entities _(include if feature involves data)_

- **Profile**: A user-defined configuration used for activation behavior; includes a name, a set of override values, and an enabled/disabled state.
- **Profile Overrides**: The set of fields within a profile that change behavior from the baseline; can be reset back to baseline/unset.

## Success Criteria _(mandatory)_

### Measurable Outcomes

- **SC-001**: In test scenarios that would otherwise activate a profile, disabled profiles are activated 0 times.
- **SC-002**: A user can disable or re-enable a profile in under 10 seconds (starting from opening the profile config modal).
- **SC-003**: In a basic usability check with 5 users, at least 4 out of 5 can correctly explain what "Reset profile" does after seeing the button label.
