# Implementation Plan: [FEATURE]

**Branch**: `[###-feature-name]` | **Date**: [DATE] | **Spec**: [link]
**Input**: Feature specification from `/specs/[###-feature-name]/spec.md`

**Note**: This template is filled in by the `/speckit.plan` command. See `.specify/templates/plan-template.md` for the execution workflow.

## Summary

[Extract from feature spec: primary requirement + technical approach from research]

## Technical Context

<!--
  ACTION REQUIRED: Replace the content in this section with the technical details
  for the project. The structure here is presented in advisory capacity to guide
  the iteration process.
-->

**Language/Version**: [e.g., Python 3.11, Swift 5.9, Rust 1.75 or NEEDS CLARIFICATION]
**Primary Dependencies**: [e.g., FastAPI, UIKit, LLVM or NEEDS CLARIFICATION]
**Storage**: [if applicable, e.g., PostgreSQL, CoreData, files or N/A]
**Testing**: [e.g., pytest, XCTest, cargo test or NEEDS CLARIFICATION]
**Target Platform**: [e.g., Linux server, iOS 15+, WASM or NEEDS CLARIFICATION]
**Project Type**: [e.g., library/cli/web-service/mobile-app/compiler/desktop-app or NEEDS CLARIFICATION]
**Performance Goals**: [domain-specific, e.g., 1000 req/s, 10k lines/sec, 60 fps or NEEDS CLARIFICATION]
**Constraints**: [domain-specific, e.g., <200ms p95, <100MB memory, offline-capable or NEEDS CLARIFICATION]
**Scale/Scope**: [domain-specific, e.g., 10k users, 1M LOC, 50 screens or NEEDS CLARIFICATION]

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

Document each gate as PASS/FAIL with concise notes. Any FAIL MUST be resolved before
implementation or justified in Complexity Tracking with the rejected simpler alternative.

- **Privacy/secrets**: Sensitive data classified; network/cloud use explicit; logs/diagnostics redacted; approved storage path identified for secrets/session material.
- **Deterministic validation**: Tests/checks can run without real network calls, API keys, paid accounts, audio devices, or timing sleeps; manual tests are ignored/documented.
- **Contract sync**: Tauri commands/events, generated schemas/types, TS wrappers, overlay listeners, and provider/model capability metadata stay synchronized.
- **Settings/migrations/runtime sync**: Setting shape/default changes include Rust seeding/migrations, TS normalization/migrations, persistence, immediate pipeline sync, and settings-change events when applicable.
- **Pipeline safety**: Recording/transcription/cancellation/hotkey/background-work changes preserve explicit state-machine transitions and cleanup/error paths.
- **Tooling/review**: Formatting runs before tests/checks; smallest relevant validation is identified; refactors/docs/changelog updates are planned when applicable.

## Project Structure

### Documentation (this feature)

```text
specs/[###-feature]/
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
  real paths touched by the feature. The delivered plan must not include unused paths.
-->

```text
app/
├── src/                         # React/Vite UI
│   ├── components/              # UI components and settings surfaces
│   ├── lib/
│   │   ├── tauri/               # invoke wrappers, settings, generated types/events
│   │   └── queries.ts           # TanStack Query hooks and UI/backend orchestration
│   └── *.test.ts(x)             # Vitest tests next to code where practical
├── scripts/                     # generation/check helper scripts
└── src-tauri/
    ├── gen/                     # generated schemas/types consumed by checks
    └── src/                     # Rust/Tauri backend, commands, pipeline, tests

docs/
├── How Tos/
├── Refactors/
└── User Docs/

specs/[###-feature]/             # this feature's Spec Kit artifacts
```

**Structure Decision**: [Document the selected structure and reference the real
directories captured above]

## Complexity Tracking

> **Fill ONLY if Constitution Check has violations that must be justified**

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| [e.g., 4th project] | [current need] | [why 3 projects insufficient] |
| [e.g., Repository pattern] | [specific problem] | [why direct DB access insufficient] |
