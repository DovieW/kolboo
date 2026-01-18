# Ticket: Split Rust commands/text.rs into focused modules

## Goal (what we want)

Make the tricky OS-level text pipeline easier to maintain by splitting `app/src-tauri/src/commands/text.rs` into focused modules, while keeping the public command API stable.

- We want: smaller modules (clipboard vs injection vs selection probing).
- So that: future bugfixes and tests are localized and less risky.

## Context (what exists today)

- Current file: `app/src-tauri/src/commands/text.rs` (~32KB)
- It currently mixes:
  - output injection
  - clipboard lifecycle (including Windows-specific behavior)
  - selection probing strategies

## Acceptance criteria (how we know it’s done)

- [ ] Create modules:
  - `app/src-tauri/src/text/clipboard.rs`
  - `app/src-tauri/src/text/inject.rs`
  - `app/src-tauri/src/text/selection_probe.rs`
- [ ] Keep `app/src-tauri/src/commands/text.rs` as thin wrappers around those modules so callers/commands do not change.
- [ ] Ensure Windows behavior stays intact (no logging of secrets/clipboard contents).
- [ ] Rust tests compile and run.

## Edge cases / gotchas

- Windows cfg-specific code: avoid breaking builds on non-Windows.
- Clipboard restore logic is easy to regress; preserve ordering carefully.

## Non-goals (explicitly out of scope)

- No behavioral changes to injection modes.
- No new dependencies unless clearly necessary.
