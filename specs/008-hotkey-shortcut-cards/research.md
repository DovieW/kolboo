# Research: Hotkey Shortcut Cards

## Decision 1: Performance expectations

- **Decision**: No new explicit performance targets beyond keeping the hotkeys page responsive for typical shortcut counts (under 50 cards).
- **Rationale**: The feature is a UI layout change with small data sets; responsiveness is the key user-visible outcome.
- **Alternatives considered**: Setting a hard $p95$ render target (e.g., 100 ms). Rejected because we do not have existing benchmarks and the change should be validated qualitatively.

## Decision 2: Constraints and offline behavior

- **Decision**: Treat shortcuts as offline-only settings with no network calls, relying on existing Tauri store persistence.
- **Rationale**: Hotkeys are local preferences and must work without connectivity; the app already uses `settings.json` for storage.
- **Alternatives considered**: Syncing shortcuts via network. Rejected because it is out of scope and would add dependencies.

## Decision 3: Scope of storage changes

- **Decision**: Extend the existing settings shape to allow multiple shortcut cards for the same action, while preserving existing configured shortcuts during migration.
- **Rationale**: The spec requires duplicates; backward compatibility prevents loss of existing user settings.
- **Alternatives considered**: Replacing the settings structure entirely. Rejected due to migration risk and higher effort.
