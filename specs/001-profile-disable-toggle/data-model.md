# Data Model: Disable Profile Toggle

This feature extends the existing per-program profile model.

## Entity: RewriteProgramPromptProfile (Profile)

Represents a named profile that may match certain programs and apply overrides.

### Fields (conceptual)

- `id: string`
	- Unique identifier for the profile.
- `name: string`
	- Human-friendly display name.
- `program_paths: string[]`
	- Program matchers (full paths or basenames).
- `disabled: boolean`
	- New field.
	- **Meaning**: if `true`, this profile is never eligible for activation.
	- **Default**: `false` when missing.
- `overrides: object`
	- Existing “override surface” (prompt overrides, provider/model overrides, UI overrides, presets/router, etc.).

### Validation Rules

- `disabled` MUST be a boolean. If missing or invalid, treat as `false`.
- Disabling a profile MUST NOT delete it.
- Resetting a profile MUST reset overrides only; it MUST NOT change `program_paths` and MUST NOT change `disabled`.

## State Transitions

### Enabled → Disabled

- Trigger: user toggles “Disable profile” on.
- Result:
	- Profile becomes ineligible for activation.
	- If it is currently active, it is immediately deactivated and the system falls back.

### Disabled → Enabled

- Trigger: user toggles “Disable profile” off.
- Result:
	- Profile becomes eligible for activation again.

## Relationships

- A profile can be associated with 0..N program paths.
- Activation logic selects 0..1 effective profile at runtime.
