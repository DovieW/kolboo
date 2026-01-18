# Hotkeys / Shortcuts (Global)

This repo is a **Tauri (Rust) backend + React/TS frontend** desktop app.

Hotkeys are “full stack”:

- **Frontend** renders the hotkey UI and persists values into the Tauri Store (`settings.json`).
- **Backend** registers global shortcuts and routes shortcut events into the recording pipeline.

This guide documents:

- Where hotkeys live (TS + Rust)
- How to change defaults safely
- How to add a new hotkey
- Platform gotchas (especially Windows modifier-only keys like Right Alt / AltGr)

---

## Mental model

There are **two shortcut mechanisms**:

1. **Global shortcut plugin** (cross-platform)

   - Implemented via `tauri-plugin-global-shortcut`.
   - Requires a “real” key (e.g. `F3`, `Ctrl+Space`, media keys, etc).
   - Used by default on macOS/Linux and for non-modifier keys on Windows.

2. **Windows-only modifier-only hotkeys** (Right Alt / AltGr)
   - Implemented via a low-level keyboard hook.
   - File: `app/src-tauri/src/windows_modifier_hotkeys.rs`
   - Forwards events to:
     - `app/src-tauri/src/lib.rs` → `handle_modifier_key_event(...)`
   - This exists because OS-level hotkey APIs (and the Tauri global shortcut plugin) generally
     do **not** support “modifier-only” hotkeys.

A single user hotkey setting may be handled by (1) or (2) depending on platform + key.

---

## Where hotkey settings are stored

These keys live in the store (`settings.json`):

- `toggle_hotkey`: start/stop recording
- `hold_hotkey`: push-to-talk style record
- `paste_last_hotkey`: output/paste last transcription
- `retry_hotkey`: retry the last available recording (re-runs STT/LLM and outputs)

### Semantics (important)

- **Missing key** ⇒ treated as “use default”.
- **Explicit `null`** ⇒ treated as “disabled”.
- **Invalid value** ⇒ treated as “use default” (or disabled in some registration paths).

This “null means explicitly disabled” convention is relied on throughout the app.

---

## Frontend files (TS/React)

### Defaults shown in the UI

- `app/src/lib/hotkeyDefaults.ts`

  - Canonical UI defaults (`DEFAULT_TOGGLE_HOTKEY`, etc).

- `app/src/lib/tauri.ts`
  - Also contains defaults used during settings normalization and “reset to defaults”.
  - Search for:
    - `defaultToggleHotkey`, `defaultHoldHotkey`, `defaultPasteLastHotkey`, `defaultRetryHotkey`

Keep these aligned with backend defaults.

### Hotkey UI

- `app/src/components/HotkeyInput.tsx`

  - Captures key combos via `react-hotkeys-hook`.
  - Also includes the “Special key” dropdown.
  - Note: it temporarily unregisters shortcuts while capturing a new hotkey
    (otherwise global shortcuts can intercept key presses).

  **Special key dropdown note:** the backend shortcut parser is provided by
  `tauri-plugin-global-shortcut` → `global-hotkey`, which only supports a specific set of
  “main key” strings. A few useful examples that *are* supported:

  - `CapsLock`, `NumLock`, `ScrollLock`
  - `PrintScreen`, `Pause`
  - `Insert`, `Delete`, `Home`, `End`, `PageUp`, `PageDown`
  - `F13`–`F24` (for extended keyboards)
  - Media/volume keys like `MediaPlayPause`, `MediaTrackNext`, `MediaTrackPrevious`, `VolumeUp`

  Some requested keys (notably the **Context Menu / Application** key) are **not supported** by the
  current parser, so they cannot be registered as global shortcuts without adding new native handling.

- `app/src/components/settings/HotkeySettings.tsx`
  - Wires inputs to React Query mutations.

### Settings writes + re-register flow

- `app/src/lib/queries.ts`
  - Mutations update store keys via `tauriAPI.update*Hotkey(...)`.
  - Then calls:
    - `tauriAPI.unregisterShortcuts()`
    - `tauriAPI.registerShortcuts()`

That explicit unregister/register sequence is intentional; it keeps registrations consistent.

---

## Backend files (Rust)

### Hotkey type + defaults

- `app/src-tauri/src/settings.rs`
  - `HotkeyConfig` struct
  - Default key constants
  - Default constructors:
    - `HotkeyConfig::default_toggle()`
    - `HotkeyConfig::default_hold()`
    - `HotkeyConfig::default_paste_last()`
    - `HotkeyConfig::default_retry()`

**Platform default:**

- Windows: default toggle is `AltRight` (Right Alt / AltGr)
- Non-Windows: default toggle is `F3`

### Seeding/migrations (first-run + missing keys)

- `app/src-tauri/src/settings/defaults.rs` → `ensure_default_settings(...)`

This runs on startup and seeds **missing** keys into `settings.json` so the UI and backend
agree on what the “effective” settings are.

**Hotkey seeding rule:** hotkeys allow explicit `null` to mean disabled, so seeding should
usually be “only if key is absent”, not “if missing OR null”.

### Global shortcut registration

There are two separate registration paths:

1. Startup registration:

   - `app/src-tauri/src/lib.rs` → `register_initial_shortcuts(...)`

2. Runtime registration (when user changes settings):
   - `app/src-tauri/src/commands/settings.rs` → `register_shortcuts` / `unregister_shortcuts`

### Shortcut event handling

- `app/src-tauri/src/lib.rs` → `handle_shortcut_event(...)`

  - Routes shortcut events to toggle/hold/paste actions.

- Windows modifier-only events:
  - `app/src-tauri/src/lib.rs` → `handle_modifier_key_event(...)`

---

## How to change the default toggle hotkey

Checklist:

1. **Frontend defaults**

   - `app/src/lib/hotkeyDefaults.ts`
   - `app/src/lib/tauri.ts`

2. **Backend defaults**

   - `app/src-tauri/src/settings.rs` (`DEFAULT_TOGGLE_KEY` + `default_toggle()`)

3. **Backend startup registration**

   - `app/src-tauri/src/lib.rs` → `register_initial_shortcuts(...)`
   - If the default is **modifier-only** on Windows (e.g. `AltRight`), ensure startup code:
     - does **not** attempt to register it via the global shortcut plugin
     - does **not** fall back to some legacy default key

4. **Docs/UI copy**
   - The setup guide has a fallback “recommended key” hint:
     - `app/src/components/settings/SettingsGuideOverlay.tsx`

---

## How to add a new hotkey (end-to-end)

Example: add a new store setting `foo_hotkey`.

### Frontend

1. Add setting to `AppSettings`

   - File: `app/src/lib/tauri.ts`

2. Add normalization

   - Implement `normalizeHotkeyConfig(value, fallback)` usage for the new key.

3. Add store update helper

   - `tauriAPI.updateFooHotkey(hotkey: HotkeyConfig | null)` in `app/src/lib/tauri.ts`

4. Add mutation

   - `useUpdateFooHotkey()` in `app/src/lib/queries.ts`
   - Follow the existing pattern:
     - read settings (for duplicate check)
     - write the key
     - unregister/register shortcuts

5. Add UI
   - Add a `HotkeyInput` in an appropriate settings panel.

### Backend

1. Add store key seeding (if needed)

   - `app/src-tauri/src/settings/defaults.rs` → `ensure_default_settings`
   - Decide whether the key should seed defaults when absent only, or also when null.

2. Read it during registration

   - `app/src-tauri/src/commands/settings.rs` → `register_shortcuts`
   - Also update startup registration:
     - `app/src-tauri/src/lib.rs` → `register_initial_shortcuts`

3. Handle the action
   - Extend `handle_shortcut_event(...)` (and `handle_modifier_key_event(...)` if you want
     modifier-only support on Windows).

---

## Gotchas (these are the ones that bite)

### 1) Windows modifier-only keys (AltRight)

- `AltRight` (Right Alt / AltGr) is handled via the Windows hook.
- Do **not** register it with the global shortcut plugin.

If you attempt to parse/register `AltRight` as a Tauri shortcut:

- parsing will fail, and
- if your code “falls back” to a legacy default (like `F3`), you can accidentally register
  **two** working hotkeys.

This exact scenario caused the bug:

> “Right Alt is my toggle recording hotkey, but F3 also toggles recording.”

Fix strategy:

- treat modifier-only hotkeys as **hook-handled** and do not fall back.

#### AltGr / Right Alt reliability notes

On Windows, the physical Right Alt key may come through to the low-level hook as:

- `VK_RMENU` (most common), or
- `VK_MENU` (generic Alt) **with** `LLKHF_EXTENDED` set (right-side Alt).

Some keyboard/input stacks (OEM utilities, RDP/VMs, certain remappers) are more likely to
emit the second form, which historically made `AltRight` appear “dead” on some machines.

Also: on many non-US layouts, Right Alt behaves as **AltGr** (often synthesized as
`Ctrl+Alt` internally). That can be:

- great for typing special characters, but
- awkward as a global “toggle recording” hotkey.

To reduce false positives while typing, the Windows hook suppresses **release-triggered**
actions (toggle/retry/paste/Quick Ask toggle) if it detects any non-modifier key pressed
while Right Alt is held.

#### How to debug in a packaged (release) build

Release builds on Windows typically don’t have a visible console, so `RUST_LOG` output isn’t
easy for end users to collect.

Instead, enable the in-app hotkey diagnostics:

- Set `hotkey_debug_enabled = true` in `settings.json`, or
- Use the UI toggle in **Request Logs → System Events (Live) → Hotkey debug**.

Then reproduce the issue and copy the events from **System Events (Live)**.
You should see entries like:

- `Hotkey debug: RightAlt down/up`
- details containing `vk`, `scan`, `flags`, `extended`, `injected`, and whether the event was
  suppressed due to AltGr-style typing.

### 2) Startup registration vs runtime registration

There are two different registration functions (startup vs runtime). When you change anything
about hotkeys, verify you updated both.

- Startup: `register_initial_shortcuts` (in `lib.rs`)
- Runtime: `commands/settings.rs::register_shortcuts`

If only one is updated, you can get different behavior after restart vs after changing settings.

### 3) `null` is meaningful

Many settings use `null` to mean “explicitly disabled”.

For hotkeys, **do not** seed defaults when the store value is `null` (unless you intend to
remove the ability to disable the hotkey).

### 4) Hotkey capture must temporarily unregister shortcuts

The hotkey capture UI intentionally unregisters shortcuts during capture:

- `HotkeyInput.tsx` calls `tauriAPI.unregisterShortcuts()` when capture begins.

If you add another capture surface, keep this behavior, otherwise the global shortcut handler
may intercept the keystrokes you’re trying to record.

---

## Quick troubleshooting

- “My shortcut still triggers the old key too”

  - Check `register_initial_shortcuts` for a fallback or double registration.
  - Check runtime registration in `commands/settings.rs`.
  - Make sure the UI calls unregister/register after updates.

- “Right Alt doesn’t work on Windows”

  - Confirm `windows_modifier_hotkeys::init(...)` is called at startup.
  - Confirm the store key is exactly `{ modifiers: [], key: "AltRight" }`.

- “HotKey already registered” errors
  - Shortcut registration is serialized via a mutex:
    - `app/src-tauri/src/shortcuts_lock.rs`
  - Ensure you are not registering multiple times concurrently.
