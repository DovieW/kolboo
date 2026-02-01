# Data Model

## Entities

### Profile

Represents a user-selectable profile that can override Quick Ask behavior.

- **id**: string (stable identifier)
- **name**: string
- **settings**: object (profile-specific overrides)

### QuickAskDismissMode

Represents how the Quick Ask overlay closes for a profile.

- **value**: "manual" | "auto"
- **default**: "manual"

### Settings

Persisted app settings (backed by `settings.json`) with a default override and per-profile overrides.

- **quickAskDismissModeDefault**: QuickAskDismissMode
- **quickAskDismissModeByProfile**: Record<Profile.id, QuickAskDismissMode | null>

## Relationships

- A **Profile** can override **Settings.quickAskDismissModeDefault** with its own **QuickAskDismissMode**.
- If a profile has no override, it inherits **quickAskDismissModeDefault**.

## Validation Rules

- If a profile override is missing or invalid, fall back to the default value.
- Only "manual" and "auto" are valid values.

## State Transitions

- Updating a profile override immediately affects the Quick Ask overlay behavior for that profile.
