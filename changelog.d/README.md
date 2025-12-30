# changelog.d

This directory stores **changelog fragments** (small Markdown files) that get assembled into release notes.

## Why this exists

Keeping changelog entries as small fragments makes it easy to:

- write release notes incrementally,
- avoid merge conflicts in a giant CHANGELOG file,
- generate GitHub Release notes automatically.

## How to add an entry

Add a new `*.md` file for your change.

Suggested naming (pick whatever you like, consistency matters more than the scheme):

- `123.feature.md`
- `124.fix.md`
- `125.breaking.md`

## Content guidelines

- Use bullet points.
- Keep it user-facing (what changed, why it matters).
- Avoid internal refactors unless they affect users.

Example:

- Added offline Whisper mode (no API key required).
- Fixed hotkey registration on Windows.
