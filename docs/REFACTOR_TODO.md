# Refactor TODOs

## Follow-ups

- Backend-only settings writer: move UI settings writes to Rust patch commands and emit `settings-changed` to prevent stale snapshot clobbers across windows.
