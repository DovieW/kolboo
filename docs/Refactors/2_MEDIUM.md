# Medium-priority refactors

<!-- Add medium-priority refactor ideas here. Keep each item specific and code-grounded. -->

## Reduce maintenance cost of schema registry

`app/src-tauri/xtask/src/schema_registry.rs` is currently a hand-maintained list of all JSON Schemas we export.

It works fine, but adding/removing a schema means touching a big list, which is easy to forget and can drift over time.

Ideas:

- Use a macro to declare the registry in a more compact “data-only” way.
- Generate the registry from a single source of truth (a small TOML/JSON manifest, or a Rust module that is codegen’d).
- If we ever add many more schemas, consider splitting the registry by domain (settings, commands, events, etc.) and merging them.
