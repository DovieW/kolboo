# Research: Disable Profile Toggle

This feature is a small extension to the existing per-program profile system.

## Findings (existing patterns in this repo)

- Profiles are persisted under the `rewrite_program_prompt_profiles` key in `settings.json`.
- The UI writes settings through the backend command `settings_apply_patch`, then reloads settings from disk.
- Profile changes already call `configAPI.syncPipelineConfig()` (see `useUpdateRewriteProgramPromptProfiles()`), so runtime behavior can update without restart.
- The backend builds runtime `ProgramPromptProfile` candidates from stored `RewriteProgramPromptProfile` values (see `app/src-tauri/src/bootstrap/mod.rs`).
- There is a schema lockfile for `RewriteProgramPromptProfile` (`app/src-tauri/gen/schemas/rewrite-program-profile.schema.json`) with tests that enforce it.

## Decisions

### Decision: Add `disabled: boolean` on each profile

- **Decision**: Extend `RewriteProgramPromptProfile` with a `disabled` boolean that indicates the profile should never be activated.
- **Rationale**: A boolean is easy to understand, easy to persist, and matches the user-facing toggle (“Disable profile”).
- **Alternatives considered**:
	- `enabled: boolean` (works, but reads awkwardly with a “Disable profile” toggle and requires inversion in the UI)
	- `disabled_at` timestamp (overkill; adds complexity with no clear benefit)

### Decision: Backward compatibility = missing `disabled` means `false`

- **Decision**: If existing settings/profiles do not have the key, treat them as enabled.
- **Rationale**: This avoids a migration requirement for existing users and matches the principle that new settings should have safe defaults.
- **Alternatives considered**:
	- Add a settings migration to explicitly seed `disabled=false` everywhere (not necessary; normalization can do it)

### Decision: Filter disabled profiles at runtime (backend), not by deleting them

- **Decision**: Keep disabled profiles in persisted settings, but exclude them when building runtime activation candidates.
- **Rationale**: Users want “temporarily disable” without losing their profile configuration. Filtering makes “never activate” reliable even if the UI has stale caches.
- **Alternatives considered**:
	- Remove disabled profiles from the list entirely (violates “temporarily” and loses data)
	- Filter only in the UI (not robust; backend still could activate)

### Decision: Immediate deactivation via pipeline config sync + active-profile validation

- **Decision**: When profiles change and pipeline config is synced, validate whether the current active profile is still eligible; if it became disabled, clear/reselect it immediately.
- **Rationale**: This matches the clarified requirement (disable while active deactivates immediately) and avoids user confusion in overlays.
- **Alternatives considered**:
	- Only apply on the next recording/transcription start (simpler, but violates “immediate”)

### Decision: Rename “Disable all overrides” → “Reset profile” without behavior change

- **Decision**: Keep the existing reset implementation (set override fields to `null`, keep `program_paths`), but rename the UI label and confirmation copy.
- **Rationale**: Users interpret “disable” as turning off the profile; “Reset profile” better communicates “clear overrides”.
- **Alternatives considered**:
	- Rename + change behavior to delete profile (out of scope and risky)

## Open Questions

None remaining for planning. The only behavior ambiguity (disabling an active profile) was clarified: it should deactivate immediately.
