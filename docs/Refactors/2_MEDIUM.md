# Medium-priority refactors

<!-- Add medium-priority refactor ideas here. Keep each item specific and code-grounded. -->

## Reduce maintenance cost of schema registry

`app/src-tauri/xtask/src/schema_registry.rs` is currently a hand-maintained list of all JSON Schemas we export.

It works fine, but adding/removing a schema means touching a big list, which is easy to forget and can drift over time.

Ideas:

- Use a macro to declare the registry in a more compact "data-only" way.
- Generate the registry from a single source of truth (a small TOML/JSON manifest, or a Rust module that is codegen'd).
- If we ever add many more schemas, consider splitting the registry by domain (settings, commands, events, etc.) and merging them.

## Consolidate context capture + prompt building

Context capture for Quick Ask / Quick Replace is currently spread across multiple places:

- Highlighted-selection capture (key injection + clipboard sentinel) lives in `app/src-tauri/src/text/selection_probe.rs` and is orchestrated via `app/src-tauri/src/sessions/selection_probe.rs`.
- Clipboard "extra context" reading lives in `app/src-tauri/src/clipboard_context.rs`.
- The *final* prompt assembly differs per feature and is mostly inline in `app/src-tauri/src/lib.rs` (Quick Ask has a helper for question+context; Quick Replace builds its user prompt inline).

Ideas:

- Centralize "context sources" (selected text + clipboard context) into a single helper that returns a structured object with consistent size limits and sentinel protection.
- Consider wiring up `ContextGrabMethod::ClipboardOnly` end-to-end (it exists in Rust but isn't currently selectable via the `context_grab_method` string mapping in `lib.rs` / settings docs).
- Move the Quick Replace user-prompt builder into `clipboard_context.rs` (or a new `prompt_builders.rs`) so both Quick Ask + Quick Replace follow the same conventions and are easier to test.
