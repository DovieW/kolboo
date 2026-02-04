# Implementation Plan: Backend CLI Subcommand

**Branch**: `001-backend-cli` | **Date**: 2026-02-03 | **Spec**: `specs/001-backend-cli/spec.md`
**Input**: Feature specification from `/specs/001-backend-cli/spec.md`

**Note**: This template is filled in by the `/speckit.plan` command. See `.github/agents/speckit.plan.agent.md` for the execution workflow.

## Summary

Deliver a comprehensive backend CLI as a Tauri subcommand with commands for pipeline run/status, settings get/set, profiles list/use, diagnostics, and configuration export. Use the Tauri CLI plugin to parse subcommands and return structured JSON results by default, with a human-readable option, while sharing the same settings store and pipeline logic as the UI.

## Technical Context

<!--
  ACTION REQUIRED: Replace the content in this section with the technical details
  for the project. The structure here is presented in advisory capacity to guide
  the iteration process.
-->

**Language/Version**: Rust (Tauri v2 backend) + TypeScript (strict) UI
**Primary Dependencies**: Tauri, tauri-plugin-cli, React/Vite
**Storage**: Tauri store (`settings.json`) for persisted settings and profiles
**Testing**: Vitest (`pnpm -C app test`), Rust tests (`pnpm -C app cargo:test`), CI gate (`pnpm -C app check:ci`)
**Target Platform**: Windows desktop (primary), macOS/Linux (secondary)
**Project Type**: Desktop app (Tauri)
**Performance Goals**: Diagnostics complete within ~5 seconds; CLI commands return promptly for scripting
**Constraints**: Non-interactive core commands; machine-readable output by default; offline-capable; no secret logging
**Scale/Scope**: Single-user desktop application; CLI used for automation and support

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- [x] Deterministic tests: no real network calls in tests; no real API keys required by default
- [x] UI↔backend contract: any command/event/type changes are updated in BOTH Rust and TypeScript
- [x] Settings discipline: any settings additions/changes include migrations/normalization and apply immediately at runtime
- [x] Secrets hygiene: no logging of secrets; redact sensitive data in logs
- [x] Tooling gate: plan includes how you’ll keep `pnpm -C app check:ci` green

## Project Structure

### Documentation (this feature)

```text
specs/001-backend-cli/
├── plan.md              # This file (/speckit.plan command output)
├── research.md          # Phase 0 output (/speckit.plan command)
├── data-model.md        # Phase 1 output (/speckit.plan command)
├── quickstart.md        # Phase 1 output (/speckit.plan command)
├── contracts/           # Phase 1 output (/speckit.plan command)
└── tasks.md             # Phase 2 output (/speckit.tasks command - NOT created by /speckit.plan)
```

### Source Code (repository root)
<!--
  ACTION REQUIRED: Replace the placeholder tree below with the concrete layout
  for this feature. Delete unused options and expand the chosen structure with
  real paths (e.g., apps/admin, packages/something). The delivered plan must
  not include Option labels.
-->

```text
app/
├── src/                # React/TypeScript UI
├── src-tauri/src/      # Rust/Tauri backend
└── tests/              # (if present) test helpers, fixtures, etc.

docs/
scripts/
```

**Structure Decision**: Use the existing repo layout: CLI logic will live in `app/src-tauri/src/` (Rust backend), with any shared types in `app/src/lib/tauri/` if the UI needs to invoke or reflect CLI-related changes. Documentation and contracts live under `specs/001-backend-cli/`.

## Complexity Tracking

> **Fill ONLY if Constitution Check has violations that must be justified**

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| None | N/A | N/A |

## Phase 0: Research

Research findings are captured in `specs/001-backend-cli/research.md` and based on Tauri v2 documentation for the CLI plugin. Key decisions include using the Tauri CLI plugin for subcommands, configuring subcommands in `tauri.conf.json`, and returning structured JSON output by default.

## Phase 1: Design & Contracts

### Data Model

Defined in `specs/001-backend-cli/data-model.md` with entities for Command, CommandResult, PipelineRun, Settings, Profile, DiagnosticsReport, and ConfigurationExport.

### CLI Contracts

OpenAPI-style command contract in `specs/001-backend-cli/contracts/cli.openapi.yaml` defining inputs/outputs for pipeline, settings, profiles, diagnostics, and export commands.

### Quickstart

Documented in `specs/001-backend-cli/quickstart.md` with examples for running commands and interpreting outputs.

### Implementation Sketch (for Phase 2)

- Define CLI subcommands/args in `app/src-tauri/tauri.conf.json` under the Tauri CLI plugin.
- Add CLI handler in `app/src-tauri/src/lib.rs` to parse matches and route to command handlers.
- Implement command handlers in a dedicated Rust module (e.g., `app/src-tauri/src/cli/`), keeping output structured and consistent.
- Reuse existing settings/profile/pipeline APIs to avoid divergence from UI behavior.
- Ensure settings changes trigger pipeline config sync and settings-changed events where applicable.
- Add unit tests for command parsing and output formatting; avoid network usage.

## Constitution Check (Post-Design)

- [x] Deterministic tests: planned tests avoid real network calls or API keys.
- [x] UI↔backend contract: any CLI-related changes to settings/events will update Rust + TS layers.
- [x] Settings discipline: CLI will reuse existing settings store and sync pipeline config on updates.
- [x] Secrets hygiene: CLI output excludes secrets and redacts sensitive fields.
- [x] Tooling gate: plan includes running `pnpm -C app check:ci` before merge.
