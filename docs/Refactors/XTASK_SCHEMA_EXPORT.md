# Refactor notes: xtask schema export

## Reduce maintenance cost of schema registry

`app/src-tauri/xtask/src/schema_registry.rs` is currently a hand-maintained list of all JSON Schemas we export.

It works fine, but adding/removing a schema means touching a big list, which is easy to forget and can drift over time.

Ideas:

- Use a macro to declare the registry in a more compact “data-only” way.
- Generate the registry from a single source of truth (a small TOML/JSON manifest, or a Rust module that is codegen’d).
- If we ever add many more schemas, consider splitting the registry by domain (settings, commands, events, etc.) and merging them.

## Optional: reduce Clippy warning noise

`pnpm -C app check:ci` passes, but `cargo clippy` emits a lot of warnings.

It might be worth either:

- cleaning up the easy ones over time, or
- choosing a small set of lints to enforce (`-D warnings` for those only), so new warnings don’t get lost in the noise.
