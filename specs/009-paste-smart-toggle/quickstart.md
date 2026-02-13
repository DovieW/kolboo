# Quickstart: Paste Safety Toggle

## Goal
Add a UI setting (default off) to control smart paste protection and wire it through settings + Windows safety checks.

## Steps
1. **Add setting key**
   - Update Rust defaults (`app/src-tauri/src/settings/defaults.rs`) to seed `output_smart_paste_protection: false`.
   - Add TS normalization in `app/src/lib/tauri/settings.ts` and add the field to `AppSettings` in `app/src/lib/tauri/types.ts`.

2. **Expose setting in UI tab**
   - Add a toggle row in `app/src/components/settings/UiSettings.tsx` with a short description.
   - Create a new `useUpdateOutputSmartPasteProtection` mutation in `app/src/lib/queries.ts` that calls the existing settings update flow.

3. **Apply at runtime (Windows)**
   - Read `output_smart_paste_protection` in output paths (e.g., `app/src-tauri/src/lib.rs` and `app/src-tauri/src/windows_uia/insert.rs`).
   - When **off**, bypass `insert_block_reason`/`allow_insert` checks and attempt paste as normal.
   - When **on**, keep the current safety behavior.

4. **Tests**
   - Add a Rust unit test in `app/src-tauri/src/windows_uia/safety.rs` for the new setting behavior.
   - Add a UI test for settings normalization (if needed) in `app/src/lib/tauri/settings.test.ts`.

## Run
- Format first: `pnpm -C app lint`
- Then tests (smallest relevant): `pnpm -C app test` or `pnpm -C app cargo:test`
- Final gate (before merge): `pnpm -C app check:ci`
