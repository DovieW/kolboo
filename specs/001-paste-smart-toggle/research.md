# Research: Paste Safety Toggle

## Decision 1: Add a new settings key for smart paste protection
- **Decision**: Introduce a new boolean setting `output_smart_paste_protection` (default: `false`) stored in `settings.json`.
- **Rationale**: A dedicated key cleanly expresses user intent and keeps behavior explicit. It matches existing settings patterns (snake_case, boolean flags) and can be migrated safely.
- **Alternatives considered**:
  - Reuse `output_clipboard_privacy_mode` (rejected: it controls clipboard restore behavior, not safety checks).
  - Make it per-profile (rejected: the feature is global output safety and should be consistent across profiles).

## Decision 2: Gate Windows safety checks with the new setting
- **Decision**: When the setting is **on**, keep the existing Windows UIA safety checks (password/read-only/disabled). When **off**, bypass these checks and attempt normal output; if output fails, fallback to clipboard as usual.
- **Rationale**: This preserves current safe behavior for users who opt in while giving others the flexibility to paste into protected fields when they want to.
- **Alternatives considered**:
  - Disable only password checks (rejected: inconsistent user expectations and partial safety).
  - Always apply safety (rejected: user request explicitly wants a toggle, default off).

## Decision 3: Use existing store and update flow
- **Decision**: Persist updates via the existing settings patch flow (`settings_apply_patch` on the backend and `@tauri-apps/plugin-store` on the frontend).
- **Rationale**: The app already uses `settings.json` as the source of truth; reusing the same flow keeps changes consistent and immediate.
- **Alternatives considered**:
  - Add a new dedicated command (rejected: unnecessary additional surface area for a single boolean setting).

## References
- Tauri plugin-store docs show using `Store.load("settings.json")` and standard `get/set` flows for settings persistence.
