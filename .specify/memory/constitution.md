<!--
Sync Impact Report

- Version change: 1.0.0 → 1.0.1
- Modified principles: placeholder principles → Kolboo-specific principles (5)
- Added sections: "Quality Gates" and "Workflow & Review" (filled from placeholders)
- Removed sections: none
- Templates requiring updates:
	- ✅ .specify/templates/plan-template.md
	- ✅ .specify/templates/tasks-template.md
	- ⚠ .specify/templates/spec-template.md (no changes needed; reviewed)
	- ⚠ .specify/templates/checklist-template.md (no changes needed; reviewed)
	- ⚠ .specify/templates/agent-file-template.md (no changes needed; reviewed)
	- ⚠ .specify/templates/commands/*.md (folder not present in this repo)
- Follow-up TODOs: none
-->

# Kolboo Constitution

## Core Principles

### Deterministic tests (no real network)

All automated tests MUST be deterministic and runnable on a fresh machine.

- Tests MUST NOT make real network calls.
- Tests MUST NOT require real API keys by default.
	- If a test truly requires keys/hardware, it MUST be marked as manual/ignored and documented.
- When changing behavior, add/adjust tests that cover the new behavior (prefer unit tests).

Rationale: CI and contributors need reliable, repeatable signal; flakey tests waste time.

### UI ↔ backend contract stays in sync

Kolboo is split across a React/TypeScript UI and a Rust/Tauri backend. Any contract change MUST be applied end-to-end.

- If you add/rename a Tauri command/event, you MUST update:
	- Rust emitters/handlers, and
	- the TS invoke wrappers and TS types.
- Event names/payload shapes MUST match exactly between Rust and UI.

Rationale: mismatched contracts lead to runtime failures that typecheck can’t catch.

### Settings are canonical, migrated, and applied immediately

Settings are persisted and drive runtime behavior.

- `settings.json` (via the Tauri store) is the canonical persisted source of truth.
- `null` often means “explicitly disabled”; missing/invalid values MUST fall back to defaults.
- If you add/rename a setting, you MUST update BOTH:
	- Rust default seeding/migrations, and
	- TypeScript normalization/migrations.
- If a setting affects runtime pipeline behavior, you MUST persist it AND trigger an immediate pipeline config sync.
- If a setting affects overlay windows, you MUST emit a settings change signal so secondary windows refresh.

Rationale: prevents broken upgrades and avoids “needs restart” surprises.

### Secrets never leak

- Never log API keys, tokens, raw authorization headers, or other secrets.
- Store secrets only through the app’s storage mechanisms (don’t hardcode).
- When logging requests/responses, redact sensitive fields.

Rationale: logs and crash reports are easy to share and hard to fully control.

### Tight diffs, strict tooling, CI gate

- Keep changes small and reviewable; avoid drive-by refactors.
- TypeScript strictness MUST remain satisfied (fix types; don’t paper over).
- Formatting/linting MUST stay clean (follow repo tooling; avoid reformatting unrelated files).
- Before merging, changes MUST pass the repo’s CI gate command.

Rationale: fast reviews and predictable main branch health.

## Quality Gates

- No new TypeScript/Rust errors or warnings in touched files.
- Tests:
	- UI-only changes: run the UI unit test suite.
	- Rust-only changes: run Rust tests.
	- Cross-cutting changes: run the combined “CI cares” check.
- Any new background work (recording/pipeline) MUST be guarded by the existing state machine patterns.
- Any change affecting user-facing behavior MUST include a short, plain-language explanation in the PR.

## Workflow & Review

- Prefer incremental commits that each keep the app runnable.
- When you touch both UI and backend, validate the full end-to-end flow (invoke → Rust → event/UI).
- If you notice a refactor that would help but is out of scope, record it in the repo’s refactor docs instead of doing a risky drive-by.
- Documentation updates are part of the work when behavior changes (especially settings, commands, events).

## Governance
- This constitution is the top-level authority for development practices in this repo.
- Any PR review MUST check for constitution compliance (especially MUST rules).
- Amendments:
	1. Make the change in `.specify/memory/constitution.md`.
	2. Include a brief rationale and any migration notes.
	3. Update dependent templates/docs in the same change if they reference the modified rules.
- Versioning policy (semantic):
	- MAJOR: removed/redefined principles in a backward-incompatible way.
	- MINOR: new principle/section or materially expanded governance.
	- PATCH: clarifications/wording/typos with no semantic rule change.

**Version**: 1.0.1 | **Ratified**: 2026-01-25 | **Last Amended**: 2026-01-25
