# Quickstart: Hotkey Shortcut Cards

## Goal

Validate the new hotkeys settings UI: cards, add flow, set/unset/delete, and duplicate shortcuts per action.

## Local validation steps

1. Open the app and navigate to Settings → Hotkeys.
2. Confirm only configured shortcuts appear as cards.
3. Use the dropdown to add a shortcut type and press Add.
4. Set a key binding and verify it displays on the card.
5. Unset the key binding and verify the card stays but shows unset.
6. Delete the card and confirm it disappears.
7. Add the same shortcut type twice and set different key bindings.
8. Try setting a key binding that already exists on another card and confirm a conflict message appears.

## Tests

- UI tests: `pnpm -C app test`
- Rust tests (only if backend changes): `pnpm -C app cargo:test`
- CI gate: `pnpm -C app check:ci`
