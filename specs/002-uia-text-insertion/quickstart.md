# Quickstart: Windows UIA Context + Insertion

This feature is Windows-only.

## Try it locally

1. Run the app in dev mode.

- `pnpm -C app dev`

2. Open a simple editor (recommended first): Notepad.

3. Place the caret in Notepad and run a flow that inserts text (dictation stop → insertion, or Quick Replace).

4) (Optional) Highlight a short snippet and run Quick Ask / Quick Replace to verify selected text is captured.
   - With “Include Clipboard Context” OFF, your clipboard should remain unchanged.
   - With “Include Clipboard Context” ON, the app may read clipboard text (but should not alter it).

## What to look for

- In normal text fields, insertion should succeed.
- In password fields, insertion should be blocked and transcript should be copied to clipboard (with a toast).
- In apps that do not expose UIA patterns, paste/typing fallback should still insert.

## Run tests

- `pnpm -C app test`
- `pnpm -C app cargo:test`
- Preferred CI gate: `pnpm -C app check:ci`
