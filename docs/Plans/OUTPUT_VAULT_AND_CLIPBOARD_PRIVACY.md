# Output Vault + clipboard privacy / paste reliability

This document captures:

1. Why **paste output** can be unreliable (sometimes pasting the *previous* clipboard value).
2. Why Kolboo output can **pollute clipboard history** (Windows Win+V and third-party managers).
3. A product/tech proposal: **Output Vault** (aka “Kolboo Clips”) — a first-party, privacy-scoped history of *only Kolboo outputs*.

---

## At a glance (plain English)

- Kolboo’s default output mode sets the system clipboard, presses Ctrl+V/Cmd+V, then restores the previous clipboard.
- Some apps read the clipboard **after** receiving the paste shortcut (lazy paste). If we restore too soon, they paste the old clipboard.
- Windows has an API to mark clipboard content as **not allowed in clipboard history**. This can prevent Win+V history pollution for Kolboo’s temporary clipboard writes.
- No app can reliably prevent **third-party clipboard managers** from seeing clipboard changes.
- The best UX + privacy compromise is an **Output Vault**: an in-app “clipboard” that stores only Kolboo outputs and offers Copy/Paste actions.

---

## Goals

### Product goals

- Make “Paste” output **reliable** across common target apps.
- Reduce accidental privacy leaks by avoiding clipboard-history pollution when the user *didn’t ask* to keep output on the clipboard.
- Offer a fast, delightful way to re-use recent outputs without requiring any third-party clipboard manager.

### Technical goals

- Keep changes **scoped** and low-risk.
- Preserve existing output modes and settings semantics.
- Avoid logging secrets or clipboard contents.

---

## Non-goals

- Building a full replacement for dedicated clipboard managers (CopyQ, Ditto, etc.).
- Preventing third-party clipboard managers from capturing clipboard updates (not realistically enforceable).
- Perfect “never touches clipboard” paste across every app (direct injection alternatives are complex and app-specific).

---

## Current behavior

### Output modes

Source of truth: `app/src-tauri/src/commands/text.rs`

- `paste` (default):
  - Save previous clipboard (text only).
  - Set clipboard to output text.
  - Simulate Ctrl+V/Cmd+V.
  - Restore previous clipboard.

- `paste_and_clipboard`:
  - Set clipboard to output text.
  - Simulate Ctrl+V/Cmd+V.
  - Do **not** restore.

- `clipboard`:
  - Set clipboard to output text.
  - Do **not** paste.

The output mode is read from settings in shortcut flows inside `app/src-tauri/src/lib.rs`.

---

## Problem 1: Paste is sometimes unreliable

### Symptom

> Sometimes it doesn’t paste the current transcript and instead pastes whatever was previously in my clipboard.

### Likely cause

A race between:

- “clipboard write becomes visible/committed” and
- “target app fetches clipboard contents after Ctrl+V”.

In particular, some apps fetch clipboard content lazily (after the key event). If Kolboo restores the clipboard too soon, the target app can paste the restored/previous clipboard value.

### Mitigation implemented

Source of truth: `app/src-tauri/src/commands/text.rs`

- **Clipboard visibility barrier**: after setting text, retry reading clipboard briefly to confirm it contains our text before sending Ctrl+V.
- **Post-paste delay before restore**: wait longer after Ctrl+V before restoring clipboard (Windows-biased delay).
- **Conditional restore**: only restore if clipboard still equals our injected text (don’t clobber user changes).

This is a pragmatic reliability improvement without changing public behavior.

---

## Problem 2: Clipboard history gets polluted

### What users see

- Windows clipboard history (Win+V) includes transcript outputs, even if the clipboard was restored.
- Third-party clipboard managers also capture transcript outputs because they monitor clipboard changes.

### Reality check (limitations)

- We *can* reduce/avoid Windows Win+V history pollution.
- We *cannot* reliably stop third-party clipboard managers from observing clipboard changes.

### Mitigation implemented (Windows)

On Windows, for Kolboo’s transient clipboard writes in `paste` mode, set clipboard content using WinRT APIs with options:

- `IsAllowedInHistory = false`
- `IsRoamable = false`

This prevents Windows clipboard history / roaming from recording those entries.

Notes:

- This only targets the default `paste` mode (restore-after-paste), since that mode is conceptually “temporary clipboard usage”.
- The modes that intentionally keep content on the clipboard (`paste_and_clipboard`, `clipboard`) continue to behave like normal clipboard writes.

---

## Proposal: Output Vault ("Kolboo Clips")

### What it is

An in-app history of **only Kolboo outputs**, separate from the system clipboard.

Think of it as:

- a private clipboard history,
- scoped to Kolboo-generated text,
- with explicit actions to Copy or Paste.

### Why it’s the right solution

- Avoids turning Kolboo into a system clipboard surveillance tool.
- Solves the "I want recent transcripts" need without requiring a third-party manager.
- Lets users opt-in to clipboard writes *only when they click Copy/Paste*.

### UX shape

- A panel (main app window + overlay-friendly) listing recent outputs.
- Each item shows:
  - text (preview)
  - timestamp
  - optional context: app/profile, request type (transcribe/rewrite/quick ask)

Actions per item:

- **Paste** (uses existing output pipeline)
- **Paste + Enter** (respects `output_hit_enter`)
- **Copy** (explicitly writes to clipboard)
- **Pin** (keep in vault)
- **Delete** (remove from vault)

### Hotkey shape (optional)

- A new “Paste Picker” hotkey:
  - Hold hotkey → overlay list pops up → select clip → paste.

This complements the existing “paste last” hotkey.

### Data model

At minimum:

- `id`
- `text`
- `created_at`
- `source` metadata:
  - request id (if any)
  - active profile id/name
  - foreground app path (if available)

### Storage and performance

- Keep a small in-memory ring buffer (fast UI).
- Persist to existing history store (or a dedicated store section).
- Apply retention:
  - max items (e.g., 100–500)
  - max age (e.g., 7/30 days)
  - pinned items exempt.

This should be performant: strings + metadata only.

---

## Option analysis: “Kolboo clipboard manager”

### Option A: Full system clipboard manager

Definition: capture and manage **all** system clipboard changes.

Pros:

- Replaces third-party clipboard managers for some users.

Cons:

- Large privacy/security surface: "records everything you copy".
- Cross-platform edge cases (formats, images/RTF, delayed rendering).
- Hard to outperform dedicated clipboard manager apps.
- Still doesn’t prevent other clipboard managers from running.

Recommendation: **don’t do this** unless Kolboo’s core product direction changes.

### Option B: Output Vault (Kolboo-only)

Pros:

- Solves the actual pain without surveillance.
- Easy to explain and earn trust.
- Fits Kolboo’s workflow (transcribe → rewrite → paste).

Recommendation: **do this**.

---

## Implementation notes / touchpoints

### Key files

- Paste/clipboard injection logic:
  - `app/src-tauri/src/commands/text.rs`

- Shortcut flows selecting output mode:
  - `app/src-tauri/src/lib.rs`

- UI history display (candidate integration points):
  - `app/src/components/HistoryFeed.tsx`
  - `app/src/lib/queries.ts`

### Settings keys involved

- `output_mode` ("paste" | "paste_and_clipboard" | "clipboard")
- `output_hit_enter` (boolean)
- `paste_last_hotkey` (nullable)

### Observability (safe logging)

- Prefer logging counts/timing, not content.
- Example events:
  - clipboard set success/failure
  - clipboard verification retries
  - restore skipped due to clipboard change

---

## Proposed milestones

1. **M0: Reliability + Windows history mitigation**
   - Improve paste reliability (clipboard barrier + delayed restore).
   - Exclude transient clipboard writes from Windows clipboard history.

2. **M1: Output Vault (basic UI)**
   - Show recent outputs (from existing history).
   - Actions: Copy / Paste / Pin / Delete.

3. **M2: Paste Picker hotkey (overlay)**
   - Overlay list + selection.
   - Paste selected output.

4. **M3: Quality and polish**
   - Search, filters, pinned-only view.
   - Retention settings.

---

## Open questions

- Should Output Vault store only *final* outputs, or also intermediate transcripts/rewrite variants?
- How should we handle sensitive outputs?
  - Optional “do not store this output” per session?
  - Quick Ask answers sometimes contain more sensitive content.
- Should “Copy” explicitly warn that clipboard managers may capture the content?

---

## Summary

- We can (and should) improve paste reliability with small timing/verification guards.
- We can reduce Windows Win+V history pollution for transient clipboard writes.
- We cannot fully prevent third-party clipboard managers from seeing clipboard changes.
- The best long-term product approach is an **Output Vault**: a Kolboo-only clipboard history that provides the utility people want without expanding the app’s privacy risk.
