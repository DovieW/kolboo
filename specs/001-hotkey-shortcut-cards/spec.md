# Feature Specification: Hotkey Shortcut Cards

**Feature Branch**: `001-hotkey-shortcut-cards`
**Created**: 2026-02-01
**Status**: Draft
**Input**: User description: "Currently the hotkeys settings page is kind of like a list and it's really hard to parse and it's messy. I would prefer if every shortcut that exists was a card and it only showed shortcuts that were set and then there was a way to create new shortcuts. So let's say you do, there was a drop down, and in the drop down it showed all the available types of shortcuts you could create, and then you pressed add, and then it would add that as an option, as a shortcut, and you can set it, or unset it, or delete it. So it's much cleaner. It doesn't show shortcuts that are not set. And as an added benefit, you can set the same shortcut, the same function multiple times. So you can have the same feature that can be activated with different shortcuts."

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

### User Story 1 - Scan and manage existing shortcuts (Priority: P1)

As a user, I can quickly scan my current shortcuts because each configured shortcut is shown as a clean card instead of a long list.

**Why this priority**: The current list is hard to read; improving scan-ability is the core value of the change.

**Independent Test**: Can be fully tested by viewing the hotkeys settings page with existing shortcuts and confirming only configured shortcuts appear as cards.

**Acceptance Scenarios**:

1. **Given** I have at least one configured shortcut, **When** I open the hotkeys settings page, **Then** I see a card for each configured shortcut and no cards for unconfigured shortcuts.
2. **Given** I have no configured shortcuts, **When** I open the hotkeys settings page, **Then** I see an empty state explaining there are no shortcuts yet.

---

### User Story 2 - Add and edit a shortcut card (Priority: P2)

As a user, I can create a new shortcut by choosing a shortcut type from a dropdown and adding it, then set, unset, or delete that card.

**Why this priority**: Creating and managing shortcuts is the next most important action after being able to read the page easily.

**Independent Test**: Can be fully tested by adding a shortcut type, editing its key binding, unsetting it, and deleting it.

**Acceptance Scenarios**:

1. **Given** I am on the hotkeys settings page, **When** I select a shortcut type from the dropdown and press Add, **Then** a new card appears for that shortcut type.
2. **Given** a shortcut card exists, **When** I set a key binding, **Then** the card shows the configured shortcut.
3. **Given** a shortcut card exists, **When** I unset the key binding, **Then** the card shows that it is not set.
4. **Given** a shortcut card exists, **When** I delete the card, **Then** the card is removed from the page.

---

### User Story 3 - Multiple shortcuts for one action (Priority: P3)

As a user, I can assign more than one shortcut to the same action so I can trigger the same feature in different ways.

**Why this priority**: This is an added benefit that makes shortcuts more flexible but is not required for the basic cleanup.

**Independent Test**: Can be fully tested by adding the same shortcut type twice and setting different key bindings for each card.

**Acceptance Scenarios**:

1. **Given** the hotkeys settings page, **When** I add the same shortcut type more than once, **Then** multiple cards are created for that action.
2. **Given** multiple cards for the same action, **When** I assign different key bindings, **Then** both are saved and displayed.

---

### Edge Cases

- No shortcuts are configured yet (empty state should still guide the user to add one).
- All shortcut types are already added; the dropdown still allows adding another instance of a type.
- A user tries to assign a key binding that is already used by another shortcut.
- A user deletes the last shortcut card (page should return to the empty state).

## Assumptions

- Shortcut key combinations must be unique across all shortcut cards; if a conflict exists, the user is prompted to choose a different key.
- Users expect previously configured shortcuts to remain intact after the UI change.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The hotkeys settings page MUST display each configured shortcut as a separate card.
- **FR-002**: The page MUST NOT display cards for shortcut types that have never been added or configured.
- **FR-003**: Users MUST be able to add a new shortcut card by selecting a shortcut type from a dropdown and pressing Add.
- **FR-004**: The dropdown MUST list all available shortcut types, including ones already added, to allow multiple cards for the same action.
- **FR-005**: Users MUST be able to set a key binding on a shortcut card.
- **FR-006**: Users MUST be able to unset a key binding on a shortcut card without deleting the card.
- **FR-007**: Users MUST be able to delete a shortcut card.
- **FR-008**: The system MUST prevent saving a key binding that is already in use by another shortcut card and explain the conflict.
- **FR-009**: Shortcut changes MUST persist so that configured cards reappear on the next visit.
- **FR-010**: When no shortcut cards exist, the page MUST show an empty state with a clear call to add a shortcut.

### Key Entities *(include if feature involves data)*

- **Shortcut Card**: A user-configured shortcut entry that references one shortcut type and its current key binding (or unset state).
- **Shortcut Type**: An available action that can be triggered by a shortcut (can appear on multiple cards).
- **Key Binding**: The user-defined key combination assigned to a shortcut card.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 90% of users can find and edit a specific shortcut within 30 seconds on the hotkeys settings page.
- **SC-002**: 95% of users can add a new shortcut card and set a key binding without assistance.
- **SC-003**: The hotkeys page shows zero cards for shortcuts that were never configured.
- **SC-004**: Support requests about “finding a shortcut” decrease by 40% within one release cycle after launch.
