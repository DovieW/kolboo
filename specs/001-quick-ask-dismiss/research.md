# Research

## Decision 1: Store dismiss mode alongside existing per-profile settings

- **Decision**: Add a per-profile `quick_ask_dismiss_mode` setting with a default override stored in the Tauri settings store.
- **Rationale**: The app already persists settings in `settings.json` and supports profile overrides, so this keeps behavior consistent and migratable.
- **Alternatives considered**: Hard-coding dismiss mode in UI state only (rejected because it would not persist per profile).

## Decision 2: Honor dismiss mode in overlay interaction logic

- **Decision**: Manual mode ignores click-away and requires the explicit close control; Auto mode dismisses on click-away.
- **Rationale**: Matches the spec’s required behaviors and keeps logic localized to overlay event handlers.
- **Alternatives considered**: Adding a separate timer-based auto-dismiss (rejected because it was out of scope).

## Decision 3: Inline close control on the question row

- **Decision**: Place the X button in the same row as the transcribed question, right-aligned, without affecting overlay height.
- **Rationale**: Provides an explicit close action while keeping layout stable.
- **Alternatives considered**: Adding a new header row for the close button (rejected because it increases height).
