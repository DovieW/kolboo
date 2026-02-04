# Research: Backend CLI Subcommand

## Decision 1: Use Tauri CLI plugin for subcommands

**Decision**: Implement the CLI as a Tauri subcommand using the Tauri CLI plugin (`tauri-plugin-cli`).
**Rationale**: Tauri v2 supports CLI subcommands via the CLI plugin, allowing arguments to be defined in `tauri.conf.json` and parsed in Rust. This keeps the CLI in the same binary and shares the same runtime/state as the app.
**Alternatives considered**:
- Separate CLI binary: Rejected due to duplicated settings/pipeline logic and extra distribution overhead.
- Custom argument parsing (e.g., clap directly): Rejected because Tauri already provides a plugin for CLI integration and configuration.

## Decision 2: Configure subcommands in `tauri.conf.json`

**Decision**: Define CLI subcommands/args in `tauri.conf.json` under the CLI plugin configuration.
**Rationale**: The Tauri CLI plugin expects subcommand definitions in config, enabling consistent parsing and help output across platforms.
**Alternatives considered**:
- Hardcode args in Rust: Rejected to keep configuration centralized and discoverable.

## Decision 3: JSON output by default, human-readable opt-in

**Decision**: Return machine-readable JSON by default and offer a human-readable format via a flag (e.g., `--human`).
**Rationale**: The CLI is intended for automation; JSON output improves scriptability and integration with other tools.
**Alternatives considered**:
- Human-readable default: Rejected because it complicates automation and parsing.

## Decision 4: Standard exit codes

**Decision**: Use consistent exit codes across commands (e.g., `0` success, `2` validation error, `3` runtime failure).
**Rationale**: Consistent exit codes allow easy scripting and reliable error handling.
**Alternatives considered**:
- One generic failure code: Rejected because it reduces debuggability for automation.
