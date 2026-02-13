# Data Model: Paste Safety Toggle

## Entity: AppSettings

### Fields
- `output_smart_paste_protection` (boolean)
  - **Meaning**: Whether smart paste protection (avoid sensitive targets) is enabled.
  - **Default**: `false`
  - **Validation**: Must be a boolean; missing/invalid values fall back to default.
  - **Scope**: Global (not per-profile).

### Relationships
- Part of `settings.json` (Tauri store) and surfaced through the `AppSettings` type.

### State Transitions
- `false → true`: User enables protection in UI tab.
- `true → false`: User disables protection in UI tab.

### Notes
- Applies primarily to Windows UIA insertion safety checks.
