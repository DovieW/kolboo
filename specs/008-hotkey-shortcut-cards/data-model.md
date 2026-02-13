# Data Model: Hotkey Shortcut Cards

## Entities

### ShortcutCard

Represents a single user-configured shortcut entry.

**Fields**:
- `id` (string): Unique identifier for the card.
- `type` (string): References a ShortcutType identifier.
- `keyBinding` (string | null): The assigned key combination; `null` means unset.
- `createdAt` (timestamp): Creation time (for ordering/display).

**Validation rules**:
- `type` must reference a valid ShortcutType.
- `keyBinding` must be unique across all ShortcutCards when not null.
- `keyBinding` may be null to represent an unset card.

**State transitions**:
- `created` → `set` when a keyBinding is assigned.
- `set` → `unset` when keyBinding is cleared.
- `created|set|unset` → `deleted` when the card is removed.

### ShortcutType

Represents an available action that can be triggered by a shortcut.

**Fields**:
- `id` (string): Stable identifier for the action.
- `label` (string): User-facing name for the action.
- `description` (string | null): Optional helper text.

**Relationships**:
- One ShortcutType can have many ShortcutCards.

### KeyBinding

A normalized representation of a key combination.

**Fields**:
- `value` (string): Display string for the key combination.
- `normalized` (string): Canonical string used for uniqueness checks.

**Validation rules**:
- `normalized` must be unique across all cards.
- `value` must be derived from a valid keyboard shortcut format.
