# Quickstart

## Goal

Verify Quick Ask dismiss behavior per profile and the inline close button.

## Prerequisites

- App builds locally.
- A profile exists (or create one in settings).

## Steps

1. Open settings and locate the Quick Ask dismiss mode dropdown for a profile.
2. Set the mode to **Manual** and save.
3. Open Quick Ask for that profile.
4. Click outside the overlay.
   - **Expected**: The overlay stays open.
5. Click the **X** button in the top-right of the question row.
   - **Expected**: The overlay closes.
   - **Expected**: Overlay height does not increase because of the X button.
6. Set the mode to **Auto** for the same profile.
7. Open Quick Ask again and click outside the overlay.
   - **Expected**: The overlay dismisses.

## Notes

- If the profile has no override set, it should use the default value (**Manual**).
