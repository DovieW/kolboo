# Quickstart: Disable Profile Toggle

This is a manual verification guide for the feature.

## Prereqs

- Run the app in dev mode (usual workflow).

## Manual verification steps

### 1) Disable profile toggle

1. Open Settings.
2. Open **Profile config**.
3. Select a non-default profile.
4. Toggle **Disable profile** ON.
5. Confirm:
   - The profile is clearly shown as disabled.
   - The profile remains editable.
   - In the profile selector dropdown, the disabled profile appears greyed out and crossed out.
   - Close and reopen the app; confirm the profile is still disabled.

### 2) Disabled profiles do not activate

1. Ensure the disabled profile would normally match a program (it has `program_paths` set).
2. Bring that program to the foreground.
3. Confirm the pipeline/overlay does not report that profile as active.

### 3) Disabling an active profile deactivates immediately

1. Make a profile active (foreground app matches it).
2. Open Profile config and toggle **Disable profile** ON.
3. Confirm:
   - The active profile updates immediately (clears or changes to fallback).

### 4) Reset profile rename

1. In Profile config, find the button previously labeled “Disable all overrides”.
2. Confirm it is now labeled **Reset profile**.
3. Click it and confirm the dialog text matches “Reset profile” language.
4. Confirm behavior is unchanged:
   - Override fields reset back to inherit/baseline.
   - Program matching remains unchanged.
   - The disabled/enabled state is unchanged.

## Automated checks to run

- `pnpm -C app test`
- `pnpm -C app check:ci`
