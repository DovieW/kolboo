# XTask schema export consolidation plan

Date: 2026-01-23

## Why this exists

Today, Kolboo generates JSON Schemas by compiling and running many small Rust binaries in `app/src-tauri/src/bin/export_*_schema.rs`.

This works, but it has two big downsides:

1. **`cargo test` on Windows becomes fragile** because Cargo wants to build *every* binary target as a test harness unless you explicitly opt out. With dozens of schema-export binaries, Windows can hit paging-file mmap failures or simply take forever.
2. **Maintenance overhead**: lots of near-identical “exporter” files and lots of config surface area.

We recently mitigated this by feature-gating schema exporter binaries (so they don’t build during tests). This plan proposes the “clean” long-term approach: **move schema generation into an `xtask` crate** and consolidate the exporters.

## Goals

- Keep schema generation **deterministic** and **cross-platform**.
- Make `cargo test` for the app crate **not build schema exporters**.
- Replace ~N exporter binaries with **one** Rust tool (`xtask`) that exports all schemas.
- Keep existing outputs stable:
  - schemas still written to `app/src-tauri/gen/schemas/*.schema.json`
  - downstream generation (TS types/events) keeps working

## Non-goals

- Changing schema formats or file naming conventions (unless required).
- Reworking the schema/type generation pipeline beyond schema exporting.
- Removing Node scripts entirely (optional later).

## Current state (baseline)

- Exporter sources live in `app/src-tauri/src/bin/export_*_schema.rs`.
- `app/scripts/generate-schemas.mjs`:
  - builds exporter bins
  - runs each exporter executable
  - writes schemas to `app/src-tauri/gen/schemas`

## Proposed design

### High-level

Create a Rust crate named `xtask` that is responsible for developer tooling:

- `cargo run -p xtask -- schemas` (or `cargo run --manifest-path ... -- schemas`)
  - generates all JSON Schema files
  - writes them to `app/src-tauri/gen/schemas`

Then remove:

- the `src/bin/export_*_schema.rs` files
- any Cargo bin declarations / feature gates needed only for those exporters

### Where `xtask` lives

Recommended:

- `app/src-tauri/xtask/` (new crate)
- `app/src-tauri/Cargo.toml` becomes the workspace root **and** remains the app package (Cargo supports a “workspace root with a package”).

This avoids introducing a new top-level Rust workspace at the repo root.

### CLI shape

Use a small CLI framework (or hand-rolled) to keep it easy to extend later:

- `xtask schemas` → generate schemas
- (future) `xtask doctor`, `xtask format`, etc.

### How schemas are produced

Option A (recommended): use existing schema export helpers already in the codebase.

- If there is an existing function like `kolboo_lib::schema_export::print_schema::<T>(name)` or similar, `xtask` should call into it.
- `xtask` owns the mapping:

```text
schema output filename  -> Rust type
pipeline-state-event    -> kolboo_lib::PipelineStateEvent
proxy-settings          -> kolboo_lib::ProxySettings
...
```

This mapping is the “source of truth” for what schemas get generated.

## Step-by-step implementation plan

### Phase 1 — Add `xtask` crate

1. Create `app/src-tauri/xtask/Cargo.toml` and `app/src-tauri/xtask/src/main.rs`.
2. Add dependencies (minimal):
   - `anyhow` (error handling)
   - `clap` (CLI) **or** a tiny hand-rolled arg parser
   - `serde_json` (if needed)
3. Make `xtask` depend on the app library crate by path:
   - `kolboo_lib = { path = ".." }` (or the correct crate name/path)

### Phase 2 — Convert `app/src-tauri` into a workspace root

1. Add a `[workspace]` section to `app/src-tauri/Cargo.toml`:
   - `members = ["xtask"]`
2. Confirm build/test commands still work:
   - `cargo test --manifest-path app/src-tauri/Cargo.toml`

### Phase 3 — Implement `xtask schemas`

1. Implement output directory handling:
   - ensure `app/src-tauri/gen/schemas` exists
2. Implement schema list:
   - define a `const SCHEMAS: &[SchemaSpec]`
3. For each item:
   - generate schema JSON (via `schemars`/existing helper)
   - normalize newlines
   - write to file
4. Print a simple summary:
   - `Generated X schemas.`

### Phase 4 — Update JS scripts to call `xtask`

Option A (minimal change): keep `generate-schemas.mjs`, but instead of compiling/running exporter bins, it just runs the xtask.

- Replace the “build bins + run each exe” logic with:
  - `cargo run -p xtask -- schemas --manifest-path app/src-tauri/Cargo.toml`

Option B (cleaner): delete the Node script and update `app/package.json` to run xtask directly.

Recommended path: **Option A first**, then consider Option B later.

### Phase 5 — Remove schema exporter binaries

1. Delete `app/src-tauri/src/bin/export_*_schema.rs` files.
2. Remove any Cargo bin declarations / feature flags introduced solely for exporting.
3. Ensure schema generation still works and produces the same filenames.

### Phase 6 — Validate contract pipeline

1. Run:
   - `pnpm -C app schemas:generate`
   - `pnpm -C app types:generate`
2. Confirm no unexpected diffs (or update expected diffs deliberately).

## Acceptance criteria

- `pnpm -C app cargo:test` works on Windows without trying to compile schema exporters.
- `pnpm -C app schemas:generate` still produces the schemas under `app/src-tauri/gen/schemas`.
- Schema filenames and contents are unchanged (or changes are explicitly explained and accepted).
- No new required environment variables.

## Risks / gotchas

- **Workspace conversion**: adding `[workspace]` should be safe, but verify commands that pass `--manifest-path` still behave as expected.
- **Crate name/path**: ensure `xtask` depends on the correct library crate (`kolboo_lib`) and not the Tauri binary.
- **Keeping filenames stable**: current naming logic converts `export_foo_bar_schema.rs` → `foo-bar.schema.json`. The xtask should keep the same convention.

## Rollback plan

If `xtask` causes unexpected CI/build issues:

- revert to feature-gated exporter bins
- keep `generate-schemas.mjs` as-is

## Nice-to-have follow-ups

- Replace the big manual schema list with a codegen-assisted list (only if it stays deterministic).
- Move other dev tooling into `xtask` (format checks, simple diagnostics).
